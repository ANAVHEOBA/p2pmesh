// TCP Transport Implementation
// Provides TCP/IP network transport for peer-to-peer communication

use crate::transport::{
    ConnectionId, ConnectionInfo, ConnectionState, PeerAddress,
    Transport, TransportConfig, TransportError, TransportEvent, TransportState, TransportStats,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

// ============================================================================
// TCP TRANSPORT CONFIG
// ============================================================================

/// Configuration for TCP transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpTransportConfig {
    /// Base transport configuration
    pub base: TransportConfig,
    /// Address to bind to
    pub bind_address: String,
    /// Port to bind to (0 for random)
    pub bind_port: u16,
    /// Enable SO_REUSEADDR
    pub reuse_address: bool,
    /// Enable TCP_NODELAY
    pub nodelay: bool,
    /// TCP keepalive interval in seconds
    pub keepalive_secs: Option<u32>,
}

impl Default for TcpTransportConfig {
    fn default() -> Self {
        Self {
            base: TransportConfig::default(),
            bind_address: "0.0.0.0".to_string(),
            bind_port: 0,
            reuse_address: true,
            nodelay: true,
            keepalive_secs: Some(60),
        }
    }
}

impl TcpTransportConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_config(mut self, base: TransportConfig) -> Self {
        self.base = base;
        self
    }

    pub fn with_bind_address(mut self, addr: &str) -> Self {
        self.bind_address = addr.to_string();
        self
    }

    pub fn with_bind_port(mut self, port: u16) -> Self {
        self.bind_port = port;
        self
    }

    pub fn with_reuse_address(mut self, reuse: bool) -> Self {
        self.reuse_address = reuse;
        self
    }

    pub fn with_nodelay(mut self, nodelay: bool) -> Self {
        self.nodelay = nodelay;
        self
    }

    pub fn with_keepalive_secs(mut self, secs: Option<u32>) -> Self {
        self.keepalive_secs = secs;
        self
    }
}

// ============================================================================
// INTERNAL CONNECTION STATE
// ============================================================================

struct TcpConnection {
    info: ConnectionInfo,
    writer: mpsc::Sender<Vec<u8>>,
}

// ============================================================================
// TCP TRANSPORT
// ============================================================================

/// TCP transport implementation
pub struct TcpTransport {
    config: TcpTransportConfig,
    state: TransportState,
    local_address: Option<PeerAddress>,
    connections: HashMap<ConnectionId, TcpConnection>,
    events: Vec<TransportEvent>,
    stats: TransportStats,
    listener_handle: Option<tokio::task::JoinHandle<()>>,
    incoming_rx: Option<mpsc::Receiver<IncomingConnection>>,
    event_rx: Option<mpsc::Receiver<TransportEvent>>,
    event_tx: Option<mpsc::Sender<TransportEvent>>,
}

struct IncomingConnection {
    stream: TcpStream,
    address: PeerAddress,
}

impl TcpTransport {
    pub fn new(config: TcpTransportConfig) -> Self {
        Self {
            config,
            state: TransportState::Stopped,
            local_address: None,
            connections: HashMap::new(),
            events: Vec::new(),
            stats: TransportStats::default(),
            listener_handle: None,
            incoming_rx: None,
            event_rx: None,
            event_tx: None,
        }
    }

    async fn setup_connection(&mut self, stream: TcpStream, address: PeerAddress) -> Result<ConnectionId, TransportError> {
        // Check max connections
        if self.connections.len() >= self.config.base.max_connections as usize {
            return Err(TransportError::MaxConnectionsReached);
        }

        // Configure socket
        stream.set_nodelay(self.config.nodelay).ok();

        let mut info = ConnectionInfo::new(address.clone());
        let conn_id = info.id().clone();
        info.set_state(ConnectionState::Connected);

        // Create write channel
        let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(100);

        // Split stream
        let (mut reader, mut writer) = stream.into_split();

        // Clone event sender
        let event_tx = self.event_tx.clone().unwrap();
        let conn_id_read = conn_id.clone();
        let conn_id_write = conn_id.clone();

        // Spawn reader task
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => {
                        // Connection closed
                        let _ = event_tx.send(TransportEvent::Disconnected {
                            connection_id: conn_id_read.clone(),
                            reason: "Connection closed".to_string(),
                        }).await;
                        break;
                    }
                    Ok(n) => {
                        let _ = event_tx.send(TransportEvent::MessageReceived {
                            connection_id: conn_id_read.clone(),
                            data: buf[..n].to_vec(),
                        }).await;
                    }
                    Err(e) => {
                        let _ = event_tx.send(TransportEvent::Disconnected {
                            connection_id: conn_id_read.clone(),
                            reason: e.to_string(),
                        }).await;
                        break;
                    }
                }
            }
        });

        // Spawn writer task
        tokio::spawn(async move {
            while let Some(data) = write_rx.recv().await {
                if writer.write_all(&data).await.is_err() {
                    break;
                }
            }
        });

        let connection = TcpConnection {
            info,
            writer: write_tx,
        };

        self.connections.insert(conn_id.clone(), connection);
        self.stats.connections_active = self.connections.len() as u32;
        self.stats.connections_total += 1;

        Ok(conn_id)
    }
}

impl Transport for TcpTransport {
    async fn start(&mut self) -> Result<(), TransportError> {
        if self.state.is_running() {
            return Err(TransportError::AlreadyRunning);
        }

        self.state = TransportState::Starting;

        // Create event channel
        let (event_tx, event_rx) = mpsc::channel::<TransportEvent>(1000);
        self.event_tx = Some(event_tx.clone());
        self.event_rx = Some(event_rx);

        // Create incoming connection channel
        let (incoming_tx, incoming_rx) = mpsc::channel::<IncomingConnection>(100);
        self.incoming_rx = Some(incoming_rx);

        // Bind listener
        let bind_addr = format!("{}:{}", self.config.bind_address, self.config.bind_port);
        let listener = TcpListener::bind(&bind_addr).await.map_err(|e| {
            self.state = TransportState::Error(e.to_string());
            TransportError::ConnectionFailed(e.to_string())
        })?;

        // Get actual local address
        let local_addr = listener.local_addr().map_err(|e| {
            TransportError::ConnectionFailed(e.to_string())
        })?;

        self.local_address = Some(PeerAddress::tcp(
            &local_addr.ip().to_string(),
            local_addr.port(),
        ));

        // Send listening event
        let listening_event = TransportEvent::Listening {
            address: self.local_address.clone().unwrap(),
        };
        let _ = event_tx.send(listening_event).await;

        // Spawn listener task
        let handle = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        let address = PeerAddress::tcp(&addr.ip().to_string(), addr.port());
                        let _ = incoming_tx.send(IncomingConnection { stream, address }).await;
                    }
                    Err(_) => break,
                }
            }
        });

        self.listener_handle = Some(handle);
        self.state = TransportState::Running;

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), TransportError> {
        if !self.state.is_running() && !matches!(self.state, TransportState::Stopped) {
            return Err(TransportError::NotRunning);
        }

        self.state = TransportState::Stopping;

        // Abort listener
        if let Some(handle) = self.listener_handle.take() {
            handle.abort();
        }

        // Close all connections
        self.connections.clear();
        self.stats.connections_active = 0;

        // Clean up channels
        self.event_tx = None;
        self.event_rx = None;
        self.incoming_rx = None;
        self.local_address = None;

        self.state = TransportState::Stopped;

        Ok(())
    }

    async fn connect(&mut self, address: PeerAddress) -> Result<ConnectionId, TransportError> {
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }

        // Check max connections
        if self.connections.len() >= self.config.base.max_connections as usize {
            return Err(TransportError::MaxConnectionsReached);
        }

        // Validate address type
        let (host, port) = match &address {
            PeerAddress::Tcp { host, port } => (host.clone(), *port),
            _ => return Err(TransportError::InvalidAddress("Expected TCP address".to_string())),
        };

        // Connect with timeout
        let connect_timeout = Duration::from_secs(self.config.base.connection_timeout_secs as u64);
        let addr_str = format!("{}:{}", host, port);

        let stream = timeout(connect_timeout, TcpStream::connect(&addr_str))
            .await
            .map_err(|_| TransportError::Timeout)?
            .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;

        let conn_id = self.setup_connection(stream, address.clone()).await?;

        // Emit connected event
        self.events.push(TransportEvent::Connected {
            connection_id: conn_id.clone(),
            address,
        });

        Ok(conn_id)
    }

    async fn disconnect(&mut self, connection_id: &ConnectionId) -> Result<(), TransportError> {
        if self.connections.remove(connection_id).is_none() {
            return Err(TransportError::NotConnected);
        }

        self.stats.connections_active = self.connections.len() as u32;

        self.events.push(TransportEvent::Disconnected {
            connection_id: connection_id.clone(),
            reason: "Disconnected by local".to_string(),
        });

        Ok(())
    }

    async fn send(&mut self, connection_id: &ConnectionId, data: &[u8]) -> Result<usize, TransportError> {
        let connection = self.connections.get_mut(connection_id)
            .ok_or(TransportError::NotConnected)?;

        connection.writer.send(data.to_vec()).await
            .map_err(|_| TransportError::SendFailed("Channel closed".to_string()))?;

        connection.info.record_bytes_sent(data.len() as u64);
        self.stats.bytes_sent += data.len() as u64;
        self.stats.messages_sent += 1;

        Ok(data.len())
    }

    async fn broadcast(&mut self, data: &[u8]) -> Result<u32, TransportError> {
        let mut count = 0u32;

        let conn_ids: Vec<ConnectionId> = self.connections.keys().cloned().collect();

        for conn_id in conn_ids {
            if self.send(&conn_id, data).await.is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }

    async fn poll_events(&mut self) -> Vec<TransportEvent> {
        // Collect incoming connections first
        let mut incoming_connections = Vec::new();
        if let Some(ref mut rx) = self.incoming_rx {
            while let Ok(incoming) = rx.try_recv() {
                incoming_connections.push(incoming);
            }
        }

        // Process incoming connections
        for incoming in incoming_connections {
            if let Ok(conn_id) = self.setup_connection(incoming.stream, incoming.address.clone()).await {
                self.events.push(TransportEvent::Connected {
                    connection_id: conn_id,
                    address: incoming.address,
                });
            }
        }

        // Collect events from channel
        if let Some(ref mut rx) = self.event_rx {
            while let Ok(event) = rx.try_recv() {
                // Handle disconnection events
                if let TransportEvent::Disconnected { ref connection_id, .. } = event {
                    self.connections.remove(connection_id);
                    self.stats.connections_active = self.connections.len() as u32;
                }
                if let TransportEvent::MessageReceived { ref connection_id, ref data } = event {
                    if let Some(conn) = self.connections.get_mut(connection_id) {
                        conn.info.record_bytes_received(data.len() as u64);
                    }
                    self.stats.bytes_received += data.len() as u64;
                    self.stats.messages_received += 1;
                }
                self.events.push(event);
            }
        }

        std::mem::take(&mut self.events)
    }

    fn state(&self) -> &TransportState {
        &self.state
    }

    fn local_address(&self) -> Option<PeerAddress> {
        self.local_address.clone()
    }

    fn connection_count(&self) -> usize {
        self.connections.len()
    }

    fn connection_info(&self, connection_id: &ConnectionId) -> Option<&ConnectionInfo> {
        self.connections.get(connection_id).map(|c| &c.info)
    }

    fn stats(&self) -> TransportStats {
        self.stats.clone()
    }
}
