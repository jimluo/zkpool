mod device;
mod prover;

use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use tracing_subscriber::util::SubscriberInitExt;

use std::ffi::OsString;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
	#[arg(short, long, default_value = "127.0.0.1:52066")]
	server: SocketAddr,

	#[arg(short, long, default_value_t = hostname())]
	//, default_value = hostname()?.into_string().unwrap_or("unknown".into()))]
	workername: String,

	#[arg(short, long, default_value = "aleozkpmarlin")]
	apikey: String,

	#[arg(short, long, default_value_t = 100)]
	count_proves_block: u32,

	#[arg(short, long, default_value_t = 4)]
	prove_degree: u32,

	#[arg(short, long, default_value_t = true)]
	debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
	let args = Args::parse();

	if args.debug {
		let filter = tracing_subscriber::EnvFilter::try_from_default_env()?
			.add_directive("jsonrpsee[method_call{}]=trace".parse()?);

		tracing_subscriber::FmtSubscriber::builder()
			.with_env_filter(filter)
			.with_max_level(tracing::Level::DEBUG)
			.finish()
			.try_init()
			.expect("setting default subscriber failed");
	}

	let device = device::Device::connect(
		args.server,
		args.workername,
		args.apikey,
		args.count_proves_block,
		args.prove_degree,
	)
	.await;

	device.prove_status().await;
	device.prove().await;

	Ok(())
}

#[cfg(any(unix))]
fn hostname() -> String {
	use libc;
	use std::os::unix::ffi::OsStringExt;

	let size = unsafe { libc::sysconf(libc::_SC_HOST_NAME_MAX) as libc::size_t };
	let mut buffer = vec![0u8; size];

	let result = unsafe { libc::gethostname(buffer.as_mut_ptr() as *mut libc::c_char, size) };

	if result != 0 {
		// return Err(io::Error::last_os_error());
		return "unknown".into();
	}

	let end = bytes.iter().position(|&byte| byte == 0x00).unwrap_or_else(|| bytes.len());
	bytes.resize(end, 0x00);

	OsString::from_vec(bytes).into_string().unwrap_or("unknown".into())
}

#[cfg(target_os = "windows")]
fn hostname() -> String {
	use std::os::windows::ffi::OsStringExt;
	use winapi::um::sysinfoapi::{
		ComputerNamePhysicalDnsHostname as Hostname, GetComputerNameExW as GetName,
	};

	let mut size = 0;
	unsafe {
		let result = GetName(Hostname, std::ptr::null_mut(), &mut size);
		debug_assert_eq!(result, 0);
	};

	let mut buffer = Vec::with_capacity(size as usize);
	let result = unsafe { GetName(Hostname, buffer.as_mut_ptr(), &mut size) };

	if result == 0 {
		// Err(std::io::Error::last_os_error().into());

		"unknown".into()
	} else {
		unsafe {
			buffer.set_len(size as usize);
		}

		OsString::from_wide(&buffer).into_string().unwrap_or("unknown".into())
	}
}
