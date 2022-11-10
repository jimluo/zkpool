// #![allow(dead_code)]
// #![allow(unused_variables)]
// #![allow(unused_imports)]

mod node;
mod pool;
mod rpc;
mod shares;

use anyhow::Result;
use clap::Parser;
use dotenv::dotenv;
use pool::Pool;
use std::{net::SocketAddr, thread::sleep, time::Duration};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
	#[clap(default_value = "true", short, long)]
	debug: bool,

	#[clap(default_value = "0.0.0.0:52066", short, long)]
	pub addr: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
	let filter =
		tracing_subscriber::EnvFilter::try_from_default_env()?.add_directive("jsonrpsee".parse()?);

	tracing_subscriber::FmtSubscriber::builder()
		.with_env_filter(filter)
		.with_max_level(tracing::Level::DEBUG)
		.try_init()
		.expect("setting default subscriber failed");

	dotenv().ok();

	let _args = Args::parse();
	let pool = Pool::start().await?;

	tokio::task::block_in_place(move || {
		pool.status();
		sleep(Duration::from_secs(2));
	});

	Ok(())
}
