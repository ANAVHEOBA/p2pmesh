use p2pmesh::identity::{Keypair, Did, Signer, Signature};
use p2pmesh::iou::{IOU, SignedIOU, IOUBuilder, IOUValidator, ValidationError};

// ============================================================================
// IOU VALIDATOR TESTS
// ============================================================================

/// Helper to create a valid signed IOU
fn create_valid_signed_iou() -> (SignedIOU, Keypair, Keypair) {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    (signed_iou, sender_kp, recipient_kp)
}

/// Test: Valid IOU passes validation
#[test]
fn test_valid_iou_passes() {
    let (signed_iou, sender_kp, _) = create_valid_signed_iou();

    let result = IOUValidator::validate(&signed_iou, &sender_kp.public_key());
    assert!(result.is_ok(), "Valid IOU should pass validation");
}

/// Test: Tampered amount fails validation
#[test]
fn test_tampered_amount_fails() {
    let (signed_iou, sender_kp, recipient_kp) = create_valid_signed_iou();

    // Create a new IOU with different amount but same signature
    let tampered_iou = IOU::new(
        signed_iou.iou().sender().clone(),
        signed_iou.iou().recipient().clone(),
        999, // Tampered amount!
        signed_iou.iou().nonce(),
        signed_iou.iou().timestamp(),
    );

    let tampered_signed = SignedIOU::from_parts(
        tampered_iou,
        signed_iou.signature().clone(),
    );

    let result = IOUValidator::validate(&tampered_signed, &sender_kp.public_key());
    assert!(result.is_err(), "Tampered amount should fail validation");
    match result {
        Err(ValidationError::InvalidSignature) => {}
        _ => panic!("Expected InvalidSignature error"),
    }
}

/// Test: Tampered recipient fails validation
#[test]
fn test_tampered_recipient_fails() {
    let (signed_iou, sender_kp, _) = create_valid_signed_iou();
    let other_kp = Keypair::generate();
    let other_did = Did::from_public_key(&other_kp.public_key());

    let tampered_iou = IOU::new(
        signed_iou.iou().sender().clone(),
        other_did, // Tampered recipient!
        signed_iou.iou().amount(),
        signed_iou.iou().nonce(),
        signed_iou.iou().timestamp(),
    );

    let tampered_signed = SignedIOU::from_parts(
        tampered_iou,
        signed_iou.signature().clone(),
    );

    let result = IOUValidator::validate(&tampered_signed, &sender_kp.public_key());
    assert!(result.is_err(), "Tampered recipient should fail validation");
}

/// Test: Tampered sender fails validation
#[test]
fn test_tampered_sender_fails() {
    let (signed_iou, sender_kp, _) = create_valid_signed_iou();
    let other_kp = Keypair::generate();
    let other_did = Did::from_public_key(&other_kp.public_key());

    let tampered_iou = IOU::new(
        other_did, // Tampered sender!
        signed_iou.iou().recipient().clone(),
        signed_iou.iou().amount(),
        signed_iou.iou().nonce(),
        signed_iou.iou().timestamp(),
    );

    let tampered_signed = SignedIOU::from_parts(
        tampered_iou,
        signed_iou.signature().clone(),
    );

    let result = IOUValidator::validate(&tampered_signed, &sender_kp.public_key());
    assert!(result.is_err(), "Tampered sender should fail validation");
}

/// Test: Tampered nonce fails validation
#[test]
fn test_tampered_nonce_fails() {
    let (signed_iou, sender_kp, _) = create_valid_signed_iou();

    let tampered_iou = IOU::new(
        signed_iou.iou().sender().clone(),
        signed_iou.iou().recipient().clone(),
        signed_iou.iou().amount(),
        99999999, // Tampered nonce!
        signed_iou.iou().timestamp(),
    );

    let tampered_signed = SignedIOU::from_parts(
        tampered_iou,
        signed_iou.signature().clone(),
    );

    let result = IOUValidator::validate(&tampered_signed, &sender_kp.public_key());
    assert!(result.is_err(), "Tampered nonce should fail validation");
}

/// Test: Tampered timestamp fails validation
#[test]
fn test_tampered_timestamp_fails() {
    let (signed_iou, sender_kp, _) = create_valid_signed_iou();

    let tampered_iou = IOU::new(
        signed_iou.iou().sender().clone(),
        signed_iou.iou().recipient().clone(),
        signed_iou.iou().amount(),
        signed_iou.iou().nonce(),
        0, // Tampered timestamp!
    );

    let tampered_signed = SignedIOU::from_parts(
        tampered_iou,
        signed_iou.signature().clone(),
    );

    let result = IOUValidator::validate(&tampered_signed, &sender_kp.public_key());
    assert!(result.is_err(), "Tampered timestamp should fail validation");
}

/// Test: Wrong public key fails validation
#[test]
fn test_wrong_public_key_fails() {
    let (signed_iou, _, _) = create_valid_signed_iou();
    let wrong_kp = Keypair::generate();

    let result = IOUValidator::validate(&signed_iou, &wrong_kp.public_key());
    assert!(result.is_err(), "Wrong public key should fail validation");
    // Can fail with either SenderMismatch (DID doesn't match key) or InvalidSignature
    match result {
        Err(ValidationError::InvalidSignature) => {}
        Err(ValidationError::SenderMismatch) => {}
        _ => panic!("Expected InvalidSignature or SenderMismatch error"),
    }
}

/// Test: Corrupted signature fails validation
#[test]
fn test_corrupted_signature_fails() {
    let (signed_iou, sender_kp, _) = create_valid_signed_iou();

    // Corrupt the signature
    let mut sig_bytes = signed_iou.signature().as_bytes().to_vec();
    sig_bytes[0] ^= 0xFF;
    let corrupted_sig = Signature::from_bytes(&sig_bytes).expect("Should create signature");

    let corrupted_signed = SignedIOU::from_parts(
        signed_iou.iou().clone(),
        corrupted_sig,
    );

    let result = IOUValidator::validate(&corrupted_signed, &sender_kp.public_key());
    assert!(result.is_err(), "Corrupted signature should fail validation");
}

/// Test: Validate checks sender DID matches public key
#[test]
fn test_sender_did_must_match_public_key() {
    let sender_kp = Keypair::generate();
    let wrong_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    // Build IOU with sender_kp
    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    // Try to validate with wrong public key
    let result = IOUValidator::validate(&signed_iou, &wrong_kp.public_key());
    assert!(result.is_err(), "Sender DID must match public key used for validation");
}

/// Test: Validation with matching sender DID and public key succeeds
#[test]
fn test_sender_did_matches_public_key_succeeds() {
    let (signed_iou, sender_kp, _) = create_valid_signed_iou();

    // Validate with correct public key
    let result = IOUValidator::validate(&signed_iou, &sender_kp.public_key());
    assert!(result.is_ok(), "Matching sender DID and public key should succeed");
}

/// Test: Validator detects future timestamp (clock skew protection)
#[test]
fn test_future_timestamp_rejected() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let future_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() + 3600; // 1 hour in the future

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .timestamp(future_time)
        .build()
        .expect("Should build IOU");

    let result = IOUValidator::validate_with_time_check(
        &signed_iou,
        &sender_kp.public_key(),
        300, // 5 minute tolerance
    );

    assert!(result.is_err(), "Future timestamp should be rejected");
    match result {
        Err(ValidationError::FutureTimestamp) => {}
        _ => panic!("Expected FutureTimestamp error"),
    }
}

/// Test: Validator accepts timestamp within tolerance
#[test]
fn test_timestamp_within_tolerance_accepted() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let slight_future = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() + 60; // 1 minute in the future

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .timestamp(slight_future)
        .build()
        .expect("Should build IOU");

    let result = IOUValidator::validate_with_time_check(
        &signed_iou,
        &sender_kp.public_key(),
        300, // 5 minute tolerance
    );

    assert!(result.is_ok(), "Timestamp within tolerance should be accepted");
}

/// Test: Validator detects expired IOU
#[test]
fn test_expired_iou_rejected() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let old_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() - 86400; // 1 day ago

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .timestamp(old_time)
        .build()
        .expect("Should build IOU");

    let result = IOUValidator::validate_with_expiry(
        &signed_iou,
        &sender_kp.public_key(),
        3600, // 1 hour expiry
    );

    assert!(result.is_err(), "Expired IOU should be rejected");
    match result {
        Err(ValidationError::Expired) => {}
        _ => panic!("Expected Expired error"),
    }
}

/// Test: Basic validation doesn't check timestamp
#[test]
fn test_basic_validation_ignores_timestamp() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let old_time = 1000; // Very old timestamp

    let signed_iou = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .timestamp(old_time)
        .build()
        .expect("Should build IOU");

    // Basic validation should still pass
    let result = IOUValidator::validate(&signed_iou, &sender_kp.public_key());
    assert!(result.is_ok(), "Basic validation should ignore timestamp age");
}

/// Test: Validate returns the verified IOU on success
#[test]
fn test_validate_returns_iou_on_success() {
    let (signed_iou, sender_kp, _) = create_valid_signed_iou();

    let result = IOUValidator::validate(&signed_iou, &sender_kp.public_key());
    match result {
        Ok(verified_iou) => {
            assert_eq!(verified_iou.id(), signed_iou.iou().id());
        }
        Err(e) => panic!("Validation should succeed: {:?}", e),
    }
}

/// Test: Self-payment IOU fails validation
#[test]
fn test_self_payment_fails_validation() {
    let sender_kp = Keypair::generate();
    let sender_did = Did::from_public_key(&sender_kp.public_key());

    // Manually construct a self-payment IOU (bypassing builder check)
    let iou = IOU::new(
        sender_did.clone(),
        sender_did, // Same as sender!
        100,
        12345,
        1703612400,
    );

    let signature = Signer::sign(&sender_kp, &iou.to_signing_bytes());
    let signed_iou = SignedIOU::from_parts(iou, signature);

    let result = IOUValidator::validate(&signed_iou, &sender_kp.public_key());
    assert!(result.is_err(), "Self-payment should fail validation");
    match result {
        Err(ValidationError::SelfPayment) => {}
        _ => panic!("Expected SelfPayment error"),
    }
}

/// Test: Zero amount IOU fails validation
#[test]
fn test_zero_amount_fails_validation() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender_did = Did::from_public_key(&sender_kp.public_key());
    let recipient_did = Did::from_public_key(&recipient_kp.public_key());

    // Manually construct a zero-amount IOU (bypassing builder check)
    let iou = IOU::new(
        sender_did,
        recipient_did,
        0, // Zero amount!
        12345,
        1703612400,
    );

    let signature = Signer::sign(&sender_kp, &iou.to_signing_bytes());
    let signed_iou = SignedIOU::from_parts(iou, signature);

    let result = IOUValidator::validate(&signed_iou, &sender_kp.public_key());
    assert!(result.is_err(), "Zero amount should fail validation");
    match result {
        Err(ValidationError::InvalidAmount) => {}
        _ => panic!("Expected InvalidAmount error"),
    }
}
