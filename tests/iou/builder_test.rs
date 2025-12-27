use p2pmesh::identity::{Keypair, Did};
use p2pmesh::iou::{IOUBuilder, SignedIOU, IOUError};

// ============================================================================
// IOU BUILDER TESTS
// ============================================================================

/// Test: Can build a basic IOU
#[test]
fn test_build_basic_iou() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    assert_eq!(signed_iou.iou().amount(), 100);
}

/// Test: Builder generates nonce automatically if not provided
#[test]
fn test_builder_auto_generates_nonce() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient.clone())
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    let iou2 = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    // Auto-generated nonces should be different
    assert_ne!(
        iou1.iou().nonce(),
        iou2.iou().nonce(),
        "Auto-generated nonces should be unique"
    );
}

/// Test: Builder generates timestamp automatically if not provided
#[test]
fn test_builder_auto_generates_timestamp() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    let timestamp = signed_iou.iou().timestamp();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Timestamp should be within 5 seconds of now
    assert!(
        timestamp >= now - 5 && timestamp <= now + 5,
        "Auto-generated timestamp should be close to current time"
    );
}

/// Test: Can set custom nonce
#[test]
fn test_builder_custom_nonce() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .nonce(99999)
        .build()
        .expect("Should build valid IOU");

    assert_eq!(signed_iou.iou().nonce(), 99999);
}

/// Test: Can set custom timestamp
#[test]
fn test_builder_custom_timestamp() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .timestamp(1703612400)
        .build()
        .expect("Should build valid IOU");

    assert_eq!(signed_iou.iou().timestamp(), 1703612400);
}

/// Test: Builder fails without sender
#[test]
fn test_builder_fails_without_sender() {
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let result = IOUBuilder::new()
        .recipient(recipient)
        .amount(100)
        .build();

    assert!(result.is_err(), "Should fail without sender");
    match result {
        Err(IOUError::MissingSender) => {}
        _ => panic!("Expected MissingSender error"),
    }
}

/// Test: Builder fails without recipient
#[test]
fn test_builder_fails_without_recipient() {
    let sender_kp = Keypair::generate();

    let result = IOUBuilder::new()
        .sender(&sender_kp)
        .amount(100)
        .build();

    assert!(result.is_err(), "Should fail without recipient");
    match result {
        Err(IOUError::MissingRecipient) => {}
        _ => panic!("Expected MissingRecipient error"),
    }
}

/// Test: Builder fails without amount
#[test]
fn test_builder_fails_without_amount() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let result = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .build();

    assert!(result.is_err(), "Should fail without amount");
    match result {
        Err(IOUError::MissingAmount) => {}
        _ => panic!("Expected MissingAmount error"),
    }
}

/// Test: Builder rejects zero amount
#[test]
fn test_builder_rejects_zero_amount() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let result = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(0)
        .build();

    assert!(result.is_err(), "Should reject zero amount");
    match result {
        Err(IOUError::InvalidAmount(_)) => {}
        _ => panic!("Expected InvalidAmount error"),
    }
}

/// Test: Builder rejects self-payment (sender == recipient)
#[test]
fn test_builder_rejects_self_payment() {
    let sender_kp = Keypair::generate();
    let sender_did = Did::from_public_key(&sender_kp.public_key());

    let result = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(sender_did) // Same as sender!
        .amount(100)
        .build();

    assert!(result.is_err(), "Should reject self-payment");
    match result {
        Err(IOUError::SelfPayment) => {}
        _ => panic!("Expected SelfPayment error"),
    }
}

/// Test: Built IOU has correct sender DID derived from keypair
#[test]
fn test_builder_derives_sender_did() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let expected_sender = Did::from_public_key(&sender_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    assert_eq!(
        signed_iou.iou().sender(),
        &expected_sender,
        "Sender DID should be derived from keypair"
    );
}

/// Test: Built IOU is automatically signed
#[test]
fn test_builder_signs_iou() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    // Should have a signature
    assert_eq!(
        signed_iou.signature().as_bytes().len(),
        64,
        "Signature should be 64 bytes"
    );
}

/// Test: Built IOU signature is valid
#[test]
fn test_builder_signature_is_valid() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    assert!(
        signed_iou.verify(&sender_kp.public_key()),
        "Built IOU signature should be valid"
    );
}

/// Test: Building same IOU twice produces same ID but different signatures (due to nonce)
#[test]
fn test_builder_produces_unique_ious() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient.clone())
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    let iou2 = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    // Different nonces mean different IDs
    assert_ne!(
        iou1.iou().id(),
        iou2.iou().id(),
        "Auto-generated IOUs should have different IDs due to unique nonces"
    );
}

/// Test: Builder with same nonce produces same ID
#[test]
fn test_builder_same_nonce_same_id() {
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
        iou1.iou().id(),
        iou2.iou().id(),
        "Same nonce and content should produce same ID"
    );
}

/// Test: Builder handles maximum amount
#[test]
fn test_builder_max_amount() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(u64::MAX)
        .build()
        .expect("Should handle maximum amount");

    assert_eq!(signed_iou.iou().amount(), u64::MAX);
}

/// Test: Builder is chainable
#[test]
fn test_builder_chainable() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    // All methods should be chainable
    let result = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .nonce(12345)
        .timestamp(1703612400)
        .build();

    assert!(result.is_ok());
}
