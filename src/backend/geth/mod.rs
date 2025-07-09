use std::fmt::Display;

pub mod client;
pub mod subscription;

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
