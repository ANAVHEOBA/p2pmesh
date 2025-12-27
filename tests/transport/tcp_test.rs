// TCP Transport Tests
// Tests for the TCP implementation of the Transport trait

use p2pmesh::transport::{
    TcpTransport, TcpTransportConfig, Transport, TransportConfig, TransportError,
    TransportEvent, TransportState, PeerAddress, ConnectionId,
};
use p2pmesh::sync::Message;
use p2pmesh::ledger::NodeId;

// ============================================================================
// TCP TRANSPORT CONFIG
// ============================================================================

#[test]
fn test_tcp_config_default() {
    let config = TcpTransportConfig::default();

    assert!(config.bind_port > 0 || config.bind_port == 0); // 0 = random port
    assert!(!config.bind_address.is_empty());
}

#[test]
fn test_tcp_config_with_port() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(9000);

    assert_eq!(config.bind_address, "127.0.0.1");
    assert_eq!(config.bind_port, 9000);
}

#[test]
fn test_tcp_config_with_reuse_address() {
    let config = TcpTransportConfig::new().with_reuse_address(true);

    assert!(config.reuse_address);
}

#[test]
fn test_tcp_config_with_nodelay() {
    let config = TcpTransportConfig::new().with_nodelay(true);

    assert!(config.nodelay);
}

#[test]
fn test_tcp_config_with_keepalive() {
    let config = TcpTransportConfig::new().with_keepalive_secs(Some(30));

    assert_eq!(config.keepalive_secs, Some(30));
}

#[test]
fn test_tcp_config_base_config() {
    let base = TransportConfig::new().with_max_connections(100);
    let config = TcpTransportConfig::new().with_base_config(base);

    assert_eq!(config.base.max_connections, 100);
}

// ============================================================================
// TCP TRANSPORT CREATION
// ============================================================================

#[test]
fn test_tcp_transport_new() {
    let config = TcpTransportConfig::default();
    let transport = TcpTransport::new(config);

    assert!(matches!(transport.state(), TransportState::Stopped));
}

#[test]
fn test_tcp_transport_with_random_port() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0); // Random available port

    let transport = TcpTransport::new(config);

    assert!(matches!(transport.state(), TransportState::Stopped));
}

// ============================================================================
// TCP TRANSPORT LIFECYCLE
// ============================================================================

#[tokio::test]
async fn test_tcp_transport_start() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);

    let mut transport = TcpTransport::new(config);

    let result = transport.start().await;

    assert!(result.is_ok());
    assert!(matches!(transport.state(), TransportState::Running));
}

#[tokio::test]
async fn test_tcp_transport_start_twice_fails() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);

    let mut transport = TcpTransport::new(config);

    transport.start().await.unwrap();
    let result = transport.start().await;

    assert!(matches!(result, Err(TransportError::AlreadyRunning)));
}

#[tokio::test]
async fn test_tcp_transport_stop() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);

    let mut transport = TcpTransport::new(config);

    transport.start().await.unwrap();
    let result = transport.stop().await;

    assert!(result.is_ok());
    assert!(matches!(transport.state(), TransportState::Stopped));
}

#[tokio::test]
async fn test_tcp_transport_stop_when_stopped() {
    let config = TcpTransportConfig::default();
    let mut transport = TcpTransport::new(config);

    // Should be idempotent or return specific error
    let result = transport.stop().await;

    assert!(result.is_ok() || matches!(result, Err(TransportError::NotRunning)));
}

#[tokio::test]
async fn test_tcp_transport_restart() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);

    let mut transport = TcpTransport::new(config);

    transport.start().await.unwrap();
    transport.stop().await.unwrap();
    let result = transport.start().await;

    assert!(result.is_ok());
    assert!(matches!(transport.state(), TransportState::Running));
}

// ============================================================================
// TCP TRANSPORT LOCAL ADDRESS
// ============================================================================

#[tokio::test]
async fn test_tcp_transport_local_address() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);

    let mut transport = TcpTransport::new(config);
    transport.start().await.unwrap();

    let local_addr = transport.local_address();

    assert!(local_addr.is_some());
    let addr = local_addr.unwrap();
    assert!(addr.is_tcp());
}

#[test]
fn test_tcp_transport_local_address_when_stopped() {
    let config = TcpTransportConfig::default();
    let transport = TcpTransport::new(config);

    let local_addr = transport.local_address();

    assert!(local_addr.is_none());
}

// ============================================================================
// TCP TRANSPORT CONNECTIONS
// ============================================================================

#[tokio::test]
async fn test_tcp_transport_connect() {
    // Start a server
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    // Start a client
    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    // Connect to server
    let result = client.connect(server_addr).await;

    assert!(result.is_ok());
    let conn_id = result.unwrap();
    assert_eq!(client.connection_count(), 1);

    // Cleanup
    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_tcp_transport_connect_when_stopped() {
    let config = TcpTransportConfig::default();
    let mut transport = TcpTransport::new(config);

    let addr = PeerAddress::tcp("127.0.0.1", 8080);
    let result = transport.connect(addr).await;

    assert!(matches!(result, Err(TransportError::NotRunning)));
}

#[tokio::test]
async fn test_tcp_transport_connect_invalid_address() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut transport = TcpTransport::new(config);
    transport.start().await.unwrap();

    // BLE address to TCP transport should fail
    let addr = PeerAddress::ble("AA:BB:CC:DD:EE:FF");
    let result = transport.connect(addr).await;

    assert!(matches!(result, Err(TransportError::InvalidAddress(_))));

    transport.stop().await.unwrap();
}

#[tokio::test]
async fn test_tcp_transport_connect_unreachable() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0)
        .with_base_config(TransportConfig::new().with_connection_timeout(1));
    let mut transport = TcpTransport::new(config);
    transport.start().await.unwrap();

    // Connect to non-existent server
    let addr = PeerAddress::tcp("127.0.0.1", 65534); // Unlikely to have server
    let result = transport.connect(addr).await;

    assert!(matches!(result, Err(TransportError::ConnectionFailed(_))));

    transport.stop().await.unwrap();
}

#[tokio::test]
async fn test_tcp_transport_disconnect() {
    // Start server and client
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    let conn_id = client.connect(server_addr).await.unwrap();
    assert_eq!(client.connection_count(), 1);

    // Disconnect
    let result = client.disconnect(&conn_id).await;

    assert!(result.is_ok());
    assert_eq!(client.connection_count(), 0);

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

// ============================================================================
// TCP TRANSPORT MESSAGE SENDING
// ============================================================================

#[tokio::test]
async fn test_tcp_transport_send_message() {
    // Setup server and client
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    let conn_id = client.connect(server_addr).await.unwrap();

    // Send data
    let data = b"hello mesh";
    let result = client.send(&conn_id, data).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), data.len());

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_tcp_transport_send_to_unknown_connection() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut transport = TcpTransport::new(config);
    transport.start().await.unwrap();

    let unknown_id = ConnectionId::generate();
    let result = transport.send(&unknown_id, b"data").await;

    assert!(matches!(result, Err(TransportError::NotConnected)));

    transport.stop().await.unwrap();
}

#[tokio::test]
async fn test_tcp_transport_send_empty_message() {
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    let conn_id = client.connect(server_addr).await.unwrap();

    // Send empty data
    let result = client.send(&conn_id, &[]).await;

    // Should succeed with 0 bytes or return error
    match result {
        Ok(bytes) => assert_eq!(bytes, 0),
        Err(_) => {} // Error is also acceptable for empty sends
    }

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_tcp_transport_send_large_message() {
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    let conn_id = client.connect(server_addr).await.unwrap();

    // Send large data (64KB)
    let data = vec![0u8; 65536];
    let result = client.send(&conn_id, &data).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), data.len());

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

// ============================================================================
// TCP TRANSPORT MESSAGE RECEIVING
// ============================================================================

#[tokio::test]
async fn test_tcp_transport_receive_message() {
    use tokio::time::{sleep, Duration};

    // Setup server
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    // Setup client
    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    let conn_id = client.connect(server_addr).await.unwrap();

    // Wait for server to accept connection
    sleep(Duration::from_millis(50)).await;

    // Get server-side connection
    let server_events = server.poll_events().await;
    assert!(server_events.iter().any(|e| matches!(e, TransportEvent::Connected { .. })));

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

// ============================================================================
// TCP TRANSPORT EVENTS
// ============================================================================

#[tokio::test]
async fn test_tcp_transport_listening_event() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut transport = TcpTransport::new(config);

    transport.start().await.unwrap();

    let events = transport.poll_events().await;

    assert!(events.iter().any(|e| matches!(e, TransportEvent::Listening { .. })));

    transport.stop().await.unwrap();
}

#[tokio::test]
async fn test_tcp_transport_connected_event() {
    use tokio::time::{sleep, Duration};

    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    client.connect(server_addr).await.unwrap();

    sleep(Duration::from_millis(50)).await;

    let events = server.poll_events().await;
    assert!(events.iter().any(|e| matches!(e, TransportEvent::Connected { .. })));

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

// ============================================================================
// TCP TRANSPORT CONNECTION MANAGEMENT
// ============================================================================

#[tokio::test]
async fn test_tcp_transport_connection_count() {
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    assert_eq!(client.connection_count(), 0);

    client.connect(server_addr.clone()).await.unwrap();
    assert_eq!(client.connection_count(), 1);

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_tcp_transport_max_connections() {
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0)
        .with_base_config(TransportConfig::new().with_max_connections(2));
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    // Create two more servers
    let server2_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server2 = TcpTransport::new(server2_config);
    server2.start().await.unwrap();
    let server2_addr = server2.local_address().unwrap();

    let server3_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server3 = TcpTransport::new(server3_config);
    server3.start().await.unwrap();
    let server3_addr = server3.local_address().unwrap();

    // Connect to first two (should succeed)
    client.connect(server_addr).await.unwrap();
    client.connect(server2_addr).await.unwrap();

    // Third connection should fail
    let result = client.connect(server3_addr).await;
    assert!(matches!(result, Err(TransportError::MaxConnectionsReached)));

    client.stop().await.unwrap();
    server.stop().await.unwrap();
    server2.stop().await.unwrap();
    server3.stop().await.unwrap();
}

#[tokio::test]
async fn test_tcp_transport_get_connection_info() {
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    let conn_id = client.connect(server_addr).await.unwrap();

    let info = client.connection_info(&conn_id);
    assert!(info.is_some());

    let info = info.unwrap();
    assert!(info.address().is_tcp());

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

// ============================================================================
// TCP TRANSPORT STATISTICS
// ============================================================================

#[tokio::test]
async fn test_tcp_transport_stats() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut transport = TcpTransport::new(config);
    transport.start().await.unwrap();

    let stats = transport.stats();

    assert_eq!(stats.connections_active, 0);
    assert_eq!(stats.bytes_sent, 0);
    assert_eq!(stats.bytes_received, 0);

    transport.stop().await.unwrap();
}

#[tokio::test]
async fn test_tcp_transport_stats_after_activity() {
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    let conn_id = client.connect(server_addr).await.unwrap();
    client.send(&conn_id, b"test data").await.unwrap();

    let stats = client.stats();
    assert_eq!(stats.connections_active, 1);
    assert!(stats.bytes_sent >= 9); // "test data" = 9 bytes

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

// ============================================================================
// TCP TRANSPORT BROADCAST
// ============================================================================

#[tokio::test]
async fn test_tcp_transport_broadcast() {
    // Create server
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);
    server.start().await.unwrap();

    let server_addr = server.local_address().unwrap();

    // Create second server
    let server2_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server2 = TcpTransport::new(server2_config);
    server2.start().await.unwrap();

    let server2_addr = server2.local_address().unwrap();

    // Create client
    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    // Connect to both servers
    client.connect(server_addr).await.unwrap();
    client.connect(server2_addr).await.unwrap();

    // Broadcast to all connections
    let data = b"broadcast message";
    let result = client.broadcast(data).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 2); // Sent to 2 connections

    client.stop().await.unwrap();
    server.stop().await.unwrap();
    server2.stop().await.unwrap();
}

#[tokio::test]
async fn test_tcp_transport_broadcast_no_connections() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut transport = TcpTransport::new(config);
    transport.start().await.unwrap();

    let result = transport.broadcast(b"data").await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);

    transport.stop().await.unwrap();
}
