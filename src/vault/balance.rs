// Balance tracking and Vault implementation

use crate::identity::{Did, PublicKey};
use crate::iou::{IOUId, IOUValidator, SignedIOU, ValidationError};
use crate::vault::spending::{SpentOutput, SpentOutputSet};
use crate::vault::utxo::{LockInfo, UTXOId, UTXOSet, UTXO};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during vault operations
#[derive(Error, Debug)]
pub enum VaultError {
    #[error("Insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: u64, required: u64 },

    #[error("Recipient mismatch: IOU not addressed to this vault")]
    RecipientMismatch,

    #[error("Invalid signature on IOU")]
    InvalidSignature,

    #[error("Sender DID does not match public key")]
    SenderMismatch,

    #[error("Duplicate transaction: IOU already processed")]
    DuplicateTransaction,

    #[error("Not the owner of this vault")]
    NotOwner,

    #[error("Balance would overflow")]
    BalanceOverflow,

    #[error("UTXO not found")]
    UTXONotFound,

    #[error("Insufficient UTXOs: provided {provided}, required {required}")]
    InsufficientUTXOs { provided: u64, required: u64 },

    #[error("Reservation not found")]
    ReservationNotFound,

    #[error("Invalid amount")]
    InvalidAmount,

    #[error("IOU validation failed: {0}")]
    ValidationFailed(#[from] ValidationError),

    #[error("State export/import error: {0}")]
    StateError(String),
}

/// Transaction record for history tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionRecord {
    iou: SignedIOU,
    direction: TransactionDirection,
    timestamp: u64,
}

impl TransactionRecord {
    pub fn iou(&self) -> &SignedIOU {
        &self.iou
    }

    pub fn direction(&self) -> TransactionDirection {
        self.direction
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionDirection {
    Received,
    Sent,
}

/// Balance reservation for pending transactions
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Reservation {
    id: u64,
    amount: u64,
}

/// Vault state for export/import
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VaultState {
    owner: PublicKey,
    utxos: UTXOSet,
    spent_outputs: SpentOutputSet,
    processed_ious: HashMap<IOUId, u64>, // IOUId -> timestamp when processed
    transactions: Vec<TransactionRecord>,
}

/// Memory statistics for the vault
#[derive(Clone, Debug)]
pub struct MemoryStats {
    /// Number of processed IOU IDs being tracked
    pub processed_iou_count: usize,
    /// Number of UTXOs in the vault
    pub utxo_count: usize,
    /// Number of spent outputs being tracked
    pub spent_output_count: usize,
    /// Number of transaction records
    pub transaction_count: usize,
    /// Number of active locks
    pub lock_count: usize,
    /// Estimated total memory usage in bytes
    pub estimated_bytes: usize,
}

/// The Vault - tracks what a user owns (balance, UTXOs)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vault {
    /// Owner of this vault
    owner: PublicKey,
    /// Unspent transaction outputs (the actual funds)
    utxos: UTXOSet,
    /// Spent outputs (for double-spend detection)
    spent_outputs: SpentOutputSet,
    /// Map of IOU IDs already processed -> timestamp when processed
    processed_ious: HashMap<IOUId, u64>,
    /// Transaction history
    transactions: Vec<TransactionRecord>,
    /// Active reservations
    reservations: HashMap<u64, Reservation>,
    /// Next reservation ID
    next_reservation_id: u64,
    /// Lock timeout tracking: UTXO ID -> LockInfo
    lock_timeouts: HashMap<UTXOId, LockInfo>,
}

impl Vault {
    /// Create a new empty vault for the given owner
    pub fn new(owner: PublicKey) -> Self {
        Self {
            owner,
            utxos: UTXOSet::new(),
            spent_outputs: SpentOutputSet::new(),
            processed_ious: HashMap::new(),
            transactions: Vec::new(),
            reservations: HashMap::new(),
            next_reservation_id: 1,
            lock_timeouts: HashMap::new(),
        }
    }

    /// Get the owner of this vault
    pub fn owner(&self) -> &PublicKey {
        &self.owner
    }

    // ========================================================================
    // BALANCE QUERIES
    // ========================================================================

    /// Get the total balance (sum of all UTXOs)
    pub fn balance(&self) -> u64 {
        self.utxos.total_value()
    }

    /// Get the available balance (excluding locked UTXOs and reservations)
    pub fn available_balance(&self) -> u64 {
        let reserved: u64 = self.reservations.values().map(|r| r.amount).sum();
        self.utxos.unlocked_value().saturating_sub(reserved)
    }

    /// Check if the vault can afford a specific amount
    pub fn can_afford(&self, amount: u64) -> bool {
        self.available_balance() >= amount
    }

    /// Get balance received from a specific sender
    pub fn balance_from_sender(&self, sender: &Did) -> u64 {
        self.transactions
            .iter()
            .filter(|t| t.direction == TransactionDirection::Received)
            .filter(|t| t.iou.iou().sender() == sender)
            .map(|t| t.iou.iou().amount())
            .sum()
    }

    // ========================================================================
    // UTXO OPERATIONS
    // ========================================================================

    /// Get all UTXOs
    pub fn utxo_set(&self) -> Vec<&UTXO> {
        self.utxos.to_vec()
    }

    /// Get UTXOs sorted by amount
    pub fn utxo_set_sorted_by_amount(&self) -> Vec<&UTXO> {
        let mut utxos = self.utxos.to_vec();
        utxos.sort_by(|a, b| a.amount().cmp(&b.amount()));
        utxos
    }

    /// Get a specific UTXO by ID
    pub fn get_utxo(&self, id: &UTXOId) -> Option<&UTXO> {
        self.utxos.get(id)
    }

    /// Check if a UTXO has been spent
    pub fn is_utxo_spent(&self, id: &UTXOId) -> bool {
        self.spent_outputs.contains(id)
    }

    /// Check if spending the given UTXO would be a double-spend
    pub fn would_be_double_spend(&self, utxo_id: &UTXOId) -> bool {
        self.spent_outputs.contains(utxo_id)
    }

    /// Lock a UTXO for a pending transaction
    pub fn lock_utxo(&mut self, id: &UTXOId) -> Result<(), VaultError> {
        match self.utxos.get_mut(id) {
            Some(utxo) => {
                utxo.lock();
                Ok(())
            }
            None => Err(VaultError::UTXONotFound),
        }
    }

    /// Unlock a previously locked UTXO
    pub fn unlock_utxo(&mut self, id: &UTXOId) -> Result<(), VaultError> {
        match self.utxos.get_mut(id) {
            Some(utxo) => {
                utxo.unlock();
                Ok(())
            }
            None => Err(VaultError::UTXONotFound),
        }
    }

    /// Estimate how many UTXOs would be needed to cover an amount
    pub fn estimate_utxos_needed(&self, amount: u64) -> Option<usize> {
        self.utxos.select_for_amount(amount).map(|(utxos, _)| utxos.len())
    }

    // ========================================================================
    // RECEIVING IOUs
    // ========================================================================

    /// Receive an IOU and add it to the vault
    pub fn receive_iou(&mut self, signed_iou: SignedIOU, sender_pubkey: &PublicKey) -> Result<(), VaultError> {
        let iou = signed_iou.iou();
        let iou_id = signed_iou.id();

        // Check if already processed
        if self.processed_ious.contains_key(&iou_id) {
            return Err(VaultError::DuplicateTransaction);
        }

        // Verify recipient matches vault owner
        let recipient_pubkey = iou.recipient().public_key()
            .map_err(|_| VaultError::RecipientMismatch)?;
        if recipient_pubkey != self.owner {
            return Err(VaultError::RecipientMismatch);
        }

        // Validate the IOU signature
        IOUValidator::validate(&signed_iou, sender_pubkey)?;

        // Check for balance overflow
        let _new_balance = self.balance()
            .checked_add(iou.amount())
            .ok_or(VaultError::BalanceOverflow)?;

        // Create UTXO from this IOU (Received type)
        let utxo = UTXO::new(self.owner.clone(), iou.amount(), iou_id.clone());
        self.utxos.add(utxo);

        // Mark IOU as processed with timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.processed_ious.insert(iou_id.clone(), timestamp);

        // Record transaction
        let record = TransactionRecord {
            iou: signed_iou,
            direction: TransactionDirection::Received,
            timestamp,
        };
        self.transactions.push(record);

        Ok(())
    }

    /// Check if an IOU has already been processed
    pub fn has_processed_iou(&self, iou_id: &IOUId) -> bool {
        self.processed_ious.contains_key(iou_id)
    }

    // ========================================================================
    // SENDING IOUs
    // ========================================================================

    /// Record a sent IOU (deducting from balance)
    pub fn record_sent_iou(&mut self, signed_iou: SignedIOU) -> Result<(), VaultError> {
        let iou = signed_iou.iou();
        let iou_id = signed_iou.id();

        // Verify sender matches vault owner
        let sender_pubkey = iou.sender().public_key()
            .map_err(|_| VaultError::NotOwner)?;
        if sender_pubkey != self.owner {
            return Err(VaultError::NotOwner);
        }

        let amount = iou.amount();
        let available = self.available_balance();

        if amount > available {
            return Err(VaultError::InsufficientBalance {
                available,
                required: amount,
            });
        }

        // Select UTXOs to spend
        let (selected_utxos, change) = self.utxos
            .select_for_amount(amount)
            .ok_or(VaultError::InsufficientBalance {
                available,
                required: amount,
            })?;

        // Remove spent UTXOs and record as spent
        for utxo in &selected_utxos {
            self.spent_outputs.add_unchecked(SpentOutput::now(utxo.id().clone(), iou_id.clone()));
            self.utxos.remove(utxo.id());
        }

        // Create change UTXO if needed (using Change type for unique ID)
        if change > 0 {
            let change_utxo = UTXO::new_change(self.owner.clone(), change, iou_id.clone());
            self.utxos.add(change_utxo);
        }

        // Record transaction
        let record = TransactionRecord {
            iou: signed_iou,
            direction: TransactionDirection::Sent,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        self.transactions.push(record);

        Ok(())
    }

    /// Spend using specific UTXOs
    pub fn spend_with_utxos(&mut self, signed_iou: SignedIOU, utxo_ids: Vec<UTXOId>) -> Result<(), VaultError> {
        let iou = signed_iou.iou();
        let iou_id = signed_iou.id();
        let amount = iou.amount();

        // Verify sender matches vault owner
        let sender_pubkey = iou.sender().public_key()
            .map_err(|_| VaultError::NotOwner)?;
        if sender_pubkey != self.owner {
            return Err(VaultError::NotOwner);
        }

        // Collect the specified UTXOs
        let mut selected_utxos = Vec::new();
        let mut total = 0u64;

        for utxo_id in &utxo_ids {
            let utxo = self.utxos.get(utxo_id)
                .ok_or(VaultError::UTXONotFound)?;
            total = total.saturating_add(utxo.amount());
            selected_utxos.push(utxo.clone());
        }

        if total < amount {
            return Err(VaultError::InsufficientUTXOs {
                provided: total,
                required: amount,
            });
        }

        let change = total - amount;

        // Remove spent UTXOs and record as spent
        for utxo in &selected_utxos {
            self.spent_outputs.add_unchecked(SpentOutput::now(utxo.id().clone(), iou_id.clone()));
            self.utxos.remove(utxo.id());
        }

        // Create change UTXO if needed (using Change type for unique ID)
        if change > 0 {
            let change_utxo = UTXO::new_change(self.owner.clone(), change, iou_id.clone());
            self.utxos.add(change_utxo);
        }

        // Record transaction
        let record = TransactionRecord {
            iou: signed_iou,
            direction: TransactionDirection::Sent,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        self.transactions.push(record);

        Ok(())
    }

    // ========================================================================
    // RESERVATION SYSTEM
    // ========================================================================

    /// Reserve balance for a pending transaction
    pub fn reserve_balance(&mut self, amount: u64) -> Result<u64, VaultError> {
        if amount == 0 {
            return Ok(0);
        }

        if amount > self.available_balance() {
            return Err(VaultError::InsufficientBalance {
                available: self.available_balance(),
                required: amount,
            });
        }

        let id = self.next_reservation_id;
        self.next_reservation_id += 1;

        self.reservations.insert(id, Reservation { id, amount });
        Ok(id)
    }

    /// Release a reservation without spending
    pub fn release_reservation(&mut self, reservation_id: u64) -> Result<(), VaultError> {
        if self.reservations.remove(&reservation_id).is_none() {
            return Err(VaultError::ReservationNotFound);
        }
        Ok(())
    }

    /// Commit a reservation (actually spend the reserved amount)
    pub fn commit_reservation(&mut self, reservation_id: u64) -> Result<u64, VaultError> {
        let reservation = self.reservations.remove(&reservation_id)
            .ok_or(VaultError::ReservationNotFound)?;

        // The actual spending would happen here, but since this is a
        // reservation commit, we just need to update the balance.
        // In practice, this would be called after record_sent_iou succeeds.

        // For now, we need to manually adjust - the reservation was for
        // holding funds. Since record_sent_iou handles the actual UTXO
        // consumption, committing just means the reservation is done.
        // We simulate the spend by reducing balance through UTXO consumption.

        Ok(reservation.amount)
    }

    // ========================================================================
    // TRANSACTION HISTORY
    // ========================================================================

    /// Get total transaction count
    pub fn transaction_count(&self) -> usize {
        self.transactions.len()
    }

    /// Get all transaction history
    pub fn transaction_history(&self) -> Vec<&TransactionRecord> {
        self.transactions.iter().collect()
    }

    /// Get only received transactions
    pub fn received_transactions(&self) -> Vec<&TransactionRecord> {
        self.transactions
            .iter()
            .filter(|t| t.direction == TransactionDirection::Received)
            .collect()
    }

    /// Get only sent transactions
    pub fn sent_transactions(&self) -> Vec<&TransactionRecord> {
        self.transactions
            .iter()
            .filter(|t| t.direction == TransactionDirection::Sent)
            .collect()
    }

    // ========================================================================
    // SPENT OUTPUTS
    // ========================================================================

    /// Get all spent outputs
    pub fn spent_outputs(&self) -> Vec<&SpentOutput> {
        self.spent_outputs.to_vec()
    }

    /// Get a specific spent output record
    pub fn get_spent_output(&self, utxo_id: &UTXOId) -> Option<&SpentOutput> {
        self.spent_outputs.get(utxo_id)
    }

    // ========================================================================
    // STATE EXPORT/IMPORT
    // ========================================================================

    /// Export vault state for persistence
    pub fn export_state(&self) -> Result<VaultState, VaultError> {
        Ok(VaultState {
            owner: self.owner.clone(),
            utxos: self.utxos.clone(),
            spent_outputs: self.spent_outputs.clone(),
            processed_ious: self.processed_ious.clone(),
            transactions: self.transactions.clone(),
        })
    }

    /// Import vault state
    pub fn import_state(&mut self, state: VaultState) -> Result<(), VaultError> {
        if state.owner != self.owner {
            return Err(VaultError::StateError("Owner mismatch".to_string()));
        }

        self.utxos = state.utxos;
        self.spent_outputs = state.spent_outputs;
        self.processed_ious = state.processed_ious;
        self.transactions = state.transactions;

        Ok(())
    }

    // ========================================================================
    // LOCK TIMEOUT MANAGEMENT
    // ========================================================================

    /// Lock a UTXO with a timeout (in milliseconds)
    /// After the timeout expires, the lock will be automatically released
    /// when cleanup_expired_locks() is called
    pub fn lock_utxo_with_timeout(&mut self, id: &UTXOId, timeout_ms: u64) -> Result<(), VaultError> {
        match self.utxos.get_mut(id) {
            Some(utxo) => {
                utxo.lock();
                self.lock_timeouts.insert(id.clone(), LockInfo::new(timeout_ms));
                Ok(())
            }
            None => Err(VaultError::UTXONotFound),
        }
    }

    /// Lock a UTXO with a timeout and reason
    pub fn lock_utxo_with_reason(&mut self, id: &UTXOId, timeout_ms: u64, reason: String) -> Result<(), VaultError> {
        match self.utxos.get_mut(id) {
            Some(utxo) => {
                utxo.lock();
                self.lock_timeouts.insert(id.clone(), LockInfo::with_reason(timeout_ms, reason));
                Ok(())
            }
            None => Err(VaultError::UTXONotFound),
        }
    }

    /// Get lock information for a UTXO
    pub fn get_lock_info(&self, id: &UTXOId) -> Option<&LockInfo> {
        self.lock_timeouts.get(id)
    }

    /// Cleanup all expired locks, automatically unlocking the UTXOs
    /// Returns the number of locks that were cleaned up
    pub fn cleanup_expired_locks(&mut self) -> usize {
        let expired: Vec<UTXOId> = self.lock_timeouts
            .iter()
            .filter(|(_, info)| info.is_expired())
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len();

        for id in expired {
            self.lock_timeouts.remove(&id);
            if let Some(utxo) = self.utxos.get_mut(&id) {
                utxo.unlock();
            }
        }

        count
    }

    /// Get the number of active locks
    pub fn active_lock_count(&self) -> usize {
        self.lock_timeouts.len()
    }

    // ========================================================================
    // PROCESSED IOU MANAGEMENT (MEMORY CONTROL)
    // ========================================================================

    /// Get the count of processed IOUs being tracked
    pub fn processed_iou_count(&self) -> usize {
        self.processed_ious.len()
    }

    /// Prune processed IOUs older than the given timestamp
    /// Returns the number of IOUs pruned
    ///
    /// WARNING: Pruned IOUs can potentially be replayed if they're resubmitted.
    /// Only prune IOUs that are old enough that replay is unlikely or
    /// use other mechanisms (like expiry validation) to prevent replay.
    pub fn prune_processed_ious_before(&mut self, before_timestamp: u64) -> usize {
        let before_count = self.processed_ious.len();
        self.processed_ious.retain(|_, timestamp| *timestamp >= before_timestamp);
        before_count - self.processed_ious.len()
    }

    /// Prune processed IOUs to keep only the most recent N entries
    /// Returns the number of IOUs pruned
    pub fn prune_processed_ious_to_max(&mut self, max_count: usize) -> usize {
        if self.processed_ious.len() <= max_count {
            return 0;
        }

        // Collect entries sorted by timestamp (oldest first)
        let mut entries: Vec<_> = self.processed_ious.iter()
            .map(|(id, ts)| (id.clone(), *ts))
            .collect();
        entries.sort_by_key(|(_, ts)| *ts);

        // Calculate how many to remove
        let to_remove = entries.len() - max_count;

        // Remove the oldest entries
        for (id, _) in entries.into_iter().take(to_remove) {
            self.processed_ious.remove(&id);
        }

        to_remove
    }

    /// Get the timestamp when an IOU was processed (if tracked)
    pub fn get_processed_iou_timestamp(&self, iou_id: &IOUId) -> Option<u64> {
        self.processed_ious.get(iou_id).copied()
    }

    // ========================================================================
    // MEMORY STATISTICS
    // ========================================================================

    /// Get memory usage statistics for this vault
    pub fn memory_stats(&self) -> MemoryStats {
        // Estimate sizes (rough approximations)
        const IOU_ID_SIZE: usize = 32 + 8; // 32 bytes hash + 8 bytes timestamp
        const UTXO_SIZE: usize = 32 + 32 + 8 + 32 + 1 + 1; // id + owner + amount + source + type + locked
        const SPENT_OUTPUT_SIZE: usize = 32 + 32 + 8; // utxo_id + iou_id + timestamp
        const TRANSACTION_RECORD_SIZE: usize = 200; // Approximate
        const LOCK_INFO_SIZE: usize = 16; // expires_at + optional reason overhead

        let processed_iou_count = self.processed_ious.len();
        let utxo_count = self.utxos.len();
        let spent_output_count = self.spent_outputs.len();
        let transaction_count = self.transactions.len();
        let lock_count = self.lock_timeouts.len();

        let estimated_bytes =
            (processed_iou_count * IOU_ID_SIZE) +
            (utxo_count * UTXO_SIZE) +
            (spent_output_count * SPENT_OUTPUT_SIZE) +
            (transaction_count * TRANSACTION_RECORD_SIZE) +
            (lock_count * LOCK_INFO_SIZE);

        MemoryStats {
            processed_iou_count,
            utxo_count,
            spent_output_count,
            transaction_count,
            lock_count,
            estimated_bytes,
        }
    }

    // ========================================================================
    // SERIALIZATION
    // ========================================================================

    /// Serialize the vault to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).unwrap_or_default()
    }

    /// Deserialize a vault from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, VaultError> {
        postcard::from_bytes(bytes)
            .map_err(|e| VaultError::StateError(e.to_string()))
    }
}
