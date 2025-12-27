// Conflict Detection Tests
// Tests for detecting double-spends in the distributed mesh

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::IOUBuilder;
use p2pmesh::ledger::{
    ConflictDetector, ConflictError, ConflictType, MeshState, NodeId,
    SpendingClaim, ConflictResolution,
};
use p2pmesh::vault::{Vault, UTXOId};

// ============================================================================
// SPENDING CLAIM BASICS
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
fn test_spending_claim_creation() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    let utxo_id = UTXOId::from_bytes([1u8; 32]);

    let claim = SpendingClaim::new(
        utxo_id.clone(),
        iou.id(),
        alice.public_key(),
    );

    assert_eq!(claim.utxo_id(), &utxo_id);
    assert_eq!(claim.spending_iou_id(), &iou.id());
    assert!(claim.timestamp() > 0);
}

#[test]
fn test_spending_claim_with_witnesses() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let witness1 = Keypair::generate();
    let witness2 = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    let utxo_id = UTXOId::from_bytes([1u8; 32]);

    let mut claim = SpendingClaim::new(
        utxo_id,
        iou.id(),
        alice.public_key(),
    );

    // Add witnesses
    claim.add_witness(NodeId::from_public_key(&witness1.public_key()));
    claim.add_witness(NodeId::from_public_key(&witness2.public_key()));

    assert_eq!(claim.witness_count(), 2);
}

// ============================================================================
// CONFLICT DETECTOR BASICS
// ============================================================================

#[test]
fn test_conflict_detector_new() {
    let detector = ConflictDetector::new();
    assert_eq!(detector.claim_count(), 0);
    assert_eq!(detector.conflict_count(), 0);
}

#[test]
fn test_register_spending_claim() {
    let mut detector = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    let utxo_id = UTXOId::from_bytes([1u8; 32]);

    let claim = SpendingClaim::new(
        utxo_id,
        iou.id(),
        alice.public_key(),
    );

    detector.register_claim(claim).unwrap();

    assert_eq!(detector.claim_count(), 1);
}

#[test]
fn test_register_same_claim_twice_ok() {
    let mut detector = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let iou = create_test_iou(&alice, &bob, 100, 1);
    let utxo_id = UTXOId::from_bytes([1u8; 32]);

    let claim = SpendingClaim::new(
        utxo_id.clone(),
        iou.id(),
        alice.public_key(),
    );

    detector.register_claim(claim.clone()).unwrap();

    // Same claim again should be ok (idempotent)
    detector.register_claim(claim).unwrap();

    assert_eq!(detector.claim_count(), 1);
    assert_eq!(detector.conflict_count(), 0);
}

// ============================================================================
// DOUBLE-SPEND DETECTION
// ============================================================================

#[test]
fn test_detect_double_spend() {
    let mut detector = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    // Same UTXO being spent
    let utxo_id = UTXOId::from_bytes([1u8; 32]);

    // First spend: Alice -> Bob
    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let claim1 = SpendingClaim::new(
        utxo_id.clone(),
        iou1.id(),
        alice.public_key(),
    );
    detector.register_claim(claim1).unwrap();

    // Second spend of SAME UTXO: Alice -> Charlie
    let iou2 = create_test_iou(&alice, &charlie, 100, 2);
    let claim2 = SpendingClaim::new(
        utxo_id.clone(),
        iou2.id(),
        alice.public_key(),
    );

    let result = detector.register_claim(claim2);

    // Should detect conflict
    assert!(matches!(result, Err(ConflictError::DoubleSpend { .. })));
    assert_eq!(detector.conflict_count(), 1);
}

#[test]
fn test_different_utxos_no_conflict() {
    let mut detector = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    // Different UTXOs
    let utxo_id1 = UTXOId::from_bytes([1u8; 32]);
    let utxo_id2 = UTXOId::from_bytes([2u8; 32]);

    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let iou2 = create_test_iou(&alice, &bob, 100, 2);

    let claim1 = SpendingClaim::new(utxo_id1, iou1.id(), alice.public_key());
    let claim2 = SpendingClaim::new(utxo_id2, iou2.id(), alice.public_key());

    detector.register_claim(claim1).unwrap();
    detector.register_claim(claim2).unwrap();

    assert_eq!(detector.claim_count(), 2);
    assert_eq!(detector.conflict_count(), 0);
}

#[test]
fn test_get_conflicting_claims() {
    let mut detector = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let utxo_id = UTXOId::from_bytes([1u8; 32]);

    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let iou2 = create_test_iou(&alice, &charlie, 100, 2);

    let claim1 = SpendingClaim::new(utxo_id.clone(), iou1.id(), alice.public_key());
    let claim2 = SpendingClaim::new(utxo_id.clone(), iou2.id(), alice.public_key());

    detector.register_claim(claim1).unwrap();
    let _ = detector.register_claim(claim2); // Will fail but record conflict

    let conflicts = detector.get_conflicts_for_utxo(&utxo_id);

    assert_eq!(conflicts.len(), 2);
}

// ============================================================================
// CONFLICT RESOLUTION
// ============================================================================

#[test]
fn test_resolve_by_first_seen() {
    let mut detector = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let utxo_id = UTXOId::from_bytes([1u8; 32]);

    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let iou2 = create_test_iou(&alice, &charlie, 100, 2);

    let claim1 = SpendingClaim::new(utxo_id.clone(), iou1.id(), alice.public_key());

    // Small delay to ensure different timestamps
    std::thread::sleep(std::time::Duration::from_millis(10));

    let claim2 = SpendingClaim::new(utxo_id.clone(), iou2.id(), alice.public_key());

    detector.register_claim(claim1.clone()).unwrap();
    let _ = detector.register_claim(claim2);

    // Resolve using first-seen rule
    let winner = detector.resolve_conflict(&utxo_id, ConflictResolution::FirstSeen).unwrap();

    assert_eq!(winner.spending_iou_id(), claim1.spending_iou_id());
}

#[test]
fn test_resolve_by_witness_count() {
    let mut detector = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();
    let witness1 = Keypair::generate();
    let witness2 = Keypair::generate();
    let witness3 = Keypair::generate();

    let utxo_id = UTXOId::from_bytes([1u8; 32]);

    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let iou2 = create_test_iou(&alice, &charlie, 100, 2);

    // Claim 1 has 1 witness
    let mut claim1 = SpendingClaim::new(utxo_id.clone(), iou1.id(), alice.public_key());
    claim1.add_witness(NodeId::from_public_key(&witness1.public_key()));

    // Claim 2 has 3 witnesses (more support)
    let mut claim2 = SpendingClaim::new(utxo_id.clone(), iou2.id(), alice.public_key());
    claim2.add_witness(NodeId::from_public_key(&witness1.public_key()));
    claim2.add_witness(NodeId::from_public_key(&witness2.public_key()));
    claim2.add_witness(NodeId::from_public_key(&witness3.public_key()));

    detector.register_claim(claim1).unwrap();
    let _ = detector.register_claim(claim2.clone());

    // Resolve using most witnesses rule
    let winner = detector.resolve_conflict(&utxo_id, ConflictResolution::MostWitnesses).unwrap();

    assert_eq!(winner.spending_iou_id(), claim2.spending_iou_id());
}

// ============================================================================
// INTEGRATION WITH MESH STATE
// ============================================================================

#[test]
fn test_validate_iou_against_detector() {
    let node_id = NodeId::generate();
    let mut state = MeshState::new(node_id);
    let mut detector = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    // Give Alice some funds (simulate receiving)
    let funding = create_test_iou(&charlie, &alice, 1000, 1);
    state.add_iou(funding.clone(), &charlie.public_key()).unwrap();

    // Create UTXO ID from the funding IOU
    let utxo_id = UTXOId::from_iou(&funding.id());

    // Alice pays Bob
    let payment1 = create_test_iou(&alice, &bob, 500, 1);
    state.add_iou(payment1.clone(), &alice.public_key()).unwrap();

    // Register the spending claim
    let claim1 = SpendingClaim::new(utxo_id.clone(), payment1.id(), alice.public_key());
    detector.register_claim(claim1).unwrap();

    // Alice tries to double-spend to someone else
    let payment2 = create_test_iou(&alice, &charlie, 500, 2);

    // This should be caught before adding to state
    let claim2 = SpendingClaim::new(utxo_id, payment2.id(), alice.public_key());
    let result = detector.register_claim(claim2);

    assert!(matches!(result, Err(ConflictError::DoubleSpend { .. })));
}

#[test]
fn test_merge_detectors_from_different_nodes() {
    let mut detector1 = ConflictDetector::new();
    let mut detector2 = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let utxo_id1 = UTXOId::from_bytes([1u8; 32]);
    let utxo_id2 = UTXOId::from_bytes([2u8; 32]);

    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let iou2 = create_test_iou(&alice, &bob, 200, 2);

    // Node 1 sees claim 1
    let claim1 = SpendingClaim::new(utxo_id1, iou1.id(), alice.public_key());
    detector1.register_claim(claim1).unwrap();

    // Node 2 sees claim 2
    let claim2 = SpendingClaim::new(utxo_id2, iou2.id(), alice.public_key());
    detector2.register_claim(claim2).unwrap();

    // Merge
    let result = detector1.merge(&detector2);

    assert_eq!(result.new_claims, 1);
    assert_eq!(detector1.claim_count(), 2);
}

#[test]
fn test_merge_detectors_finds_conflict() {
    let mut detector1 = ConflictDetector::new();
    let mut detector2 = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    // Same UTXO
    let utxo_id = UTXOId::from_bytes([1u8; 32]);

    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let iou2 = create_test_iou(&alice, &charlie, 100, 2);

    // Node 1 sees Alice -> Bob
    let claim1 = SpendingClaim::new(utxo_id.clone(), iou1.id(), alice.public_key());
    detector1.register_claim(claim1).unwrap();

    // Node 2 sees Alice -> Charlie (same UTXO!)
    let claim2 = SpendingClaim::new(utxo_id.clone(), iou2.id(), alice.public_key());
    detector2.register_claim(claim2).unwrap();

    // Merge reveals conflict
    let result = detector1.merge(&detector2);

    assert_eq!(result.conflicts_detected, 1);
    assert_eq!(detector1.conflict_count(), 1);
}

// ============================================================================
// CONFLICT TYPES
// ============================================================================

#[test]
fn test_conflict_type_double_spend() {
    let mut detector = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let utxo_id = UTXOId::from_bytes([1u8; 32]);

    let iou1 = create_test_iou(&alice, &bob, 100, 1);
    let iou2 = create_test_iou(&alice, &charlie, 100, 2);

    let claim1 = SpendingClaim::new(utxo_id.clone(), iou1.id(), alice.public_key());
    let claim2 = SpendingClaim::new(utxo_id.clone(), iou2.id(), alice.public_key());

    detector.register_claim(claim1).unwrap();
    let result = detector.register_claim(claim2);

    if let Err(ConflictError::DoubleSpend { conflict_type, .. }) = result {
        assert_eq!(conflict_type, ConflictType::SameUtxoDifferentRecipient);
    } else {
        panic!("Expected DoubleSpend error");
    }
}

// ============================================================================
// SERIALIZATION
// ============================================================================

#[test]
fn test_conflict_detector_serialization() {
    let mut detector = ConflictDetector::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let utxo_id = UTXOId::from_bytes([1u8; 32]);
    let iou = create_test_iou(&alice, &bob, 100, 1);

    let claim = SpendingClaim::new(utxo_id, iou.id(), alice.public_key());
    detector.register_claim(claim).unwrap();

    // Serialize
    let bytes = detector.to_bytes();

    // Deserialize
    let restored = ConflictDetector::from_bytes(&bytes).unwrap();

    assert_eq!(restored.claim_count(), 1);
}

#[test]
fn test_spending_claim_serialization() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let utxo_id = UTXOId::from_bytes([1u8; 32]);
    let iou = create_test_iou(&alice, &bob, 100, 1);

    let mut claim = SpendingClaim::new(utxo_id, iou.id(), alice.public_key());
    claim.add_witness(NodeId::generate());

    // Serialize
    let bytes = claim.to_bytes();

    // Deserialize
    let restored = SpendingClaim::from_bytes(&bytes).unwrap();

    assert_eq!(restored.witness_count(), 1);
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn test_empty_detector_merge() {
    let detector1 = ConflictDetector::new();
    let mut detector2 = ConflictDetector::new();

    let result = detector2.merge(&detector1);

    assert_eq!(result.new_claims, 0);
    assert_eq!(result.conflicts_detected, 0);
}

#[test]
fn test_resolve_nonexistent_conflict() {
    let detector = ConflictDetector::new();
    let utxo_id = UTXOId::from_bytes([1u8; 32]);

    let result = detector.resolve_conflict(&utxo_id, ConflictResolution::FirstSeen);

    assert!(result.is_none());
}

#[test]
fn test_witness_deduplication() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let witness = Keypair::generate();

    let utxo_id = UTXOId::from_bytes([1u8; 32]);
    let iou = create_test_iou(&alice, &bob, 100, 1);

    let mut claim = SpendingClaim::new(utxo_id, iou.id(), alice.public_key());

    let witness_id = NodeId::from_public_key(&witness.public_key());
    claim.add_witness(witness_id.clone());
    claim.add_witness(witness_id.clone()); // Duplicate
    claim.add_witness(witness_id); // Another duplicate

    // Should only count as 1
    assert_eq!(claim.witness_count(), 1);
}
