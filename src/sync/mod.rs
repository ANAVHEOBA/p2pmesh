// Sync module - HOW NODES TALK
// Handles gossip protocol, peer management, and state synchronization

mod gossip;
mod peer;
mod protocol;

pub use gossip::{GossipConfig, GossipEngine, GossipEvent, GossipStats};
pub use peer::{PeerError, PeerInfo, PeerRegistry, PeerState, PeerStats};
pub use protocol::{
    Heartbeat, IOUAnnouncement, Message, MessageId, MessageType, PeerAnnouncement,
    ProtocolError, SyncRequest, SyncResponse,
};
