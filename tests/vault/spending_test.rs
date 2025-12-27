// Spending logic and double-spend prevention tests

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::{IOUBuilder, SignedIOU, IOUId};
use p2pmesh::vault::{Vault, VaultError, SpentOutput, SpentOutputSet};

// ============================================================================
// BASIC SPENDING TESTS
// ============================================================================

#[test]
fn test_can_spend_received_funds() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive 100
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Spend 50
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(50)
        .build()
        .unwrap();

    assert!(vault.record_sent_iou(outgoing).is_ok());
    assert_eq!(vault.balance(), 50);
}

#[test]
fn test_cannot_spend_more_than_balance() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive 100
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Try to spend 150
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(150)
        .build()
        .unwrap();

    let result = vault.record_sent_iou(outgoing);
    assert!(matches!(result, Err(VaultError::InsufficientBalance { available: 100, required: 150 })));
}

#[test]
fn test_spending_tracks_consumed_utxos() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive 100
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    let original_utxo_id = vault.utxo_set()[0].id().clone();

    // Spend 50
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(50)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    // Original UTXO should be in spent outputs
    assert!(vault.is_utxo_spent(&original_utxo_id));
}

// ============================================================================
// DOUBLE-SPEND PREVENTION TESTS
// ============================================================================

#[test]
fn test_same_iou_cannot_be_received_twice() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .nonce(1)
        .build()
        .unwrap();

    vault.receive_iou(iou.clone(), &alice.public_key()).unwrap();

    // Try to receive the same IOU again
    let result = vault.receive_iou(iou, &alice.public_key());
    assert!(matches!(result, Err(VaultError::DuplicateTransaction)));
}

#[test]
fn test_spent_utxo_cannot_be_spent_again() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive 100
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Spend all 100
    let outgoing1 = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .nonce(1)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing1).unwrap();

    assert_eq!(vault.balance(), 0);

    // Try to spend again - should fail
    let outgoing2 = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(50)
        .nonce(2)
        .build()
        .unwrap();

    let result = vault.record_sent_iou(outgoing2);
    assert!(matches!(result, Err(VaultError::InsufficientBalance { .. })));
}

#[test]
fn test_double_spend_detection_across_sync() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive 100
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    let incoming_id = incoming.id();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Get the UTXO ID before spending
    let utxo_id = vault.utxo_set()[0].id().clone();

    // Spend to Bob
    let outgoing_bob = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .nonce(1)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing_bob.clone()).unwrap();

    // Now simulate receiving a conflicting spend (same UTXO spent to Charlie)
    let outgoing_charlie = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&charlie.public_key()))
        .amount(100)
        .nonce(2)
        .build()
        .unwrap();

    // Check if this would be a double-spend
    let is_double_spend = vault.would_be_double_spend(&utxo_id);
    assert!(is_double_spend);
}

#[test]
fn test_check_double_spend_by_iou_id() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let iou_id = iou.id();
    vault.receive_iou(iou, &alice.public_key()).unwrap();

    // Check if same IOU ID was already processed
    assert!(vault.has_processed_iou(&iou_id));
}

// ============================================================================
// SPENT OUTPUT TRACKING TESTS
// ============================================================================

#[test]
fn test_spent_output_records_spending_transaction() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    let utxo_id = vault.utxo_set()[0].id().clone();

    // Spend
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();
    let outgoing_id = outgoing.id();
    vault.record_sent_iou(outgoing).unwrap();

    // Check spent output record
    let spent = vault.get_spent_output(&utxo_id);
    assert!(spent.is_some());
    assert_eq!(spent.unwrap().spending_iou_id(), &outgoing_id);
}

#[test]
fn test_spent_output_records_timestamp() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    let utxo_id = vault.utxo_set()[0].id().clone();

    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let spent = vault.get_spent_output(&utxo_id).unwrap();
    assert!(spent.spent_at() >= before);
    assert!(spent.spent_at() <= after);
}

#[test]
fn test_get_all_spent_outputs() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive multiple
    for i in 0..3 {
        let incoming = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(50)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(incoming, &bob.public_key()).unwrap();
    }

    // Spend all
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(150)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    let spent_outputs = vault.spent_outputs();
    assert_eq!(spent_outputs.len(), 3);
}

// ============================================================================
// SPENT OUTPUT SET OPERATIONS
// ============================================================================

#[test]
fn test_spent_output_set_new_is_empty() {
    let set = SpentOutputSet::new();
    assert!(set.is_empty());
    assert_eq!(set.len(), 0);
}

#[test]
fn test_spent_output_set_add() {
    let mut set = SpentOutputSet::new();
    let utxo_id = p2pmesh::vault::UTXOId::from_bytes([1u8; 32]);
    let spending_id = IOUId::from_bytes([2u8; 32]);

    let spent = SpentOutput::new(utxo_id.clone(), spending_id, 12345);
    set.add(spent);

    assert_eq!(set.len(), 1);
    assert!(set.contains(&utxo_id));
}

#[test]
fn test_spent_output_set_duplicate_rejected() {
    let mut set = SpentOutputSet::new();
    let utxo_id = p2pmesh::vault::UTXOId::from_bytes([1u8; 32]);

    let spent1 = SpentOutput::new(utxo_id.clone(), IOUId::from_bytes([2u8; 32]), 12345);
    let spent2 = SpentOutput::new(utxo_id.clone(), IOUId::from_bytes([3u8; 32]), 12346);

    assert!(set.add(spent1).is_ok());
    assert!(set.add(spent2).is_err()); // Already spent
}

#[test]
fn test_spent_output_set_get() {
    let mut set = SpentOutputSet::new();
    let utxo_id = p2pmesh::vault::UTXOId::from_bytes([1u8; 32]);
    let spending_id = IOUId::from_bytes([2u8; 32]);

    let spent = SpentOutput::new(utxo_id.clone(), spending_id.clone(), 12345);
    set.add(spent).unwrap();

    let found = set.get(&utxo_id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().spending_iou_id(), &spending_id);
}

// ============================================================================
// SPENDING VALIDATION TESTS
// ============================================================================

#[test]
fn test_validate_spending_amount_check() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive 100
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Check if we can afford various amounts
    assert!(vault.can_afford(50));
    assert!(vault.can_afford(100));
    assert!(!vault.can_afford(101));
    assert!(!vault.can_afford(200));
}

#[test]
fn test_can_afford_considers_pending() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive 100
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Reserve 30
    vault.reserve_balance(30).unwrap();

    // Can afford 70 but not 80
    assert!(vault.can_afford(70));
    assert!(!vault.can_afford(80));
}

#[test]
fn test_estimate_utxos_needed() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive multiple small amounts
    for i in 0..5 {
        let incoming = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(20)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(incoming, &bob.public_key()).unwrap();
    }

    // Spending 50 should need at least 3 UTXOs (3*20=60)
    let estimate = vault.estimate_utxos_needed(50);
    assert!(estimate.is_some());
    assert!(estimate.unwrap() >= 3);
}

#[test]
fn test_estimate_utxos_needed_insufficient() {
    let alice = Keypair::generate();
    let vault = Vault::new(alice.public_key());

    // No balance
    let estimate = vault.estimate_utxos_needed(100);
    assert!(estimate.is_none());
}

// ============================================================================
// SPENDING WITH SPECIFIC UTXOS
// ============================================================================

#[test]
fn test_spend_specific_utxo() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive two amounts
    let incoming1 = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .nonce(1)
        .build()
        .unwrap();
    let incoming2 = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(200)
        .nonce(2)
        .build()
        .unwrap();

    vault.receive_iou(incoming1, &bob.public_key()).unwrap();
    vault.receive_iou(incoming2, &bob.public_key()).unwrap();

    // Get the 100 UTXO specifically
    let utxos = vault.utxo_set();
    let small_utxo = utxos.iter().find(|u| u.amount() == 100).unwrap();
    let small_utxo_id = small_utxo.id().clone();

    // Spend using that specific UTXO
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    vault.spend_with_utxos(outgoing, vec![small_utxo_id.clone()]).unwrap();

    // The 100 UTXO should be spent, 200 should remain
    assert!(vault.is_utxo_spent(&small_utxo_id));
    assert_eq!(vault.balance(), 200);
}

#[test]
fn test_spend_specific_utxo_insufficient_fails() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive 100
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    let utxo_id = vault.utxo_set()[0].id().clone();

    // Try to spend 150 with only 100 UTXO
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(150)
        .build()
        .unwrap();

    let result = vault.spend_with_utxos(outgoing, vec![utxo_id]);
    assert!(matches!(result, Err(VaultError::InsufficientUTXOs { .. })));
}

#[test]
fn test_spend_nonexistent_utxo_fails() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive 100
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    let fake_utxo_id = p2pmesh::vault::UTXOId::from_bytes([99u8; 32]);

    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(50)
        .build()
        .unwrap();

    let result = vault.spend_with_utxos(outgoing, vec![fake_utxo_id]);
    assert!(matches!(result, Err(VaultError::UTXONotFound)));
}

// ============================================================================
// CONCURRENT SPENDING PREVENTION
// ============================================================================

#[test]
fn test_concurrent_spend_attempt_fails() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive 100
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Start preparing two payments that would both use the 100 UTXO
    let utxo_id = vault.utxo_set()[0].id().clone();

    // Lock for first payment
    vault.lock_utxo(&utxo_id).unwrap();

    // Second payment attempt should see no available balance
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&charlie.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let result = vault.record_sent_iou(outgoing);
    assert!(matches!(result, Err(VaultError::InsufficientBalance { .. })));
}
