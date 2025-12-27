// Collector - Gathers IOUs for settlement
// Responsible for collecting IOUs from mesh state and creating settlement batches

use crate::identity::Did;
use crate::iou::{IOUId, SignedIOU};
use crate::ledger::MeshState;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

// ============================================================================
// BATCH ID
// ============================================================================

/// Unique identifier for a settlement batch
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BatchId([u8; 32]);

impl BatchId {
    /// Generate a random batch ID
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
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

    /// Convert to bytes (for serialization)
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for BatchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "batch:{}", hex::encode(&self.0[..8]))
    }
}

// ============================================================================
// BATCH STATUS
// ============================================================================

/// Status of a settlement batch
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchStatus {
    /// Batch created but not yet submitted
    Pending,
    /// Batch is being processed
    Processing,
    /// Batch has been submitted to settlement target
    Submitted,
    /// Settlement confirmed
    Confirmed,
    /// Settlement failed
    Failed,
    /// Batch was cancelled
    Cancelled,
}

// ============================================================================
// SETTLEMENT ENTRY
// ============================================================================

/// A single entry in a settlement batch
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettlementEntry {
    iou_id: IOUId,
    sender: Did,
    recipient: Did,
    amount: u64,
    timestamp: u64,
}

impl SettlementEntry {
    /// Create a settlement entry from a signed IOU
    pub fn from_iou(iou: &SignedIOU) -> Self {
        Self {
            iou_id: iou.id(),
            sender: iou.iou().sender().clone(),
            recipient: iou.iou().recipient().clone(),
            amount: iou.iou().amount(),
            timestamp: iou.iou().timestamp(),
        }
    }

    /// Get the IOU ID
    pub fn iou_id(&self) -> &IOUId {
        &self.iou_id
    }

    /// Get the sender DID
    pub fn sender(&self) -> &Did {
        &self.sender
    }

    /// Get the recipient DID
    pub fn recipient(&self) -> &Did {
        &self.recipient
    }

    /// Get the amount
    pub fn amount(&self) -> u64 {
        self.amount
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CollectorError> {
        postcard::from_bytes(bytes).map_err(|_| CollectorError::DeserializationFailed)
    }
}

// ============================================================================
// NET POSITION
// ============================================================================

/// Net position of a party in a settlement batch
#[derive(Clone, Debug)]
pub struct NetPosition {
    party: Did,
    net_amount: i64,
}

impl NetPosition {
    /// Get the party DID
    pub fn party(&self) -> &Did {
        &self.party
    }

    /// Get the net amount (positive = receives, negative = owes)
    pub fn net_amount(&self) -> i64 {
        self.net_amount
    }
}

// ============================================================================
// SETTLEMENT BATCH
// ============================================================================

/// A batch of settlement entries ready for external settlement
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettlementBatch {
    id: BatchId,
    entries: Vec<SettlementEntry>,
    status: BatchStatus,
    created_at: u64,
    total_amount: u64,
}

impl SettlementBatch {
    /// Create a new empty batch
    pub fn new() -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            id: BatchId::generate(),
            entries: Vec::new(),
            status: BatchStatus::Pending,
            created_at,
            total_amount: 0,
        }
    }

    /// Get the batch ID
    pub fn id(&self) -> &BatchId {
        &self.id
    }

    /// Get all entries in the batch
    pub fn entries(&self) -> &[SettlementEntry] {
        &self.entries
    }

    /// Get the total amount in the batch
    pub fn total_amount(&self) -> u64 {
        self.total_amount
    }

    /// Get the batch status
    pub fn status(&self) -> &BatchStatus {
        &self.status
    }

    /// Set the batch status
    pub fn set_status(&mut self, status: BatchStatus) {
        self.status = status;
    }

    /// Get the creation timestamp
    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    /// Add an entry to the batch
    pub fn add_entry(&mut self, entry: SettlementEntry) {
        self.total_amount += entry.amount;
        self.entries.push(entry);
    }

    /// Calculate net positions for all parties in the batch
    pub fn calculate_net_positions(&self) -> Vec<NetPosition> {
        let mut positions: HashMap<Did, i64> = HashMap::new();

        for entry in &self.entries {
            // Sender loses money (negative)
            *positions.entry(entry.sender.clone()).or_insert(0) -= entry.amount as i64;
            // Recipient gains money (positive)
            *positions.entry(entry.recipient.clone()).or_insert(0) += entry.amount as i64;
        }

        positions
            .into_iter()
            .map(|(party, net_amount)| NetPosition { party, net_amount })
            .collect()
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CollectorError> {
        postcard::from_bytes(bytes).map_err(|_| CollectorError::DeserializationFailed)
    }
}

impl Default for SettlementBatch {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// COLLECTOR CONFIG
// ============================================================================

/// Configuration for the collector
#[derive(Clone, Debug)]
pub struct CollectorConfig {
    /// Minimum number of IOUs required to create a batch
    pub min_batch_size: u32,
    /// Maximum number of IOUs in a single batch
    pub max_batch_size: u32,
    /// Minimum age of IOU in seconds before collection
    pub min_iou_age_secs: u64,
    /// Minimum amount for an IOU to be collected
    pub min_amount: u64,
    /// Threshold amount that triggers automatic settlement
    pub settlement_threshold: u64,
}

impl CollectorConfig {
    /// Create a new config with builder pattern
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the minimum batch size
    pub fn with_min_batch_size(mut self, size: u32) -> Self {
        self.min_batch_size = size;
        self
    }

    /// Set the maximum batch size
    pub fn with_max_batch_size(mut self, size: u32) -> Self {
        self.max_batch_size = size;
        self
    }

    /// Set the minimum IOU age in seconds
    pub fn with_min_iou_age_secs(mut self, secs: u64) -> Self {
        self.min_iou_age_secs = secs;
        self
    }

    /// Set the minimum amount for collection
    pub fn with_min_amount(mut self, amount: u64) -> Self {
        self.min_amount = amount;
        self
    }

    /// Set the settlement threshold
    pub fn with_settlement_threshold(mut self, threshold: u64) -> Self {
        self.settlement_threshold = threshold;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), CollectorError> {
        if self.max_batch_size < self.min_batch_size {
            return Err(CollectorError::InvalidConfig(
                "max_batch_size must be >= min_batch_size".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            min_batch_size: 10,
            max_batch_size: 1000,
            min_iou_age_secs: 0,
            min_amount: 0,
            settlement_threshold: 0,
        }
    }
}

// ============================================================================
// COLLECTOR STATS
// ============================================================================

/// Statistics about collector operations
#[derive(Clone, Debug, Default)]
pub struct CollectorStats {
    pub total_collected: u64,
    pub total_amount_collected: u64,
    pub batches_created: u64,
}

// ============================================================================
// COLLECTOR ERROR
// ============================================================================

/// Errors that can occur during collection
#[derive(Error, Debug)]
pub enum CollectorError {
    #[error("Insufficient IOUs to create batch")]
    InsufficientIOUs,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Batch not found")]
    BatchNotFound,

    #[error("Deserialization failed")]
    DeserializationFailed,
}

// ============================================================================
// COLLECTOR
// ============================================================================

/// Collector for gathering IOUs for settlement
pub struct Collector {
    config: CollectorConfig,
    /// IOUs that have been collected but not yet batched
    collected_ious: Vec<SettlementEntry>,
    /// Set of IOU IDs that have been collected (to avoid duplicates)
    collected_ids: HashSet<Vec<u8>>,
    /// Pending batches that have been created
    batches: HashMap<BatchId, SettlementBatch>,
    /// Statistics
    stats: CollectorStats,
}

impl Collector {
    /// Create a new collector with the given configuration
    pub fn new(config: CollectorConfig) -> Self {
        Self {
            config,
            collected_ious: Vec::new(),
            collected_ids: HashSet::new(),
            batches: HashMap::new(),
            stats: CollectorStats::default(),
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &CollectorConfig {
        &self.config
    }

    /// Get the number of pending batches
    pub fn pending_batches(&self) -> usize {
        self.batches.len()
    }

    /// Get the total number of collected IOUs
    pub fn total_collected(&self) -> u64 {
        self.stats.total_collected
    }

    /// Collect IOUs from mesh state
    pub fn collect_from_state(&mut self, state: &MeshState) -> Result<usize, CollectorError> {
        let mut collected = 0;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        for entry in state.all_entries() {
            let iou = entry.iou();
            let iou_id = iou.id();
            let id_bytes = iou_id.as_bytes().to_vec();

            // Skip if already collected
            if self.collected_ids.contains(&id_bytes) {
                continue;
            }

            // Check minimum amount
            if iou.iou().amount() < self.config.min_amount {
                continue;
            }

            // Check minimum age
            let age = now.saturating_sub(iou.iou().timestamp());
            if age < self.config.min_iou_age_secs {
                continue;
            }

            // Collect this IOU
            let settlement_entry = SettlementEntry::from_iou(iou);
            self.stats.total_amount_collected += settlement_entry.amount;
            self.collected_ious.push(settlement_entry);
            self.collected_ids.insert(id_bytes);
            self.stats.total_collected += 1;
            collected += 1;
        }

        Ok(collected)
    }

    /// Collect IOUs by sender
    pub fn collect_by_sender(
        &mut self,
        state: &MeshState,
        sender: &Did,
    ) -> Result<usize, CollectorError> {
        let mut collected = 0;

        for entry in state.get_ious_by_sender(sender) {
            let iou = entry.iou();
            let iou_id = iou.id();
            let id_bytes = iou_id.as_bytes().to_vec();

            // Skip if already collected
            if self.collected_ids.contains(&id_bytes) {
                continue;
            }

            // Collect this IOU
            let settlement_entry = SettlementEntry::from_iou(iou);
            self.stats.total_amount_collected += settlement_entry.amount;
            self.collected_ious.push(settlement_entry);
            self.collected_ids.insert(id_bytes);
            self.stats.total_collected += 1;
            collected += 1;
        }

        Ok(collected)
    }

    /// Collect IOUs by recipient
    pub fn collect_by_recipient(
        &mut self,
        state: &MeshState,
        recipient: &Did,
    ) -> Result<usize, CollectorError> {
        let mut collected = 0;

        for entry in state.get_ious_by_recipient(recipient) {
            let iou = entry.iou();
            let iou_id = iou.id();
            let id_bytes = iou_id.as_bytes().to_vec();

            // Skip if already collected
            if self.collected_ids.contains(&id_bytes) {
                continue;
            }

            // Collect this IOU
            let settlement_entry = SettlementEntry::from_iou(iou);
            self.stats.total_amount_collected += settlement_entry.amount;
            self.collected_ious.push(settlement_entry);
            self.collected_ids.insert(id_bytes);
            self.stats.total_collected += 1;
            collected += 1;
        }

        Ok(collected)
    }

    /// Create a batch from collected IOUs
    pub fn create_batch(&mut self) -> Result<SettlementBatch, CollectorError> {
        if self.collected_ious.len() < self.config.min_batch_size as usize {
            return Err(CollectorError::InsufficientIOUs);
        }

        let mut batch = SettlementBatch::new();

        // Take up to max_batch_size entries
        let take_count = std::cmp::min(
            self.collected_ious.len(),
            self.config.max_batch_size as usize,
        );

        for entry in self.collected_ious.drain(..take_count) {
            batch.add_entry(entry);
        }

        self.stats.batches_created += 1;

        // Store the batch
        let batch_clone = batch.clone();
        self.batches.insert(batch.id().clone(), batch);

        Ok(batch_clone)
    }

    /// Get a batch by ID
    pub fn get_batch(&self, batch_id: &BatchId) -> Option<&SettlementBatch> {
        self.batches.get(batch_id)
    }

    /// Remove a batch by ID
    pub fn remove_batch(&mut self, batch_id: &BatchId) -> Result<(), CollectorError> {
        self.batches
            .remove(batch_id)
            .map(|_| ())
            .ok_or(CollectorError::BatchNotFound)
    }

    /// Update the status of a batch
    pub fn update_batch_status(
        &mut self,
        batch_id: &BatchId,
        status: BatchStatus,
    ) -> Result<(), CollectorError> {
        self.batches
            .get_mut(batch_id)
            .map(|batch| batch.set_status(status))
            .ok_or(CollectorError::BatchNotFound)
    }

    /// Clear all pending batches
    pub fn clear_batches(&mut self) {
        self.batches.clear();
    }

    /// Get statistics
    pub fn stats(&self) -> &CollectorStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = CollectorStats::default();
    }
}
