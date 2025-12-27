// Collector Tests
// Tests for gathering IOUs for settlement

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::{IOUBuilder, SignedIOU};
use p2pmesh::ledger::{MeshState, NodeId};
use p2pmesh::gateway::{
    Collector, CollectorConfig, CollectorError,
    SettlementBatch, BatchId, BatchStatus,
    SettlementEntry, NetPosition,
};

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn create_test_iou(sender: &Keypair, recipient: &Keypair, amount: u64, nonce: u64) -> SignedIOU {
    IOUBuilder::new()
        .sender(sender)
        .recipient(Did::from_public_key(&recipient.public_key()))
        .amount(amount)
        .nonce(nonce)
        .build()
        .unwrap()
}

fn create_mesh_with_ious(node_id: NodeId, ious: Vec<(SignedIOU, &Keypair)>) -> MeshState {
    let mut state = MeshState::new(node_id);
    for (iou, sender_kp) in ious {
        state.add_iou(iou, &sender_kp.public_key()).unwrap();
    }
    state
}

// ============================================================================
// COLLECTOR CONFIG
// ============================================================================

#[test]
fn test_collector_config_default() {
    let config = CollectorConfig::default();

    assert!(config.min_batch_size > 0);
    assert!(config.max_batch_size > config.min_batch_size);
    assert!(config.min_iou_age_secs >= 0);
}

#[test]
fn test_collector_config_custom() {
    let config = CollectorConfig::new()
        .with_min_batch_size(5)
        .with_max_batch_size(100)
        .with_min_iou_age_secs(3600)
        .with_min_amount(100);

    assert_eq!(config.min_batch_size, 5);
    assert_eq!(config.max_batch_size, 100);
    assert_eq!(config.min_iou_age_secs, 3600);
    assert_eq!(config.min_amount, 100);
}

#[test]
fn test_collector_config_with_settlement_threshold() {
    let config = CollectorConfig::new()
        .with_settlement_threshold(1000);

    assert_eq!(config.settlement_threshold, 1000);
}

#[test]
fn test_collector_config_validation() {
    let valid_config = CollectorConfig::default();
    assert!(valid_config.validate().is_ok());

    let invalid_config = CollectorConfig::new()
        .with_min_batch_size(100)
        .with_max_batch_size(10); // max < min
    assert!(invalid_config.validate().is_err());
}

// ============================================================================
// COLLECTOR CREATION
// ============================================================================

#[test]
fn test_collector_new() {
    let config = CollectorConfig::default();
    let collector = Collector::new(config);

    assert_eq!(collector.pending_batches(), 0);
    assert_eq!(collector.total_collected(), 0);
}

#[test]
fn test_collector_with_config() {
    let config = CollectorConfig::new()
        .with_min_batch_size(10);
    let collector = Collector::new(config);

    assert_eq!(collector.config().min_batch_size, 10);
}

// ============================================================================
// IOU COLLECTION
// ============================================================================

#[test]
fn test_collector_collect_from_empty_state() {
    let config = CollectorConfig::default();
    let mut collector = Collector::new(config);

    let node_id = NodeId::generate();
    let state = MeshState::new(node_id);

    let result = collector.collect_from_state(&state);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0); // No IOUs collected
}

#[test]
fn test_collector_collect_single_iou() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let iou = create_test_iou(&alice, &bob, 100, 1);

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![(iou, &alice)]);

    let result = collector.collect_from_state(&state);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);
    assert_eq!(collector.total_collected(), 1);
}

#[test]
fn test_collector_collect_multiple_ious() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let ious: Vec<(SignedIOU, &Keypair)> = (0..5)
        .map(|i| (create_test_iou(&alice, &bob, 100, i), &alice))
        .collect();

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, ious);

    let result = collector.collect_from_state(&state);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);
}

#[test]
fn test_collector_filters_by_min_amount() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0)
        .with_min_amount(50);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou_small = create_test_iou(&alice, &bob, 10, 1);  // Below threshold
    let iou_large = create_test_iou(&alice, &bob, 100, 2); // Above threshold

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![
        (iou_small, &alice),
        (iou_large, &alice),
    ]);

    let result = collector.collect_from_state(&state);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1); // Only the large IOU
}

#[test]
fn test_collector_skips_already_collected() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let iou = create_test_iou(&alice, &bob, 100, 1);

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![(iou, &alice)]);

    // Collect once
    collector.collect_from_state(&state).unwrap();
    assert_eq!(collector.total_collected(), 1);

    // Collect again - should not duplicate
    let result = collector.collect_from_state(&state);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0); // No new IOUs
    assert_eq!(collector.total_collected(), 1);
}

#[test]
fn test_collector_collect_by_sender() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let iou_from_alice = create_test_iou(&alice, &bob, 100, 1);
    let iou_from_bob = create_test_iou(&bob, &charlie, 100, 2);

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![
        (iou_from_alice, &alice),
        (iou_from_bob, &bob),
    ]);

    let alice_did = Did::from_public_key(&alice.public_key());
    let result = collector.collect_by_sender(&state, &alice_did);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1); // Only Alice's IOU
}

#[test]
fn test_collector_collect_by_recipient() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let iou_to_bob = create_test_iou(&alice, &bob, 100, 1);
    let iou_to_charlie = create_test_iou(&alice, &charlie, 100, 2);

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![
        (iou_to_bob, &alice),
        (iou_to_charlie, &alice),
    ]);

    let bob_did = Did::from_public_key(&bob.public_key());
    let result = collector.collect_by_recipient(&state, &bob_did);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1); // Only IOU to Bob
}

// ============================================================================
// BATCH CREATION
// ============================================================================

#[test]
fn test_collector_create_batch_empty() {
    let config = CollectorConfig::new().with_min_batch_size(1);
    let mut collector = Collector::new(config);

    let result = collector.create_batch();

    assert!(matches!(result, Err(CollectorError::InsufficientIOUs)));
}

#[test]
fn test_collector_create_batch() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![
        (create_test_iou(&alice, &bob, 100, 1), &alice),
        (create_test_iou(&alice, &bob, 200, 2), &alice),
    ]);

    collector.collect_from_state(&state).unwrap();

    let batch = collector.create_batch().unwrap();

    assert_eq!(batch.entries().len(), 2);
    assert_eq!(batch.total_amount(), 300);
    assert!(matches!(batch.status(), BatchStatus::Pending));
}

#[test]
fn test_collector_create_batch_respects_max_size() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_max_batch_size(3)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let ious: Vec<(SignedIOU, &Keypair)> = (0..10)
        .map(|i| (create_test_iou(&alice, &bob, 100, i), &alice))
        .collect();

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, ious);

    collector.collect_from_state(&state).unwrap();

    let batch = collector.create_batch().unwrap();

    assert_eq!(batch.entries().len(), 3); // Capped at max_batch_size
}

#[test]
fn test_collector_create_batch_min_size_not_met() {
    let config = CollectorConfig::new()
        .with_min_batch_size(5)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![
        (create_test_iou(&alice, &bob, 100, 1), &alice),
        (create_test_iou(&alice, &bob, 100, 2), &alice),
    ]);

    collector.collect_from_state(&state).unwrap();

    let result = collector.create_batch();

    assert!(matches!(result, Err(CollectorError::InsufficientIOUs)));
}

// ============================================================================
// BATCH ID
// ============================================================================

#[test]
fn test_batch_id_generation() {
    let id1 = BatchId::generate();
    let id2 = BatchId::generate();

    assert_ne!(id1, id2);
}

#[test]
fn test_batch_id_from_bytes() {
    let bytes = [1u8; 32];
    let id = BatchId::from_bytes(bytes);

    assert_eq!(id.as_bytes(), &bytes);
}

#[test]
fn test_batch_id_display() {
    let id = BatchId::generate();
    let display = format!("{}", id);

    assert!(!display.is_empty());
}

// ============================================================================
// SETTLEMENT BATCH
// ============================================================================

#[test]
fn test_settlement_batch_creation() {
    let batch = SettlementBatch::new();

    assert!(batch.entries().is_empty());
    assert_eq!(batch.total_amount(), 0);
    assert!(matches!(batch.status(), BatchStatus::Pending));
}

#[test]
fn test_settlement_batch_add_entry() {
    let mut batch = SettlementBatch::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let iou = create_test_iou(&alice, &bob, 100, 1);

    let entry = SettlementEntry::from_iou(&iou);
    batch.add_entry(entry);

    assert_eq!(batch.entries().len(), 1);
    assert_eq!(batch.total_amount(), 100);
}

#[test]
fn test_settlement_batch_multiple_entries() {
    let mut batch = SettlementBatch::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    for i in 1..=5 {
        let iou = create_test_iou(&alice, &bob, i * 100, i);
        batch.add_entry(SettlementEntry::from_iou(&iou));
    }

    assert_eq!(batch.entries().len(), 5);
    assert_eq!(batch.total_amount(), 1500); // 100+200+300+400+500
}

#[test]
fn test_settlement_batch_status_transitions() {
    let mut batch = SettlementBatch::new();

    assert!(matches!(batch.status(), BatchStatus::Pending));

    batch.set_status(BatchStatus::Processing);
    assert!(matches!(batch.status(), BatchStatus::Processing));

    batch.set_status(BatchStatus::Submitted);
    assert!(matches!(batch.status(), BatchStatus::Submitted));

    batch.set_status(BatchStatus::Confirmed);
    assert!(matches!(batch.status(), BatchStatus::Confirmed));
}

#[test]
fn test_settlement_batch_has_id() {
    let batch = SettlementBatch::new();

    assert!(!batch.id().as_bytes().iter().all(|&b| b == 0));
}

#[test]
fn test_settlement_batch_created_at() {
    let batch = SettlementBatch::new();

    assert!(batch.created_at() > 0);
}

// ============================================================================
// SETTLEMENT ENTRY
// ============================================================================

#[test]
fn test_settlement_entry_from_iou() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let iou = create_test_iou(&alice, &bob, 500, 1);

    let entry = SettlementEntry::from_iou(&iou);

    assert_eq!(entry.amount(), 500);
    assert_eq!(entry.sender(), iou.iou().sender());
    assert_eq!(entry.recipient(), iou.iou().recipient());
    assert_eq!(entry.iou_id(), &iou.id());
}

#[test]
fn test_settlement_entry_serialization() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let iou = create_test_iou(&alice, &bob, 100, 1);

    let entry = SettlementEntry::from_iou(&iou);
    let bytes = entry.to_bytes();

    assert!(!bytes.is_empty());

    let restored = SettlementEntry::from_bytes(&bytes).unwrap();
    assert_eq!(entry.amount(), restored.amount());
    assert_eq!(entry.iou_id(), restored.iou_id());
}

// ============================================================================
// NET POSITION CALCULATION
// ============================================================================

#[test]
fn test_net_position_single_direction() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let alice_did = Did::from_public_key(&alice.public_key());
    let bob_did = Did::from_public_key(&bob.public_key());

    let mut batch = SettlementBatch::new();

    // Alice owes Bob 300 (100 + 200)
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&alice, &bob, 100, 1)));
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&alice, &bob, 200, 2)));

    let positions = batch.calculate_net_positions();

    let alice_pos = positions.iter().find(|p| p.party() == &alice_did).unwrap();
    let bob_pos = positions.iter().find(|p| p.party() == &bob_did).unwrap();

    assert_eq!(alice_pos.net_amount(), -300); // Alice owes 300
    assert_eq!(bob_pos.net_amount(), 300);    // Bob is owed 300
}

#[test]
fn test_net_position_bidirectional() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let alice_did = Did::from_public_key(&alice.public_key());
    let bob_did = Did::from_public_key(&bob.public_key());

    let mut batch = SettlementBatch::new();

    // Alice → Bob: 300
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&alice, &bob, 300, 1)));
    // Bob → Alice: 100
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&bob, &alice, 100, 2)));

    let positions = batch.calculate_net_positions();

    let alice_pos = positions.iter().find(|p| p.party() == &alice_did).unwrap();
    let bob_pos = positions.iter().find(|p| p.party() == &bob_did).unwrap();

    // Net: Alice owes Bob 200
    assert_eq!(alice_pos.net_amount(), -200);
    assert_eq!(bob_pos.net_amount(), 200);
}

#[test]
fn test_net_position_three_parties() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let alice_did = Did::from_public_key(&alice.public_key());
    let bob_did = Did::from_public_key(&bob.public_key());
    let charlie_did = Did::from_public_key(&charlie.public_key());

    let mut batch = SettlementBatch::new();

    // Alice → Bob: 100
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&alice, &bob, 100, 1)));
    // Bob → Charlie: 150
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&bob, &charlie, 150, 2)));
    // Charlie → Alice: 50
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&charlie, &alice, 50, 3)));

    let positions = batch.calculate_net_positions();

    let alice_pos = positions.iter().find(|p| p.party() == &alice_did).unwrap();
    let bob_pos = positions.iter().find(|p| p.party() == &bob_did).unwrap();
    let charlie_pos = positions.iter().find(|p| p.party() == &charlie_did).unwrap();

    // Alice: -100 + 50 = -50
    assert_eq!(alice_pos.net_amount(), -50);
    // Bob: +100 - 150 = -50
    assert_eq!(bob_pos.net_amount(), -50);
    // Charlie: +150 - 50 = +100
    assert_eq!(charlie_pos.net_amount(), 100);

    // Sum should be zero
    let total: i64 = positions.iter().map(|p| p.net_amount()).sum();
    assert_eq!(total, 0);
}

#[test]
fn test_net_position_perfectly_balanced() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let alice_did = Did::from_public_key(&alice.public_key());
    let bob_did = Did::from_public_key(&bob.public_key());

    let mut batch = SettlementBatch::new();

    // Alice → Bob: 100
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&alice, &bob, 100, 1)));
    // Bob → Alice: 100
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&bob, &alice, 100, 2)));

    let positions = batch.calculate_net_positions();

    let alice_pos = positions.iter().find(|p| p.party() == &alice_did).unwrap();
    let bob_pos = positions.iter().find(|p| p.party() == &bob_did).unwrap();

    // Net: both zero
    assert_eq!(alice_pos.net_amount(), 0);
    assert_eq!(bob_pos.net_amount(), 0);
}

// ============================================================================
// BATCH MANAGEMENT
// ============================================================================

#[test]
fn test_collector_pending_batches() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_max_batch_size(2)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let ious: Vec<(SignedIOU, &Keypair)> = (0..6)
        .map(|i| (create_test_iou(&alice, &bob, 100, i), &alice))
        .collect();

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, ious);

    collector.collect_from_state(&state).unwrap();

    // Create multiple batches
    collector.create_batch().unwrap();
    collector.create_batch().unwrap();

    assert_eq!(collector.pending_batches(), 2);
}

#[test]
fn test_collector_get_batch() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![
        (create_test_iou(&alice, &bob, 100, 1), &alice),
    ]);

    collector.collect_from_state(&state).unwrap();
    let batch = collector.create_batch().unwrap();
    let batch_id = batch.id().clone();

    let retrieved = collector.get_batch(&batch_id);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id(), &batch_id);
}

#[test]
fn test_collector_remove_batch() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![
        (create_test_iou(&alice, &bob, 100, 1), &alice),
    ]);

    collector.collect_from_state(&state).unwrap();
    let batch = collector.create_batch().unwrap();
    let batch_id = batch.id().clone();

    assert_eq!(collector.pending_batches(), 1);

    collector.remove_batch(&batch_id).unwrap();

    assert_eq!(collector.pending_batches(), 0);
    assert!(collector.get_batch(&batch_id).is_none());
}

#[test]
fn test_collector_update_batch_status() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![
        (create_test_iou(&alice, &bob, 100, 1), &alice),
    ]);

    collector.collect_from_state(&state).unwrap();
    let batch = collector.create_batch().unwrap();
    let batch_id = batch.id().clone();

    collector.update_batch_status(&batch_id, BatchStatus::Submitted).unwrap();

    let batch = collector.get_batch(&batch_id).unwrap();
    assert!(matches!(batch.status(), BatchStatus::Submitted));
}

// ============================================================================
// STATISTICS
// ============================================================================

#[test]
fn test_collector_stats() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![
        (create_test_iou(&alice, &bob, 100, 1), &alice),
        (create_test_iou(&alice, &bob, 200, 2), &alice),
    ]);

    collector.collect_from_state(&state).unwrap();
    collector.create_batch().unwrap();

    let stats = collector.stats();

    assert_eq!(stats.total_collected, 2);
    assert_eq!(stats.total_amount_collected, 300);
    assert_eq!(stats.batches_created, 1);
}

#[test]
fn test_collector_reset_stats() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let state = create_mesh_with_ious(node_id, vec![
        (create_test_iou(&alice, &bob, 100, 1), &alice),
    ]);

    collector.collect_from_state(&state).unwrap();

    collector.reset_stats();

    let stats = collector.stats();
    assert_eq!(stats.total_collected, 0);
}
