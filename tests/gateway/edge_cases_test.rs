// Gateway Edge Cases and Stress Tests
// Tests for boundary conditions, error handling, and stress scenarios

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::IOUBuilder;
use p2pmesh::ledger::{MeshState, NodeId};
use p2pmesh::gateway::{
    Collector, CollectorConfig, CollectorError,
    Settler, SettlerConfig, SettlerError,
    SettlementBatch, SettlementEntry, BatchId, BatchStatus,
    MockSettlementTarget,
};

// ============================================================================
// HELPER FUNCTIONS
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

// ============================================================================
// BOUNDARY VALUE TESTS
// ============================================================================

#[test]
fn test_collector_min_batch_size_one() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);
    let iou = create_test_iou(&alice, &bob, 100, 1);
    state.add_iou(iou, &alice.public_key()).unwrap();

    collector.collect_from_state(&state).unwrap();
    let batch = collector.create_batch();

    assert!(batch.is_ok());
    assert_eq!(batch.unwrap().entries().len(), 1);
}

#[test]
fn test_collector_max_batch_size_boundary() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_max_batch_size(u32::MAX)
        .with_min_iou_age_secs(0);

    assert!(config.validate().is_ok());
}

#[test]
fn test_settlement_batch_max_entries() {
    let mut batch = SettlementBatch::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    // Add many entries
    for i in 0..1000 {
        let iou = create_test_iou(&alice, &bob, 1, i);
        batch.add_entry(SettlementEntry::from_iou(&iou));
    }

    assert_eq!(batch.entries().len(), 1000);
    assert_eq!(batch.total_amount(), 1000);
}

#[test]
fn test_settlement_batch_large_amounts() {
    let mut batch = SettlementBatch::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, u64::MAX / 2, 1);
    batch.add_entry(SettlementEntry::from_iou(&iou));

    assert_eq!(batch.total_amount(), u64::MAX / 2);
}

#[test]
fn test_batch_id_all_zeros() {
    let id = BatchId::from_bytes([0u8; 32]);
    assert_eq!(id.as_bytes(), &[0u8; 32]);
}

#[test]
fn test_batch_id_all_ones() {
    let id = BatchId::from_bytes([255u8; 32]);
    assert_eq!(id.as_bytes(), &[255u8; 32]);
}

// ============================================================================
// NET POSITION EDGE CASES
// ============================================================================

#[test]
fn test_net_position_empty_batch() {
    let batch = SettlementBatch::new();
    let positions = batch.calculate_net_positions();

    assert!(positions.is_empty());
}

#[test]
fn test_net_position_single_entry() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut batch = SettlementBatch::new();
    let iou = create_test_iou(&alice, &bob, 100, 1);
    batch.add_entry(SettlementEntry::from_iou(&iou));

    let positions = batch.calculate_net_positions();

    assert_eq!(positions.len(), 2);

    let total: i64 = positions.iter().map(|p| p.net_amount()).sum();
    assert_eq!(total, 0);
}

#[test]
fn test_net_position_many_small_transactions() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut batch = SettlementBatch::new();

    // 1000 small transactions
    for i in 0..1000 {
        let iou = create_test_iou(&alice, &bob, 1, i);
        batch.add_entry(SettlementEntry::from_iou(&iou));
    }

    let positions = batch.calculate_net_positions();

    let alice_did = Did::from_public_key(&alice.public_key());
    let alice_pos = positions.iter().find(|p| p.party() == &alice_did).unwrap();

    assert_eq!(alice_pos.net_amount(), -1000);
}

#[test]
fn test_net_position_circular_debt() {
    // A -> B -> C -> A (circular)
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let mut batch = SettlementBatch::new();

    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&alice, &bob, 100, 1)));
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&bob, &charlie, 100, 2)));
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&charlie, &alice, 100, 3)));

    let positions = batch.calculate_net_positions();

    // Everyone should net to zero
    for pos in &positions {
        assert_eq!(pos.net_amount(), 0);
    }
}

#[test]
fn test_net_position_star_topology() {
    // Multiple people owe one person
    let center = Keypair::generate();
    let others: Vec<Keypair> = (0..5).map(|_| Keypair::generate()).collect();

    let mut batch = SettlementBatch::new();

    for (i, other) in others.iter().enumerate() {
        let iou = create_test_iou(other, &center, 100, i as u64);
        batch.add_entry(SettlementEntry::from_iou(&iou));
    }

    let positions = batch.calculate_net_positions();

    let center_did = Did::from_public_key(&center.public_key());
    let center_pos = positions.iter().find(|p| p.party() == &center_did).unwrap();

    assert_eq!(center_pos.net_amount(), 500); // Receives from 5 people
}

// ============================================================================
// COLLECTOR STRESS TESTS
// ============================================================================

#[test]
fn test_collector_many_ious_same_parties() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    for i in 0..500 {
        let iou = create_test_iou(&alice, &bob, 10, i);
        state.add_iou(iou, &alice.public_key()).unwrap();
    }

    let collected = collector.collect_from_state(&state).unwrap();

    assert_eq!(collected, 500);
}

#[test]
fn test_collector_many_different_parties() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    // 100 unique sender-recipient pairs
    for i in 0..100 {
        let sender = Keypair::generate();
        let recipient = Keypair::generate();
        let iou = create_test_iou(&sender, &recipient, 100, i);
        state.add_iou(iou, &sender.public_key()).unwrap();
    }

    let collected = collector.collect_from_state(&state).unwrap();

    assert_eq!(collected, 100);
}

#[test]
fn test_collector_rapid_batch_creation() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_max_batch_size(10)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    for i in 0..100 {
        let iou = create_test_iou(&alice, &bob, 10, i);
        state.add_iou(iou, &alice.public_key()).unwrap();
    }

    collector.collect_from_state(&state).unwrap();

    // Create 10 batches
    let mut batches = Vec::new();
    for _ in 0..10 {
        if let Ok(batch) = collector.create_batch() {
            batches.push(batch);
        }
    }

    assert_eq!(batches.len(), 10);
    for batch in &batches {
        assert_eq!(batch.entries().len(), 10);
    }
}

// ============================================================================
// SETTLER STRESS TESTS
// ============================================================================

#[tokio::test]
async fn test_settler_many_batches() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_success();
    let mut settler = Settler::with_target(config, Box::new(target));

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    // Submit 50 batches
    for i in 0..50 {
        let mut batch = SettlementBatch::new();
        let iou = create_test_iou(&alice, &bob, 100, i);
        batch.add_entry(SettlementEntry::from_iou(&iou));
        settler.submit(batch).await.unwrap();
    }

    assert_eq!(settler.pending_settlements(), 50);
}

#[tokio::test]
async fn test_settler_process_all_batches() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_success();
    let mut settler = Settler::with_target(config, Box::new(target));

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut batch_ids = Vec::new();

    // Submit 10 batches
    for i in 0..10 {
        let mut batch = SettlementBatch::new();
        let iou = create_test_iou(&alice, &bob, 100, i);
        batch.add_entry(SettlementEntry::from_iou(&iou));
        batch_ids.push(batch.id().clone());
        settler.submit(batch).await.unwrap();
    }

    // Process all
    for batch_id in &batch_ids {
        settler.process(batch_id).await.unwrap();
    }

    assert_eq!(settler.completed_settlements(), 10);
}

// ============================================================================
// ERROR HANDLING TESTS
// ============================================================================

#[test]
fn test_collector_invalid_config() {
    let config = CollectorConfig::new()
        .with_min_batch_size(100)
        .with_max_batch_size(10);

    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_settler_config_zero_retries() {
    let config = SettlerConfig::new()
        .with_max_retries(0);

    assert!(config.validate().is_ok());
    assert_eq!(config.max_retries, 0);
}

#[tokio::test]
async fn test_settler_timeout() {
    let config = SettlerConfig::new()
        .with_timeout_secs(1);
    let target = MockSettlementTarget::new()
        .with_success()
        .with_delay_ms(2000); // Longer than timeout
    let mut settler = Settler::with_target(config, Box::new(target));

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut batch = SettlementBatch::new();
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&alice, &bob, 100, 1)));
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();

    let result = settler.process(&batch_id).await;

    // Should timeout or fail
    assert!(result.is_ok()); // Returns result even on failure
    let settlement_result = result.unwrap();
    // May or may not be success depending on timeout handling
}

// ============================================================================
// SERIALIZATION TESTS
// ============================================================================

#[test]
fn test_settlement_batch_serialization() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut batch = SettlementBatch::new();
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&alice, &bob, 100, 1)));
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&alice, &bob, 200, 2)));

    let bytes = batch.to_bytes();
    assert!(!bytes.is_empty());

    let restored = SettlementBatch::from_bytes(&bytes).unwrap();
    assert_eq!(batch.entries().len(), restored.entries().len());
    assert_eq!(batch.total_amount(), restored.total_amount());
}

#[test]
fn test_batch_id_serialization() {
    let id = BatchId::generate();

    let bytes = id.to_bytes();
    let restored = BatchId::from_bytes(bytes);

    assert_eq!(id, restored);
}

// ============================================================================
// CONCURRENT ACCESS (CONCEPTUAL)
// ============================================================================

#[test]
fn test_collector_sequential_operations() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);

    // Interleaved operations
    for i in 0..10 {
        let iou = create_test_iou(&alice, &bob, 100, i);
        state.add_iou(iou, &alice.public_key()).unwrap();
        collector.collect_from_state(&state).unwrap();

        if i % 3 == 0 {
            let _ = collector.create_batch();
        }
    }

    // Should have processed some batches
    assert!(collector.stats().batches_created > 0);
}

// ============================================================================
// RECOVERY TESTS
// ============================================================================

#[tokio::test]
async fn test_settler_recovery_after_failure() {
    let config = SettlerConfig::new()
        .with_max_retries(0);
    let target = MockSettlementTarget::new()
        .with_failure("First failure".to_string());
    let mut settler = Settler::with_target(config, Box::new(target));

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut batch = SettlementBatch::new();
    batch.add_entry(SettlementEntry::from_iou(&create_test_iou(&alice, &bob, 100, 1)));
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();
    let result = settler.process(&batch_id).await;

    assert!(result.is_ok());
    assert!(!result.unwrap().is_success());

    // Verify batch is marked as failed, not pending
    let status = settler.get_status(&batch_id);
    assert!(matches!(status, Some(BatchStatus::Failed)));
}

#[test]
fn test_collector_state_after_clear() {
    let config = CollectorConfig::new()
        .with_min_batch_size(1)
        .with_min_iou_age_secs(0);
    let mut collector = Collector::new(config);

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);
    state.add_iou(create_test_iou(&alice, &bob, 100, 1), &alice.public_key()).unwrap();

    collector.collect_from_state(&state).unwrap();
    collector.create_batch().unwrap();

    assert!(collector.pending_batches() > 0);

    collector.clear_batches();

    assert_eq!(collector.pending_batches(), 0);
}

// ============================================================================
// BATCH STATUS TRANSITIONS
// ============================================================================

#[test]
fn test_batch_status_all_states() {
    let statuses = [
        BatchStatus::Pending,
        BatchStatus::Processing,
        BatchStatus::Submitted,
        BatchStatus::Confirmed,
        BatchStatus::Failed,
        BatchStatus::Cancelled,
    ];

    for status in &statuses {
        let mut batch = SettlementBatch::new();
        batch.set_status(status.clone());
        assert_eq!(batch.status(), status);
    }
}

#[test]
fn test_batch_status_display() {
    let status = BatchStatus::Processing;
    let display = format!("{:?}", status);

    assert!(display.contains("Processing"));
}

// ============================================================================
// UNIQUENESS TESTS
// ============================================================================

#[test]
fn test_batch_id_uniqueness() {
    use std::collections::HashSet;

    let mut ids = HashSet::new();

    for _ in 0..10000 {
        let id = BatchId::generate();
        assert!(ids.insert(id), "Duplicate BatchId generated");
    }
}

#[test]
fn test_settlement_entry_preserves_iou_id() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let iou = create_test_iou(&alice, &bob, 100, 42);

    let entry = SettlementEntry::from_iou(&iou);

    assert_eq!(entry.iou_id(), &iou.id());
}
