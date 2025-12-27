use crate::identity::{Did, PublicKey};
use crate::iou::{IOU, SignedIOU};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Errors that can occur when validating an IOU
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Invalid signature: signature does not match the IOU content")]
    InvalidSignature,

    #[error("Self-payment not allowed: sender and recipient cannot be the same")]
    SelfPayment,

    #[error("Invalid amount: amount cannot be zero")]
    InvalidAmount,

    #[error("Future timestamp: IOU timestamp is too far in the future")]
    FutureTimestamp,

    #[error("Expired: IOU has expired")]
    Expired,

    #[error("Sender mismatch: the provided public key does not match the sender DID")]
    SenderMismatch,
}

/// Validator for IOUs
pub struct IOUValidator;

impl IOUValidator {
    /// Validate an IOU signature and basic rules
    ///
    /// This performs:
    /// - Signature verification
    /// - Self-payment check
    /// - Zero amount check
    /// - Sender DID matches public key check
    pub fn validate(signed_iou: &SignedIOU, sender_pubkey: &PublicKey) -> Result<IOU, ValidationError> {
        let iou = signed_iou.iou();

        // Check sender DID matches the public key
        let expected_did = Did::from_public_key(sender_pubkey);
        if iou.sender() != &expected_did {
            return Err(ValidationError::SenderMismatch);
        }

        // Verify signature
        if !signed_iou.verify(sender_pubkey) {
            return Err(ValidationError::InvalidSignature);
        }

        // Check for self-payment
        if iou.sender() == iou.recipient() {
            return Err(ValidationError::SelfPayment);
        }

        // Check for zero amount
        if iou.amount() == 0 {
            return Err(ValidationError::InvalidAmount);
        }

        Ok(iou.clone())
    }

    /// Validate with timestamp check (for clock skew protection)
    ///
    /// tolerance_secs: How many seconds into the future a timestamp is allowed
    pub fn validate_with_time_check(
        signed_iou: &SignedIOU,
        sender_pubkey: &PublicKey,
        tolerance_secs: u64,
    ) -> Result<IOU, ValidationError> {
        // First do basic validation
        let iou = Self::validate(signed_iou, sender_pubkey)?;

        // Check timestamp is not too far in the future
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if iou.timestamp() > now + tolerance_secs {
            return Err(ValidationError::FutureTimestamp);
        }

        Ok(iou)
    }

    /// Validate with expiry check
    ///
    /// max_age_secs: Maximum age of the IOU in seconds
    pub fn validate_with_expiry(
        signed_iou: &SignedIOU,
        sender_pubkey: &PublicKey,
        max_age_secs: u64,
    ) -> Result<IOU, ValidationError> {
        // First do basic validation
        let iou = Self::validate(signed_iou, sender_pubkey)?;

        // Check IOU hasn't expired
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if iou.timestamp() + max_age_secs < now {
            return Err(ValidationError::Expired);
        }

        Ok(iou)
    }

    /// Full validation with both time checks
    pub fn validate_full(
        signed_iou: &SignedIOU,
        sender_pubkey: &PublicKey,
        future_tolerance_secs: u64,
        max_age_secs: u64,
    ) -> Result<IOU, ValidationError> {
        // First do basic validation
        let iou = Self::validate(signed_iou, sender_pubkey)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Check not too far in the future
        if iou.timestamp() > now + future_tolerance_secs {
            return Err(ValidationError::FutureTimestamp);
        }

        // Check not expired
        if iou.timestamp() + max_age_secs < now {
            return Err(ValidationError::Expired);
        }

        Ok(iou)
    }
}
