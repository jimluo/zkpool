use crate::pool::Services;
use anyhow::Result;
use deadpool_postgres::{
    ClientWrapper,
    Config,
    Hook,
    HookError,
    HookErrorCause,
    Manager,
    ManagerConfig,
    Pool,
    RecyclingMethod,
    Runtime,
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
    NewBlock(u32, BlockHash, String), //height, block_hash, address
                                      // NewBlock(u32, String, i64), //height, block_hash, reward
}

#[derive(Clone, Debug)]
#[allow(clippy::type_complexity)]
pub struct Shares {
    connection_pool: deadpool_postgres::Pool,
    block_count: usize,
    shares_router: SharesRouter,
    services: Arc<Services>,
}

impl Shares {
    pub async fn new(services: Arc<Services>) -> Result<(Self, mpsc::Receiver<SharesRequest>)> {
        tracing::debug!("Shares starting");

        let connection_pool = Shares::initialize_db();

        let (shares_router, shares_handler) = mpsc::channel(1024);
        let shares = Self {
            connection_pool,
            block_count: 0,
            shares_router,
            services,
        };

        Ok((shares, shares_handler))
    }

    pub fn status(&self) -> String {
        format!("block count {}", self.block_count)
    }

    pub fn router(&self) -> &SharesRouter {
        &self.shares_router
    }

    /// Performs the given `request` to the rpc server.
    /// All requests must go through this `update`, so that a unified view is preserved.
    pub(super) async fn update(&mut self, request: SharesRequest) {
        debug!("receive new block, save_db");
        match request {
            SharesRequest::NewBlock(height, block_hash, address) => {
                match Shares::save_db(&self.connection_pool, height, block_hash.to_string(), address).await {
                    Ok(id) => {
                        self.block_count += 1;
                        info!("Recorded block id={id}, count={}", self.block_count);
                    }
                    Err(e) => error!("Failed to save block reward : {}", e),
                }
            }
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

        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Verified,
        });

        // This is almost like directly using deadpool, but we really need the hooks
        // The helper methods from deadpool_postgres helps as well
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

    async fn save_db(
        conn: &Pool,
        height: u32,
        block_hash: String,
        // reward: i64,
        address: String,
        // shares: HashMap<String, u64>,
    ) -> Result<i32> {
        let mut conn = conn.get().await?;
        let transaction = conn.transaction().await?;

        let reward: i32 = 666;
        let block_id: i32 = transaction
            .query_one(
                "INSERT INTO block (height, block_hash, reward) VALUES ($1, $2, $3) RETURNING id",
                &[&(height as i64), &block_hash, &reward],
            )
            .await?
            .try_get("id")
            .unwrap_or(0);

        debug!("save_db -> block_id: {:?}", block_id);

        // let stmt = transaction
        //     .prepare_cached("INSERT INTO share (block_id, miner, share) VALUES ($1, $2, $3)")
        //     .await?;
        let shares: i64 = 1;
        transaction
            .execute("INSERT INTO share (block_id, miner, share) VALUES ($1, $2, $3)", &[
                &block_id, &address, &shares,
            ])
            .await?;
        // for (address, share) in shares {
        //     transaction
        //         .query(&stmt, &[&block_id, &address, &(share as i64)])
        //         .await?;
        // }

        transaction.commit().await?;

        debug!("save db a block id: {block_id}");

        Ok(block_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::shares::Shares;
    use dotenv::dotenv;
    use rand::{thread_rng, RngCore};
    use snarkvm::prelude::{Address, AleoID, Field, Network, Testnet3};
    pub type BlockHash = <Testnet3 as Network>::BlockHash;

    #[tokio::test]
    async fn save_db_should_work() {
        dotenv().ok();

        let pool = Shares::initialize_db();
        assert!(pool.is_closed() == false);

        let rng = &mut thread_rng();

        let height = rng.next_u32();
        let blockhash: BlockHash = AleoID::from(Field::from_u64(rng.next_u64()));
        let address = Address::rand(rng);
        let block_id = Shares::save_db(&pool, height, blockhash.to_string(), address)
            .await
            .unwrap();

        let conn = pool.get().await.unwrap();
        let rows = conn
            .query("SELECT * FROM block ORDER BY id DESC LIMIT 1", &[])
            .await
            .unwrap();

        let row = rows.first().unwrap();
        assert!(row.columns().len() == 8);
        assert!(block_id == row.get::<&str, i32>("id"));
        assert!(height == row.get::<&str, i64>("height") as u32);
        assert!(blockhash.to_string() == row.get::<&str, String>("block_hash"));

        let sql_delete_row = format!("DELETE FROM pool.block WHERE id={}", block_id);
        let rows = conn.query(sql_delete_row.as_str(), &[]).await.unwrap();
    }
}
