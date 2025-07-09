use std::task::{Context, Poll};

use alloy::primitives::{Address, TxKind, U256};
use alloy_sol_types::{SolCall, SolType};
use reth::revm::{
    Database, ExecuteEvm,
    context::result::{ExecutionResult, Output},
};
use reth_evm::{
    op_revm::{DefaultOp, OpBuilder},
    revm,
};

pub trait State {
    fn database(&mut self) -> &mut impl Database;
    fn call<C: SolCall>(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
        calldata: <C::Parameters<'_> as SolType>::RustType,
    ) -> Result<C::Return, String> {
        let database = self.database();
        let mut evm = revm::Context::op().with_db(database).build_op().0;

        let abi_encoded = C::new(calldata).abi_encode();

        evm.modify_cfg(|x| {
            x.disable_nonce_check = true;
            x.disable_base_fee = true;
            x.disable_block_gas_limit = true;
            x.disable_eip3607 = true;
        });

        evm.modify_tx(|x| {
            x.base.kind = TxKind::Call(to);
            x.base.value = value;
            x.base.data = abi_encoded.into();
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

                match C::abi_decode_returns(&output) {
                    Ok(x) => Ok(x),
                    Err(err) => Err(format!("Failed to decode abi response: {err}")),
                }
            }
            ExecutionResult::Revert { output, .. } => Err(format!("Reverted: {output}, state: {:?}", call_result.state)),
            ExecutionResult::Halt { reason, gas_used } => Err(format!("Halted: {reason:?}, gas used: {gas_used}, state: {:?}", call_result.state)),
        }
    }
}

pub trait StateProvider {
    // Poll, receive list of addresses that have changed state
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Vec<Address>>;
    fn state(&mut self) -> &mut impl State;
}
