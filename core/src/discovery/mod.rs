//! mDNS service discovery for finding peers on the local network

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::protocol::constants::{SERVICE_TYPE, PROTOCOL_VERSION};
use crate::{Error, Result};

/// Information about a discovered peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub device_id: Uuid,
    pub device_name: String,
    pub fingerprint: String,
    pub addresses: Vec<IpAddr>,
    pub port: u16,
}

/// Event from the discovery service
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    PeerFound(PeerInfo),
    PeerLost(Uuid),
}

/// mDNS discovery service
pub struct DiscoveryService {
    daemon: ServiceDaemon,
    our_device_id: Uuid,
    peers: Arc<RwLock<HashMap<Uuid, PeerInfo>>>,
}

impl DiscoveryService {
    /// Create a new discovery service
    pub fn new(device_id: Uuid) -> Result<Self> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| Error::Discovery(e.to_string()))?;

        Ok(Self {
            daemon,
            our_device_id: device_id,
            peers: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Register our service for others to discover
    pub fn register(
        &self,
        device_name: &str,
        fingerprint: &str,
        port: u16,
    ) -> Result<()> {
        let instance_name = format!("{}-{}", device_name, &self.our_device_id.to_string()[..8]);

        let mut properties = HashMap::new();
        properties.insert("id".to_string(), self.our_device_id.to_string());
        properties.insert("fp".to_string(), fingerprint.to_string());
        properties.insert("v".to_string(), PROTOCOL_VERSION.to_string());

        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &format!("{}.local.", hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "omniclip".to_string())),
            (),
            port,
            properties,
        ).map_err(|e| Error::Discovery(e.to_string()))?;

        self.daemon
            .register(service)
            .map_err(|e| Error::Discovery(e.to_string()))?;

        tracing::info!("registered mDNS service: {}", instance_name);
        Ok(())
    }

    /// Start browsing for peers, returns a channel of discovery events
    pub fn browse(&self) -> Result<mpsc::Receiver<DiscoveryEvent>> {
        let (tx, rx) = mpsc::channel(32);
        let peers = self.peers.clone();
        let our_id = self.our_device_id;

        let receiver = self.daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| Error::Discovery(e.to_string()))?;

        tokio::spawn(async move {
            while let Ok(event) = receiver.recv() {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        // Parse device info from TXT records
                        let props = info.get_properties();

                        let device_id = props.get("id")
                            .and_then(|v| v.val_str().parse::<Uuid>().ok());

                        let fingerprint = props.get("fp")
                            .map(|v| v.val_str().to_string())
                            .unwrap_or_default();

                        if let Some(id) = device_id {
                            // Don't discover ourselves
                            if id == our_id {
                                continue;
                            }

                            let peer = PeerInfo {
                                device_id: id,
                                device_name: info.get_fullname()
                                    .split('.')
                                    .next()
                                    .unwrap_or("Unknown")
                                    .to_string(),
                                fingerprint,
                                addresses: info.get_addresses().iter().copied().collect(),
                                port: info.get_port(),
                            };

                            peers.write().await.insert(id, peer.clone());

                            if tx.send(DiscoveryEvent::PeerFound(peer)).await.is_err() {
                                break;
                            }
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        // Try to find and remove the peer
                        let mut peers_guard = peers.write().await;
                        let removed_id = peers_guard.iter()
                            .find(|(_, p)| fullname.contains(&p.device_name))
                            .map(|(id, _)| *id);

                        if let Some(id) = removed_id {
                            peers_guard.remove(&id);
                            if tx.send(DiscoveryEvent::PeerLost(id)).await.is_err() {
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        Ok(rx)
    }

    /// Get currently known peers
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.values().cloned().collect()
    }

    /// Get a specific peer by ID
    pub async fn get_peer(&self, id: &Uuid) -> Option<PeerInfo> {
        self.peers.read().await.get(id).cloned()
    }

    /// Shutdown the discovery service
    pub fn shutdown(self) -> Result<()> {
        self.daemon
            .shutdown()
            .map_err(|e| Error::Discovery(e.to_string()))?;
        Ok(())
    }
}

/// Get local IP addresses (non-loopback)
pub fn get_local_ips() -> Vec<IpAddr> {
    let mut ips = Vec::new();

    if let Ok(interfaces) = get_if_addrs::get_if_addrs() {
        for iface in interfaces {
            if !iface.is_loopback() {
                ips.push(iface.ip());
            }
        }
    }

    ips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_local_ips() {
        let ips = get_local_ips();
        // Should have at least one IP in most environments
        println!("Local IPs: {:?}", ips);
    }
}
