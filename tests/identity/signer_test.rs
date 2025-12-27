use p2pmesh::identity::{Keypair, Signature, Signer};

/// Test: Can sign a message
#[test]
fn test_sign_message() {
    let keypair = Keypair::generate();
    let message = b"Hello, P2P Mesh!";

    let signature = Signer::sign(&keypair, message);

    // Signature should be 64 bytes for Ed25519
    assert_eq!(
        signature.as_bytes().len(),
        64,
        "Ed25519 signature should be 64 bytes"
    );
}

/// Test: Can verify a valid signature
#[test]
fn test_verify_valid_signature() {
    let keypair = Keypair::generate();
    let message = b"Hello, P2P Mesh!";

    let signature = Signer::sign(&keypair, message);

    let is_valid = Signer::verify(
        &keypair.public_key(),
        message,
        &signature,
    );

    assert!(is_valid, "Valid signature should verify successfully");
}

/// Test: Tampered message fails verification
#[test]
fn test_tampered_message_fails() {
    let keypair = Keypair::generate();
    let original_message = b"Hello, P2P Mesh!";
    let tampered_message = b"Hello, P2P Mesh?"; // Changed ! to ?

    let signature = Signer::sign(&keypair, original_message);

    let is_valid = Signer::verify(
        &keypair.public_key(),
        tampered_message,
        &signature,
    );

    assert!(!is_valid, "Tampered message should fail verification");
}

/// Test: Wrong public key fails verification
#[test]
fn test_wrong_public_key_fails() {
    let keypair1 = Keypair::generate();
    let keypair2 = Keypair::generate();
    let message = b"Hello, P2P Mesh!";

    let signature = Signer::sign(&keypair1, message);

    let is_valid = Signer::verify(
        &keypair2.public_key(), // Wrong key!
        message,
        &signature,
    );

    assert!(!is_valid, "Wrong public key should fail verification");
}

/// Test: Corrupted signature fails verification
#[test]
fn test_corrupted_signature_fails() {
    let keypair = Keypair::generate();
    let message = b"Hello, P2P Mesh!";

    let signature = Signer::sign(&keypair, message);
    let mut corrupted_bytes = signature.as_bytes().to_vec();
    corrupted_bytes[0] ^= 0xFF; // Flip bits in first byte

    let corrupted_signature = Signature::from_bytes(&corrupted_bytes)
        .expect("Should create signature from bytes");

    let is_valid = Signer::verify(
        &keypair.public_key(),
        message,
        &corrupted_signature,
    );

    assert!(!is_valid, "Corrupted signature should fail verification");
}

/// Test: Empty message can be signed and verified
#[test]
fn test_empty_message() {
    let keypair = Keypair::generate();
    let message = b"";

    let signature = Signer::sign(&keypair, message);

    let is_valid = Signer::verify(
        &keypair.public_key(),
        message,
        &signature,
    );

    assert!(is_valid, "Empty message should be signable and verifiable");
}

/// Test: Large message can be signed and verified
#[test]
fn test_large_message() {
    let keypair = Keypair::generate();
    let message = vec![0xAB; 1_000_000]; // 1MB message

    let signature = Signer::sign(&keypair, &message);

    let is_valid = Signer::verify(
        &keypair.public_key(),
        &message,
        &signature,
    );

    assert!(is_valid, "Large message should be signable and verifiable");
}

/// Test: Signing is deterministic (same message = same signature)
#[test]
fn test_signing_deterministic() {
    let keypair = Keypair::generate();
    let message = b"Hello, P2P Mesh!";

    let signature1 = Signer::sign(&keypair, message);
    let signature2 = Signer::sign(&keypair, message);

    assert_eq!(
        signature1.as_bytes(),
        signature2.as_bytes(),
        "Same keypair + message should produce same signature"
    );
}

/// Test: Different messages produce different signatures
#[test]
fn test_different_messages_different_signatures() {
    let keypair = Keypair::generate();
    let message1 = b"Message 1";
    let message2 = b"Message 2";

    let signature1 = Signer::sign(&keypair, message1);
    let signature2 = Signer::sign(&keypair, message2);

    assert_ne!(
        signature1.as_bytes(),
        signature2.as_bytes(),
        "Different messages should produce different signatures"
    );
}

/// Test: Signature can be serialized and deserialized
#[test]
fn test_signature_serialization() {
    let keypair = Keypair::generate();
    let message = b"Hello, P2P Mesh!";

    let original_sig = Signer::sign(&keypair, message);
    let bytes = original_sig.as_bytes();

    let restored_sig = Signature::from_bytes(bytes)
        .expect("Should deserialize signature from bytes");

    assert_eq!(
        original_sig.as_bytes(),
        restored_sig.as_bytes(),
        "Restored signature should match original"
    );

    // Restored signature should still verify
    let is_valid = Signer::verify(
        &keypair.public_key(),
        message,
        &restored_sig,
    );
    assert!(is_valid, "Restored signature should verify");
}

/// Test: Invalid signature bytes should fail
#[test]
fn test_invalid_signature_bytes_fails() {
    let invalid_bytes = vec![0u8; 32]; // Wrong length (should be 64)

    let result = Signature::from_bytes(&invalid_bytes);
    assert!(result.is_err(), "Invalid signature bytes should fail");
}

/// Test: Binary data can be signed (not just text)
#[test]
fn test_sign_binary_data() {
    let keypair = Keypair::generate();
    let binary_data: Vec<u8> = (0..=255).collect(); // All byte values

    let signature = Signer::sign(&keypair, &binary_data);

    let is_valid = Signer::verify(
        &keypair.public_key(),
        &binary_data,
        &signature,
    );

    assert!(is_valid, "Binary data should be signable and verifiable");
}
