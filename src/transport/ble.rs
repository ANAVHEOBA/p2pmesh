// BLE Transport Implementation
// Provides Bluetooth Low Energy transport for mobile peer-to-peer communication

use crate::transport::{
    ConnectionId, ConnectionInfo, ConnectionState, PeerAddress,
    Transport, TransportConfig, TransportError, TransportEvent, TransportState, TransportStats,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// BLE SERVICE AND CHARACTERISTIC
// ============================================================================

/// BLE GATT Service definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BleService {
    uuid: String,
    characteristics: Vec<BleCharacteristic>,
}

impl BleService {
    pub fn new(uuid: &str) -> Self {
        Self {
            uuid: uuid.to_string(),
            characteristics: Vec::new(),
        }
    }

    pub fn uuid(&self) -> &str {
        &self.uuid
    }

    pub fn characteristics(&self) -> &[BleCharacteristic] {
        &self.characteristics
    }

    pub fn with_characteristic(mut self, characteristic: BleCharacteristic) -> Self {
        self.characteristics.push(characteristic);
        self
    }
}

/// BLE GATT Characteristic definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BleCharacteristic {
    uuid: String,
    read: bool,
    write: bool,
    notify: bool,
}

impl BleCharacteristic {
    pub fn new(uuid: &str) -> Self {
        Self {
            uuid: uuid.to_string(),
            read: false,
            write: false,
            notify: false,
        }
    }

    pub fn uuid(&self) -> &str {
        &self.uuid
    }

    pub fn with_read(mut self) -> Self {
        self.read = true;
        self
    }

    pub fn with_write(mut self) -> Self {
        self.write = true;
        self
    }

    pub fn with_notify(mut self) -> Self {
        self.notify = true;
        self
    }

    pub fn can_read(&self) -> bool {
        self.read
    }

    pub fn can_write(&self) -> bool {
        self.write
    }

    pub fn can_notify(&self) -> bool {
        self.notify
    }
}

// ============================================================================
// BLE TRANSPORT CONFIG
// ============================================================================

/// Configuration for BLE transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BleTransportConfig {
    /// Base transport configuration
    pub base: TransportConfig,
    /// Service UUID for mesh communication
    pub service_uuid: String,
    /// Characteristic UUID for data transfer
    pub characteristic_uuid: String,
    /// Maximum Transmission Unit
    pub mtu: u16,
    /// Scan duration in seconds
    pub scan_duration_secs: u32,
    /// Advertise name (for peripheral mode)
    pub advertise_name: Option<String>,
    /// Operate as BLE Central (initiator)
    pub is_central: bool,
    /// Operate as BLE Peripheral (advertiser)
    pub is_peripheral: bool,
    /// Auto-reconnect on disconnect
    pub auto_reconnect: bool,
    /// Number of reconnection attempts
    pub reconnect_attempts: u32,
    /// Delay between reconnection attempts in ms
    pub reconnect_delay_ms: u32,
}

impl Default for BleTransportConfig {
    fn default() -> Self {
        Self {
            base: TransportConfig::default(),
            service_uuid: "6e400001-b5a3-f393-e0a9-e50e24dcca9e".to_string(), // Nordic UART
            characteristic_uuid: "6e400002-b5a3-f393-e0a9-e50e24dcca9e".to_string(),
            mtu: 247, // BLE 4.2 default
            scan_duration_secs: 10,
            advertise_name: None,
            is_central: false,
            is_peripheral: false,
            auto_reconnect: false,
            reconnect_attempts: 3,
            reconnect_delay_ms: 1000,
        }
    }
}

impl BleTransportConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_config(mut self, base: TransportConfig) -> Self {
        self.base = base;
        self
    }

    pub fn with_service_uuid(mut self, uuid: &str) -> Self {
        self.service_uuid = uuid.to_string();
        self
    }

    pub fn with_characteristic_uuid(mut self, uuid: &str) -> Self {
        self.characteristic_uuid = uuid.to_string();
        self
    }

    pub fn with_mtu(mut self, mtu: u16) -> Self {
        self.mtu = mtu.max(20); // BLE minimum
        self
    }

    pub fn with_scan_duration_secs(mut self, secs: u32) -> Self {
        self.scan_duration_secs = secs;
        self
    }

    pub fn with_advertise_name(mut self, name: &str) -> Self {
        self.advertise_name = Some(name.to_string());
        self
    }

    pub fn as_central(mut self) -> Self {
        self.is_central = true;
        self
    }

    pub fn as_peripheral(mut self) -> Self {
        self.is_peripheral = true;
        self
    }

    pub fn with_auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }

    pub fn with_reconnect_attempts(mut self, attempts: u32) -> Self {
        self.reconnect_attempts = attempts;
        self
    }

    pub fn with_reconnect_delay_ms(mut self, delay_ms: u32) -> Self {
        self.reconnect_delay_ms = delay_ms;
        self
    }
}

// ============================================================================
// DISCOVERED DEVICE
// ============================================================================

#[derive(Debug, Clone)]
struct DiscoveredDevice {
    address: PeerAddress,
    name: Option<String>,
    rssi: Option<i8>,
}

// ============================================================================
// BLE TRANSPORT
// ============================================================================

/// BLE transport implementation
pub struct BleTransport {
    config: BleTransportConfig,
    state: TransportState,
    connections: HashMap<ConnectionId, ConnectionInfo>,
    discovered_devices: Vec<DiscoveredDevice>,
    events: Vec<TransportEvent>,
    stats: TransportStats,
    is_scanning: bool,
    is_advertising: bool,
}

impl BleTransport {
    pub fn new(config: BleTransportConfig) -> Self {
        Self {
            config,
            state: TransportState::Stopped,
            connections: HashMap::new(),
            discovered_devices: Vec::new(),
            events: Vec::new(),
            stats: TransportStats::default(),
            is_scanning: false,
            is_advertising: false,
        }
    }

    /// Check if operating as central
    pub fn is_central(&self) -> bool {
        self.config.is_central
    }

    /// Check if operating as peripheral
    pub fn is_peripheral(&self) -> bool {
        self.config.is_peripheral
    }

    /// Get requested MTU
    pub fn requested_mtu(&self) -> u16 {
        self.config.mtu
    }

    /// Start scanning for BLE devices (central mode)
    pub async fn start_scan(&mut self) -> Result<(), TransportError> {
        if !self.config.is_central {
            return Err(TransportError::InvalidOperation(
                "Scanning requires central mode".to_string()
            ));
        }
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }

        self.is_scanning = true;
        self.discovered_devices.clear();

        // In a real implementation, this would start BLE scanning
        // For now, we just set the flag

        Ok(())
    }

    /// Stop scanning
    pub async fn stop_scan(&mut self) -> Result<(), TransportError> {
        self.is_scanning = false;
        Ok(())
    }

    /// Start advertising (peripheral mode)
    pub async fn start_advertising(&mut self) -> Result<(), TransportError> {
        if !self.config.is_peripheral {
            return Err(TransportError::InvalidOperation(
                "Advertising requires peripheral mode".to_string()
            ));
        }
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }

        self.is_advertising = true;

        // In a real implementation, this would start BLE advertising

        Ok(())
    }

    /// Stop advertising
    pub async fn stop_advertising(&mut self) -> Result<(), TransportError> {
        self.is_advertising = false;
        Ok(())
    }

    /// Get discovered devices
    pub fn discovered_devices(&self) -> Vec<PeerAddress> {
        self.discovered_devices.iter().map(|d| d.address.clone()).collect()
    }

    /// Get RSSI for a connection
    pub fn get_rssi(&self, _connection_id: &ConnectionId) -> Option<i8> {
        // In a real implementation, this would query the BLE stack
        None
    }

    /// Get required permissions for BLE operations
    pub fn required_permissions(&self) -> Vec<&'static str> {
        vec!["bluetooth"]
    }
}

impl Transport for BleTransport {
    async fn start(&mut self) -> Result<(), TransportError> {
        if self.state.is_running() {
            return Err(TransportError::AlreadyRunning);
        }

        self.state = TransportState::Starting;

        // In a real implementation, this would:
        // 1. Initialize BLE adapter
        // 2. Check permissions
        // 3. Set up GATT server if peripheral
        // 4. Start discovery if central

        // For now, simulate successful start
        // On systems without BLE, we'd return HardwareUnavailable

        self.state = TransportState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), TransportError> {
        if !self.state.is_running() && !matches!(self.state, TransportState::Stopped) {
            return Err(TransportError::NotRunning);
        }

        self.state = TransportState::Stopping;

        // Stop scanning/advertising
        self.is_scanning = false;
        self.is_advertising = false;

        // Disconnect all
        self.connections.clear();
        self.stats.connections_active = 0;

        self.state = TransportState::Stopped;
        Ok(())
    }

    async fn connect(&mut self, address: PeerAddress) -> Result<ConnectionId, TransportError> {
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }

        // Validate address type
        if !address.is_ble() {
            return Err(TransportError::InvalidAddress("Expected BLE address".to_string()));
        }

        // Check max connections
        if self.connections.len() >= self.config.base.max_connections as usize {
            return Err(TransportError::MaxConnectionsReached);
        }

        // In a real implementation, this would initiate BLE connection
        let mut info = ConnectionInfo::new(address.clone());
        let conn_id = info.id().clone();
        info.set_state(ConnectionState::Connected);

        self.connections.insert(conn_id.clone(), info);
        self.stats.connections_active = self.connections.len() as u32;
        self.stats.connections_total += 1;

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
            reason: "Disconnected".to_string(),
        });

        Ok(())
    }

    async fn send(&mut self, connection_id: &ConnectionId, data: &[u8]) -> Result<usize, TransportError> {
        let connection = self.connections.get_mut(connection_id)
            .ok_or(TransportError::NotConnected)?;

        // Check MTU
        if data.len() > self.config.mtu as usize {
            return Err(TransportError::PayloadTooLarge);
        }

        // In a real implementation, this would write to BLE characteristic
        connection.record_bytes_sent(data.len() as u64);
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
        std::mem::take(&mut self.events)
    }

    fn state(&self) -> &TransportState {
        &self.state
    }

    fn local_address(&self) -> Option<PeerAddress> {
        // In a real implementation, this would return the local BLE address
        None
    }

    fn connection_count(&self) -> usize {
        self.connections.len()
    }

    fn connection_info(&self, connection_id: &ConnectionId) -> Option<&ConnectionInfo> {
        self.connections.get(connection_id)
    }

    fn stats(&self) -> TransportStats {
        self.stats.clone()
    }
}
