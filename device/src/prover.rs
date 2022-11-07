use std::{
    collections::VecDeque,
    fs::File,
    io::Read,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    thread::sleep,
    time::Duration,
};

use rand::RngCore;
use snarkvm::{
    algorithms::polycommit::kzg10::UniversalParams,
    curves::bls12_377::Bls12_377,
    prelude::{
        Address,
        AleoID,
        CanonicalDeserialize,
        CoinbasePuzzle,
        EpochChallenge,
        PrivateKey,
        PuzzleConfig,
        Testnet3,
    },
    console::types::Field,
    utilities::serialize::*,
};

type CoinbasePuzzleInst = CoinbasePuzzle<Testnet3>;

#[derive(Clone)] //, Debug)]
pub struct Prover {
    // job: Arc<RwLock<Job>>,
    // target: Arc<AtomicU64>,
    // total_shares: Arc<AtomicU32>,
    total_proofs: Arc<AtomicU32>,
    status_log: VecDeque<u32>,
    puzzle: Box<CoinbasePuzzleInst>,
    degree: u32,
    pub(super) prove_count_per_block: u32,
}

impl Prover {
    pub fn new(prove_count_per_block: u32, degree_prove: u32) -> Self {
        let (puzzle, degree) = Prover::setup_prove(degree_prove);

        Self {
            // total_shares: Default::default(),
            total_proofs: Default::default(),
            status_log: VecDeque::<u32>::from(vec![0; 60]),
            puzzle,
            degree,
            prove_count_per_block,
        }
    }

    fn setup_prove(degree: u32) -> (Box<CoinbasePuzzleInst>, u32) {
        tracing::info!("wating: setup_prover degree:{}", degree);

        let mut file = File::open("./universal.srs").expect("need universal20.srs file");
        let mut srs = Vec::new();
        file.read_to_end(&mut srs).expect("need to read the whole file");

        let universal_srs: UniversalParams<Bls12_377> =
            CanonicalDeserialize::deserialize_with_mode(&*srs, Compress::No, Validate::No)
                .expect("Failed to init universal SRS");
        // let universal_srs = CoinbasePuzzleInst::setup(max_config, &mut thread_rng()).unwrap();

        let config = PuzzleConfig {
            degree: 2_u32.pow(degree),
        };
        let prover = CoinbasePuzzleInst::trim(&universal_srs, config).unwrap();

        (Box::new(prover), config.degree)
    }

    pub fn status_prove_shares(&mut self) {
        let new = self.total_proofs.load(Ordering::SeqCst);
        self.status_log.pop_front();
        self.status_log.push_back(new);

        let mut status = String::from("");
        for i in [1, 5, 15, 30, 60] {
            let old = self.status_log.get(i - 1).unwrap_or(&0);
            let rate = (new - old) as f64 / (i * 60) as f64;
            status.push_str(format!("{}m:{:.2} ", i, rate).as_str());
        }
        tracing::info!("Total shares: {}, Rate proves/s: [{}]", new, status);
    }

    // pub fn _new_job(&mut self, job: Job) {
    //     self.job = Arc::new(RwLock::new(job));
    // }

    pub fn next_prove(&mut self) -> Result<(), ()> {
        self.total_proofs.fetch_add(1, Ordering::SeqCst);

        let mut i = 0;
        while i < self.prove_count_per_block {
            sleep(Duration::from_millis(10));

            let rng = &mut ::rand::thread_rng();
            let blockhash = AleoID::from(Field::from_u64(rng.next_u64()));
            let pk = PrivateKey::new(rng).expect("Fail on PrivateKey::new");

            let challenge = EpochChallenge::<Testnet3>::new(rng.next_u32(), blockhash, self.degree);
            let address = Address::try_from(pk);
            let nonce = rng.next_u64();

            if challenge.is_ok() && address.is_ok() {
                let solution = self.puzzle.prove(&challenge.unwrap(), address.unwrap(), nonce);
                if solution.is_ok() {
                    i += 1;
                }
            }
        }

        Ok(())
    }
}
