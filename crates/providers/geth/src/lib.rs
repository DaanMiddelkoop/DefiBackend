use std::{fmt::Display, task::ready};

use providers::{State, StateProvider};
use revm::{Database, context::DBErrorMarker, database::InMemoryDB};

use crate::{
    client::GethClient,
    subscriptions::{NewHeads, Subscription, new_heads},
};
pub mod client;
pub mod subscriptions;

#[derive(Debug)]
pub enum GethError {
    ConnectionError(String),
    DeserializeError(String),
}

impl Display for GethError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GethError::ConnectionError(err) => write!(f, "Connection error: {err}"),
            GethError::DeserializeError(err) => write!(f, "Deserialization error: {err}"),
        }
    }
}

impl std::error::Error for GethError {}
impl DBErrorMarker for GethError {}

pub struct GethProvider {
    subscription: Subscription<NewHeads>,
    state: GethState,
}

impl GethProvider {
    pub async fn new(url: String) -> GethProvider {
        let rt = tokio::runtime::Handle::current();
        let mut client = GethClient::connect(&url, std::time::Duration::from_secs(1)).await.unwrap();
        let current_block = client.block_number().await.unwrap_or(0);

        let subscription = new_heads(&url).await.unwrap();

        let state = GethState {
            database: InMemoryDB::default(),
            client,
            current_block,
            rt,
        };

        GethProvider { state, subscription }
    }
}

impl StateProvider for GethProvider {
    fn poll(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Vec<alloy::primitives::Address>> {
        let result = ready!(self.subscription.next(cx));
        match result {
            Ok(new_heads) => {
                self.state.current_block = u64::try_from(new_heads.number).unwrap();
                todo!("Generate update list from newHeads");
            }
            Err(err) => {
                eprintln!("Error while polling subscription: {err}");
                std::task::Poll::Ready(vec![])
            }
        }
    }

    fn state(&mut self) -> &mut impl State {
        &mut self.state
    }
}

pub struct GethState {
    database: InMemoryDB,
    client: GethClient,
    current_block: u64,
    rt: tokio::runtime::Handle,
}

impl State for GethState {
    fn database(&mut self) -> &mut impl Database {
        &mut self.database
    }

    fn current_block(&self) -> u64 {
        self.current_block
    }

    async fn call<C: alloy_sol_types::SolCall>(
        &mut self,
        from: alloy::primitives::Address,
        to: alloy::primitives::Address,
        value: alloy::primitives::U256,
        calldata: <C::Parameters<'_> as alloy_sol_types::SolType>::RustType,
    ) -> Result<C::Return, String> {
        let _guard = self.rt.enter();
        let abi_encoded = C::new(calldata).abi_encode();
        let response = self
            .client
            .eth_call(self.current_block.into(), from, to, value, &abi_encoded)
            .await
            .map_err(|x| format!("Failed to call contract: {x}"))?;

        match C::abi_decode_returns(&response) {
            Ok(result) => Ok(result),
            Err(err) => Err(format!("Failed to decode abi response: {err}")),
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use alloy::{
        hex,
        primitives::{U256, address},
    };
    use futures::{StreamExt, stream::FuturesUnordered};

    use crate::client::GethClient;

    #[tokio::test]
    async fn test_parallel_calls() {
        let call_stack = (0..10).map(|_| async move {
            let mut client = GethClient::connect("ws://10.8.0.1:8546", Duration::from_secs(10)).await.unwrap();
            let block = client.block_number().await.unwrap().into();
            let calldata = hex!("0xc6a5026a000000000000000000000000ceba9300f2b948710d2653dd7b07f33a8b32118c00000000000000000000000048065fbbe25f71c9282ddf5e1cd6d6a887483d5e00000000000000000000000000000000000000000000000000000028479a820000000000000000000000000000000000000000000000000000000000000001f40000000000000000000000000000000000000000000000000000000000000000");

            for _ in 0..1000 {
                client
                    .eth_call(
                        block,
                        address!("0x935442D6a9e2FA94C8F46772F301D5707145252A"),
                        address!("0x82825d0554fA07f7FC52Ab63c961F330fdEFa8E8"),
                        U256::ZERO,
                        &calldata,
                    )
                    .await
                    .unwrap();
            }
        });

        let now = std::time::Instant::now();
        let mut futures_unordered: FuturesUnordered<_> = call_stack.collect();
        while (futures_unordered.next().await).is_some() {}
        println!("Time taken for 1000 calls: {:?}", now.elapsed());
    }
}
