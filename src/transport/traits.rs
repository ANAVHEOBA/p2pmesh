// Transport Traits and Core Types
// Defines the abstract Transport trait and common types used across all implementations

use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};
use thiserror::Error;

// ============================================================================
// TRANSPORT CONFIG
// ============================================================================

/// Base configuration for all transport types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Maximum number of simultaneous connections
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u32,
    /// Message send/receive timeout in seconds
    pub message_timeout_secs: u32,
    /// Buffer size for read/write operations
    pub buffer_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            max_connections: 100,
            connection_timeout_secs: 30,
            message_timeout_secs: 10,
            buffer_size: 4096,
        }
    }
}

impl TransportConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }

    pub fn with_connection_timeout(mut self, secs: u32) -> Self {
        self.connection_timeout_secs = secs;
        self
    }

    pub fn with_message_timeout(mut self, secs: u32) -> Self {
        self.message_timeout_secs = secs;
        self
    }

    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), TransportError> {
        if self.max_connections == 0 {
            return Err(TransportError::InvalidConfig("max_connections cannot be 0".to_string()));
        }
        Ok(())
    }
}

// ============================================================================
// PEER ADDRESS
// ============================================================================

/// Represents a peer's network address across different transport types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerAddress {
    /// TCP/IP address
    Tcp { host: String, port: u16 },
    /// Bluetooth Low Energy address
    Ble { mac_address: String },
    /// LoRa device address
    Lora { device_id: u8, frequency: u32 },
}

impl PeerAddress {
    /// Create a TCP address
    pub fn tcp(host: &str, port: u16) -> Self {
        Self::Tcp {
            host: host.to_string(),
            port,
        }
    }

    /// Create a BLE address
    pub fn ble(mac_address: &str) -> Self {
        Self::Ble {
            mac_address: mac_address.to_uppercase(),
        }
    }

    /// Create a LoRa address
    pub fn lora(device_id: u8, frequency: u32) -> Self {
        Self::Lora { device_id, frequency }
    }

    /// Create a LoRa broadcast address
    pub fn lora_broadcast(frequency: u32) -> Self {
        Self::Lora {
            device_id: 0xFF,
            frequency,
        }
    }

    /// Check if this is a TCP address
    pub fn is_tcp(&self) -> bool {
        matches!(self, Self::Tcp { .. })
    }

    /// Check if this is a BLE address
    pub fn is_ble(&self) -> bool {
        matches!(self, Self::Ble { .. })
    }

    /// Check if this is a LoRa address
    pub fn is_lora(&self) -> bool {
        matches!(self, Self::Lora { .. })
    }

    /// Check if this is a broadcast address
    pub fn is_broadcast(&self) -> bool {
        match self {
            Self::Lora { device_id, .. } => *device_id == 0xFF,
            _ => false,
        }
    }
}

impl fmt::Display for PeerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tcp { host, port } => write!(f, "tcp://{}:{}", host, port),
            Self::Ble { mac_address } => write!(f, "ble://{}", mac_address),
            Self::Lora { device_id, frequency } => {
                write!(f, "lora://0x{:02X}@{}Hz", device_id, frequency)
            }
        }
    }
}

impl PartialEq for PeerAddress {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Tcp { host: h1, port: p1 }, Self::Tcp { host: h2, port: p2 }) => {
                h1 == h2 && p1 == p2
            }
            (Self::Ble { mac_address: m1 }, Self::Ble { mac_address: m2 }) => {
                m1.to_uppercase() == m2.to_uppercase()
            }
            (
                Self::Lora { device_id: d1, frequency: f1 },
                Self::Lora { device_id: d2, frequency: f2 },
            ) => d1 == d2 && f1 == f2,
            _ => false,
        }
    }
}

impl Eq for PeerAddress {}

impl Hash for PeerAddress {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Tcp { host, port } => {
                0u8.hash(state);
                host.hash(state);
                port.hash(state);
            }
            Self::Ble { mac_address } => {
                1u8.hash(state);
                mac_address.to_uppercase().hash(state);
            }
            Self::Lora { device_id, frequency } => {
                2u8.hash(state);
                device_id.hash(state);
                frequency.hash(state);
            }
        }
    }
}

// ============================================================================
// CONNECTION ID
// ============================================================================

/// Unique identifier for a connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionId([u8; 16]);

impl ConnectionId {
    /// Generate a new unique connection ID
    pub fn generate() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 16];
        rng.fill(&mut bytes);
        Self(bytes)
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0[..8]))
    }
}

impl PartialEq for ConnectionId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for ConnectionId {}

impl Hash for ConnectionId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

// ============================================================================
// CONNECTION STATE
// ============================================================================

/// State of a connection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self::Disconnected
    }
}

impl ConnectionState {
    /// Check if transition to another state is valid
    pub fn can_transition_to(&self, target: &ConnectionState) -> bool {
        match (self, target) {
            (Self::Disconnected, Self::Connecting) => true,
            (Self::Connecting, Self::Connected) => true,
            (Self::Connecting, Self::Disconnected) => true, // Connection failed
            (Self::Connected, Self::Disconnecting) => true,
            (Self::Connected, Self::Disconnected) => true, // Abrupt disconnect
            (Self::Disconnecting, Self::Disconnected) => true,
            _ => false,
        }
    }

    /// Check if the connection is active (not fully disconnected)
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Disconnected)
    }
}

// ============================================================================
// CONNECTION INFO
// ============================================================================

/// Information about an active connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    id: ConnectionId,
    address: PeerAddress,
    state: ConnectionState,
    node_id: Option<crate::ledger::NodeId>,
    created_at: u64,
    last_activity: Option<u64>,
    bytes_sent: u64,
    bytes_received: u64,
    latency_ms: Option<u32>,
}

impl ConnectionInfo {
    /// Create new connection info
    pub fn new(address: PeerAddress) -> Self {
        Self {
            id: ConnectionId::generate(),
            address,
            state: ConnectionState::Disconnected,
            node_id: None,
            created_at: Self::now(),
            last_activity: None,
            bytes_sent: 0,
            bytes_received: 0,
            latency_ms: None,
        }
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Get the connection ID
    pub fn id(&self) -> &ConnectionId {
        &self.id
    }

    /// Get the peer address
    pub fn address(&self) -> &PeerAddress {
        &self.address
    }

    /// Get the connection state
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }

    /// Set the connection state
    pub fn set_state(&mut self, state: ConnectionState) {
        self.state = state;
    }

    /// Get the associated node ID, if known
    pub fn node_id(&self) -> Option<&crate::ledger::NodeId> {
        self.node_id.as_ref()
    }

    /// Set the node ID for this connection
    pub fn with_node_id(mut self, node_id: crate::ledger::NodeId) -> Self {
        self.node_id = Some(node_id);
        self
    }

    /// Get when the connection was created
    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    /// Get the last activity timestamp
    pub fn last_activity(&self) -> Option<u64> {
        self.last_activity
    }

    /// Record activity on this connection
    pub fn record_activity(&mut self) {
        self.last_activity = Some(Self::now());
    }

    /// Get bytes sent
    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent
    }

    /// Record bytes sent
    pub fn record_bytes_sent(&mut self, bytes: u64) {
        self.bytes_sent = self.bytes_sent.saturating_add(bytes);
        self.record_activity();
    }

    /// Get bytes received
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received
    }

    /// Record bytes received
    pub fn record_bytes_received(&mut self, bytes: u64) {
        self.bytes_received = self.bytes_received.saturating_add(bytes);
        self.record_activity();
    }

    /// Get latency in milliseconds
    pub fn latency_ms(&self) -> Option<u32> {
        self.latency_ms
    }

    /// Record latency measurement
    pub fn record_latency_ms(&mut self, ms: u32) {
        self.latency_ms = Some(ms);
    }

    /// Export state for serialization
    pub fn export_state(&self) -> Result<Vec<u8>, TransportError> {
        postcard::to_allocvec(self)
            .map_err(|e| TransportError::SerializationError(e.to_string()))
    }

    /// Import state from serialized bytes
    pub fn import_state(bytes: &[u8]) -> Result<Self, TransportError> {
        postcard::from_bytes(bytes)
            .map_err(|e| TransportError::SerializationError(e.to_string()))
    }
}

// ============================================================================
// TRANSPORT STATE
// ============================================================================

/// State of the transport layer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error(String),
}

impl Default for TransportState {
    fn default() -> Self {
        Self::Stopped
    }
}

impl TransportState {
    /// Check if transition to another state is valid
    pub fn can_transition_to(&self, target: &TransportState) -> bool {
        match (self, target) {
            (Self::Stopped, Self::Starting) => true,
            (Self::Starting, Self::Running) => true,
            (Self::Starting, Self::Error(_)) => true,
            (Self::Running, Self::Stopping) => true,
            (Self::Running, Self::Error(_)) => true,
            (Self::Stopping, Self::Stopped) => true,
            (Self::Error(_), Self::Stopped) => true,
            (Self::Error(_), Self::Starting) => true,
            _ => false,
        }
    }

    /// Check if the transport is running
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Check if the transport is in a transitional state
    pub fn is_transitioning(&self) -> bool {
        matches!(self, Self::Starting | Self::Stopping)
    }
}

// ============================================================================
// TRANSPORT EVENTS
// ============================================================================

/// Events emitted by the transport layer
#[derive(Debug, Clone)]
pub enum TransportEvent {
    /// Transport started listening
    Listening { address: PeerAddress },

    /// New connection established
    Connected {
        connection_id: ConnectionId,
        address: PeerAddress,
    },

    /// Connection closed
    Disconnected {
        connection_id: ConnectionId,
        reason: String,
    },

    /// Message received
    MessageReceived {
        connection_id: ConnectionId,
        data: Vec<u8>,
    },

    /// Error occurred
    Error {
        connection_id: Option<ConnectionId>,
        error: TransportError,
    },

    /// BLE device discovered
    DeviceDiscovered {
        address: PeerAddress,
        rssi: Option<i8>,
        name: Option<String>,
    },

    /// LoRa packet received (with signal quality)
    LoraPacketReceived {
        data: Vec<u8>,
        rssi: i16,
        snr: f32,
        frequency: u32,
    },
}

// ============================================================================
// TRANSPORT ERRORS
// ============================================================================

/// Errors that can occur in the transport layer
#[derive(Debug, Clone, Error)]
pub enum TransportError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    #[error("Connection timeout")]
    Timeout,

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Maximum connections reached")]
    MaxConnectionsReached,

    #[error("Not connected")]
    NotConnected,

    #[error("Already connected")]
    AlreadyConnected,

    #[error("Transport not running")]
    NotRunning,

    #[error("Transport already running")]
    AlreadyRunning,

    #[error("Invalid state")]
    InvalidState,

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Hardware unavailable")]
    HardwareUnavailable,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Payload too large")]
    PayloadTooLarge,

    #[error("LoRa CRC mismatch")]
    LoraCrcMismatch,

    #[error("LoRa receive timeout")]
    LoraReceiveTimeout,

    #[error("LoRa channel busy")]
    LoraChannelBusy,

    #[error("IO error: {0}")]
    IoError(String),
}

impl TransportError {
    /// Check if this is a connection-related error
    pub fn is_connection_error(&self) -> bool {
        matches!(
            self,
            Self::ConnectionFailed(_) | Self::NotConnected | Self::AlreadyConnected
        )
    }

    /// Check if this is a send-related error
    pub fn is_send_error(&self) -> bool {
        matches!(self, Self::SendFailed(_) | Self::PayloadTooLarge)
    }

    /// Check if this is a receive-related error
    pub fn is_receive_error(&self) -> bool {
        matches!(
            self,
            Self::ReceiveFailed(_) | Self::LoraCrcMismatch | Self::LoraReceiveTimeout
        )
    }

    /// Check if this is a timeout error
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout | Self::LoraReceiveTimeout)
    }

    /// Check if the operation can be retried
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout
                | Self::ConnectionFailed(_)
                | Self::SendFailed(_)
                | Self::ReceiveFailed(_)
                | Self::LoraReceiveTimeout
                | Self::LoraChannelBusy
        )
    }
}

impl From<std::io::Error> for TransportError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e.to_string())
    }
}

// ============================================================================
// TRANSPORT STATISTICS
// ============================================================================

/// Statistics for transport operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransportStats {
    /// Number of active connections
    pub connections_active: u32,
    /// Total connections established
    pub connections_total: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Total packets sent (LoRa)
    pub packets_sent: u64,
    /// Total packets received (LoRa)
    pub packets_received: u64,
    /// Errors encountered
    pub errors: u64,
}

// ============================================================================
// TRANSPORT TRAIT
// ============================================================================

/// Abstract transport trait for network communication
#[allow(async_fn_in_trait)]
pub trait Transport {
    /// Start the transport layer
    async fn start(&mut self) -> Result<(), TransportError>;

    /// Stop the transport layer
    async fn stop(&mut self) -> Result<(), TransportError>;

    /// Connect to a peer
    async fn connect(&mut self, address: PeerAddress) -> Result<ConnectionId, TransportError>;

    /// Disconnect from a peer
    async fn disconnect(&mut self, connection_id: &ConnectionId) -> Result<(), TransportError>;

    /// Send data to a connected peer
    async fn send(&mut self, connection_id: &ConnectionId, data: &[u8]) -> Result<usize, TransportError>;

    /// Broadcast data to all connected peers
    async fn broadcast(&mut self, data: &[u8]) -> Result<u32, TransportError>;

    /// Poll for events (non-blocking)
    async fn poll_events(&mut self) -> Vec<TransportEvent>;

    /// Get the current transport state
    fn state(&self) -> &TransportState;

    /// Get the local address (if listening)
    fn local_address(&self) -> Option<PeerAddress>;

    /// Get connection count
    fn connection_count(&self) -> usize;

    /// Get information about a specific connection
    fn connection_info(&self, connection_id: &ConnectionId) -> Option<&ConnectionInfo>;

    /// Get transport statistics
    fn stats(&self) -> TransportStats;
}
