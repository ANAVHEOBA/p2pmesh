use crate::identity::{PublicKey, KeypairError};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};
use thiserror::Error;

const DID_PREFIX: &str = "did:mesh:";

#[derive(Error, Debug)]
pub enum DidError {
    #[error("Invalid DID format: {0}")]
    InvalidFormat(String),

    #[error("Invalid DID method: expected 'mesh', got '{0}'")]
    InvalidMethod(String),

    #[error("Invalid base58 encoding: {0}")]
    InvalidBase58(String),

    #[error("Invalid public key: {0}")]
    InvalidPublicKey(#[from] KeypairError),
}

/// Decentralized Identifier in the format: did:mesh:<base58_public_key>
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Did {
    /// The base58-encoded public key
    key_part: String,
}

impl Did {
    /// Create a DID from a public key
    pub fn from_public_key(public_key: &PublicKey) -> Self {
        let key_part = bs58::encode(public_key.as_bytes()).into_string();
        Self { key_part }
    }

    /// Parse a DID from a string
    pub fn parse(s: &str) -> Result<Self, DidError> {
        // Check empty
        if s.is_empty() {
            return Err(DidError::InvalidFormat("DID cannot be empty".into()));
        }

        // Split by ':'
        let parts: Vec<&str> = s.split(':').collect();

        if parts.len() != 3 {
            return Err(DidError::InvalidFormat(
                format!("Expected 3 parts separated by ':', got {}", parts.len())
            ));
        }

        // Check scheme
        if parts[0] != "did" {
            return Err(DidError::InvalidFormat(
                format!("Expected 'did' scheme, got '{}'", parts[0])
            ));
        }

        // Check method
        if parts[1] != "mesh" {
            return Err(DidError::InvalidMethod(parts[1].to_string()));
        }

        // Check key part is not empty
        if parts[2].is_empty() {
            return Err(DidError::InvalidFormat("Key part cannot be empty".into()));
        }

        // Validate base58 encoding by attempting to decode
        let key_part = parts[2].to_string();
        bs58::decode(&key_part)
            .into_vec()
            .map_err(|e| DidError::InvalidBase58(e.to_string()))?;

        Ok(Self { key_part })
    }

    /// Extract the public key from this DID
    pub fn public_key(&self) -> Result<PublicKey, DidError> {
        let bytes = bs58::decode(&self.key_part)
            .into_vec()
            .map_err(|e| DidError::InvalidBase58(e.to_string()))?;

        PublicKey::from_bytes(&bytes).map_err(DidError::InvalidPublicKey)
    }

    /// Get the key part of the DID (base58 encoded)
    pub fn key_part(&self) -> &str {
        &self.key_part
    }
}

impl fmt::Display for Did {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", DID_PREFIX, self.key_part)
    }
}

impl PartialEq for Did {
    fn eq(&self, other: &Self) -> bool {
        self.key_part == other.key_part
    }
}

impl Eq for Did {}

impl Hash for Did {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key_part.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Keypair;

    #[test]
    fn test_did_roundtrip() {
        let kp = Keypair::generate();
        let did = Did::from_public_key(&kp.public_key());
        let parsed = Did::parse(&did.to_string()).unwrap();
        assert_eq!(did, parsed);
    }
}
