// LoRa Transport Tests
// Tests for the LoRa (Long Range) implementation of the Transport trait
// Designed for Raspberry Pi and embedded systems

use p2pmesh::transport::{
    LoraTransport, LoraTransportConfig, LoraModulation, LoraSpreadingFactor,
    LoraBandwidth, LoraCodingRate, Transport, TransportConfig, TransportError,
    TransportEvent, TransportState, PeerAddress, ConnectionId,
};
use p2pmesh::ledger::NodeId;

// ============================================================================
// LORA TRANSPORT CONFIG
// ============================================================================

#[test]
fn test_lora_config_default() {
    let config = LoraTransportConfig::default();

    assert!(config.frequency > 0);
    assert!(config.device_id > 0 || config.device_id == 0);
    assert!(config.spreading_factor.is_valid());
    assert!(config.bandwidth.is_valid());
}

#[test]
fn test_lora_config_frequency_us915() {
    let config = LoraTransportConfig::new()
        .with_frequency(915_000_000); // 915 MHz (US ISM band)

    assert_eq!(config.frequency, 915_000_000);
}

#[test]
fn test_lora_config_frequency_eu868() {
    let config = LoraTransportConfig::new()
        .with_frequency(868_000_000); // 868 MHz (EU ISM band)

    assert_eq!(config.frequency, 868_000_000);
}

#[test]
fn test_lora_config_frequency_as923() {
    let config = LoraTransportConfig::new()
        .with_frequency(923_000_000); // 923 MHz (Asia ISM band)

    assert_eq!(config.frequency, 923_000_000);
}

#[test]
fn test_lora_config_device_id() {
    let config = LoraTransportConfig::new().with_device_id(0x42);

    assert_eq!(config.device_id, 0x42);
}

#[test]
fn test_lora_config_spreading_factor() {
    let config = LoraTransportConfig::new()
        .with_spreading_factor(LoraSpreadingFactor::SF7);

    assert_eq!(config.spreading_factor, LoraSpreadingFactor::SF7);
}

#[test]
fn test_lora_config_all_spreading_factors() {
    // Test all valid spreading factors (SF7 to SF12)
    for sf in [
        LoraSpreadingFactor::SF7,
        LoraSpreadingFactor::SF8,
        LoraSpreadingFactor::SF9,
        LoraSpreadingFactor::SF10,
        LoraSpreadingFactor::SF11,
        LoraSpreadingFactor::SF12,
    ] {
        assert!(sf.is_valid());
    }
}

#[test]
fn test_lora_config_bandwidth() {
    let config = LoraTransportConfig::new()
        .with_bandwidth(LoraBandwidth::BW125);

    assert_eq!(config.bandwidth, LoraBandwidth::BW125);
}

#[test]
fn test_lora_config_all_bandwidths() {
    for bw in [
        LoraBandwidth::BW125,
        LoraBandwidth::BW250,
        LoraBandwidth::BW500,
    ] {
        assert!(bw.is_valid());
    }
}

#[test]
fn test_lora_config_coding_rate() {
    let config = LoraTransportConfig::new()
        .with_coding_rate(LoraCodingRate::CR4_5);

    assert_eq!(config.coding_rate, LoraCodingRate::CR4_5);
}

#[test]
fn test_lora_config_all_coding_rates() {
    for cr in [
        LoraCodingRate::CR4_5,
        LoraCodingRate::CR4_6,
        LoraCodingRate::CR4_7,
        LoraCodingRate::CR4_8,
    ] {
        assert!(cr.is_valid());
    }
}

#[test]
fn test_lora_config_tx_power() {
    let config = LoraTransportConfig::new().with_tx_power(14); // 14 dBm

    assert_eq!(config.tx_power_dbm, 14);
}

#[test]
fn test_lora_config_tx_power_limits() {
    // Typical max is 20 dBm, min is 2 dBm
    let config_max = LoraTransportConfig::new().with_tx_power(20);
    let config_min = LoraTransportConfig::new().with_tx_power(2);

    assert!(config_max.tx_power_dbm <= 20);
    assert!(config_min.tx_power_dbm >= 2);
}

#[test]
fn test_lora_config_preamble_length() {
    let config = LoraTransportConfig::new().with_preamble_length(8);

    assert_eq!(config.preamble_length, 8);
}

#[test]
fn test_lora_config_sync_word() {
    let config = LoraTransportConfig::new().with_sync_word(0x34);

    assert_eq!(config.sync_word, 0x34);
}

#[test]
fn test_lora_config_crc_enabled() {
    let config = LoraTransportConfig::new().with_crc(true);

    assert!(config.crc_enabled);
}

#[test]
fn test_lora_config_implicit_header() {
    let config = LoraTransportConfig::new().with_implicit_header(true);

    assert!(config.implicit_header);
}

#[test]
fn test_lora_config_base_config() {
    let base = TransportConfig::new().with_max_connections(50);
    let config = LoraTransportConfig::new().with_base_config(base);

    assert_eq!(config.base.max_connections, 50);
}

// ============================================================================
// LORA MODULATION PARAMETERS
// ============================================================================

#[test]
fn test_lora_modulation_creation() {
    let modulation = LoraModulation::new(
        LoraSpreadingFactor::SF7,
        LoraBandwidth::BW125,
        LoraCodingRate::CR4_5,
    );

    assert_eq!(modulation.spreading_factor(), LoraSpreadingFactor::SF7);
    assert_eq!(modulation.bandwidth(), LoraBandwidth::BW125);
    assert_eq!(modulation.coding_rate(), LoraCodingRate::CR4_5);
}

#[test]
fn test_lora_modulation_data_rate() {
    // SF7, BW125 should give highest data rate
    let fast = LoraModulation::new(
        LoraSpreadingFactor::SF7,
        LoraBandwidth::BW125,
        LoraCodingRate::CR4_5,
    );

    // SF12, BW125 should give lowest data rate (but longest range)
    let slow = LoraModulation::new(
        LoraSpreadingFactor::SF12,
        LoraBandwidth::BW125,
        LoraCodingRate::CR4_5,
    );

    assert!(fast.data_rate_bps() > slow.data_rate_bps());
}

#[test]
fn test_lora_modulation_time_on_air() {
    let modulation = LoraModulation::new(
        LoraSpreadingFactor::SF7,
        LoraBandwidth::BW125,
        LoraCodingRate::CR4_5,
    );

    let toa_10_bytes = modulation.time_on_air_ms(10);
    let toa_100_bytes = modulation.time_on_air_ms(100);

    // More bytes = more time
    assert!(toa_100_bytes > toa_10_bytes);
}

#[test]
fn test_lora_modulation_max_payload() {
    let modulation = LoraModulation::new(
        LoraSpreadingFactor::SF7,
        LoraBandwidth::BW125,
        LoraCodingRate::CR4_5,
    );

    // LoRa typically supports up to 255 bytes
    assert!(modulation.max_payload_size() <= 255);
    assert!(modulation.max_payload_size() > 0);
}

// ============================================================================
// LORA ADDRESS
// ============================================================================

#[test]
fn test_lora_address_creation() {
    let addr = PeerAddress::lora(0x01, 915_000_000);

    assert!(addr.is_lora());
    assert!(!addr.is_tcp());
    assert!(!addr.is_ble());
}

#[test]
fn test_lora_address_components() {
    let addr = PeerAddress::lora(0x42, 868_000_000);

    if let PeerAddress::Lora { device_id, frequency } = addr {
        assert_eq!(device_id, 0x42);
        assert_eq!(frequency, 868_000_000);
    } else {
        panic!("Expected LoRa address");
    }
}

#[test]
fn test_lora_address_broadcast() {
    let addr = PeerAddress::lora_broadcast(915_000_000);

    assert!(addr.is_lora());
    assert!(addr.is_broadcast());
}

#[test]
fn test_lora_address_equality() {
    let addr1 = PeerAddress::lora(0x01, 915_000_000);
    let addr2 = PeerAddress::lora(0x01, 915_000_000);
    let addr3 = PeerAddress::lora(0x02, 915_000_000);
    let addr4 = PeerAddress::lora(0x01, 868_000_000);

    assert_eq!(addr1, addr2);
    assert_ne!(addr1, addr3); // Different device ID
    assert_ne!(addr1, addr4); // Different frequency
}

#[test]
fn test_lora_address_display() {
    let addr = PeerAddress::lora(0x42, 915_000_000);
    let display = format!("{}", addr);

    assert!(display.contains("42") || display.contains("0x42"));
    assert!(display.contains("915"));
}

// ============================================================================
// LORA TRANSPORT CREATION
// ============================================================================

#[test]
fn test_lora_transport_new() {
    let config = LoraTransportConfig::default();
    let transport = LoraTransport::new(config);

    assert!(matches!(transport.state(), TransportState::Stopped));
}

#[test]
fn test_lora_transport_with_spi_device() {
    let config = LoraTransportConfig::new()
        .with_spi_device("/dev/spidev0.0")
        .with_reset_pin(17)
        .with_dio0_pin(27);

    assert_eq!(config.spi_device, "/dev/spidev0.0");
    assert_eq!(config.reset_pin, Some(17));
    assert_eq!(config.dio0_pin, Some(27));
}

// ============================================================================
// LORA TRANSPORT LIFECYCLE
// ============================================================================

#[tokio::test]
async fn test_lora_transport_start() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    // May fail on systems without LoRa hardware
    let result = transport.start().await;

    assert!(result.is_ok() || matches!(result, Err(TransportError::HardwareUnavailable)));
}

#[tokio::test]
async fn test_lora_transport_stop() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.stop().await;
        assert!(result.is_ok());
        assert!(matches!(transport.state(), TransportState::Stopped));
    }
}

#[tokio::test]
async fn test_lora_transport_stop_when_stopped() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    let result = transport.stop().await;

    assert!(result.is_ok() || matches!(result, Err(TransportError::NotRunning)));
}

// ============================================================================
// LORA TRANSPORT RADIO OPERATIONS
// ============================================================================

#[tokio::test]
async fn test_lora_transport_set_frequency() {
    let config = LoraTransportConfig::new().with_frequency(915_000_000);
    let mut transport = LoraTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.set_frequency(868_000_000).await;
        assert!(result.is_ok());
        assert_eq!(transport.current_frequency(), 868_000_000);

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_lora_transport_set_spreading_factor() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.set_spreading_factor(LoraSpreadingFactor::SF10).await;
        assert!(result.is_ok());

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_lora_transport_set_tx_power() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.set_tx_power(17).await;
        assert!(result.is_ok());

        transport.stop().await.unwrap();
    }
}

// ============================================================================
// LORA TRANSPORT RX/TX MODES
// ============================================================================

#[tokio::test]
async fn test_lora_transport_receive_mode() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.start_receive().await;
        assert!(result.is_ok());
        assert!(transport.is_receiving());

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_lora_transport_standby_mode() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    if transport.start().await.is_ok() {
        transport.start_receive().await.ok();
        let result = transport.standby().await;
        assert!(result.is_ok());
        assert!(!transport.is_receiving());

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_lora_transport_sleep_mode() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.sleep().await;
        assert!(result.is_ok());
        assert!(transport.is_sleeping());

        transport.stop().await.unwrap();
    }
}

// ============================================================================
// LORA TRANSPORT SENDING
// ============================================================================

#[tokio::test]
async fn test_lora_transport_send_not_running() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    let addr = PeerAddress::lora(0x01, 915_000_000);
    let result = transport.send_to(&addr, b"hello").await;

    assert!(matches!(result, Err(TransportError::NotRunning)));
}

#[tokio::test]
async fn test_lora_transport_send_invalid_address() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    if transport.start().await.is_ok() {
        // TCP address should fail for LoRa
        let addr = PeerAddress::tcp("127.0.0.1", 8080);
        let result = transport.send_to(&addr, b"data").await;

        assert!(matches!(result, Err(TransportError::InvalidAddress(_))));

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_lora_transport_send_exceeds_max_payload() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    if transport.start().await.is_ok() {
        let addr = PeerAddress::lora(0x01, 915_000_000);
        let large_data = vec![0u8; 300]; // Exceeds typical 255 byte limit

        let result = transport.send_to(&addr, &large_data).await;

        assert!(matches!(result, Err(TransportError::PayloadTooLarge)));

        transport.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_lora_transport_broadcast() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.broadcast(b"broadcast message").await;
        assert!(result.is_ok());

        transport.stop().await.unwrap();
    }
}

// ============================================================================
// LORA TRANSPORT RECEIVING
// ============================================================================

#[tokio::test]
async fn test_lora_transport_poll_events_empty() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    let events = transport.poll_events().await;

    assert!(events.is_empty());
}

#[test]
fn test_lora_packet_received_event() {
    let event = TransportEvent::LoraPacketReceived {
        data: vec![1, 2, 3, 4],
        rssi: -80,
        snr: 10.5,
        frequency: 915_000_000,
    };

    match event {
        TransportEvent::LoraPacketReceived { data, rssi, snr, frequency } => {
            assert_eq!(data, vec![1, 2, 3, 4]);
            assert_eq!(rssi, -80);
            assert!((snr - 10.5).abs() < 0.01);
            assert_eq!(frequency, 915_000_000);
        }
        _ => panic!("Expected LoraPacketReceived event"),
    }
}

// ============================================================================
// LORA TRANSPORT STATISTICS
// ============================================================================

#[tokio::test]
async fn test_lora_transport_stats() {
    let config = LoraTransportConfig::default();
    let transport = LoraTransport::new(config);

    let stats = transport.stats();

    assert_eq!(stats.packets_sent, 0);
    assert_eq!(stats.packets_received, 0);
    assert_eq!(stats.bytes_sent, 0);
    assert_eq!(stats.bytes_received, 0);
}

#[tokio::test]
async fn test_lora_transport_rssi() {
    let config = LoraTransportConfig::default();
    let transport = LoraTransport::new(config);

    // No packet received yet
    let rssi = transport.last_rssi();
    assert!(rssi.is_none());
}

#[tokio::test]
async fn test_lora_transport_snr() {
    let config = LoraTransportConfig::default();
    let transport = LoraTransport::new(config);

    // No packet received yet
    let snr = transport.last_snr();
    assert!(snr.is_none());
}

// ============================================================================
// LORA TRANSPORT DUTY CYCLE
// ============================================================================

#[test]
fn test_lora_config_duty_cycle() {
    // EU regulations require duty cycle limits
    let config = LoraTransportConfig::new()
        .with_duty_cycle_percent(1.0); // 1% duty cycle

    assert!((config.duty_cycle_percent - 1.0).abs() < 0.01);
}

#[tokio::test]
async fn test_lora_transport_time_until_transmit() {
    let config = LoraTransportConfig::new()
        .with_duty_cycle_percent(1.0);
    let transport = LoraTransport::new(config);

    // Initially should be able to transmit immediately
    let time = transport.time_until_transmit_ms();
    assert_eq!(time, 0);
}

#[tokio::test]
async fn test_lora_transport_can_transmit() {
    let config = LoraTransportConfig::default();
    let transport = LoraTransport::new(config);

    // Initially should be able to transmit
    assert!(transport.can_transmit());
}

// ============================================================================
// LORA TRANSPORT CAD (CHANNEL ACTIVITY DETECTION)
// ============================================================================

#[tokio::test]
async fn test_lora_transport_channel_activity_detection() {
    let config = LoraTransportConfig::default();
    let mut transport = LoraTransport::new(config);

    if transport.start().await.is_ok() {
        let result = transport.check_channel_activity().await;

        // Should return whether channel is busy or not
        assert!(result.is_ok());

        transport.stop().await.unwrap();
    }
}

// ============================================================================
// LORA TRANSPORT MESH ADDRESSING
// ============================================================================

#[test]
fn test_lora_mesh_header() {
    // LoRa packets should include mesh routing header
    let header = p2pmesh::transport::LoraMeshHeader::new(
        0x01, // Source device ID
        0x02, // Destination device ID
        0x00, // Flags (broadcast = false)
        0,    // Hop count
    );

    assert_eq!(header.source(), 0x01);
    assert_eq!(header.destination(), 0x02);
    assert!(!header.is_broadcast());
    assert_eq!(header.hop_count(), 0);
}

#[test]
fn test_lora_mesh_header_broadcast() {
    let header = p2pmesh::transport::LoraMeshHeader::broadcast(
        0x01, // Source device ID
    );

    assert_eq!(header.source(), 0x01);
    assert_eq!(header.destination(), 0xFF); // Broadcast address
    assert!(header.is_broadcast());
}

#[test]
fn test_lora_mesh_header_increment_hop() {
    let mut header = p2pmesh::transport::LoraMeshHeader::new(0x01, 0x02, 0x00, 0);

    header.increment_hop();
    assert_eq!(header.hop_count(), 1);

    header.increment_hop();
    assert_eq!(header.hop_count(), 2);
}

#[test]
fn test_lora_mesh_header_serialization() {
    let header = p2pmesh::transport::LoraMeshHeader::new(0x01, 0x02, 0x00, 3);

    let bytes = header.to_bytes();
    let restored = p2pmesh::transport::LoraMeshHeader::from_bytes(&bytes).unwrap();

    assert_eq!(header.source(), restored.source());
    assert_eq!(header.destination(), restored.destination());
    assert_eq!(header.hop_count(), restored.hop_count());
}

// ============================================================================
// LORA TRANSPORT POWER MANAGEMENT
// ============================================================================

#[test]
fn test_lora_config_low_power_mode() {
    let config = LoraTransportConfig::new().with_low_power_mode(true);

    assert!(config.low_power_mode);
}

#[tokio::test]
async fn test_lora_transport_battery_voltage() {
    let config = LoraTransportConfig::default();
    let transport = LoraTransport::new(config);

    // If battery monitoring is supported
    let voltage = transport.battery_voltage();

    // Either returns None or a reasonable voltage
    if let Some(v) = voltage {
        assert!(v > 0.0 && v < 5.0); // Typical range for battery
    }
}

// ============================================================================
// LORA TRANSPORT ERROR HANDLING
// ============================================================================

#[test]
fn test_lora_transport_error_crc_mismatch() {
    let error = TransportError::LoraCrcMismatch;

    assert!(matches!(error, TransportError::LoraCrcMismatch));
    assert!(!error.is_retryable()); // CRC error should not retry same packet
}

#[test]
fn test_lora_transport_error_timeout() {
    let error = TransportError::LoraReceiveTimeout;

    assert!(matches!(error, TransportError::LoraReceiveTimeout));
    assert!(error.is_retryable());
}

#[test]
fn test_lora_transport_error_channel_busy() {
    let error = TransportError::LoraChannelBusy;

    assert!(matches!(error, TransportError::LoraChannelBusy));
    assert!(error.is_retryable());
}
