// Store Tests
// Tests for the sled key-value store wrapper

use p2pmesh::identity::{Did, Keypair};
use p2pmesh::iou::IOUBuilder;
use p2pmesh::ledger::{MeshState, NodeId};
use p2pmesh::storage::{MeshStore, StoreError};
use p2pmesh::vault::Vault;
use tempfile::TempDir;

// ============================================================================
// STORE CREATION AND BASIC OPERATIONS
// ============================================================================

#[test]
fn test_store_open_new() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    assert!(store.is_empty().unwrap());
}

#[test]
fn test_store_open_existing() {
    let temp_dir = TempDir::new().unwrap();

    // Create and write something
    {
        let store = MeshStore::open(temp_dir.path()).unwrap();
        store.put_raw(b"test_key", b"test_value").unwrap();
    }

    // Reopen and verify
    {
        let store = MeshStore::open(temp_dir.path()).unwrap();
        let value = store.get_raw(b"test_key").unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));
    }
}

#[test]
fn test_store_put_get_raw() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    store.put_raw(b"key1", b"value1").unwrap();
    store.put_raw(b"key2", b"value2").unwrap();

    assert_eq!(store.get_raw(b"key1").unwrap(), Some(b"value1".to_vec()));
    assert_eq!(store.get_raw(b"key2").unwrap(), Some(b"value2".to_vec()));
}

#[test]
fn test_store_get_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    assert_eq!(store.get_raw(b"nonexistent").unwrap(), None);
}

#[test]
fn test_store_delete() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    store.put_raw(b"key", b"value").unwrap();
    assert!(store.get_raw(b"key").unwrap().is_some());

    store.delete(b"key").unwrap();
    assert!(store.get_raw(b"key").unwrap().is_none());
}

#[test]
fn test_store_overwrite() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    store.put_raw(b"key", b"value1").unwrap();
    store.put_raw(b"key", b"value2").unwrap();

    assert_eq!(store.get_raw(b"key").unwrap(), Some(b"value2".to_vec()));
}

// ============================================================================
// IDENTITY PERSISTENCE
// ============================================================================

#[test]
fn test_save_load_keypair() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    let keypair = Keypair::generate();
    let original_pubkey = keypair.public_key();

    store.save_keypair(&keypair).unwrap();
    let loaded = store.load_keypair().unwrap().unwrap();

    assert_eq!(loaded.public_key().as_bytes(), original_pubkey.as_bytes());
}

#[test]
fn test_load_keypair_none_when_empty() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    assert!(store.load_keypair().unwrap().is_none());
}

#[test]
fn test_keypair_persists_across_reopens() {
    let temp_dir = TempDir::new().unwrap();
    let original_pubkey_bytes: Vec<u8>;

    {
        let store = MeshStore::open(temp_dir.path()).unwrap();
        let keypair = Keypair::generate();
        original_pubkey_bytes = keypair.public_key().as_bytes().to_vec();
        store.save_keypair(&keypair).unwrap();
    }

    {
        let store = MeshStore::open(temp_dir.path()).unwrap();
        let loaded = store.load_keypair().unwrap().unwrap();
        assert_eq!(loaded.public_key().as_bytes(), original_pubkey_bytes.as_slice());
    }
}

#[test]
fn test_save_keypair_with_label() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    let main_keypair = Keypair::generate();
    let backup_keypair = Keypair::generate();

    store.save_keypair_with_label(&main_keypair, "main").unwrap();
    store.save_keypair_with_label(&backup_keypair, "backup").unwrap();

    let loaded_main = store.load_keypair_with_label("main").unwrap().unwrap();
    let loaded_backup = store.load_keypair_with_label("backup").unwrap().unwrap();

    assert_eq!(loaded_main.public_key().as_bytes(), main_keypair.public_key().as_bytes());
    assert_eq!(loaded_backup.public_key().as_bytes(), backup_keypair.public_key().as_bytes());
}

// ============================================================================
// VAULT PERSISTENCE
// ============================================================================

fn create_funded_vault() -> (Vault, Keypair, Keypair) {
    let alice = Keypair::generate();
    let funder = Keypair::generate();

    let mut vault = Vault::new(alice.public_key());

    // Fund the vault with some IOUs
    for i in 0..3 {
        let iou = IOUBuilder::new()
            .sender(&funder)
            .recipient(Did::from_public_key(&alice.public_key()))
            .amount(100 * (i + 1))
            .nonce(i)
            .build()
            .unwrap();

        vault.receive_iou(iou, &funder.public_key()).unwrap();
    }

    (vault, alice, funder)
}

#[test]
fn test_save_load_vault() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    let (vault, _, _) = create_funded_vault();
    let original_balance = vault.balance();
    let original_utxo_count = vault.utxo_set().len();

    store.save_vault(&vault).unwrap();
    let loaded = store.load_vault().unwrap().unwrap();

    assert_eq!(loaded.balance(), original_balance);
    assert_eq!(loaded.utxo_set().len(), original_utxo_count);
}

#[test]
fn test_load_vault_none_when_empty() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    assert!(store.load_vault().unwrap().is_none());
}

#[test]
fn test_vault_persists_across_reopens() {
    let temp_dir = TempDir::new().unwrap();
    let original_balance: u64;
    let original_utxo_count: usize;

    {
        let store = MeshStore::open(temp_dir.path()).unwrap();
        let (vault, _, _) = create_funded_vault();
        original_balance = vault.balance();
        original_utxo_count = vault.utxo_set().len();
        store.save_vault(&vault).unwrap();
    }

    {
        let store = MeshStore::open(temp_dir.path()).unwrap();
        let loaded = store.load_vault().unwrap().unwrap();
        assert_eq!(loaded.balance(), original_balance);
        assert_eq!(loaded.utxo_set().len(), original_utxo_count);
    }
}

#[test]
fn test_vault_update_persists() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    let (mut vault, alice, funder) = create_funded_vault();

    // Save initial state
    store.save_vault(&vault).unwrap();
    let initial_balance = vault.balance();

    // Receive more funds
    let new_iou = IOUBuilder::new()
        .sender(&funder)
        .recipient(Did::from_public_key(&alice.public_key()))
        .amount(500)
        .nonce(999)
        .build()
        .unwrap();

    vault.receive_iou(new_iou, &funder.public_key()).unwrap();

    // Save updated state
    store.save_vault(&vault).unwrap();

    // Load and verify
    let loaded = store.load_vault().unwrap().unwrap();
    assert_eq!(loaded.balance(), initial_balance + 500);
}

// ============================================================================
// LEDGER STATE PERSISTENCE
// ============================================================================

fn create_mesh_state_with_ious() -> (MeshState, Keypair, Keypair) {
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    let node_id = NodeId::generate();

    let mut state = MeshState::new(node_id);

    for i in 0..5 {
        let iou = IOUBuilder::new()
            .sender(&alice)
            .recipient(Did::from_public_key(&bob.public_key()))
            .amount(100 * (i + 1))
            .nonce(i)
            .build()
            .unwrap();

        state.add_iou(iou, &alice.public_key()).unwrap();
    }

    (state, alice, bob)
}

#[test]
fn test_save_load_mesh_state() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    let (state, _, _) = create_mesh_state_with_ious();
    let original_iou_count = state.iou_count();
    let original_version = state.version();

    store.save_mesh_state(&state).unwrap();
    let loaded = store.load_mesh_state().unwrap().unwrap();

    assert_eq!(loaded.iou_count(), original_iou_count);
    assert_eq!(loaded.version(), original_version);
}

#[test]
fn test_load_mesh_state_none_when_empty() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    assert!(store.load_mesh_state().unwrap().is_none());
}

#[test]
fn test_mesh_state_persists_across_reopens() {
    let temp_dir = TempDir::new().unwrap();
    let original_iou_count: usize;

    {
        let store = MeshStore::open(temp_dir.path()).unwrap();
        let (state, _, _) = create_mesh_state_with_ious();
        original_iou_count = state.iou_count();
        store.save_mesh_state(&state).unwrap();
    }

    {
        let store = MeshStore::open(temp_dir.path()).unwrap();
        let loaded = store.load_mesh_state().unwrap().unwrap();
        assert_eq!(loaded.iou_count(), original_iou_count);
    }
}

#[test]
fn test_mesh_state_indexes_rebuilt_on_load() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    let (state, alice, bob) = create_mesh_state_with_ious();
    let alice_did = Did::from_public_key(&alice.public_key());
    let bob_did = Did::from_public_key(&bob.public_key());

    store.save_mesh_state(&state).unwrap();
    let loaded = store.load_mesh_state().unwrap().unwrap();

    // Verify indexes work after deserialization
    let alice_sent = loaded.get_ious_by_sender(&alice_did);
    let bob_received = loaded.get_ious_by_recipient(&bob_did);

    assert_eq!(alice_sent.len(), 5);
    assert_eq!(bob_received.len(), 5);
}

// ============================================================================
// NODE ID PERSISTENCE
// ============================================================================

#[test]
fn test_save_load_node_id() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    let node_id = NodeId::generate();
    let original_bytes = node_id.as_bytes().clone();

    store.save_node_id(&node_id).unwrap();
    let loaded = store.load_node_id().unwrap().unwrap();

    assert_eq!(loaded.as_bytes(), &original_bytes);
}

#[test]
fn test_get_or_create_node_id() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    // First call should create
    let node_id1 = store.get_or_create_node_id().unwrap();

    // Second call should return the same
    let node_id2 = store.get_or_create_node_id().unwrap();

    assert_eq!(node_id1.as_bytes(), node_id2.as_bytes());
}

// ============================================================================
// ATOMIC OPERATIONS
// ============================================================================

#[test]
fn test_flush_persists_immediately() {
    let temp_dir = TempDir::new().unwrap();

    {
        let store = MeshStore::open(temp_dir.path()).unwrap();
        store.put_raw(b"key", b"value").unwrap();
        store.flush().unwrap();
    }

    // Without explicit drop, verify data is persisted
    {
        let store = MeshStore::open(temp_dir.path()).unwrap();
        assert_eq!(store.get_raw(b"key").unwrap(), Some(b"value".to_vec()));
    }
}

// ============================================================================
// BATCH OPERATIONS
// ============================================================================

#[test]
fn test_save_all_state() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    let keypair = Keypair::generate();
    let (vault, _, _) = create_funded_vault();
    let (mesh_state, _, _) = create_mesh_state_with_ious();

    // Save all state atomically
    store.save_keypair(&keypair).unwrap();
    store.save_vault(&vault).unwrap();
    store.save_mesh_state(&mesh_state).unwrap();
    store.flush().unwrap();

    // Verify all loaded correctly
    assert!(store.load_keypair().unwrap().is_some());
    assert!(store.load_vault().unwrap().is_some());
    assert!(store.load_mesh_state().unwrap().is_some());
}

// ============================================================================
// KEY ENUMERATION
// ============================================================================

#[test]
fn test_list_keys_with_prefix() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    store.put_raw(b"peer:abc", b"data1").unwrap();
    store.put_raw(b"peer:def", b"data2").unwrap();
    store.put_raw(b"peer:ghi", b"data3").unwrap();
    store.put_raw(b"other:xyz", b"data4").unwrap();

    let peer_keys = store.list_keys_with_prefix(b"peer:").unwrap();

    assert_eq!(peer_keys.len(), 3);
}

#[test]
fn test_delete_with_prefix() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    store.put_raw(b"temp:1", b"data1").unwrap();
    store.put_raw(b"temp:2", b"data2").unwrap();
    store.put_raw(b"keep:1", b"data3").unwrap();

    store.delete_with_prefix(b"temp:").unwrap();

    assert!(store.get_raw(b"temp:1").unwrap().is_none());
    assert!(store.get_raw(b"temp:2").unwrap().is_none());
    assert!(store.get_raw(b"keep:1").unwrap().is_some());
}

// ============================================================================
// ERROR HANDLING
// ============================================================================

#[test]
fn test_corrupted_data_returns_error() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    // Write garbage data where a keypair should be
    store.put_raw(b"identity:keypair", b"not_valid_keypair_bytes").unwrap();

    let result = store.load_keypair();
    assert!(matches!(result, Err(StoreError::DeserializationFailed(_))));
}

// ============================================================================
// STORAGE STATS
// ============================================================================

#[test]
fn test_storage_stats() {
    let temp_dir = TempDir::new().unwrap();
    let store = MeshStore::open(temp_dir.path()).unwrap();

    let keypair = Keypair::generate();
    let (vault, _, _) = create_funded_vault();
    let (mesh_state, _, _) = create_mesh_state_with_ious();

    store.save_keypair(&keypair).unwrap();
    store.save_vault(&vault).unwrap();
    store.save_mesh_state(&mesh_state).unwrap();

    let stats = store.stats().unwrap();

    assert!(stats.key_count > 0);
    assert!(stats.disk_size_bytes > 0);
}
