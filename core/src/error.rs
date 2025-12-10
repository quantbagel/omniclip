use thiserror::Error;

/// Omniclip error types
#[derive(Error, Debug)]
pub enum Error {
    #[error("Cryptographic operation failed: {0}")]
    Crypto(String),

    #[error("Failed to serialize/deserialize: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Discovery error: {0}")]
    Discovery(String),

    #[error("Clipboard error: {0}")]
    Clipboard(String),

    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    #[error("Device not paired: {0}")]
    NotPaired(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
