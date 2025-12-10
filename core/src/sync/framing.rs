//! Length-prefixed message framing for TCP transport
//!
//! This module provides utilities for reading and writing length-prefixed
//! messages over TCP streams. Each message is prefixed with a 4-byte
//! big-endian length, followed by the payload.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::protocol::constants::MAX_MESSAGE_SIZE;
use crate::{Error, Result};

/// Read a length-prefixed message from an async reader.
///
/// The wire format is:
/// - 4 bytes: big-endian u32 length
/// - N bytes: message payload
///
/// Returns an error if the message exceeds MAX_MESSAGE_SIZE.
pub async fn read_framed_message<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    // Read 4-byte length prefix
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await
        .map_err(|e| Error::Network(e.to_string()))?;

    let len = u32::from_be_bytes(len_buf) as usize;

    // Validate message size
    if len > MAX_MESSAGE_SIZE {
        return Err(Error::InvalidMessage(format!(
            "message too large: {} bytes (max {})",
            len, MAX_MESSAGE_SIZE
        )));
    }

    // Read payload
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await
        .map_err(|e| Error::Network(e.to_string()))?;

    Ok(payload)
}

/// Write a length-prefixed message to an async writer.
///
/// The wire format is:
/// - 4 bytes: big-endian u32 length
/// - N bytes: message payload
///
/// Returns an error if the message exceeds MAX_MESSAGE_SIZE.
pub async fn write_framed_message<W: AsyncWrite + Unpin>(
    writer: &mut W,
    payload: &[u8],
) -> Result<()> {
    // Validate message size
    if payload.len() > MAX_MESSAGE_SIZE {
        return Err(Error::InvalidMessage(format!(
            "message too large: {} bytes (max {})",
            payload.len(), MAX_MESSAGE_SIZE
        )));
    }

    // Write length prefix
    let len_bytes = (payload.len() as u32).to_be_bytes();
    writer.write_all(&len_bytes).await
        .map_err(|e| Error::Network(e.to_string()))?;

    // Write payload
    writer.write_all(payload).await
        .map_err(|e| Error::Network(e.to_string()))?;

    // Flush to ensure data is sent
    writer.flush().await
        .map_err(|e| Error::Network(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn test_framing_roundtrip() {
        let original = b"Hello, World!";

        // Write to buffer
        let mut buffer = Vec::new();
        write_framed_message(&mut buffer, original).await.unwrap();

        // Read back
        let mut cursor = Cursor::new(buffer);
        let result = read_framed_message(&mut cursor).await.unwrap();

        assert_eq!(result, original);
    }

    #[tokio::test]
    async fn test_framing_empty_message() {
        let original = b"";

        let mut buffer = Vec::new();
        write_framed_message(&mut buffer, original).await.unwrap();

        let mut cursor = Cursor::new(buffer);
        let result = read_framed_message(&mut cursor).await.unwrap();

        assert_eq!(result, original);
    }

    #[tokio::test]
    async fn test_message_too_large() {
        let large_payload = vec![0u8; MAX_MESSAGE_SIZE + 1];
        let mut buffer = Vec::new();

        let result = write_framed_message(&mut buffer, &large_payload).await;
        assert!(result.is_err());
    }
}
