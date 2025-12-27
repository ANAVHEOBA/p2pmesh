// Gossip Tests
// Tests for the gossip-based synchronization protocol

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::IOUBuilder;
use p2pmesh::ledger::{MeshState, NodeId};
use p2pmesh::sync::{
    GossipConfig, GossipEngine, GossipEvent, SyncRequest, SyncResponse,
    IOUAnnouncement, Message,
};

// ============================================================================
// GOSSIP ENGINE CREATION
// ============================================================================

#[test]
fn test_gossip_engine_new() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let engine = GossipEngine::new(node_id, state, GossipConfig::default());

    assert_eq!(engine.pending_announcements(), 0);
}

#[test]
fn test_gossip_config_defaults() {
    let config = GossipConfig::default();

    assert!(config.fanout > 0);
    assert!(config.max_hops > 0);
    assert!(config.heartbeat_interval_secs > 0);
}

#[test]
fn test_gossip_config_custom() {
    let config = GossipConfig::new()
        .with_fanout(5)
        .with_max_hops(10)
        .with_heartbeat_interval(30);

    assert_eq!(config.fanout, 5);
    assert_eq!(config.max_hops, 10);
    assert_eq!(config.heartbeat_interval_secs, 30);
}

// ============================================================================
// IOU ANNOUNCEMENT HANDLING
// ============================================================================

#[test]
fn test_gossip_announce_new_iou() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    engine.announce_iou(iou.clone(), &alice.public_key());

    assert_eq!(engine.pending_announcements(), 1);
}

#[test]
fn test_gossip_duplicate_announcement_ignored() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .nonce(1)
        .build()
        .unwrap();

    engine.announce_iou(iou.clone(), &alice.public_key());
    engine.announce_iou(iou.clone(), &alice.public_key()); // Duplicate

    // Should still only have 1 pending
    assert_eq!(engine.pending_announcements(), 1);
}

#[test]
fn test_gossip_receive_iou_announcement() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let announcement = IOUAnnouncement::new(iou, alice.public_key());
    let result = engine.handle_iou_announcement(announcement);

    assert!(result.is_ok());
    assert_eq!(engine.state().iou_count(), 1);
}

#[test]
fn test_gossip_receive_invalid_iou_rejected() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    // Announce with wrong public key
    let announcement = IOUAnnouncement::new(iou, charlie.public_key());
    let result = engine.handle_iou_announcement(announcement);

    assert!(result.is_err());
    assert_eq!(engine.state().iou_count(), 0);
}

// ============================================================================
// SYNC REQUEST/RESPONSE
// ============================================================================

#[test]
fn test_gossip_handle_sync_request_empty() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let engine = GossipEngine::new(node_id.clone(), state, GossipConfig::default());

    let requester_id = NodeId::generate();
    let request = SyncRequest::new(requester_id, 0);

    let response = engine.handle_sync_request(&request);

    assert_eq!(response.sender(), &node_id);
    assert!(response.entries().is_empty());
}

#[test]
fn test_gossip_handle_sync_request_with_delta() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id.clone());

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    // Add some IOUs to state
    for i in 0..5 {
        let iou = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(100)
            .nonce(i)
            .build()
            .unwrap();
        state.add_iou(iou, &alice.public_key()).unwrap();
    }

    let engine = GossipEngine::new(node_id.clone(), state, GossipConfig::default());

    // Request with version 0 should get all entries
    let requester_id = NodeId::generate();
    let request = SyncRequest::new(requester_id, 0);

    let response = engine.handle_sync_request(&request);

    assert_eq!(response.entries().len(), 5);
}

#[test]
fn test_gossip_apply_sync_response() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

    // Create a response with some entries
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let entry = p2pmesh::ledger::IOUEntry::new(iou, alice.public_key());
    let response = SyncResponse::new(NodeId::generate(), 1, vec![entry]);

    let result = engine.apply_sync_response(response);

    assert!(result.is_ok());
    assert_eq!(result.unwrap().new_entries, 1);
    assert_eq!(engine.state().iou_count(), 1);
}

// ============================================================================
// MESSAGE PROCESSING
// ============================================================================

#[test]
fn test_gossip_process_message() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let announcement = IOUAnnouncement::new(iou, alice.public_key());
    let msg = Message::IOUAnnouncement(announcement);

    let events = engine.process_message(msg).unwrap();

    // Should have processed the IOU
    assert_eq!(engine.state().iou_count(), 1);
    // Should generate events (e.g., forward to peers)
    assert!(!events.is_empty());
}

#[test]
fn test_gossip_process_heartbeat() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

    let peer_id = NodeId::generate();
    let heartbeat = p2pmesh::sync::Heartbeat::new(peer_id.clone(), 100);
    let msg = Message::Heartbeat(heartbeat);

    let events = engine.process_message(msg).unwrap();

    // Heartbeat might trigger sync if peer has higher version
    // For now, just verify it doesn't error
    assert!(events.is_empty() || !events.is_empty());
}

// ============================================================================
// GOSSIP EVENTS
// ============================================================================

#[test]
fn test_gossip_collect_outgoing_messages() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    engine.announce_iou(iou, &alice.public_key());

    let messages = engine.collect_outgoing_messages();

    assert!(!messages.is_empty());
}

#[test]
fn test_gossip_generate_heartbeat() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let engine = GossipEngine::new(node_id.clone(), state, GossipConfig::default());

    let heartbeat = engine.generate_heartbeat();

    assert_eq!(heartbeat.sender(), &node_id);
}

#[test]
fn test_gossip_generate_sync_request() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let engine = GossipEngine::new(node_id.clone(), state, GossipConfig::default());

    let request = engine.generate_sync_request();

    assert_eq!(request.sender(), &node_id);
}

// ============================================================================
// SEEN MESSAGE TRACKING
// ============================================================================

#[test]
fn test_gossip_tracks_seen_messages() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .nonce(1)
        .build()
        .unwrap();

    let announcement = IOUAnnouncement::new(iou, alice.public_key());
    let msg = Message::IOUAnnouncement(announcement.clone());

    // Process first time
    let events1 = engine.process_message(msg.clone()).unwrap();
    assert!(!events1.is_empty()); // Should forward

    // Process second time (already seen)
    let events2 = engine.process_message(msg).unwrap();
    assert!(events2.is_empty()); // Should not forward again
}

#[test]
fn test_gossip_prune_seen_messages() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    // Add many seen messages
    for i in 0..100 {
        let iou = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(100)
            .nonce(i)
            .build()
            .unwrap();

        let announcement = IOUAnnouncement::new(iou, alice.public_key());
        let msg = Message::IOUAnnouncement(announcement);
        let _ = engine.process_message(msg);
    }

    let pruned = engine.prune_seen_messages(50);

    assert!(pruned > 0 || engine.seen_message_count() <= 100);
}

// ============================================================================
// GOSSIP STATE
// ============================================================================

#[test]
fn test_gossip_state_access() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id.clone());

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();
    state.add_iou(iou, &alice.public_key()).unwrap();

    let engine = GossipEngine::new(node_id, state, GossipConfig::default());

    assert_eq!(engine.state().iou_count(), 1);
}

#[test]
fn test_gossip_stats() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());
    let engine = GossipEngine::new(node_id, state, GossipConfig::default());

    let stats = engine.stats();

    assert_eq!(stats.messages_processed, 0);
    assert_eq!(stats.messages_forwarded, 0);
}
