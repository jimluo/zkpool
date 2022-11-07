use crate::prover::Prover;
// use anyhow::Result;
use jsonrpsee::{
    core::client::{ClientT, Subscription, SubscriptionClientT},
    rpc_params,
    ws_client::{WsClient, WsClientBuilder},
};
use serde_json;
use std::{net::SocketAddr, sync::Arc};
use tokio::time::Duration;
use tracing::{error, info};

#[derive(Clone)] //, Debug)]
pub struct Device {
    prover: Prover, //TODO Arc<Mutex>
    wsclient: Arc<WsClient>,
    address: String,
    token: String,
}

impl Device {
    pub async fn connect(server: SocketAddr, address: &str, token: &str, count_prove: u32, degree_prove: u32) -> Self {
        let url = format!("ws://{}", server);
        let wsclient = WsClientBuilder::default().build(&url).await.unwrap();

        let device = Device {
            prover: Prover::new(count_prove, degree_prove),
            wsclient: Arc::new(wsclient),
            address: address.to_string(),
            token: token.to_string(),
        };

        device.connect_server().await;

        device
    }

    // upstream server
    async fn connect_server(&self) {
        let agent = format!("{}-{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        let protocol = "AleoStratum-1.0.0";
        let params = rpc_params![agent, protocol, self.address.as_str(), self.token.as_str()];

        let d = self.clone();
        let mut subscribe: Subscription<String> = self
            .wsclient
            .subscribe("subscribe", params, "unsubscribe")
            .await
            .unwrap();

        tokio::spawn(async move {
            while let Some(Ok(msg)) = subscribe.next().await {
                d.parse_message_to_miner(msg).await;
            }
        });
    }

    // downstream miner
    pub async fn prove(&self) {
        let mut d = self.clone();
        while d.prover.next_prove().is_ok() {
            info!("next_prove ok");

            let params = rpc_params![self.address.clone(), self.prover.prove_count_per_block];
            let isok: bool = self.wsclient.request("submit", params).await.unwrap_or(false);
            if !isok {
                error!("Fail on send shares to server");
            }
        }
    }

    async fn parse_message_to_miner(&self, msg: String) {
        info!("subscription with one param: {:?}", msg);

        let result: Vec<String> = serde_json::from_str(&msg).unwrap_or_default();
        if result.len() < 1 {
            error!("Faile on receiving server response");
            return;
        }

        // let mut miner = self.miner.clone();
        // match result[0] {
        //     "target" =>  miner.new_target(target),
        //     "job" => miner.new_job(job),
        //     _ => tracing::error!("match error {}", ws[0]),
        // }
    }

    pub async fn prove_status(&self) {
        let mut miner = self.prover.clone();
        tokio::spawn(async move {
            loop {
                miner.status_prove_shares();
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }
}
