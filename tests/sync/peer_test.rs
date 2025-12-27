// Peer Tests
// Tests for peer management and registry

use p2pmesh::ledger::NodeId;
use p2pmesh::sync::{PeerInfo, PeerRegistry, PeerState, PeerError};
use std::net::SocketAddr;

// ============================================================================
// PEER INFO
// ============================================================================

#[test]
fn test_peer_info_creation() {
    let node_id = NodeId::generate();
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let peer = PeerInfo::new(node_id.clone(), addr);

    assert_eq!(peer.node_id(), &node_id);
    assert_eq!(peer.address(), &addr);
    assert_eq!(peer.state(), PeerState::Unknown);
}

#[test]
fn test_peer_info_update_state() {
    let node_id = NodeId::generate();
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let mut peer = PeerInfo::new(node_id, addr);

    peer.set_state(PeerState::Connected);
    assert_eq!(peer.state(), PeerState::Connected);

    peer.set_state(PeerState::Syncing);
    assert_eq!(peer.state(), PeerState::Syncing);
}

#[test]
fn test_peer_info_version_tracking() {
    let node_id = NodeId::generate();
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let mut peer = PeerInfo::new(node_id, addr);

    assert_eq!(peer.known_version(), 0);
    peer.update_version(100);
    assert_eq!(peer.known_version(), 100);
}

#[test]
fn test_peer_info_last_seen() {
    let node_id = NodeId::generate();
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let mut peer = PeerInfo::new(node_id, addr);

    let initial_seen = peer.last_seen();
    std::thread::sleep(std::time::Duration::from_millis(10));
    peer.touch();

    assert!(peer.last_seen() >= initial_seen);
}

#[test]
fn test_peer_info_is_stale() {
    let node_id = NodeId::generate();
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let peer = PeerInfo::new(node_id, addr);

    // Just created, should not be stale
    assert!(!peer.is_stale(60)); // 60 second timeout
}

#[test]
fn test_peer_info_rtt_tracking() {
    let node_id = NodeId::generate();
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let mut peer = PeerInfo::new(node_id, addr);

    peer.record_rtt(50);
    peer.record_rtt(100);
    peer.record_rtt(75);

    // Should have an average RTT
    assert!(peer.average_rtt().is_some());
    let avg = peer.average_rtt().unwrap();
    assert!(avg >= 50 && avg <= 100);
}

// ============================================================================
// PEER REGISTRY
// ============================================================================

#[test]
fn test_peer_registry_new() {
    let my_node_id = NodeId::generate();
    let registry = PeerRegistry::new(my_node_id);

    assert!(registry.is_empty());
    assert_eq!(registry.peer_count(), 0);
}

#[test]
fn test_peer_registry_add_peer() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    let peer_node_id = NodeId::generate();
    let addr: SocketAddr = "192.168.1.100:8080".parse().unwrap();

    registry.add_peer(peer_node_id.clone(), addr).unwrap();

    assert_eq!(registry.peer_count(), 1);
    assert!(registry.has_peer(&peer_node_id));
}

#[test]
fn test_peer_registry_add_self_rejected() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id.clone());

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let result = registry.add_peer(my_node_id, addr);

    assert!(matches!(result, Err(PeerError::CannotAddSelf)));
}

#[test]
fn test_peer_registry_add_duplicate() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    let peer_id = NodeId::generate();
    let addr: SocketAddr = "192.168.1.100:8080".parse().unwrap();

    registry.add_peer(peer_id.clone(), addr).unwrap();
    let result = registry.add_peer(peer_id, addr);

    // Adding duplicate should just update, not error
    assert!(result.is_ok());
    assert_eq!(registry.peer_count(), 1);
}

#[test]
fn test_peer_registry_remove_peer() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    let peer_id = NodeId::generate();
    let addr: SocketAddr = "192.168.1.100:8080".parse().unwrap();

    registry.add_peer(peer_id.clone(), addr).unwrap();
    assert_eq!(registry.peer_count(), 1);

    registry.remove_peer(&peer_id);
    assert_eq!(registry.peer_count(), 0);
}

#[test]
fn test_peer_registry_get_peer() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    let peer_id = NodeId::generate();
    let addr: SocketAddr = "192.168.1.100:8080".parse().unwrap();

    registry.add_peer(peer_id.clone(), addr).unwrap();

    let peer = registry.get_peer(&peer_id);
    assert!(peer.is_some());
    assert_eq!(peer.unwrap().address(), &addr);
}

#[test]
fn test_peer_registry_get_peer_mut() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    let peer_id = NodeId::generate();
    let addr: SocketAddr = "192.168.1.100:8080".parse().unwrap();

    registry.add_peer(peer_id.clone(), addr).unwrap();

    {
        let peer = registry.get_peer_mut(&peer_id).unwrap();
        peer.set_state(PeerState::Connected);
    }

    let peer = registry.get_peer(&peer_id).unwrap();
    assert_eq!(peer.state(), PeerState::Connected);
}

// ============================================================================
// PEER SELECTION
// ============================================================================

#[test]
fn test_peer_registry_select_random() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    // Add several peers
    for i in 0..5 {
        let peer_id = NodeId::generate();
        let addr: SocketAddr = format!("192.168.1.{}:8080", i + 1).parse().unwrap();
        registry.add_peer(peer_id, addr).unwrap();
    }

    // Select 3 random peers
    let selected = registry.select_random_peers(3);
    assert_eq!(selected.len(), 3);
}

#[test]
fn test_peer_registry_select_random_fewer_available() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    // Add only 2 peers
    for i in 0..2 {
        let peer_id = NodeId::generate();
        let addr: SocketAddr = format!("192.168.1.{}:8080", i + 1).parse().unwrap();
        registry.add_peer(peer_id, addr).unwrap();
    }

    // Request 5, but only 2 available
    let selected = registry.select_random_peers(5);
    assert_eq!(selected.len(), 2);
}

#[test]
fn test_peer_registry_select_by_state() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    // Add peers with different states
    let connected_id = NodeId::generate();
    let disconnected_id = NodeId::generate();

    registry.add_peer(connected_id.clone(), "192.168.1.1:8080".parse().unwrap()).unwrap();
    registry.add_peer(disconnected_id.clone(), "192.168.1.2:8080".parse().unwrap()).unwrap();

    registry.get_peer_mut(&connected_id).unwrap().set_state(PeerState::Connected);
    registry.get_peer_mut(&disconnected_id).unwrap().set_state(PeerState::Disconnected);

    let connected = registry.peers_by_state(PeerState::Connected);
    assert_eq!(connected.len(), 1);
}

#[test]
fn test_peer_registry_get_outdated_peers() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    // Add peers with different versions
    let peer1_id = NodeId::generate();
    let peer2_id = NodeId::generate();

    registry.add_peer(peer1_id.clone(), "192.168.1.1:8080".parse().unwrap()).unwrap();
    registry.add_peer(peer2_id.clone(), "192.168.1.2:8080".parse().unwrap()).unwrap();

    registry.get_peer_mut(&peer1_id).unwrap().update_version(50);
    registry.get_peer_mut(&peer2_id).unwrap().update_version(100);

    // Get peers behind version 75
    let outdated = registry.peers_behind_version(75);
    assert_eq!(outdated.len(), 1);
    assert_eq!(outdated[0].node_id(), &peer1_id);
}

// ============================================================================
// PEER CLEANUP
// ============================================================================

#[test]
fn test_peer_registry_remove_stale() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    let peer_id = NodeId::generate();
    registry.add_peer(peer_id, "192.168.1.1:8080".parse().unwrap()).unwrap();

    // With a very long timeout, no peers should be removed
    let removed = registry.remove_stale_peers(3600);
    assert_eq!(removed, 0);
}

#[test]
fn test_peer_registry_all_peers() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    for i in 0..5 {
        let peer_id = NodeId::generate();
        let addr: SocketAddr = format!("192.168.1.{}:8080", i + 1).parse().unwrap();
        registry.add_peer(peer_id, addr).unwrap();
    }

    let all = registry.all_peers();
    assert_eq!(all.len(), 5);
}

// ============================================================================
// PEER SERIALIZATION (for persistence)
// ============================================================================

#[test]
fn test_peer_registry_serialization_roundtrip() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id.clone());

    let peer_id = NodeId::generate();
    registry.add_peer(peer_id.clone(), "192.168.1.1:8080".parse().unwrap()).unwrap();

    let bytes = registry.to_bytes();
    let restored = PeerRegistry::from_bytes(&bytes, my_node_id).unwrap();

    assert_eq!(restored.peer_count(), 1);
    assert!(restored.has_peer(&peer_id));
}

// ============================================================================
// PEER STATISTICS
// ============================================================================

#[test]
fn test_peer_registry_stats() {
    let my_node_id = NodeId::generate();
    let mut registry = PeerRegistry::new(my_node_id);

    let peer1 = NodeId::generate();
    let peer2 = NodeId::generate();

    registry.add_peer(peer1.clone(), "192.168.1.1:8080".parse().unwrap()).unwrap();
    registry.add_peer(peer2.clone(), "192.168.1.2:8080".parse().unwrap()).unwrap();

    registry.get_peer_mut(&peer1).unwrap().set_state(PeerState::Connected);

    let stats = registry.stats();

    assert_eq!(stats.total_peers, 2);
    assert_eq!(stats.connected_peers, 1);
}
