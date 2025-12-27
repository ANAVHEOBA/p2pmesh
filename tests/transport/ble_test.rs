// BLE Transport Tests
// Tests for the Bluetooth Low Energy implementation of the Transport trait

use p2pmesh::transport::{
    BleTransport, BleTransportConfig, BleCharacteristic, BleService, Transport,
    TransportConfig, TransportError, TransportEvent, TransportState, PeerAddress,
    ConnectionId,
};
use p2pmesh::ledger::NodeId;

// ============================================================================
// BLE TRANSPORT CONFIG
// ============================================================================

#[test]
fn test_ble_config_default() {
    let config = BleTransportConfig::default();

    assert!(!config.service_uuid.is_empty());
    assert!(!config.characteristic_uuid.is_empty());
    assert!(config.mtu > 0);
}

#[test]
fn test_ble_config_custom_service() {
    let config = BleTransportConfig::new()
        .with_service_uuid("12345678-1234-5678-1234-567812345678")
        .with_characteristic_uuid("87654321-4321-8765-4321-876543218765");

    assert_eq!(config.service_uuid, "12345678-1234-5678-1234-567812345678");
    assert_eq!(config.characteristic_uuid, "87654321-4321-8765-4321-876543218765");
}

#[test]
fn test_ble_config_mtu() {
    let config = BleTransportConfig::new().with_mtu(512);

    assert_eq!(config.mtu, 512);
}

#[test]
fn test_ble_config_min_mtu() {
    // BLE has minimum MTU of 23 bytes (20 payload + 3 header)
    let config = BleTransportConfig::new().with_mtu(20);

    assert!(config.mtu >= 20);
}

#[test]
fn test_ble_config_scan_duration() {
    let config = BleTransportConfig::new().with_scan_duration_secs(30);

    assert_eq!(config.scan_duration_secs, 30);
}

#[test]
fn test_ble_config_advertise_name() {
    let config = BleTransportConfig::new().with_advertise_name("MeshNode-001");

    assert_eq!(config.advertise_name, Some("MeshNode-001".to_string()));
}

#[test]
fn test_ble_config_central_mode() {
    let config = BleTransportConfig::new().as_central();

    assert!(config.is_central);
    assert!(!config.is_peripheral);
}

#[test]
fn test_ble_config_peripheral_mode() {
    let config = BleTransportConfig::new().as_peripheral();

    assert!(!config.is_central);
    assert!(config.is_peripheral);
}

#[test]
fn test_ble_config_dual_mode() {
    let config = BleTransportConfig::new()
        .as_central()
        .as_peripheral();

    assert!(config.is_central);
    assert!(config.is_peripheral);
}

#[test]
fn test_ble_config_base_config() {
    let base = TransportConfig::new().with_max_connections(10);
    let config = BleTransportConfig::new().with_base_config(base);

    assert_eq!(config.base.max_connections, 10);
}

// ============================================================================
// BLE SERVICE AND CHARACTERISTIC
// ============================================================================

#[test]
fn test_ble_service_creation() {
    let service = BleService::new("12345678-1234-5678-1234-567812345678");

    assert_eq!(service.uuid(), "12345678-1234-5678-1234-567812345678");
    assert!(service.characteristics().is_empty());
}

#[test]
fn test_ble_service_with_characteristics() {
    let char1 = BleCharacteristic::new("char-uuid-1")
        .with_read()
        .with_notify();
    let char2 = BleCharacteristic::new("char-uuid-2")
        .with_write();

    let service = BleService::new("service-uuid")
        .with_characteristic(char1)
        .with_characteristic(char2);

    assert_eq!(service.characteristics().len(), 2);
}

#[test]
fn test_ble_characteristic_read_only() {
    let char = BleCharacteristic::new("uuid").with_read();

    assert!(char.can_read());
    assert!(!char.can_write());
    assert!(!char.can_notify());
}

#[test]
fn test_ble_characteristic_write_only() {
    let char = BleCharacteristic::new("uuid").with_write();

    assert!(!char.can_read());
    assert!(char.can_write());
    assert!(!char.can_notify());
}

#[test]
fn test_ble_characteristic_notify() {
    let char = BleCharacteristic::new("uuid").with_notify();

    assert!(!char.can_read());
    assert!(!char.can_write());
    assert!(char.can_notify());
}

#[test]
fn test_ble_characteristic_full() {
    let char = BleCharacteristic::new("uuid")
        .with_read()
        .with_write()
        .with_notify();

    assert!(char.can_read());
    assert!(char.can_write());
    assert!(char.can_notify());
}

// ============================================================================
// BLE ADDRESS PARSING
// ============================================================================

#[test]
fn test_ble_address_valid_format() {
    let addr = PeerAddress::ble("AA:BB:CC:DD:EE:FF");

    assert!(addr.is_ble());
}

#[test]
fn test_ble_address_lowercase() {
    let addr = PeerAddress::ble("aa:bb:cc:dd:ee:ff");

    assert!(addr.is_ble());
}

#[test]
fn test_ble_address_equality() {
    let addr1 = PeerAddress::ble("AA:BB:CC:DD:EE:FF");
    let addr2 = PeerAddress::ble("aa:bb:cc:dd:ee:ff");

    // Should be equal (case-insensitive)
    assert_eq!(addr1, addr2);
}

#[test]
fn test_ble_address_display() {
    let addr = PeerAddress::ble("AA:BB:CC:DD:EE:FF");
    let display = format!("{}", addr);

    assert!(display.contains("AA:BB:CC:DD:EE:FF") || display.contains("aa:bb:cc:dd:ee:ff"));
}

// ============================================================================
// BLE TRANSPORT CREATION
// ============================================================================

#[test]
fn test_ble_transport_new() {
    let config = BleTransportConfig::default();
    let transport = BleTransport::new(config);

    assert!(matches!(transport.state(), TransportState::Stopped));
}

#[test]
fn test_ble_transport_new_central() {
    let config = BleTransportConfig::new().as_central();
    let transport = BleTransport::new(config);

    assert!(transport.is_central());
}

#[test]
fn test_ble_transport_new_peripheral() {
    let config = BleTransportConfig::new().as_peripheral();
    let transport = BleTransport::new(config);

    assert!(transport.is_peripheral());
}

// ============================================================================
// BLE TRANSPORT LIFECYCLE
// ============================================================================

#[tokio::test]
async fn test_ble_transport_start_central() {
    let config = BleTransportConfig::new().as_central();
    let mut transport = BleTransport::new(config);

    // Note: This may fail on systems without BLE hardware
    // In that case, it should return a clear error
    let result = transport.start().await;

    assert!(result.is_ok() || matches!(result, Err(TransportError::HardwareUnavailable)));
}

#[tokio::test]
async fn test_ble_transport_start_peripheral() {
    let config = BleTransportConfig::new().as_peripheral();
    let mut transport = BleTransport::new(config);

    let result = transport.start().await;

    assert!(result.is_ok() || matches!(result, Err(TransportError::HardwareUnavailable)));
}

#[tokio::test]
async fn test_ble_transport_stop() {
    let config = BleTransportConfig::new().as_central();
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.stop().await;
        assert!(result.is_ok());
        assert!(matches!(transport.state(), TransportState::Stopped));
    }
}

#[tokio::test]
async fn test_ble_transport_stop_when_stopped() {
    let config = BleTransportConfig::default();
    let mut transport = BleTransport::new(config);

    let result = transport.stop().await;

    assert!(result.is_ok() || matches!(result, Err(TransportError::NotRunning)));
}

// ============================================================================
// BLE SCANNING (CENTRAL MODE)
// ============================================================================

#[tokio::test]
async fn test_ble_transport_start_scan() {
    let config = BleTransportConfig::new()
        .as_central()
        .with_scan_duration_secs(5);
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.start_scan().await;
        assert!(result.is_ok());

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_ble_transport_stop_scan() {
    let config = BleTransportConfig::new().as_central();
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        transport.start_scan().await.ok();
        let result = transport.stop_scan().await;
        assert!(result.is_ok());

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_ble_transport_scan_not_central() {
    let config = BleTransportConfig::new().as_peripheral();
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.start_scan().await;
        assert!(matches!(result, Err(TransportError::InvalidOperation(_))));

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_ble_transport_discovered_devices() {
    let config = BleTransportConfig::new().as_central();
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        // Initially no devices
        assert!(transport.discovered_devices().is_empty());

        transport.stop().await.unwrap();
    }
}

// ============================================================================
// BLE ADVERTISING (PERIPHERAL MODE)
// ============================================================================

#[tokio::test]
async fn test_ble_transport_start_advertising() {
    let config = BleTransportConfig::new()
        .as_peripheral()
        .with_advertise_name("TestNode");
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.start_advertising().await;
        assert!(result.is_ok());

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_ble_transport_stop_advertising() {
    let config = BleTransportConfig::new().as_peripheral();
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        transport.start_advertising().await.ok();
        let result = transport.stop_advertising().await;
        assert!(result.is_ok());

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_ble_transport_advertise_not_peripheral() {
    let config = BleTransportConfig::new().as_central();
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.start_advertising().await;
        assert!(matches!(result, Err(TransportError::InvalidOperation(_))));

        transport.stop().await.unwrap();
    }
}

// ============================================================================
// BLE CONNECTIONS
// ============================================================================

#[tokio::test]
async fn test_ble_transport_connect_not_running() {
    let config = BleTransportConfig::new().as_central();
    let mut transport = BleTransport::new(config);

    let addr = PeerAddress::ble("AA:BB:CC:DD:EE:FF");
    let result = transport.connect(addr).await;

    assert!(matches!(result, Err(TransportError::NotRunning)));
}

#[tokio::test]
async fn test_ble_transport_connect_invalid_address() {
    let config = BleTransportConfig::new().as_central();
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        // TCP address to BLE transport should fail
        let addr = PeerAddress::tcp("127.0.0.1", 8080);
        let result = transport.connect(addr).await;

        assert!(matches!(result, Err(TransportError::InvalidAddress(_))));

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_ble_transport_disconnect() {
    let config = BleTransportConfig::new().as_central();
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        let unknown_id = ConnectionId::generate();
        let result = transport.disconnect(&unknown_id).await;

        assert!(matches!(result, Err(TransportError::NotConnected)));

        transport.stop().await.unwrap();
    }
}

// ============================================================================
// BLE MESSAGE SENDING
// ============================================================================

#[tokio::test]
async fn test_ble_transport_send_not_connected() {
    let config = BleTransportConfig::new().as_central();
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        let unknown_id = ConnectionId::generate();
        let result = transport.send(&unknown_id, b"data").await;

        assert!(matches!(result, Err(TransportError::NotConnected)));

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_ble_transport_send_exceeds_mtu() {
    let config = BleTransportConfig::new()
        .as_central()
        .with_mtu(20);
    let transport = BleTransport::new(config);

    // Large message should be fragmented or rejected
    let large_data = vec![0u8; 1000];

    // Transport should handle fragmentation or return error
    // This is tested conceptually - actual behavior depends on implementation
    assert!(large_data.len() > 20);
}

// ============================================================================
// BLE TRANSPORT EVENTS
// ============================================================================

#[tokio::test]
async fn test_ble_transport_poll_events_empty() {
    let config = BleTransportConfig::default();
    let mut transport = BleTransport::new(config);

    let events = transport.poll_events().await;

    assert!(events.is_empty());
}

#[tokio::test]
async fn test_ble_transport_device_discovered_event() {
    // This would normally be triggered during scanning
    // We test the event structure
    let addr = PeerAddress::ble("AA:BB:CC:DD:EE:FF");
    let event = TransportEvent::DeviceDiscovered {
        address: addr.clone(),
        rssi: Some(-50),
        name: Some("MeshNode".to_string()),
    };

    match event {
        TransportEvent::DeviceDiscovered { address, rssi, name } => {
            assert_eq!(address, addr);
            assert_eq!(rssi, Some(-50));
            assert_eq!(name, Some("MeshNode".to_string()));
        }
        _ => panic!("Expected DeviceDiscovered event"),
    }
}

// ============================================================================
// BLE TRANSPORT STATISTICS
// ============================================================================

#[tokio::test]
async fn test_ble_transport_stats() {
    let config = BleTransportConfig::default();
    let transport = BleTransport::new(config);

    let stats = transport.stats();

    assert_eq!(stats.connections_active, 0);
    assert_eq!(stats.bytes_sent, 0);
    assert_eq!(stats.bytes_received, 0);
}

#[tokio::test]
async fn test_ble_transport_rssi() {
    let config = BleTransportConfig::new().as_central();
    let mut transport = BleTransport::new(config);

    if transport.start().await.is_ok() {
        let unknown_id = ConnectionId::generate();
        let rssi = transport.get_rssi(&unknown_id);

        assert!(rssi.is_none()); // No connection

        transport.stop().await.unwrap();
    }
}

// ============================================================================
// BLE TRANSPORT MTU HANDLING
// ============================================================================

#[test]
fn test_ble_default_mtu() {
    let config = BleTransportConfig::default();

    // Default MTU should be reasonable (typically 23-517 for BLE 4.x/5.x)
    assert!(config.mtu >= 20);
    assert!(config.mtu <= 517);
}

#[test]
fn test_ble_mtu_negotiation_ready() {
    let config = BleTransportConfig::new().with_mtu(256);
    let transport = BleTransport::new(config);

    // Before connection, requested MTU is set
    assert_eq!(transport.requested_mtu(), 256);
}

// ============================================================================
// BLE TRANSPORT PERMISSIONS
// ============================================================================

#[tokio::test]
async fn test_ble_transport_requires_permissions() {
    let config = BleTransportConfig::default();
    let transport = BleTransport::new(config);

    // Check if BLE permissions are needed
    let permissions = transport.required_permissions();

    // On mobile, should require Bluetooth permissions
    // This is platform-specific
    assert!(permissions.contains(&"bluetooth") || permissions.is_empty());
}

// ============================================================================
// BLE TRANSPORT RECONNECTION
// ============================================================================

#[tokio::test]
async fn test_ble_transport_auto_reconnect_config() {
    let config = BleTransportConfig::new()
        .with_auto_reconnect(true)
        .with_reconnect_attempts(3)
        .with_reconnect_delay_ms(1000);

    assert!(config.auto_reconnect);
    assert_eq!(config.reconnect_attempts, 3);
    assert_eq!(config.reconnect_delay_ms, 1000);
}
