use anyhow::Result;
use jsonrpsee::{
	core::error::SubscriptionClosed,
	server::{
		logger::{self, HttpRequest, MethodKind, TransportProtocol},
		ServerBuilder,
	},
	types::Params,
	RpcModule,
};
use std::{
	collections::HashMap,
	env,
	net::SocketAddr,
	sync::{Arc, Mutex},
	time::Instant,
};
use tracing::{debug, error, warn};

use crate::{node::NodeRequest, pool::Services, shares::SharesRequest};

#[derive(Default, Debug)]
struct CounterInner {
	// pub(super) requests: u32,
	// pub(super) clients: HashSet<SocketAddr>,
	pub(super) clients: HashMap<SocketAddr, (String, String)>, // (workername, apikey)
	current_socketaddr: Option<SocketAddr>,
}

#[derive(Clone, Debug)]
pub struct Rpc {
	inner: Arc<Mutex<CounterInner>>,
	services: Arc<Services>,
}

impl Rpc {
	pub async fn new(services: Arc<Services>) -> Self {
		debug!("Rpc starting");

		let inner = CounterInner { clients: HashMap::new(), current_socketaddr: None };

		Self { inner: Arc::new(Mutex::new(inner)), services }
	}

	pub fn status(&self) -> String {
		format!("connected client count={}", self.inner.lock().unwrap().clients.len())
	}

	pub async fn initialize(&self) {
		let port = env::var("RPC_PORT").unwrap_or_else(|_| "52066".to_string());
		let addr = format!("0.0.0.0:{}", port);

		let ws = ServerBuilder::default()
			.max_request_body_size(1024 * 1024)
			.max_response_body_size(100 * 1024)
			.max_connections(1000)
			.set_logger(self.clone())
			.build(addr)
			.await
			.expect(format!("Fail on initialize RPC WsServer at port {}", port).as_str());

		let (rpc1, rpc2) = (self.clone(), self.clone());
		let mut module = RpcModule::new(());
		module
			.register_method("submit", move |params, _| {
				rpc1.on_submit(params);
				return Ok(true);
			})
			.expect("Fail on register method submit");
		module
			.register_subscription(
				"subscribe",
				"subscribe",
				"unsubscribe",
				move |params, mut sink, _| {
					let result = rpc2.on_subscribe(params);

					let stream = futures_util::stream::iter(result);
					tokio::spawn(async move {
						if let SubscriptionClosed::Failed(err) = sink.pipe_from_stream(stream).await
						{
							sink.close(err);
						}
					});
					Ok(())
				},
			)
			.expect("Fail on register method subscribe");

		let addr = ws.local_addr().unwrap();
		let handle = ws.start(module).expect("Fail on Websocket Server starting");
		tokio::spawn(handle.stopped()); //ServerHandle

		debug!("Websocket Server starting at {}", addr);
	}

	/// call by device rpc request submit new share
	pub fn on_submit(&self, params: Params) {
		if let Ok((workername, apikey, count)) = params.parse::<(String, String, u64)>() {
			debug!("Websocket on_submit at {workername} {apikey} {count}");

			let req = NodeRequest::NewBlock(workername, apikey, count);
			let node = self.services.node().router().clone();
			tokio::spawn(async move {
				if let Err(error) = node.send(req).await {
					warn!("Fail on on_submit() NodeRequest, {}", error);
				}
				debug!("send node request nwe block");
			});
		}
	}

	/// call by device rpc request subscribe authen
	pub fn on_subscribe(&self, params: Params) -> Result<String> {
		match params.parse::<(String, String, String, String)>() {
			Ok((_agent, _protocol, workername, apikey)) => {
				if let Some(remote_addr) = self.inner.lock().unwrap().current_socketaddr {
					self.inner
						.lock()
						.unwrap()
						.clients
						.insert(remote_addr, (workername.clone(), apikey.clone()));
					self.on_client_connet(workername, apikey, true);
				}

				let nonce = env::var("POOL_NONCE").unwrap_or_else(|_| "NONCE".to_string());
				let address = env::var("POOL_ADDRESS").unwrap_or_else(|_| {
					"aleo15te0tcennt8lmrd4e0wjvgwr775cpq5rcsvws0ckgenqx6aaxv8sjlgg9a".to_string()
				});

				let resp = format!("[\"subscribe\", \"{nonce}\", \"{address}\"]");

				Ok(resp)
			},
			Err(e) => {
				error!("Failed on_subscribe params.parse: {}", e);
				Err(e.into())
			},
		}
	}

	/// call by device rpc request submit new share
	pub fn on_client_connet(&self, workername: String, apikey: String, is_connected: bool) {
		let req = SharesRequest::Connect(workername, apikey, is_connected);
		let shares = self.services.shares().router().clone();
		tokio::spawn(async move {
			if let Err(error) = shares.send(req).await {
				error!("Fail on on_client_connet() SharesRequest, {}", error);
			}
		});
	}
}

//  on_connect - on_request/on_call/on_result/on_response
impl logger::Logger for Rpc {
	type Instant = Instant;

	fn on_connect(&self, remote_addr: SocketAddr, _request: &HttpRequest, _t: TransportProtocol) {
		// debug!("on_connect: {} {}", remote_addr, t);
		self.inner.lock().unwrap().current_socketaddr = Some(remote_addr);
	}

	fn on_request(&self, _t: TransportProtocol) -> Self::Instant {
		// self.inner.lock().unwrap().requests += 1;
		Instant::now()
	}

	fn on_disconnect(&self, remote_addr: SocketAddr, t: TransportProtocol) {
		debug!("on_disconnect: {} {}", remote_addr, t);

		if let Some((workername, apikey)) = self.inner.lock().unwrap().clients.get(&remote_addr) {
			self.inner.lock().unwrap().clients.remove(&remote_addr);
			self.on_client_connet(workername.to_string(), apikey.to_string(), false);
		}
	}

	fn on_call(&self, _method: &str, _: Params<'_>, _: MethodKind, _t: TransportProtocol) {}

	fn on_result(&self, _method: &str, _: bool, _: Instant, _t: TransportProtocol) {}

	fn on_response(&self, _method: &str, _: Instant, _t: TransportProtocol) {}
}

#[cfg(test)]
mod tests {
	use crate::{pool::Services, rpc::Rpc};
	use dotenv::dotenv;
	use jsonrpsee::{
		core::{
			client::{ClientT, Subscription, SubscriptionClientT},
			traits::ToRpcParams,
		},
		rpc_params,
		ws_client::{WsClient, WsClientBuilder},
	};
	use std::{env, sync::Arc};
	use tracing_subscriber::FmtSubscriber;

	async fn create_client_start_server() -> (Rpc, WsClient) {
		//     let filter =
		//     tracing_subscriber::EnvFilter::try_from_default_env()?.add_directive("jsonrpsee[method_call{}]=trace".parse()?);
		// .with_env_filter(filter)

		let _ = FmtSubscriber::builder().with_max_level(tracing::Level::DEBUG).try_init();

		dotenv().ok();

		let services = Arc::new(Services::new());
		let rpc = Rpc::new(services.clone()).await;

		assert!(rpc.inner.lock().unwrap().clients.len() == 0);

		let port = env::var("RPC_PORT").unwrap_or_else(|_| "52066".to_string());
		let addr = format!("ws://127.0.0.1:{}", port);

		let wsclient =
			WsClientBuilder::default().build(&addr).await.expect("Fail on WsClientBuilder");

		(rpc, wsclient)
	}

	#[tokio::test]
	async fn response_subscribe_submit_should_work() {
		let (rpc, wsclient) = create_client_start_server().await;
		// assert!(rpc.inner.lock().unwrap().requests == 0);
		assert!(rpc.inner.lock().unwrap().clients.len() == 1);

		let params = rpc_params!["aleodevice-0.1.0", "AleoStratum-1.0.0", "name", "token"];
		let mut subscribe: Subscription<String> = wsclient
			.subscribe("subscribe", params, "unsubscribe")
			.await
			.expect("Fail on wsclient.subscribe");

		let msg = subscribe.next().await.unwrap().unwrap();
		let result: Vec<String> = serde_json::from_str(&msg).unwrap();

		let nonce = env::var("POOL_NONCE").unwrap_or_else(|_| "NONCE".to_string());
		let address = env::var("POOL_ADDRESS").unwrap_or_else(|_| {
			"aleo15te0tcennt8lmrd4e0wjvgwr775cpq5rcsvws0ckgenqx6aaxv8sjlgg9a".to_string()
		});

		assert!("subscribe" == result[0]);
		assert!(nonce == result[1]);
		assert!(address == result[2]);
		// assert!(rpc.inner.lock().unwrap().requests == 1);

		let params = rpc_params![address, 100];
		let isok: bool = wsclient.request("submit", params).await.unwrap();
		assert!(isok);
		// assert!(rpc.inner.lock().unwrap().requests == 2);

		let params = rpc_params!["abc", 100];
		let params2 = rpc_params!["abc", 100];
		let pp = params.to_rpc_params().unwrap().unwrap();
		tracing::debug!("==============={:?}==============={:?}", params2, pp);
	}
}
