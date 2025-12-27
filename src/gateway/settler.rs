// Settler - Pushes settlements to external systems
// Responsible for submitting batches to banks, blockchains, or other settlement targets

use super::{BatchId, BatchStatus, SettlementBatch};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

// ============================================================================
// SETTLEMENT TARGET TRAIT
// ============================================================================

/// Trait for settlement targets (banks, blockchains, etc.)
#[async_trait]
pub trait SettlementTarget: Send + Sync {
    /// Attempt to settle a batch
    /// Returns transaction ID on success, error message on failure
    async fn settle(&self, batch: &SettlementBatch) -> Result<String, String>;
}

// ============================================================================
// MOCK SETTLEMENT TARGET
// ============================================================================

/// Mock implementation of SettlementTarget for testing
pub struct MockSettlementTarget {
    should_succeed: bool,
    failure_message: Option<String>,
    delay_ms: u64,
    failures_before_success: AtomicUsize,
    call_count: AtomicUsize,
}

impl MockSettlementTarget {
    /// Create a new mock target (defaults to failure)
    pub fn new() -> Self {
        Self {
            should_succeed: false,
            failure_message: None,
            delay_ms: 0,
            failures_before_success: AtomicUsize::new(0),
            call_count: AtomicUsize::new(0),
        }
    }

    /// Configure to always succeed
    pub fn with_success(mut self) -> Self {
        self.should_succeed = true;
        self
    }

    /// Configure to always fail with a message
    pub fn with_failure(mut self, message: String) -> Self {
        self.should_succeed = false;
        self.failure_message = Some(message);
        self
    }

    /// Add a delay before responding
    pub fn with_delay_ms(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }

    /// Fail N times, then succeed
    pub fn with_failures_then_success(mut self, failures: usize) -> Self {
        self.should_succeed = true;
        self.failures_before_success = AtomicUsize::new(failures);
        self
    }
}

impl Default for MockSettlementTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SettlementTarget for MockSettlementTarget {
    async fn settle(&self, _batch: &SettlementBatch) -> Result<String, String> {
        // Apply delay if configured
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }

        let call_num = self.call_count.fetch_add(1, Ordering::SeqCst);
        let failures_remaining = self.failures_before_success.load(Ordering::SeqCst);

        // Check if we should fail first
        if failures_remaining > 0 && call_num < failures_remaining {
            return Err(self
                .failure_message
                .clone()
                .unwrap_or_else(|| "Mock failure".to_string()));
        }

        if self.should_succeed {
            Ok(format!("tx-mock-{}", call_num))
        } else {
            Err(self
                .failure_message
                .clone()
                .unwrap_or_else(|| "Mock failure".to_string()))
        }
    }
}

// ============================================================================
// SETTLEMENT RECEIPT
// ============================================================================

/// Receipt from a successful settlement
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettlementReceipt {
    transaction_id: String,
    amount: u64,
    timestamp: u64,
    metadata: HashMap<String, String>,
}

impl SettlementReceipt {
    /// Create a new receipt
    pub fn new(transaction_id: &str, amount: u64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            transaction_id: transaction_id.to_string(),
            amount,
            timestamp,
            metadata: HashMap::new(),
        }
    }

    /// Get the transaction ID
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    /// Get the amount
    pub fn amount(&self) -> u64 {
        self.amount
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Get metadata by key
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SettlerError> {
        postcard::from_bytes(bytes).map_err(|_| SettlerError::DeserializationFailed)
    }
}

// ============================================================================
// SETTLEMENT RESULT
// ============================================================================

/// Result of a settlement attempt
#[derive(Clone, Debug)]
pub struct SettlementResult {
    batch_id: BatchId,
    success: bool,
    transaction_id: Option<String>,
    error_message: Option<String>,
    attempts: u32,
    receipt: Option<SettlementReceipt>,
}

impl SettlementResult {
    /// Create a success result
    pub fn success(batch_id: BatchId, transaction_id: String) -> Self {
        Self {
            batch_id,
            success: true,
            transaction_id: Some(transaction_id),
            error_message: None,
            attempts: 1,
            receipt: None,
        }
    }

    /// Create a failure result
    pub fn failure(batch_id: BatchId, error_message: String) -> Self {
        Self {
            batch_id,
            success: false,
            transaction_id: None,
            error_message: Some(error_message),
            attempts: 1,
            receipt: None,
        }
    }

    /// Check if the result is successful
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Get the batch ID
    pub fn batch_id(&self) -> &BatchId {
        &self.batch_id
    }

    /// Get the transaction ID (if successful)
    pub fn transaction_id(&self) -> Option<&str> {
        self.transaction_id.as_deref()
    }

    /// Get the error message (if failed)
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    /// Get the number of attempts
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Set the number of attempts
    pub fn with_attempts(mut self, attempts: u32) -> Self {
        self.attempts = attempts;
        self
    }

    /// Get the receipt (if present)
    pub fn receipt(&self) -> Option<&SettlementReceipt> {
        self.receipt.as_ref()
    }

    /// Set the receipt
    pub fn with_receipt(mut self, receipt: SettlementReceipt) -> Self {
        self.receipt = Some(receipt);
        self
    }
}

// ============================================================================
// SETTLER EVENTS
// ============================================================================

/// Events emitted by the settler
#[derive(Clone, Debug)]
pub enum SettlerEvent {
    /// A batch was submitted for settlement
    BatchSubmitted {
        batch_id: BatchId,
        entries: usize,
        total_amount: u64,
    },
    /// Settlement completed successfully
    SettlementComplete {
        batch_id: BatchId,
        success: bool,
        transaction_id: Option<String>,
    },
    /// Settlement failed after all retries
    SettlementFailed {
        batch_id: BatchId,
        error: String,
        attempts: u32,
    },
}

// ============================================================================
// SETTLER CONFIG
// ============================================================================

/// Configuration for the settler
#[derive(Clone, Debug)]
pub struct SettlerConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Delay between retries in seconds
    pub retry_delay_secs: u64,
    /// Timeout for settlement attempts in seconds
    pub timeout_secs: u64,
    /// Optional endpoint URL for the settlement target
    pub endpoint: Option<String>,
    /// Optional API key for authentication
    pub api_key: Option<String>,
}

impl SettlerConfig {
    /// Create a new config with builder pattern
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of retries
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set the retry delay in seconds
    pub fn with_retry_delay_secs(mut self, secs: u64) -> Self {
        self.retry_delay_secs = secs;
        self
    }

    /// Set the timeout in seconds
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set the endpoint URL
    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = Some(endpoint.to_string());
        self
    }

    /// Set the API key
    pub fn with_api_key(mut self, key: &str) -> Self {
        self.api_key = Some(key.to_string());
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), SettlerError> {
        if self.timeout_secs == 0 {
            return Err(SettlerError::InvalidConfig(
                "timeout_secs must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for SettlerConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_secs: 10,
            timeout_secs: 60,
            endpoint: None,
            api_key: None,
        }
    }
}

// ============================================================================
// SETTLER STATS
// ============================================================================

/// Statistics about settler operations
#[derive(Clone, Debug, Default)]
pub struct SettlerStats {
    pub batches_submitted: u64,
    pub batches_settled: u64,
    pub batches_failed: u64,
    pub total_entries_settled: u64,
    pub total_amount_settled: u64,
}

// ============================================================================
// SETTLER ERROR
// ============================================================================

/// Errors that can occur during settlement
#[derive(Error, Debug)]
pub enum SettlerError {
    #[error("Empty batch cannot be submitted")]
    EmptyBatch,

    #[error("No settlement target configured")]
    NoTarget,

    #[error("Duplicate batch: already submitted")]
    DuplicateBatch,

    #[error("Batch not found")]
    BatchNotFound,

    #[error("Batch already processed")]
    BatchAlreadyProcessed,

    #[error("Settlement timed out")]
    Timeout,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Deserialization failed")]
    DeserializationFailed,
}

// ============================================================================
// SETTLER
// ============================================================================

/// Settler for submitting batches to external systems
pub struct Settler {
    config: SettlerConfig,
    target: Option<Box<dyn SettlementTarget>>,
    /// Batches that have been submitted
    batches: HashMap<BatchId, SettlementBatch>,
    /// Results of processed batches
    results: HashMap<BatchId, SettlementResult>,
    /// Events queue
    events: Vec<SettlerEvent>,
    /// Statistics
    stats: SettlerStats,
}

impl Settler {
    /// Create a new settler without a target
    pub fn new(config: SettlerConfig) -> Self {
        Self {
            config,
            target: None,
            batches: HashMap::new(),
            results: HashMap::new(),
            events: Vec::new(),
            stats: SettlerStats::default(),
        }
    }

    /// Create a new settler with a settlement target
    pub fn with_target(config: SettlerConfig, target: Box<dyn SettlementTarget>) -> Self {
        Self {
            config,
            target: Some(target),
            batches: HashMap::new(),
            results: HashMap::new(),
            events: Vec::new(),
            stats: SettlerStats::default(),
        }
    }

    /// Check if a target is configured
    pub fn has_target(&self) -> bool {
        self.target.is_some()
    }

    /// Get the number of pending settlements
    pub fn pending_settlements(&self) -> usize {
        self.batches
            .values()
            .filter(|b| matches!(b.status(), BatchStatus::Pending | BatchStatus::Processing))
            .count()
    }

    /// Get the number of completed settlements
    pub fn completed_settlements(&self) -> usize {
        self.batches
            .values()
            .filter(|b| matches!(b.status(), BatchStatus::Confirmed))
            .count()
    }

    /// Submit a batch for settlement
    pub async fn submit(&mut self, batch: SettlementBatch) -> Result<(), SettlerError> {
        // Check for empty batch
        if batch.entries().is_empty() {
            return Err(SettlerError::EmptyBatch);
        }

        // Check for target
        if self.target.is_none() {
            return Err(SettlerError::NoTarget);
        }

        // Check for duplicate
        if self.batches.contains_key(batch.id()) {
            return Err(SettlerError::DuplicateBatch);
        }

        // Emit event
        self.events.push(SettlerEvent::BatchSubmitted {
            batch_id: batch.id().clone(),
            entries: batch.entries().len(),
            total_amount: batch.total_amount(),
        });

        self.stats.batches_submitted += 1;

        // Store the batch
        self.batches.insert(batch.id().clone(), batch);

        Ok(())
    }

    /// Process a submitted batch
    pub async fn process(&mut self, batch_id: &BatchId) -> Result<SettlementResult, SettlerError> {
        // Get the batch
        let batch = self
            .batches
            .get_mut(batch_id)
            .ok_or(SettlerError::BatchNotFound)?;

        // Get the target
        let target = self.target.as_ref().ok_or(SettlerError::NoTarget)?;

        // Update status
        batch.set_status(BatchStatus::Processing);

        // Try to settle with retries
        let mut attempts = 0u32;
        let mut last_error = String::new();

        loop {
            attempts += 1;

            // Create timeout future
            let settle_future = target.settle(batch);
            let timeout_duration = Duration::from_secs(self.config.timeout_secs);

            let result = tokio::time::timeout(timeout_duration, settle_future).await;

            match result {
                Ok(Ok(tx_id)) => {
                    // Success!
                    let batch = self.batches.get_mut(batch_id).unwrap();
                    batch.set_status(BatchStatus::Confirmed);

                    self.stats.batches_settled += 1;
                    self.stats.total_entries_settled += batch.entries().len() as u64;
                    self.stats.total_amount_settled += batch.total_amount();

                    self.events.push(SettlerEvent::SettlementComplete {
                        batch_id: batch_id.clone(),
                        success: true,
                        transaction_id: Some(tx_id.clone()),
                    });

                    let result = SettlementResult::success(batch_id.clone(), tx_id)
                        .with_attempts(attempts);
                    self.results.insert(batch_id.clone(), result.clone());

                    return Ok(result);
                }
                Ok(Err(e)) => {
                    last_error = e;
                }
                Err(_) => {
                    last_error = "Timeout".to_string();
                }
            }

            // Check if we should retry
            if attempts > self.config.max_retries {
                break;
            }

            // Wait before retry
            if self.config.retry_delay_secs > 0 {
                tokio::time::sleep(Duration::from_secs(self.config.retry_delay_secs)).await;
            }
        }

        // All retries exhausted
        let batch = self.batches.get_mut(batch_id).unwrap();
        batch.set_status(BatchStatus::Failed);

        self.stats.batches_failed += 1;

        self.events.push(SettlerEvent::SettlementFailed {
            batch_id: batch_id.clone(),
            error: last_error.clone(),
            attempts,
        });

        self.events.push(SettlerEvent::SettlementComplete {
            batch_id: batch_id.clone(),
            success: false,
            transaction_id: None,
        });

        let result =
            SettlementResult::failure(batch_id.clone(), last_error).with_attempts(attempts);
        self.results.insert(batch_id.clone(), result.clone());

        Ok(result)
    }

    /// Cancel a pending batch
    pub fn cancel(&mut self, batch_id: &BatchId) -> Result<(), SettlerError> {
        let batch = self
            .batches
            .get_mut(batch_id)
            .ok_or(SettlerError::BatchNotFound)?;

        // Can only cancel pending batches
        match batch.status() {
            BatchStatus::Confirmed | BatchStatus::Failed => {
                return Err(SettlerError::BatchAlreadyProcessed);
            }
            _ => {}
        }

        // Remove the batch
        self.batches.remove(batch_id);

        Ok(())
    }

    /// Get the status of a batch
    pub fn get_status(&self, batch_id: &BatchId) -> Option<BatchStatus> {
        self.batches.get(batch_id).map(|b| b.status().clone())
    }

    /// List batches by status
    pub fn list_by_status(&self, status: BatchStatus) -> Vec<&SettlementBatch> {
        self.batches
            .values()
            .filter(|b| b.status() == &status)
            .collect()
    }

    /// Poll for events (clears the event queue)
    pub fn poll_events(&mut self) -> Vec<SettlerEvent> {
        std::mem::take(&mut self.events)
    }

    /// Get statistics
    pub fn stats(&self) -> &SettlerStats {
        &self.stats
    }
}
