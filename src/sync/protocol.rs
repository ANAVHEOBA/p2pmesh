// Protocol - Message types for sync communication
//
// Defines the wire format for all messages exchanged between nodes:
// - SyncRequest/Response: Pull-based state synchronization
// - IOUAnnouncement: Push-based IOU propagation
// - PeerAnnouncement: Peer discovery
// - Heartbeat: Keep-alive and version broadcast

use crate::identity::{Did, PublicKey};
use crate::iou::SignedIOU;
use crate::ledger::{IOUEntry, NodeId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Unique identifier for a message (for deduplication)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId([u8; 32]);

impl MessageId {
    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Types of messages in the protocol
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    SyncRequest,
    SyncResponse,
    IOUAnnouncement,
    PeerAnnouncement,
    Heartbeat,
}

/// Protocol errors
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Deserialization failed")]
    DeserializationFailed,

    #[error("Invalid message format")]
    InvalidFormat,

    #[error("Message too large")]
    MessageTooLarge,
}

/// Wrapper for all message types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    SyncRequest(SyncRequest),
    SyncResponse(SyncResponse),
    IOUAnnouncement(IOUAnnouncement),
    PeerAnnouncement(PeerAnnouncement),
    Heartbeat(Heartbeat),
}

impl Message {
    /// Get the message type
    pub fn message_type(&self) -> MessageType {
        match self {
            Message::SyncRequest(_) => MessageType::SyncRequest,
            Message::SyncResponse(_) => MessageType::SyncResponse,
            Message::IOUAnnouncement(_) => MessageType::IOUAnnouncement,
            Message::PeerAnnouncement(_) => MessageType::PeerAnnouncement,
            Message::Heartbeat(_) => MessageType::Heartbeat,
        }
    }

    /// Get a unique ID for this message (for deduplication)
    pub fn id(&self) -> MessageId {
        let mut hasher = Sha256::new();
        hasher.update(b"msg:");

        match self {
            Message::SyncRequest(r) => {
                hasher.update(b"sync_req:");
                hasher.update(r.sender.as_bytes());
                hasher.update(r.known_version.to_le_bytes());
            }
            Message::SyncResponse(r) => {
                hasher.update(b"sync_resp:");
                hasher.update(r.sender.as_bytes());
                hasher.update(r.current_version.to_le_bytes());
            }
            Message::IOUAnnouncement(a) => {
                hasher.update(b"iou_ann:");
                hasher.update(a.id().as_bytes());
            }
            Message::PeerAnnouncement(a) => {
                hasher.update(b"peer_ann:");
                hasher.update(a.node_id.as_bytes());
                hasher.update(a.timestamp.to_le_bytes());
            }
            Message::Heartbeat(h) => {
                hasher.update(b"heartbeat:");
                hasher.update(h.sender.as_bytes());
                hasher.update(h.version.to_le_bytes());
                hasher.update(h.timestamp.to_le_bytes());
            }
        }

        let result = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        MessageId(bytes)
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        postcard::from_bytes(bytes).map_err(|_| ProtocolError::DeserializationFailed)
    }
}

// ============================================================================
// SYNC REQUEST
// ============================================================================

/// Request for state synchronization
///
/// Sent to request IOUs that the sender doesn't have.
/// The receiver should respond with entries newer than known_version.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncRequest {
    /// Node ID of the requester
    sender: NodeId,
    /// The version the requester currently has
    known_version: u64,
    /// Optional filter: only want IOUs from this sender
    sender_filter: Option<Did>,
    /// Optional filter: only want IOUs to this recipient
    recipient_filter: Option<Did>,
    /// Timestamp when request was created
    timestamp: u64,
}

impl SyncRequest {
    /// Create a new sync request
    pub fn new(sender: NodeId, known_version: u64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            sender,
            known_version,
            sender_filter: None,
            recipient_filter: None,
            timestamp,
        }
    }

    /// Add a sender filter
    pub fn with_sender_filter(mut self, sender: Did) -> Self {
        self.sender_filter = Some(sender);
        self
    }

    /// Add a recipient filter
    pub fn with_recipient_filter(mut self, recipient: Did) -> Self {
        self.recipient_filter = Some(recipient);
        self
    }

    /// Get the sender node ID
    pub fn sender(&self) -> &NodeId {
        &self.sender
    }

    /// Get the known version
    pub fn known_version(&self) -> u64 {
        self.known_version
    }

    /// Get the sender filter
    pub fn sender_filter(&self) -> Option<&Did> {
        self.sender_filter.as_ref()
    }

    /// Get the recipient filter
    pub fn recipient_filter(&self) -> Option<&Did> {
        self.recipient_filter.as_ref()
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

// ============================================================================
// SYNC RESPONSE
// ============================================================================

/// Response to a sync request
///
/// Contains IOU entries that the requester is missing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncResponse {
    /// Node ID of the responder
    sender: NodeId,
    /// Current version of the responder's state
    current_version: u64,
    /// IOU entries being sent
    entries: Vec<IOUEntry>,
    /// Whether there are more entries available
    has_more: bool,
    /// Timestamp
    timestamp: u64,
}

impl SyncResponse {
    /// Create a new sync response
    pub fn new(sender: NodeId, current_version: u64, entries: Vec<IOUEntry>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            sender,
            current_version,
            entries,
            has_more: false,
            timestamp,
        }
    }

    /// Mark that there are more entries
    pub fn with_has_more(mut self, has_more: bool) -> Self {
        self.has_more = has_more;
        self
    }

    /// Get the sender node ID
    pub fn sender(&self) -> &NodeId {
        &self.sender
    }

    /// Get the current version
    pub fn current_version(&self) -> u64 {
        self.current_version
    }

    /// Get the entries
    pub fn entries(&self) -> &[IOUEntry] {
        &self.entries
    }

    /// Check if there are more entries
    pub fn has_more(&self) -> bool {
        self.has_more
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

// ============================================================================
// IOU ANNOUNCEMENT
// ============================================================================

/// Announcement of a new IOU to the network
///
/// Used for rumor spreading - nodes forward new IOUs to their peers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IOUAnnouncement {
    /// The IOU being announced
    iou: SignedIOU,
    /// Public key of the sender (for verification)
    sender_pubkey: PublicKey,
    /// How many hops this announcement has traveled
    hop_count: u8,
    /// Maximum hops before stopping propagation
    max_hops: u8,
    /// Timestamp when first announced
    timestamp: u64,
}

impl IOUAnnouncement {
    /// Create a new IOU announcement
    pub fn new(iou: SignedIOU, sender_pubkey: PublicKey) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            iou,
            sender_pubkey,
            hop_count: 0,
            max_hops: 6, // Default: 6 hops like typical gossip
            timestamp,
        }
    }

    /// Set maximum hops
    pub fn with_max_hops(mut self, max_hops: u8) -> Self {
        self.max_hops = max_hops;
        self
    }

    /// Get the IOU
    pub fn iou(&self) -> &SignedIOU {
        &self.iou
    }

    /// Get the sender's public key
    pub fn sender_pubkey(&self) -> &PublicKey {
        &self.sender_pubkey
    }

    /// Get the hop count
    pub fn hop_count(&self) -> u8 {
        self.hop_count
    }

    /// Increment hop count (when forwarding)
    pub fn increment_hop(&mut self) {
        self.hop_count = self.hop_count.saturating_add(1);
    }

    /// Check if propagation should stop
    pub fn should_stop_propagation(&self) -> bool {
        self.hop_count >= self.max_hops
    }

    /// Get timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Get a unique ID for this announcement (based on IOU ID)
    pub fn id(&self) -> MessageId {
        let mut hasher = Sha256::new();
        hasher.update(b"iou_ann:");
        hasher.update(self.iou.id().as_bytes());
        let result = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        MessageId(bytes)
    }
}

// ============================================================================
// PEER ANNOUNCEMENT
// ============================================================================

/// Announcement of a peer's presence
///
/// Used for peer discovery - nodes announce themselves and share known peers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerAnnouncement {
    /// Node ID of the announcing peer
    node_id: NodeId,
    /// Port the peer is listening on
    port: u16,
    /// Optional address (IP or hostname)
    address: Option<String>,
    /// Capabilities this peer supports
    capabilities: HashSet<String>,
    /// Timestamp
    timestamp: u64,
}

impl PeerAnnouncement {
    /// Create a new peer announcement
    pub fn new(node_id: NodeId, port: u16) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            node_id,
            port,
            address: None,
            capabilities: HashSet::new(),
            timestamp,
        }
    }

    /// Set the address
    pub fn with_address(mut self, address: String) -> Self {
        self.address = Some(address);
        self
    }

    /// Add a capability
    pub fn with_capability(mut self, capability: &str) -> Self {
        self.capabilities.insert(capability.to_string());
        self
    }

    /// Get node ID
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// Get port
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get address
    pub fn address(&self) -> Option<&String> {
        self.address.as_ref()
    }

    /// Check if peer has a capability
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }

    /// Get timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

// ============================================================================
// HEARTBEAT
// ============================================================================

/// Heartbeat message for keep-alive and version broadcasting
///
/// Sent periodically to indicate liveness and current state version.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heartbeat {
    /// Node ID of the sender
    sender: NodeId,
    /// Current state version
    version: u64,
    /// Timestamp
    timestamp: u64,
}

impl Heartbeat {
    /// Create a new heartbeat
    pub fn new(sender: NodeId, version: u64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            sender,
            version,
            timestamp,
        }
    }

    /// Get the sender node ID
    pub fn sender(&self) -> &NodeId {
        &self.sender
    }

    /// Get the version
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let node_id = NodeId::generate();
        let request = SyncRequest::new(node_id, 42);
        let msg = Message::SyncRequest(request);

        let bytes = msg.to_bytes();
        let restored = Message::from_bytes(&bytes).unwrap();

        assert_eq!(restored.message_type(), MessageType::SyncRequest);
    }

    #[test]
    fn test_message_id_uniqueness() {
        let node_id = NodeId::generate();
        let hb1 = Heartbeat::new(node_id.clone(), 1);
        let hb2 = Heartbeat::new(node_id, 2);

        let msg1 = Message::Heartbeat(hb1);
        let msg2 = Message::Heartbeat(hb2);

        // Different messages should have different IDs
        // (Note: timestamp makes them different)
        assert_ne!(msg1.id(), msg2.id());
    }
}
