// Ledger module - THE SHARED HISTORY
// Handles distributed state, CRDT, and conflict detection

mod conflict;
mod crdt;
mod state;

pub use conflict::{
    ConflictDetector, ConflictError, ConflictResolution, ConflictType,
    DetectorMergeResult, SpendingClaim,
};
pub use crdt::{GSet, GSetError, IOUEntry, MergeResult};
pub use state::{MeshState, MeshStateError, MeshStatistics, NodeId};
