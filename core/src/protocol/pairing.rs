//! Pairing session management and QR code generation

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as BASE64URL};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::{EphemeralSecret, PublicKey, SigningKey, SessionKey};
use crate::{Error, Result};

/// Active pairing session state
pub struct PairingSession {
    pub session_id: Uuid,
    pub ephemeral_secret: EphemeralSecret,
    pub ephemeral_public: PublicKey,
}

impl PairingSession {
    /// Start a new pairing session
    pub fn new() -> Self {
        let ephemeral_secret = EphemeralSecret::generate();
        let ephemeral_public = ephemeral_secret.public_key();

        Self {
            session_id: Uuid::new_v4(),
            ephemeral_secret,
            ephemeral_public,
        }
    }

    /// Generate QR code data for this session
    pub fn qr_data(&self, local_ip: &str, port: u16, device_name: &str) -> PairingQrData {
        PairingQrData {
            session_id: self.session_id,
            pubkey: self.ephemeral_public.to_bytes(),
            ip: local_ip.to_string(),
            port,
            name: device_name.to_string(),
        }
    }

    /// Complete pairing with peer's public key, derive session key
    pub fn complete(self, peer_pubkey: &PublicKey) -> SessionKey {
        let shared = self.ephemeral_secret.diffie_hellman(peer_pubkey);
        SessionKey::from_shared_secret(&shared)
    }

    /// Sign the pairing data for verification
    pub fn sign_pairing(&self, signing_key: &SigningKey, peer_pubkey: &PublicKey) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend(self.session_id.as_bytes());
        data.extend(self.ephemeral_public.to_bytes());
        data.extend(peer_pubkey.to_bytes());
        signing_key.sign(&data)
    }
}

impl Default for PairingSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Data encoded in pairing QR code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingQrData {
    pub session_id: Uuid,
    pub pubkey: [u8; 32],
    pub ip: String,
    pub port: u16,
    pub name: String,
}

impl PairingQrData {
    /// Encode as a URL for QR code
    pub fn to_url(&self) -> String {
        let pubkey_b64 = BASE64URL.encode(self.pubkey);
        format!(
            "omniclip://pair?s={}&k={}&h={}&p={}&n={}",
            self.session_id,
            pubkey_b64,
            urlencoding::encode(&self.ip),
            self.port,
            urlencoding::encode(&self.name),
        )
    }

    /// Parse from URL
    pub fn from_url(url: &str) -> Result<Self> {
        let url = url.strip_prefix("omniclip://pair?")
            .ok_or_else(|| Error::InvalidMessage("invalid scheme".to_string()))?;

        let mut session_id = None;
        let mut pubkey = None;
        let mut ip = None;
        let mut port = None;
        let mut name = None;

        for part in url.split('&') {
            let (key, value) = part.split_once('=')
                .ok_or_else(|| Error::InvalidMessage("invalid param".to_string()))?;

            match key {
                "s" => session_id = Some(Uuid::parse_str(value)
                    .map_err(|_| Error::InvalidMessage("invalid session id".to_string()))?),
                "k" => {
                    let bytes = BASE64URL.decode(value)
                        .map_err(|_| Error::InvalidMessage("invalid pubkey".to_string()))?;
                    let arr: [u8; 32] = bytes.try_into()
                        .map_err(|_| Error::InvalidMessage("invalid pubkey length".to_string()))?;
                    pubkey = Some(arr);
                }
                "h" => ip = Some(urlencoding::decode(value)
                    .map_err(|_| Error::InvalidMessage("invalid ip".to_string()))?
                    .to_string()),
                "p" => port = Some(value.parse()
                    .map_err(|_| Error::InvalidMessage("invalid port".to_string()))?),
                "n" => name = Some(urlencoding::decode(value)
                    .map_err(|_| Error::InvalidMessage("invalid name".to_string()))?
                    .to_string()),
                _ => {}
            }
        }

        Ok(Self {
            session_id: session_id.ok_or_else(|| Error::InvalidMessage("missing session_id".to_string()))?,
            pubkey: pubkey.ok_or_else(|| Error::InvalidMessage("missing pubkey".to_string()))?,
            ip: ip.ok_or_else(|| Error::InvalidMessage("missing ip".to_string()))?,
            port: port.ok_or_else(|| Error::InvalidMessage("missing port".to_string()))?,
            name: name.ok_or_else(|| Error::InvalidMessage("missing name".to_string()))?,
        })
    }

    /// Generate QR code as SVG string
    pub fn to_qr_svg(&self) -> Result<String> {
        use qrcode::{QrCode, render::svg};

        let url = self.to_url();
        let code = QrCode::new(url.as_bytes())
            .map_err(|e| Error::Crypto(format!("QR generation failed: {}", e)))?;

        let svg = code.render::<svg::Color>()
            .min_dimensions(200, 200)
            .build();

        Ok(svg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qr_url_roundtrip() {
        let session = PairingSession::new();
        let qr_data = session.qr_data("192.168.1.100", 17394, "My Device");

        let url = qr_data.to_url();
        let parsed = PairingQrData::from_url(&url).unwrap();

        assert_eq!(parsed.session_id, qr_data.session_id);
        assert_eq!(parsed.pubkey, qr_data.pubkey);
        assert_eq!(parsed.ip, qr_data.ip);
        assert_eq!(parsed.port, qr_data.port);
        assert_eq!(parsed.name, qr_data.name);
    }

    #[test]
    fn test_pairing_key_derivation() {
        // Device A starts session
        let session_a = PairingSession::new();
        let pubkey_a = session_a.ephemeral_public.clone();

        // Device B starts session and gets A's public key from QR
        let session_b = PairingSession::new();
        let pubkey_b = session_b.ephemeral_public.clone();

        // Both derive session keys
        let key_a = session_a.complete(&pubkey_b);
        let key_b = session_b.complete(&pubkey_a);

        // Keys should work for encryption/decryption
        let plaintext = b"test message";
        let encrypted = key_a.encrypt(plaintext).unwrap();
        let decrypted = key_b.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
