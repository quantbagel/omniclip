//! Omniclip Core - Cross-platform clipboard sync library
//!
//! This library provides the core functionality for syncing clipboard
//! content across devices over LAN using mDNS discovery and encrypted
//! peer-to-peer connections.

pub mod clipboard;
pub mod crypto;
pub mod discovery;
pub mod protocol;
pub mod service;
pub mod sync;

mod error;

pub use error::{Error, Result};

/// Device identity containing keys and metadata
#[derive(Debug, Clone)]
pub struct DeviceIdentity {
    pub id: uuid::Uuid,
    pub name: String,
    pub signing_key: crypto::SigningKey,
}

impl DeviceIdentity {
    /// Create a new device identity with generated keys
    pub fn new(name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name,
            signing_key: crypto::SigningKey::generate(),
        }
    }

    /// Get the public key fingerprint for display/verification
    pub fn fingerprint(&self) -> String {
        self.signing_key.public_key_fingerprint()
    }
}

/// Configuration for the Omniclip service
#[derive(Debug, Clone)]
pub struct Config {
    /// Port to listen on for incoming connections
    pub port: u16,
    /// mDNS service name
    pub service_name: String,
    /// Path to store persistent data (keys, paired devices)
    pub data_dir: std::path::PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: protocol::constants::DEFAULT_PORT,
            service_name: protocol::constants::SERVICE_TYPE.to_string(),
            data_dir: dirs_home().join(".omniclip"),
        }
    }
}

fn dirs_home() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
}

// Re-export key types for convenience
pub use crypto::{EncryptedPayload, SessionKey};
pub use discovery::PeerInfo;
pub use protocol::{ClipboardContent, Message};
pub use service::{OmniclipService, ServiceEvent};
