use std::{convert::Infallible, fmt::Display};

use ::reth::revm::{
    db::{CacheDB, DBErrorMarker, InMemoryDB},
    primitives::{StorageKey, StorageValue},
    state::{AccountInfo, Bytecode},
};
use alloy::primitives::{Address, B256};
use reth_evm::revm::Database;

pub mod geth;
pub mod reth;

// pub struct FallbackDatabase<'a, T> {
//     database: &'a mut CacheDB<NoDB>,
//     fallback: T,
// }

// impl<'a, T: Database> Database for FallbackDatabase<'a, T> {
//     #[doc = " The database error type."]
//     type Error = T::Error;

//     #[doc = " Gets basic account information."]
//     fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
//         if let Ok(Some(account)) = self.database.basic(address) {
//             return Ok(Some(account));
//         }
//         self.fallback.basic(address)
//     }

//     #[doc = " Gets account code by its hash."]
//     fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
//         if let Ok(Some(code)) = self.database.code_by_hash(code_hash) {
//             return Ok(code);
//         }
//         self.fallback.code_by_hash(code_hash)
//     }

//     #[doc = " Gets storage value of address at index."]
//     fn storage(&mut self, address: Address, index: StorageKey) -> Result<StorageValue, Self::Error> {
//         if let Ok(value) = self.database.storage(address, index) {
//             return Ok(value);
//         }
//         self.fallback.storage(address, index)
//     }

//     #[doc = " Gets block hash by block number."]
//     fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
//         if let Some(hash) = self.database.block_hash(number)? {
//             return Ok(hash);
//         }
//         self.fallback.block_hash(number)
//     }
// }

// pub struct NoDB {}

// #[derive(Debug)]
// pub struct NoDBError;
// impl std::error::Error for NoDBError {}
// impl Display for NoDBError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "NoDB does not support any operations")
//     }
// }
// impl DBErrorMarker for NoDBError {}

// impl Database for NoDB {
//     type Error = NoDBError;

//     fn basic(&mut self, _address: Address) -> Result<Option<AccountInfo>, Self::Error> {
//         Err(NoDBError)
//     }

//     fn code_by_hash(&mut self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
//         Err(NoDBError)
//     }

//     fn storage(&mut self, _address: Address, _index: StorageKey) -> Result<StorageValue, Self::Error> {
//         Err(NoDBError)
//     }

//     fn block_hash(&mut self, _number: u64) -> Result<B256, Self::Error> {
//         Err(NoDBError)
//     }
// }

// #[cfg(test)]
// mod test {}
