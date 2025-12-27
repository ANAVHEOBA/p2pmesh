// UTXO (Unspent Transaction Output) management

use crate::identity::PublicKey;
use crate::iou::IOUId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Type of UTXO - distinguishes between received payments and change
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UTXOType {
    /// UTXO from a received payment
    Received,
    /// UTXO from change after sending a payment
    Change,
}

/// Unique identifier for a UTXO
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UTXOId([u8; 32]);

impl UTXOId {
    /// Create a new UTXO ID for a received payment
    pub fn from_iou(iou_id: &IOUId) -> Self {
        Self::from_iou_with_type(iou_id, UTXOType::Received)
    }

    /// Create a new UTXO ID with explicit type (received vs change)
    /// This prevents ID collisions between payment and change UTXOs
    pub fn from_iou_with_type(iou_id: &IOUId, utxo_type: UTXOType) -> Self {
        let mut hasher = Sha256::new();
        match utxo_type {
            UTXOType::Received => hasher.update(b"utxo:received:"),
            UTXOType::Change => hasher.update(b"utxo:change:"),
        }
        hasher.update(iou_id.as_bytes());
        let result = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Self(bytes)
    }

    /// Create a UTXO ID from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the raw bytes of this ID
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Information about a UTXO lock
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LockInfo {
    /// When the lock expires (Unix timestamp in milliseconds)
    pub expires_at: u64,
    /// Optional reason for the lock
    pub reason: Option<String>,
}

impl LockInfo {
    /// Create a new lock info with expiry
    pub fn new(timeout_ms: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        Self {
            expires_at: now + timeout_ms,
            reason: None,
        }
    }

    /// Create a lock with a reason
    pub fn with_reason(timeout_ms: u64, reason: String) -> Self {
        let mut lock = Self::new(timeout_ms);
        lock.reason = Some(reason);
        lock
    }

    /// Check if this lock has expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        now >= self.expires_at
    }

    /// Get remaining time in milliseconds (0 if expired)
    pub fn remaining_ms(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.expires_at.saturating_sub(now)
    }
}

/// An Unspent Transaction Output - represents unspent funds
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UTXO {
    /// Unique identifier for this UTXO
    id: UTXOId,
    /// Owner of this UTXO (who can spend it)
    owner: PublicKey,
    /// Amount of value in this UTXO
    amount: u64,
    /// The IOU that created this UTXO
    source_iou_id: IOUId,
    /// Type of UTXO (received payment vs change)
    utxo_type: UTXOType,
    /// Whether this UTXO is locked for a pending transaction
    locked: bool,
}

impl UTXO {
    /// Create a new UTXO (defaults to Received type)
    pub fn new(owner: PublicKey, amount: u64, source_iou_id: IOUId) -> Self {
        Self::with_type(owner, amount, source_iou_id, UTXOType::Received)
    }

    /// Create a new UTXO with explicit type
    pub fn with_type(owner: PublicKey, amount: u64, source_iou_id: IOUId, utxo_type: UTXOType) -> Self {
        let id = UTXOId::from_iou_with_type(&source_iou_id, utxo_type);
        Self {
            id,
            owner,
            amount,
            source_iou_id,
            utxo_type,
            locked: false,
        }
    }

    /// Create a change UTXO
    pub fn new_change(owner: PublicKey, amount: u64, source_iou_id: IOUId) -> Self {
        Self::with_type(owner, amount, source_iou_id, UTXOType::Change)
    }

    /// Get the unique ID of this UTXO
    pub fn id(&self) -> &UTXOId {
        &self.id
    }

    /// Get the owner of this UTXO
    pub fn owner(&self) -> &PublicKey {
        &self.owner
    }

    /// Get the amount in this UTXO
    pub fn amount(&self) -> u64 {
        self.amount
    }

    /// Get the source IOU ID
    pub fn source_iou_id(&self) -> &IOUId {
        &self.source_iou_id
    }

    /// Get the type of this UTXO
    pub fn utxo_type(&self) -> UTXOType {
        self.utxo_type
    }

    /// Check if this UTXO is locked
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Lock this UTXO for a pending transaction
    pub fn lock(&mut self) {
        self.locked = true;
    }

    /// Unlock this UTXO
    pub fn unlock(&mut self) {
        self.locked = false;
    }
}

/// A set of UTXOs for efficient management
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UTXOSet {
    utxos: HashMap<UTXOId, UTXO>,
}

impl UTXOSet {
    /// Create a new empty UTXO set
    pub fn new() -> Self {
        Self {
            utxos: HashMap::new(),
        }
    }

    /// Add a UTXO to the set
    pub fn add(&mut self, utxo: UTXO) {
        self.utxos.insert(utxo.id().clone(), utxo);
    }

    /// Remove a UTXO from the set by ID
    pub fn remove(&mut self, id: &UTXOId) -> Option<UTXO> {
        self.utxos.remove(id)
    }

    /// Get a UTXO by ID
    pub fn get(&self, id: &UTXOId) -> Option<&UTXO> {
        self.utxos.get(id)
    }

    /// Get a mutable reference to a UTXO by ID
    pub fn get_mut(&mut self, id: &UTXOId) -> Option<&mut UTXO> {
        self.utxos.get_mut(id)
    }

    /// Check if a UTXO exists in the set
    pub fn contains(&self, id: &UTXOId) -> bool {
        self.utxos.contains_key(id)
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.utxos.is_empty()
    }

    /// Get the number of UTXOs
    pub fn len(&self) -> usize {
        self.utxos.len()
    }

    /// Get the total value of all UTXOs
    pub fn total_value(&self) -> u64 {
        self.utxos.values().map(|u| u.amount()).sum()
    }

    /// Get all UTXOs as a vector
    pub fn to_vec(&self) -> Vec<&UTXO> {
        self.utxos.values().collect()
    }

    /// Get all unlocked UTXOs
    pub fn unlocked(&self) -> Vec<&UTXO> {
        self.utxos.values().filter(|u| !u.is_locked()).collect()
    }

    /// Get the total value of unlocked UTXOs
    pub fn unlocked_value(&self) -> u64 {
        self.utxos
            .values()
            .filter(|u| !u.is_locked())
            .map(|u| u.amount())
            .sum()
    }

    /// Select UTXOs to cover a specific amount
    /// Returns (selected UTXOs, change amount) or None if insufficient funds
    pub fn select_for_amount(&self, amount: u64) -> Option<(Vec<UTXO>, u64)> {
        if amount == 0 {
            return Some((vec![], 0));
        }

        // Get unlocked UTXOs sorted by amount (descending for efficiency)
        let mut available: Vec<_> = self.utxos.values().filter(|u| !u.is_locked()).collect();
        available.sort_by(|a, b| b.amount().cmp(&a.amount()));

        // First, check if we have an exact match
        for utxo in &available {
            if utxo.amount() == amount {
                return Some((vec![(*utxo).clone()], 0));
            }
        }

        // Otherwise, greedily select UTXOs
        let mut selected = Vec::new();
        let mut total = 0u64;

        for utxo in available {
            if total >= amount {
                break;
            }
            selected.push(utxo.clone());
            total = total.saturating_add(utxo.amount());
        }

        if total >= amount {
            Some((selected, total - amount))
        } else {
            None
        }
    }

    /// Iterate over all UTXOs
    pub fn iter(&self) -> impl Iterator<Item = &UTXO> {
        self.utxos.values()
    }
}
