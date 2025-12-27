// Settler Tests
// Tests for pushing settlements to external systems (bank/blockchain)

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::IOUBuilder;
use p2pmesh::gateway::{
    Settler, SettlerConfig, SettlerError, SettlerEvent,
    SettlementBatch, SettlementEntry, BatchStatus, BatchId,
    SettlementResult, SettlementReceipt,
    SettlementTarget, MockSettlementTarget,
};

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn create_test_batch(num_entries: usize) -> SettlementBatch {
    let mut batch = SettlementBatch::new();

    let alice = Keypair::generate();
    let bob = Keypair::generate();

    for i in 0..num_entries {
        let iou = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(100)
            .nonce(i as u64)
            .build()
            .unwrap();

        batch.add_entry(SettlementEntry::from_iou(&iou));
    }

    batch
}

// ============================================================================
// SETTLER CONFIG
// ============================================================================

#[test]
fn test_settler_config_default() {
    let config = SettlerConfig::default();

    assert!(config.max_retries > 0);
    assert!(config.retry_delay_secs > 0);
    assert!(config.timeout_secs > 0);
}

#[test]
fn test_settler_config_custom() {
    let config = SettlerConfig::new()
        .with_max_retries(5)
        .with_retry_delay_secs(30)
        .with_timeout_secs(120);

    assert_eq!(config.max_retries, 5);
    assert_eq!(config.retry_delay_secs, 30);
    assert_eq!(config.timeout_secs, 120);
}

#[test]
fn test_settler_config_with_endpoint() {
    let config = SettlerConfig::new()
        .with_endpoint("https://bank.example.com/settle");

    assert_eq!(config.endpoint, Some("https://bank.example.com/settle".to_string()));
}

#[test]
fn test_settler_config_with_api_key() {
    let config = SettlerConfig::new()
        .with_api_key("secret-key-123");

    assert_eq!(config.api_key, Some("secret-key-123".to_string()));
}

#[test]
fn test_settler_config_validation() {
    let valid_config = SettlerConfig::default();
    assert!(valid_config.validate().is_ok());

    let invalid_config = SettlerConfig::new()
        .with_timeout_secs(0);
    assert!(invalid_config.validate().is_err());
}

// ============================================================================
// SETTLER CREATION
// ============================================================================

#[test]
fn test_settler_new() {
    let config = SettlerConfig::default();
    let settler = Settler::new(config);

    assert_eq!(settler.pending_settlements(), 0);
    assert_eq!(settler.completed_settlements(), 0);
}

#[test]
fn test_settler_with_mock_target() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new();
    let settler = Settler::with_target(config, Box::new(target));

    assert!(settler.has_target());
}

// ============================================================================
// SETTLEMENT SUBMISSION
// ============================================================================

#[tokio::test]
async fn test_settler_submit_batch() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_success();
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(3);
    let batch_id = batch.id().clone();

    let result = settler.submit(batch).await;

    assert!(result.is_ok());
    assert_eq!(settler.pending_settlements(), 1);
}

#[tokio::test]
async fn test_settler_submit_empty_batch() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new();
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = SettlementBatch::new(); // Empty

    let result = settler.submit(batch).await;

    assert!(matches!(result, Err(SettlerError::EmptyBatch)));
}

#[tokio::test]
async fn test_settler_submit_without_target() {
    let config = SettlerConfig::default();
    let mut settler = Settler::new(config);

    let batch = create_test_batch(1);

    let result = settler.submit(batch).await;

    assert!(matches!(result, Err(SettlerError::NoTarget)));
}

#[tokio::test]
async fn test_settler_submit_duplicate_batch() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_success();
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(1);
    let batch_clone = batch.clone();

    settler.submit(batch).await.unwrap();
    let result = settler.submit(batch_clone).await;

    assert!(matches!(result, Err(SettlerError::DuplicateBatch)));
}

// ============================================================================
// SETTLEMENT PROCESSING
// ============================================================================

#[tokio::test]
async fn test_settler_process_success() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_success();
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(2);
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();

    let result = settler.process(&batch_id).await;

    assert!(result.is_ok());
    let settlement_result = result.unwrap();
    assert!(settlement_result.is_success());
}

#[tokio::test]
async fn test_settler_process_failure() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_failure("Bank declined".to_string());
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(1);
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();

    let result = settler.process(&batch_id).await;

    assert!(result.is_ok());
    let settlement_result = result.unwrap();
    assert!(!settlement_result.is_success());
    assert_eq!(settlement_result.error_message(), Some("Bank declined"));
}

#[tokio::test]
async fn test_settler_process_not_found() {
    let config = SettlerConfig::default();
    let mut settler = Settler::new(config);

    let unknown_id = BatchId::generate();
    let result = settler.process(&unknown_id).await;

    assert!(matches!(result, Err(SettlerError::BatchNotFound)));
}

#[tokio::test]
async fn test_settler_process_with_retry() {
    let config = SettlerConfig::new()
        .with_max_retries(3)
        .with_retry_delay_secs(0); // No delay for test
    let target = MockSettlementTarget::new()
        .with_failures_then_success(2); // Fail twice, then succeed
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(1);
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();

    let result = settler.process(&batch_id).await;

    assert!(result.is_ok());
    let settlement_result = result.unwrap();
    assert!(settlement_result.is_success());
    assert_eq!(settlement_result.attempts(), 3); // 2 failures + 1 success
}

#[tokio::test]
async fn test_settler_process_max_retries_exceeded() {
    let config = SettlerConfig::new()
        .with_max_retries(2)
        .with_retry_delay_secs(0);
    let target = MockSettlementTarget::new()
        .with_failure("Always fails".to_string());
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(1);
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();

    let result = settler.process(&batch_id).await;

    assert!(result.is_ok());
    let settlement_result = result.unwrap();
    assert!(!settlement_result.is_success());
    assert_eq!(settlement_result.attempts(), 3); // Initial + 2 retries
}

// ============================================================================
// SETTLEMENT RESULT
// ============================================================================

#[test]
fn test_settlement_result_success() {
    let batch_id = BatchId::generate();
    let result = SettlementResult::success(batch_id.clone(), "tx-123".to_string());

    assert!(result.is_success());
    assert_eq!(result.batch_id(), &batch_id);
    assert_eq!(result.transaction_id(), Some("tx-123"));
    assert!(result.error_message().is_none());
}

#[test]
fn test_settlement_result_failure() {
    let batch_id = BatchId::generate();
    let result = SettlementResult::failure(batch_id.clone(), "Insufficient funds".to_string());

    assert!(!result.is_success());
    assert_eq!(result.batch_id(), &batch_id);
    assert!(result.transaction_id().is_none());
    assert_eq!(result.error_message(), Some("Insufficient funds"));
}

#[test]
fn test_settlement_result_with_attempts() {
    let batch_id = BatchId::generate();
    let result = SettlementResult::success(batch_id, "tx-456".to_string())
        .with_attempts(3);

    assert_eq!(result.attempts(), 3);
}

#[test]
fn test_settlement_result_with_receipt() {
    let batch_id = BatchId::generate();
    let receipt = SettlementReceipt::new("tx-789", 1000);

    let result = SettlementResult::success(batch_id, "tx-789".to_string())
        .with_receipt(receipt.clone());

    assert!(result.receipt().is_some());
    assert_eq!(result.receipt().unwrap().transaction_id(), "tx-789");
}

// ============================================================================
// SETTLEMENT RECEIPT
// ============================================================================

#[test]
fn test_settlement_receipt_creation() {
    let receipt = SettlementReceipt::new("tx-abc", 5000);

    assert_eq!(receipt.transaction_id(), "tx-abc");
    assert_eq!(receipt.amount(), 5000);
    assert!(receipt.timestamp() > 0);
}

#[test]
fn test_settlement_receipt_with_metadata() {
    let receipt = SettlementReceipt::new("tx-def", 1000)
        .with_metadata("bank_ref", "REF123")
        .with_metadata("currency", "USD");

    assert_eq!(receipt.get_metadata("bank_ref"), Some(&"REF123".to_string()));
    assert_eq!(receipt.get_metadata("currency"), Some(&"USD".to_string()));
    assert_eq!(receipt.get_metadata("missing"), None);
}

#[test]
fn test_settlement_receipt_serialization() {
    let receipt = SettlementReceipt::new("tx-ghi", 2500);

    let bytes = receipt.to_bytes();
    assert!(!bytes.is_empty());

    let restored = SettlementReceipt::from_bytes(&bytes).unwrap();
    assert_eq!(receipt.transaction_id(), restored.transaction_id());
    assert_eq!(receipt.amount(), restored.amount());
}

// ============================================================================
// SETTLEMENT TARGET TRAIT
// ============================================================================

#[tokio::test]
async fn test_mock_target_success() {
    let target = MockSettlementTarget::new().with_success();
    let batch = create_test_batch(1);

    let result = target.settle(&batch).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_target_failure() {
    let target = MockSettlementTarget::new()
        .with_failure("Network error".to_string());
    let batch = create_test_batch(1);

    let result = target.settle(&batch).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Network error"));
}

#[tokio::test]
async fn test_mock_target_delay() {
    use std::time::Instant;

    let target = MockSettlementTarget::new()
        .with_success()
        .with_delay_ms(100);
    let batch = create_test_batch(1);

    let start = Instant::now();
    let _ = target.settle(&batch).await;
    let elapsed = start.elapsed();

    assert!(elapsed.as_millis() >= 100);
}

// ============================================================================
// SETTLER EVENTS
// ============================================================================

#[tokio::test]
async fn test_settler_events() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_success();
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(1);
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();
    settler.process(&batch_id).await.unwrap();

    let events = settler.poll_events();

    assert!(events.iter().any(|e| matches!(e, SettlerEvent::BatchSubmitted { .. })));
    assert!(events.iter().any(|e| matches!(e, SettlerEvent::SettlementComplete { .. })));
}

#[test]
fn test_settler_event_batch_submitted() {
    let batch_id = BatchId::generate();
    let event = SettlerEvent::BatchSubmitted {
        batch_id: batch_id.clone(),
        entries: 5,
        total_amount: 1000,
    };

    match event {
        SettlerEvent::BatchSubmitted { batch_id: id, entries, total_amount } => {
            assert_eq!(id, batch_id);
            assert_eq!(entries, 5);
            assert_eq!(total_amount, 1000);
        }
        _ => panic!("Wrong event type"),
    }
}

#[test]
fn test_settler_event_settlement_complete() {
    let batch_id = BatchId::generate();
    let event = SettlerEvent::SettlementComplete {
        batch_id: batch_id.clone(),
        success: true,
        transaction_id: Some("tx-complete".to_string()),
    };

    match event {
        SettlerEvent::SettlementComplete { batch_id: id, success, transaction_id } => {
            assert_eq!(id, batch_id);
            assert!(success);
            assert_eq!(transaction_id, Some("tx-complete".to_string()));
        }
        _ => panic!("Wrong event type"),
    }
}

#[test]
fn test_settler_event_settlement_failed() {
    let batch_id = BatchId::generate();
    let event = SettlerEvent::SettlementFailed {
        batch_id: batch_id.clone(),
        error: "Timeout".to_string(),
        attempts: 3,
    };

    match event {
        SettlerEvent::SettlementFailed { batch_id: id, error, attempts } => {
            assert_eq!(id, batch_id);
            assert_eq!(error, "Timeout");
            assert_eq!(attempts, 3);
        }
        _ => panic!("Wrong event type"),
    }
}

// ============================================================================
// SETTLER STATUS TRACKING
// ============================================================================

#[tokio::test]
async fn test_settler_get_status() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_success();
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(1);
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();

    let status = settler.get_status(&batch_id);
    assert!(status.is_some());
    assert!(matches!(status.unwrap(), BatchStatus::Pending));

    settler.process(&batch_id).await.unwrap();

    let status = settler.get_status(&batch_id);
    assert!(status.is_some());
    assert!(matches!(status.unwrap(), BatchStatus::Confirmed));
}

#[tokio::test]
async fn test_settler_list_by_status() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_success();
    let mut settler = Settler::with_target(config, Box::new(target));

    // Submit multiple batches
    let batch1 = create_test_batch(1);
    let batch2 = create_test_batch(2);
    let batch3 = create_test_batch(3);
    let batch1_id = batch1.id().clone();

    settler.submit(batch1).await.unwrap();
    settler.submit(batch2).await.unwrap();
    settler.submit(batch3).await.unwrap();

    let pending = settler.list_by_status(BatchStatus::Pending);
    assert_eq!(pending.len(), 3);

    // Process one
    settler.process(&batch1_id).await.unwrap();

    let pending = settler.list_by_status(BatchStatus::Pending);
    let confirmed = settler.list_by_status(BatchStatus::Confirmed);

    assert_eq!(pending.len(), 2);
    assert_eq!(confirmed.len(), 1);
}

// ============================================================================
// SETTLER STATISTICS
// ============================================================================

#[tokio::test]
async fn test_settler_stats() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_success();
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(3);
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();
    settler.process(&batch_id).await.unwrap();

    let stats = settler.stats();

    assert_eq!(stats.batches_submitted, 1);
    assert_eq!(stats.batches_settled, 1);
    assert_eq!(stats.batches_failed, 0);
    assert_eq!(stats.total_entries_settled, 3);
    assert_eq!(stats.total_amount_settled, 300);
}

#[tokio::test]
async fn test_settler_stats_with_failures() {
    let config = SettlerConfig::new()
        .with_max_retries(0);
    let target = MockSettlementTarget::new()
        .with_failure("Failed".to_string());
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(1);
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();
    settler.process(&batch_id).await.unwrap();

    let stats = settler.stats();

    assert_eq!(stats.batches_submitted, 1);
    assert_eq!(stats.batches_settled, 0);
    assert_eq!(stats.batches_failed, 1);
}

// ============================================================================
// CANCELLATION
// ============================================================================

#[tokio::test]
async fn test_settler_cancel_pending() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_success();
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(1);
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();

    let result = settler.cancel(&batch_id);

    assert!(result.is_ok());
    assert!(settler.get_status(&batch_id).is_none());
}

#[tokio::test]
async fn test_settler_cancel_not_found() {
    let config = SettlerConfig::default();
    let mut settler = Settler::new(config);

    let unknown_id = BatchId::generate();
    let result = settler.cancel(&unknown_id);

    assert!(matches!(result, Err(SettlerError::BatchNotFound)));
}

#[tokio::test]
async fn test_settler_cancel_already_processed() {
    let config = SettlerConfig::default();
    let target = MockSettlementTarget::new().with_success();
    let mut settler = Settler::with_target(config, Box::new(target));

    let batch = create_test_batch(1);
    let batch_id = batch.id().clone();

    settler.submit(batch).await.unwrap();
    settler.process(&batch_id).await.unwrap();

    let result = settler.cancel(&batch_id);

    assert!(matches!(result, Err(SettlerError::BatchAlreadyProcessed)));
}
