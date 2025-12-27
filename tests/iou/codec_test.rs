use p2pmesh::identity::{Keypair, Did};
use p2pmesh::iou::{IOUBuilder, SignedIOU, IOUCodec};

// ============================================================================
// IOU CODEC (SERIALIZATION) TESTS
// ============================================================================

/// Helper to create a valid signed IOU
fn create_signed_iou() -> SignedIOU {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .nonce(12345)
        .timestamp(1703612400)
        .build()
        .expect("Should build valid IOU")
}

/// Test: Can serialize IOU to bytes
#[test]
fn test_serialize_to_bytes() {
    let signed_iou = create_signed_iou();

    let bytes = IOUCodec::encode(&signed_iou);

    assert!(!bytes.is_empty(), "Serialized bytes should not be empty");
}

/// Test: Can deserialize IOU from bytes
#[test]
fn test_deserialize_from_bytes() {
    let original = create_signed_iou();
    let bytes = IOUCodec::encode(&original);

    let decoded = IOUCodec::decode(&bytes)
        .expect("Should decode valid bytes");

    assert_eq!(original.iou().id(), decoded.iou().id());
}

/// Test: Round-trip preserves all fields
#[test]
fn test_roundtrip_preserves_all_fields() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let original = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(12345678)
        .nonce(87654321)
        .timestamp(1703612400)
        .build()
        .expect("Should build valid IOU");

    let bytes = IOUCodec::encode(&original);
    let decoded = IOUCodec::decode(&bytes)
        .expect("Should decode valid bytes");

    assert_eq!(original.iou().sender(), decoded.iou().sender());
    assert_eq!(original.iou().recipient(), decoded.iou().recipient());
    assert_eq!(original.iou().amount(), decoded.iou().amount());
    assert_eq!(original.iou().nonce(), decoded.iou().nonce());
    assert_eq!(original.iou().timestamp(), decoded.iou().timestamp());
    assert_eq!(original.signature().as_bytes(), decoded.signature().as_bytes());
}

/// Test: Serialized size is reasonable for BLE/LoRa
#[test]
fn test_serialized_size_is_compact() {
    let signed_iou = create_signed_iou();
    let bytes = IOUCodec::encode(&signed_iou);

    // IOU should fit in a LoRa packet (~250 bytes max)
    // Breakdown estimate:
    // - sender DID: ~44 bytes (32 byte key + 12 prefix)
    // - recipient DID: ~44 bytes
    // - amount: 8 bytes (u64)
    // - nonce: 8 bytes (u64)
    // - timestamp: 8 bytes (u64)
    // - signature: 64 bytes
    // - overhead: ~20 bytes
    // Total: ~196 bytes

    assert!(
        bytes.len() <= 250,
        "Serialized IOU should fit in LoRa packet (got {} bytes)",
        bytes.len()
    );

    println!("IOU serialized size: {} bytes", bytes.len());
}

/// Test: Invalid bytes fail to deserialize
#[test]
fn test_invalid_bytes_fail() {
    let invalid_bytes = vec![0u8; 10];

    let result = IOUCodec::decode(&invalid_bytes);
    assert!(result.is_err(), "Invalid bytes should fail to decode");
}

/// Test: Empty bytes fail to deserialize
#[test]
fn test_empty_bytes_fail() {
    let result = IOUCodec::decode(&[]);
    assert!(result.is_err(), "Empty bytes should fail to decode");
}

/// Test: Truncated bytes fail to deserialize
#[test]
fn test_truncated_bytes_fail() {
    let signed_iou = create_signed_iou();
    let bytes = IOUCodec::encode(&signed_iou);

    // Truncate the bytes
    let truncated = &bytes[..bytes.len() / 2];

    let result = IOUCodec::decode(truncated);
    assert!(result.is_err(), "Truncated bytes should fail to decode");
}

/// Test: Extra bytes after valid data are rejected or ignored
#[test]
fn test_extra_bytes_handling() {
    let signed_iou = create_signed_iou();
    let mut bytes = IOUCodec::encode(&signed_iou);

    // Add extra garbage bytes
    bytes.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);

    // Depending on implementation, this should either:
    // 1. Fail (strict parsing)
    // 2. Succeed but ignore extra bytes (lenient parsing)
    let result = IOUCodec::decode(&bytes);

    // We'll accept either behavior, but the decoded IOU should be valid if it succeeds
    if let Ok(decoded) = result {
        assert_eq!(signed_iou.iou().id(), decoded.iou().id());
    }
}

/// Test: Corrupted bytes in middle fail to deserialize
#[test]
fn test_corrupted_bytes_fail() {
    let signed_iou = create_signed_iou();
    let mut bytes = IOUCodec::encode(&signed_iou);

    // Corrupt bytes in the middle
    if bytes.len() > 50 {
        bytes[50] ^= 0xFF;
    }

    let result = IOUCodec::decode(&bytes);

    // Might succeed in parsing but signature will be invalid
    if let Ok(decoded) = result {
        // The IOU should parse but data will be different
        // (unless we're very unlucky and the corruption preserves validity)
        assert!(
            decoded.iou().id() != signed_iou.iou().id() ||
            decoded.signature().as_bytes() != signed_iou.signature().as_bytes(),
            "Corrupted bytes should produce different data"
        );
    }
}

/// Test: Maximum amount serializes correctly
#[test]
fn test_max_amount_serializes() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let original = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(u64::MAX)
        .build()
        .expect("Should build valid IOU");

    let bytes = IOUCodec::encode(&original);
    let decoded = IOUCodec::decode(&bytes)
        .expect("Should decode valid bytes");

    assert_eq!(decoded.iou().amount(), u64::MAX);
}

/// Test: Zero nonce serializes correctly
#[test]
fn test_zero_nonce_serializes() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    // Note: nonce(0) is allowed for serialization even if builder rejects it
    let original = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .nonce(0)
        .build()
        .expect("Should build valid IOU");

    let bytes = IOUCodec::encode(&original);
    let decoded = IOUCodec::decode(&bytes)
        .expect("Should decode valid bytes");

    assert_eq!(decoded.iou().nonce(), 0);
}

/// Test: Encode is deterministic
#[test]
fn test_encode_deterministic() {
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

    let bytes1 = IOUCodec::encode(&iou1);
    let bytes2 = IOUCodec::encode(&iou2);

    assert_eq!(bytes1, bytes2, "Same IOU should produce same bytes");
}

/// Test: Can encode to hex string
#[test]
fn test_encode_to_hex() {
    let signed_iou = create_signed_iou();

    let hex = IOUCodec::encode_hex(&signed_iou);

    assert!(!hex.is_empty(), "Hex string should not be empty");
    assert!(
        hex.chars().all(|c| c.is_ascii_hexdigit()),
        "Should be valid hex"
    );
}

/// Test: Can decode from hex string
#[test]
fn test_decode_from_hex() {
    let original = create_signed_iou();
    let hex = IOUCodec::encode_hex(&original);

    let decoded = IOUCodec::decode_hex(&hex)
        .expect("Should decode valid hex");

    assert_eq!(original.iou().id(), decoded.iou().id());
}

/// Test: Invalid hex fails to decode
#[test]
fn test_invalid_hex_fails() {
    let result = IOUCodec::decode_hex("not-valid-hex!!!");
    assert!(result.is_err(), "Invalid hex should fail to decode");
}

/// Test: Encode to base64 for transport
#[test]
fn test_encode_to_base64() {
    let signed_iou = create_signed_iou();

    let b64 = IOUCodec::encode_base64(&signed_iou);

    assert!(!b64.is_empty(), "Base64 string should not be empty");
}

/// Test: Decode from base64
#[test]
fn test_decode_from_base64() {
    let original = create_signed_iou();
    let b64 = IOUCodec::encode_base64(&original);

    let decoded = IOUCodec::decode_base64(&b64)
        .expect("Should decode valid base64");

    assert_eq!(original.iou().id(), decoded.iou().id());
}

/// Test: Multiple IOUs serialize to different bytes
#[test]
fn test_different_ious_different_bytes() {
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
        .amount(200) // Different amount
        .build()
        .expect("Should build valid IOU");

    let bytes1 = IOUCodec::encode(&iou1);
    let bytes2 = IOUCodec::encode(&iou2);

    assert_ne!(bytes1, bytes2, "Different IOUs should produce different bytes");
}

/// Test: Serialization preserves signature validity
#[test]
fn test_serialization_preserves_signature() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let original = IOUBuilder::new()
        .sender(&sender_kp)
        .recipient(recipient)
        .amount(100)
        .build()
        .expect("Should build valid IOU");

    // Verify original
    assert!(original.verify(&sender_kp.public_key()));

    // Serialize and deserialize
    let bytes = IOUCodec::encode(&original);
    let decoded = IOUCodec::decode(&bytes).expect("Should decode");

    // Verify decoded
    assert!(
        decoded.verify(&sender_kp.public_key()),
        "Deserialized IOU should still have valid signature"
    );
}
