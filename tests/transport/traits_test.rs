// Transport Trait Tests
// Tests for the abstract Transport trait and related types

use p2pmesh::transport::{
    Transport, TransportConfig, TransportError, TransportEvent, TransportState,
    ConnectionId, ConnectionInfo, ConnectionState, PeerAddress,
};
use p2pmesh::sync::Message;
use p2pmesh::ledger::NodeId;

// ============================================================================
// TRANSPORT CONFIG
// ============================================================================

#[test]
fn test_transport_config_default() {
    let config = TransportConfig::default();

    assert!(config.max_connections > 0);
    assert!(config.connection_timeout_secs > 0);
    assert!(config.message_timeout_secs > 0);
}

#[test]
fn test_transport_config_custom() {
    let config = TransportConfig::new()
        .with_max_connections(50)
        .with_connection_timeout(30)
        .with_message_timeout(10)
        .with_buffer_size(8192);

    assert_eq!(config.max_connections, 50);
    assert_eq!(config.connection_timeout_secs, 30);
    assert_eq!(config.message_timeout_secs, 10);
    assert_eq!(config.buffer_size, 8192);
}

#[test]
fn test_transport_config_zero_connections_invalid() {
    let config = TransportConfig::new().with_max_connections(0);

    // Should either use a sensible minimum or fail validation
    assert!(config.max_connections >= 1 || config.validate().is_err());
}

#[test]
fn test_transport_config_validation() {
    let valid_config = TransportConfig::default();
    assert!(valid_config.validate().is_ok());
}

// ============================================================================
// PEER ADDRESS
// ============================================================================

#[test]
fn test_peer_address_tcp_creation() {
    let addr = PeerAddress::tcp("127.0.0.1", 8080);

    assert!(addr.is_tcp());
    assert!(!addr.is_ble());
    assert!(!addr.is_lora());
}

#[test]
fn test_peer_address_ble_creation() {
    let addr = PeerAddress::ble("AA:BB:CC:DD:EE:FF");

    assert!(addr.is_ble());
    assert!(!addr.is_tcp());
    assert!(!addr.is_lora());
}

#[test]
fn test_peer_address_lora_creation() {
    let addr = PeerAddress::lora(0x01, 915_000_000); // Device ID and frequency

    assert!(addr.is_lora());
    assert!(!addr.is_tcp());
    assert!(!addr.is_ble());
}

#[test]
fn test_peer_address_display() {
    let tcp_addr = PeerAddress::tcp("192.168.1.100", 9000);
    let display = format!("{}", tcp_addr);

    assert!(display.contains("192.168.1.100"));
    assert!(display.contains("9000"));
}

#[test]
fn test_peer_address_equality() {
    let addr1 = PeerAddress::tcp("127.0.0.1", 8080);
    let addr2 = PeerAddress::tcp("127.0.0.1", 8080);
    let addr3 = PeerAddress::tcp("127.0.0.1", 9000);

    assert_eq!(addr1, addr2);
    assert_ne!(addr1, addr3);
}

#[test]
fn test_peer_address_hashable() {
    use std::collections::HashSet;

    let addr1 = PeerAddress::tcp("127.0.0.1", 8080);
    let addr2 = PeerAddress::tcp("127.0.0.1", 8080);

    let mut set = HashSet::new();
    set.insert(addr1);

    assert!(set.contains(&addr2));
}

#[test]
fn test_peer_address_clone() {
    let addr = PeerAddress::tcp("10.0.0.1", 5000);
    let cloned = addr.clone();

    assert_eq!(addr, cloned);
}

// ============================================================================
// CONNECTION ID
// ============================================================================

#[test]
fn test_connection_id_generation() {
    let id1 = ConnectionId::generate();
    let id2 = ConnectionId::generate();

    assert_ne!(id1, id2);
}

#[test]
fn test_connection_id_from_bytes() {
    let bytes = [1u8; 16];
    let id = ConnectionId::from_bytes(bytes);

    assert_eq!(id.as_bytes(), &bytes);
}

#[test]
fn test_connection_id_display() {
    let id = ConnectionId::generate();
    let display = format!("{}", id);

    assert!(!display.is_empty());
}

#[test]
fn test_connection_id_hashable() {
    use std::collections::HashMap;

    let id = ConnectionId::generate();
    let mut map = HashMap::new();
    map.insert(id.clone(), "test");

    assert_eq!(map.get(&id), Some(&"test"));
}

// ============================================================================
// CONNECTION STATE
// ============================================================================

#[test]
fn test_connection_state_initial() {
    let state = ConnectionState::default();

    assert!(matches!(state, ConnectionState::Disconnected));
}

#[test]
fn test_connection_state_transitions() {
    // Valid state transitions
    assert!(ConnectionState::Disconnected.can_transition_to(&ConnectionState::Connecting));
    assert!(ConnectionState::Connecting.can_transition_to(&ConnectionState::Connected));
    assert!(ConnectionState::Connected.can_transition_to(&ConnectionState::Disconnecting));
    assert!(ConnectionState::Disconnecting.can_transition_to(&ConnectionState::Disconnected));

    // Invalid transitions
    assert!(!ConnectionState::Disconnected.can_transition_to(&ConnectionState::Connected));
    assert!(!ConnectionState::Connected.can_transition_to(&ConnectionState::Connecting));
}

#[test]
fn test_connection_state_is_active() {
    assert!(!ConnectionState::Disconnected.is_active());
    assert!(ConnectionState::Connecting.is_active());
    assert!(ConnectionState::Connected.is_active());
    assert!(ConnectionState::Disconnecting.is_active());
}

// ============================================================================
// CONNECTION INFO
// ============================================================================

#[test]
fn test_connection_info_creation() {
    let addr = PeerAddress::tcp("127.0.0.1", 8080);
    let info = ConnectionInfo::new(addr.clone());

    assert_eq!(info.address(), &addr);
    assert!(matches!(info.state(), ConnectionState::Disconnected));
}

#[test]
fn test_connection_info_with_node_id() {
    let addr = PeerAddress::tcp("127.0.0.1", 8080);
    let node_id = NodeId::generate();
    let info = ConnectionInfo::new(addr).with_node_id(node_id.clone());

    assert_eq!(info.node_id(), Some(&node_id));
}

#[test]
fn test_connection_info_timestamps() {
    let addr = PeerAddress::tcp("127.0.0.1", 8080);
    let info = ConnectionInfo::new(addr);

    assert!(info.created_at() > 0);
    assert!(info.last_activity().is_none()); // No activity yet
}

#[test]
fn test_connection_info_update_activity() {
    let addr = PeerAddress::tcp("127.0.0.1", 8080);
    let mut info = ConnectionInfo::new(addr);

    info.record_activity();

    assert!(info.last_activity().is_some());
}

#[test]
fn test_connection_info_bytes_tracking() {
    let addr = PeerAddress::tcp("127.0.0.1", 8080);
    let mut info = ConnectionInfo::new(addr);

    info.record_bytes_sent(100);
    info.record_bytes_received(200);

    assert_eq!(info.bytes_sent(), 100);
    assert_eq!(info.bytes_received(), 200);
}

#[test]
fn test_connection_info_latency() {
    let addr = PeerAddress::tcp("127.0.0.1", 8080);
    let mut info = ConnectionInfo::new(addr);

    info.record_latency_ms(50);

    assert_eq!(info.latency_ms(), Some(50));
}

// ============================================================================
// TRANSPORT EVENTS
// ============================================================================

#[test]
fn test_transport_event_connected() {
    let conn_id = ConnectionId::generate();
    let addr = PeerAddress::tcp("127.0.0.1", 8080);
    let event = TransportEvent::Connected { connection_id: conn_id.clone(), address: addr };

    match event {
        TransportEvent::Connected { connection_id, .. } => {
            assert_eq!(connection_id, conn_id);
        }
        _ => panic!("Expected Connected event"),
    }
}

#[test]
fn test_transport_event_disconnected() {
    let conn_id = ConnectionId::generate();
    let event = TransportEvent::Disconnected {
        connection_id: conn_id.clone(),
        reason: "Connection closed".to_string(),
    };

    match event {
        TransportEvent::Disconnected { connection_id, reason } => {
            assert_eq!(connection_id, conn_id);
            assert!(!reason.is_empty());
        }
        _ => panic!("Expected Disconnected event"),
    }
}

#[test]
fn test_transport_event_message_received() {
    let conn_id = ConnectionId::generate();
    let data = vec![1, 2, 3, 4];
    let event = TransportEvent::MessageReceived {
        connection_id: conn_id.clone(),
        data: data.clone(),
    };

    match event {
        TransportEvent::MessageReceived { connection_id, data: received_data } => {
            assert_eq!(connection_id, conn_id);
            assert_eq!(received_data, data);
        }
        _ => panic!("Expected MessageReceived event"),
    }
}

#[test]
fn test_transport_event_error() {
    let conn_id = ConnectionId::generate();
    let event = TransportEvent::Error {
        connection_id: Some(conn_id.clone()),
        error: TransportError::ConnectionFailed("timeout".to_string()),
    };

    match event {
        TransportEvent::Error { connection_id, error } => {
            assert_eq!(connection_id, Some(conn_id));
            assert!(matches!(error, TransportError::ConnectionFailed(_)));
        }
        _ => panic!("Expected Error event"),
    }
}

#[test]
fn test_transport_event_listening() {
    let addr = PeerAddress::tcp("0.0.0.0", 8080);
    let event = TransportEvent::Listening { address: addr.clone() };

    match event {
        TransportEvent::Listening { address } => {
            assert_eq!(address, addr);
        }
        _ => panic!("Expected Listening event"),
    }
}

// ============================================================================
// TRANSPORT ERRORS
// ============================================================================

#[test]
fn test_transport_error_connection_failed() {
    let error = TransportError::ConnectionFailed("host unreachable".to_string());

    assert!(matches!(error, TransportError::ConnectionFailed(_)));
    assert!(error.is_connection_error());
}

#[test]
fn test_transport_error_send_failed() {
    let error = TransportError::SendFailed("buffer full".to_string());

    assert!(matches!(error, TransportError::SendFailed(_)));
    assert!(error.is_send_error());
}

#[test]
fn test_transport_error_receive_failed() {
    let error = TransportError::ReceiveFailed("connection reset".to_string());

    assert!(matches!(error, TransportError::ReceiveFailed(_)));
    assert!(error.is_receive_error());
}

#[test]
fn test_transport_error_timeout() {
    let error = TransportError::Timeout;

    assert!(matches!(error, TransportError::Timeout));
    assert!(error.is_timeout());
}

#[test]
fn test_transport_error_invalid_address() {
    let error = TransportError::InvalidAddress("bad format".to_string());

    assert!(matches!(error, TransportError::InvalidAddress(_)));
}

#[test]
fn test_transport_error_max_connections() {
    let error = TransportError::MaxConnectionsReached;

    assert!(matches!(error, TransportError::MaxConnectionsReached));
}

#[test]
fn test_transport_error_not_connected() {
    let error = TransportError::NotConnected;

    assert!(matches!(error, TransportError::NotConnected));
}

#[test]
fn test_transport_error_already_connected() {
    let error = TransportError::AlreadyConnected;

    assert!(matches!(error, TransportError::AlreadyConnected));
}

#[test]
fn test_transport_error_display() {
    let error = TransportError::ConnectionFailed("timeout".to_string());
    let display = format!("{}", error);

    assert!(display.contains("timeout") || display.contains("connection"));
}

#[test]
fn test_transport_error_is_retryable() {
    assert!(TransportError::Timeout.is_retryable());
    assert!(TransportError::ConnectionFailed("temp".to_string()).is_retryable());
    assert!(!TransportError::InvalidAddress("bad".to_string()).is_retryable());
}

// ============================================================================
// TRANSPORT STATE
// ============================================================================

#[test]
fn test_transport_state_initial() {
    let state = TransportState::default();

    assert!(matches!(state, TransportState::Stopped));
}

#[test]
fn test_transport_state_transitions() {
    assert!(TransportState::Stopped.can_transition_to(&TransportState::Starting));
    assert!(TransportState::Starting.can_transition_to(&TransportState::Running));
    assert!(TransportState::Running.can_transition_to(&TransportState::Stopping));
    assert!(TransportState::Stopping.can_transition_to(&TransportState::Stopped));

    // Error state can come from anywhere
    assert!(TransportState::Running.can_transition_to(&TransportState::Error("fail".to_string())));
}

#[test]
fn test_transport_state_is_running() {
    assert!(!TransportState::Stopped.is_running());
    assert!(!TransportState::Starting.is_running());
    assert!(TransportState::Running.is_running());
    assert!(!TransportState::Stopping.is_running());
}

#[test]
fn test_transport_state_is_transitioning() {
    assert!(!TransportState::Stopped.is_transitioning());
    assert!(TransportState::Starting.is_transitioning());
    assert!(!TransportState::Running.is_transitioning());
    assert!(TransportState::Stopping.is_transitioning());
}

// ============================================================================
// TRANSPORT TRAIT - MOCK IMPLEMENTATION TESTS
// ============================================================================

// These tests verify the trait interface using a mock implementation
// The actual implementations (TCP, BLE, LoRa) are tested in their own files

#[cfg(test)]
mod mock_transport {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    // Mock transport for testing trait interface
    pub struct MockTransport {
        state: TransportState,
        connections: HashMap<ConnectionId, ConnectionInfo>,
        config: TransportConfig,
        events: Vec<TransportEvent>,
        sent_messages: Arc<Mutex<Vec<(ConnectionId, Vec<u8>)>>>,
    }

    impl MockTransport {
        pub fn new(config: TransportConfig) -> Self {
            Self {
                state: TransportState::Stopped,
                connections: HashMap::new(),
                config,
                events: Vec::new(),
                sent_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn sent_messages(&self) -> Vec<(ConnectionId, Vec<u8>)> {
            self.sent_messages.lock().unwrap().clone()
        }

        pub fn simulate_incoming_connection(&mut self, addr: PeerAddress) -> ConnectionId {
            let conn_id = ConnectionId::generate();
            let info = ConnectionInfo::new(addr.clone());
            self.connections.insert(conn_id.clone(), info);
            self.events.push(TransportEvent::Connected {
                connection_id: conn_id.clone(),
                address: addr,
            });
            conn_id
        }

        pub fn simulate_incoming_message(&mut self, conn_id: ConnectionId, data: Vec<u8>) {
            self.events.push(TransportEvent::MessageReceived {
                connection_id: conn_id,
                data,
            });
        }
    }

    // Simulate Transport trait methods
    impl MockTransport {
        pub fn start(&mut self) -> Result<(), TransportError> {
            if !self.state.can_transition_to(&TransportState::Starting) {
                return Err(TransportError::InvalidState);
            }
            self.state = TransportState::Running;
            Ok(())
        }

        pub fn stop(&mut self) -> Result<(), TransportError> {
            self.state = TransportState::Stopped;
            self.connections.clear();
            Ok(())
        }

        pub fn connect(&mut self, addr: PeerAddress) -> Result<ConnectionId, TransportError> {
            if !self.state.is_running() {
                return Err(TransportError::NotRunning);
            }
            if self.connections.len() >= self.config.max_connections as usize {
                return Err(TransportError::MaxConnectionsReached);
            }

            let info = ConnectionInfo::new(addr);
            let conn_id = info.id().clone();
            self.connections.insert(conn_id.clone(), info);
            Ok(conn_id)
        }

        pub fn disconnect(&mut self, conn_id: &ConnectionId) -> Result<(), TransportError> {
            self.connections.remove(conn_id)
                .map(|_| ())
                .ok_or(TransportError::NotConnected)
        }

        pub fn send(&mut self, conn_id: &ConnectionId, data: &[u8]) -> Result<usize, TransportError> {
            if !self.connections.contains_key(conn_id) {
                return Err(TransportError::NotConnected);
            }
            self.sent_messages.lock().unwrap().push((conn_id.clone(), data.to_vec()));
            Ok(data.len())
        }

        pub fn poll_events(&mut self) -> Vec<TransportEvent> {
            std::mem::take(&mut self.events)
        }

        pub fn state(&self) -> &TransportState {
            &self.state
        }

        pub fn connections(&self) -> Vec<&ConnectionInfo> {
            self.connections.values().collect()
        }

        pub fn connection_count(&self) -> usize {
            self.connections.len()
        }
    }

    #[test]
    fn test_mock_transport_start_stop() {
        let config = TransportConfig::default();
        let mut transport = MockTransport::new(config);

        assert!(matches!(transport.state(), TransportState::Stopped));

        transport.start().unwrap();
        assert!(matches!(transport.state(), TransportState::Running));

        transport.stop().unwrap();
        assert!(matches!(transport.state(), TransportState::Stopped));
    }

    #[test]
    fn test_mock_transport_connect() {
        let config = TransportConfig::default();
        let mut transport = MockTransport::new(config);

        transport.start().unwrap();

        let addr = PeerAddress::tcp("127.0.0.1", 8080);
        let conn_id = transport.connect(addr).unwrap();

        assert_eq!(transport.connection_count(), 1);
        assert!(transport.connections().iter().any(|c| c.id() == &conn_id));
    }

    #[test]
    fn test_mock_transport_connect_when_stopped() {
        let config = TransportConfig::default();
        let mut transport = MockTransport::new(config);

        let addr = PeerAddress::tcp("127.0.0.1", 8080);
        let result = transport.connect(addr);

        assert!(matches!(result, Err(TransportError::NotRunning)));
    }

    #[test]
    fn test_mock_transport_max_connections() {
        let config = TransportConfig::new().with_max_connections(2);
        let mut transport = MockTransport::new(config);

        transport.start().unwrap();

        transport.connect(PeerAddress::tcp("127.0.0.1", 8080)).unwrap();
        transport.connect(PeerAddress::tcp("127.0.0.1", 8081)).unwrap();

        let result = transport.connect(PeerAddress::tcp("127.0.0.1", 8082));
        assert!(matches!(result, Err(TransportError::MaxConnectionsReached)));
    }

    #[test]
    fn test_mock_transport_disconnect() {
        let config = TransportConfig::default();
        let mut transport = MockTransport::new(config);

        transport.start().unwrap();

        let addr = PeerAddress::tcp("127.0.0.1", 8080);
        let conn_id = transport.connect(addr).unwrap();

        assert_eq!(transport.connection_count(), 1);

        transport.disconnect(&conn_id).unwrap();

        assert_eq!(transport.connection_count(), 0);
    }

    #[test]
    fn test_mock_transport_disconnect_unknown() {
        let config = TransportConfig::default();
        let mut transport = MockTransport::new(config);

        transport.start().unwrap();

        let unknown_id = ConnectionId::generate();
        let result = transport.disconnect(&unknown_id);

        assert!(matches!(result, Err(TransportError::NotConnected)));
    }

    #[test]
    fn test_mock_transport_send() {
        let config = TransportConfig::default();
        let mut transport = MockTransport::new(config);

        transport.start().unwrap();

        let addr = PeerAddress::tcp("127.0.0.1", 8080);
        let conn_id = transport.connect(addr).unwrap();

        let data = b"hello world";
        let bytes_sent = transport.send(&conn_id, data).unwrap();

        assert_eq!(bytes_sent, data.len());
        assert_eq!(transport.sent_messages().len(), 1);
    }

    #[test]
    fn test_mock_transport_send_to_unknown() {
        let config = TransportConfig::default();
        let mut transport = MockTransport::new(config);

        transport.start().unwrap();

        let unknown_id = ConnectionId::generate();
        let result = transport.send(&unknown_id, b"data");

        assert!(matches!(result, Err(TransportError::NotConnected)));
    }

    #[test]
    fn test_mock_transport_poll_events() {
        let config = TransportConfig::default();
        let mut transport = MockTransport::new(config);

        transport.start().unwrap();

        // Simulate incoming connection and message
        let addr = PeerAddress::tcp("192.168.1.1", 9000);
        let conn_id = transport.simulate_incoming_connection(addr);
        transport.simulate_incoming_message(conn_id, vec![1, 2, 3]);

        let events = transport.poll_events();

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], TransportEvent::Connected { .. }));
        assert!(matches!(events[1], TransportEvent::MessageReceived { .. }));

        // Events should be cleared
        let events2 = transport.poll_events();
        assert!(events2.is_empty());
    }

    #[test]
    fn test_mock_transport_stop_clears_connections() {
        let config = TransportConfig::default();
        let mut transport = MockTransport::new(config);

        transport.start().unwrap();
        transport.connect(PeerAddress::tcp("127.0.0.1", 8080)).unwrap();
        transport.connect(PeerAddress::tcp("127.0.0.1", 8081)).unwrap();

        assert_eq!(transport.connection_count(), 2);

        transport.stop().unwrap();

        assert_eq!(transport.connection_count(), 0);
    }
}
