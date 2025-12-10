//! Base64 serialization utilities for serde
//!
//! This module provides reusable serde modules for serializing/deserializing
//! byte arrays and vectors as base64 strings in JSON.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Deserializer, Serializer};

/// Serialize/deserialize a `Vec<u8>` as a base64 string.
///
/// Usage:
/// ```ignore
/// #[serde(with = "crate::crypto::serde_utils::base64_bytes")]
/// pub field: Vec<u8>,
/// ```
pub mod base64_bytes {
    use super::*;

    pub fn serialize<S>(data: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64.encode(data))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        BASE64.decode(&s).map_err(serde::de::Error::custom)
    }
}

/// Serialize/deserialize a `[u8; 12]` array as a base64 string.
/// Commonly used for AES-GCM nonces.
///
/// Usage:
/// ```ignore
/// #[serde(with = "crate::crypto::serde_utils::base64_array_12")]
/// pub nonce: [u8; 12],
/// ```
pub mod base64_array_12 {
    use super::*;

    pub fn serialize<S>(data: &[u8; 12], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64.encode(data))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 12], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let bytes = BASE64.decode(&s).map_err(serde::de::Error::custom)?;
        bytes.try_into().map_err(|_| {
            serde::de::Error::custom("invalid length: expected 12 bytes")
        })
    }
}

/// Serialize/deserialize a `[u8; 32]` array as a base64 string.
/// Commonly used for keys and hashes.
///
/// Usage:
/// ```ignore
/// #[serde(with = "crate::crypto::serde_utils::base64_array_32")]
/// pub hash: [u8; 32],
/// ```
pub mod base64_array_32 {
    use super::*;

    pub fn serialize<S>(data: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64.encode(data))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let bytes = BASE64.decode(&s).map_err(serde::de::Error::custom)?;
        bytes.try_into().map_err(|_| {
            serde::de::Error::custom("invalid length: expected 32 bytes")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestVec {
        #[serde(with = "base64_bytes")]
        data: Vec<u8>,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestArray12 {
        #[serde(with = "base64_array_12")]
        nonce: [u8; 12],
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestArray32 {
        #[serde(with = "base64_array_32")]
        hash: [u8; 32],
    }

    #[test]
    fn test_base64_bytes_roundtrip() {
        let original = TestVec { data: vec![1, 2, 3, 4, 5] };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: TestVec = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_base64_array_12_roundtrip() {
        let original = TestArray12 { nonce: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: TestArray12 = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_base64_array_32_roundtrip() {
        let original = TestArray32 { hash: [42u8; 32] };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: TestArray32 = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded);
    }
}
