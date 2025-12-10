//! Centralized protocol constants
//!
//! All protocol-level constants are defined here to ensure consistency
//! across the codebase and make configuration easier.

/// Default TCP port for the sync server
pub const DEFAULT_PORT: u16 = 17394;

/// mDNS service type for discovery
pub const SERVICE_TYPE: &str = "_omniclip._tcp.local.";

/// URL scheme prefix for pairing QR codes
pub const PAIRING_URL_SCHEME: &str = "omniclip://pair";

/// Info string used in session key derivation (HKDF-like)
pub const SESSION_KEY_INFO: &[u8] = b"omniclip-session-key";

/// Maximum message size (10 MB)
pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// Current protocol version
pub const PROTOCOL_VERSION: u16 = 1;

/// Clipboard polling interval in milliseconds
pub const CLIPBOARD_POLL_INTERVAL_MS: u64 = 500;
