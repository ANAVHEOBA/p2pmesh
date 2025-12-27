use crate::identity::{Did, PublicKey, Signature, Signer};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::hash::{Hash, Hasher};

/// Unique identifier for an IOU (SHA256 hash of contents)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IOUId([u8; 32]);

impl IOUId {
    /// Create an IOUId from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Hash for IOUId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// The IOU (payment packet) - an unsigned representation of a payment intent
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IOU {
    sender: Did,
    recipient: Did,
    amount: u64,
    nonce: u64,
    timestamp: u64,
}

impl IOU {
    /// Create a new IOU
    pub fn new(
        sender: Did,
        recipient: Did,
        amount: u64,
        nonce: u64,
        timestamp: u64,
    ) -> Self {
        Self {
            sender,
            recipient,
            amount,
            nonce,
            timestamp,
        }
    }

    /// Get the sender DID
    pub fn sender(&self) -> &Did {
        &self.sender
    }

    /// Get the recipient DID
    pub fn recipient(&self) -> &Did {
        &self.recipient
    }

    /// Get the amount
    pub fn amount(&self) -> u64 {
        self.amount
    }

    /// Get the nonce
    pub fn nonce(&self) -> u64 {
        self.nonce
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Compute the unique ID for this IOU (SHA256 of all fields)
    pub fn id(&self) -> IOUId {
        let bytes = self.to_signing_bytes();
        let hash = Sha256::digest(&bytes);
        let mut id = [0u8; 32];
        id.copy_from_slice(&hash);
        IOUId(id)
    }

    /// Get the bytes that should be signed
    pub fn to_signing_bytes(&self) -> Vec<u8> {
        // Deterministic serialization of all fields
        let mut bytes = Vec::new();

        // Sender DID string
        let sender_str = self.sender.to_string();
        bytes.extend_from_slice(&(sender_str.len() as u32).to_le_bytes());
        bytes.extend_from_slice(sender_str.as_bytes());

        // Recipient DID string
        let recipient_str = self.recipient.to_string();
        bytes.extend_from_slice(&(recipient_str.len() as u32).to_le_bytes());
        bytes.extend_from_slice(recipient_str.as_bytes());

        // Amount
        bytes.extend_from_slice(&self.amount.to_le_bytes());

        // Nonce
        bytes.extend_from_slice(&self.nonce.to_le_bytes());

        // Timestamp
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());

        bytes
    }
}

/// A signed IOU - contains the IOU and its signature
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedIOU {
    iou: IOU,
    signature: Signature,
}

impl SignedIOU {
    /// Create a SignedIOU from parts
    pub fn from_parts(iou: IOU, signature: Signature) -> Self {
        Self { iou, signature }
    }

    /// Get the underlying IOU
    pub fn iou(&self) -> &IOU {
        &self.iou
    }

    /// Get the signature
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Get the unique ID of this IOU
    pub fn id(&self) -> IOUId {
        self.iou.id()
    }

    /// Verify the signature against a public key
    pub fn verify(&self, public_key: &PublicKey) -> bool {
        let bytes = self.iou.to_signing_bytes();
        Signer::verify(public_key, &bytes, &self.signature)
    }
}

impl PartialEq for SignedIOU {
    fn eq(&self, other: &Self) -> bool {
        self.iou == other.iou && self.signature.as_bytes() == other.signature.as_bytes()
    }
}

impl Eq for SignedIOU {}
