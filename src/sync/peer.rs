// Peer Management - Track known peers and their state
//
// Manages the registry of known peers, their connection state,
// and provides selection algorithms for gossip.

use crate::ledger::NodeId;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Peer-related errors
#[derive(Error, Debug)]
pub enum PeerError {
    #[error("Cannot add self as a peer")]
    CannotAddSelf,

    #[error("Peer not found")]
    PeerNotFound,

    #[error("Deserialization failed")]
    DeserializationFailed,
}

/// State of a peer connection
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerState {
    /// Just discovered, haven't connected yet
    Unknown,
    /// Attempting to connect
    Connecting,
    /// Successfully connected
    Connected,
    /// Currently syncing state
    Syncing,
    /// Connection lost or failed
    Disconnected,
    /// Peer misbehaved (bad messages, etc.)
    Banned,
}

/// Statistics about a peer registry
#[derive(Clone, Debug)]
pub struct PeerStats {
    pub total_peers: usize,
    pub connected_peers: usize,
    pub syncing_peers: usize,
    pub disconnected_peers: usize,
    pub banned_peers: usize,
}

/// Information about a known peer
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Unique node identifier
    node_id: NodeId,
    /// Network address
    address: SocketAddr,
    /// Current state
    state: PeerState,
    /// Last known state version
    known_version: u64,
    /// Last time we heard from this peer (unix timestamp ms)
    last_seen: u64,
    /// Round-trip time samples (for prioritization)
    rtt_samples: Vec<u32>,
    /// Number of failed connection attempts
    failed_attempts: u32,
}

impl PeerInfo {
    /// Create a new peer info
    pub fn new(node_id: NodeId, address: SocketAddr) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            node_id,
            address,
            state: PeerState::Unknown,
            known_version: 0,
            last_seen: now,
            rtt_samples: Vec::new(),
            failed_attempts: 0,
        }
    }

    /// Get the node ID
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// Get the address
    pub fn address(&self) -> &SocketAddr {
        &self.address
    }

    /// Get the current state
    pub fn state(&self) -> PeerState {
        self.state
    }

    /// Set the state
    pub fn set_state(&mut self, state: PeerState) {
        self.state = state;
        if state == PeerState::Connected {
            self.failed_attempts = 0;
        }
    }

    /// Get known version
    pub fn known_version(&self) -> u64 {
        self.known_version
    }

    /// Update known version
    pub fn update_version(&mut self, version: u64) {
        self.known_version = version;
    }

    /// Get last seen timestamp
    pub fn last_seen(&self) -> u64 {
        self.last_seen
    }

    /// Update last seen timestamp
    pub fn touch(&mut self) {
        self.last_seen = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
    }

    /// Check if peer is stale (not seen in timeout_secs)
    pub fn is_stale(&self, timeout_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let timeout_ms = timeout_secs * 1000;
        now.saturating_sub(self.last_seen) > timeout_ms
    }

    /// Record a round-trip time measurement
    pub fn record_rtt(&mut self, rtt_ms: u32) {
        self.rtt_samples.push(rtt_ms);
        // Keep only last 10 samples
        if self.rtt_samples.len() > 10 {
            self.rtt_samples.remove(0);
        }
    }

    /// Get average RTT
    pub fn average_rtt(&self) -> Option<u32> {
        if self.rtt_samples.is_empty() {
            return None;
        }
        let sum: u32 = self.rtt_samples.iter().sum();
        Some(sum / self.rtt_samples.len() as u32)
    }

    /// Record a failed connection attempt
    pub fn record_failure(&mut self) {
        self.failed_attempts = self.failed_attempts.saturating_add(1);
        self.set_state(PeerState::Disconnected);
    }

    /// Get number of failed attempts
    pub fn failed_attempts(&self) -> u32 {
        self.failed_attempts
    }
}

/// Registry of known peers
#[derive(Clone, Debug)]
pub struct PeerRegistry {
    /// Our own node ID
    my_node_id: NodeId,
    /// Map of node ID to peer info
    peers: HashMap<NodeId, PeerInfo>,
}

impl PeerRegistry {
    /// Create a new peer registry
    pub fn new(my_node_id: NodeId) -> Self {
        Self {
            my_node_id,
            peers: HashMap::new(),
        }
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// Get number of peers
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Check if we have a peer
    pub fn has_peer(&self, node_id: &NodeId) -> bool {
        self.peers.contains_key(node_id)
    }

    /// Add or update a peer
    pub fn add_peer(&mut self, node_id: NodeId, address: SocketAddr) -> Result<(), PeerError> {
        // Can't add ourselves
        if node_id == self.my_node_id {
            return Err(PeerError::CannotAddSelf);
        }

        // Update or insert
        self.peers
            .entry(node_id.clone())
            .and_modify(|p| {
                // Update address if peer exists
                p.address = address;
                p.touch();
            })
            .or_insert_with(|| PeerInfo::new(node_id, address));

        Ok(())
    }

    /// Remove a peer
    pub fn remove_peer(&mut self, node_id: &NodeId) {
        self.peers.remove(node_id);
    }

    /// Get a peer by node ID
    pub fn get_peer(&self, node_id: &NodeId) -> Option<&PeerInfo> {
        self.peers.get(node_id)
    }

    /// Get a mutable reference to a peer
    pub fn get_peer_mut(&mut self, node_id: &NodeId) -> Option<&mut PeerInfo> {
        self.peers.get_mut(node_id)
    }

    /// Get all peers
    pub fn all_peers(&self) -> Vec<&PeerInfo> {
        self.peers.values().collect()
    }

    /// Select random peers for gossip
    pub fn select_random_peers(&self, count: usize) -> Vec<&PeerInfo> {
        let mut rng = rand::thread_rng();
        let mut peers: Vec<&PeerInfo> = self.peers.values().collect();
        peers.shuffle(&mut rng);
        peers.truncate(count);
        peers
    }

    /// Get peers by state
    pub fn peers_by_state(&self, state: PeerState) -> Vec<&PeerInfo> {
        self.peers
            .values()
            .filter(|p| p.state == state)
            .collect()
    }

    /// Get peers behind a certain version
    pub fn peers_behind_version(&self, version: u64) -> Vec<&PeerInfo> {
        self.peers
            .values()
            .filter(|p| p.known_version < version)
            .collect()
    }

    /// Remove stale peers
    pub fn remove_stale_peers(&mut self, timeout_secs: u64) -> usize {
        let stale: Vec<NodeId> = self
            .peers
            .values()
            .filter(|p| p.is_stale(timeout_secs))
            .map(|p| p.node_id.clone())
            .collect();

        let count = stale.len();
        for node_id in stale {
            self.peers.remove(&node_id);
        }
        count
    }

    /// Get statistics
    pub fn stats(&self) -> PeerStats {
        let mut stats = PeerStats {
            total_peers: self.peers.len(),
            connected_peers: 0,
            syncing_peers: 0,
            disconnected_peers: 0,
            banned_peers: 0,
        };

        for peer in self.peers.values() {
            match peer.state {
                PeerState::Connected => stats.connected_peers += 1,
                PeerState::Syncing => stats.syncing_peers += 1,
                PeerState::Disconnected => stats.disconnected_peers += 1,
                PeerState::Banned => stats.banned_peers += 1,
                _ => {}
            }
        }

        stats
    }

    /// Serialize for persistence
    pub fn to_bytes(&self) -> Vec<u8> {
        // Serialize just the peer list
        let peers: Vec<&PeerInfo> = self.peers.values().collect();
        postcard::to_allocvec(&peers).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8], my_node_id: NodeId) -> Result<Self, PeerError> {
        let peers: Vec<PeerInfo> =
            postcard::from_bytes(bytes).map_err(|_| PeerError::DeserializationFailed)?;

        let mut registry = Self::new(my_node_id);
        for peer in peers {
            registry.peers.insert(peer.node_id.clone(), peer);
        }
        Ok(registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_info_basic() {
        let node_id = NodeId::generate();
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let peer = PeerInfo::new(node_id.clone(), addr);

        assert_eq!(peer.node_id(), &node_id);
        assert_eq!(peer.state(), PeerState::Unknown);
    }

    #[test]
    fn test_peer_registry_add_remove() {
        let my_id = NodeId::generate();
        let mut registry = PeerRegistry::new(my_id);

        let peer_id = NodeId::generate();
        let addr: SocketAddr = "192.168.1.1:8080".parse().unwrap();

        registry.add_peer(peer_id.clone(), addr).unwrap();
        assert_eq!(registry.peer_count(), 1);

        registry.remove_peer(&peer_id);
        assert_eq!(registry.peer_count(), 0);
    }

    #[test]
    fn test_peer_registry_cannot_add_self() {
        let my_id = NodeId::generate();
        let mut registry = PeerRegistry::new(my_id.clone());

        let result = registry.add_peer(my_id, "127.0.0.1:8080".parse().unwrap());
        assert!(matches!(result, Err(PeerError::CannotAddSelf)));
    }
}
