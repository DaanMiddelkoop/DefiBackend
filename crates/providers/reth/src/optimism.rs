use std::{
    collections::HashSet,
    fmt::Display,
    future::poll_fn,
    task::{Context, Poll, ready},
};

use alloy::{
    eips::BlockNumberOrTag,
    primitives::{Address, B256, StorageValue, U256},
};
use clap::Parser;
use futures::{FutureExt, SinkExt, StreamExt, TryStreamExt, select};
use providers::{State, StateProvider};
use reth::{
    api::{FullNodeComponents, NodeTypes},
    providers::StateProviderFactory,
    revm::primitives::StorageKey,
};
use reth_execution_types::Chain;
use reth_exex::{ExExContext, ExExNotification};
use reth_optimism_cli::{Cli, chainspec::OpChainSpecParser};
use reth_optimism_node::{OpNode, args::RollupArgs};
use reth_tracing::tracing::info;
use revm::{
    Database, DatabaseRef,
    context::DBErrorMarker,
    database::CacheDB,
    state::{AccountInfo, Bytecode},
};

pub struct OptimismState {
    database: CacheDB<OptimismFetcher>,
}

impl State for OptimismState {
    fn database(&mut self) -> &mut impl Database {
        &mut self.database
    }

    fn current_block(&self) -> u64 {
        self.database.db.block
    }
}

struct OptimismCall {
    block: u64,
    call_type: OptimismCallType,
    response: std::sync::mpsc::Sender<Result<OptimismCallResponse, FetcherError>>,
}

impl OptimismCall {}

enum OptimismCallResponse {
    Storage(U256),
    Basic(Option<AccountInfo>),
}

enum OptimismCallType {
    Storage(Address, U256),
    Basic(Address),
}

struct OptimismFetcher {
    block: u64,
    call: tokio::sync::mpsc::Sender<OptimismCall>,
}

impl OptimismFetcher {
    pub fn call(&self, call: OptimismCallType) -> Result<OptimismCallResponse, FetcherError> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.call
            .try_send(OptimismCall {
                block: self.block,
                call_type: call,
                response: tx,
            })
            .unwrap();
        rx.recv().map_err(|x| FetcherError(format!("Failed to receive response: {x}")))?
    }
}

#[derive(Debug)]
pub struct FetcherError(String);
impl std::error::Error for FetcherError {}
impl Display for FetcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl DBErrorMarker for FetcherError {}

impl DatabaseRef for OptimismFetcher {
    #[doc = " The database error type."]
    type Error = FetcherError;

    #[doc = " Gets basic account information."]
    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        if let OptimismCallResponse::Basic(account) = self.call(OptimismCallType::Basic(address))? {
            println!("Fetched account {address:?}");
            Ok(account)
        } else {
            Err(FetcherError("Failed to fetch basic account information".to_string()))
        }
    }

    #[doc = " Gets account code by its hash."]
    fn code_by_hash_ref(&self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
        todo!()
    }

    #[doc = " Gets storage value of address at index."]
    fn storage_ref(&self, address: Address, index: StorageKey) -> Result<StorageValue, Self::Error> {
        if let OptimismCallResponse::Storage(value) = self.call(OptimismCallType::Storage(address, index))? {
            println!("Fetched storage value for {address} at {index:x}: {value:x}");
            Ok(value)
        } else {
            Err(FetcherError("Failed to fetch storage value".to_string()))
        }
    }

    #[doc = " Gets block hash by block number."]
    fn block_hash_ref(&self, _number: u64) -> Result<B256, Self::Error> {
        todo!()
    }
}

struct OptimismReplier {
    calls: tokio::sync::mpsc::Receiver<OptimismCall>,
}

impl OptimismReplier {
    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<OptimismCall> {
        let call = ready!(self.calls.poll_recv(cx)).expect("Optimism replier channel closed");
        Poll::Ready(call)
    }
}

pub struct OptimismStateProvider {
    chain_id: u16,
    state: OptimismState,
    state_updates: futures::channel::mpsc::Receiver<StateDiff>,
}

impl StateProvider for OptimismStateProvider {
    fn chain_id(&self) -> u16 {
        self.chain_id
    }

    fn state(&mut self) -> &mut impl State {
        &mut self.state
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Vec<Address>> {
        println!("Polling for state updates...");
        let update = ready!(self.state_updates.poll_next_unpin(cx)).unwrap();

        assert!(update.start_block == self.state.database.db.block);

        let updated = update.diffs.iter().map(|(address, _)| *address).collect::<Vec<_>>();
        for (address, storage) in update.diffs {
            for (slot, value) in storage {
                self.state.database.insert_account_storage(address, slot.into(), value).unwrap();
            }
        }

        self.state.database.db.block = update.end_block;

        Poll::Ready(updated)
    }
}

#[allow(unused)]
impl OptimismStateProvider {
    pub async fn start() -> Self {
        let (state_updates_tx, mut state_updates_rx) = futures::channel::mpsc::channel(16);
        let (call_tx, call_rx) = tokio::sync::mpsc::channel(16);
        let communicator = Communicator {
            state_updates: state_updates_tx,
            replier: OptimismReplier { calls: call_rx },
        };

        start_reth(communicator);

        // Wait for first state update
        let update = state_updates_rx.next().await.expect("Failed to receive initial state update");
        println!("Received initial update from {} to {}", update.start_block, update.end_block);

        Self {
            chain_id: todo!("Fetch chain ID from reth"),
            state: OptimismState {
                database: CacheDB::new(OptimismFetcher {
                    block: update.end_block,
                    call: call_tx,
                }),
            },
            state_updates: state_updates_rx,
        }
    }
}

struct StateDiff {
    start_block: u64,
    end_block: u64,
    diffs: Vec<(Address, Vec<(B256, U256)>)>,
}

struct Communicator {
    state_updates: futures::channel::mpsc::Sender<StateDiff>,
    replier: OptimismReplier,
}

fn start_reth(communicator: Communicator) {
    std::thread::spawn(move || {
        let args = std::fs::read_to_string("test_args.sh").unwrap();
        println!("Args: {:?}", args.split_whitespace().collect::<Vec<_>>());

        if let Err(err) = Cli::<OpChainSpecParser, RollupArgs>::parse_from(args.split_whitespace()).run(async move |builder, rollup_args| {
            info!(target: "reth::cli", "Launching node");
            let handle = builder
                .node(OpNode::new(rollup_args))
                .install_exex("DefiBackend", async move |ctx| Ok(run_statediff(ctx, communicator)))
                .launch_with_debug_capabilities()
                .await?;
            handle.node_exit_future.await
        }) {
            eprintln!("Error: {err:?}");
            std::process::exit(1);
        }
    });
}

async fn run_statediff<Node: FullNodeComponents>(mut ctx: ExExContext<Node>, mut communicator: Communicator) -> eyre::Result<()> {
    let mut addresses = HashSet::new();

    let current_block = ctx.head.number;
    let state_diff = StateDiff {
        start_block: current_block,
        end_block: current_block,
        diffs: Vec::new(),
    };

    communicator
        .state_updates
        .send(state_diff)
        .await
        .expect("Failed to send initial state diff");

    loop {
        select! {
            notification = ctx.notifications.try_next().fuse() => {
                let notification = match notification? {
                    Some(notification) => notification,
                    None => panic!("ExEx notifications stream closed unexpectedly"),
                };

                match &notification {
                    ExExNotification::ChainCommitted { new } => {
                        info!(target: "reth::exex", "Chain committed new block {:?}", new.range());
                        let state_diff = get_state_diff::<Node::Types>(new);
                        communicator.state_updates.send(state_diff).await.expect("Failed to send state diff");
                    }
                    ExExNotification::ChainReverted { old } => {
                        info!(target: "reth::exex", "Chain reverted to block {:?}", old.range());
                    }
                    ExExNotification::ChainReorged { old, new } => {
                        info!(target: "reth::exex", "Chain reorganized from block {:?} to {:?}", old.range(), new.range());
                    }
                }
            }
            call = poll_fn(|cx| communicator.replier.poll(cx)).fuse() => {
                handle_call(&mut ctx, &mut addresses, call).await;
            }
        }
    }
}

fn get_state_diff<Types: NodeTypes>(chain: &Chain<Types::Primitives>) -> StateDiff {
    let execution_outcome = chain.execution_outcome();

    let state_diff = execution_outcome
        .bundle
        .state
        .iter()
        .map(|(address, storage)| {
            (
                *address,
                storage
                    .storage
                    .iter()
                    .map(|(key, value)| ((*key).into(), value.present_value()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    let mut range = chain.range();
    let start_block = range.next().unwrap();
    let end_block = range.last().unwrap();

    StateDiff {
        start_block,
        end_block,
        diffs: state_diff,
    }
}

async fn handle_call<Node: FullNodeComponents>(ctx: &mut ExExContext<Node>, interested_addresses: &mut HashSet<Address>, call: OptimismCall) {
    let provider = ctx.provider();
    let provider = provider.state_by_block_number_or_tag(BlockNumberOrTag::Number(call.block)).unwrap();

    match call.call_type {
        OptimismCallType::Basic(address) => {
            interested_addresses.insert(address);
            let result = provider.basic_account(&address).map(|x| {
                let account = x.map(|account| {
                    let hash = account.bytecode_hash;
                    let mut account: AccountInfo = account.into();

                    if let Some(code_hash) = hash {
                        let code = provider.bytecode_by_hash(&code_hash).unwrap();
                        account.code = Some(code.unwrap().0);
                    }
                    account
                });
                OptimismCallResponse::Basic(account)
            });

            call.response
                .send(result.map_err(|e| FetcherError(format!("Failed to fetch basic account: {e}"))))
                .unwrap();
        }
        OptimismCallType::Storage(address, index) => {
            interested_addresses.insert(address);
            let result = provider
                .storage(address, index.into())
                .map(|x| OptimismCallResponse::Storage(x.unwrap_or_default()))
                .map_err(|e| FetcherError(format!("Failed to fetch storage: {e}")));
            call.response.send(result).unwrap();
        }
    };
}

#[cfg(test)]
mod test {
    use futures::future::poll_fn;

    use providers::{State, StateProvider};

    #[tokio::test]
    async fn test_optimism_state_provider() {
        let mut provider = super::OptimismStateProvider::start().await;
        loop {
            poll_fn(|cx| provider.poll(cx)).await;
            let state = provider.state();
            println!("Current block: {}", state.current_block());
        }
    }
}
