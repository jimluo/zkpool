## rpc protocol strtum

### subscribe
```js
{"id": 1, "method": "mining.subscribe", "params": ["MINER_USER_AGENT", "PROTOCOL_VERSION", "SESSION_ID"]}
{"id": 1, "result": ["SESSION_ID", "SERVER_NONCE", "pool address"], "error": null}
```
### authorize
```js
{"id": 1, "method": "mining.authorize", "params": ["WORKER_NAME", "WORKER_PASSWORD"]}
{"id": 1, "result": RESULT, "error": null}
```

### set_target
```js
{"id": null, "method": "mining.set_target", "params": ["TARGET"]}
{"id": 1, "result": RESULT, "error": null}
```

### notify
```js
{"id": null, "method": "mining.notify", "params": ["JOB_ID", "EPOCH_CHALLENGE", "ADDRESS", "CLEAN_JOBS"]}
{"id": 1, "result": RESULT, "error": null}
```

### submit
```js
{"id": 1, "method": "mining.submit", "params": ["WORKER_NAME", "JOB_ID", "NONCE", "COMMITMENT", "PROOF"]}
{"id": 1, "result": RESULT, "error": null}
```
## pool server services flow
- v1: rpc - node - shares
- rpc collection many miner's trsactions
- node group trsactions into one block
- shares save block and miner shares into db

### Solution
```rs
pub struct UnconfirmedSolution<N: Network> {
    pub puzzle_commitment: PuzzleCommitment<N>,
    pub solution: Data<ProverSolution<N>>,
}


pub struct PuzzleCommitment<N: Network> {
    /// The commitment for the solution.
    commitment: KZGCommitment<<N as Environment>::PairingCurve>,
}


pub struct ProverSolution<N: Network> {
    /// The core data of the prover solution.
    partial_solution: PartialSolution<N>,
    /// The proof for the solution.
    proof: PuzzleProof<N>,
}

pub type PuzzleProof<N> = KZGProof<<N as Environment>::PairingCurve>;

pub struct PartialSolution<N: Network> {
    /// The address of the prover.
    address: Address<N>,
    /// The nonce for the solution.
    nonce: u64,
    /// The commitment for the solution.
    commitment: PuzzleCommitment<N>,
}

PairingCurve as PairingEngine
pub struct DensePolynomial<Field> {
    /// The coefficient of `x^i` is stored at location `i` in `self.coeffs`.
    pub coeffs: Vec<Field>,
}
```

## architecture
- server
  - thread
    - clean nonce
    - listen connect
    - listen rpc: 
      - subscribe/disconnected: provers.conneted.(insert/remove)
      - auth/exit: provers.authenticationted.(insert/remove)
      - submit/enewEpochChannlenge: shares.(insert/remove)
    - write new job/...
  - state
    - pool
    - prover
  - provers
    - connected
    - authenticationted 
- connection
- operator_peers: aleo peer
- accounting: PPLNS
  - DB/storage


## server
  
- Builder
  - Settings
  - set_xxx
  - ServiceData
    - remote_addr
    - id_provider
    - ping_interval
    - conn_id
    - conn:OwnedSemaphorePermit
- Server
  - start
``` rs
    let (stop_tx, stop_rx) = watch::channel(());
    ServerHandle::new(stop_tx)
    start_inner(methods, stop_handle)
      loop { connections:(FutureDriver).select_with(incoming:(Monitored)
        tower_service{inner: ServiceData{conn=connection_guard.try_acquire()}}
        let service = self.service_builder.service(tower_service);
        connections.add(service, curr_conns=connection_guard.available_connections())
      }          
```            
   - connections: FutureDriver
      - service: TowerService
        - inner: ServiceData
          - conn: OwnedSemaphorePermit
     - ServerHandle(Arc<watch::Sender<()>>)
     - ConnectionGuard(Arc<Semaphore>)        
     - TowerService
       - call
        - ws::background_task::

- FutureDriver
  - futures: Vec<F>,
  - stop_monitor_heartbeat: Interval,
  - poll/selct_with/add(futue)
- Transport
    - ws::background_task(sender, recevier, ServiceData)
	    - tokio::spawn(send_task(rx, sender, stop_handle.clone(), ping_interval));
        - loop { match receiver.receive(&mut data).await? {
        - method_executors.select_with(Monitored::new(receive, &stop_handle)
        - parse json
          - process_batch_request
            - execute_call(req, callData)
              - let response = match methods.method_with_name(name) { Sync/Async/Subscription/Unsubscription
              - method.claim(name, resources)
              - let resp = (callback)(id, params, max_response_body_size as usize);
              - MethodResult::SendAndLogger(response)
    - http::handle_request(request, data)    

## jsonrpsee
```rs
目录结构
client/
  http-client/client.rs
  ws-client/lib.rs
  tranport/src
    web
    ws/stream

server/
  server
  logger
  future
  middleware/
  tranport/

  core/src/
    client/
      mod.rs
      async_client/
    server/
      rpc_modules
      host_filtering
request流程
client = (Http/Ws)ClientBuilder::default().build(url)?;
response= client.request::<String>("say_hello", params).await;

    guard = self.id_manager.next_request_id()?;
    id = guard.inner();
    params = params.to_rpc_params()?;
    request = RequestSer::new(&id, method, params);
    raw = serde_json::to_string(&request)
    ...
    response = Response<&JsonRawValue> = serde_json::from_slice(&body) 

http-client/src/tranport.rs ::request(method, params)
    fut = self.transport.send_and_read_body(raw);
        inner_send {  hyper::Request::post(&self.targetUri) }  //hyper
    body = timeout(self.request_timeout, fut).await

core/src/client/mod.rs ::request(method, params)
    build_with_tokio ==> background_task()
    msg = FrontToBack::Request(RequestMessage { raw, id, send_back_tx })
    future-sink.send(msg)
      background_task
        Either::Left((frontend_value) => handle_frontend_messages()
          sender.send(request.raw)
            impl TransportSenderT for Sender {
             async fn send(&mut self, body: String) -> Result<(), Self::Error> {
                inner: connection::Sender<BufReader<BufWriter<EitherStream>>>,
                // soketto dpendencices
                self.inner.send_text(body).await?;
                self.inner.flush().await?;

        Either::Right((backend_value) => handle_backend_messages()
          handle_recv_message, process_xxx_response()
    body = call_with_timeout(self.request_timeout, send_back_rx) // Wait for a stream to complete json_value
```    

```rs
NewShare(address, value) <= ProverSubmit
    pplns.write().add_share(Share::init(value=difficulty_target_n, address))
SetN(n) <= NewEpochChallenge(n = coinbase_target * 5)
    pplns.write().set_n(n);
NewBlock(height, block_hash, reward) 
    database.save_block(height, block_hash, reward, address_shares)
Exit =>  backup pplns
    pplns_storage.put(&Null {}, &pplns.read().await.clone());

current_round "admin" / "current_round"   当前上链的块
    result = pplns_to_provers_shares(&pplns);
        pplns.queue.iter().for_each(|share| {
            address_shares<share.owner, sum(share.value))?
            provers = address_shares.len() as u32

    self.round_cache.write().set(Null {}, result.clone());
    json!({ pplns.n, pplns.current_n, provers, address_shares })

blocks_mined API "admin" / "recent_blocks_mined" block_canonical_cache 当前挖出的块
  get_latest_block_height()
  let blocks = self.database.get_blocks(limit, page).await?;
  let shares = self.database.get_block_shares(blocks.iter().map(|b| b.0).collect())
  for (id, height, block_hash, mut is_canonical, reward) in blocks {
    let cache_result = self.block_canonical_cache.read().get(block_hash);
    self.update_block_canonical(height, block_hash, is_canonical, reward)
    self.block_canonical_cache.write().set(block_hash.clone(), is_canonical);

    let mut value = json!({ height,block_hash, confirmed,  canonical, reward, shartes },

payout_loop  thread loop
  get_latest_block_height()
  let blocks = self.database.get_should_pay_blocks(latest_block_height).await;
  for (id, height, block_hash, is_canonical, reward) in blocks {
    let node_canonical = self.update_block_canonical(height, block_hash, is_canonical, reward)
    self.block_canonical_cache.write().set(block_hash, canonical);
    self.database.pay_block(id)
    self.database.set_checked_blocks(latest_block_height)

db 
pay_block:    PROCEDURE pool.pay_block {block_id_arg}
    $block = SELECT * FROM block WHERE id = {block_id_arg}
    block_id = block[0]["id"]
    shares = SELECT * FROM share WHERE block_id = {block_id}
    for share in shares:
        data[share["miner"]] = share["share"]

    reward = block[0]["reward"]
    total_shares = sum(data.values())
        reward_per_share = reward // total_shares

    payout_plan = INSERT INTO payout (block_id, miner, amount)
    balance_plan =INSERT INTO balance (address, unpaid) DO UPDATE SET unpaid = balance.unpaid + $2
    stats_plan =  INSERT INTO stats (key, value) DO UPDATE SET value = stats.value + $2
    block_plan =  UPDATE block SET paid = true WHERE id = $1

    for miner, share in data.items():
      amount = reward_per_share * share
      payout_plan.execute([block_id, miner, amount])
      balance_plan.execute([miner, amount])
      block_plan.execute([block_id])
      paid += amount
    stats_plan.execute(["total_paid", paid])
    stats_plan.execute(["total_fee", raw_reward - reward])
    stats_plan.execute(["total_rounding", reward - paid])


get_blocks [block]
    row = SELECT id FROM block ORDER BY id DESC LIMIT 1
    last_id: i32 = row.first().unwrap().get("id"); // $1
    stmt = SELECT * FROM block WHERE id <= $1 AND id > $2 ORDER BY id DESC
    rows = conn.query(&stmt, &[&last_id, &(last_id - page as i32 * limit as i32)])// $1, $2

get_block_shares [share]
    stmt = SELECT * FROM share WHERE block_id = ANY(&id)
    for (block_id, miner, share) in stmt {
        shares::HashMap<block_id, (miner, share)>

    
save_block [block, share]
    block_id = INSERT INTO block (height, block_hash, reward) RETURNING id,
    let stmt = INSERT INTO share (block_id, miner, share) VALUES ($1, $2, $3)
    for (address, share) in shares {
        transaction.query(&stmt, &[&block_id, &address, &(share as i64)])

set_block_canonical [block]
    UPDATE block SET is_canonical = $1 WHERE block_hash = $2

set_checked_blocks  [block]
    UPDATE block SET checked = true WHERE height <= $1 AND checked = false
```    
warp::serve(routes) // 0.0.0.0:80
node:   0.0.0.0:4133
beacon: 159.203.77.113:4130
rpc:    0.0.0.0:3033 (rpc_username, rpc_password）

```rs
main
initialize_logger()
rayon::ThreadPoolBuilder::new()
runtime = runtime::Builder::new_multi_thread()
runtime.block_on(async move {
    cli.start().await.expect("Failed to start the node");
});
CLI.start()
  self.start_node(None) //only client
    account = Account::sample()
    node = Node::new(self, account)
      handle_listener::<N>(listener, ledge) //TcpListener::bind(cli.node)
      connect_to_leader(leader_addr, ledger)
        spawn{ 
          ledger.peers().read().contains_key(&initial_peer)}
          handle_peer(stream, initial_peer, ledger)
        }
      send_pings(ledger)
    node.connect_to(connect-peer_ip) // foreach
      spawn( handle_peer::(stream, peer_ip, ledger)
    handle_signals(node);
    std::future::pending::<()>().await;

networks/mod.rs
::Node
  Ledger::load(*account.private_key(), cli.dev)?
  ledger.initial_sync_with_network(cli.beacon_addr.ip()
    resp = reqwest::get(format!(http://{leader_ip}/testnet3/
    ledger.add_next_block(&blocklatest/height)
  if ledger is None:
    genesis_block = request_genesis_block::<N>(cli.beacon_addr.ip()).await?;
    Ledger::new_with_genesis(*account.private_key(), genesis_block, cli.dev)?
  
::handle_peer
loop{ tokio::select!
peer.inbound.recv() > peer.outbound.send(msg)
peer.outbound.next()
  Ping/Pong
  BlockRequset: ledger.get_block(height)
  BlockResponse: ledger.add_next_block(block) //(block_height, block_hash)
  TransactionBroadcast:
    ledger.add_to_memory_pool(transaction)
    // Broadcast transaction to all peers except the sender.
    spawn( for (_, sender) in peers{ sender.send(transaction_bytes)
  BlockBroadcase:
    ledger.add_next_block(block)
    // Broadcast transaction to all peers except the sender.
    spawn( for (_, sender) in peers{ sender.send(block_bytes)    

ledger/mod.rs
::Ledger
load: 
  InternalLedger::open(dev)
  InternalServer::start(ledger, additional_routes[address/count/all])
    warp::serve(routes) // 0.0.0.0:80
new_with_genesis: InternalLedger::new_with_genesis(&genesis_block,
initial_sync_with_network

add_next_block: InternalLedger::add_next_block(block)
add_to_memory_pool: InternalLedger.add_to_memory_pool(transaction)

get_block
find_spent_records
create_deploy:  Transaction::deploy(InternalLedger.vm
create_transfer:Transaction::execute(InternalLedger.vm


pub struct Ledger<N: Network> {
    ledger: Arc<RwLock<InternalLedger<N>>>,
    server: InternalServer<N>,
    peers: Peers<N>,
    private_key: PrivateKey<N>,
    view_key: ViewKey<N>,
    address: Address<N>,
}

snarkVm/vm/compiler/src/ledger/mod.rs
pub struct InternalLedger<N: Network, B: BlockStorage<N>, P: ProgramStorage<N>> {
    current_hash: N::BlockHash,
    current_height: u32,
    current_round: u64,
    block_tree: BlockTree<N>,
    blocks: BlockStore<N, B>,
    transactions: TransactionStore<N, B::TransactionStorage>,
    transitions: TransitionStore<N, B::TransitionStorage>,
    validators: IndexMap<Address<N>, ()>,
    memory_pool: IndexMap<N::TransactionID, Transaction<N>>,
    vm: VM<N, P>,
}

}
```
```rs
Ledger
  vm { Process, ConsensusStore } add_next_block/finalize foreach transactions -> process(deploy/execute)
  current_block

  add_next_block() -> vm.add_next_block() foreach transactions -> process(deploy/execute)
  create_transfer() -> records = self.find_unspent_records(&ViewKey), Transaction::execute(

  latest_epoch_challenge()
    epoch_starting_height = latest_height - latest_height % N::NUM_BLOCKS_PER_EPOCH;
    epoch_block_hash = self.vm.block_store().get_previous_block_hash(epoch_starting_height)?;
    latest_epoch_number = self.current_block.read().height() / N::NUM_BLOCKS_PER_EPOCH

    EpochChallenge::new(latest_epoch_number, epoch_block_hash, N::COINBASE_PUZZLE_DEGREE)


Prover 
  puzzle_request -> self.ledger.latest_epoch_challenge() (num/hash/degree)
  task_loop {
    solution = prove(epoch,height,address,nonce)
    message = Message::UnconfirmedSolution({ solution.commitment(), solution })
    request = RouterRequest::MessagePropagate(message,)
    prover.router.process(request)
  }

Beacon
  task_loop { 定时打块，块里
    let time_to_wait = ROUND_TIME.saturating_sub(beacon.block_generation_time.load(Ordering::SeqCst));
    // Produce the next block and propagate it to all peers.
    beacon.produce_next_block()
      // Produce a transaction if the mempool is empty.
      transaction = beacon.ledger.create_transfer(beacon.private_key()
      beacon.consensus.add_unconfirmed_transaction(transaction)
      // XXX，从mem_pool取所有候选交易和候选证明解决方案
      // 创建coinbase，rewards和Block
      next_block = beacon.consensus.propose_next_block(beacon.private_key()

      beacon.consensus.advance_to_next_block(&next_block) 
      message = Message::UnconfirmedBlock({height,hash,serialized_block),
      router.process(RouterRequest::MessagePropagate(message,
  }  

add_unconfirmed_transaction
  Ledger/beacon -> Consensus -> Mempool
```