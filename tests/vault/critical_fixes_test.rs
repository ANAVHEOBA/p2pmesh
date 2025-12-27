// Critical fixes tests - exposing and validating fixes for:
// 1. UTXO ID collision on change UTXOs
// 2. Lock timeout mechanism
// 3. Memory growth in processed_ious

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::IOUBuilder;
use p2pmesh::vault::{Vault, VaultError, UTXOType};
use std::thread;
use std::time::Duration;

// ============================================================================
// ISSUE 1: UTXO ID COLLISION ON CHANGE
// ============================================================================

/// Test: Change UTXO should have different ID than payment UTXO
///
/// Scenario: Alice has 100, pays Bob 60, should have change of 40.
/// The change UTXO must have a different ID than the UTXO Bob receives.
#[test]
fn test_change_utxo_has_unique_id() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let mut alice_vault = Vault::new(alice.public_key());
    let mut bob_vault = Vault::new(bob.public_key());

    // Alice receives 100 from Charlie
    let funding = IOUBuilder::new()
        .sender(&charlie)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    alice_vault.receive_iou(funding, &charlie.public_key()).unwrap();

    // Alice pays Bob 60 (will have 40 change)
    let payment = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(60)
        .build()
        .unwrap();

    alice_vault.record_sent_iou(payment.clone()).unwrap();

    // Alice should have 40 as change
    assert_eq!(alice_vault.balance(), 40);

    // Bob receives the payment
    bob_vault.receive_iou(payment.clone(), &alice.public_key()).unwrap();
    assert_eq!(bob_vault.balance(), 60);

    // CRITICAL: Get the UTXO IDs
    let alice_utxo_id = {
        let utxos = alice_vault.utxo_set();
        utxos.first().unwrap().id().clone()
    };
    let bob_utxo_id = {
        let utxos = bob_vault.utxo_set();
        utxos.first().unwrap().id().clone()
    };

    // These IDs MUST be different - same source IOU but different purposes
    assert_ne!(
        alice_utxo_id,
        bob_utxo_id,
        "Change UTXO and payment UTXO must have different IDs even from same IOU"
    );
}

/// Test: Multiple change UTXOs from sequential transactions should be unique
#[test]
fn test_sequential_change_utxos_unique() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let mut alice_vault = Vault::new(alice.public_key());

    // Alice receives 1000
    let funding = IOUBuilder::new()
        .sender(&charlie)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(1000)
        .build()
        .unwrap();
    alice_vault.receive_iou(funding, &charlie.public_key()).unwrap();

    let mut seen_ids: std::collections::HashSet<Vec<u8>> = std::collections::HashSet::new();

    // Make 10 payments, each creating change
    for i in 0..10 {
        let payment = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(50)
            .nonce(i)
            .build()
            .unwrap();
        alice_vault.record_sent_iou(payment).unwrap();

        // Check all current UTXO IDs are unique
        for utxo in alice_vault.utxo_set() {
            let id_bytes = utxo.id().as_bytes().to_vec();
            assert!(
                seen_ids.insert(id_bytes.clone()),
                "UTXO ID collision detected at iteration {}",
                i
            );
        }
    }

    assert_eq!(alice_vault.balance(), 500); // 1000 - 10*50
}

/// Test: UTXO type should be distinguishable
#[test]
fn test_utxo_type_distinguishable() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let mut alice_vault = Vault::new(alice.public_key());

    // Alice receives 100
    let funding = IOUBuilder::new()
        .sender(&charlie)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    alice_vault.receive_iou(funding, &charlie.public_key()).unwrap();

    // Check it's a received UTXO
    {
        let utxos = alice_vault.utxo_set();
        let received_utxo = utxos.first().unwrap();
        assert_eq!(received_utxo.utxo_type(), UTXOType::Received);
    }

    // Alice pays Bob 60, creating change
    let payment = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(60)
        .build()
        .unwrap();
    alice_vault.record_sent_iou(payment).unwrap();

    // Check the remaining UTXO is change type
    {
        let utxos = alice_vault.utxo_set();
        let change_utxo = utxos.first().unwrap();
        assert_eq!(change_utxo.utxo_type(), UTXOType::Change);
        assert_eq!(change_utxo.amount(), 40);
    }
}

// ============================================================================
// ISSUE 2: LOCK TIMEOUT
// ============================================================================

/// Test: Locked UTXOs should expire after timeout
#[test]
fn test_lock_expires_after_timeout() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut vault = Vault::new(alice.public_key());

    // Receive funds
    let funding = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(funding, &bob.public_key()).unwrap();

    let utxo_id = vault.utxo_set().first().unwrap().id().clone();

    // Lock with a very short timeout (100ms for testing)
    vault.lock_utxo_with_timeout(&utxo_id, 100).unwrap();

    // Should be locked immediately
    assert!(vault.get_utxo(&utxo_id).unwrap().is_locked());
    assert_eq!(vault.available_balance(), 0);

    // Wait for timeout
    thread::sleep(Duration::from_millis(150));

    // Cleanup expired locks
    vault.cleanup_expired_locks();

    // Should be unlocked now
    assert!(!vault.get_utxo(&utxo_id).unwrap().is_locked());
    assert_eq!(vault.available_balance(), 100);
}

/// Test: Lock should persist if not expired
#[test]
fn test_lock_persists_before_timeout() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut vault = Vault::new(alice.public_key());

    let funding = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(funding, &bob.public_key()).unwrap();

    let utxo_id = vault.utxo_set().first().unwrap().id().clone();

    // Lock with 1 second timeout
    vault.lock_utxo_with_timeout(&utxo_id, 1000).unwrap();

    // Wait only 100ms
    thread::sleep(Duration::from_millis(100));

    vault.cleanup_expired_locks();

    // Should still be locked
    assert!(vault.get_utxo(&utxo_id).unwrap().is_locked());
}

/// Test: Multiple locks with different timeouts
#[test]
fn test_multiple_locks_different_timeouts() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut vault = Vault::new(alice.public_key());

    // Receive multiple UTXOs
    for i in 0..3 {
        let funding = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(100)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(funding, &bob.public_key()).unwrap();
    }

    let utxo_ids: Vec<_> = vault.utxo_set().iter().map(|u| u.id().clone()).collect();

    // Lock with different timeouts
    vault.lock_utxo_with_timeout(&utxo_ids[0], 50).unwrap();   // 50ms
    vault.lock_utxo_with_timeout(&utxo_ids[1], 150).unwrap();  // 150ms
    vault.lock_utxo_with_timeout(&utxo_ids[2], 300).unwrap();  // 300ms

    assert_eq!(vault.available_balance(), 0);

    // After 100ms, first should be expired
    thread::sleep(Duration::from_millis(100));
    vault.cleanup_expired_locks();
    assert_eq!(vault.available_balance(), 100);

    // After another 100ms, second should be expired
    thread::sleep(Duration::from_millis(100));
    vault.cleanup_expired_locks();
    assert_eq!(vault.available_balance(), 200);

    // After another 150ms, third should be expired
    thread::sleep(Duration::from_millis(150));
    vault.cleanup_expired_locks();
    assert_eq!(vault.available_balance(), 300);
}

/// Test: Getting lock info
#[test]
fn test_get_lock_info() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut vault = Vault::new(alice.public_key());

    let funding = IOUBuilder::new()
        .sender(&bob)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(100)
        .build()
        .unwrap();
    vault.receive_iou(funding, &bob.public_key()).unwrap();

    let utxo_id = vault.utxo_set().first().unwrap().id().clone();

    // No lock initially
    assert!(vault.get_lock_info(&utxo_id).is_none());

    // Lock it
    vault.lock_utxo_with_timeout(&utxo_id, 5000).unwrap();

    // Should have lock info now
    let lock_info = vault.get_lock_info(&utxo_id).unwrap();
    assert!(lock_info.expires_at > 0);
    assert!(!lock_info.is_expired());
}

// ============================================================================
// ISSUE 3: PROCESSED IOUS MEMORY GROWTH
// ============================================================================

/// Test: Old processed IOUs should be prunable
#[test]
fn test_prune_old_processed_ious() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut vault = Vault::new(alice.public_key());

    // Receive many IOUs
    for i in 0..100 {
        let funding = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(1)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(funding, &bob.public_key()).unwrap();
    }

    assert_eq!(vault.processed_iou_count(), 100);

    // Get a timestamp that's slightly in the future to prune all
    let future_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() + 100;

    // Prune all IOUs (they were all processed "before" the future)
    let pruned = vault.prune_processed_ious_before(future_timestamp);

    assert_eq!(pruned, 100); // All should be pruned
    assert_eq!(vault.processed_iou_count(), 0);
}

/// Test: Pruning respects max count
#[test]
fn test_prune_to_max_count() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut vault = Vault::new(alice.public_key());

    // Receive many IOUs with sequential timestamps
    for i in 0..200u64 {
        let funding = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(1)
            .nonce(i)
            .timestamp(i)
            .build()
            .unwrap();
        vault.receive_iou(funding, &bob.public_key()).unwrap();
    }

    assert_eq!(vault.processed_iou_count(), 200);

    // Prune to max 50 entries (keeping most recent)
    let pruned = vault.prune_processed_ious_to_max(50);

    assert_eq!(pruned, 150);
    assert_eq!(vault.processed_iou_count(), 50);
}

/// Test: Pruning to max count keeps entries and prevents replay
#[test]
fn test_recent_iou_protected_from_prune() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut vault = Vault::new(alice.public_key());

    // Receive multiple IOUs
    let mut ious = Vec::new();
    for i in 0..5 {
        let iou = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(10)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(iou.clone(), &bob.public_key()).unwrap();
        ious.push(iou);
    }

    assert_eq!(vault.processed_iou_count(), 5);

    // Prune to keep only 3
    vault.prune_processed_ious_to_max(3);

    assert_eq!(vault.processed_iou_count(), 3);

    // Count how many are still detected as duplicates
    let mut duplicates = 0;
    for iou in &ious {
        if vault.has_processed_iou(&iou.id()) {
            duplicates += 1;
        }
    }

    // Should have exactly 3 still protected
    assert_eq!(duplicates, 3);
}

/// Test: Memory stats are available
#[test]
fn test_memory_stats() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();

    let mut vault = Vault::new(alice.public_key());

    for i in 0..50 {
        let funding = IOUBuilder::new()
            .sender(&bob)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(1)
            .nonce(i)
            .build()
            .unwrap();
        vault.receive_iou(funding, &bob.public_key()).unwrap();
    }

    let stats = vault.memory_stats();

    assert_eq!(stats.processed_iou_count, 50);
    assert_eq!(stats.utxo_count, 50);
    assert!(stats.estimated_bytes > 0);
}

// ============================================================================
// COMBINED SCENARIOS
// ============================================================================

/// Test: Full lifecycle with all fixes working together
#[test]
fn test_full_lifecycle_with_fixes() {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let charlie = Keypair::generate();

    let mut alice_vault = Vault::new(alice.public_key());
    let mut bob_vault = Vault::new(bob.public_key());

    // 1. Alice receives funds
    let funding = IOUBuilder::new()
        .sender(&charlie)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(1000)
        .build()
        .unwrap();
    alice_vault.receive_iou(funding, &charlie.public_key()).unwrap();

    // 2. Lock funds with timeout before payment
    let utxo_id = {
        let utxos = alice_vault.utxo_set();
        utxos.first().unwrap().id().clone()
    };
    alice_vault.lock_utxo_with_timeout(&utxo_id, 5000).unwrap();

    // 3. Can't spend locked funds
    let payment = IOUBuilder::new()
        .sender(&alice)
        .recipient(Did::from_public_key(&bob.public_key()))
        .amount(500)
        .build()
        .unwrap();

    // This should fail because funds are locked
    assert!(alice_vault.record_sent_iou(payment.clone()).is_err());

    // 4. Unlock and pay
    alice_vault.unlock_utxo(&utxo_id).unwrap();
    alice_vault.record_sent_iou(payment.clone()).unwrap();

    // 5. Verify change UTXO is correct type and unique ID
    assert_eq!(alice_vault.balance(), 500);
    let change_id = {
        let utxos = alice_vault.utxo_set();
        let change = utxos.first().unwrap();
        assert_eq!(change.utxo_type(), UTXOType::Change);
        change.id().clone()
    };

    // 6. Bob receives and verifies
    bob_vault.receive_iou(payment, &alice.public_key()).unwrap();
    assert_eq!(bob_vault.balance(), 500);
    let received_id = {
        let utxos = bob_vault.utxo_set();
        let received = utxos.first().unwrap();
        assert_eq!(received.utxo_type(), UTXOType::Received);
        received.id().clone()
    };

    // 7. IDs must be different
    assert_ne!(change_id, received_id);

    // 8. Memory management
    assert!(alice_vault.processed_iou_count() > 0);
    assert!(bob_vault.processed_iou_count() > 0);
}
