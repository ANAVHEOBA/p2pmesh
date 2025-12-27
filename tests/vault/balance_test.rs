// Balance tracking tests for the vault module

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::{IOUBuilder, SignedIOU};
use p2pmesh::vault::{Vault, VaultError};

// ============================================================================
// VAULT CREATION TESTS
// ============================================================================

#[test]
fn test_new_vault_has_zero_balance() {
    let keypair = Keypair::generate();
    let vault = Vault::new(keypair.public_key());

    assert_eq!(vault.balance(), 0);
}

#[test]
fn test_vault_owner_matches_public_key() {
    let keypair = Keypair::generate();
    let vault = Vault::new(keypair.public_key());

    assert_eq!(vault.owner(), &keypair.public_key());
}

#[test]
fn test_new_vault_has_no_transactions() {
    let keypair = Keypair::generate();
    let vault = Vault::new(keypair.public_key());

    assert_eq!(vault.transaction_count(), 0);
}

// ============================================================================
// RECEIVING IOU TESTS
// ============================================================================

#[test]
fn test_receive_iou_increases_balance() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    vault.receive_iou(iou, &alice.public_key()).unwrap();

    assert_eq!(vault.balance(), 100);
}

#[test]
fn test_receive_multiple_ious_accumulates_balance() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    for i in 1..=5 {
        let iou = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(100)
            .nonce(i)
            .build()
            .unwrap();

        vault.receive_iou(iou, &alice.public_key()).unwrap();
    }

    assert_eq!(vault.balance(), 500);
}

#[test]
fn test_receive_iou_from_different_senders() {
    let alice = Keypair::generate();
    let charlie = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    let iou1 = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let iou2 = IOUBuilder::new()
        .sender(&charlie)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(200)
        .build()
        .unwrap();

    vault.receive_iou(iou1, &alice.public_key()).unwrap();
    vault.receive_iou(iou2, &charlie.public_key()).unwrap();

    assert_eq!(vault.balance(), 300);
}

#[test]
fn test_receive_iou_wrong_recipient_fails() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    // IOU addressed to Charlie, not Bob
    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&charlie.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let result = vault.receive_iou(iou, &alice.public_key());

    assert!(matches!(result, Err(VaultError::RecipientMismatch)));
}

#[test]
fn test_receive_iou_invalid_signature_fails() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    // Verify with wrong public key
    let result = vault.receive_iou(iou, &charlie.public_key());

    // The error can be InvalidSignature, SenderMismatch, or wrapped in ValidationFailed
    assert!(result.is_err());
}

#[test]
fn test_receive_duplicate_iou_fails() {
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
    let result = vault.receive_iou(iou, &alice.public_key());

    assert!(matches!(result, Err(VaultError::DuplicateTransaction)));
}

#[test]
fn test_receive_iou_zero_amount_fails() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    // Zero amount should fail at builder level, but vault should also reject
    let result = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(0)
        .build();

    assert!(result.is_err());
}

// ============================================================================
// SENDING IOU TESTS (Balance Reduction)
// ============================================================================

#[test]
fn test_send_iou_decreases_balance() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // First receive some funds
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();

    vault.receive_iou(incoming, &bob.public_key()).unwrap();
    assert_eq!(vault.balance(), 100);

    // Now send some
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(30)
        .build()
        .unwrap();

    vault.record_sent_iou(outgoing).unwrap();

    assert_eq!(vault.balance(), 70);
}

#[test]
fn test_send_iou_insufficient_balance_fails() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Vault has 0 balance, try to send
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let result = vault.record_sent_iou(outgoing);

    assert!(matches!(result, Err(VaultError::InsufficientBalance { available: 0, required: 100 })));
}

#[test]
fn test_send_exact_balance_succeeds() {
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

    // Send exactly 100
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    vault.record_sent_iou(outgoing).unwrap();

    assert_eq!(vault.balance(), 0);
}

#[test]
fn test_send_iou_wrong_sender_fails() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive funds
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();

    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Try to record an IOU from Charlie (not vault owner)
    let outgoing = IOUBuilder::new()
        .sender(&charlie)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(50)
        .build()
        .unwrap();

    let result = vault.record_sent_iou(outgoing);

    assert!(matches!(result, Err(VaultError::NotOwner)));
}

// ============================================================================
// BALANCE QUERY TESTS
// ============================================================================

#[test]
fn test_available_balance_excludes_pending() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive funds
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();

    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Reserve some for pending transaction
    vault.reserve_balance(30).unwrap();

    assert_eq!(vault.balance(), 100);
    assert_eq!(vault.available_balance(), 70);
}

#[test]
fn test_reserve_more_than_available_fails() {
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

    // Try to reserve 150
    let result = vault.reserve_balance(150);

    assert!(matches!(result, Err(VaultError::InsufficientBalance { .. })));
}

#[test]
fn test_release_reserved_balance() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive funds
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();

    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Reserve and then release
    let reservation_id = vault.reserve_balance(30).unwrap();
    assert_eq!(vault.available_balance(), 70);

    vault.release_reservation(reservation_id).unwrap();
    assert_eq!(vault.available_balance(), 100);
}

#[test]
fn test_commit_reservation_releases_hold() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive funds
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();

    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Reserve and commit - commit just releases the hold
    // The actual balance reduction happens via record_sent_iou
    let reservation_id = vault.reserve_balance(30).unwrap();
    assert_eq!(vault.available_balance(), 70); // 30 is reserved

    let committed_amount = vault.commit_reservation(reservation_id).unwrap();

    assert_eq!(committed_amount, 30);
    // After commit, reservation is released - full balance available again
    // (In practice, record_sent_iou would be called to actually spend)
    assert_eq!(vault.balance(), 100);
    assert_eq!(vault.available_balance(), 100);
}

// ============================================================================
// TRANSACTION HISTORY TESTS
// ============================================================================

#[test]
fn test_transaction_count_increases() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    for i in 1..=3 {
        let iou = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(10)
            .nonce(i)
            .build()
            .unwrap();

        vault.receive_iou(iou, &alice.public_key()).unwrap();
    }

    assert_eq!(vault.transaction_count(), 3);
}

#[test]
fn test_get_transaction_history() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    let iou = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    vault.receive_iou(iou.clone(), &alice.public_key()).unwrap();

    let history = vault.transaction_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].iou().iou().amount(), 100);
}

#[test]
fn test_get_received_transactions_only() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive from bob
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Send to bob
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(30)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    let received = vault.received_transactions();
    assert_eq!(received.len(), 1);
}

#[test]
fn test_get_sent_transactions_only() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive from bob
    let incoming = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(incoming, &bob.public_key()).unwrap();

    // Send to bob
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(30)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    let sent = vault.sent_transactions();
    assert_eq!(sent.len(), 1);
}

// ============================================================================
// BALANCE BY SENDER TESTS
// ============================================================================

#[test]
fn test_balance_from_specific_sender() {
    let alice = Keypair::generate();
    let charlie = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    // 100 from Alice
    let iou1 = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(iou1, &alice.public_key()).unwrap();

    // 200 from Charlie
    let iou2 = IOUBuilder::new()
        .sender(&charlie)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(200)
        .build()
        .unwrap();
    vault.receive_iou(iou2, &charlie.public_key()).unwrap();

    let alice_did = Did::from_public_key(&alice.public_key());
    let charlie_did = Did::from_public_key(&charlie.public_key());

    assert_eq!(vault.balance_from_sender(&alice_did), 100);
    assert_eq!(vault.balance_from_sender(&charlie_did), 200);
    assert_eq!(vault.balance(), 300);
}

#[test]
fn test_balance_from_unknown_sender_is_zero() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();
    let vault = Vault::new(bob.public_key());

    let charlie_did = Did::from_public_key(&charlie.public_key());

    assert_eq!(vault.balance_from_sender(&charlie_did), 0);
}
