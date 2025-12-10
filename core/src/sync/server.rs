//! TCP server for accepting peer connections

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::crypto::SessionKey;
use crate::protocol::{Message, PairAcceptMessage, PairingSession};
use crate::sync::framing::{read_framed_message, write_framed_message};
use crate::{DeviceIdentity, Error, Result};

/// Event from the sync server
#[derive(Debug)]
pub enum SyncEvent {
    /// New peer connected
    PeerConnected { peer_id: Uuid, peer_name: String },
    /// Peer disconnected
    PeerDisconnected { peer_id: Uuid },
    /// Message received from peer
    MessageReceived { peer_id: Uuid, message: Message },
    /// Device was paired successfully
    DevicePaired { device: PairedDevice },
}

/// Paired device info for connection handling
#[derive(Clone, Debug)]
pub struct PairedDevice {
    pub device_id: Uuid,
    pub device_name: String,
    pub session_key: SessionKey,
}

/// TCP sync server
pub struct SyncServer {
    listener: TcpListener,
    port: u16,
    paired_devices: Arc<RwLock<HashMap<Uuid, PairedDevice>>>,
}

impl SyncServer {
    /// Bind to a port and create the server
    pub async fn bind(port: u16) -> Result<Self> {
        let addr: SocketAddr = ([0, 0, 0, 0], port).into();
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| Error::Network(format!("failed to bind: {}", e)))?;

        let actual_port = listener.local_addr()
            .map_err(|e| Error::Network(e.to_string()))?
            .port();

        tracing::info!("sync server listening on port {}", actual_port);

        Ok(Self {
            listener,
            port: actual_port,
            paired_devices: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get the port we're listening on
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Add a paired device
    pub async fn add_paired_device(&self, device: PairedDevice) {
        self.paired_devices.write().await.insert(device.device_id, device);
    }

    /// Remove a paired device
    pub async fn remove_paired_device(&self, device_id: &Uuid) {
        self.paired_devices.write().await.remove(device_id);
    }

    /// Get all paired devices
    pub async fn get_paired_devices(&self) -> Vec<PairedDevice> {
        self.paired_devices.read().await.values().cloned().collect()
    }

    /// Start accepting connections with pairing support
    pub fn start_with_pairing(
        self,
        active_pairing: Arc<RwLock<Option<PairingSession>>>,
        identity: DeviceIdentity,
    ) -> (mpsc::Receiver<SyncEvent>, SyncServerHandle) {
        let (tx, rx) = mpsc::channel(64);
        let paired_devices = self.paired_devices.clone();

        let handle = tokio::spawn(async move {
            loop {
                match self.listener.accept().await {
                    Ok((stream, addr)) => {
                        tracing::debug!("incoming connection from {}", addr);
                        let tx = tx.clone();
                        let devices = paired_devices.clone();
                        let pairing = active_pairing.clone();
                        let ident = identity.clone();

                        tokio::spawn(async move {
                            tracing::info!("handling connection from {}", addr);
                            if let Err(e) = Self::handle_connection_with_pairing(
                                stream, addr, tx, devices, pairing, ident
                            ).await {
                                tracing::error!("connection error from {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("accept error: {}", e);
                    }
                }
            }
        });

        (rx, SyncServerHandle { task: handle })
    }

    /// Start accepting connections (legacy, without pairing)
    pub fn start(self) -> (mpsc::Receiver<SyncEvent>, SyncServerHandle) {
        let (tx, rx) = mpsc::channel(64);
        let paired_devices = self.paired_devices.clone();

        let handle = tokio::spawn(async move {
            loop {
                match self.listener.accept().await {
                    Ok((stream, addr)) => {
                        tracing::debug!("incoming connection from {}", addr);
                        let tx = tx.clone();
                        let devices = paired_devices.clone();

                        tokio::spawn(async move {
                            tracing::info!("handling connection from {}", addr);
                            if let Err(e) = Self::handle_connection(stream, addr, tx, devices).await {
                                tracing::error!("connection error from {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("accept error: {}", e);
                    }
                }
            }
        });

        (rx, SyncServerHandle { task: handle })
    }

    async fn handle_connection_with_pairing(
        mut stream: tokio::net::TcpStream,
        addr: SocketAddr,
        tx: mpsc::Sender<SyncEvent>,
        paired_devices: Arc<RwLock<HashMap<Uuid, PairedDevice>>>,
        active_pairing: Arc<RwLock<Option<PairingSession>>>,
        identity: DeviceIdentity,
    ) -> Result<()> {
        // Read message using the framing module
        let payload = read_framed_message(&mut stream).await?;
        let message = Message::from_bytes(&payload)?;

        match message {
            Message::PairRequest(req) => {
                tracing::info!("pairing request from {} at {}", req.device_name, addr);

                // Take the active pairing session
                let pairing_session = active_pairing.write().await.take()
                    .ok_or_else(|| Error::NotPaired("no active pairing session".to_string()))?;

                // Verify session ID matches
                if req.session_id != pairing_session.session_id {
                    // Put the session back
                    *active_pairing.write().await = Some(pairing_session);
                    return Err(Error::InvalidMessage("session ID mismatch".to_string()));
                }

                // Get our ephemeral public key before consuming the session
                let our_ephemeral_pubkey = pairing_session.ephemeral_public.clone();

                // Complete ECDH key exchange
                let session_key = pairing_session.complete(&req.ephemeral_pubkey);

                // Create signature over session data
                let mut sign_data = Vec::new();
                sign_data.extend(req.session_id.as_bytes());
                sign_data.extend(our_ephemeral_pubkey.to_bytes());
                sign_data.extend(req.ephemeral_pubkey.to_bytes());
                let signature = identity.signing_key.sign(&sign_data);

                // Create PairAccept message
                let accept = Message::PairAccept(PairAcceptMessage {
                    session_id: req.session_id,
                    device_id: identity.id,
                    device_name: identity.name.clone(),
                    ephemeral_pubkey: our_ephemeral_pubkey,
                    identity_pubkey: identity.signing_key.verifying_key(),
                    signature,
                });

                // Send PairAccept response using the framing module
                let response_bytes = accept.to_bytes()?;
                write_framed_message(&mut stream, &response_bytes).await?;

                tracing::info!("sent PairAccept to {} ({})", req.device_name, req.device_id);

                // Store the paired device
                let paired_device = PairedDevice {
                    device_id: req.device_id,
                    device_name: req.device_name.clone(),
                    session_key: session_key.clone(),
                };
                paired_devices.write().await.insert(req.device_id, paired_device.clone());

                // Notify the service
                let _ = tx.send(SyncEvent::DevicePaired { device: paired_device }).await;

                tracing::info!("paired successfully with {} ({})", req.device_name, req.device_id);

                // Keep connection open for potential follow-up messages
                // For now, we just return - a more complete implementation would
                // loop reading messages here
            }
            Message::ClipboardSync(sync_msg) => {
                // Try to decrypt if we have the session key
                if let Some(device) = paired_devices.read().await.get(&sync_msg.sender_id) {
                    let _ = tx.send(SyncEvent::MessageReceived {
                        peer_id: sync_msg.sender_id,
                        message: Message::ClipboardSync(sync_msg),
                    }).await;
                } else {
                    tracing::warn!("clipboard sync from unknown device {}", sync_msg.sender_id);
                }
            }
            other => {
                tracing::debug!("received {:?} from {}", other, addr);
            }
        }

        Ok(())
    }

    async fn handle_connection(
        mut stream: tokio::net::TcpStream,
        addr: SocketAddr,
        tx: mpsc::Sender<SyncEvent>,
        _paired_devices: Arc<RwLock<HashMap<Uuid, PairedDevice>>>,
    ) -> Result<()> {
        // Read message using the framing module
        let payload = read_framed_message(&mut stream).await?;
        let message = Message::from_bytes(&payload)?;

        // Handle based on message type
        match &message {
            Message::PairRequest(req) => {
                tracing::info!("pairing request from {} at {}", req.device_name, addr);
                let _ = tx.send(SyncEvent::MessageReceived {
                    peer_id: req.device_id,
                    message,
                }).await;
            }
            Message::Announce(ann) => {
                tracing::info!("announce from {} at {}", ann.device_name, addr);
            }
            _ => {
                tracing::debug!("received {:?} from {}", message, addr);
            }
        }

        Ok(())
    }
}

/// Handle to the running sync server
pub struct SyncServerHandle {
    task: tokio::task::JoinHandle<()>,
}

impl SyncServerHandle {
    /// Stop the server
    pub fn abort(self) {
        self.task.abort();
    }
}
