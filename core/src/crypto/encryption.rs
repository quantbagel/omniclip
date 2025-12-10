//! Symmetric encryption using AES-256-GCM

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use x25519_dalek::SharedSecret;

use crate::protocol::constants::SESSION_KEY_INFO;
use crate::{Error, Result};

/// AES-256-GCM session key derived from ECDH shared secret
#[derive(Clone)]
pub struct SessionKey {
    cipher: Aes256Gcm,
}

impl std::fmt::Debug for SessionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionKey").finish_non_exhaustive()
    }
}

impl SessionKey {
    /// Derive a session key from an ECDH shared secret
    pub fn from_shared_secret(shared: &SharedSecret) -> Self {
        // Use HKDF-like derivation: SHA256(shared_secret || SESSION_KEY_INFO)
        let mut hasher = Sha256::new();
        hasher.update(shared.as_bytes());
        hasher.update(SESSION_KEY_INFO);
        let key_bytes = hasher.finalize();

        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .expect("SHA256 always produces 32 bytes");

        Self { cipher }
    }

    /// Create a session key from raw bytes (for persistence)
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new_from_slice(bytes)
            .expect("32 bytes is valid key length");
        Self { cipher }
    }

    /// Encrypt data with a random nonce
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedPayload> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self.cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| Error::Crypto(format!("encryption failed: {}", e)))?;

        Ok(EncryptedPayload {
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    /// Decrypt an encrypted payload
    pub fn decrypt(&self, payload: &EncryptedPayload) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(&payload.nonce);

        self.cipher
            .decrypt(nonce, payload.ciphertext.as_ref())
            .map_err(|e| Error::Crypto(format!("decryption failed: {}", e)))
    }
}

/// Encrypted data with its nonce
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    #[serde(with = "crate::crypto::serde_utils::base64_array_12")]
    pub nonce: [u8; 12],
    #[serde(with = "crate::crypto::serde_utils::base64_bytes")]
    pub ciphertext: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::EphemeralSecret;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        // Simulate key exchange
        let alice = EphemeralSecret::generate();
        let bob = EphemeralSecret::generate();

        let alice_pub = alice.public_key();
        let bob_pub = bob.public_key();

        let alice_shared = alice.diffie_hellman(&bob_pub);
        let bob_shared = bob.diffie_hellman(&alice_pub);

        let alice_key = SessionKey::from_shared_secret(&alice_shared);
        let bob_key = SessionKey::from_shared_secret(&bob_shared);

        // Alice encrypts
        let plaintext = b"Hello from Alice!";
        let encrypted = alice_key.encrypt(plaintext).unwrap();

        // Bob decrypts
        let decrypted = bob_key.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_different_nonces() {
        let alice = EphemeralSecret::generate();
        let bob = EphemeralSecret::generate();
        let shared = alice.diffie_hellman(&bob.public_key());
        let key = SessionKey::from_shared_secret(&shared);

        let plaintext = b"same message";
        let enc1 = key.encrypt(plaintext).unwrap();
        let enc2 = key.encrypt(plaintext).unwrap();

        // Same plaintext should produce different ciphertexts due to random nonce
        assert_ne!(enc1.ciphertext, enc2.ciphertext);
        assert_ne!(enc1.nonce, enc2.nonce);
    }
}
