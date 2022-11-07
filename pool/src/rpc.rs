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
// use snarkvm::prelude::{Address, Testnet3};
use std::{
    collections::{ HashSet},
    env,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::{sync::mpsc};
use tracing::{debug, warn, error};

use crate::{node::NodeRequest, pool::Services};

pub type RpcRouter = mpsc::Sender<RpcRequest>; //upstream channel
pub type RpcHandler = mpsc::Receiver<RpcRequest>; //downstrean channel
// pub type BlockHash = <Testnet3 as Network>::BlockHash;

pub enum RpcRequest {
    // NewShare(String, u64),
    // NewBlock(u32, BlockHash, i64),
    // SetTarget(u64),
    // Exit,
}

#[derive(Default, Debug)]
struct CounterInner {
    pub(super) requests: u32,
    pub(super) clients: HashSet<SocketAddr>,
    // client_addrs: HashMap<String, HashSet<SocketAddr>>,
}

#[derive(Clone, Debug)]
pub struct Rpc {
    inner: Arc<Mutex<CounterInner>>,
    rpc_router: RpcRouter, // send to node
    services: Arc<Services>,
}
// TODO: broadcast target and job
// TODO: channel recevie pool job and send broadcast to devices
impl Rpc {
    pub async fn new(services: Arc<Services>) -> Result<(Self, mpsc::Receiver<RpcRequest>)> {
        debug!("Rpc starting");

        let (rpc_router, rpc_handler) = mpsc::channel(1024);
        let inner = CounterInner {
            requests: 0,
            clients: HashSet::new(), //<SocketAddr>,
                                     // client_addrs: HashMap::new(), // <String, HashSet<SocketAddr>>,
        };

        let rpc = Self {
            inner: Arc::new(Mutex::new(inner)),
            rpc_router,
            services,
        };

        Ok((rpc, rpc_handler))
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
            .register_subscription("subscribe", "subscribe", "unsubscribe", move |params, mut sink, _| {
                let result = rpc2.on_subscribe(params);

                let stream = futures_util::stream::iter(result);
                tokio::spawn(async move {
                    if let SubscriptionClosed::Failed(err) = sink.pipe_from_stream(stream).await {
                        sink.close(err);
                    }
                });
                Ok(())
            })
            .expect("Fail on register method subscribe");

        let addr = ws.local_addr().unwrap();
        let handle = ws.start(module).expect("Fail on Websocket Server starting");
        tokio::spawn(handle.stopped()); //ServerHandle

        debug!("Websocket Server starting at {}", addr);
    }

    /// call by device rpc request submit new share
    pub fn on_submit(&self, params: Params) {
        if let Ok((address, count)) = params.parse::<(String, u64)>() {
            debug!("Websocket on_submit at {} {}", address, count);

            let req = NodeRequest::NewBlock(address, count);
            let node = self.services.node().router().clone();
            tokio::spawn(async move {
                if let Err(error) = node.send(req).await {
                    warn!("[NodeRequest] {}", error);
                }
                debug!("send node request nwe block");
            });
        }
    }

    /// call by device rpc request subscribe authen
    pub fn on_subscribe(&self, _: Params) -> Result<String> {
        let nonce = env::var("POOL_NONCE").unwrap_or_else(|_| "NONCE".to_string());
        let address = env::var("POOL_ADDRESS")
            .unwrap_or_else(|_| "aleo15te0tcennt8lmrd4e0wjvgwr775cpq5rcsvws0ckgenqx6aaxv8sjlgg9a".to_string());

        let resp = format!("[\"subscribe\", \"{nonce}\", \"{address}\"]");

        Ok(resp)
    }

    /// Returns an instance of the operator router.
    pub fn router(&self) -> &RpcRouter {
        &self.rpc_router
    }

    /// Performs the given `request` to the rpc server.
    /// All requests must go through this `update`, so that a unified view is preserved.
    pub(super) async fn update(&self, _request: RpcRequest) {
        let req = NodeRequest::NewBlock("address".into(), 3);
        let node = self.services.node().router().clone();
        tokio::spawn(async move {
            if let Err(error) = node.send(req).await {
                error!("[NodeRequest] {}", error);
            }
        });
    }
}

//  on_connect - on_request/on_call/on_result/on_response
impl logger::Logger for Rpc {
    type Instant = Instant;

    fn on_connect(&self, remote_addr: SocketAddr, _request: &HttpRequest, t: TransportProtocol) {
        self.inner.lock().unwrap().clients.insert(remote_addr);
        debug!("on_connect: {} {}", remote_addr, t);
    }

    fn on_request(&self, _t: TransportProtocol) -> Self::Instant {
        self.inner.lock().unwrap().requests += 1;
        // debug!("on_request: {}", t);

        Instant::now()
    }

    fn on_disconnect(&self, remote_addr: SocketAddr, t: TransportProtocol) {
        self.inner.lock().unwrap().clients.remove(&remote_addr);
        debug!("on_disconnect: {} {}", remote_addr, t);
    }

    fn on_call(&self, _method: &str, _: Params<'_>, _: MethodKind, _t: TransportProtocol) {
        // debug!("on_call: {} {}", method, t);
    }

    fn on_result(&self, _method: &str, _: bool, _: Instant, _t: TransportProtocol) {
        // debug!("on_result: {} {}", method, t);
    }

    fn on_response(&self, _method: &str, _: Instant, _t: TransportProtocol) {
        // debug!("on_response: {} {}", method, t);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        pool::Services,
        rpc::{Rpc},
    };
    use dotenv::dotenv;
    use jsonrpsee::{
        core::{
            client::{ClientT, Subscription, SubscriptionClientT},
            traits::ToRpcParams,
        },
        rpc_params,
        ws_client::{WsClient, WsClientBuilder},
    };
    use std::{ env, sync::Arc};
    use tracing_subscriber::FmtSubscriber;

    async fn create_client_start_server() -> (Rpc, WsClient) {
        //     let filter =
        //     tracing_subscriber::EnvFilter::try_from_default_env()?.add_directive("jsonrpsee[method_call{}]=trace".parse()?);
        // .with_env_filter(filter)

        let _ = FmtSubscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .try_init();

        dotenv().ok();

        let services = Arc::new(Services::new());
        let (rpc, rpc_handler) = Rpc::new(services.clone()).await.unwrap();
        services.initialize_rpc(rpc.clone(), rpc_handler).await;

        assert!(rpc.inner.lock().unwrap().clients.len() == 0);

        let port = env::var("RPC_PORT").unwrap_or_else(|_| "52066".to_string());
        let addr = format!("ws://127.0.0.1:{}", port);

        let wsclient = WsClientBuilder::default()
            .build(&addr)
            .await
            .expect("Fail on WsClientBuilder");

        (rpc, wsclient)
    }

    #[tokio::test]
    async fn response_subscribe_submit_should_work() {
        let (rpc, wsclient) = create_client_start_server().await;
        assert!(rpc.inner.lock().unwrap().requests == 0);
        assert!(rpc.inner.lock().unwrap().clients.len() == 1);

        let params = rpc_params!["aleodevice-0.1.0", "AleoStratum-1.0.0", "name", "token"];
        let mut subscribe: Subscription<String> = wsclient
            .subscribe("subscribe", params, "unsubscribe")
            .await
            .expect("Fail on wsclient.subscribe");

        let msg = subscribe.next().await.unwrap().unwrap();
        let result: Vec<String> = serde_json::from_str(&msg).unwrap();

        let nonce = env::var("POOL_NONCE").unwrap_or_else(|_| "NONCE".to_string());
        let address = env::var("POOL_ADDRESS")
            .unwrap_or_else(|_| "aleo15te0tcennt8lmrd4e0wjvgwr775cpq5rcsvws0ckgenqx6aaxv8sjlgg9a".to_string());

        assert!("subscribe" == result[0]);
        assert!(nonce == result[1]);
        assert!(address == result[2]);
        assert!(rpc.inner.lock().unwrap().requests == 1);

        let params = rpc_params![address, 100];
        let isok: bool = wsclient.request("submit", params).await.unwrap();
        assert!(isok);
        assert!(rpc.inner.lock().unwrap().requests == 2);

        let params = rpc_params!["abc", 100];
        let params2 = rpc_params!["abc", 100];
        let pp = params.to_rpc_params().unwrap().unwrap();
        debug!("==============={:?}==============={:?}", params2, pp);
    }
}
