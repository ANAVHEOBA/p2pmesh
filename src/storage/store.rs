// MeshStore - Persistent key-value storage using sled
//
// Provides typed access for storing:
// - Identity keypairs
// - Personal vault state
// - Mesh ledger state
// - Node configuration

use crate::identity::Keypair;
use crate::ledger::{MeshState, MeshStateError, NodeId};
use crate::vault::{Vault, VaultError};
use std::path::Path;
use thiserror::Error;

/// Key prefixes for organizing data
mod keys {
    pub const IDENTITY_KEYPAIR: &[u8] = b"identity:keypair";
    pub const IDENTITY_KEYPAIR_PREFIX: &[u8] = b"identity:keypair:";
    pub const VAULT: &[u8] = b"vault:state";
    pub const MESH_STATE: &[u8] = b"ledger:mesh_state";
    pub const NODE_ID: &[u8] = b"node:id";
}

/// Errors from storage operations
#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Failed to open database: {0}")]
    OpenFailed(String),

    #[error("Database operation failed: {0}")]
    DatabaseError(String),

    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),

    #[error("Flush failed: {0}")]
    FlushFailed(String),
}

impl From<sled::Error> for StoreError {
    fn from(err: sled::Error) -> Self {
        StoreError::DatabaseError(err.to_string())
    }
}

/// Statistics about the storage
#[derive(Clone, Debug)]
pub struct StorageStats {
    /// Number of keys in the database
    pub key_count: usize,
    /// Approximate disk size in bytes
    pub disk_size_bytes: u64,
}

/// Persistent key-value store for mesh data
///
/// Uses sled for crash-safe, embedded storage.
/// All writes are atomic and durable after flush.
pub struct MeshStore {
    db: sled::Db,
}

impl MeshStore {
    /// Open or create a store at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let db = sled::open(path).map_err(|e| StoreError::OpenFailed(e.to_string()))?;
        Ok(Self { db })
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> Result<bool, StoreError> {
        Ok(self.db.is_empty())
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<(), StoreError> {
        self.db
            .flush()
            .map_err(|e| StoreError::FlushFailed(e.to_string()))?;
        Ok(())
    }

    /// Get storage statistics
    pub fn stats(&self) -> Result<StorageStats, StoreError> {
        Ok(StorageStats {
            key_count: self.db.len(),
            disk_size_bytes: self.db.size_on_disk().unwrap_or(0),
        })
    }

    // ========================================================================
    // RAW KEY-VALUE OPERATIONS
    // ========================================================================

    /// Put raw bytes
    pub fn put_raw(&self, key: &[u8], value: &[u8]) -> Result<(), StoreError> {
        self.db.insert(key, value)?;
        Ok(())
    }

    /// Get raw bytes
    pub fn get_raw(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.db.get(key)?.map(|v| v.to_vec()))
    }

    /// Delete a key
    pub fn delete(&self, key: &[u8]) -> Result<(), StoreError> {
        self.db.remove(key)?;
        Ok(())
    }

    /// List all keys with a given prefix
    pub fn list_keys_with_prefix(&self, prefix: &[u8]) -> Result<Vec<Vec<u8>>, StoreError> {
        let mut keys = Vec::new();
        for result in self.db.scan_prefix(prefix) {
            let (key, _) = result?;
            keys.push(key.to_vec());
        }
        Ok(keys)
    }

    /// Delete all keys with a given prefix
    pub fn delete_with_prefix(&self, prefix: &[u8]) -> Result<usize, StoreError> {
        let mut deleted = 0;
        for result in self.db.scan_prefix(prefix) {
            let (key, _) = result?;
            self.db.remove(key)?;
            deleted += 1;
        }
        Ok(deleted)
    }

    // ========================================================================
    // IDENTITY PERSISTENCE
    // ========================================================================

    /// Save the primary keypair
    pub fn save_keypair(&self, keypair: &Keypair) -> Result<(), StoreError> {
        let bytes = keypair.to_bytes();
        self.put_raw(keys::IDENTITY_KEYPAIR, &bytes)
    }

    /// Load the primary keypair
    pub fn load_keypair(&self) -> Result<Option<Keypair>, StoreError> {
        match self.get_raw(keys::IDENTITY_KEYPAIR)? {
            Some(bytes) => {
                let keypair = Keypair::from_bytes(&bytes)
                    .map_err(|e| StoreError::DeserializationFailed(e.to_string()))?;
                Ok(Some(keypair))
            }
            None => Ok(None),
        }
    }

    /// Save a keypair with a label
    pub fn save_keypair_with_label(&self, keypair: &Keypair, label: &str) -> Result<(), StoreError> {
        let key = [keys::IDENTITY_KEYPAIR_PREFIX, label.as_bytes()].concat();
        let bytes = keypair.to_bytes();
        self.put_raw(&key, &bytes)
    }

    /// Load a keypair by label
    pub fn load_keypair_with_label(&self, label: &str) -> Result<Option<Keypair>, StoreError> {
        let key = [keys::IDENTITY_KEYPAIR_PREFIX, label.as_bytes()].concat();
        match self.get_raw(&key)? {
            Some(bytes) => {
                let keypair = Keypair::from_bytes(&bytes)
                    .map_err(|e| StoreError::DeserializationFailed(e.to_string()))?;
                Ok(Some(keypair))
            }
            None => Ok(None),
        }
    }

    // ========================================================================
    // VAULT PERSISTENCE
    // ========================================================================

    /// Save the vault state
    pub fn save_vault(&self, vault: &Vault) -> Result<(), StoreError> {
        let bytes = vault.to_bytes();
        self.put_raw(keys::VAULT, &bytes)
    }

    /// Load the vault state
    pub fn load_vault(&self) -> Result<Option<Vault>, StoreError> {
        match self.get_raw(keys::VAULT)? {
            Some(bytes) => {
                let vault = Vault::from_bytes(&bytes)
                    .map_err(|e: VaultError| StoreError::DeserializationFailed(e.to_string()))?;
                Ok(Some(vault))
            }
            None => Ok(None),
        }
    }

    // ========================================================================
    // LEDGER STATE PERSISTENCE
    // ========================================================================

    /// Save the mesh state
    pub fn save_mesh_state(&self, state: &MeshState) -> Result<(), StoreError> {
        let bytes = state.to_bytes();
        self.put_raw(keys::MESH_STATE, &bytes)
    }

    /// Load the mesh state
    pub fn load_mesh_state(&self) -> Result<Option<MeshState>, StoreError> {
        match self.get_raw(keys::MESH_STATE)? {
            Some(bytes) => {
                let state = MeshState::from_bytes(&bytes)
                    .map_err(|e: MeshStateError| StoreError::DeserializationFailed(e.to_string()))?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }

    // ========================================================================
    // NODE CONFIGURATION
    // ========================================================================

    /// Save the node ID
    pub fn save_node_id(&self, node_id: &NodeId) -> Result<(), StoreError> {
        self.put_raw(keys::NODE_ID, node_id.as_bytes())
    }

    /// Load the node ID
    pub fn load_node_id(&self) -> Result<Option<NodeId>, StoreError> {
        match self.get_raw(keys::NODE_ID)? {
            Some(bytes) => {
                if bytes.len() != 32 {
                    return Err(StoreError::DeserializationFailed(
                        "Invalid node ID length".to_string(),
                    ));
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Ok(Some(NodeId::from_bytes(arr)))
            }
            None => Ok(None),
        }
    }

    /// Get the node ID, creating one if it doesn't exist
    pub fn get_or_create_node_id(&self) -> Result<NodeId, StoreError> {
        if let Some(node_id) = self.load_node_id()? {
            return Ok(node_id);
        }

        let node_id = NodeId::generate();
        self.save_node_id(&node_id)?;
        Ok(node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_store_basic() {
        let temp_dir = TempDir::new().unwrap();
        let store = MeshStore::open(temp_dir.path()).unwrap();

        store.put_raw(b"test", b"value").unwrap();
        let result = store.get_raw(b"test").unwrap();

        assert_eq!(result, Some(b"value".to_vec()));
    }

    #[test]
    fn test_store_persistence() {
        let temp_dir = TempDir::new().unwrap();

        {
            let store = MeshStore::open(temp_dir.path()).unwrap();
            store.put_raw(b"persist", b"data").unwrap();
            store.flush().unwrap();
        }

        {
            let store = MeshStore::open(temp_dir.path()).unwrap();
            let result = store.get_raw(b"persist").unwrap();
            assert_eq!(result, Some(b"data".to_vec()));
        }
    }
}
