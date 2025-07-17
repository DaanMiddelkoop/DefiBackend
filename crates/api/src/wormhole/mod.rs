use alloy::primitives::{Address, B256, Bytes, U256};
use alloy_sol_types::{SolCall, sol};
use base64::{Engine, prelude::BASE64_STANDARD};
use providers::transaction::Transaction;
use serde::Deserialize;

use crate::erc20::ERC20;

sol! {
    "src/wormhole/IWormhole.sol"
}

pub struct WormholeCore {
    address: Address,
}

impl WormholeCore {
    pub fn new(address: Address) -> Self {
        Self { address }
    }

    pub async fn message_fee(&self, state: &mut impl providers::State) -> Result<U256, String> {
        state.call::<IWormhole::messageFeeCall>(Address::ZERO, self.address, U256::ZERO, ()).await
    }
}

pub struct WormholeBridge {
    address: Address,
}

impl WormholeBridge {
    pub fn new(address: Address) -> Self {
        Self { address }
    }

    pub fn transfer_tokens(
        &self,
        from: Address,
        to: Address,
        token: ERC20,
        amount: U256,
        recipient_chain: &Wormhole,
        wormhole_fee: U256,
    ) -> Transaction<ITokenBridge::transferTokensCall> {
        let random = rand::random::<u32>();

        let calldata =
            ITokenBridge::transferTokensCall::new((token.address, amount, recipient_chain.chain_id, to.into_word(), U256::ZERO, random)).abi_encode();
        Transaction {
            from,
            to: self.address,
            value: wormhole_fee,
            calldata: calldata.into(),
            return_data: std::marker::PhantomData,
        }
    }

    pub fn complete_transfer(&self, from: Address, signature: Bytes) -> Transaction<ITokenBridge::completeTransferCall> {
        let calldata = ITokenBridge::completeTransferCall::new((signature,)).abi_encode();

        Transaction {
            from,
            to: self.address,
            value: U256::ZERO,
            calldata: calldata.into(),
            return_data: std::marker::PhantomData,
        }
    }
}

pub struct Wormhole {
    pub chain_id: u16,
    pub core: WormholeCore,
    pub bridge: WormholeBridge,
}

impl Wormhole {
    pub fn new(chain: u16, core: Address, bridge: Address) -> Self {
        Self {
            chain_id: chain,
            core: WormholeCore::new(core),
            bridge: WormholeBridge::new(bridge),
        }
    }
}

#[derive(Deserialize)]
struct VAAResponse {
    data: Vec<Vaa>,
}

#[derive(Deserialize)]
struct Vaa {
    vaa: String,
}

pub async fn fetch_wormhole_vaa(txid: B256) -> Result<Option<Bytes>, String> {
    let url = format!("https://api.wormholescan.io/api/v1/vaas?txHash={txid}");
    let response = surf::get(url).recv_string().await.map_err(|e| e.to_string())?;

    let vaas = serde_json::from_str::<VAAResponse>(&response).map_err(|x| format!("Failed to deserialize {response}: {x}"))?;

    Ok(vaas.data.into_iter().next().map(|x| BASE64_STANDARD.decode(x.vaa).unwrap().into()))
}

#[cfg(test)]
mod test {
    use alloy::primitives::b256;

    #[tokio::test]
    async fn test_fetch_wormhole_vaa() {
        let txid = b256!("0xa76b0e67783d05a6cdebe4d5b414d87ad464e2bec44cafdd0637293b8d2da51a");
        let result = super::fetch_wormhole_vaa(txid).await;
        println!("Result: {result:?}");
    }
}
