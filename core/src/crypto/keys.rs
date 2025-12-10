//! Key management for device identity and key exchange

use ed25519_dalek::{
    SigningKey as Ed25519SigningKey, VerifyingKey as Ed25519VerifyingKey,
    Signature, Signer, Verifier,
};
use x25519_dalek::{EphemeralSecret as X25519Secret, PublicKey as X25519Public, SharedSecret};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::{Error, Result};

/// Ed25519 signing key for device identity
#[derive(Clone)]
pub struct SigningKey {
    inner: Ed25519SigningKey,
}

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningKey")
            .field("public", &self.public_key_fingerprint())
            .finish()
    }
}

impl SigningKey {
    /// Generate a new random signing key
    pub fn generate() -> Self {
        Self {
            inner: Ed25519SigningKey::generate(&mut OsRng),
        }
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            inner: Ed25519SigningKey::from_bytes(bytes),
        }
    }

    /// Export as raw bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        self.inner.to_bytes()
    }

    /// Get the corresponding verifying (public) key
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey {
            inner: self.inner.verifying_key(),
        }
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.inner.sign(message).to_bytes().to_vec()
    }

    /// Get a human-readable fingerprint of the public key
    pub fn public_key_fingerprint(&self) -> String {
        self.verifying_key().fingerprint()
    }
}

/// Ed25519 verifying (public) key
#[derive(Clone)]
pub struct VerifyingKey {
    inner: Ed25519VerifyingKey,
}

// Custom Serialize/Deserialize to serialize as a bare base64 string (not a struct)
impl Serialize for VerifyingKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&BASE64.encode(self.inner.as_bytes()))
    }
}

impl<'de> Deserialize<'de> for VerifyingKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let bytes = BASE64.decode(&s).map_err(serde::de::Error::custom)?;
        let array: [u8; 32] = bytes.try_into().map_err(|_| {
            serde::de::Error::custom("invalid key length")
        })?;
        let inner = Ed25519VerifyingKey::from_bytes(&array).map_err(serde::de::Error::custom)?;
        Ok(VerifyingKey { inner })
    }
}

impl std::fmt::Debug for VerifyingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifyingKey")
            .field("fingerprint", &self.fingerprint())
            .finish()
    }
}

impl VerifyingKey {
    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self> {
        Ok(Self {
            inner: Ed25519VerifyingKey::from_bytes(bytes)
                .map_err(|e| Error::Crypto(e.to_string()))?,
        })
    }

    /// Export as raw bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        self.inner.to_bytes()
    }

    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<()> {
        let sig_bytes: [u8; 64] = signature.try_into()
            .map_err(|_| Error::Crypto("invalid signature length".to_string()))?;
        let sig = Signature::from_bytes(&sig_bytes);
        self.inner.verify(message, &sig)
            .map_err(|e| Error::Crypto(e.to_string()))
    }

    /// Get a human-readable fingerprint (first 8 bytes of SHA256, base64)
    pub fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.inner.as_bytes());
        let hash = hasher.finalize();
        BASE64.encode(&hash[..8])
    }
}

/// X25519 ephemeral secret for ECDH key exchange
pub struct EphemeralSecret {
    inner: X25519Secret,
}

impl EphemeralSecret {
    /// Generate a new ephemeral secret
    pub fn generate() -> Self {
        Self {
            inner: X25519Secret::random_from_rng(OsRng),
        }
    }

    /// Get the corresponding public key
    pub fn public_key(&self) -> PublicKey {
        PublicKey {
            inner: X25519Public::from(&self.inner),
        }
    }

    /// Perform ECDH key exchange
    pub fn diffie_hellman(self, their_public: &PublicKey) -> SharedSecret {
        self.inner.diffie_hellman(&their_public.inner)
    }
}

/// X25519 public key for ECDH
#[derive(Clone)]
pub struct PublicKey {
    inner: X25519Public,
}

// Custom Serialize/Deserialize to serialize as a bare base64 string (not a struct)
impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&BASE64.encode(self.inner.as_bytes()))
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let bytes = BASE64.decode(&s).map_err(serde::de::Error::custom)?;
        let array: [u8; 32] = bytes.try_into().map_err(|_| {
            serde::de::Error::custom("invalid key length")
        })?;
        Ok(PublicKey { inner: X25519Public::from(array) })
    }
}

impl std::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicKey")
            .field("bytes", &BASE64.encode(self.inner.as_bytes()))
            .finish()
    }
}

impl PublicKey {
    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            inner: X25519Public::from(bytes),
        }
    }

    /// Export as raw bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        *self.inner.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signing_roundtrip() {
        let key = SigningKey::generate();
        let message = b"hello world";
        let signature = key.sign(message);

        let verifying = key.verifying_key();
        assert!(verifying.verify(message, &signature).is_ok());
    }

    #[test]
    fn test_ecdh() {
        let alice_secret = EphemeralSecret::generate();
        let alice_public = alice_secret.public_key();

        let bob_secret = EphemeralSecret::generate();
        let bob_public = bob_secret.public_key();

        let alice_shared = alice_secret.diffie_hellman(&bob_public);
        let bob_shared = bob_secret.diffie_hellman(&alice_public);

        assert_eq!(alice_shared.as_bytes(), bob_shared.as_bytes());
    }

    #[test]
    fn test_fingerprint_consistency() {
        let key = SigningKey::generate();
        let fp1 = key.public_key_fingerprint();
        let fp2 = key.verifying_key().fingerprint();
        assert_eq!(fp1, fp2);
    }
}
