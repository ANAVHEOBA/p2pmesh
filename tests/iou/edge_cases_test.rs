use p2pmesh::identity::{Keypair, Did, Signer, Signature};
use p2pmesh::iou::{IOU, IOUBuilder, SignedIOU, IOUValidator, IOUCodec, IOUId};
use std::collections::HashSet;

// ============================================================================
// EDGE CASES AND SECURITY TESTS
// ============================================================================

// ----------------------------------------------------------------------------
// REPLAY ATTACK PREVENTION
// ----------------------------------------------------------------------------

/// Test: Same IOU ID can be detected (for replay prevention)
#[test]
fn test_iou_id_can_detect_replay() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .nonce(12345)
        .timestamp(1703612400)
        .build()
        .expect("Should build valid IOU");

    // In a real system, we'd track seen IOU IDs
    let mut seen_ids: HashSet<IOUId> = HashSet::new();

    // First time - should be new
    let id = iou.iou().id();
    assert!(seen_ids.insert(id.clone()), "First submission should be new");

    // Second time - should be detected as replay
    assert!(!seen_ids.insert(id), "Second submission should be detected as replay");
}

/// Test: Each new IOU has unique ID (prevents accidental collision)
#[test]
fn test_unique_ids_for_different_transactions() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let mut ids: HashSet<IOUId> = HashSet::new();

    // Create 100 IOUs - all should have unique IDs
    for i in 0..100 {
        let iou = IOUBuilder::new()
            .sender(&sender_kp)
            .recipient(recipient.clone())
            .amount(100 + i)
            .build()
            .expect("Should build valid IOU");

        let is_new = ids.insert(iou.iou().id());
        assert!(is_new, "IOU {} should have unique ID", i);
    }

    assert_eq!(ids.len(), 100);
}

// ----------------------------------------------------------------------------
// SIGNATURE MALLEABILITY
// ----------------------------------------------------------------------------

/// Test: Signature cannot be modified to produce another valid signature
#[test]
fn test_signature_not_malleable() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    // Try to create a "malleable" signature by flipping bits
    let original_sig = signed_iou.signature().as_bytes().to_vec();

    for i in 0..original_sig.len() {
        for bit in 0..8 {
            let mut modified = original_sig.clone();
            modified[i] ^= 1 << bit;

            if let Ok(modified_sig) = Signature::from_bytes(&modified) {
                // The modified signature should NOT verify
                let modified_iou = SignedIOU::from_parts(
                    signed_iou.iou().clone(),
                    modified_sig,
                );

                assert!(
                    !modified_iou.verify(&sender_kp.public_key()),
                    "Modified signature at byte {} bit {} should not verify",
                    i, bit
                );
            }
        }
    }
}

// ----------------------------------------------------------------------------
// AMOUNT OVERFLOW/UNDERFLOW
// ----------------------------------------------------------------------------

/// Test: u64::MAX amount is handled correctly
#[test]
fn test_max_amount_handling() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(u64::MAX)
        .build()
        .expect("Should build valid IOU");

    // Serialize and deserialize
    let bytes = IOUCodec::encode(&signed_iou);
    let decoded = IOUCodec::decode(&bytes).expect("Should decode");

    assert_eq!(decoded.iou().amount(), u64::MAX);

    // Validation should pass
    let result = IOUValidator::validate(&decoded, &sender_kp.public_key());
    assert!(result.is_ok());
}

/// Test: Amount of 1 (minimum valid) is handled correctly
#[test]
fn test_min_amount_handling() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(1)
        .build()
        .expect("Should build valid IOU");

    assert_eq!(signed_iou.iou().amount(), 1);
}

// ----------------------------------------------------------------------------
// TIMESTAMP EDGE CASES
// ----------------------------------------------------------------------------

/// Test: Timestamp of 0 is valid (for testing/genesis)
#[test]
fn test_zero_timestamp_valid() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .timestamp(0)
        .build()
        .expect("Should build valid IOU");

    assert_eq!(signed_iou.iou().timestamp(), 0);
}

/// Test: Maximum timestamp is handled
#[test]
fn test_max_timestamp_handling() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .timestamp(u64::MAX)
        .build()
        .expect("Should build valid IOU");

    let bytes = IOUCodec::encode(&signed_iou);
    let decoded = IOUCodec::decode(&bytes).expect("Should decode");

    assert_eq!(decoded.iou().timestamp(), u64::MAX);
}

// ----------------------------------------------------------------------------
// NONCE EDGE CASES
// ----------------------------------------------------------------------------

/// Test: Nonce of 0 is valid
#[test]
fn test_zero_nonce_valid() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .nonce(0)
        .build()
        .expect("Should build valid IOU");

    assert_eq!(signed_iou.iou().nonce(), 0);
}

/// Test: Maximum nonce is handled
#[test]
fn test_max_nonce_handling() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .nonce(u64::MAX)
        .build()
        .expect("Should build valid IOU");

    let bytes = IOUCodec::encode(&signed_iou);
    let decoded = IOUCodec::decode(&bytes).expect("Should decode");

    assert_eq!(decoded.iou().nonce(), u64::MAX);
}

// ----------------------------------------------------------------------------
// DID EDGE CASES
// ----------------------------------------------------------------------------

/// Test: IOU with different DID formats is handled correctly
#[test]
fn test_did_format_consistency() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();

    let sender_did = Did::from_public_key(&sender_kp.public_key());
    let recipient_did = Did::from_public_key(&recipient_kp.public_key());

    // Create IOU
    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient_did.clone())
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    // Sender DID should match what we'd derive
    assert_eq!(signed_iou.iou().sender(), &sender_did);
    assert_eq!(signed_iou.iou().recipient(), &recipient_did);
}

// ----------------------------------------------------------------------------
// CONCURRENT OPERATIONS
// ----------------------------------------------------------------------------

/// Test: Multiple IOUs from same sender at same timestamp with same amount
/// should still be distinguishable (due to different nonces)
#[test]
fn test_concurrent_ious_distinguishable() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let timestamp = 1703612400;
    let amount = 100;

    let iou1 = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient.clone())
        .amount(amount)
        .timestamp(timestamp)
        .build()
        .expect("Should build valid IOU");

    let iou2 = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(amount)
        .timestamp(timestamp)
        .build()
        .expect("Should build valid IOU");

    // Should have different IDs due to auto-generated nonces
    assert_ne!(iou1.iou().id(), iou2.iou().id());
}

// ----------------------------------------------------------------------------
// DATA INTEGRITY
// ----------------------------------------------------------------------------

/// Test: IOU ID is computed from all fields
#[test]
fn test_id_includes_all_fields() {
    let sender1_kp = Keypair::generate();
    let sender2_kp = Keypair::generate();
    let recipient1_kp = Keypair::generate();
    let recipient2_kp = Keypair::generate();

    let sender1 = Did::from_public_key(&sender1_kp.public_key());
    let sender2 = Did::from_public_key(&sender2_kp.public_key());
    let recipient1 = Did::from_public_key(&recipient1_kp.public_key());
    let recipient2 = Did::from_public_key(&recipient2_kp.public_key());

    // Base IOU
    let base = IOU::new(sender1.clone(), recipient1.clone(), 100, 12345, 1703612400);

    // Change each field and verify ID changes
    let changed_sender = IOU::new(sender2, recipient1.clone(), 100, 12345, 1703612400);
    let changed_recipient = IOU::new(sender1.clone(), recipient2, 100, 12345, 1703612400);
    let changed_amount = IOU::new(sender1.clone(), recipient1.clone(), 200, 12345, 1703612400);
    let changed_nonce = IOU::new(sender1.clone(), recipient1.clone(), 100, 99999, 1703612400);
    let changed_timestamp = IOU::new(sender1, recipient1, 100, 12345, 9999999999);

    assert_ne!(base.id(), changed_sender.id(), "Sender affects ID");
    assert_ne!(base.id(), changed_recipient.id(), "Recipient affects ID");
    assert_ne!(base.id(), changed_amount.id(), "Amount affects ID");
    assert_ne!(base.id(), changed_nonce.id(), "Nonce affects ID");
    assert_ne!(base.id(), changed_timestamp.id(), "Timestamp affects ID");
}

// ----------------------------------------------------------------------------
// SIGNING CONSISTENCY
// ----------------------------------------------------------------------------

/// Test: Same IOU content produces same signature (deterministic)
#[test]
fn test_signing_is_deterministic() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient.clone())
        .amount(100)
        .nonce(12345)
        .timestamp(1703612400)
        .build()
        .expect("Should build valid IOU");

    let iou2 = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .nonce(12345)
        .timestamp(1703612400)
        .build()
        .expect("Should build valid IOU");

    assert_eq!(
        iou1.signature().as_bytes(),
        iou2.signature().as_bytes(),
        "Same content should produce same signature"
    );
}

/// Test: Different content produces different signature
#[test]
fn test_different_content_different_signature() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient.clone())
        .amount(100)
        .nonce(11111)
        .timestamp(1703612400)
        .build()
        .expect("Should build valid IOU");

    let iou2 = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .nonce(22222) // Different nonce
        .timestamp(1703612400)
        .build()
        .expect("Should build valid IOU");

    assert_ne!(
        iou1.signature().as_bytes(),
        iou2.signature().as_bytes(),
        "Different content should produce different signature"
    );
}

// ----------------------------------------------------------------------------
// MEMORY SAFETY
// ----------------------------------------------------------------------------

/// Test: Large batch of IOUs doesn't cause memory issues
#[test]
fn test_large_batch_handling() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let mut ious = Vec::new();

    for i in 0..1000 {
        let iou = IOUBuilder::new()
            .sender(&sender_kp)
            .recipient(recipient.clone())
            .amount(i + 1)
            .build()
            .expect("Should build valid IOU");

        ious.push(iou);
    }

    assert_eq!(ious.len(), 1000);

    // Serialize all
    let serialized: Vec<Vec<u8>> = ious.iter()
        .map(|iou| IOUCodec::encode(iou))
        .collect();

    assert_eq!(serialized.len(), 1000);

    // Deserialize all
    let deserialized: Vec<SignedIOU> = serialized.iter()
        .map(|bytes| IOUCodec::decode(bytes).expect("Should decode"))
        .collect();

    assert_eq!(deserialized.len(), 1000);
}

// ----------------------------------------------------------------------------
// ERROR MESSAGE QUALITY
// ----------------------------------------------------------------------------

/// Test: Error messages are descriptive
#[test]
fn test_error_messages_descriptive() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    // Test missing sender error
    let result = IOUBuilder::new()
        .recipient(recipient.clone())
        .amount(100)
        .build();

    if let Err(e) = result {
        let msg = format!("{}", e);
        assert!(
            msg.to_lowercase().contains("sender") || msg.to_lowercase().contains("missing"),
            "Error should mention missing sender: {}",
            msg
        );
    }

    // Test zero amount error
    let result = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient.clone())
        .amount(0)
        .build();

    if let Err(e) = result {
        let msg = format!("{}", e);
        assert!(
            msg.to_lowercase().contains("amount") || msg.to_lowercase().contains("zero"),
            "Error should mention invalid amount: {}",
            msg
        );
    }

    // Test self-payment error
    let sender_did = Did::from_public_key(&sender_kp.public_key());
    let result = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(sender_did)
        .amount(100)
        .build();

    if let Err(e) = result {
        let msg = format!("{}", e);
        assert!(
            msg.to_lowercase().contains("self") || msg.to_lowercase().contains("same"),
            "Error should mention self-payment: {}",
            msg
        );
    }
}
