//! Peer connection handling

use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::crypto::SessionKey;
use crate::protocol::Message;
use crate::{Error, Result};

/// Active connection to a peer
pub struct PeerConnection {
    pub peer_id: Uuid,
    pub peer_name: String,
    stream: TcpStream,
    session_key: SessionKey,
}

impl PeerConnection {
    /// Create a new peer connection from an established stream
    pub fn new(
        peer_id: Uuid,
        peer_name: String,
        stream: TcpStream,
        session_key: SessionKey,
    ) -> Self {
        Self {
            peer_id,
            peer_name,
            stream,
            session_key,
        }
    }

    /// Connect to a peer
    pub async fn connect(
        addr: SocketAddr,
        peer_id: Uuid,
        peer_name: String,
        session_key: SessionKey,
    ) -> Result<Self> {
        let stream = TcpStream::connect(addr)
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        Ok(Self::new(peer_id, peer_name, stream, session_key))
    }

    /// Send a message to the peer
    pub async fn send(&mut self, message: &Message) -> Result<()> {
        let frame = message.to_frame()
            .map_err(|e| Error::Serialization(e))?;

        self.stream
            .write_all(&frame)
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        self.stream
            .flush()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        Ok(())
    }

    /// Receive a message from the peer
    pub async fn recv(&mut self) -> Result<Message> {
        // Read length prefix (4 bytes, big-endian)
        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        let len = u32::from_be_bytes(len_buf) as usize;

        // Sanity check on message size (max 10MB)
        if len > 10 * 1024 * 1024 {
            return Err(Error::InvalidMessage("message too large".to_string()));
        }

        // Read payload
        let mut payload = vec![0u8; len];
        self.stream
            .read_exact(&mut payload)
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        Message::from_bytes(&payload)
            .map_err(|e| Error::Serialization(e))
    }

    /// Get the session key for encrypting clipboard content
    pub fn session_key(&self) -> &SessionKey {
        &self.session_key
    }

    /// Get peer address
    pub fn peer_addr(&self) -> Result<SocketAddr> {
        self.stream
            .peer_addr()
            .map_err(|e| Error::Network(e.to_string()))
    }

    /// Split into read and write halves for concurrent processing
    pub fn into_split(self) -> (PeerConnectionReader, PeerConnectionWriter) {
        let (read_half, write_half) = self.stream.into_split();
        (
            PeerConnectionReader {
                peer_id: self.peer_id,
                stream: read_half,
            },
            PeerConnectionWriter {
                peer_id: self.peer_id,
                stream: write_half,
            },
        )
    }
}

/// Read half of a peer connection
pub struct PeerConnectionReader {
    pub peer_id: Uuid,
    stream: tokio::net::tcp::OwnedReadHalf,
}

impl PeerConnectionReader {
    /// Receive a message
    pub async fn recv(&mut self) -> Result<Message> {
        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        let len = u32::from_be_bytes(len_buf) as usize;

        if len > 10 * 1024 * 1024 {
            return Err(Error::InvalidMessage("message too large".to_string()));
        }

        let mut payload = vec![0u8; len];
        self.stream
            .read_exact(&mut payload)
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        Message::from_bytes(&payload)
            .map_err(|e| Error::Serialization(e))
    }
}

/// Write half of a peer connection
pub struct PeerConnectionWriter {
    pub peer_id: Uuid,
    stream: tokio::net::tcp::OwnedWriteHalf,
}

impl PeerConnectionWriter {
    /// Send a message
    pub async fn send(&mut self, message: &Message) -> Result<()> {
        let frame = message.to_frame()
            .map_err(|e| Error::Serialization(e))?;

        self.stream
            .write_all(&frame)
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        self.stream
            .flush()
            .await
            .map_err(|e| Error::Network(e.to_string()))
    }
}
