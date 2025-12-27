// Transport Edge Cases and Stress Tests
// Tests for boundary conditions, error handling, and stress scenarios

use p2pmesh::transport::{
    Transport, TransportConfig, TransportError, TransportEvent, TransportState,
    ConnectionId, ConnectionInfo, PeerAddress, TcpTransport, TcpTransportConfig,
};
use p2pmesh::sync::Message;
use p2pmesh::ledger::NodeId;
use std::time::Duration;

// ============================================================================
// BOUNDARY VALUE TESTS
// ============================================================================

#[test]
fn test_max_connections_boundary() {
    let config = TransportConfig::new().with_max_connections(u32::MAX);

    assert_eq!(config.max_connections, u32::MAX);
}

#[test]
fn test_min_connections_boundary() {
    let config = TransportConfig::new().with_max_connections(1);

    assert_eq!(config.max_connections, 1);
}

#[test]
fn test_zero_timeout_handled() {
    let config = TransportConfig::new().with_connection_timeout(0);

    // Should either use default or validate
    assert!(config.connection_timeout_secs == 0 || config.validate().is_err());
}

#[test]
fn test_max_timeout_value() {
    let config = TransportConfig::new().with_connection_timeout(u32::MAX);

    assert_eq!(config.connection_timeout_secs, u32::MAX);
}

#[test]
fn test_zero_buffer_size() {
    let config = TransportConfig::new().with_buffer_size(0);

    // Should either use default or validate
    assert!(config.buffer_size == 0 || config.validate().is_err());
}

#[test]
fn test_large_buffer_size() {
    let config = TransportConfig::new().with_buffer_size(1024 * 1024 * 100); // 100MB

    assert_eq!(config.buffer_size, 1024 * 1024 * 100);
}

// ============================================================================
// CONNECTION ID EDGE CASES
// ============================================================================

#[test]
fn test_connection_id_all_zeros() {
    let id = ConnectionId::from_bytes([0u8; 16]);

    assert_eq!(id.as_bytes(), &[0u8; 16]);
}

#[test]
fn test_connection_id_all_ones() {
    let id = ConnectionId::from_bytes([255u8; 16]);

    assert_eq!(id.as_bytes(), &[255u8; 16]);
}

#[test]
fn test_connection_id_uniqueness_stress() {
    use std::collections::HashSet;

    let mut ids = HashSet::new();
    for _ in 0..10000 {
        let id = ConnectionId::generate();
        assert!(ids.insert(id), "Duplicate connection ID generated");
    }
}

#[test]
fn test_connection_id_clone_equality() {
    let id1 = ConnectionId::generate();
    let id2 = id1.clone();

    assert_eq!(id1, id2);
    assert_eq!(id1.as_bytes(), id2.as_bytes());
}

// ============================================================================
// PEER ADDRESS EDGE CASES
// ============================================================================

#[test]
fn test_tcp_address_port_zero() {
    let addr = PeerAddress::tcp("127.0.0.1", 0);

    assert!(addr.is_tcp());
}

#[test]
fn test_tcp_address_port_max() {
    let addr = PeerAddress::tcp("127.0.0.1", 65535);

    assert!(addr.is_tcp());
}

#[test]
fn test_tcp_address_ipv6() {
    let addr = PeerAddress::tcp("::1", 8080);

    assert!(addr.is_tcp());
}

#[test]
fn test_tcp_address_ipv6_full() {
    let addr = PeerAddress::tcp("2001:0db8:85a3:0000:0000:8a2e:0370:7334", 443);

    assert!(addr.is_tcp());
}

#[test]
fn test_ble_address_all_zeros() {
    let addr = PeerAddress::ble("00:00:00:00:00:00");

    assert!(addr.is_ble());
}

#[test]
fn test_ble_address_all_fs() {
    let addr = PeerAddress::ble("FF:FF:FF:FF:FF:FF");

    assert!(addr.is_ble());
}

#[test]
fn test_lora_address_device_id_zero() {
    let addr = PeerAddress::lora(0x00, 915_000_000);

    assert!(addr.is_lora());
}

#[test]
fn test_lora_address_device_id_max() {
    let addr = PeerAddress::lora(0xFF, 915_000_000);

    assert!(addr.is_lora());
}

#[test]
fn test_lora_address_frequency_boundaries() {
    // Low frequency (Sub-GHz)
    let low = PeerAddress::lora(0x01, 433_000_000);
    assert!(low.is_lora());

    // High frequency (Sub-GHz)
    let high = PeerAddress::lora(0x01, 928_000_000);
    assert!(high.is_lora());
}

// ============================================================================
// CONNECTION STATE EDGE CASES
// ============================================================================

#[test]
fn test_connection_state_error_message() {
    let state = TransportState::Error("specific error message".to_string());

    if let TransportState::Error(msg) = state {
        assert_eq!(msg, "specific error message");
    } else {
        panic!("Expected Error state");
    }
}

#[test]
fn test_connection_state_error_empty_message() {
    let state = TransportState::Error("".to_string());

    if let TransportState::Error(msg) = state {
        assert!(msg.is_empty());
    } else {
        panic!("Expected Error state");
    }
}

#[test]
fn test_connection_info_bytes_overflow() {
    let addr = PeerAddress::tcp("127.0.0.1", 8080);
    let mut info = ConnectionInfo::new(addr);

    // Add near-max bytes
    info.record_bytes_sent(u64::MAX - 100);

    // Adding more should saturate, not overflow
    info.record_bytes_sent(200);

    assert!(info.bytes_sent() >= u64::MAX - 100);
}

#[test]
fn test_connection_info_latency_updates() {
    let addr = PeerAddress::tcp("127.0.0.1", 8080);
    let mut info = ConnectionInfo::new(addr);

    info.record_latency_ms(100);
    assert_eq!(info.latency_ms(), Some(100));

    info.record_latency_ms(50);
    // Could be latest, average, or smoothed - implementation dependent
    assert!(info.latency_ms().is_some());
}

// ============================================================================
// TRANSPORT ERROR EDGE CASES
// ============================================================================

#[test]
fn test_transport_error_long_message() {
    let long_msg = "a".repeat(10000);
    let error = TransportError::ConnectionFailed(long_msg.clone());

    if let TransportError::ConnectionFailed(msg) = error {
        assert_eq!(msg.len(), 10000);
    }
}

#[test]
fn test_transport_error_unicode_message() {
    let unicode_msg = "Connection failed: 连接失败 🔌❌".to_string();
    let error = TransportError::ConnectionFailed(unicode_msg.clone());

    if let TransportError::ConnectionFailed(msg) = error {
        assert_eq!(msg, unicode_msg);
    }
}

#[test]
fn test_transport_error_empty_message() {
    let error = TransportError::ConnectionFailed("".to_string());

    if let TransportError::ConnectionFailed(msg) = error {
        assert!(msg.is_empty());
    }
}

// ============================================================================
// TRANSPORT CONFIG COMBINATIONS
// ============================================================================

#[test]
fn test_config_all_defaults() {
    let config = TransportConfig::default();

    assert!(config.max_connections > 0);
    assert!(config.connection_timeout_secs > 0);
    assert!(config.message_timeout_secs > 0);
    assert!(config.buffer_size > 0);
}

#[test]
fn test_config_builder_chain() {
    let config = TransportConfig::new()
        .with_max_connections(100)
        .with_connection_timeout(30)
        .with_message_timeout(10)
        .with_buffer_size(4096);

    assert_eq!(config.max_connections, 100);
    assert_eq!(config.connection_timeout_secs, 30);
    assert_eq!(config.message_timeout_secs, 10);
    assert_eq!(config.buffer_size, 4096);
}

#[test]
fn test_config_override_values() {
    let config = TransportConfig::new()
        .with_max_connections(10)
        .with_max_connections(20) // Override
        .with_max_connections(30); // Override again

    assert_eq!(config.max_connections, 30);
}

// ============================================================================
// MESSAGE EDGE CASES
// ============================================================================

#[test]
fn test_empty_message_data() {
    let conn_id = ConnectionId::generate();
    let event = TransportEvent::MessageReceived {
        connection_id: conn_id,
        data: vec![],
    };

    if let TransportEvent::MessageReceived { data, .. } = event {
        assert!(data.is_empty());
    }
}

#[test]
fn test_large_message_data() {
    let conn_id = ConnectionId::generate();
    let large_data = vec![0u8; 1024 * 1024]; // 1MB
    let event = TransportEvent::MessageReceived {
        connection_id: conn_id,
        data: large_data.clone(),
    };

    if let TransportEvent::MessageReceived { data, .. } = event {
        assert_eq!(data.len(), 1024 * 1024);
    }
}

#[test]
fn test_binary_message_data() {
    let conn_id = ConnectionId::generate();
    // All possible byte values
    let binary_data: Vec<u8> = (0..=255).collect();
    let event = TransportEvent::MessageReceived {
        connection_id: conn_id,
        data: binary_data.clone(),
    };

    if let TransportEvent::MessageReceived { data, .. } = event {
        assert_eq!(data, binary_data);
    }
}

// ============================================================================
// CONCURRENT OPERATIONS (STRESS TESTS)
// ============================================================================

#[tokio::test]
async fn test_many_rapid_connections() {
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0)
        .with_base_config(TransportConfig::new().with_max_connections(100));
    let mut server = TcpTransport::new(server_config);

    if server.start().await.is_err() {
        return; // Skip if can't start
    }

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0)
        .with_base_config(TransportConfig::new().with_max_connections(100));
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    // Rapidly connect and disconnect
    for _ in 0..20 {
        if let Ok(conn_id) = client.connect(server_addr.clone()).await {
            let _ = client.disconnect(&conn_id).await;
        }
    }

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_many_rapid_sends() {
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);

    if server.start().await.is_err() {
        return;
    }

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    let conn_id = client.connect(server_addr).await.unwrap();

    // Rapidly send many small messages
    for i in 0..100 {
        let data = format!("message {}", i);
        let _ = client.send(&conn_id, data.as_bytes()).await;
    }

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_simultaneous_connect_disconnect() {
    use tokio::time::sleep;

    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);

    if server.start().await.is_err() {
        return;
    }

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0)
        .with_base_config(TransportConfig::new().with_max_connections(10));
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    // Create some connections
    let mut conn_ids = Vec::new();
    for _ in 0..5 {
        if let Ok(id) = client.connect(server_addr.clone()).await {
            conn_ids.push(id);
        }
    }

    // Disconnect half while keeping others
    for (i, id) in conn_ids.iter().enumerate() {
        if i % 2 == 0 {
            let _ = client.disconnect(id).await;
        }
    }

    sleep(Duration::from_millis(10)).await;

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

// ============================================================================
// RECONNECTION SCENARIOS
// ============================================================================

#[tokio::test]
async fn test_connect_after_server_restart() {
    use tokio::time::sleep;

    // Start server
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config.clone());

    if server.start().await.is_err() {
        return;
    }

    let server_addr = server.local_address().unwrap();

    // Start client
    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    // Connect
    let conn_id = client.connect(server_addr.clone()).await.unwrap();

    // Stop server
    server.stop().await.unwrap();

    // Connection should eventually fail
    sleep(Duration::from_millis(50)).await;

    // Try to send - should fail
    let result = client.send(&conn_id, b"test").await;
    // Either NotConnected or SendFailed
    assert!(result.is_err());

    client.stop().await.unwrap();
}

// ============================================================================
// ERROR RECOVERY
// ============================================================================

#[tokio::test]
async fn test_send_after_disconnect() {
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);

    if server.start().await.is_err() {
        return;
    }

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    let conn_id = client.connect(server_addr).await.unwrap();
    client.disconnect(&conn_id).await.unwrap();

    // Send after disconnect should fail
    let result = client.send(&conn_id, b"test").await;
    assert!(matches!(result, Err(TransportError::NotConnected)));

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_double_disconnect() {
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);

    if server.start().await.is_err() {
        return;
    }

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    let conn_id = client.connect(server_addr).await.unwrap();

    // First disconnect
    client.disconnect(&conn_id).await.unwrap();

    // Second disconnect should fail gracefully
    let result = client.disconnect(&conn_id).await;
    assert!(matches!(result, Err(TransportError::NotConnected)));

    client.stop().await.unwrap();
    server.stop().await.unwrap();
}

// ============================================================================
// MEMORY AND RESOURCE TESTS
// ============================================================================

#[tokio::test]
async fn test_event_accumulation() {
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut transport = TcpTransport::new(config);

    if transport.start().await.is_err() {
        return;
    }

    // Generate events
    for _ in 0..100 {
        let _ = transport.poll_events().await;
    }

    // Should not have memory issues
    transport.stop().await.unwrap();
}

#[test]
fn test_connection_info_serializable() {
    let addr = PeerAddress::tcp("127.0.0.1", 8080);
    let info = ConnectionInfo::new(addr);

    // Export and import state
    let state = info.export_state();
    assert!(state.is_ok());

    let state = state.unwrap();
    let restored = ConnectionInfo::import_state(&state);
    assert!(restored.is_ok());
}

// ============================================================================
// TRANSPORT TYPE COMPATIBILITY
// ============================================================================

#[test]
fn test_address_type_mismatch_detection() {
    let tcp = PeerAddress::tcp("127.0.0.1", 8080);
    let ble = PeerAddress::ble("AA:BB:CC:DD:EE:FF");
    let lora = PeerAddress::lora(0x01, 915_000_000);

    // All should be detected as their correct types
    assert!(tcp.is_tcp() && !tcp.is_ble() && !tcp.is_lora());
    assert!(!ble.is_tcp() && ble.is_ble() && !ble.is_lora());
    assert!(!lora.is_tcp() && !lora.is_ble() && lora.is_lora());
}

#[test]
fn test_address_type_string_representation() {
    let tcp = PeerAddress::tcp("127.0.0.1", 8080);
    let ble = PeerAddress::ble("AA:BB:CC:DD:EE:FF");
    let lora = PeerAddress::lora(0x01, 915_000_000);

    // Each should have a unique string representation
    let tcp_str = format!("{}", tcp);
    let ble_str = format!("{}", ble);
    let lora_str = format!("{}", lora);

    assert_ne!(tcp_str, ble_str);
    assert_ne!(tcp_str, lora_str);
    assert_ne!(ble_str, lora_str);
}

// ============================================================================
// GRACEFUL SHUTDOWN
// ============================================================================

#[tokio::test]
async fn test_stop_with_pending_connections() {
    let server_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut server = TcpTransport::new(server_config);

    if server.start().await.is_err() {
        return;
    }

    let server_addr = server.local_address().unwrap();

    let client_config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut client = TcpTransport::new(client_config);
    client.start().await.unwrap();

    // Create multiple connections
    for _ in 0..5 {
        let _ = client.connect(server_addr.clone()).await;
    }

    // Stop with active connections - should close gracefully
    let result = client.stop().await;
    assert!(result.is_ok());
    assert_eq!(client.connection_count(), 0);

    server.stop().await.unwrap();
}

#[tokio::test]
async fn test_drop_cleans_up_resources() {
    {
        let config = TcpTransportConfig::new()
            .with_bind_address("127.0.0.1")
            .with_bind_port(0);
        let mut transport = TcpTransport::new(config);

        if transport.start().await.is_ok() {
            // Transport will be dropped here
        }
    }

    // Should be able to create a new transport
    let config = TcpTransportConfig::new()
        .with_bind_address("127.0.0.1")
        .with_bind_port(0);
    let mut transport = TcpTransport::new(config);

    let result = transport.start().await;
    // Should succeed because previous was cleaned up
    assert!(result.is_ok() || matches!(result, Err(TransportError::HardwareUnavailable)));

    if result.is_ok() {
        transport.stop().await.unwrap();
    }
}
