// Protocol Tests
// Tests for sync message types and serialization

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::IOUBuilder;
use p2pmesh::ledger::NodeId;
use p2pmesh::sync::{
    Message, MessageType, SyncRequest, SyncResponse, IOUAnnouncement,
    PeerAnnouncement, Heartbeat, ProtocolError,
};

// ============================================================================
// MESSAGE TYPE IDENTIFICATION
// ============================================================================

#[test]
fn test_message_type_sync_request() {
    let node_id = NodeId::generate();
    let request = SyncRequest::new(node_id, 0);
    let msg = Message::SyncRequest(request);

    assert_eq!(msg.message_type(), MessageType::SyncRequest);
}

#[test]
fn test_message_type_sync_response() {
    let node_id = NodeId::generate();
    let response = SyncResponse::new(node_id, 5, vec![]);
    let msg = Message::SyncResponse(response);

    assert_eq!(msg.message_type(), MessageType::SyncResponse);
}

#[test]
fn test_message_type_iou_announcement() {
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

    assert_eq!(msg.message_type(), MessageType::IOUAnnouncement);
}

#[test]
fn test_message_type_peer_announcement() {
    let node_id = NodeId::generate();
    let announcement = PeerAnnouncement::new(node_id, 8080);
    let msg = Message::PeerAnnouncement(announcement);

    assert_eq!(msg.message_type(), MessageType::PeerAnnouncement);
}

#[test]
fn test_message_type_heartbeat() {
    let node_id = NodeId::generate();
    let heartbeat = Heartbeat::new(node_id, 10);
    let msg = Message::Heartbeat(heartbeat);

    assert_eq!(msg.message_type(), MessageType::Heartbeat);
}

// ============================================================================
// SYNC REQUEST
// ============================================================================

#[test]
fn test_sync_request_creation() {
    let node_id = NodeId::generate();
    let request = SyncRequest::new(node_id.clone(), 42);

    assert_eq!(request.sender(), &node_id);
    assert_eq!(request.known_version(), 42);
}

#[test]
fn test_sync_request_with_filter() {
    let node_id = NodeId::generate();
    let alice = Keypair::generate();
    let alice_did = Did::from_public_key(&alice.public_key());

    let request = SyncRequest::new(node_id, 0)
        .with_sender_filter(alice_did.clone());

    assert_eq!(request.sender_filter(), Some(&alice_did));
}

// ============================================================================
// SYNC RESPONSE
// ============================================================================

#[test]
fn test_sync_response_empty() {
    let node_id = NodeId::generate();
    let response = SyncResponse::new(node_id.clone(), 100, vec![]);

    assert_eq!(response.sender(), &node_id);
    assert_eq!(response.current_version(), 100);
    assert!(response.entries().is_empty());
}

#[test]
fn test_sync_response_with_entries() {
    let node_id = NodeId::generate();
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let entry = p2pmesh::ledger::IOUEntry::new(iou, alice.public_key());
    let response = SyncResponse::new(node_id, 1, vec![entry.clone()]);

    assert_eq!(response.entries().len(), 1);
}

// ============================================================================
// IOU ANNOUNCEMENT
// ============================================================================

#[test]
fn test_iou_announcement_creation() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(500)
        .build()
        .unwrap();

    let announcement = IOUAnnouncement::new(iou.clone(), alice.public_key());

    assert_eq!(announcement.iou().id(), iou.id());
    assert_eq!(announcement.sender_pubkey().as_bytes(), alice.public_key().as_bytes());
}

#[test]
fn test_iou_announcement_hop_count() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let mut announcement = IOUAnnouncement::new(iou, alice.public_key());

    assert_eq!(announcement.hop_count(), 0);
    announcement.increment_hop();
    assert_eq!(announcement.hop_count(), 1);
}

#[test]
fn test_iou_announcement_max_hops() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let mut announcement = IOUAnnouncement::new(iou, alice.public_key())
        .with_max_hops(3);

    assert!(!announcement.should_stop_propagation());
    announcement.increment_hop();
    announcement.increment_hop();
    announcement.increment_hop();
    assert!(announcement.should_stop_propagation());
}

// ============================================================================
// PEER ANNOUNCEMENT
// ============================================================================

#[test]
fn test_peer_announcement_creation() {
    let node_id = NodeId::generate();
    let announcement = PeerAnnouncement::new(node_id.clone(), 9000);

    assert_eq!(announcement.node_id(), &node_id);
    assert_eq!(announcement.port(), 9000);
}

#[test]
fn test_peer_announcement_with_address() {
    let node_id = NodeId::generate();
    let announcement = PeerAnnouncement::new(node_id, 8080)
        .with_address("192.168.1.100".to_string());

    assert_eq!(announcement.address(), Some(&"192.168.1.100".to_string()));
}

#[test]
fn test_peer_announcement_capabilities() {
    let node_id = NodeId::generate();
    let announcement = PeerAnnouncement::new(node_id, 8080)
        .with_capability("relay")
        .with_capability("gateway");

    assert!(announcement.has_capability("relay"));
    assert!(announcement.has_capability("gateway"));
    assert!(!announcement.has_capability("unknown"));
}

// ============================================================================
// HEARTBEAT
// ============================================================================

#[test]
fn test_heartbeat_creation() {
    let node_id = NodeId::generate();
    let heartbeat = Heartbeat::new(node_id.clone(), 42);

    assert_eq!(heartbeat.sender(), &node_id);
    assert_eq!(heartbeat.version(), 42);
}

#[test]
fn test_heartbeat_timestamp() {
    let node_id = NodeId::generate();
    let heartbeat = Heartbeat::new(node_id, 0);

    assert!(heartbeat.timestamp() > 0);
}

// ============================================================================
// MESSAGE SERIALIZATION
// ============================================================================

#[test]
fn test_sync_request_serialization_roundtrip() {
    let node_id = NodeId::generate();
    let request = SyncRequest::new(node_id, 100);
    let msg = Message::SyncRequest(request);

    let bytes = msg.to_bytes();
    let restored = Message::from_bytes(&bytes).unwrap();

    assert_eq!(restored.message_type(), MessageType::SyncRequest);
}

#[test]
fn test_sync_response_serialization_roundtrip() {
    let node_id = NodeId::generate();
    let response = SyncResponse::new(node_id, 50, vec![]);
    let msg = Message::SyncResponse(response);

    let bytes = msg.to_bytes();
    let restored = Message::from_bytes(&bytes).unwrap();

    assert_eq!(restored.message_type(), MessageType::SyncResponse);
}

#[test]
fn test_iou_announcement_serialization_roundtrip() {
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

    let bytes = msg.to_bytes();
    let restored = Message::from_bytes(&bytes).unwrap();

    assert_eq!(restored.message_type(), MessageType::IOUAnnouncement);
}

#[test]
fn test_peer_announcement_serialization_roundtrip() {
    let node_id = NodeId::generate();
    let announcement = PeerAnnouncement::new(node_id, 8080)
        .with_address("127.0.0.1".to_string());
    let msg = Message::PeerAnnouncement(announcement);

    let bytes = msg.to_bytes();
    let restored = Message::from_bytes(&bytes).unwrap();

    assert_eq!(restored.message_type(), MessageType::PeerAnnouncement);
}

#[test]
fn test_heartbeat_serialization_roundtrip() {
    let node_id = NodeId::generate();
    let heartbeat = Heartbeat::new(node_id, 25);
    let msg = Message::Heartbeat(heartbeat);

    let bytes = msg.to_bytes();
    let restored = Message::from_bytes(&bytes).unwrap();

    assert_eq!(restored.message_type(), MessageType::Heartbeat);
}

#[test]
fn test_invalid_message_bytes() {
    let result = Message::from_bytes(b"garbage");

    assert!(matches!(result, Err(ProtocolError::DeserializationFailed)));
}

// ============================================================================
// MESSAGE ID AND DEDUPLICATION
// ============================================================================

#[test]
fn test_message_has_unique_id() {
    let node_id = NodeId::generate();
    let msg1 = Message::Heartbeat(Heartbeat::new(node_id.clone(), 1));
    let msg2 = Message::Heartbeat(Heartbeat::new(node_id, 2));

    assert_ne!(msg1.id(), msg2.id());
}

#[test]
fn test_same_content_same_id() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .nonce(42)
        .build()
        .unwrap();

    let ann1 = IOUAnnouncement::new(iou.clone(), alice.public_key());
    let ann2 = IOUAnnouncement::new(iou, alice.public_key());

    // Same IOU should produce same announcement ID (for deduplication)
    assert_eq!(ann1.id(), ann2.id());
}
