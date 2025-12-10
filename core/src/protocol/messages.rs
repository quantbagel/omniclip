//! Protocol message definitions

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;

use crate::crypto::{EncryptedPayload, PublicKey, VerifyingKey};

/// All protocol messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Device announces presence on the network
    Announce(AnnounceMessage),

    /// Request to pair with another device
    PairRequest(PairRequestMessage),

    /// Accept a pairing request
    PairAccept(PairAcceptMessage),

    /// Reject a pairing request
    PairReject { session_id: Uuid, reason: String },

    /// Sync clipboard content to paired devices
    ClipboardSync(ClipboardSyncMessage),

    /// Acknowledge receipt of a message
    Ack { message_id: Uuid },

    /// Ping to check if peer is alive
    Ping { timestamp: u64 },

    /// Response to ping
    Pong { timestamp: u64 },
}

impl Message {
    /// Serialize message to bytes using JSON (for cross-platform compatibility)
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize message from bytes (JSON)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Create a length-prefixed frame for TCP transport
    pub fn to_frame(&self) -> Result<Vec<u8>, serde_json::Error> {
        let payload = self.to_bytes()?;
        let len = payload.len() as u32;
        let mut frame = len.to_be_bytes().to_vec();
        frame.extend(payload);
        Ok(frame)
    }
}

/// Device announcement for discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnounceMessage {
    pub device_id: Uuid,
    pub device_name: String,
    pub pubkey_fingerprint: String,
    pub protocol_version: u16,
}

/// Pairing request (step 1 of pairing handshake)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairRequestMessage {
    pub session_id: Uuid,
    pub device_id: Uuid,
    pub device_name: String,
    pub ephemeral_pubkey: PublicKey,
    pub identity_pubkey: VerifyingKey,
}

/// Pairing acceptance (step 2 of pairing handshake)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairAcceptMessage {
    pub session_id: Uuid,
    pub device_id: Uuid,
    pub device_name: String,
    pub ephemeral_pubkey: PublicKey,
    pub identity_pubkey: VerifyingKey,
    /// Signature over session_id || both ephemeral pubkeys
    #[serde(with = "crate::crypto::serde_utils::base64_bytes")]
    pub signature: Vec<u8>,
}

/// Clipboard content sync message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardSyncMessage {
    pub message_id: Uuid,
    pub sender_id: Uuid,
    pub content_hash: ContentHash,
    pub encrypted_content: EncryptedPayload,
    pub timestamp: u64,
}

/// Clipboard content types (text only for MVP)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardContent {
    /// Plain text content
    Text(String),
    /// Rich text (HTML)
    RichText { plain: String, html: String },
}

impl ClipboardContent {
    /// Compute hash of content for deduplication
    pub fn hash(&self) -> ContentHash {
        let mut hasher = Sha256::new();
        match self {
            ClipboardContent::Text(text) => {
                hasher.update(b"text:");
                hasher.update(text.as_bytes());
            }
            ClipboardContent::RichText { plain, html } => {
                hasher.update(b"rich:");
                hasher.update(plain.as_bytes());
                hasher.update(html.as_bytes());
            }
        }
        ContentHash(hasher.finalize().into())
    }

    /// Serialize for encryption (using JSON for cross-platform compatibility)
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from decrypted bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

/// SHA256 hash of clipboard content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash(#[serde(with = "crate::crypto::serde_utils::base64_array_32")] pub [u8; 32]);

impl ContentHash {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_roundtrip() {
        let msg = Message::Announce(AnnounceMessage {
            device_id: Uuid::new_v4(),
            device_name: "Test Device".to_string(),
            pubkey_fingerprint: "abc123".to_string(),
            protocol_version: 1,
        });

        let bytes = msg.to_bytes().unwrap();
        let decoded = Message::from_bytes(&bytes).unwrap();

        match decoded {
            Message::Announce(a) => {
                assert_eq!(a.device_name, "Test Device");
            }
            _ => panic!("wrong message type"),
        }
    }

    #[test]
    fn test_content_hash_consistency() {
        let content = ClipboardContent::Text("hello".to_string());
        let hash1 = content.hash();
        let hash2 = content.hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_different() {
        let content1 = ClipboardContent::Text("hello".to_string());
        let content2 = ClipboardContent::Text("world".to_string());
        assert_ne!(content1.hash(), content2.hash());
    }
}
