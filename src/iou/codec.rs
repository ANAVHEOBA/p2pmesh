use crate::iou::SignedIOU;
use thiserror::Error;

/// Errors that can occur during encoding/decoding
#[derive(Error, Debug)]
pub enum CodecError {
    #[error("Failed to encode IOU: {0}")]
    EncodeError(String),

    #[error("Failed to decode IOU: {0}")]
    DecodeError(String),

    #[error("Invalid hex string: {0}")]
    InvalidHex(String),

    #[error("Invalid base64 string: {0}")]
    InvalidBase64(String),
}

/// Codec for serializing/deserializing IOUs
pub struct IOUCodec;

impl IOUCodec {
    /// Encode a SignedIOU to binary bytes (using postcard for compact serialization)
    pub fn encode(signed_iou: &SignedIOU) -> Vec<u8> {
        postcard::to_allocvec(signed_iou).expect("Failed to encode IOU")
    }

    /// Decode a SignedIOU from binary bytes
    pub fn decode(bytes: &[u8]) -> Result<SignedIOU, CodecError> {
        postcard::from_bytes(bytes)
            .map_err(|e| CodecError::DecodeError(e.to_string()))
    }

    /// Encode to hex string
    pub fn encode_hex(signed_iou: &SignedIOU) -> String {
        hex::encode(Self::encode(signed_iou))
    }

    /// Decode from hex string
    pub fn decode_hex(hex_str: &str) -> Result<SignedIOU, CodecError> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| CodecError::InvalidHex(e.to_string()))?;
        Self::decode(&bytes)
    }

    /// Encode to base64 string (URL-safe, no padding)
    pub fn encode_base64(signed_iou: &SignedIOU) -> String {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        URL_SAFE_NO_PAD.encode(Self::encode(signed_iou))
    }

    /// Decode from base64 string
    pub fn decode_base64(b64_str: &str) -> Result<SignedIOU, CodecError> {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        let bytes = URL_SAFE_NO_PAD.decode(b64_str)
            .map_err(|e| CodecError::InvalidBase64(e.to_string()))?;
        Self::decode(&bytes)
    }
}
