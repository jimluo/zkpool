use crate::pool::Services;
use anyhow::Result;
use deadpool_postgres::{
	ClientWrapper, Config, Hook, HookError, HookErrorCause, Manager, ManagerConfig, Pool,
	RecyclingMethod, Runtime,
};
use snarkvm::prelude::{Network, Testnet3};
use std::{env, sync::Arc};
use tokio::sync::mpsc;
use tokio_postgres::NoTls;
use tracing::{debug, error, info};

pub type SharesRouter = mpsc::Sender<SharesRequest>; //upstream channel
pub type SharesHandler = mpsc::Receiver<SharesRequest>; //downstrean channel
pub type BlockHash = <Testnet3 as Network>::BlockHash;

#[derive(Clone, Debug)]
pub enum SharesRequest {
	NewBlock(u32, BlockHash, String, String), //height, block_hash, workername, apikey
	Connect(String, String, bool),            // (workername, apikey, is_connected)
}

#[derive(Clone, Debug)]
// #[allow(clippy::type_complexity)]
pub struct Shares {
	connection_pool: deadpool_postgres::Pool,
	block_count: usize,
	shares_router: SharesRouter,
}

impl Shares {
	pub async fn new(_services: Arc<Services>) -> Result<(Self, mpsc::Receiver<SharesRequest>)> {
		tracing::debug!("Shares starting");

		let connection_pool = Shares::initialize_db();

		let (shares_router, shares_handler) = mpsc::channel(1024);
		let shares = Self { connection_pool, block_count: 0, shares_router };

		Ok((shares, shares_handler))
	}

	pub fn status(&self) -> String {
		format!("block count {}", self.block_count)
	}

	pub fn router(&self) -> &SharesRouter {
		&self.shares_router
	}

	pub(super) async fn update(&mut self, request: SharesRequest) {
		debug!("receive new block, save_db");
		match request {
			SharesRequest::NewBlock(height, block_hash, workername, apikey) => {
				match Shares::save_db_block_shares_tables(
					&self.connection_pool,
					height,
					block_hash.to_string(),
					workername,
					apikey,
				)
				.await
				{
					Ok(id) => {
						self.block_count += 1;
						info!("Recorded block id={id}, count={}", self.block_count);
					},
					Err(e) => error!("Failed to save block reward : {}", e),
				}
			},
			SharesRequest::Connect(workername, apikey, is_connect) => {
				if let Err(e) = Shares::save_db_worker_tables(
					&self.connection_pool,
					workername,
					is_connect,
					apikey,
				)
				.await
				{
					error!("Failed to save worker: {}", e);
				}
			},
		}
	}

	fn initialize_db() -> Pool {
		debug!("db starting...");

		let mut cfg = Config::new();

		cfg.host = Some(env::var("DB_HOST").expect("No database host defined"));
		cfg.port = Some(
			env::var("DB_PORT")
				.unwrap_or_else(|_| "5432".to_string())
				.parse::<u16>()
				.expect("Invalid database port"),
		);
		cfg.dbname = Some(env::var("DB_DATABASE").expect("No database name defined"));
		cfg.user = Some(env::var("DB_USER").expect("No database user defined"));
		cfg.password = Some(env::var("DB_PASSWORD").expect("No database password defined"));
		let schema = env::var("DB_SCHEMA").unwrap_or_else(|_| {
			tracing::warn!("Using schema public as default");
			"public".to_string()
		});

		cfg.manager = Some(ManagerConfig { recycling_method: RecyclingMethod::Verified });

		let pool = Pool::builder(Manager::from_config(
			cfg.get_pg_config().expect("Invalid database config"),
			NoTls,
			cfg.get_manager_config(),
		))
		.config(cfg.get_pool_config())
		.post_create(Hook::async_fn(move |client: &mut ClientWrapper, _| {
			let schema = schema.clone();
			Box::pin(async move {
				client
					.simple_query(&*format!("set search_path = {}", schema))
					.await
					.map_err(|e| HookError::Abort(HookErrorCause::Backend(e)))?;
				Ok(())
			})
		}))
		.runtime(Runtime::Tokio1)
		.build()
		.expect("Failed to create database connection pool");

		debug!("db started");

		pool
	}

	async fn save_db_block_shares_tables(
		conn: &Pool,
		height: u32,
		block_hash: String,
		workername: String,
		apikey: String,
	) -> Result<i32> {
		let mut conn = conn.get().await?;
		let transaction = conn.transaction().await?;

		let reward: i32 = 82;
		let block_id: i32 = transaction
			.query_one(
				"INSERT INTO block (height, block_hash, reward) VALUES ($1, $2, $3) RETURNING id",
				&[&(height as i64), &block_hash, &reward],
			)
			.await?
			.try_get("id")
			.unwrap_or(0);

		debug!("save_db -> block_id: {:?}", block_id);

		let mut worker_id = 0;
		if let Ok(row) = transaction
			.query_one(
				"SELECT id FROM worker WHERE (name, api_key) VALUES (&1, &2) limit 1",
				&[&workername, &apikey],
			)
			.await
		{
			worker_id = row.get::<&str, i32>("id");
		}

		let shares: i64 = 1;
		transaction
			.execute(
				"INSERT INTO share (block_id, worker_id, share) VALUES ($1, $2, $3)",
				&[&block_id, &worker_id, &shares],
			)
			.await?;

		debug!("save_db_block_shares_tables, block id: {block_id}");

		Ok(block_id)
	}

	async fn save_db_worker_tables(
		conn: &Pool,
		name: String,
		is_connect: bool,
		apikey: String,
	) -> Result<i32> {
		let mut conn = conn.get().await?;
		let transaction = conn.transaction().await?;

		let status = if is_connect { "online" } else { "offline" };
		let worker_id: i32 = transaction
			.query_one(
				"INSERT INTO worker (name, status, api_key) VALUES ($1, $2, $3) RETURNING id",
				&[&(name), &status.to_string(), &apikey],
			)
			.await?
			.try_get("id")
			.unwrap_or(0);

		debug!("save_db_worker_tables -> worker_id: {:?}", worker_id);

		Ok(worker_id)
	}
}

#[cfg(test)]
mod tests {
	use crate::shares::Shares;
	use dotenv::dotenv;
	use rand::{thread_rng, RngCore};
	use snarkvm::prelude::{AleoID, Field, Network, Testnet3};
	pub type BlockHash = <Testnet3 as Network>::BlockHash;

	#[tokio::test]
	async fn save_db_should_work() {
		dotenv().ok();

		let pool = Shares::initialize_db();
		assert!(pool.is_closed() == false);

		let rng = &mut thread_rng();

		let height = rng.next_u32();
		let blockhash: BlockHash = AleoID::from(Field::from_u64(rng.next_u64()));
		let workername = "unkown".to_string();
		let apikey = "unkown".to_string();
		let block_id = Shares::save_db_block_shares_tables(
			&pool,
			height,
			blockhash.to_string(),
			workername,
			apikey,
		)
		.await
		.unwrap();

		let conn = pool.get().await.unwrap();
		let rows = conn.query("SELECT * FROM block ORDER BY id DESC LIMIT 1", &[]).await.unwrap();

		let row = rows.first().unwrap();
		assert!(row.columns().len() == 8);
		assert!(block_id == row.get::<&str, i32>("id"));
		assert!(height == row.get::<&str, i64>("height") as u32);
		assert!(blockhash.to_string() == row.get::<&str, String>("block_hash"));

		let sql_delete_row = format!("DELETE FROM pool.block WHERE id={}", block_id);
		let rows = conn.query(sql_delete_row.as_str(), &[]).await.unwrap();
	}
}
