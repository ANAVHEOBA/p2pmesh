// Spending logic and double-spend prevention

use crate::iou::IOUId;
use crate::vault::utxo::UTXOId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Error type for spent output operations
#[derive(Error, Debug)]
pub enum SpentOutputError {
    #[error("UTXO already spent")]
    AlreadySpent,
}

/// Record of a spent output - proof that a UTXO was consumed
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpentOutput {
    /// The UTXO that was spent
    utxo_id: UTXOId,
    /// The IOU that spent this UTXO
    spending_iou_id: IOUId,
    /// When the UTXO was spent (Unix timestamp)
    spent_at: u64,
}

impl SpentOutput {
    /// Create a new spent output record
    pub fn new(utxo_id: UTXOId, spending_iou_id: IOUId, spent_at: u64) -> Self {
        Self {
            utxo_id,
            spending_iou_id,
            spent_at,
        }
    }

    /// Create a new spent output record with current timestamp
    pub fn now(utxo_id: UTXOId, spending_iou_id: IOUId) -> Self {
        let spent_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self::new(utxo_id, spending_iou_id, spent_at)
    }

    /// Get the UTXO ID that was spent
    pub fn utxo_id(&self) -> &UTXOId {
        &self.utxo_id
    }

    /// Get the IOU ID that spent this UTXO
    pub fn spending_iou_id(&self) -> &IOUId {
        &self.spending_iou_id
    }

    /// Get when this UTXO was spent
    pub fn spent_at(&self) -> u64 {
        self.spent_at
    }
}

/// A set of spent outputs for tracking consumed UTXOs
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SpentOutputSet {
    spent: HashMap<UTXOId, SpentOutput>,
}

impl SpentOutputSet {
    /// Create a new empty spent output set
    pub fn new() -> Self {
        Self {
            spent: HashMap::new(),
        }
    }

    /// Add a spent output to the set
    /// Returns error if already spent (double-spend attempt)
    pub fn add(&mut self, spent: SpentOutput) -> Result<(), SpentOutputError> {
        if self.spent.contains_key(spent.utxo_id()) {
            return Err(SpentOutputError::AlreadySpent);
        }
        self.spent.insert(spent.utxo_id().clone(), spent);
        Ok(())
    }

    /// Force add a spent output (for importing state)
    pub fn add_unchecked(&mut self, spent: SpentOutput) {
        self.spent.insert(spent.utxo_id().clone(), spent);
    }

    /// Check if a UTXO has been spent
    pub fn contains(&self, utxo_id: &UTXOId) -> bool {
        self.spent.contains_key(utxo_id)
    }

    /// Get the spent output record for a UTXO
    pub fn get(&self, utxo_id: &UTXOId) -> Option<&SpentOutput> {
        self.spent.get(utxo_id)
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.spent.is_empty()
    }

    /// Get the number of spent outputs
    pub fn len(&self) -> usize {
        self.spent.len()
    }

    /// Get all spent outputs as a vector
    pub fn to_vec(&self) -> Vec<&SpentOutput> {
        self.spent.values().collect()
    }

    /// Iterate over all spent outputs
    pub fn iter(&self) -> impl Iterator<Item = &SpentOutput> {
        self.spent.values()
    }
}
