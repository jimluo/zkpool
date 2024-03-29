use anyhow::Result;
use once_cell::race::OnceBox;
use std::sync::Arc;
use tracing::debug;

use crate::{
	node::{Node, NodeHandler},
	rpc::Rpc,
	shares::{Shares, SharesHandler},
};

#[derive(Debug)]
pub struct Services {
	rpc: OnceBox<Rpc>,
	node: OnceBox<Node>,
	shares: OnceBox<Shares>,
}

impl Services {
	pub fn new() -> Self {
		Services { rpc: Default::default(), node: Default::default(), shares: Default::default() }
	}

	pub fn rpc(&self) -> &Rpc {
		self.rpc.get().unwrap()
	}

	pub fn node(&self) -> &Node {
		self.node.get().unwrap()
	}

	pub fn shares(&self) -> &Shares {
		self.shares.get().unwrap()
	}

	pub async fn initialize_rpc(&self, rpc: Rpc) {
		self.rpc.set(rpc.into()).map_err(|_| ()).unwrap();
		self.rpc().initialize().await;
	}

	pub async fn initialize_node(&self, node: Node, mut handler: NodeHandler) {
		self.node.set(node.into()).map_err(|_| ()).unwrap();

		if let Err(error) = self.node().initialize().await {
			debug!("Fail on initial node: {}", error);
		}
		let node = self.node().clone();
		tokio::spawn(async move {
			while let Some(request) = handler.recv().await {
				debug!("handler.recv() into node");
				node.update(request).await;
			}
		});
	}

	pub async fn initialize_shares(&self, shares: Shares, mut handler: SharesHandler) {
		self.shares.set(shares.into()).map_err(|_| ()).unwrap();

		let mut shares = self.shares().clone();
		tokio::spawn(async move {
			while let Some(request) = handler.recv().await {
				debug!("handler.recv() into shares");
				shares.update(request).await;
			}
		});
	}
}

#[derive(Clone, Debug)]
pub struct Pool {
	services: Arc<Services>,
}

impl Pool {
	pub async fn start() -> Result<Self> {
		debug!("Pool server starting");

		let services = Arc::new(Services::new());

		let (shares, shares_handler) = Shares::new(services.clone()).await?;
		let (node, node_handler) = Node::new(services.clone()).await?;
		let rpc = Rpc::new(services.clone()).await;

		services.initialize_shares(shares, shares_handler).await;
		services.initialize_node(node, node_handler).await;
		services.initialize_rpc(rpc).await;

		Ok(Pool { services })
	}

	pub fn status(&self) {
		println!("{} {}", self.services.rpc().status(), self.services.shares().status());
	}
}
