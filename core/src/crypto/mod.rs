//! Cryptographic primitives for Omniclip
//!
//! - Ed25519 for device identity and signing
//! - X25519 for ECDH key exchange
//! - AES-256-GCM for symmetric encryption

mod keys;
mod encryption;
pub mod serde_utils;

pub use keys::{SigningKey, VerifyingKey, EphemeralSecret, PublicKey};
pub use encryption::{SessionKey, EncryptedPayload};
