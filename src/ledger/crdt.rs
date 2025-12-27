// CRDT (Conflict-free Replicated Data Type) Implementation
// G-Set (Grow-only Set) for eventual consistency in distributed systems

use crate::identity::PublicKey;
use crate::iou::{IOUId, SignedIOU};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::Hash;
use std::time::{SystemTime, UNIX_EPOCH};

/// Result of a merge operation
#[derive(Clone, Debug)]
pub struct MergeResult {
    /// Number of new entries added during merge
    pub new_entries: usize,
    /// Total entries after merge
    pub total_after_merge: usize,
}

/// G-Set (Grow-only Set) - A CRDT where elements can only be added, never removed
///
/// Properties:
/// - Commutative: merge(A, B) == merge(B, A)
/// - Associative: merge(merge(A, B), C) == merge(A, merge(B, C))
/// - Idempotent: merge(A, A) == A
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GSet<T>
where
    T: Eq + Hash + Clone,
{
    elements: HashSet<T>,
}

impl<T> Default for GSet<T>
where
    T: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> GSet<T>
where
    T: Eq + Hash + Clone,
{
    /// Create a new empty G-Set
    pub fn new() -> Self {
        Self {
            elements: HashSet::new(),
        }
    }

    /// Insert an element into the set
    /// Returns true if the element was new, false if it already existed
    pub fn insert(&mut self, element: T) -> bool {
        self.elements.insert(element)
    }

    /// Check if the set contains an element
    pub fn contains(&self, element: &T) -> bool {
        self.elements.contains(element)
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Get the number of elements in the set
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Iterate over all elements
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.elements.iter()
    }

    /// Merge another G-Set into this one (union operation)
    /// This is the key CRDT operation - it's commutative, associative, and idempotent
    pub fn merge(&mut self, other: &GSet<T>) {
        for element in &other.elements {
            self.elements.insert(element.clone());
        }
    }

    /// Merge with result tracking
    pub fn merge_with_result(&mut self, other: &GSet<T>) -> MergeResult {
        let before = self.elements.len();

        for element in &other.elements {
            self.elements.insert(element.clone());
        }

        let after = self.elements.len();

        MergeResult {
            new_entries: after - before,
            total_after_merge: after,
        }
    }

    /// Get elements that are in this set but not in other (for efficient sync)
    pub fn delta(&self, other: &GSet<T>) -> GSet<T> {
        let mut delta = GSet::new();
        for element in &self.elements {
            if !other.contains(element) {
                delta.insert(element.clone());
            }
        }
        delta
    }

    /// Convert to a vector
    pub fn to_vec(&self) -> Vec<T> {
        self.elements.iter().cloned().collect()
    }
}

impl<T> GSet<T>
where
    T: Eq + Hash + Clone + Serialize + for<'de> Deserialize<'de>,
{
    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, GSetError> {
        postcard::from_bytes(bytes).map_err(|_| GSetError::DeserializationFailed)
    }
}

/// Error types for G-Set operations
#[derive(Debug, Clone)]
pub enum GSetError {
    DeserializationFailed,
}

impl std::fmt::Display for GSetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GSetError::DeserializationFailed => write!(f, "Failed to deserialize G-Set"),
        }
    }
}

impl std::error::Error for GSetError {}

/// An IOU entry for the ledger - wraps a SignedIOU with metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IOUEntry {
    /// The signed IOU
    iou: SignedIOU,
    /// Public key of the sender (for verification)
    sender_pubkey: PublicKey,
    /// When this entry was received by this node
    received_at: u64,
}

impl IOUEntry {
    /// Create a new IOU entry
    pub fn new(iou: SignedIOU, sender_pubkey: PublicKey) -> Self {
        let received_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            iou,
            sender_pubkey,
            received_at,
        }
    }

    /// Create with explicit timestamp (for importing)
    pub fn with_timestamp(iou: SignedIOU, sender_pubkey: PublicKey, received_at: u64) -> Self {
        Self {
            iou,
            sender_pubkey,
            received_at,
        }
    }

    /// Get the underlying IOU
    pub fn iou(&self) -> &SignedIOU {
        &self.iou
    }

    /// Get the sender's public key
    pub fn sender_pubkey(&self) -> &PublicKey {
        &self.sender_pubkey
    }

    /// Get the IOU ID
    pub fn id(&self) -> IOUId {
        self.iou.id()
    }

    /// Get when this entry was received
    pub fn received_at(&self) -> u64 {
        self.received_at
    }

    /// Verify the IOU signature
    pub fn verify(&self) -> bool {
        self.iou.verify(&self.sender_pubkey)
    }
}

impl PartialEq for IOUEntry {
    fn eq(&self, other: &Self) -> bool {
        // Two entries are equal if they have the same IOU ID
        self.id() == other.id()
    }
}

impl Eq for IOUEntry {}

impl Hash for IOUEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash by IOU ID for set membership
        self.id().as_bytes().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gset_basic_operations() {
        let mut gset: GSet<i32> = GSet::new();
        assert!(gset.is_empty());

        gset.insert(1);
        assert!(!gset.is_empty());
        assert_eq!(gset.len(), 1);
        assert!(gset.contains(&1));
        assert!(!gset.contains(&2));
    }

    #[test]
    fn test_gset_merge() {
        let mut gset1: GSet<i32> = GSet::new();
        gset1.insert(1);
        gset1.insert(2);

        let mut gset2: GSet<i32> = GSet::new();
        gset2.insert(2);
        gset2.insert(3);

        gset1.merge(&gset2);

        assert_eq!(gset1.len(), 3);
        assert!(gset1.contains(&1));
        assert!(gset1.contains(&2));
        assert!(gset1.contains(&3));
    }

    #[test]
    fn test_gset_delta() {
        let mut gset1: GSet<i32> = GSet::new();
        gset1.insert(1);
        gset1.insert(2);

        let mut gset2: GSet<i32> = GSet::new();
        gset2.insert(2);
        gset2.insert(3);
        gset2.insert(4);

        let delta = gset2.delta(&gset1);

        assert_eq!(delta.len(), 2);
        assert!(delta.contains(&3));
        assert!(delta.contains(&4));
        assert!(!delta.contains(&2));
    }
}
