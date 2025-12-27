// Storage module - PERSISTENCE
// Handles persistent key-value storage using sled

mod store;

pub use store::{MeshStore, StoreError, StorageStats};
