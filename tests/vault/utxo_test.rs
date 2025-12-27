// UTXO (Unspent Transaction Output) management tests

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::{IOUBuilder, SignedIOU, IOUId};
use p2pmesh::vault::{Vault, VaultError, UTXO, UTXOSet, UTXOId};

// ============================================================================
// UTXO CREATION TESTS
// ============================================================================

#[test]
fn test_utxo_created_from_received_iou() {
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

    let utxos = vault.utxo_set();
    assert_eq!(utxos.len(), 1);
    assert_eq!(utxos[0].amount(), 100);
}

#[test]
fn test_utxo_has_unique_id() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

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

    let utxos = vault.utxo_set();
    assert_eq!(utxos.len(), 2);
    assert_ne!(utxos[0].id(), utxos[1].id());
}

#[test]
fn test_utxo_references_source_iou() {
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

    let utxos = vault.utxo_set();
    assert_eq!(utxos[0].source_iou_id(), &iou_id);
}

#[test]
fn test_utxo_owner_matches_recipient() {
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

    let utxos = vault.utxo_set();
    assert_eq!(utxos[0].owner(), &bob.public_key());
}

// ============================================================================
// UTXO SPENDING TESTS
// ============================================================================

#[test]
fn test_spending_removes_utxo() {
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
    assert_eq!(vault.utxo_set().len(), 1);

    // Spend all
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    assert_eq!(vault.utxo_set().len(), 0);
}

#[test]
fn test_partial_spend_creates_change_utxo() {
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

    // Spend 30, should have 70 as change
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(30)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    let utxos = vault.utxo_set();
    assert_eq!(utxos.len(), 1);
    assert_eq!(utxos[0].amount(), 70);
}

#[test]
fn test_spending_selects_optimal_utxos() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive multiple amounts
    for (i, amount) in [10, 20, 50, 100].iter().enumerate() {
        let incoming = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(*amount)
            .nonce(i as u64)
            .build()
            .unwrap();
        vault.receive_iou(incoming, &bob.public_key()).unwrap();
    }

    assert_eq!(vault.utxo_set().len(), 4);
    assert_eq!(vault.balance(), 180);

    // Spend 50 - should use the 50 UTXO exactly
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(50)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    assert_eq!(vault.balance(), 130);
}

#[test]
fn test_spending_consolidates_small_utxos() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    // Receive many small amounts
    for i in 0..10 {
        let incoming = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(10)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(incoming, &bob.public_key()).unwrap();
    }

    assert_eq!(vault.utxo_set().len(), 10);
    assert_eq!(vault.balance(), 100);

    // Spend 55 - will need to consume multiple UTXOs
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(55)
        .build()
        .unwrap();
    vault.record_sent_iou(outgoing).unwrap();

    assert_eq!(vault.balance(), 45);
    // Should have fewer UTXOs now (consumed 6, created 1 change = 5 remaining)
    assert!(vault.utxo_set().len() < 10);
}

// ============================================================================
// UTXO QUERY TESTS
// ============================================================================

#[test]
fn test_get_utxo_by_id() {
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

    let utxos = vault.utxo_set();
    let utxo_id = utxos[0].id().clone();

    let found = vault.get_utxo(&utxo_id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().amount(), 100);
}

#[test]
fn test_get_nonexistent_utxo_returns_none() {
    let bob = Keypair::generate();
    let vault = Vault::new(bob.public_key());

    let fake_id = UTXOId::from_bytes([0u8; 32]);
    assert!(vault.get_utxo(&fake_id).is_none());
}

#[test]
fn test_total_utxo_value_equals_balance() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    for i in 1..=5 {
        let iou = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(i * 10)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(iou, &alice.public_key()).unwrap();
    }

    let total: u64 = vault.utxo_set().iter().map(|u| u.amount()).sum();
    assert_eq!(total, vault.balance());
}

#[test]
fn test_utxo_set_is_ordered_by_amount() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let mut vault = Vault::new(bob.public_key());

    // Receive in random order
    for (i, amount) in [50, 10, 100, 25].iter().enumerate() {
        let iou = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(*amount)
            .nonce(i as u64)
            .build()
            .unwrap();
        vault.receive_iou(iou, &alice.public_key()).unwrap();
    }

    let utxos = vault.utxo_set_sorted_by_amount();
    let amounts: Vec<u64> = utxos.iter().map(|u| u.amount()).collect();

    // Should be sorted ascending or descending
    let is_ascending = amounts.windows(2).all(|w| w[0] <= w[1]);
    let is_descending = amounts.windows(2).all(|w| w[0] >= w[1]);
    assert!(is_ascending || is_descending);
}

// ============================================================================
// UTXO SET OPERATIONS
// ============================================================================

#[test]
fn test_utxo_set_new_is_empty() {
    let set = UTXOSet::new();
    assert!(set.is_empty());
    assert_eq!(set.len(), 0);
    assert_eq!(set.total_value(), 0);
}

#[test]
fn test_utxo_set_add() {
    let mut set = UTXOSet::new();
    let owner = Keypair::generate().public_key();
    let iou_id = IOUId::from_bytes([1u8; 32]);

    let utxo = UTXO::new(owner, 100, iou_id);
    set.add(utxo);

    assert_eq!(set.len(), 1);
    assert_eq!(set.total_value(), 100);
}

#[test]
fn test_utxo_set_remove() {
    let mut set = UTXOSet::new();
    let owner = Keypair::generate().public_key();
    let iou_id = IOUId::from_bytes([1u8; 32]);

    let utxo = UTXO::new(owner, 100, iou_id);
    let utxo_id = utxo.id().clone();
    set.add(utxo);

    let removed = set.remove(&utxo_id);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().amount(), 100);
    assert!(set.is_empty());
}

#[test]
fn test_utxo_set_contains() {
    let mut set = UTXOSet::new();
    let owner = Keypair::generate().public_key();
    let iou_id = IOUId::from_bytes([1u8; 32]);

    let utxo = UTXO::new(owner, 100, iou_id);
    let utxo_id = utxo.id().clone();
    set.add(utxo);

    assert!(set.contains(&utxo_id));

    let fake_id = UTXOId::from_bytes([2u8; 32]);
    assert!(!set.contains(&fake_id));
}

#[test]
fn test_utxo_set_select_for_amount_exact() {
    let mut set = UTXOSet::new();
    let owner = Keypair::generate().public_key();

    // Add 100
    let utxo = UTXO::new(owner.clone(), 100, IOUId::from_bytes([1u8; 32]));
    set.add(utxo);

    let selected = set.select_for_amount(100);
    assert!(selected.is_some());
    let (utxos, change) = selected.unwrap();
    assert_eq!(utxos.len(), 1);
    assert_eq!(change, 0);
}

#[test]
fn test_utxo_set_select_for_amount_with_change() {
    let mut set = UTXOSet::new();
    let owner = Keypair::generate().public_key();

    // Add 100
    let utxo = UTXO::new(owner.clone(), 100, IOUId::from_bytes([1u8; 32]));
    set.add(utxo);

    let selected = set.select_for_amount(30);
    assert!(selected.is_some());
    let (utxos, change) = selected.unwrap();
    assert_eq!(utxos.len(), 1);
    assert_eq!(change, 70);
}

#[test]
fn test_utxo_set_select_insufficient_returns_none() {
    let mut set = UTXOSet::new();
    let owner = Keypair::generate().public_key();

    // Add 50
    let utxo = UTXO::new(owner.clone(), 50, IOUId::from_bytes([1u8; 32]));
    set.add(utxo);

    let selected = set.select_for_amount(100);
    assert!(selected.is_none());
}

#[test]
fn test_utxo_set_select_multiple_utxos() {
    let mut set = UTXOSet::new();
    let owner = Keypair::generate().public_key();

    // Add 30, 40, 50
    set.add(UTXO::new(owner.clone(), 30, IOUId::from_bytes([1u8; 32])));
    set.add(UTXO::new(owner.clone(), 40, IOUId::from_bytes([2u8; 32])));
    set.add(UTXO::new(owner.clone(), 50, IOUId::from_bytes([3u8; 32])));

    let selected = set.select_for_amount(100);
    assert!(selected.is_some());
    let (utxos, change) = selected.unwrap();

    let total: u64 = utxos.iter().map(|u| u.amount()).sum();
    assert!(total >= 100);
    assert_eq!(total - 100, change);
}

// ============================================================================
// UTXO LOCKING TESTS (for pending transactions)
// ============================================================================

#[test]
fn test_lock_utxo_prevents_spending() {
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

    // Lock the UTXO
    let utxo_id = vault.utxo_set()[0].id().clone();
    vault.lock_utxo(&utxo_id).unwrap();

    // Try to spend - should fail
    let outgoing = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(100)
        .build()
        .unwrap();

    let result = vault.record_sent_iou(outgoing);
    assert!(matches!(result, Err(VaultError::InsufficientBalance { .. })));
}

#[test]
fn test_unlock_utxo_allows_spending() {
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

    // Lock then unlock
    let utxo_id = vault.utxo_set()[0].id().clone();
    vault.lock_utxo(&utxo_id).unwrap();
    vault.unlock_utxo(&utxo_id).unwrap();

    // Spend should work now
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
fn test_lock_nonexistent_utxo_fails() {
    let alice = Keypair::generate();
    let mut vault = Vault::new(alice.public_key());

    let fake_id = UTXOId::from_bytes([0u8; 32]);
    let result = vault.lock_utxo(&fake_id);

    assert!(matches!(result, Err(VaultError::UTXONotFound)));
}

#[test]
fn test_locked_utxos_excluded_from_available_balance() {
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
    vault.lock_utxo(&utxo_id).unwrap();

    assert_eq!(vault.balance(), 100);
    assert_eq!(vault.available_balance(), 0);
}
