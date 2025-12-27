// Mesh State Tests
// Tests for tracking the current state of the mesh network

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::IOUBuilder;
use p2pmesh::ledger::{MeshState, MeshStateError, NodeId};

// ============================================================================
// MESH STATE CREATION
// ============================================================================

#[test]
fn test_mesh_state_new() {
    let node_id = NodeId::generate();
    let state = MeshState::new(node_id.clone());

    assert_eq!(state.node_id(), &node_id);
    assert!(state.is_empty());
    assert_eq!(state.iou_count(), 0);
}

#[test]
fn test_node_id_generate_unique() {
    let id1 = NodeId::generate();
    let id2 = NodeId::generate();

    assert_ne!(id1, id2);
}

#[test]
fn test_node_id_from_keypair() {
    let keypair = Keypair::generate();
    let node_id = NodeId::from_public_key(&keypair.public_key());

    // Same keypair should produce same node ID
    let node_id2 = NodeId::from_public_key(&keypair.public_key());
    assert_eq!(node_id, node_id2);
}

// ============================================================================
// ADDING IOUs TO MESH STATE
// ============================================================================

fn create_test_iou(sender: &Keypair, recipient: &Keypair, amount: u64, nonce: u64) -> p2pmesh::iou::SignedIOU {
    IOUBuilder::new()
        .sender(sender)
        .recipient(Did::from_public_key(&recipient.public_key()))
        .amount(amount)
        .nonce(nonce)
        .build()
        .unwrap()
}

#[test]
fn test_add_iou_to_state() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);

    state.add_iou(iou.clone(), &alice.public_key()).unwrap();

    assert_eq!(state.iou_count(), 1);
    assert!(state.has_iou(&iou.id()));
}

#[test]
fn test_add_duplicate_iou_fails() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);

    state.add_iou(iou.clone(), &alice.public_key()).unwrap();
    let result = state.add_iou(iou, &alice.public_key());

    assert!(matches!(result, Err(MeshStateError::DuplicateIOU)));
}

#[test]
fn test_add_iou_wrong_sender_pubkey_fails() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);

    // Try to add with wrong sender public key
    // This fails with SenderMismatch (DID check) before signature verification
    let result = state.add_iou(iou, &charlie.public_key());

    assert!(matches!(result, Err(MeshStateError::ValidationFailed(_))));
}

#[test]
fn test_add_multiple_ious() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    for i in 0..10 {
        let iou = create_test_iou(&alice, &bob, 100, i);
        state.add_iou(iou, &alice.public_key()).unwrap();
    }

    assert_eq!(state.iou_count(), 10);
}

// ============================================================================
// QUERYING MESH STATE
// ============================================================================

#[test]
fn test_get_iou_by_id() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    let iou_id = iou.id();

    state.add_iou(iou, &alice.public_key()).unwrap();

    let entry = state.get_iou(&iou_id).unwrap();
    assert_eq!(entry.iou().iou().amount(), 100);
}

#[test]
fn test_get_ious_by_sender() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    // Alice sends 3 IOUs
    for i in 0..3 {
        let iou = create_test_iou(&alice, &bob, 100, i);
        state.add_iou(iou, &alice.public_key()).unwrap();
    }

    // Bob sends 2 IOUs
    for i in 0..2 {
        let iou = create_test_iou(&bob, &charlie, 50, i);
        state.add_iou(iou, &bob.public_key()).unwrap();
    }

    let alice_did = Did::from_public_key(&alice.public_key());
    let alice_ious = state.get_ious_by_sender(&alice_did);

    assert_eq!(alice_ious.len(), 3);
}

#[test]
fn test_get_ious_by_recipient() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    // Both Alice and Charlie send to Bob
    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let iou2 = create_test_iou(&charlie, &bob, 200, 1);

    state.add_iou(iou1, &alice.public_key()).unwrap();
    state.add_iou(iou2, &charlie.public_key()).unwrap();

    let bob_did = Did::from_public_key(&bob.public_key());
    let bob_received = state.get_ious_by_recipient(&bob_did);

    assert_eq!(bob_received.len(), 2);
}

// ============================================================================
// MESH STATE SYNCHRONIZATION
// ============================================================================

#[test]
fn test_merge_states() {
    let node1_id = NodeId::generate();
    let node2_id = NodeId::generate();

    let mut state1 = MeshState::new(node1_id);
    let mut state2 = MeshState::new(node2_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    // Node 1 has IOU 1
    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    state1.add_iou(iou1, &alice.public_key()).unwrap();

    // Node 2 has IOU 2
    let iou2 = create_test_iou(&alice, &bob, 200, 2);
    state2.add_iou(iou2, &alice.public_key()).unwrap();

    // Merge node2 into node1
    let result = state1.merge(&state2);

    assert_eq!(result.new_entries, 1);
    assert_eq!(state1.iou_count(), 2);
}

#[test]
fn test_merge_identical_states() {
    let node1_id = NodeId::generate();
    let node2_id = NodeId::generate();

    let mut state1 = MeshState::new(node1_id);
    let mut state2 = MeshState::new(node2_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    state1.add_iou(iou.clone(), &alice.public_key()).unwrap();
    state2.add_iou(iou, &alice.public_key()).unwrap();

    let result = state1.merge(&state2);

    assert_eq!(result.new_entries, 0);
    assert_eq!(state1.iou_count(), 1);
}

#[test]
fn test_get_delta_for_sync() {
    let node1_id = NodeId::generate();
    let node2_id = NodeId::generate();

    let mut state1 = MeshState::new(node1_id);
    let mut state2 = MeshState::new(node2_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    // Both have IOU 1
    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    state1.add_iou(iou1.clone(), &alice.public_key()).unwrap();
    state2.add_iou(iou1, &alice.public_key()).unwrap();

    // Only state2 has IOU 2 and 3
    let iou2 = create_test_iou(&alice, &bob, 200, 2);
    let iou3 = create_test_iou(&alice, &bob, 300, 3);
    state2.add_iou(iou2, &alice.public_key()).unwrap();
    state2.add_iou(iou3, &alice.public_key()).unwrap();

    // Get delta (what state2 has that state1 doesn't)
    let delta = state2.delta(&state1);

    assert_eq!(delta.len(), 2);
}

// ============================================================================
// STATE EXPORT/IMPORT FOR PERSISTENCE
// ============================================================================

#[test]
fn test_state_serialization_roundtrip() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id.clone());

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    state.add_iou(iou, &alice.public_key()).unwrap();

    // Serialize
    let bytes = state.to_bytes();

    // Deserialize
    let restored = MeshState::from_bytes(&bytes).unwrap();

    assert_eq!(restored.node_id(), &node_id);
    assert_eq!(restored.iou_count(), 1);
}

// ============================================================================
// STATE VERSION/CLOCK
// ============================================================================

#[test]
fn test_state_version_increments() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let initial_version = state.version();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    state.add_iou(iou, &alice.public_key()).unwrap();

    assert!(state.version() > initial_version);
}

#[test]
fn test_state_version_increments_on_merge() {
    let node1_id = NodeId::generate();
    let node2_id = NodeId::generate();

    let mut state1 = MeshState::new(node1_id);
    let mut state2 = MeshState::new(node2_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    state2.add_iou(iou, &alice.public_key()).unwrap();

    let version_before = state1.version();
    state1.merge(&state2);

    assert!(state1.version() > version_before);
}

// ============================================================================
// BALANCE COMPUTATION FROM STATE
// ============================================================================

#[test]
fn test_compute_balance_for_did() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    // Charlie sends 1000 to Alice (funding)
    let funding = create_test_iou(&charlie, &alice, 1000, 1);
    state.add_iou(funding, &charlie.public_key()).unwrap();

    // Alice sends 300 to Bob
    let payment = create_test_iou(&alice, &bob, 300, 1);
    state.add_iou(payment, &alice.public_key()).unwrap();

    let alice_did = Did::from_public_key(&alice.public_key());

    // Note: This is a simplified view - actual balance would need
    // to account for UTXO consumption, which is tracked in the vault
    let received = state.total_received(&alice_did);
    let sent = state.total_sent(&alice_did);

    assert_eq!(received, 1000);
    assert_eq!(sent, 300);
}

// ============================================================================
// MESH STATE STATISTICS
// ============================================================================

#[test]
fn test_state_statistics() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    // Various transactions
    state.add_iou(create_test_iou(&alice, &bob, 100, 1), &alice.public_key()).unwrap();
    state.add_iou(create_test_iou(&bob, &charlie, 50, 1), &bob.public_key()).unwrap();
    state.add_iou(create_test_iou(&charlie, &alice, 25, 1), &charlie.public_key()).unwrap();

    let stats = state.statistics();

    assert_eq!(stats.total_ious, 3);
    assert_eq!(stats.unique_senders, 3);
    assert_eq!(stats.unique_recipients, 3);
    assert_eq!(stats.total_value, 175);
}
