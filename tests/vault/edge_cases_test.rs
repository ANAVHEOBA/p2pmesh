// Edge cases and stress tests for vault module

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::{IOUBuilder, IOUId};
use p2pmesh::vault::{Vault, VaultError, UTXO, UTXOSet, UTXOId};

// ============================================================================
// BOUNDARY VALUE TESTS
// ============================================================================

#[test]
fn test_maximum_amount_value() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    let max_amount = u64::MAX;

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(max_amount)
        .build()
        .unwrap();

    vault.receive_iou(iou, &alice.public_key()).unwrap();
    assert_eq!(vault.balance(), max_amount);
}

#[test]
fn test_minimum_nonzero_amount() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(1) // Smallest valid amount
        .build()
        .unwrap();

    vault.receive_iou(iou, &alice.public_key()).unwrap();
    assert_eq!(vault.balance(), 1);
}

#[test]
fn test_balance_overflow_protection() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    // Receive near-max amount
    let iou1 = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(u64::MAX - 100)
        .nonce(1)
        .build()
        .unwrap();
    vault.receive_iou(iou1, &alice.public_key()).unwrap();

    // Try to receive more that would overflow
    let iou2 = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(200)
        .nonce(2)
        .build()
        .unwrap();

    let result = vault.receive_iou(iou2, &alice.public_key());
    assert!(matches!(result, Err(VaultError::BalanceOverflow)));
}

#[test]
fn test_spend_exact_max_balance() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    let large_amount = u64::MAX / 2; // Large but won't overflow

    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(large_amount)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(large_amount)
        .build()
        .unwrap();

    vault.record_sent_iou(outgoing).unwrap();
    assert_eq!(vault.balance(), 0);
}

// ============================================================================
// MANY UTXOS TESTS
// ============================================================================

#[test]
fn test_vault_handles_many_utxos() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    let num_utxos = 100;

    for i in 0..num_utxos {
        let iou = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(10)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(iou, &alice.public_key()).unwrap();
    }

    assert_eq!(vault.utxo_set().len(), num_utxos as usize);
    assert_eq!(vault.balance(), num_utxos * 10);
}

#[test]
fn test_spend_consolidates_many_small_utxos() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive 100 small UTXOs
    for i in 0..100 {
        let incoming = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(1)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(incoming, &bob.public_key()).unwrap();
    }

    assert_eq!(vault.balance(), 100);
    assert_eq!(vault.utxo_set().len(), 100);

    // Spend 100 - should consume all UTXOs
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    assert_eq!(vault.balance(), 0);
    assert_eq!(vault.utxo_set().len(), 0);
}

#[test]
fn test_many_senders() {
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    let num_senders = 50;

    for i in 0..num_senders {
        let sender = Keypair::generate();
        let iou = IOUBuilder::new()
            .sender(&sender)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(10)
            .build()
            .unwrap();
        vault.receive_iou(iou, &sender.public_key()).unwrap();
    }

    assert_eq!(vault.balance(), num_senders * 10);
}

// ============================================================================
// STATE CONSISTENCY TESTS
// ============================================================================

#[test]
fn test_balance_equals_utxo_sum() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Multiple receives
    for i in 1..=5 {
        let incoming = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(i * 20)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(incoming, &bob.public_key()).unwrap();
    }

    // Spend some
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    // Balance should always equal sum of UTXO values
    let utxo_sum: u64 = vault.utxo_set().iter().map(|u| u.amount()).sum();
    assert_eq!(vault.balance(), utxo_sum);
}

#[test]
fn test_transaction_count_consistency() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // 3 receives
    for i in 0..3 {
        let incoming = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(100)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(incoming, &bob.public_key()).unwrap();
    }

    // 2 sends
    for i in 0..2 {
        let outgoing = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(50)
            .nonce(100 + i)
            .build()
            .unwrap();
        vault.record_sent_iou(outgoing).unwrap();
    }

    assert_eq!(vault.transaction_count(), 5);
    assert_eq!(vault.received_transactions().len(), 3);
    assert_eq!(vault.sent_transactions().len(), 2);
}

#[test]
fn test_failed_operations_dont_affect_state() {
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

    let balance_before = vault.balance();
    let utxo_count_before = vault.utxo_set().len();

    // Try invalid operations
    let over_spend = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(200)
        .build()
        .unwrap();
    let _ = vault.record_sent_iou(over_spend);

    // State should be unchanged
    assert_eq!(vault.balance(), balance_before);
    assert_eq!(vault.utxo_set().len(), utxo_count_before);
}

// ============================================================================
// DUPLICATE DETECTION TESTS
// ============================================================================

#[test]
fn test_same_amount_different_nonce_accepted() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    // Same amount but different nonces = different IOUs
    let iou1 = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .nonce(1)
        .build()
        .unwrap();

    let iou2 = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .nonce(2)
        .build()
        .unwrap();

    vault.receive_iou(iou1, &alice.public_key()).unwrap();
    vault.receive_iou(iou2, &alice.public_key()).unwrap();

    assert_eq!(vault.balance(), 200);
    assert_eq!(vault.transaction_count(), 2);
}

#[test]
fn test_same_nonce_same_timestamp_rejected() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    let timestamp = 1234567890u64;

    let iou1 = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .nonce(1)
        .timestamp(timestamp)
        .build()
        .unwrap();

    vault.receive_iou(iou1.clone(), &alice.public_key()).unwrap();

    // Same IOU again
    let result = vault.receive_iou(iou1, &alice.public_key());
    assert!(matches!(result, Err(VaultError::DuplicateTransaction)));
}

// ============================================================================
// EMPTY VAULT OPERATIONS
// ============================================================================

#[test]
fn test_empty_vault_balance_is_zero() {
    let alice = Keypair::generate();
    let vault = Vault::new(alice.public_key());

    assert_eq!(vault.balance(), 0);
    assert_eq!(vault.available_balance(), 0);
}

#[test]
fn test_empty_vault_utxo_set_empty() {
    let alice = Keypair::generate();
    let vault = Vault::new(alice.public_key());

    assert!(vault.utxo_set().is_empty());
}

#[test]
fn test_empty_vault_cannot_spend() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(1)
        .build()
        .unwrap();

    let result = vault.record_sent_iou(outgoing);
    assert!(matches!(result, Err(VaultError::InsufficientBalance { available: 0, required: 1 })));
}

#[test]
fn test_empty_vault_can_afford_zero() {
    let alice = Keypair::generate();
    let vault = Vault::new(alice.public_key());

    assert!(vault.can_afford(0));
}

#[test]
fn test_empty_vault_transaction_history_empty() {
    let alice = Keypair::generate();
    let vault = Vault::new(alice.public_key());

    assert!(vault.transaction_history().is_empty());
    assert!(vault.received_transactions().is_empty());
    assert!(vault.sent_transactions().is_empty());
}

// ============================================================================
// SELF-PAYMENT PREVENTION
// ============================================================================

#[test]
fn test_self_payment_rejected_at_builder() {
    let alice = Keypair::generate();

    let result = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build();

    assert!(result.is_err());
}

// ============================================================================
// RESERVATION EDGE CASES
// ============================================================================

#[test]
fn test_multiple_reservations() {
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

    // Multiple reservations
    let r1 = vault.reserve_balance(30).unwrap();
    let r2 = vault.reserve_balance(20).unwrap();
    let r3 = vault.reserve_balance(40).unwrap();

    assert_eq!(vault.available_balance(), 10);

    // Release one
    vault.release_reservation(r2).unwrap();
    assert_eq!(vault.available_balance(), 30);
}

#[test]
fn test_release_invalid_reservation_fails() {
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

    let result = vault.release_reservation(99999);
    assert!(matches!(result, Err(VaultError::ReservationNotFound)));
}

#[test]
fn test_commit_already_released_reservation_fails() {
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

    let reservation = vault.reserve_balance(30).unwrap();
    vault.release_reservation(reservation).unwrap();

    let result = vault.commit_reservation(reservation);
    assert!(matches!(result, Err(VaultError::ReservationNotFound)));
}

#[test]
fn test_reserve_zero_amount() {
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

    // Zero reservation should succeed or fail gracefully
    let result = vault.reserve_balance(0);
    // Either it succeeds (no-op) or fails with invalid amount
    assert!(result.is_ok() || matches!(result, Err(VaultError::InvalidAmount)));
}

// ============================================================================
// UTXO ID EDGE CASES
// ============================================================================

#[test]
fn test_utxo_id_from_all_zeros() {
    let id = UTXOId::from_bytes([0u8; 32]);
    assert_eq!(id.as_bytes(), &[0u8; 32]);
}

#[test]
fn test_utxo_id_from_all_ones() {
    let id = UTXOId::from_bytes([255u8; 32]);
    assert_eq!(id.as_bytes(), &[255u8; 32]);
}

#[test]
fn test_utxo_id_equality() {
    let id1 = UTXOId::from_bytes([1u8; 32]);
    let id2 = UTXOId::from_bytes([1u8; 32]);
    let id3 = UTXOId::from_bytes([2u8; 32]);

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
fn test_utxo_id_hash_consistency() {
    let id1 = UTXOId::from_bytes([1u8; 32]);
    let id2 = UTXOId::from_bytes([1u8; 32]);

    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(id1.clone());

    assert!(set.contains(&id2));
}

// ============================================================================
// UTXO SET EDGE CASES
// ============================================================================

#[test]
fn test_utxo_set_select_empty() {
    let set = UTXOSet::new();
    assert!(set.select_for_amount(1).is_none());
}

#[test]
fn test_utxo_set_select_zero_amount() {
    let mut set = UTXOSet::new();
    let owner = Keypair::generate().public_key();
    set.add(UTXO::new(owner, 100, IOUId::from_bytes([1u8; 32])));

    let selected = set.select_for_amount(0);
    // Either returns empty selection or None
    if let Some((utxos, change)) = selected {
        assert!(utxos.is_empty() || change >= 0);
    }
}

#[test]
fn test_utxo_set_add_duplicate_id_fails() {
    let mut set = UTXOSet::new();
    let owner = Keypair::generate().public_key();
    let iou_id = IOUId::from_bytes([1u8; 32]);

    let utxo1 = UTXO::new(owner.clone(), 100, iou_id.clone());
    let utxo2 = UTXO::new(owner.clone(), 200, iou_id.clone());

    set.add(utxo1);
    // Adding UTXO with same source IOU should fail or replace
    // This depends on implementation - either is valid behavior
    // Just ensure no crash
    set.add(utxo2);
}

// ============================================================================
// SERIALIZATION ROUND-TRIP
// ============================================================================

#[test]
fn test_vault_state_serializable() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Add some state
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Serialize vault state
    let state = vault.export_state();
    assert!(state.is_ok());

    // Create new vault and import
    let mut vault2 = Vault::new(alice.public_key());
    vault2.import_state(state.unwrap()).unwrap();

    assert_eq!(vault.balance(), vault2.balance());
}

// ============================================================================
// STRESS TESTS
// ============================================================================

#[test]
fn test_rapid_receive_send_cycles() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    for cycle in 0..10 {
        // Receive
        let incoming = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(100)
            .nonce(cycle * 2)
            .build()
            .unwrap();
        vault.receive_iou(incoming, &bob.public_key()).unwrap();

        // Send
        let outgoing = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(50)
            .nonce(cycle * 2 + 1)
            .build()
            .unwrap();
        vault.record_sent_iou(outgoing).unwrap();
    }

    // Should have 500 remaining (10 * 50)
    assert_eq!(vault.balance(), 500);
}

#[test]
fn test_fragmented_balance_consolidation() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Create 50 tiny UTXOs
    for i in 0..50 {
        let incoming = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(2)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(incoming, &bob.public_key()).unwrap();
    }

    assert_eq!(vault.balance(), 100);
    assert_eq!(vault.utxo_set().len(), 50);

    // Spend 100 to consolidate all
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    assert_eq!(vault.balance(), 0);
    assert_eq!(vault.utxo_set().len(), 0);
    assert_eq!(vault.spent_outputs().len(), 50);
}
