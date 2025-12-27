use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KeypairError {
    #[error("Invalid key length: expected {expected}, got {got}")]
    InvalidLength { expected: usize, got: usize },

    #[error("Invalid key bytes: {0}")]
    InvalidBytes(String),
}

/// Ed25519 public key (32 bytes)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey(VerifyingKey);

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as raw bytes
        serializer.serialize_bytes(self.0.as_bytes())
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PublicKeyVisitor;

        impl<'de> serde::de::Visitor<'de> for PublicKeyVisitor {
            type Value = PublicKey;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a 32-byte public key")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v.len() != 32 {
                    return Err(E::invalid_length(v.len(), &"32 bytes"));
                }

                let bytes: [u8; 32] = v.try_into().map_err(|_| {
                    E::custom("failed to convert to 32-byte array")
                })?;

                let verifying_key = VerifyingKey::from_bytes(&bytes)
                    .map_err(|e| E::custom(format!("invalid public key: {}", e)))?;

                Ok(PublicKey(verifying_key))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut bytes = [0u8; 32];
                for i in 0..32 {
                    bytes[i] = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &"32 bytes"))?;
                }

                let verifying_key = VerifyingKey::from_bytes(&bytes)
                    .map_err(|e| serde::de::Error::custom(format!("invalid public key: {}", e)))?;

                Ok(PublicKey(verifying_key))
            }
        }

        deserializer.deserialize_bytes(PublicKeyVisitor)
    }
}

impl PublicKey {
    /// Get the raw bytes of the public key
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Create a public key from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KeypairError> {
        if bytes.len() != 32 {
            return Err(KeypairError::InvalidLength {
                expected: 32,
                got: bytes.len(),
            });
        }

        let bytes_array: [u8; 32] = bytes.try_into().map_err(|_| {
            KeypairError::InvalidBytes("Failed to convert to array".into())
        })?;

        let verifying_key = VerifyingKey::from_bytes(&bytes_array)
            .map_err(|e| KeypairError::InvalidBytes(e.to_string()))?;

        Ok(Self(verifying_key))
    }

    /// Get the inner verifying key (for internal use)
    pub(crate) fn inner(&self) -> &VerifyingKey {
        &self.0
    }
}

/// Ed25519 secret key (32 bytes)
#[derive(Clone)]
pub struct SecretKey(SigningKey);

impl SecretKey {
    /// Get the raw bytes of the secret key
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Create a secret key from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KeypairError> {
        if bytes.len() != 32 {
            return Err(KeypairError::InvalidLength {
                expected: 32,
                got: bytes.len(),
            });
        }

        let bytes_array: [u8; 32] = bytes.try_into().map_err(|_| {
            KeypairError::InvalidBytes("Failed to convert to array".into())
        })?;

        let signing_key = SigningKey::from_bytes(&bytes_array);
        Ok(Self(signing_key))
    }

    /// Get the inner signing key (for internal use)
    pub(crate) fn inner(&self) -> &SigningKey {
        &self.0
    }
}

/// Ed25519 keypair containing both public and secret keys
#[derive(Clone)]
pub struct Keypair {
    signing_key: SigningKey,
}

impl Keypair {
    /// Generate a new random keypair
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Get the public key
    pub fn public_key(&self) -> PublicKey {
        PublicKey(self.signing_key.verifying_key())
    }

    /// Get the secret key
    pub fn secret_key(&self) -> SecretKey {
        SecretKey(self.signing_key.clone())
    }

    /// Serialize the keypair to bytes (secret key bytes)
    pub fn to_bytes(&self) -> Vec<u8> {
        self.signing_key.to_bytes().to_vec()
    }

    /// Deserialize a keypair from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KeypairError> {
        if bytes.len() != 32 {
            return Err(KeypairError::InvalidLength {
                expected: 32,
                got: bytes.len(),
            });
        }

        let bytes_array: [u8; 32] = bytes.try_into().map_err(|_| {
            KeypairError::InvalidBytes("Failed to convert to array".into())
        })?;

        let signing_key = SigningKey::from_bytes(&bytes_array);
        Ok(Self { signing_key })
    }

    /// Create a keypair from an existing secret key
    pub fn from_secret_key(secret: SecretKey) -> Self {
        Self {
            signing_key: secret.0,
        }
    }

    /// Get the inner signing key (for internal use)
    pub(crate) fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_keypair() {
        let kp = Keypair::generate();
        assert_eq!(kp.public_key().as_bytes().len(), 32);
    }
}
