use p2pmesh::identity::{Keypair, PublicKey, SecretKey};

/// Test: Can generate a new keypair
#[test]
fn test_generate_keypair() {
    let keypair = Keypair::generate();

    // Should have both public and secret keys
    let _public = keypair.public_key();
    let _secret = keypair.secret_key();
}

/// Test: Each generated keypair should be unique
#[test]
fn test_keypairs_are_unique() {
    let keypair1 = Keypair::generate();
    let keypair2 = Keypair::generate();

    // Public keys should be different
    assert_ne!(
        keypair1.public_key().as_bytes(),
        keypair2.public_key().as_bytes(),
        "Two generated keypairs should have different public keys"
    );
}

/// Test: Public key has correct length (32 bytes for Ed25519)
#[test]
fn test_public_key_length() {
    let keypair = Keypair::generate();
    let public_key = keypair.public_key();

    assert_eq!(
        public_key.as_bytes().len(),
        32,
        "Ed25519 public key should be 32 bytes"
    );
}

/// Test: Can serialize keypair to bytes and restore it
#[test]
fn test_keypair_serialization() {
    let original = Keypair::generate();
    let bytes = original.to_bytes();

    let restored = Keypair::from_bytes(&bytes)
        .expect("Should deserialize keypair from bytes");

    assert_eq!(
        original.public_key().as_bytes(),
        restored.public_key().as_bytes(),
        "Restored keypair should have same public key"
    );
}

/// Test: Can serialize just the public key
#[test]
fn test_public_key_serialization() {
    let keypair = Keypair::generate();
    let public_key = keypair.public_key();

    let bytes = public_key.as_bytes();
    let restored = PublicKey::from_bytes(bytes)
        .expect("Should deserialize public key from bytes");

    assert_eq!(
        public_key.as_bytes(),
        restored.as_bytes(),
        "Restored public key should match original"
    );
}

/// Test: Invalid bytes should fail to deserialize
#[test]
fn test_invalid_keypair_bytes_fails() {
    let invalid_bytes = vec![0u8; 10]; // Wrong length

    let result = Keypair::from_bytes(&invalid_bytes);
    assert!(result.is_err(), "Invalid bytes should fail to deserialize");
}

/// Test: Invalid public key bytes should fail
#[test]
fn test_invalid_public_key_bytes_fails() {
    let invalid_bytes = [0u8; 16]; // Wrong length (should be 32)

    let result = PublicKey::from_bytes(&invalid_bytes);
    assert!(result.is_err(), "Invalid public key bytes should fail");
}

/// Test: Keypair can be created from existing secret key
#[test]
fn test_keypair_from_secret_key() {
    let original = Keypair::generate();
    let secret_bytes = original.secret_key().to_bytes();

    let secret = SecretKey::from_bytes(&secret_bytes)
        .expect("Should create secret key from bytes");
    let restored = Keypair::from_secret_key(secret);

    assert_eq!(
        original.public_key().as_bytes(),
        restored.public_key().as_bytes(),
        "Keypair from same secret should have same public key"
    );
}
