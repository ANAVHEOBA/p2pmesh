use crate::identity::{Keypair, PublicKey};
use ed25519_dalek::{Signature as DalekSignature, Signer as DalekSigner, Verifier};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SignatureError {
    #[error("Invalid signature length: expected 64, got {0}")]
    InvalidLength(usize),

    #[error("Invalid signature bytes: {0}")]
    InvalidBytes(String),
}

/// Ed25519 signature (64 bytes)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    inner: DalekSignature,
    bytes: [u8; 64],
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.bytes)
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct SignatureVisitor;

        impl<'de> Visitor<'de> for SignatureVisitor {
            type Value = Signature;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("64 bytes for Ed25519 signature")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Signature::from_bytes(v).map_err(|e| E::custom(e.to_string()))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut bytes = Vec::with_capacity(64);
                while let Some(byte) = seq.next_element()? {
                    bytes.push(byte);
                }
                Signature::from_bytes(&bytes).map_err(|e| de::Error::custom(e.to_string()))
            }
        }

        deserializer.deserialize_bytes(SignatureVisitor)
    }
}

impl Signature {
    /// Get the raw bytes of the signature
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Create a signature from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SignatureError> {
        if bytes.len() != 64 {
            return Err(SignatureError::InvalidLength(bytes.len()));
        }

        let bytes_array: [u8; 64] = bytes.try_into().map_err(|_| {
            SignatureError::InvalidBytes("Failed to convert to array".into())
        })?;

        let inner = DalekSignature::from_bytes(&bytes_array);
        Ok(Self { inner, bytes: bytes_array })
    }

    /// Create from inner dalek signature
    fn from_inner(inner: DalekSignature) -> Self {
        let bytes = inner.to_bytes();
        Self { inner, bytes }
    }

    /// Get the inner signature (for internal use)
    pub(crate) fn inner(&self) -> &DalekSignature {
        &self.inner
    }
}

/// Signing and verification operations
pub struct Signer;

impl Signer {
    /// Sign a message with a keypair
    pub fn sign(keypair: &Keypair, message: &[u8]) -> Signature {
        let sig = keypair.signing_key().sign(message);
        Signature::from_inner(sig)
    }

    /// Verify a signature against a public key and message
    pub fn verify(public_key: &PublicKey, message: &[u8], signature: &Signature) -> bool {
        public_key.inner().verify(message, signature.inner()).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify() {
        let kp = Keypair::generate();
        let msg = b"test message";
        let sig = Signer::sign(&kp, msg);
        assert!(Signer::verify(&kp.public_key(), msg, &sig));
    }

    #[test]
    fn test_wrong_message_fails() {
        let kp = Keypair::generate();
        let msg = b"test message";
        let sig = Signer::sign(&kp, msg);
        assert!(!Signer::verify(&kp.public_key(), b"wrong message", &sig));
    }
}
