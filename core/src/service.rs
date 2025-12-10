//! High-level Omniclip service that coordinates all components

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::clipboard::{self};
use crate::crypto::SessionKey;
use crate::discovery::{DiscoveryEvent, DiscoveryService, PeerInfo};
use crate::protocol::{ClipboardContent, ClipboardSyncMessage, ContentHash, Message, PairingSession};
use crate::sync::server::{SyncEvent, SyncServer, SyncServerHandle};
use crate::{Config, DeviceIdentity, Error, Result};

/// Events emitted by the Omniclip service
#[derive(Debug, Clone)]
pub enum ServiceEvent {
    /// A new device was discovered on the network
    DeviceDiscovered(PeerInfo),
    /// A device went offline
    DeviceLost(Uuid),
    /// Pairing request received from another device
    PairingRequest { device_id: Uuid, device_name: String },
    /// Clipboard was synced from another device
    ClipboardReceived { from_device: Uuid, content: ClipboardContent },
    /// Our clipboard was sent to other devices
    ClipboardSent { to_devices: Vec<Uuid> },
    /// Error occurred
    Error(String),
}

/// Paired device storage
#[derive(Clone)]
struct PairedDeviceInfo {
    device_id: Uuid,
    device_name: String,
    session_key: SessionKey,
    last_seen: std::time::Instant,
}

/// Main Omniclip service
pub struct OmniclipService {
    config: Config,
    identity: DeviceIdentity,
    discovery: Option<DiscoveryService>,
    server: Option<SyncServerHandle>,
    paired_devices: Arc<RwLock<HashMap<Uuid, PairedDeviceInfo>>>,
    active_pairing: Arc<RwLock<Option<PairingSession>>>,
    last_sent_hash: Arc<RwLock<Option<ContentHash>>>,
}

impl OmniclipService {
    /// Create a new Omniclip service
    pub fn new(device_name: String) -> Self {
        let identity = DeviceIdentity::new(device_name);
        Self {
            config: Config::default(),
            identity,
            discovery: None,
            server: None,
            paired_devices: Arc::new(RwLock::new(HashMap::new())),
            active_pairing: Arc::new(RwLock::new(None)),
            last_sent_hash: Arc::new(RwLock::new(None)),
        }
    }

    /// Create with custom config
    pub fn with_config(device_name: String, config: Config) -> Self {
        let identity = DeviceIdentity::new(device_name);
        Self {
            config,
            identity,
            discovery: None,
            server: None,
            paired_devices: Arc::new(RwLock::new(HashMap::new())),
            active_pairing: Arc::new(RwLock::new(None)),
            last_sent_hash: Arc::new(RwLock::new(None)),
        }
    }

    /// Get our device ID
    pub fn device_id(&self) -> Uuid {
        self.identity.id
    }

    /// Get our device name
    pub fn device_name(&self) -> &str {
        &self.identity.name
    }

    /// Get our public key fingerprint
    pub fn fingerprint(&self) -> String {
        self.identity.fingerprint()
    }

    /// Start the service and return event channel
    pub async fn start(&mut self) -> Result<mpsc::Receiver<ServiceEvent>> {
        let (tx, rx) = mpsc::channel(64);

        // Start sync server
        let server = SyncServer::bind(self.config.port).await?;
        let port = server.port();

        // Start discovery
        let discovery = DiscoveryService::new(self.identity.id)?;
        discovery.register(&self.identity.name, &self.fingerprint(), port)?;

        // Browse for peers
        let mut discovery_rx = discovery.browse()?;

        // Start server with pairing support
        let (mut server_rx, server_handle) = server.start_with_pairing(
            self.active_pairing.clone(),
            self.identity.clone(),
        );

        self.server = Some(server_handle);
        self.discovery = Some(discovery);

        // Spawn task to forward discovery events
        let tx_discovery = tx.clone();
        tokio::spawn(async move {
            while let Some(event) = discovery_rx.recv().await {
                let service_event = match event {
                    DiscoveryEvent::PeerFound(peer) => ServiceEvent::DeviceDiscovered(peer),
                    DiscoveryEvent::PeerLost(id) => ServiceEvent::DeviceLost(id),
                };
                if tx_discovery.send(service_event).await.is_err() {
                    break;
                }
            }
        });

        // Spawn task to forward server events
        let tx_server = tx.clone();
        let paired_devices = self.paired_devices.clone();
        tokio::spawn(async move {
            while let Some(event) = server_rx.recv().await {
                match event {
                    SyncEvent::DevicePaired { device } => {
                        tracing::info!("device paired: {} ({})", device.device_name, device.device_id);
                        // Store in our local paired devices
                        paired_devices.write().await.insert(device.device_id, PairedDeviceInfo {
                            device_id: device.device_id,
                            device_name: device.device_name.clone(),
                            session_key: device.session_key,
                            last_seen: std::time::Instant::now(),
                        });
                        let _ = tx_server.send(ServiceEvent::PairingRequest {
                            device_id: device.device_id,
                            device_name: device.device_name,
                        }).await;
                    }
                    SyncEvent::MessageReceived { peer_id, message } => {
                        match message {
                            Message::PairRequest(req) => {
                                let _ = tx_server.send(ServiceEvent::PairingRequest {
                                    device_id: req.device_id,
                                    device_name: req.device_name,
                                }).await;
                            }
                            Message::ClipboardSync(sync_msg) => {
                                // Try to decrypt if we have the session key
                                if let Some(device) = paired_devices.read().await.get(&peer_id) {
                                    if let Ok(decrypted) = device.session_key.decrypt(&sync_msg.encrypted_content) {
                                        if let Ok(content) = ClipboardContent::from_bytes(&decrypted) {
                                            let _ = tx_server.send(ServiceEvent::ClipboardReceived {
                                                from_device: peer_id,
                                                content,
                                            }).await;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        });

        // Spawn clipboard monitoring task
        let tx_clipboard = tx.clone();
        let paired = self.paired_devices.clone();
        let last_sent = self.last_sent_hash.clone();
        let our_id = self.identity.id;

        tokio::spawn(async move {
            let (mut clip_rx, _handle) = clipboard::start_monitor(Duration::from_millis(500));

            while let Some(change) = clip_rx.recv().await {
                // Skip if this is content we just received
                if let Some(last) = last_sent.read().await.as_ref() {
                    if *last == change.hash {
                        continue;
                    }
                }

                // Send to all paired devices
                let devices = paired.read().await;
                let mut sent_to = Vec::new();

                for (id, device) in devices.iter() {
                    if let Ok(plaintext) = change.content.to_bytes() {
                        if let Ok(encrypted) = device.session_key.encrypt(&plaintext) {
                            let _msg = Message::ClipboardSync(ClipboardSyncMessage {
                                message_id: Uuid::new_v4(),
                                sender_id: our_id,
                                content_hash: change.hash,
                                encrypted_content: encrypted,
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                            });

                            // TODO: Actually send to peer connection
                            sent_to.push(*id);
                        }
                    }
                }

                if !sent_to.is_empty() {
                    *last_sent.write().await = Some(change.hash);
                    let _ = tx_clipboard.send(ServiceEvent::ClipboardSent { to_devices: sent_to }).await;
                }
            }
        });

        tracing::info!("omniclip service started on port {}", port);
        Ok(rx)
    }

    /// Start a new pairing session and return QR code data
    pub async fn start_pairing(&self) -> Result<String> {
        let session = PairingSession::new();
        let local_ips = crate::discovery::get_local_ips();
        let ip = local_ips.first()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "127.0.0.1".to_string());

        let qr_data = session.qr_data(&ip, self.config.port, &self.identity.name);
        let url = qr_data.to_url();

        *self.active_pairing.write().await = Some(session);
        Ok(url)
    }

    /// Get QR code as SVG for current pairing session
    pub async fn get_pairing_qr_svg(&self) -> Result<String> {
        let pairing = self.active_pairing.read().await;
        let session = pairing.as_ref()
            .ok_or_else(|| Error::InvalidMessage("no active pairing session".to_string()))?;

        let local_ips = crate::discovery::get_local_ips();
        let ip = local_ips.first()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "127.0.0.1".to_string());

        let qr_data = session.qr_data(&ip, self.config.port, &self.identity.name);
        qr_data.to_qr_svg()
    }

    /// Get list of paired devices
    pub async fn get_paired_devices(&self) -> Vec<(Uuid, String)> {
        self.paired_devices.read().await
            .iter()
            .map(|(id, d)| (*id, d.device_name.clone()))
            .collect()
    }

    /// Remove a paired device
    pub async fn unpair_device(&self, device_id: Uuid) {
        self.paired_devices.write().await.remove(&device_id);
    }
}
