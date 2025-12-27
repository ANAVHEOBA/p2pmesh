use p2pmesh::identity::{Keypair, Did, PublicKey};

/// Test: Can create DID from public key
#[test]
fn test_did_from_public_key() {
    let keypair = Keypair::generate();
    let did = Did::from_public_key(&keypair.public_key());

    // DID should start with "did:mesh:"
    let did_string = did.to_string();
    assert!(
        did_string.starts_with("did:mesh:"),
        "DID should start with 'did:mesh:', got: {}",
        did_string
    );
}

/// Test: DID string has correct format
#[test]
fn test_did_format() {
    let keypair = Keypair::generate();
    let did = Did::from_public_key(&keypair.public_key());
    let did_string = did.to_string();

    // Format: did:mesh:<base58_encoded_public_key>
    let parts: Vec<&str> = did_string.split(':').collect();
    assert_eq!(parts.len(), 3, "DID should have 3 parts separated by ':'");
    assert_eq!(parts[0], "did", "First part should be 'did'");
    assert_eq!(parts[1], "mesh", "Second part should be 'mesh'");
    assert!(!parts[2].is_empty(), "Third part (key) should not be empty");
}

/// Test: Can parse DID string back to DID object
#[test]
fn test_did_parse() {
    let keypair = Keypair::generate();
    let original_did = Did::from_public_key(&keypair.public_key());
    let did_string = original_did.to_string();

    let parsed_did = Did::parse(&did_string)
        .expect("Should parse valid DID string");

    assert_eq!(
        original_did.to_string(),
        parsed_did.to_string(),
        "Parsed DID should match original"
    );
}

/// Test: Can extract public key from DID
#[test]
fn test_did_to_public_key() {
    let keypair = Keypair::generate();
    let original_pubkey = keypair.public_key();
    let did = Did::from_public_key(&original_pubkey);

    let extracted_pubkey = did.public_key()
        .expect("Should extract public key from DID");

    assert_eq!(
        original_pubkey.as_bytes(),
        extracted_pubkey.as_bytes(),
        "Extracted public key should match original"
    );
}

/// Test: Same public key produces same DID
#[test]
fn test_did_deterministic() {
    let keypair = Keypair::generate();
    let did1 = Did::from_public_key(&keypair.public_key());
    let did2 = Did::from_public_key(&keypair.public_key());

    assert_eq!(
        did1.to_string(),
        did2.to_string(),
        "Same public key should produce same DID"
    );
}

/// Test: Different public keys produce different DIDs
#[test]
fn test_different_keys_different_dids() {
    let keypair1 = Keypair::generate();
    let keypair2 = Keypair::generate();

    let did1 = Did::from_public_key(&keypair1.public_key());
    let did2 = Did::from_public_key(&keypair2.public_key());

    assert_ne!(
        did1.to_string(),
        did2.to_string(),
        "Different public keys should produce different DIDs"
    );
}

/// Test: Invalid DID format should fail to parse
#[test]
fn test_invalid_did_format_fails() {
    let invalid_dids = vec![
        "",                          // Empty
        "did",                       // Incomplete
        "did:mesh",                  // Missing key part
        "did:mesh:",                 // Empty key part
        "did:other:abc123",          // Wrong method (not 'mesh')
        "abc:mesh:key123",           // Wrong scheme (not 'did')
        "did:mesh:!!!invalid",       // Invalid base58 characters
    ];

    for invalid in invalid_dids {
        let result = Did::parse(invalid);
        assert!(
            result.is_err(),
            "Should reject invalid DID: '{}'",
            invalid
        );
    }
}

/// Test: DID is case-sensitive
#[test]
fn test_did_case_sensitive() {
    let keypair = Keypair::generate();
    let did = Did::from_public_key(&keypair.public_key());
    let did_string = did.to_string();

    // Uppercase version should fail or produce different result
    let uppercase = did_string.to_uppercase();
    if uppercase != did_string {
        let result = Did::parse(&uppercase);
        // Either fails to parse or parses to different key
        if let Ok(parsed) = result {
            // If it parses, public key extraction should fail or differ
            if let Ok(pubkey) = parsed.public_key() {
                assert_ne!(
                    keypair.public_key().as_bytes(),
                    pubkey.as_bytes(),
                    "Uppercase DID should not match original"
                );
            }
        }
    }
}

/// Test: DID implements equality
#[test]
fn test_did_equality() {
    let keypair = Keypair::generate();
    let did1 = Did::from_public_key(&keypair.public_key());
    let did2 = Did::from_public_key(&keypair.public_key());

    assert_eq!(did1, did2, "DIDs from same key should be equal");
}

/// Test: DID can be used as identifier (hashable)
#[test]
fn test_did_hashable() {
    use std::collections::HashMap;

    let keypair = Keypair::generate();
    let did = Did::from_public_key(&keypair.public_key());

    let mut map: HashMap<Did, u64> = HashMap::new();
    map.insert(did.clone(), 100);

    assert_eq!(map.get(&did), Some(&100), "DID should be usable as HashMap key");
}
