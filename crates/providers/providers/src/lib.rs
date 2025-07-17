pub mod transaction;

use std::task::{Context, Poll};

use alloy::primitives::{Address, TxKind, U256};
use alloy_sol_types::{SolCall, SolType};
use op_revm::{DefaultOp, OpBuilder};
use revm::{
    Database, ExecuteEvm,
    context::result::{ExecutionResult, Output},
    primitives::Bytes,
};

#[allow(async_fn_in_trait)]
pub trait State {
    fn database(&mut self) -> &mut impl Database;
    fn current_block(&self) -> u64;

    async fn call_raw(&mut self, from: Address, to: Address, value: U256, calldata: Bytes) -> Result<Bytes, String> {
        let database = self.database();
        let mut evm = revm::Context::op().with_db(database).build_op().0;

        evm.modify_cfg(|x| {
            x.disable_nonce_check = true;
        });

        evm.modify_tx(|x| {
            x.base.kind = TxKind::Call(to);
            x.base.value = value;
            x.base.data = calldata;
            x.base.caller = from;
            x.base.gas_limit = 1000000;
        });

        let call_result = evm.replay().unwrap();

        match call_result.result {
            ExecutionResult::Success { output, .. } => {
                let output = match output {
                    Output::Call(bytes) => bytes,
                    Output::Create(bytes, ..) => bytes,
                };

                Ok(output)
            }
            ExecutionResult::Revert { output, .. } => Err(format!("Reverted: {output}, state: {:?}", call_result.state)),
            ExecutionResult::Halt { reason, gas_used } => Err(format!("Halted: {reason:?}, gas used: {gas_used}, state: {:?}", call_result.state)),
        }
    }

    async fn call<C: SolCall>(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
        calldata: <C::Parameters<'_> as SolType>::RustType,
    ) -> Result<C::Return, String> {
        let abi_encoded = C::new(calldata).abi_encode();
        let result = self.call_raw(from, to, value, abi_encoded.into()).await?;
        match C::abi_decode_returns(&result) {
            Ok(x) => Ok(x),
            Err(err) => Err(format!("Failed to decode abi response: {err}")),
        }
    }
}

pub trait StateProvider {
    fn chain_id(&self) -> u16;
    // Poll, receive list of addresses that have changed state
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Vec<Address>>;
    fn state(&mut self) -> &mut impl State;
}
