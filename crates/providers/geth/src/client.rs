use std::{
    collections::BTreeMap,
    task::{Poll, ready},
    time::Duration,
};

use alloy::{
    eips::BlockNumberOrTag,
    hex,
    primitives::{Address, B256, U16, U64, U256},
};
use base::connection::{Connection, Message};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;

use super::GethError;

pub struct GethClient {
    connection: Connection,
}

#[allow(unused)]
#[derive(Deserialize)]
struct Response<T> {
    result: T,
}

#[derive(Deserialize)]
pub struct KeyValue {
    pub key: Option<B256>,
    pub value: U256,
}

impl GethClient {
    pub async fn connect(url: &str, timeout: Duration) -> Result<Self, GethError> {
        let connection = Connection::new(url, timeout).await.map_err(GethError::ConnectionError)?;
        Ok(Self { connection })
    }

    pub fn poll(&mut self, cx: &mut std::task::Context<'_>) -> Poll<()> {
        let _ = ready!(self.connection.poll(cx));
        Poll::Ready(())
    }

    pub async fn chain_id(&mut self) -> Result<u16, GethError> {
        self.request::<U16>("eth_chainId", &[] as &[u8]).await.map(|x| u16::try_from(x).unwrap())
    }

    pub async fn request<T: DeserializeOwned>(&mut self, method: &str, params: impl Serialize) -> Result<T, GethError> {
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        self.connection
            .send(Message::Text(message.to_string().into()))
            .map_err(GethError::ConnectionError)?;

        let response = self.connection.recv().await.map_err(GethError::ConnectionError)?;
        let response: Response<T> =
            serde_json::from_str(&response).map_err(|err| GethError::DeserializeError(format!("While deserializing {response} response: {err}")))?;
        Ok(response.result)
    }

    pub async fn block_number(&mut self) -> Result<u64, GethError> {
        self.request::<U64>("eth_blockNumber", &[] as &[u8])
            .await
            .map(|x| u64::try_from(x).unwrap())
    }

    pub async fn debug_storage_range_at(&mut self, block: BlockNumberOrTag, address: Address) -> Result<BTreeMap<B256, KeyValue>, GethError> {
        let mut next_key = Some(B256::ZERO);

        #[derive(Deserialize)]
        pub struct StorageRangeResponse {
            storage: BTreeMap<B256, KeyValue>,
            next_key: Option<B256>,
        }

        let mut storage = BTreeMap::<B256, KeyValue>::new();

        while let Some(key) = next_key {
            println!("Fetching storage at key: {key}");
            let values = self
                .request::<StorageRangeResponse>("debug_storageRangeAt", json!([block, 0, address, key, 10000]))
                .await?;
            next_key = values.next_key;
            storage.extend(values.storage);
        }

        Ok(storage)
    }

    pub async fn eth_call(&mut self, block: BlockNumberOrTag, from: Address, to: Address, value: U256, data: &[u8]) -> Result<Vec<u8>, GethError> {
        let params = json!([{
            "from": from,
            "to": to,
            "value": value,
            "data": hex::encode_prefixed(data)
        }, block]);

        let response = self.request::<String>("eth_call", params).await?;
        hex::decode(response).map_err(|err| GethError::DeserializeError(format!("Failed to decode response: {err}")))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::time::Duration;

    use alloy::primitives::{U64, address};

    #[tokio::test]
    async fn test_geth_client() {
        let url = "ws://10.8.0.1:8546";
        let mut client = GethClient::connect(url, Duration::from_secs(5))
            .await
            .expect("Failed to connect to Geth client");
        let block_number: U64 = client.request("eth_blockNumber", &[] as &[u8]).await.expect("Failed to get block number");
        println!("Current block number: {block_number}");
    }

    #[tokio::test]
    async fn test_debug_storage_range_at() {
        let url = "ws://10.8.0.1:8546";
        let mut client = GethClient::connect(url, Duration::from_secs(5))
            .await
            .expect("Failed to connect to Geth client");

        let address = address!("0x6cde5f5a192fBf3fD84df983aa6DC30dbd9f8Fac");
        let storage = client
            .debug_storage_range_at(BlockNumberOrTag::Latest, address)
            .await
            .unwrap_or_else(|x| panic!("Failed to get storage range: {x}"));
        println!("Storage keys: {}", storage.len());
    }
}
