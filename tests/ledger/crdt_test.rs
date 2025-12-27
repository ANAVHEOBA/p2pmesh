// CRDT (Conflict-free Replicated Data Type) Tests
// Tests for G-Set (Grow-only Set) implementation

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::{IOUBuilder, SignedIOU};
use p2pmesh::ledger::{GSet, IOUEntry, MergeResult};

// ============================================================================
// G-SET BASIC OPERATIONS
// ============================================================================

#[test]
fn test_gset_new_is_empty() {
    let gset: GSet<String> = GSet::new();
    assert!(gset.is_empty());
    assert_eq!(gset.len(), 0);
}

#[test]
fn test_gset_insert() {
    let mut gset: GSet<String> = GSet::new();
    gset.insert("hello".to_string());

    assert!(!gset.is_empty());
    assert_eq!(gset.len(), 1);
    assert!(gset.contains(&"hello".to_string()));
}

#[test]
fn test_gset_insert_duplicate_no_change() {
    let mut gset: GSet<String> = GSet::new();
    gset.insert("hello".to_string());
    gset.insert("hello".to_string());

    assert_eq!(gset.len(), 1);
}

#[test]
fn test_gset_insert_multiple() {
    let mut gset: GSet<String> = GSet::new();
    gset.insert("a".to_string());
    gset.insert("b".to_string());
    gset.insert("c".to_string());

    assert_eq!(gset.len(), 3);
    assert!(gset.contains(&"a".to_string()));
    assert!(gset.contains(&"b".to_string()));
    assert!(gset.contains(&"c".to_string()));
}

#[test]
fn test_gset_does_not_contain_missing() {
    let gset: GSet<String> = GSet::new();
    assert!(!gset.contains(&"missing".to_string()));
}

#[test]
fn test_gset_iter() {
    let mut gset: GSet<i32> = GSet::new();
    gset.insert(1);
    gset.insert(2);
    gset.insert(3);

    let collected: Vec<i32> = gset.iter().cloned().collect();
    assert_eq!(collected.len(), 3);
}

// ============================================================================
// G-SET MERGE OPERATIONS (THE KEY CRDT PROPERTY)
// ============================================================================

#[test]
fn test_gset_merge_disjoint() {
    let mut gset1: GSet<String> = GSet::new();
    gset1.insert("a".to_string());
    gset1.insert("b".to_string());

    let mut gset2: GSet<String> = GSet::new();
    gset2.insert("c".to_string());
    gset2.insert("d".to_string());

    gset1.merge(&gset2);

    assert_eq!(gset1.len(), 4);
    assert!(gset1.contains(&"a".to_string()));
    assert!(gset1.contains(&"b".to_string()));
    assert!(gset1.contains(&"c".to_string()));
    assert!(gset1.contains(&"d".to_string()));
}

#[test]
fn test_gset_merge_overlapping() {
    let mut gset1: GSet<String> = GSet::new();
    gset1.insert("a".to_string());
    gset1.insert("b".to_string());

    let mut gset2: GSet<String> = GSet::new();
    gset2.insert("b".to_string());
    gset2.insert("c".to_string());

    gset1.merge(&gset2);

    // Should have a, b, c (no duplicates)
    assert_eq!(gset1.len(), 3);
}

#[test]
fn test_gset_merge_identical() {
    let mut gset1: GSet<String> = GSet::new();
    gset1.insert("a".to_string());
    gset1.insert("b".to_string());

    let gset2 = gset1.clone();

    gset1.merge(&gset2);

    assert_eq!(gset1.len(), 2);
}

#[test]
fn test_gset_merge_empty_into_full() {
    let mut gset1: GSet<String> = GSet::new();
    gset1.insert("a".to_string());

    let gset2: GSet<String> = GSet::new();

    gset1.merge(&gset2);

    assert_eq!(gset1.len(), 1);
}

#[test]
fn test_gset_merge_full_into_empty() {
    let mut gset1: GSet<String> = GSet::new();

    let mut gset2: GSet<String> = GSet::new();
    gset2.insert("a".to_string());

    gset1.merge(&gset2);

    assert_eq!(gset1.len(), 1);
    assert!(gset1.contains(&"a".to_string()));
}

#[test]
fn test_gset_merge_is_commutative() {
    let mut gset1: GSet<String> = GSet::new();
    gset1.insert("a".to_string());

    let mut gset2: GSet<String> = GSet::new();
    gset2.insert("b".to_string());

    // Merge 1 into 2
    let mut result1 = gset1.clone();
    result1.merge(&gset2);

    // Merge 2 into 1
    let mut result2 = gset2.clone();
    result2.merge(&gset1);

    // Results should be identical
    assert_eq!(result1.len(), result2.len());
    for item in result1.iter() {
        assert!(result2.contains(item));
    }
}

#[test]
fn test_gset_merge_is_associative() {
    let mut gset1: GSet<String> = GSet::new();
    gset1.insert("a".to_string());

    let mut gset2: GSet<String> = GSet::new();
    gset2.insert("b".to_string());

    let mut gset3: GSet<String> = GSet::new();
    gset3.insert("c".to_string());

    // (1 merge 2) merge 3
    let mut result1 = gset1.clone();
    result1.merge(&gset2);
    result1.merge(&gset3);

    // 1 merge (2 merge 3)
    let mut merged23 = gset2.clone();
    merged23.merge(&gset3);
    let mut result2 = gset1.clone();
    result2.merge(&merged23);

    // Results should be identical
    assert_eq!(result1.len(), result2.len());
    for item in result1.iter() {
        assert!(result2.contains(item));
    }
}

#[test]
fn test_gset_merge_is_idempotent() {
    let mut gset1: GSet<String> = GSet::new();
    gset1.insert("a".to_string());
    gset1.insert("b".to_string());

    let original_len = gset1.len();

    // Merge with self multiple times
    let clone = gset1.clone();
    gset1.merge(&clone);
    gset1.merge(&clone);
    gset1.merge(&clone);

    assert_eq!(gset1.len(), original_len);
}

// ============================================================================
// IOU ENTRY G-SET
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

#[test]
fn test_iou_entry_gset_insert() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    let entry = IOUEntry::new(iou.clone(), alice.public_key());

    let mut gset: GSet<IOUEntry> = GSet::new();
    gset.insert(entry);

    assert_eq!(gset.len(), 1);
}

#[test]
fn test_iou_entry_gset_duplicate_detection() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    let entry1 = IOUEntry::new(iou.clone(), alice.public_key());
    let entry2 = IOUEntry::new(iou.clone(), alice.public_key());

    let mut gset: GSet<IOUEntry> = GSet::new();
    gset.insert(entry1);
    gset.insert(entry2);

    // Should only have one entry (same IOU)
    assert_eq!(gset.len(), 1);
}

#[test]
fn test_iou_entry_gset_different_ious() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let iou2 = create_test_iou(&alice, &bob, 100, 2); // Different nonce

    let entry1 = IOUEntry::new(iou1, alice.public_key());
    let entry2 = IOUEntry::new(iou2, alice.public_key());

    let mut gset: GSet<IOUEntry> = GSet::new();
    gset.insert(entry1);
    gset.insert(entry2);

    assert_eq!(gset.len(), 2);
}

#[test]
fn test_iou_entry_gset_merge_from_different_nodes() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    // Node 1 has Alice -> Bob payment
    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let entry1 = IOUEntry::new(iou1, alice.public_key());
    let mut node1_gset: GSet<IOUEntry> = GSet::new();
    node1_gset.insert(entry1);

    // Node 2 has Bob -> Charlie payment
    let iou2 = create_test_iou(&bob, &charlie, 50, 1);
    let entry2 = IOUEntry::new(iou2, bob.public_key());
    let mut node2_gset: GSet<IOUEntry> = GSet::new();
    node2_gset.insert(entry2);

    // Merge
    node1_gset.merge(&node2_gset);

    assert_eq!(node1_gset.len(), 2);
}

// ============================================================================
// MERGE RESULT TRACKING
// ============================================================================

#[test]
fn test_merge_result_reports_new_entries() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let iou2 = create_test_iou(&alice, &bob, 100, 2);

    let entry1 = IOUEntry::new(iou1, alice.public_key());
    let entry2 = IOUEntry::new(iou2, alice.public_key());

    let mut gset1: GSet<IOUEntry> = GSet::new();
    gset1.insert(entry1);

    let mut gset2: GSet<IOUEntry> = GSet::new();
    gset2.insert(entry2);

    let result = gset1.merge_with_result(&gset2);

    assert_eq!(result.new_entries, 1);
    assert_eq!(result.total_after_merge, 2);
}

#[test]
fn test_merge_result_no_new_entries() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    let entry = IOUEntry::new(iou, alice.public_key());

    let mut gset1: GSet<IOUEntry> = GSet::new();
    gset1.insert(entry.clone());

    let mut gset2: GSet<IOUEntry> = GSet::new();
    gset2.insert(entry);

    let result = gset1.merge_with_result(&gset2);

    assert_eq!(result.new_entries, 0);
    assert_eq!(result.total_after_merge, 1);
}

// ============================================================================
// G-SET SERIALIZATION
// ============================================================================

#[test]
fn test_gset_serialization_roundtrip() {
    let mut gset: GSet<String> = GSet::new();
    gset.insert("a".to_string());
    gset.insert("b".to_string());
    gset.insert("c".to_string());

    // Serialize
    let bytes = gset.to_bytes();

    // Deserialize
    let restored: GSet<String> = GSet::from_bytes(&bytes).unwrap();

    assert_eq!(restored.len(), 3);
    assert!(restored.contains(&"a".to_string()));
    assert!(restored.contains(&"b".to_string()));
    assert!(restored.contains(&"c".to_string()));
}

#[test]
fn test_gset_empty_serialization() {
    let gset: GSet<String> = GSet::new();
    let bytes = gset.to_bytes();
    let restored: GSet<String> = GSet::from_bytes(&bytes).unwrap();

    assert!(restored.is_empty());
}

// ============================================================================
// G-SET DELTA SYNC (EFFICIENT SYNCHRONIZATION)
// ============================================================================

#[test]
fn test_gset_delta() {
    let mut gset1: GSet<String> = GSet::new();
    gset1.insert("a".to_string());
    gset1.insert("b".to_string());

    let mut gset2: GSet<String> = GSet::new();
    gset2.insert("b".to_string());
    gset2.insert("c".to_string());
    gset2.insert("d".to_string());

    // Get what gset2 has that gset1 doesn't
    let delta = gset2.delta(&gset1);

    assert_eq!(delta.len(), 2); // c and d
    assert!(delta.contains(&"c".to_string()));
    assert!(delta.contains(&"d".to_string()));
    assert!(!delta.contains(&"b".to_string())); // Already in gset1
}

#[test]
fn test_gset_delta_empty_when_subset() {
    let mut gset1: GSet<String> = GSet::new();
    gset1.insert("a".to_string());
    gset1.insert("b".to_string());
    gset1.insert("c".to_string());

    let mut gset2: GSet<String> = GSet::new();
    gset2.insert("a".to_string());
    gset2.insert("b".to_string());

    // gset2 is a subset of gset1, so delta should be empty
    let delta = gset2.delta(&gset1);

    assert!(delta.is_empty());
}

// ============================================================================
// VECTOR CLOCK FOR CAUSAL ORDERING (OPTIONAL BUT USEFUL)
// ============================================================================

#[test]
fn test_iou_entry_has_timestamp() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    let entry = IOUEntry::new(iou, alice.public_key());

    assert!(entry.received_at() > 0);
}

#[test]
fn test_iou_entry_ordering_by_timestamp() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let entry1 = IOUEntry::new(iou1, alice.public_key());

    // Small delay to ensure different timestamp
    std::thread::sleep(std::time::Duration::from_millis(10));

    let iou2 = create_test_iou(&alice, &bob, 100, 2);
    let entry2 = IOUEntry::new(iou2, alice.public_key());

    assert!(entry2.received_at() >= entry1.received_at());
}
