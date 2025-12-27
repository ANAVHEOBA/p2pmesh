use crate::identity::{Did, Keypair, Signer};
use crate::iou::{IOU, SignedIOU};
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Errors that can occur when building an IOU
#[derive(Error, Debug)]
pub enum IOUError {
    #[error("Missing sender: sender keypair is required")]
    MissingSender,

    #[error("Missing recipient: recipient DID is required")]
    MissingRecipient,

    #[error("Missing amount: payment amount is required")]
    MissingAmount,

    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Self-payment not allowed: sender and recipient cannot be the same")]
    SelfPayment,
}

/// Builder for creating signed IOUs
pub struct IOUBuilder<'a> {
    sender: Option<&'a Keypair>,
    recipient: Option<Did>,
    amount: Option<u64>,
    nonce: Option<u64>,
    timestamp: Option<u64>,
}

impl<'a> IOUBuilder<'a> {
    /// Create a new IOUBuilder
    pub fn new() -> Self {
        Self {
            sender: None,
            recipient: None,
            amount: None,
            nonce: None,
            timestamp: None,
        }
    }

    /// Set the sender (required)
    pub fn sender(mut self, keypair: &'a Keypair) -> Self {
        self.sender = Some(keypair);
        self
    }

    /// Set the recipient (required)
    pub fn recipient(mut self, did: Did) -> Self {
        self.recipient = Some(did);
        self
    }

    /// Set the amount (required)
    pub fn amount(mut self, amount: u64) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Set the nonce (optional - auto-generated if not provided)
    pub fn nonce(mut self, nonce: u64) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Set the timestamp (optional - auto-generated if not provided)
    pub fn timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Build and sign the IOU
    pub fn build(self) -> Result<SignedIOU, IOUError> {
        // Validate required fields
        let sender_keypair = self.sender.ok_or(IOUError::MissingSender)?;
        let recipient = self.recipient.ok_or(IOUError::MissingRecipient)?;
        let amount = self.amount.ok_or(IOUError::MissingAmount)?;

        // Validate amount is not zero
        if amount == 0 {
            return Err(IOUError::InvalidAmount("amount cannot be zero".to_string()));
        }

        // Derive sender DID from keypair
        let sender_did = Did::from_public_key(&sender_keypair.public_key());

        // Check for self-payment
        if sender_did == recipient {
            return Err(IOUError::SelfPayment);
        }

        // Generate nonce if not provided
        let nonce = self.nonce.unwrap_or_else(|| {
            rand::thread_rng().gen::<u64>()
        });

        // Generate timestamp if not provided
        let timestamp = self.timestamp.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });

        // Create the IOU
        let iou = IOU::new(sender_did, recipient, amount, nonce, timestamp);

        // Sign it
        let signing_bytes = iou.to_signing_bytes();
        let signature = Signer::sign(sender_keypair, &signing_bytes);

        Ok(SignedIOU::from_parts(iou, signature))
    }
}

impl<'a> Default for IOUBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}
