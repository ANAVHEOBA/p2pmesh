// Gossip Engine - The heart of mesh synchronization
//
// Implements gossip-based state synchronization:
// - Push: Rumor spreading for new IOUs
// - Pull: Anti-entropy for state reconciliation
// - Heartbeat: Liveness and version broadcasting

use crate::identity::PublicKey;
use crate::iou::SignedIOU;
use crate::ledger::{IOUEntry, MergeResult, MeshState, NodeId};
use crate::sync::protocol::{
    Heartbeat, IOUAnnouncement, Message, MessageId, SyncRequest, SyncResponse,
};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Gossip-related errors
#[derive(Error, Debug)]
pub enum GossipError {
    #[error("Invalid IOU: {0}")]
    InvalidIOU(String),

    #[error("Message already seen")]
    AlreadySeen,

    #[error("State error: {0}")]
    StateError(String),
}

/// Configuration for the gossip engine
#[derive(Clone, Debug)]
pub struct GossipConfig {
    /// Number of peers to forward messages to
    pub fanout: usize,
    /// Maximum hops for IOU announcements
    pub max_hops: u8,
    /// Heartbeat interval in seconds
    pub heartbeat_interval_secs: u64,
    /// How long to remember seen messages (seconds)
    pub seen_ttl_secs: u64,
    /// Maximum seen messages to track
    pub max_seen_messages: usize,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            fanout: 3,
            max_hops: 6,
            heartbeat_interval_secs: 30,
            seen_ttl_secs: 300, // 5 minutes
            max_seen_messages: 10000,
        }
    }
}

impl GossipConfig {
    /// Create a new config builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set fanout
    pub fn with_fanout(mut self, fanout: usize) -> Self {
        self.fanout = fanout;
        self
    }

    /// Set max hops
    pub fn with_max_hops(mut self, max_hops: u8) -> Self {
        self.max_hops = max_hops;
        self
    }

    /// Set heartbeat interval
    pub fn with_heartbeat_interval(mut self, secs: u64) -> Self {
        self.heartbeat_interval_secs = secs;
        self
    }
}

/// Events produced by the gossip engine
#[derive(Clone, Debug)]
pub enum GossipEvent {
    /// Forward a message to peers
    Forward(Message),
    /// Request sync from a peer
    RequestSync(NodeId),
    /// New IOU was added to state
    NewIOU(SignedIOU),
    /// State was updated
    StateUpdated(MergeResult),
}

/// Statistics about the gossip engine
#[derive(Clone, Debug, Default)]
pub struct GossipStats {
    pub messages_processed: u64,
    pub messages_forwarded: u64,
    pub ious_received: u64,
    pub ious_rejected: u64,
    pub syncs_initiated: u64,
    pub syncs_completed: u64,
}

/// The gossip engine - orchestrates state synchronization
pub struct GossipEngine {
    /// Our node ID
    node_id: NodeId,
    /// The mesh state we're synchronizing
    state: MeshState,
    /// Configuration
    config: GossipConfig,
    /// Messages we've already seen (for deduplication)
    seen_messages: HashMap<MessageId, u64>, // ID -> timestamp
    /// Pending outgoing IOU announcements
    pending_announcements: Vec<IOUAnnouncement>,
    /// Statistics
    stats: GossipStats,
}

impl GossipEngine {
    /// Create a new gossip engine
    pub fn new(node_id: NodeId, state: MeshState, config: GossipConfig) -> Self {
        Self {
            node_id,
            state,
            config,
            seen_messages: HashMap::new(),
            pending_announcements: Vec::new(),
            stats: GossipStats::default(),
        }
    }

    /// Get the current state
    pub fn state(&self) -> &MeshState {
        &self.state
    }

    /// Get mutable state
    pub fn state_mut(&mut self) -> &mut MeshState {
        &mut self.state
    }

    /// Get statistics
    pub fn stats(&self) -> &GossipStats {
        &self.stats
    }

    /// Get number of pending announcements
    pub fn pending_announcements(&self) -> usize {
        self.pending_announcements.len()
    }

    /// Get number of seen messages
    pub fn seen_message_count(&self) -> usize {
        self.seen_messages.len()
    }

    // ========================================================================
    // IOU ANNOUNCEMENT
    // ========================================================================

    /// Announce a new IOU to the network
    pub fn announce_iou(&mut self, iou: SignedIOU, sender_pubkey: &PublicKey) {
        let announcement = IOUAnnouncement::new(iou, sender_pubkey.clone())
            .with_max_hops(self.config.max_hops);

        // Check if we've already announced this
        let msg_id = announcement.id();
        if self.seen_messages.contains_key(&msg_id) {
            return;
        }

        // Mark as seen
        let now = Self::now();
        self.seen_messages.insert(msg_id, now);

        // Add to pending
        self.pending_announcements.push(announcement);
    }

    /// Handle an incoming IOU announcement
    pub fn handle_iou_announcement(
        &mut self,
        announcement: IOUAnnouncement,
    ) -> Result<(), GossipError> {
        let iou = announcement.iou().clone();
        let sender_pubkey = announcement.sender_pubkey().clone();

        // Add to state (validates signature internally)
        self.state
            .add_iou(iou, &sender_pubkey)
            .map_err(|e| GossipError::InvalidIOU(e.to_string()))?;

        self.stats.ious_received += 1;
        Ok(())
    }

    // ========================================================================
    // SYNC REQUEST/RESPONSE
    // ========================================================================

    /// Handle an incoming sync request
    pub fn handle_sync_request(&self, _request: &SyncRequest) -> SyncResponse {
        // Get all entries (in a real implementation, we'd filter by version delta)
        // TODO: Use request.known_version() to send only delta
        let entries: Vec<IOUEntry> = self.state.all_entries().into_iter().cloned().collect();

        SyncResponse::new(self.node_id.clone(), self.state.version(), entries)
    }

    /// Apply a sync response to our state
    pub fn apply_sync_response(
        &mut self,
        response: SyncResponse,
    ) -> Result<MergeResult, GossipError> {
        // Create a temporary state from the entries
        let mut temp_state = MeshState::new(response.sender().clone());

        for entry in response.entries() {
            // Add each entry to temp state (re-validate)
            let iou = entry.iou().clone();
            let pubkey = entry.sender_pubkey().clone();
            let _ = temp_state.add_iou(iou, &pubkey);
        }

        // Merge into our state
        let result = self.state.merge(&temp_state);

        if result.new_entries > 0 {
            self.stats.syncs_completed += 1;
        }

        Ok(result)
    }

    /// Generate a sync request
    pub fn generate_sync_request(&self) -> SyncRequest {
        SyncRequest::new(self.node_id.clone(), self.state.version())
    }

    // ========================================================================
    // HEARTBEAT
    // ========================================================================

    /// Generate a heartbeat message
    pub fn generate_heartbeat(&self) -> Heartbeat {
        Heartbeat::new(self.node_id.clone(), self.state.version())
    }

    // ========================================================================
    // MESSAGE PROCESSING
    // ========================================================================

    /// Process an incoming message
    pub fn process_message(&mut self, msg: Message) -> Result<Vec<GossipEvent>, GossipError> {
        self.stats.messages_processed += 1;

        // Check if we've seen this message
        let msg_id = msg.id();
        let now = Self::now();

        if self.seen_messages.contains_key(&msg_id) {
            return Ok(vec![]); // Already seen, don't process
        }

        // Mark as seen
        self.seen_messages.insert(msg_id, now);

        let mut events = Vec::new();

        match msg {
            Message::IOUAnnouncement(mut announcement) => {
                // Try to add to our state
                match self.handle_iou_announcement(announcement.clone()) {
                    Ok(()) => {
                        // Forward if not at max hops
                        if !announcement.should_stop_propagation() {
                            announcement.increment_hop();
                            events.push(GossipEvent::Forward(Message::IOUAnnouncement(
                                announcement.clone(),
                            )));
                            events.push(GossipEvent::NewIOU(announcement.iou().clone()));
                            self.stats.messages_forwarded += 1;
                        }
                    }
                    Err(_) => {
                        self.stats.ious_rejected += 1;
                    }
                }
            }

            Message::Heartbeat(heartbeat) => {
                // If peer has higher version, we might want to sync
                if heartbeat.version() > self.state.version() {
                    events.push(GossipEvent::RequestSync(heartbeat.sender().clone()));
                    self.stats.syncs_initiated += 1;
                }
            }

            Message::SyncRequest(request) => {
                // Generate and return response (handled by caller)
                let response = self.handle_sync_request(&request);
                events.push(GossipEvent::Forward(Message::SyncResponse(response)));
            }

            Message::SyncResponse(response) => {
                // Apply the response
                if let Ok(result) = self.apply_sync_response(response) {
                    if result.new_entries > 0 {
                        events.push(GossipEvent::StateUpdated(result));
                    }
                }
            }

            Message::PeerAnnouncement(_) => {
                // Peer announcements are handled by the peer registry
                // Just forward
                events.push(GossipEvent::Forward(msg));
            }
        }

        Ok(events)
    }

    /// Collect outgoing messages to send
    pub fn collect_outgoing_messages(&mut self) -> Vec<Message> {
        let messages: Vec<Message> = self
            .pending_announcements
            .drain(..)
            .map(Message::IOUAnnouncement)
            .collect();

        messages
    }

    // ========================================================================
    // MAINTENANCE
    // ========================================================================

    /// Prune old seen messages
    pub fn prune_seen_messages(&mut self, max_age_secs: u64) -> usize {
        let now = Self::now();
        let cutoff = now.saturating_sub(max_age_secs * 1000);

        let before = self.seen_messages.len();
        self.seen_messages.retain(|_, timestamp| *timestamp > cutoff);
        let after = self.seen_messages.len();

        // Also enforce max count
        if self.seen_messages.len() > self.config.max_seen_messages {
            // Collect IDs to remove (oldest entries)
            let mut entries: Vec<_> = self.seen_messages.iter()
                .map(|(id, ts)| (id.clone(), *ts))
                .collect();
            entries.sort_by_key(|(_, ts)| *ts);

            let to_remove = entries.len() - self.config.max_seen_messages;
            let ids_to_remove: Vec<_> = entries.into_iter()
                .take(to_remove)
                .map(|(id, _)| id)
                .collect();

            for id in ids_to_remove {
                self.seen_messages.remove(&id);
            }
        }

        before - after
    }

    /// Get current timestamp in milliseconds
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{Did, Keypair};
    use crate::iou::IOUBuilder;

    fn create_test_iou(sender: &Keypair, recipient: &Keypair, amount: u64) -> SignedIOU {
        IOUBuilder::new()
            .sender(sender)
            .recipient(Did::from_public_key(&recipient.public_key()))
            .amount(amount)
            .build()
            .unwrap()
    }

    #[test]
    fn test_gossip_engine_basic() {
        let node_id = NodeId::generate();
        let state = MeshState::new(node_id.clone());
        let engine = GossipEngine::new(node_id, state, GossipConfig::default());

        assert_eq!(engine.pending_announcements(), 0);
        assert_eq!(engine.state().iou_count(), 0);
    }

    #[test]
    fn test_gossip_announce_and_collect() {
        let node_id = NodeId::generate();
        let state = MeshState::new(node_id.clone());
        let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

        let alice = Keypair::generate();
        let bob = Keypair::generate();
        let iou = create_test_iou(&alice, &bob, 100);

        engine.announce_iou(iou, &alice.public_key());

        let messages = engine.collect_outgoing_messages();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_gossip_process_iou_announcement() {
        let node_id = NodeId::generate();
        let state = MeshState::new(node_id.clone());
        let mut engine = GossipEngine::new(node_id, state, GossipConfig::default());

        let alice = Keypair::generate();
        let bob = Keypair::generate();
        let iou = create_test_iou(&alice, &bob, 100);

        let announcement = IOUAnnouncement::new(iou, alice.public_key());
        let msg = Message::IOUAnnouncement(announcement);

        let events = engine.process_message(msg).unwrap();

        assert_eq!(engine.state().iou_count(), 1);
        assert!(!events.is_empty());
    }
}
