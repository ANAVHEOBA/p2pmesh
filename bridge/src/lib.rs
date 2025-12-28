// P2PMesh UniFFI Bridge
// Wraps the core Rust library for Kotlin/Swift - Full Integration

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::{IOUBuilder, SignedIOU as CoreSignedIOU};
use p2pmesh::ledger::{MeshState, NodeId};
use p2pmesh::vault::Vault;
use p2pmesh::gateway::{
    Collector as CoreCollector, CollectorConfig, SettlerConfig,
    SettlementBatch as CoreSettlementBatch, BatchStatus,
};
use std::sync::{Arc, Mutex};

uniffi::setup_scaffolding!();

// ============================================================================
// ERROR TYPE
// ============================================================================

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum MeshError {
    #[error("Invalid key")]
    InvalidKey,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error("Invalid IOU")]
    InvalidIOU,
    #[error("Storage error")]
    StorageError,
    #[error("Sync error")]
    SyncError,
    #[error("Transport error")]
    TransportError,
    #[error("Serialization error")]
    SerializationError,
    #[error("Recipient mismatch")]
    RecipientMismatch,
    #[error("Duplicate transaction")]
    DuplicateTransaction,
}

// ============================================================================
// WALLET - Full Integration
// ============================================================================

#[derive(uniffi::Object)]
pub struct Wallet {
    keypair: Keypair,
    did: Did,
    vault: Mutex<Vault>,
    mesh_state: Mutex<MeshState>,
    pending_ious: Mutex<Vec<Arc<SignedIOU>>>,
    nonce_counter: Mutex<u64>,
}

#[uniffi::export]
impl Wallet {
    /// Get the DID string (did:mesh:xxx)
    pub fn did(&self) -> String {
        self.did.to_string()
    }

    /// Get public key as bytes
    pub fn public_key(&self) -> Vec<u8> {
        self.keypair.public_key().as_bytes().to_vec()
    }

    /// Get secret key as bytes (for backup/restore)
    pub fn secret_key(&self) -> Vec<u8> {
        self.keypair.secret_key().to_bytes().to_vec()
    }

    /// Get current balance (total UTXOs)
    pub fn balance(&self) -> u64 {
        self.vault.lock().unwrap().balance()
    }

    /// Get available balance (excluding locked UTXOs)
    pub fn available_balance(&self) -> u64 {
        self.vault.lock().unwrap().available_balance()
    }

    /// Get count of UTXOs
    pub fn utxo_count(&self) -> u64 {
        self.vault.lock().unwrap().utxo_set().len() as u64
    }

    /// Create and sign an IOU payment to a recipient
    pub fn create_payment(&self, recipient_did: String, amount: u64) -> Result<Arc<SignedIOU>, MeshError> {
        let recipient = Did::parse(&recipient_did)
            .map_err(|_| MeshError::InvalidKey)?;

        let vault = self.vault.lock().unwrap();

        // Check balance
        if vault.available_balance() < amount {
            return Err(MeshError::InsufficientBalance);
        }
        drop(vault);

        // Get next nonce
        let mut nonce_counter = self.nonce_counter.lock().unwrap();
        *nonce_counter += 1;
        let nonce = *nonce_counter;
        drop(nonce_counter);

        // Build and sign the IOU
        let signed_iou = IOUBuilder::new()
            .sender(&self.keypair)
            .recipient(recipient)
            .amount(amount)
            .nonce(nonce)
            .build()
            .map_err(|_| MeshError::InvalidIOU)?;

        Ok(Arc::new(SignedIOU { inner: signed_iou }))
    }

    /// Mark an IOU as sent (record in vault and mesh state)
    pub fn mark_sent(&self, iou: Arc<SignedIOU>) -> Result<(), MeshError> {
        let mut vault = self.vault.lock().unwrap();

        // Record the sent IOU in vault
        vault.record_sent_iou(iou.inner.clone())
            .map_err(|_| MeshError::DuplicateTransaction)?;
        drop(vault);

        // Add to mesh state
        let mut state = self.mesh_state.lock().unwrap();
        state.add_iou(iou.inner.clone(), &self.keypair.public_key())
            .map_err(|_| MeshError::DuplicateTransaction)?;

        Ok(())
    }

    /// Receive an IOU (add to pending for verification)
    pub fn receive_payment(&self, iou: Arc<SignedIOU>) -> Result<(), MeshError> {
        // Verify the IOU is for us
        let recipient_did = Did::parse(&iou.recipient())
            .map_err(|_| MeshError::InvalidKey)?;

        if recipient_did != self.did {
            return Err(MeshError::RecipientMismatch);
        }

        // Add to pending
        self.pending_ious.lock().unwrap().push(iou);
        Ok(())
    }

    /// Process a received IOU (verify signature and add to vault)
    pub fn process_payment(&self, iou: Arc<SignedIOU>) -> Result<(), MeshError> {
        // Verify the IOU is for us
        let recipient_did = Did::parse(&iou.recipient())
            .map_err(|_| MeshError::InvalidKey)?;

        if recipient_did != self.did {
            return Err(MeshError::RecipientMismatch);
        }

        // Extract sender's public key from their DID
        let sender_did = Did::parse(&iou.sender())
            .map_err(|_| MeshError::InvalidKey)?;
        let sender_pubkey = sender_did.public_key()
            .map_err(|_| MeshError::InvalidKey)?;

        // Add to vault (this verifies signature and creates UTXO)
        let mut vault = self.vault.lock().unwrap();
        vault.receive_iou(iou.inner.clone(), &sender_pubkey)
            .map_err(|e| match e {
                p2pmesh::vault::VaultError::InvalidSignature => MeshError::InvalidSignature,
                p2pmesh::vault::VaultError::RecipientMismatch => MeshError::RecipientMismatch,
                p2pmesh::vault::VaultError::DuplicateTransaction => MeshError::DuplicateTransaction,
                _ => MeshError::InvalidIOU,
            })?;
        drop(vault);

        // Add to mesh state
        let mut state = self.mesh_state.lock().unwrap();
        let _ = state.add_iou(iou.inner.clone(), &sender_pubkey);

        // Remove from pending
        let mut pending = self.pending_ious.lock().unwrap();
        pending.retain(|p| p.id() != iou.id());

        Ok(())
    }

    /// Process a payment with explicit sender public key (for when DID lookup isn't possible)
    pub fn process_payment_with_key(&self, iou: Arc<SignedIOU>, sender_pubkey: Vec<u8>) -> Result<(), MeshError> {
        // Verify the IOU is for us
        let recipient_did = Did::parse(&iou.recipient())
            .map_err(|_| MeshError::InvalidKey)?;

        if recipient_did != self.did {
            return Err(MeshError::RecipientMismatch);
        }

        // Parse sender public key
        let pubkey = p2pmesh::identity::PublicKey::from_bytes(&sender_pubkey)
            .map_err(|_| MeshError::InvalidKey)?;

        // Add to vault
        let mut vault = self.vault.lock().unwrap();
        vault.receive_iou(iou.inner.clone(), &pubkey)
            .map_err(|e| match e {
                p2pmesh::vault::VaultError::InvalidSignature => MeshError::InvalidSignature,
                p2pmesh::vault::VaultError::RecipientMismatch => MeshError::RecipientMismatch,
                p2pmesh::vault::VaultError::DuplicateTransaction => MeshError::DuplicateTransaction,
                _ => MeshError::InvalidIOU,
            })?;
        drop(vault);

        // Add to mesh state
        let mut state = self.mesh_state.lock().unwrap();
        let _ = state.add_iou(iou.inner.clone(), &pubkey);

        // Remove from pending
        let mut pending = self.pending_ious.lock().unwrap();
        pending.retain(|p| p.id() != iou.id());

        Ok(())
    }

    /// Get all pending IOUs
    pub fn pending_ious(&self) -> Vec<Arc<SignedIOU>> {
        self.pending_ious.lock().unwrap().clone()
    }

    /// Clear a specific pending IOU (e.g., rejected)
    pub fn clear_pending(&self, iou_id: String) {
        let mut pending = self.pending_ious.lock().unwrap();
        pending.retain(|p| p.id() != iou_id);
    }

    /// Get transaction history
    pub fn transaction_count(&self) -> u64 {
        self.vault.lock().unwrap().transaction_count() as u64
    }

    /// Export wallet state as bytes (for persistence)
    pub fn export_state(&self) -> Vec<u8> {
        let vault = self.vault.lock().unwrap();
        let state = self.mesh_state.lock().unwrap();
        let nonce = *self.nonce_counter.lock().unwrap();

        // Combine exports
        let vault_bytes = vault.to_bytes();
        let state_bytes = state.to_bytes();

        let mut result = Vec::new();
        // Format: [vault_len:4][vault_bytes][state_len:4][state_bytes][nonce:8]
        result.extend_from_slice(&(vault_bytes.len() as u32).to_le_bytes());
        result.extend_from_slice(&vault_bytes);
        result.extend_from_slice(&(state_bytes.len() as u32).to_le_bytes());
        result.extend_from_slice(&state_bytes);
        result.extend_from_slice(&nonce.to_le_bytes());
        result
    }

    /// Import wallet state from bytes
    pub fn import_state(&self, data: Vec<u8>) -> Result<(), MeshError> {
        if data.len() < 16 {
            return Err(MeshError::SerializationError);
        }

        let mut offset = 0;

        // Read vault
        let vault_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        offset += 4;
        if data.len() < offset + vault_len + 12 {
            return Err(MeshError::SerializationError);
        }
        let vault_bytes = &data[offset..offset + vault_len];
        offset += vault_len;

        // Read state
        let state_len = u32::from_le_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
        ]) as usize;
        offset += 4;
        if data.len() < offset + state_len + 8 {
            return Err(MeshError::SerializationError);
        }
        let state_bytes = &data[offset..offset + state_len];
        offset += state_len;

        // Read nonce
        let nonce = u64::from_le_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
        ]);

        // Apply imports
        let mut vault = self.vault.lock().unwrap();
        *vault = Vault::from_bytes(vault_bytes)
            .map_err(|_| MeshError::SerializationError)?;

        let mut state = self.mesh_state.lock().unwrap();
        *state = MeshState::from_bytes(state_bytes)
            .map_err(|_| MeshError::SerializationError)?;

        *self.nonce_counter.lock().unwrap() = nonce;

        Ok(())
    }

    /// Simulate receiving funds (for testing/initial funding)
    /// In production, funds come from receiving IOUs from other users
    pub fn simulate_receive(&self, amount: u64) -> Result<(), MeshError> {
        // Create a self-signed IOU (for testing only)
        let signed_iou = IOUBuilder::new()
            .sender(&self.keypair)
            .recipient(self.did.clone())
            .amount(amount)
            .nonce(0)
            .build()
            .map_err(|_| MeshError::InvalidIOU)?;

        // Add to vault
        let mut vault = self.vault.lock().unwrap();
        vault.receive_iou(signed_iou, &self.keypair.public_key())
            .map_err(|_| MeshError::InvalidIOU)?;
        Ok(())
    }
}

#[uniffi::export]
pub fn create_wallet() -> Result<Arc<Wallet>, MeshError> {
    let keypair = Keypair::generate();
    let did = Did::from_public_key(&keypair.public_key());
    let node_id = NodeId::from_public_key(&keypair.public_key());
    let pubkey = keypair.public_key();

    Ok(Arc::new(Wallet {
        keypair,
        did,
        vault: Mutex::new(Vault::new(pubkey)),
        mesh_state: Mutex::new(MeshState::new(node_id)),
        pending_ious: Mutex::new(Vec::new()),
        nonce_counter: Mutex::new(0),
    }))
}

#[uniffi::export]
pub fn restore_wallet(secret_key: Vec<u8>) -> Result<Arc<Wallet>, MeshError> {
    let keypair = Keypair::from_bytes(&secret_key)
        .map_err(|_| MeshError::InvalidKey)?;
    let did = Did::from_public_key(&keypair.public_key());
    let node_id = NodeId::from_public_key(&keypair.public_key());
    let pubkey = keypair.public_key();

    Ok(Arc::new(Wallet {
        keypair,
        did,
        vault: Mutex::new(Vault::new(pubkey)),
        mesh_state: Mutex::new(MeshState::new(node_id)),
        pending_ious: Mutex::new(Vec::new()),
        nonce_counter: Mutex::new(0),
    }))
}

// ============================================================================
// SIGNED IOU
// ============================================================================

#[derive(uniffi::Object)]
pub struct SignedIOU {
    inner: CoreSignedIOU,
}

#[uniffi::export]
impl SignedIOU {
    /// Get unique ID as hex string
    pub fn id(&self) -> String {
        hex::encode(self.inner.id().as_bytes())
    }

    /// Get sender DID
    pub fn sender(&self) -> String {
        self.inner.iou().sender().to_string()
    }

    /// Get recipient DID
    pub fn recipient(&self) -> String {
        self.inner.iou().recipient().to_string()
    }

    /// Get amount
    pub fn amount(&self) -> u64 {
        self.inner.iou().amount()
    }

    /// Get timestamp
    pub fn timestamp(&self) -> u64 {
        self.inner.iou().timestamp()
    }

    /// Get nonce
    pub fn nonce(&self) -> u64 {
        self.inner.iou().nonce()
    }

    /// Serialize to bytes (for transmission)
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(&self.inner).unwrap_or_default()
    }

    /// Verify the signature against sender's public key (from DID)
    pub fn verify(&self) -> Result<bool, MeshError> {
        let sender_did = Did::parse(&self.sender())
            .map_err(|_| MeshError::InvalidKey)?;
        let sender_pubkey = sender_did.public_key()
            .map_err(|_| MeshError::InvalidKey)?;
        Ok(self.inner.verify(&sender_pubkey))
    }
}

#[uniffi::export]
pub fn signed_iou_from_bytes(data: Vec<u8>) -> Result<Arc<SignedIOU>, MeshError> {
    let inner: CoreSignedIOU = postcard::from_bytes(&data)
        .map_err(|_| MeshError::SerializationError)?;
    Ok(Arc::new(SignedIOU { inner }))
}

// ============================================================================
// MESH NODE (for P2P sync)
// ============================================================================

#[derive(uniffi::Object)]
pub struct MeshNode {
    wallet: Arc<Wallet>,
    sync_count: Mutex<u64>,
    last_sync: Mutex<u64>,
}

#[derive(uniffi::Record)]
pub struct MergeResult {
    pub new_entries: u64,
    pub total_entries: u64,
}

#[derive(uniffi::Record)]
pub struct SyncStats {
    pub total_ious: u64,
    pub total_syncs: u64,
    pub last_sync_timestamp: u64,
}

#[uniffi::export]
impl MeshNode {
    #[uniffi::constructor]
    pub fn new(wallet: Arc<Wallet>) -> Arc<Self> {
        Arc::new(Self {
            wallet,
            sync_count: Mutex::new(0),
            last_sync: Mutex::new(0),
        })
    }

    /// Get the local mesh state as bytes
    pub fn get_state(&self) -> Vec<u8> {
        self.wallet.mesh_state.lock().unwrap().to_bytes()
    }

    /// Merge remote state (from another node)
    pub fn merge_state(&self, remote_state: Vec<u8>) -> Result<MergeResult, MeshError> {
        let remote = MeshState::from_bytes(&remote_state)
            .map_err(|_| MeshError::SerializationError)?;

        let mut local = self.wallet.mesh_state.lock().unwrap();
        let result = local.merge(&remote);

        // Update sync stats
        *self.sync_count.lock().unwrap() += 1;
        *self.last_sync.lock().unwrap() = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(MergeResult {
            new_entries: result.new_entries as u64,
            total_entries: local.iou_count() as u64,
        })
    }

    /// Get delta (what we have that remote doesn't)
    pub fn get_delta(&self, remote_state: Vec<u8>) -> Vec<u8> {
        let remote = match MeshState::from_bytes(&remote_state) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let local = self.wallet.mesh_state.lock().unwrap();
        let delta = local.delta(&remote);

        postcard::to_allocvec(&delta).unwrap_or_default()
    }

    /// Get sync statistics
    pub fn stats(&self) -> SyncStats {
        let state = self.wallet.mesh_state.lock().unwrap();
        let stats = state.statistics();

        SyncStats {
            total_ious: stats.total_ious as u64,
            total_syncs: *self.sync_count.lock().unwrap(),
            last_sync_timestamp: *self.last_sync.lock().unwrap(),
        }
    }

    /// Get count of IOUs in mesh
    pub fn iou_count(&self) -> u64 {
        self.wallet.mesh_state.lock().unwrap().iou_count() as u64
    }
}

// ============================================================================
// TRANSPORT
// ============================================================================

#[derive(Clone, uniffi::Record)]
pub struct PeerInfo {
    pub address: String,
    pub transport_type: String,
    pub connected: bool,
}

#[derive(uniffi::Object)]
pub struct Transport {
    peers: Mutex<Vec<PeerInfo>>,
    bind_address: String,
}

#[uniffi::export]
impl Transport {
    /// Connect to a peer
    pub fn connect(&self, address: String) -> Result<(), MeshError> {
        let mut peers = self.peers.lock().unwrap();

        // Check if already connected
        if peers.iter().any(|p| p.address == address) {
            return Ok(());
        }

        peers.push(PeerInfo {
            address,
            transport_type: "tcp".to_string(),
            connected: true,
        });
        Ok(())
    }

    /// Disconnect from a peer
    pub fn disconnect(&self, address: String) {
        let mut peers = self.peers.lock().unwrap();
        peers.retain(|p| p.address != address);
    }

    /// Send data to a peer
    pub fn send(&self, address: String, _data: Vec<u8>) -> Result<(), MeshError> {
        let peers = self.peers.lock().unwrap();
        if !peers.iter().any(|p| p.address == address && p.connected) {
            return Err(MeshError::TransportError);
        }
        // In real implementation, this would send over network
        Ok(())
    }

    /// Get list of connected peers
    pub fn connected_peers(&self) -> Vec<PeerInfo> {
        self.peers.lock().unwrap().clone()
    }

    /// Check if connected to any peers
    pub fn is_connected(&self) -> bool {
        !self.peers.lock().unwrap().is_empty()
    }

    /// Get peer count
    pub fn peer_count(&self) -> u64 {
        self.peers.lock().unwrap().len() as u64
    }

    /// Get bind address
    pub fn bind_address(&self) -> String {
        self.bind_address.clone()
    }
}

#[uniffi::export]
pub fn create_tcp_transport(bind_address: String) -> Result<Arc<Transport>, MeshError> {
    Ok(Arc::new(Transport {
        peers: Mutex::new(Vec::new()),
        bind_address,
    }))
}

// ============================================================================
// SETTLEMENT - Full Integration
// ============================================================================

#[derive(uniffi::Object)]
pub struct SettlementBatch {
    inner: CoreSettlementBatch,
}

#[uniffi::export]
impl SettlementBatch {
    /// Get batch ID as hex string
    pub fn id(&self) -> String {
        hex::encode(self.inner.id().as_bytes())
    }

    /// Get number of entries
    pub fn entry_count(&self) -> u64 {
        self.inner.entries().len() as u64
    }

    /// Get total amount
    pub fn total_amount(&self) -> u64 {
        self.inner.total_amount()
    }

    /// Get status as string
    pub fn status(&self) -> String {
        match self.inner.status() {
            BatchStatus::Pending => "pending".to_string(),
            BatchStatus::Processing => "processing".to_string(),
            BatchStatus::Submitted => "submitted".to_string(),
            BatchStatus::Confirmed => "confirmed".to_string(),
            BatchStatus::Failed => "failed".to_string(),
            BatchStatus::Cancelled => "cancelled".to_string(),
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_bytes()
    }
}

#[derive(uniffi::Object)]
pub struct Collector {
    inner: Mutex<CoreCollector>,
}

#[uniffi::export]
impl Collector {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let config = CollectorConfig::new()
            .with_min_batch_size(1)
            .with_min_iou_age_secs(0);
        Arc::new(Self {
            inner: Mutex::new(CoreCollector::new(config)),
        })
    }

    #[uniffi::constructor]
    pub fn with_config(min_batch_size: u32, max_batch_size: u32, min_amount: u64) -> Arc<Self> {
        let config = CollectorConfig::new()
            .with_min_batch_size(min_batch_size)
            .with_max_batch_size(max_batch_size)
            .with_min_amount(min_amount)
            .with_min_iou_age_secs(0);
        Arc::new(Self {
            inner: Mutex::new(CoreCollector::new(config)),
        })
    }

    /// Collect IOUs from wallet's mesh state
    pub fn collect_from_wallet(&self, wallet: Arc<Wallet>) -> Result<u64, MeshError> {
        let state = wallet.mesh_state.lock().unwrap();
        let mut collector = self.inner.lock().unwrap();

        let count = collector.collect_from_state(&state)
            .map_err(|_| MeshError::InvalidIOU)?;

        Ok(count as u64)
    }

    /// Create a settlement batch from collected IOUs
    pub fn create_batch(&self) -> Result<Arc<SettlementBatch>, MeshError> {
        let mut collector = self.inner.lock().unwrap();
        let batch = collector.create_batch()
            .map_err(|_| MeshError::InvalidIOU)?;

        Ok(Arc::new(SettlementBatch { inner: batch }))
    }

    /// Get number of pending batches
    pub fn pending_batches(&self) -> u64 {
        self.inner.lock().unwrap().pending_batches() as u64
    }

    /// Get total collected count
    pub fn total_collected(&self) -> u64 {
        self.inner.lock().unwrap().total_collected()
    }

    /// Clear all batches
    pub fn clear(&self) {
        self.inner.lock().unwrap().clear_batches();
    }
}

// ============================================================================
// SETTLER - For submitting to external systems
// ============================================================================

#[derive(Clone, uniffi::Record)]
pub struct SettlementResult {
    pub success: bool,
    pub batch_id: String,
    pub transaction_id: Option<String>,
    pub error_message: Option<String>,
    pub attempts: u32,
}

#[derive(uniffi::Object)]
pub struct Settler {
    config: SettlerConfig,
    batches: Mutex<Vec<CoreSettlementBatch>>,
    results: Mutex<Vec<SettlementResult>>,
}

#[uniffi::export]
impl Settler {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            config: SettlerConfig::default(),
            batches: Mutex::new(Vec::new()),
            results: Mutex::new(Vec::new()),
        })
    }

    #[uniffi::constructor]
    pub fn with_config(max_retries: u32, timeout_secs: u64) -> Arc<Self> {
        let config = SettlerConfig::new()
            .with_max_retries(max_retries)
            .with_timeout_secs(timeout_secs);
        Arc::new(Self {
            config,
            batches: Mutex::new(Vec::new()),
            results: Mutex::new(Vec::new()),
        })
    }

    /// Submit a batch for settlement
    pub fn submit(&self, batch: Arc<SettlementBatch>) -> Result<(), MeshError> {
        if batch.entry_count() == 0 {
            return Err(MeshError::InvalidIOU);
        }
        self.batches.lock().unwrap().push(batch.inner.clone());
        Ok(())
    }

    /// Get pending batch count
    pub fn pending_count(&self) -> u64 {
        self.batches.lock().unwrap().len() as u64
    }

    /// Get all results
    pub fn results(&self) -> Vec<SettlementResult> {
        self.results.lock().unwrap().clone()
    }

    /// Clear results
    pub fn clear_results(&self) {
        self.results.lock().unwrap().clear();
    }

    /// Simulate processing a batch (for testing)
    pub fn simulate_process(&self, batch_id: String, success: bool, tx_id: Option<String>) {
        let mut results = self.results.lock().unwrap();
        results.push(SettlementResult {
            success,
            batch_id,
            transaction_id: tx_id,
            error_message: if success { None } else { Some("Simulated failure".to_string()) },
            attempts: 1,
        });
    }
}

// ============================================================================
// FAUCET - Offline funding for hackathon demo
// ============================================================================

/// Hardcoded faucet seed - deterministic keypair for demo purposes.
/// In production, this would be a secure secret, but for hackathon it's fine.
const FAUCET_SEED: [u8; 32] = [
    0x50, 0x32, 0x50, 0x4d, 0x45, 0x53, 0x48, 0x5f, // "P2PMESH_"
    0x46, 0x41, 0x55, 0x43, 0x45, 0x54, 0x5f, 0x4b, // "FAUCET_K"
    0x45, 0x59, 0x5f, 0x53, 0x45, 0x45, 0x44, 0x5f, // "EY_SEED_"
    0x48, 0x41, 0x43, 0x4b, 0x41, 0x54, 0x48, 0x4f, // "HACKATHO"
];

/// Get the faucet's public key bytes.
/// Use this to verify IOUs from the faucet.
#[uniffi::export]
pub fn faucet_public_key() -> Vec<u8> {
    let keypair = Keypair::from_bytes(&FAUCET_SEED)
        .expect("Faucet seed is valid");
    keypair.public_key().as_bytes().to_vec()
}

/// Get the faucet's DID string.
/// This is the sender DID that appears on faucet IOUs.
#[uniffi::export]
pub fn faucet_did() -> String {
    let keypair = Keypair::from_bytes(&FAUCET_SEED)
        .expect("Faucet seed is valid");
    let did = Did::from_public_key(&keypair.public_key());
    did.to_string()
}

/// Request funds from the faucet.
/// Returns a signed IOU that can be processed by the recipient's wallet.
///
/// # Arguments
/// * `recipient_did` - The DID of the wallet requesting funds (e.g., "did:mesh:abc123...")
/// * `amount` - The amount of credits to receive
///
/// # Returns
/// A SignedIOU from the faucet to the recipient
#[uniffi::export]
pub fn request_from_faucet(recipient_did: String, amount: u64) -> Result<Arc<SignedIOU>, MeshError> {
    if amount == 0 {
        return Err(MeshError::InvalidIOU);
    }

    // Parse recipient DID
    let recipient = Did::parse(&recipient_did)
        .map_err(|_| MeshError::InvalidKey)?;

    // Create faucet keypair from seed
    let faucet_keypair = Keypair::from_bytes(&FAUCET_SEED)
        .expect("Faucet seed is valid");

    // Generate unique nonce based on recipient and current time
    let nonce = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        // Mix in recipient DID to prevent same-second collisions
        time ^ (recipient_did.len() as u64 * 31)
    };

    // Build and sign IOU from faucet to recipient
    let signed_iou = IOUBuilder::new()
        .sender(&faucet_keypair)
        .recipient(recipient)
        .amount(amount)
        .nonce(nonce)
        .build()
        .map_err(|_| MeshError::InvalidIOU)?;

    Ok(Arc::new(SignedIOU { inner: signed_iou }))
}

/// Fund a wallet directly from the faucet.
/// This is a convenience function that requests funds and processes them in one call.
///
/// # Arguments
/// * `wallet` - The wallet to fund
/// * `amount` - The amount of credits to add
#[uniffi::export]
pub fn fund_wallet_from_faucet(wallet: Arc<Wallet>, amount: u64) -> Result<(), MeshError> {
    // Get faucet IOU
    let iou = request_from_faucet(wallet.did(), amount)?;

    // Process it with the faucet's public key
    wallet.process_payment_with_key(iou, faucet_public_key())
}
