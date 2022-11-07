use anyhow::{bail, ensure, Result};
use core::str::FromStr;
use parking_lot::RwLock;
use rand::{RngCore};
use snarkvm::{
    file::Manifest,
    package::Package,
    prelude::{
        Address,
        AleoID,
        Block,
        BlockMemory,
        Field,
        Identifier,
        PrivateKey,
        Program,
        ProgramID,
        ProgramMemory,
        ProgramStore,
        RecordsFilter,
        Testnet3,
        Transaction,
        Value,
        ViewKey,
        VM,
    },
};

use crate::{
    pool::Services,
    shares::{BlockHash, SharesRequest},
};
use std::{convert::TryFrom, env, fmt, sync::Arc};
use tokio::sync::mpsc;
use tracing::{debug, warn};

pub(crate) type InternalStorage<Testnet3> = ProgramMemory<Testnet3>;
pub(crate) type InternalLedger<Testnet3> =
    snarkvm::prelude::Ledger<Testnet3, BlockMemory<Testnet3>, InternalStorage<Testnet3>>;

pub type NodeRouter = mpsc::Sender<NodeRequest>; //upstream channel
pub type NodeHandler = mpsc::Receiver<NodeRequest>; //downstrean channel

pub enum NodeRequest {
    // NewBlock(Address<Testnet3>, u64), //address, count transaction
    NewBlock(String, u64), //address, count transaction
                           // Exit,
}

#[derive(Clone)]
pub struct Node {
    pub ledger: Arc<RwLock<InternalLedger<Testnet3>>>,
    private_key: PrivateKey<Testnet3>,
    view_key: ViewKey<Testnet3>,
    address: Address<Testnet3>,
    // block_height: u32,
    // block_hash: BlockHash,

    node_router: NodeRouter, // send to node
    services: Arc<Services>,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "address {}", self.address)
    }
}

// Executing 'credits.aleo/genesis'...
// Executed 'genesis' (in 2529 ms)
// Verified 'genesis' (in 9 ms)
// Loaded universal setup (in 2466 ms)
// Built 'hello' (in 12176 ms)
// Certified 'hello': 341 ms
// Calling 'credits.aleo/fee'...
// Executed 'fee' (in 5564 ms)
// Verified certificate for 'hello': 55 ms
// Verified 'fee' (in 10 ms)
// Executing 'credits.aleo/transfer'...
// Executed 'transfer' (in 8045 ms)
impl Node {
    pub async fn new(services: Arc<Services>) -> Result<(Self, mpsc::Receiver<NodeRequest>)> {
        debug!("Node starting");

        // 1. Retrieve the private key.
        let directory = env::current_dir().unwrap(); //.join("./program/");
        let manifest = Manifest::open(&directory)?;
        let private_key = manifest.development_private_key();
        let view_key = ViewKey::try_from(private_key)?;
        let address = Address::try_from(&view_key)?;

        // // create genesis block and ledger
        let rng = &mut ::rand::thread_rng();
        let store = ProgramStore::<_, InternalStorage<_>>::open(None)?;
        let genesis = Block::genesis(&VM::new(store)?, &private_key, rng)?;
        let ledger = Arc::new(RwLock::new(InternalLedger::new_with_genesis(&genesis, address, None)?));

        // let block_hash = AleoID::from(Field::from_u64(rng.next_u64()));

        let (node_router, node_handler) = mpsc::channel(1024);
        let node = Self {
            ledger,
            // runtime,
            private_key: *private_key,
            view_key,
            address,
            // block_hash,
            // block_height: genesis.height(),
            // block_hash: genesis.hash(),
            node_router,
            services,
        };

        Ok((node, node_handler))
    }

    pub async fn initialize(&self) -> Result<()> {
        debug!("load and deploy program");

        let directory = env::current_dir().unwrap(); //.join("./program/");

        // 2. Load the package, program
        let package = Package::open(&directory)?;
        let program = package.program();

        // 3. Deploy the local program.
        let transaction = self.create_deploy(program, 1)?;
        // Add the transaction to the memory pool.
        self.add_to_memory_pool(transaction.clone())?;
        // Advance to the next block.
        let _next_block = self.advance_to_next_block()?;

        Ok(())
    }

    /// Returns an instance of the operator router.
    pub fn router(&self) -> &NodeRouter {
        &self.node_router
    }

    /// Performs the given `request` to the rpc server.
    /// All requests must go through this `update`, so that a unified view is preserved.
    pub(super) async fn update(&self, request: NodeRequest) {
        debug!("receive new block, add to mempool, add new block");

        let shares = self.services.shares().router();
        match request {
            NodeRequest::NewBlock(address, count) => {
                for _ in 0..count {
                    let transaction = self.create_transfer(address.clone(), 1).unwrap();
                    self.add_to_memory_pool(transaction).unwrap();
                }
                let block = self.advance_to_next_block().unwrap();

                let req = SharesRequest::NewBlock(block.height() + 1, block.hash(), address);
                // let req = SharesRequest::NewBlock(1, self.block_hash, 666); // test
                if let Err(error) = shares.send(req).await {
                    warn!("[NodeRequest] {}", error);
                }
            }
        }
    }

    /// Returns the account address.
    pub const fn _address(&self) -> &Address<Testnet3> {
        &self.address
    }

    pub fn add_to_memory_pool(&self, transaction: Transaction<Testnet3>) -> Result<()> {
        self.ledger.write().add_to_memory_pool(transaction)
    }

    // Advances the ledger to the next block.
    pub fn advance_to_next_block(&self) -> Result<Block<Testnet3>> {
        let rng = &mut ::rand::thread_rng();
        let next_block = self.ledger.read().propose_next_block(&self.private_key, rng)?;
        if let Err(error) = self.ledger.write().add_next_block(&next_block) {
            eprintln!("{error}");
        }

        Ok(next_block)
    }

    pub fn create_deploy(&self, program: &Program<Testnet3>, additional_fee: u64) -> Result<Transaction<Testnet3>> {
        // Fetch the unspent record with the most gates.
        let record = self
            .ledger
            .read()
            .find_records(&self.view_key, RecordsFilter::Unspent)?
            .last();
        // .max_by(|(_, a), (_, b)| (**a.gates()).cmp(&**b.gates()));

        // Prepare the additional fee.
        let credits = match record {
            Some((_, record)) => record,
            None => bail!("The Aleo account has no records to spend."),
        };
        ensure!(
            ***credits.gates() >= additional_fee,
            "The additional fee exceeds the record balance."
        );

        // Deploy.
        let transaction = Transaction::deploy(
            self.ledger.read().vm(),
            &self.private_key,
            program,
            (credits, additional_fee),
            &mut rand::thread_rng(),
        )?;

        // Verify.
        // assert!(self.ledger.read().vm().verify(&transaction));

        Ok(transaction)
    }

    // Creates a transfer transaction.
    pub fn create_transfer(&self, to: String, amount: u64) -> Result<Transaction<Testnet3>> {
        // Fetch the unspent record with the least gates.
        let record = self
            .ledger
            .read()
            .find_records(&self.view_key, RecordsFilter::Unspent)?
            .last();
        // .filter(|(_, record)| !record.gates().is_zero())
        // .min_by(|(_, a), (_, b)| (**a.gates()).cmp(&**b.gates()));

        // Prepare the record.
        let record = match record {
            Some((_, record)) => record,
            None => bail!("The Aleo account has no records to spend."),
        };

        // Create a new transaction.
        Transaction::execute(
            self.ledger.read().vm(),
            &self.private_key,
            &ProgramID::from_str("credits.aleo")?,
            Identifier::from_str("transfer")?,
            &[
                Value::Record(record),
                Value::from_str(&format!("{to}"))?,
                Value::from_str(&format!("{amount}u64"))?,
            ],
            None,
            &mut rand::thread_rng(),
        )
    }
}

// #[cfg(test)]
// mod tests {
// use crate::{
//     node::{Node, NodeRequest},
//     pool::Services,
// };
// use rand::{thread_rng, RngCore};
// use snarkvm::prelude::{Address, Block, Network, PrivateKey, Testnet3, Transaction, ViewKey};
// use std::{convert::TryFrom, env, sync::Arc};

// fn address_to() -> Address<Testnet3> {
// let rng = &mut rand::thread_rng();
// let address = Address::rand(rng);
// let private_key = PrivateKey::<Testnet3>::new(rng).unwrap();
// let view_key = ViewKey::try_from(private_key).unwrap();
// let address = Address::try_from(view_key).unwrap();

// return address;
// }

// #[tokio::test]
// async fn new_one_block_should_work() {
// let services = Arc::new(Services::new());
// let (node, node_handler) = Node::new(services.clone()).await.unwrap();
// services.initialize_node(node.clone(), node_handler).await;

// let height = node.ledger.read().latest_height();

// let addr = address_to();
// let req = NodeRequest::NewBlock(addr, 100);
// node.update(req).await;

// let last_height = node.ledger.read().latest_height();

// assert!(height + 1 == last_height);
// }
// }
