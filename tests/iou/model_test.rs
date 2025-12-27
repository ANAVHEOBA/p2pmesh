use p2pmesh::identity::{Keypair, Did};
use p2pmesh::iou::{IOU, IOUId};

// ============================================================================
// IOU STRUCTURE TESTS
// ============================================================================

/// Test: IOU has all required fields
#[test]
fn test_iou_has_required_fields() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender = Did::from_public_key(&sender_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou = IOU::new(
        sender.clone(),
        recipient.clone(),
        100,
        12345, // nonce
        1703612400, // timestamp
    );

    assert_eq!(iou.sender(), &sender);
    assert_eq!(iou.recipient(), &recipient);
    assert_eq!(iou.amount(), 100);
    assert_eq!(iou.nonce(), 12345);
    assert_eq!(iou.timestamp(), 1703612400);
}

/// Test: IOU has unique ID derived from content
#[test]
fn test_iou_has_unique_id() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender = Did::from_public_key(&sender_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou = IOU::new(
        sender,
        recipient,
        100,
        12345,
        1703612400,
    );

    let id = iou.id();
    assert_eq!(id.as_bytes().len(), 32, "IOU ID should be 32 bytes (SHA256)");
}

/// Test: Same content produces same ID (deterministic)
#[test]
fn test_iou_id_deterministic() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender = Did::from_public_key(&sender_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOU::new(sender.clone(), recipient.clone(), 100, 12345, 1703612400);
    let iou2 = IOU::new(sender, recipient, 100, 12345, 1703612400);

    assert_eq!(iou1.id(), iou2.id(), "Same content should produce same ID");
}

/// Test: Different nonce produces different ID
#[test]
fn test_different_nonce_different_id() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender = Did::from_public_key(&sender_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOU::new(sender.clone(), recipient.clone(), 100, 11111, 1703612400);
    let iou2 = IOU::new(sender, recipient, 100, 22222, 1703612400);

    assert_ne!(iou1.id(), iou2.id(), "Different nonce should produce different ID");
}

/// Test: Different amount produces different ID
#[test]
fn test_different_amount_different_id() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender = Did::from_public_key(&sender_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOU::new(sender.clone(), recipient.clone(), 100, 12345, 1703612400);
    let iou2 = IOU::new(sender, recipient, 200, 12345, 1703612400);

    assert_ne!(iou1.id(), iou2.id(), "Different amount should produce different ID");
}

/// Test: Different recipient produces different ID
#[test]
fn test_different_recipient_different_id() {
    let sender_kp = Keypair::generate();
    let recipient1_kp = Keypair::generate();
    let recipient2_kp = Keypair::generate();
    let sender = Did::from_public_key(&sender_kp.public_key());
    let recipient1 = Did::from_public_key(&recipient1_kp.public_key());
    let recipient2 = Did::from_public_key(&recipient2_kp.public_key());

    let iou1 = IOU::new(sender.clone(), recipient1, 100, 12345, 1703612400);
    let iou2 = IOU::new(sender, recipient2, 100, 12345, 1703612400);

    assert_ne!(iou1.id(), iou2.id(), "Different recipient should produce different ID");
}

/// Test: Different sender produces different ID
#[test]
fn test_different_sender_different_id() {
    let sender1_kp = Keypair::generate();
    let sender2_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender1 = Did::from_public_key(&sender1_kp.public_key());
    let sender2 = Did::from_public_key(&sender2_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOU::new(sender1, recipient.clone(), 100, 12345, 1703612400);
    let iou2 = IOU::new(sender2, recipient, 100, 12345, 1703612400);

    assert_ne!(iou1.id(), iou2.id(), "Different sender should produce different ID");
}

/// Test: Different timestamp produces different ID
#[test]
fn test_different_timestamp_different_id() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender = Did::from_public_key(&sender_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOU::new(sender.clone(), recipient.clone(), 100, 12345, 1703612400);
    let iou2 = IOU::new(sender, recipient, 100, 12345, 1703612500);

    assert_ne!(iou1.id(), iou2.id(), "Different timestamp should produce different ID");
}

/// Test: IOU ID can be used as HashMap key
#[test]
fn test_iou_id_hashable() {
    use std::collections::HashMap;

    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender = Did::from_public_key(&sender_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou = IOU::new(sender, recipient, 100, 12345, 1703612400);
    let id = iou.id();

    let mut map: HashMap<IOUId, String> = HashMap::new();
    map.insert(id.clone(), "test".to_string());

    assert_eq!(map.get(&id), Some(&"test".to_string()));
}

/// Test: IOU implements Clone
#[test]
fn test_iou_clone() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender = Did::from_public_key(&sender_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOU::new(sender, recipient, 100, 12345, 1703612400);
    let iou2 = iou1.clone();

    assert_eq!(iou1.id(), iou2.id());
    assert_eq!(iou1.amount(), iou2.amount());
}

/// Test: IOU implements PartialEq
#[test]
fn test_iou_equality() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender = Did::from_public_key(&sender_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOU::new(sender.clone(), recipient.clone(), 100, 12345, 1703612400);
    let iou2 = IOU::new(sender, recipient, 100, 12345, 1703612400);

    assert_eq!(iou1, iou2, "IOUs with same content should be equal");
}

/// Test: IOUs with different content are not equal
#[test]
fn test_iou_inequality() {
    let sender_kp = Keypair::generate();
    let recipient_kp = Keypair::generate();
    let sender = Did::from_public_key(&sender_kp.public_key());
    let recipient = Did::from_public_key(&recipient_kp.public_key());

    let iou1 = IOU::new(sender.clone(), recipient.clone(), 100, 12345, 1703612400);
    let iou2 = IOU::new(sender, recipient, 200, 12345, 1703612400);

    assert_ne!(iou1, iou2, "IOUs with different content should not be equal");
}
