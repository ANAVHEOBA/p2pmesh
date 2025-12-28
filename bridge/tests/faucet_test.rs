// Faucet tests for the bridge module
// Tests the offline funding mechanism for hackathon demo

use p2pmesh_bridge::{
    create_wallet, faucet_did, faucet_public_key, fund_wallet_from_faucet, request_from_faucet,
};

// ============================================================================
// FAUCET IDENTITY TESTS
// ============================================================================

#[test]
fn test_faucet_public_key_is_32_bytes() {
    let pubkey = faucet_public_key();
    assert_eq!(pubkey.len(), 32, "Faucet public key should be 32 bytes");
}

#[test]
fn test_faucet_public_key_is_deterministic() {
    let pubkey1 = faucet_public_key();
    let pubkey2 = faucet_public_key();
    assert_eq!(pubkey1, pubkey2, "Faucet public key should be deterministic");
}

#[test]
fn test_faucet_did_starts_with_prefix() {
    let did = faucet_did();
    assert!(
        did.starts_with("did:mesh:"),
        "Faucet DID should start with 'did:mesh:', got: {}",
        did
    );
}

#[test]
fn test_faucet_did_is_deterministic() {
    let did1 = faucet_did();
    let did2 = faucet_did();
    assert_eq!(did1, did2, "Faucet DID should be deterministic");
}

// ============================================================================
// REQUEST FROM FAUCET TESTS
// ============================================================================

#[test]
fn test_request_from_faucet_returns_signed_iou() {
    let wallet = create_wallet().unwrap();
    let iou = request_from_faucet(wallet.did(), 1000).unwrap();

    assert_eq!(iou.amount(), 1000, "IOU amount should match request");
}

#[test]
fn test_request_from_faucet_sender_is_faucet() {
    let wallet = create_wallet().unwrap();
    let iou = request_from_faucet(wallet.did(), 500).unwrap();

    assert_eq!(
        iou.sender(),
        faucet_did(),
        "IOU sender should be the faucet DID"
    );
}

#[test]
fn test_request_from_faucet_recipient_matches() {
    let wallet = create_wallet().unwrap();
    let wallet_did = wallet.did();
    let iou = request_from_faucet(wallet_did.clone(), 100).unwrap();

    assert_eq!(
        iou.recipient(),
        wallet_did,
        "IOU recipient should match the wallet DID"
    );
}

#[test]
fn test_request_from_faucet_signature_is_valid() {
    let wallet = create_wallet().unwrap();
    let iou = request_from_faucet(wallet.did(), 100).unwrap();

    assert!(iou.verify().unwrap(), "IOU signature should be valid");
}

#[test]
fn test_request_from_faucet_zero_amount_fails() {
    let wallet = create_wallet().unwrap();
    let result = request_from_faucet(wallet.did(), 0);

    assert!(result.is_err(), "Zero amount should fail");
}

#[test]
fn test_request_from_faucet_invalid_did_fails() {
    let result = request_from_faucet("invalid-did".to_string(), 100);
    assert!(result.is_err(), "Invalid DID should fail");
}

#[test]
fn test_request_from_faucet_multiple_requests_have_unique_ids() {
    let wallet = create_wallet().unwrap();

    let iou1 = request_from_faucet(wallet.did(), 100).unwrap();
    let iou2 = request_from_faucet(wallet.did(), 100).unwrap();

    assert_ne!(
        iou1.id(),
        iou2.id(),
        "Multiple requests should have unique IOU IDs"
    );
}

// ============================================================================
// FUND WALLET TESTS
// ============================================================================

#[test]
fn test_fund_wallet_increases_balance() {
    let wallet = create_wallet().unwrap();
    assert_eq!(wallet.balance(), 0, "New wallet should have zero balance");

    fund_wallet_from_faucet(wallet.clone(), 1000).unwrap();

    assert_eq!(wallet.balance(), 1000, "Balance should be 1000 after funding");
}

#[test]
fn test_fund_wallet_multiple_times() {
    let wallet = create_wallet().unwrap();

    fund_wallet_from_faucet(wallet.clone(), 500).unwrap();
    fund_wallet_from_faucet(wallet.clone(), 300).unwrap();
    fund_wallet_from_faucet(wallet.clone(), 200).unwrap();

    assert_eq!(
        wallet.balance(),
        1000,
        "Balance should accumulate from multiple fundings"
    );
}

#[test]
fn test_fund_wallet_zero_amount_fails() {
    let wallet = create_wallet().unwrap();
    let result = fund_wallet_from_faucet(wallet, 0);

    assert!(result.is_err(), "Zero amount funding should fail");
}

#[test]
fn test_fund_wallet_large_amount() {
    let wallet = create_wallet().unwrap();

    fund_wallet_from_faucet(wallet.clone(), 1_000_000_000).unwrap();

    assert_eq!(
        wallet.balance(),
        1_000_000_000,
        "Should handle large amounts"
    );
}

// ============================================================================
// INTEGRATION TESTS - FAUCET TO PAYMENT FLOW
// ============================================================================

#[test]
fn test_faucet_funded_wallet_can_send_payment() {
    let alice = create_wallet().unwrap();
    let bob = create_wallet().unwrap();

    // Fund Alice from faucet
    fund_wallet_from_faucet(alice.clone(), 1000).unwrap();
    assert_eq!(alice.balance(), 1000);

    // Alice creates payment to Bob
    let iou = alice.create_payment(bob.did(), 300).unwrap();
    alice.mark_sent(iou.clone()).unwrap();

    assert_eq!(alice.balance(), 700, "Alice should have 700 after sending 300");
}

#[test]
fn test_faucet_funded_wallet_payment_can_be_received() {
    let alice = create_wallet().unwrap();
    let bob = create_wallet().unwrap();

    // Fund Alice
    fund_wallet_from_faucet(alice.clone(), 1000).unwrap();

    // Alice sends to Bob
    let iou = alice.create_payment(bob.did(), 400).unwrap();
    alice.mark_sent(iou.clone()).unwrap();

    // Bob processes the payment
    bob.process_payment(iou).unwrap();

    assert_eq!(bob.balance(), 400, "Bob should have 400 after receiving");
    assert_eq!(alice.balance(), 600, "Alice should have 600 after sending");
}

#[test]
fn test_full_payment_cycle_with_faucet() {
    let alice = create_wallet().unwrap();
    let bob = create_wallet().unwrap();

    // Both get funded
    fund_wallet_from_faucet(alice.clone(), 1000).unwrap();
    fund_wallet_from_faucet(bob.clone(), 500).unwrap();

    // Alice sends 200 to Bob
    let iou1 = alice.create_payment(bob.did(), 200).unwrap();
    alice.mark_sent(iou1.clone()).unwrap();
    bob.process_payment(iou1).unwrap();

    // Bob sends 100 back to Alice
    let iou2 = bob.create_payment(alice.did(), 100).unwrap();
    bob.mark_sent(iou2.clone()).unwrap();
    alice.process_payment(iou2).unwrap();

    assert_eq!(alice.balance(), 900, "Alice: 1000 - 200 + 100 = 900");
    assert_eq!(bob.balance(), 600, "Bob: 500 + 200 - 100 = 600");
}

#[test]
fn test_insufficient_balance_after_faucet_funding() {
    let alice = create_wallet().unwrap();
    let bob = create_wallet().unwrap();

    // Fund with small amount
    fund_wallet_from_faucet(alice.clone(), 100).unwrap();

    // Try to send more than balance
    let result = alice.create_payment(bob.did(), 200);

    assert!(result.is_err(), "Should fail when trying to send more than balance");
}
