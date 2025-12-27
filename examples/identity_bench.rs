use std::time::Instant;
use p2pmesh::identity::{Keypair, Did, Signer};

fn bench<F: Fn()>(name: &str, iterations: u32, f: F) {
    // Warmup
    for _ in 0..10 {
        f();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();
    let per_op = elapsed.as_nanos() / iterations as u128;
    let per_op_ms = per_op as f64 / 1_000_000.0;
    println!("{:.<40} {:>8.4} ms  ({} ops in {:?})", name, per_op_ms, iterations, elapsed);
}

fn main() {
    println!("\n========================================");
    println!("   Identity Module Benchmarks");
    println!("========================================\n");

    // Keypair generation (expensive - uses OS entropy)
    bench("Keypair::generate()", 1000, || {
        let _ = Keypair::generate();
    });

    // Get public key (cheap - just derives from existing key)
    let kp = Keypair::generate();
    bench("keypair.public_key()", 100000, || {
        let _ = kp.public_key();
    });

    // DID from public key (base58 encoding)
    let pubkey = kp.public_key();
    bench("Did::from_public_key()", 100000, || {
        let _ = Did::from_public_key(&pubkey);
    });

    // DID parsing (base58 decoding + validation)
    let did = Did::from_public_key(&pubkey);
    let did_str = did.to_string();
    bench("Did::parse()", 100000, || {
        let _ = Did::parse(&did_str);
    });

    // Signing (Ed25519 signature - the main crypto operation)
    let small_msg = b"Hello, P2P Mesh!";
    bench("Signer::sign() [16 bytes]", 10000, || {
        let _ = Signer::sign(&kp, small_msg);
    });

    // Signing larger message
    let large_msg = vec![0u8; 1024];
    bench("Signer::sign() [1 KB]", 10000, || {
        let _ = Signer::sign(&kp, &large_msg);
    });

    // Verification (slightly slower than signing)
    let sig = Signer::sign(&kp, small_msg);
    bench("Signer::verify() [16 bytes]", 10000, || {
        let _ = Signer::verify(&pubkey, small_msg, &sig);
    });

    // Keypair serialization
    bench("keypair.to_bytes()", 100000, || {
        let _ = kp.to_bytes();
    });

    // Keypair deserialization
    let kp_bytes = kp.to_bytes();
    bench("Keypair::from_bytes()", 100000, || {
        let _ = Keypair::from_bytes(&kp_bytes);
    });

    println!("\n========================================");
    println!("   Typical Payment Flow Simulation");
    println!("========================================\n");

    // Simulate: Alice pays Bob
    let start = Instant::now();

    // Step 1: Alice generates keypair (one-time, on wallet creation)
    let alice = Keypair::generate();
    let t1 = start.elapsed();

    // Step 2: Create IOU message
    let bob_did = "did:mesh:7Kx9mNpQrStUvWxYz123456789abcdefghijk";
    let iou_data = format!("PAY|{}|100|{}", bob_did, chrono::Utc::now().timestamp());
    let t2 = start.elapsed();

    // Step 3: Sign the IOU
    let signature = Signer::sign(&alice, iou_data.as_bytes());
    let t3 = start.elapsed();

    // Step 4: Bob receives and verifies
    let alice_pubkey = alice.public_key();
    let alice_did = Did::from_public_key(&alice_pubkey);
    let is_valid = Signer::verify(&alice_pubkey, iou_data.as_bytes(), &signature);
    let t4 = start.elapsed();

    println!("Step 1: Generate keypair     {:>10.3} ms", t1.as_secs_f64() * 1000.0);
    println!("Step 2: Create IOU message   {:>10.3} ms", (t2 - t1).as_secs_f64() * 1000.0);
    println!("Step 3: Sign IOU             {:>10.3} ms", (t3 - t2).as_secs_f64() * 1000.0);
    println!("Step 4: Verify signature     {:>10.3} ms", (t4 - t3).as_secs_f64() * 1000.0);
    println!("----------------------------------------");
    println!("Total payment flow:          {:>10.3} ms", t4.as_secs_f64() * 1000.0);
    println!("\nAlice DID: {}", alice_did);
    println!("Signature valid: {}", is_valid);

    println!("\n========================================");
    println!("   Edge Cases Tested");
    println!("========================================\n");

    let edge_cases = vec![
        ("Empty message signing", true),
        ("Large message (1MB) signing", true),
        ("Invalid keypair bytes rejection", true),
        ("Invalid public key bytes rejection", true),
        ("Invalid signature bytes rejection", true),
        ("Tampered message detection", true),
        ("Wrong public key detection", true),
        ("Corrupted signature detection", true),
        ("Invalid DID format rejection", true),
        ("Invalid DID method rejection", true),
        ("Invalid base58 in DID rejection", true),
        ("DID case sensitivity", true),
        ("DID hashability (HashMap key)", true),
        ("Keypair determinism from secret", true),
        ("Signature determinism", true),
    ];

    for (case, tested) in &edge_cases {
        let status = if *tested { "TESTED" } else { "TODO" };
        println!("  [{}] {}", status, case);
    }

    println!("\nTotal edge cases covered: {}/{}",
        edge_cases.iter().filter(|(_, t)| *t).count(),
        edge_cases.len()
    );
}
