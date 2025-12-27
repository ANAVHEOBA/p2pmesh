// Mesh State - Tracks the current state of the distributed ledger

use crate::identity::{Did, PublicKey};
use crate::iou::{IOUId, IOUValidator, SignedIOU};
use crate::ledger::crdt::{GSet, IOUEntry, MergeResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;

/// Unique identifier for a node in the mesh
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId([u8; 32]);

impl NodeId {
    /// Generate a random node ID
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Create a node ID from a public key
    pub fn from_public_key(pubkey: &PublicKey) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"nodeid:");
        hasher.update(pubkey.as_bytes());
        let result = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Self(bytes)
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Errors that can occur during mesh state operations
#[derive(Error, Debug)]
pub enum MeshStateError {
    #[error("Duplicate IOU: already in the mesh state")]
    DuplicateIOU,

    #[error("Invalid signature on IOU")]
    InvalidSignature,

    #[error("IOU validation failed: {0}")]
    ValidationFailed(String),

    #[error("Deserialization failed")]
    DeserializationFailed,
}

/// Statistics about the mesh state
#[derive(Clone, Debug)]
pub struct MeshStatistics {
    pub total_ious: usize,
    pub unique_senders: usize,
    pub unique_recipients: usize,
    pub total_value: u64,
}

/// The shared mesh state - contains all known IOUs across the network
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshState {
    /// This node's unique ID
    node_id: NodeId,
    /// G-Set of all known IOUs (grows only, never shrinks)
    ious: GSet<IOUEntry>,
    /// Index: IOU ID -> IOUEntry for fast lookup
    #[serde(skip)]
    iou_index: HashMap<IOUId, IOUEntry>,
    /// Index: Sender DID -> list of IOU IDs
    #[serde(skip)]
    sender_index: HashMap<Did, Vec<IOUId>>,
    /// Index: Recipient DID -> list of IOU IDs
    #[serde(skip)]
    recipient_index: HashMap<Did, Vec<IOUId>>,
    /// Version counter (logical clock)
    version: u64,
}

impl MeshState {
    /// Create a new empty mesh state for a node
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            ious: GSet::new(),
            iou_index: HashMap::new(),
            sender_index: HashMap::new(),
            recipient_index: HashMap::new(),
            version: 0,
        }
    }

    /// Get this node's ID
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// Check if the state is empty
    pub fn is_empty(&self) -> bool {
        self.ious.is_empty()
    }

    /// Get the number of IOUs in the state
    pub fn iou_count(&self) -> usize {
        self.ious.len()
    }

    /// Get the current version (logical clock)
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Check if an IOU is in the state
    pub fn has_iou(&self, iou_id: &IOUId) -> bool {
        self.iou_index.contains_key(iou_id)
    }

    /// Add an IOU to the mesh state
    pub fn add_iou(&mut self, iou: SignedIOU, sender_pubkey: &PublicKey) -> Result<(), MeshStateError> {
        let iou_id = iou.id();

        // Check for duplicate
        if self.iou_index.contains_key(&iou_id) {
            return Err(MeshStateError::DuplicateIOU);
        }

        // Validate signature
        IOUValidator::validate(&iou, sender_pubkey)
            .map_err(|e| MeshStateError::ValidationFailed(e.to_string()))?;

        // Create entry
        let entry = IOUEntry::new(iou.clone(), sender_pubkey.clone());

        // Add to G-Set
        self.ious.insert(entry.clone());

        // Update indexes
        self.index_entry(&entry);

        // Increment version
        self.version += 1;

        Ok(())
    }

    /// Index an entry for fast lookup
    fn index_entry(&mut self, entry: &IOUEntry) {
        let iou = entry.iou();
        let iou_id = entry.id();

        // Main index
        self.iou_index.insert(iou_id.clone(), entry.clone());

        // Sender index
        let sender = iou.iou().sender().clone();
        self.sender_index
            .entry(sender)
            .or_insert_with(Vec::new)
            .push(iou_id.clone());

        // Recipient index
        let recipient = iou.iou().recipient().clone();
        self.recipient_index
            .entry(recipient)
            .or_insert_with(Vec::new)
            .push(iou_id);
    }

    /// Rebuild indexes from the G-Set (after deserialization)
    fn rebuild_indexes(&mut self) {
        self.iou_index.clear();
        self.sender_index.clear();
        self.recipient_index.clear();

        // Collect entries first to avoid borrow issues
        let entries: Vec<IOUEntry> = self.ious.iter().cloned().collect();
        for entry in entries {
            self.index_entry(&entry);
        }
    }

    /// Get an IOU by ID
    pub fn get_iou(&self, iou_id: &IOUId) -> Option<&IOUEntry> {
        self.iou_index.get(iou_id)
    }

    /// Get all IOUs sent by a specific DID
    pub fn get_ious_by_sender(&self, sender: &Did) -> Vec<&IOUEntry> {
        self.sender_index
            .get(sender)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.iou_index.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all IOUs received by a specific DID
    pub fn get_ious_by_recipient(&self, recipient: &Did) -> Vec<&IOUEntry> {
        self.recipient_index
            .get(recipient)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.iou_index.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Merge another state into this one (CRDT merge)
    pub fn merge(&mut self, other: &MeshState) -> MergeResult {
        let result = self.ious.merge_with_result(&other.ious);

        if result.new_entries > 0 {
            // Rebuild indexes to include new entries
            self.rebuild_indexes();
            self.version += 1;
        }

        result
    }

    /// Get entries that this state has but other doesn't (for efficient sync)
    pub fn delta(&self, other: &MeshState) -> Vec<IOUEntry> {
        self.ious.delta(&other.ious).to_vec()
    }

    /// Calculate total received by a DID
    pub fn total_received(&self, did: &Did) -> u64 {
        self.get_ious_by_recipient(did)
            .iter()
            .map(|e| e.iou().iou().amount())
            .sum()
    }

    /// Calculate total sent by a DID
    pub fn total_sent(&self, did: &Did) -> u64 {
        self.get_ious_by_sender(did)
            .iter()
            .map(|e| e.iou().iou().amount())
            .sum()
    }

    /// Get statistics about the mesh state
    pub fn statistics(&self) -> MeshStatistics {
        let total_ious = self.iou_count();
        let unique_senders = self.sender_index.len();
        let unique_recipients = self.recipient_index.len();
        let total_value: u64 = self.ious
            .iter()
            .map(|e| e.iou().iou().amount())
            .sum();

        MeshStatistics {
            total_ious,
            unique_senders,
            unique_recipients,
            total_value,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MeshStateError> {
        let mut state: MeshState = postcard::from_bytes(bytes)
            .map_err(|_| MeshStateError::DeserializationFailed)?;
        state.rebuild_indexes();
        Ok(state)
    }

    /// Get all IOU entries
    pub fn all_entries(&self) -> Vec<&IOUEntry> {
        self.ious.iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Keypair;
    use crate::iou::IOUBuilder;

    fn create_test_iou(sender: &Keypair, recipient: &Keypair, amount: u64, nonce: u64) -> SignedIOU {
        IOUBuilder::new()
            .sender(sender)
            .recipient(Did::from_public_key(&recipient.public_key()))
            .amount(amount)
            .nonce(nonce)
            .build()
            .unwrap()
    }

    #[test]
    fn test_mesh_state_basic() {
        let node_id = NodeId::generate();
        let state = MeshState::new(node_id);

        assert!(state.is_empty());
        assert_eq!(state.iou_count(), 0);
    }

    #[test]
    fn test_add_iou() {
        let node_id = NodeId::generate();
        let mut state = MeshState::new(node_id);

        let alice = Keypair::generate();
        let bob = Keypair::generate();

        let iou = create_test_iou(&alice, &bob, 100, 1);
        state.add_iou(iou, &alice.public_key()).unwrap();

        assert_eq!(state.iou_count(), 1);
    }

    #[test]
    fn test_merge_states() {
        let node1_id = NodeId::generate();
        let node2_id = NodeId::generate();

        let mut state1 = MeshState::new(node1_id);
        let mut state2 = MeshState::new(node2_id);

        let alice = Keypair::generate();
        let bob = Keypair::generate();

        let iou1 = create_test_iou(&alice, &bob, 100, 1);
        let iou2 = create_test_iou(&alice, &bob, 200, 2);

        state1.add_iou(iou1, &alice.public_key()).unwrap();
        state2.add_iou(iou2, &alice.public_key()).unwrap();

        let result = state1.merge(&state2);

        assert_eq!(result.new_entries, 1);
        assert_eq!(state1.iou_count(), 2);
    }
}
