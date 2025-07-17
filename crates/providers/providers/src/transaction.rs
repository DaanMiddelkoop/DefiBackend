use std::marker::PhantomData;

use crate::State;
use alloy::primitives::{Address, Bytes, U256};
use alloy_sol_types::SolCall;

pub struct Transaction<T> {
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub calldata: Bytes,
    pub return_data: PhantomData<T>,
}

impl<T: SolCall> Transaction<T> {
    pub async fn simulate(&self, state: &mut impl State) -> Result<T::Return, String> {
        let result = state.call_raw(self.from, self.to, self.value, self.calldata.clone()).await?;
        T::abi_decode_returns(&result).map_err(|err| format!("Failed to decode abi response: {err}"))
    }

    pub fn into_raw_response(self) -> Transaction<Bytes> {
        Transaction {
            from: self.from,
            to: self.to,
            value: self.value,
            calldata: self.calldata,
            return_data: PhantomData,
        }
    }

    pub fn sign(self) -> SignedTransaction {
        todo!()
    }
}

pub struct SignedTransaction {}

pub trait TransactionSubmitter {
    fn sumbit(&mut self, tx: SignedTransaction) -> impl Future<Output = ()>;
}
