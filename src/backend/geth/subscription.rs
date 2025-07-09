use std::{
    marker::PhantomData,
    task::{Context, Poll, ready},
    time::Duration,
};

use alloy::primitives::{B256, U64};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

use crate::{backend::geth::GethError, connection::Connection};

type Result<T> = std::result::Result<T, GethError>;

#[derive(Debug, Deserialize)]
struct SubscriptionResponse<T> {
    params: SubscriptionParams<T>,
}

#[derive(Debug, Deserialize)]

struct SubscriptionParams<T> {
    result: T,
}

pub struct Subscription<T> {
    connection: Connection,
    data: PhantomData<T>,
}

#[allow(unused)]
#[derive(Deserialize)]
pub struct SubscriptionConfirmation {
    result: String,
}

impl<T: DeserializeOwned> Subscription<T> {
    pub async fn connect(topic: impl Serialize, url: &str, timeout: Duration) -> Result<Self> {
        let mut connection = Connection::new(url, timeout).await.map_err(GethError::ConnectionError)?;
        let message = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_subscribe",
            "params": topic
        });
        connection
            .send(Message::Text(message.to_string().into()))
            .map_err(GethError::ConnectionError)?;

        let response = connection.recv().await.map_err(GethError::ConnectionError)?;
        let _: SubscriptionConfirmation = match serde_json::from_str(&response) {
            Ok(x) => x,
            Err(err) => return Err(GethError::DeserializeError(format!("While deserializing confirmation: {err}"))),
        };

        Ok(Self {
            connection,
            data: PhantomData,
        })
    }

    pub fn next(&mut self, cx: &mut Context<'_>) -> Poll<Result<T>> {
        let message = match ready!(self.connection.poll(cx)) {
            Ok(x) => x,
            Err(err) => return Poll::Ready(Err(GethError::ConnectionError(err))),
        };

        let deserialized: SubscriptionResponse<T> = match serde_json::from_str(&message) {
            Ok(x) => x,
            Err(err) => return Poll::Ready(Err(GethError::DeserializeError(format!("While deserializing {message}: {err}")))),
        };

        Poll::Ready(Ok(deserialized.params.result))
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct NewHeads {
    pub number: U64,
    pub hash: B256,
}

pub async fn new_heads(url: &str) -> Result<Subscription<NewHeads>> {
    Subscription::<NewHeads>::connect(["newHeads"], url, Duration::from_secs(10)).await
}

#[cfg(test)]
mod tests {
    use futures::future::poll_fn;

    use super::*;

    #[tokio::test]
    async fn test_new_heads_subscription() {
        let url = "ws://10.8.0.1:8546"; // Replace with your WebSocket URL
        let subscription = new_heads(url).await;
        let mut sub = subscription.unwrap_or_else(|_| panic!("Failed to create subscription"));

        let message = poll_fn(|cx| sub.next(cx))
            .await
            .unwrap_or_else(|err| panic!("Failed to receive message: {err}"));
        // assert!(message.number > 0, "Received new head with number: {}", message.number);
        println!("Received new head: {message:?}");
    }
}
