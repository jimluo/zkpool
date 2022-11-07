mod device;
mod prover;

use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(default_value = "127.0.0.1:52066", short, long)]
    pub server: SocketAddr,

    #[clap(
        default_value = "aleo15te0tcennt8lmrd4e0wjvgwr775cpq5rcsvws0ckgenqx6aaxv8sjlgg9a",
        short,
        long = "account-address"
    )]
    address: String,

    #[clap(default_value = "xxx", short, long = "api-key")]
    pub token: String,

    #[clap(default_value = "100", short, long)]
    pub count_prove: u32,

    #[clap(default_value = "4", short, long)]
    pub degree_prove: u32,
    // #[arg(short, long, default_value_t = true)]
    // debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()?
        // .add_directive("jsonrpsee[method_call{name = \"authorize\"}]=trace".parse()?);
        .add_directive("jsonrpsee[method_call{}]=trace".parse()?);

    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_max_level(tracing::Level::DEBUG)
        .finish()
        .try_init()
        .expect("setting default subscriber failed");

    let args = Args::parse();
    let device = device::Device::connect(
        args.server,
        &args.address,
        &args.token,
        args.count_prove,
        args.degree_prove,
    )
    .await;
    device.prove_status().await;
    device.prove().await;

    Ok(())
}

#[cfg(test)]
mod tests {
    // use futures::StreamExt;
    // use jsonrpsee::{core::error::SubscriptionClosed, server::ServerBuilder, RpcModule};
    // use tokio::time::interval;
    // use tokio_stream::wrappers::IntervalStream;
    // use std::net::SocketAddr;
    // pub async fn run_server(addr: &str) -> Result<SocketAddr> {
    //     let server = ServerBuilder::default().build(addr).await?;
    //     let mut module = RpcModule::new(());
    //     module
    //         .register_subscription("subscribe", "subscribe", "unsubscribe", |_params, mut sink, _| {
    //             let interval = interval(Duration::from_secs(6));
    //             let stream = IntervalStream::new(interval).map(move |i| {
    //                 if i.elapsed().as_secs() % 2 ==0 {
    //                     "target, 11"
    //                 } else {
    //                     "job, 10, 111, 123456789cba, 9"
    //                 }
    //             });

    //             tokio::spawn(async move {
    //                 if let SubscriptionClosed::Failed(err) = sink.pipe_from_stream(stream).await {
    //                     sink.close(err);
    //                 }
    //             });
    //             Ok(())
    //         })
    //         .unwrap();

    //     module.register_method("submit", |_, _| Ok(true))?;

    //     let addr = server.local_addr()?;
    //     let handle = server.start(module)?;

    //     tokio::spawn(handle.stopped()); //ServerHandle

    // 	Ok(addr)
    // }

    #[tokio::test]
    async fn client_should_work() {
        // mockserver::run_server(&args.address).await?;
    }
}
