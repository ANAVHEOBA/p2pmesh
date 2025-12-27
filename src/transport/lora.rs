// LoRa Transport Implementation
// Provides Long Range (LoRa) radio transport for Raspberry Pi and embedded systems

use crate::transport::{
    ConnectionId, ConnectionInfo, PeerAddress,
    Transport, TransportConfig, TransportError, TransportEvent, TransportState, TransportStats,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// LORA MODULATION PARAMETERS
// ============================================================================

/// LoRa Spreading Factor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoraSpreadingFactor {
    SF7,
    SF8,
    SF9,
    SF10,
    SF11,
    SF12,
}

impl LoraSpreadingFactor {
    pub fn is_valid(&self) -> bool {
        true // All defined values are valid
    }

    pub fn to_value(&self) -> u8 {
        match self {
            Self::SF7 => 7,
            Self::SF8 => 8,
            Self::SF9 => 9,
            Self::SF10 => 10,
            Self::SF11 => 11,
            Self::SF12 => 12,
        }
    }
}

impl Default for LoraSpreadingFactor {
    fn default() -> Self {
        Self::SF7
    }
}

/// LoRa Bandwidth
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoraBandwidth {
    BW125, // 125 kHz
    BW250, // 250 kHz
    BW500, // 500 kHz
}

impl LoraBandwidth {
    pub fn is_valid(&self) -> bool {
        true
    }

    pub fn to_hz(&self) -> u32 {
        match self {
            Self::BW125 => 125_000,
            Self::BW250 => 250_000,
            Self::BW500 => 500_000,
        }
    }
}

impl Default for LoraBandwidth {
    fn default() -> Self {
        Self::BW125
    }
}

/// LoRa Coding Rate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoraCodingRate {
    CR4_5, // 4/5
    CR4_6, // 4/6
    CR4_7, // 4/7
    CR4_8, // 4/8
}

impl LoraCodingRate {
    pub fn is_valid(&self) -> bool {
        true
    }

    pub fn to_value(&self) -> u8 {
        match self {
            Self::CR4_5 => 5,
            Self::CR4_6 => 6,
            Self::CR4_7 => 7,
            Self::CR4_8 => 8,
        }
    }
}

impl Default for LoraCodingRate {
    fn default() -> Self {
        Self::CR4_5
    }
}

/// LoRa Modulation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoraModulation {
    spreading_factor: LoraSpreadingFactor,
    bandwidth: LoraBandwidth,
    coding_rate: LoraCodingRate,
}

impl LoraModulation {
    pub fn new(
        spreading_factor: LoraSpreadingFactor,
        bandwidth: LoraBandwidth,
        coding_rate: LoraCodingRate,
    ) -> Self {
        Self {
            spreading_factor,
            bandwidth,
            coding_rate,
        }
    }

    pub fn spreading_factor(&self) -> LoraSpreadingFactor {
        self.spreading_factor
    }

    pub fn bandwidth(&self) -> LoraBandwidth {
        self.bandwidth
    }

    pub fn coding_rate(&self) -> LoraCodingRate {
        self.coding_rate
    }

    /// Calculate data rate in bits per second
    pub fn data_rate_bps(&self) -> u32 {
        let sf = self.spreading_factor.to_value() as u32;
        let bw = self.bandwidth.to_hz();
        let cr = self.coding_rate.to_value() as u32;

        // Data rate = SF * (BW / 2^SF) * (4 / CR)
        let rate = (sf * bw * 4) / ((1 << sf) * cr);
        rate
    }

    /// Calculate time on air for a payload
    pub fn time_on_air_ms(&self, payload_bytes: usize) -> u32 {
        let sf = self.spreading_factor.to_value() as f64;
        let bw = self.bandwidth.to_hz() as f64;
        let cr = self.coding_rate.to_value() as f64;

        // Simplified calculation
        let symbol_duration = (2.0_f64.powf(sf)) / bw * 1000.0;
        let preamble_symbols = 8.0 + 4.25;
        let preamble_time = preamble_symbols * symbol_duration;

        let payload_symbols = 8.0 + ((8.0 * payload_bytes as f64 - 4.0 * sf + 28.0) / (4.0 * sf)).ceil() * cr;
        let payload_time = payload_symbols * symbol_duration;

        (preamble_time + payload_time) as u32
    }

    /// Get maximum payload size
    pub fn max_payload_size(&self) -> usize {
        // LoRa max payload depends on SF and region, typically 255 bytes max
        match self.spreading_factor {
            LoraSpreadingFactor::SF7 | LoraSpreadingFactor::SF8 => 255,
            LoraSpreadingFactor::SF9 => 222,
            LoraSpreadingFactor::SF10 => 222,
            LoraSpreadingFactor::SF11 => 109,
            LoraSpreadingFactor::SF12 => 51,
        }
    }
}

impl Default for LoraModulation {
    fn default() -> Self {
        Self {
            spreading_factor: LoraSpreadingFactor::default(),
            bandwidth: LoraBandwidth::default(),
            coding_rate: LoraCodingRate::default(),
        }
    }
}

// ============================================================================
// LORA MESH HEADER
// ============================================================================

/// Mesh routing header for LoRa packets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoraMeshHeader {
    source: u8,
    destination: u8,
    flags: u8,
    hop_count: u8,
}

impl LoraMeshHeader {
    pub fn new(source: u8, destination: u8, flags: u8, hop_count: u8) -> Self {
        Self {
            source,
            destination,
            flags,
            hop_count,
        }
    }

    pub fn broadcast(source: u8) -> Self {
        Self {
            source,
            destination: 0xFF,
            flags: 0x01, // Broadcast flag
            hop_count: 0,
        }
    }

    pub fn source(&self) -> u8 {
        self.source
    }

    pub fn destination(&self) -> u8 {
        self.destination
    }

    pub fn is_broadcast(&self) -> bool {
        self.destination == 0xFF || (self.flags & 0x01) != 0
    }

    pub fn hop_count(&self) -> u8 {
        self.hop_count
    }

    pub fn increment_hop(&mut self) {
        self.hop_count = self.hop_count.saturating_add(1);
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        [self.source, self.destination, self.flags, self.hop_count]
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TransportError> {
        if bytes.len() < 4 {
            return Err(TransportError::ReceiveFailed("Header too short".to_string()));
        }
        Ok(Self {
            source: bytes[0],
            destination: bytes[1],
            flags: bytes[2],
            hop_count: bytes[3],
        })
    }
}

// ============================================================================
// LORA TRANSPORT CONFIG
// ============================================================================

/// Configuration for LoRa transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoraTransportConfig {
    /// Base transport configuration
    pub base: TransportConfig,
    /// Radio frequency in Hz
    pub frequency: u32,
    /// Local device ID
    pub device_id: u8,
    /// Spreading factor
    pub spreading_factor: LoraSpreadingFactor,
    /// Bandwidth
    pub bandwidth: LoraBandwidth,
    /// Coding rate
    pub coding_rate: LoraCodingRate,
    /// Transmit power in dBm
    pub tx_power_dbm: i8,
    /// Preamble length
    pub preamble_length: u16,
    /// Sync word
    pub sync_word: u8,
    /// Enable CRC
    pub crc_enabled: bool,
    /// Use implicit header mode
    pub implicit_header: bool,
    /// Duty cycle percentage (for regulatory compliance)
    pub duty_cycle_percent: f32,
    /// SPI device path (for Raspberry Pi)
    pub spi_device: String,
    /// Reset GPIO pin
    pub reset_pin: Option<u8>,
    /// DIO0 GPIO pin (for interrupts)
    pub dio0_pin: Option<u8>,
    /// Low power mode
    pub low_power_mode: bool,
}

impl Default for LoraTransportConfig {
    fn default() -> Self {
        Self {
            base: TransportConfig::default(),
            frequency: 915_000_000, // US ISM band
            device_id: 0x01,
            spreading_factor: LoraSpreadingFactor::SF7,
            bandwidth: LoraBandwidth::BW125,
            coding_rate: LoraCodingRate::CR4_5,
            tx_power_dbm: 14,
            preamble_length: 8,
            sync_word: 0x12,
            crc_enabled: true,
            implicit_header: false,
            duty_cycle_percent: 1.0,
            spi_device: "/dev/spidev0.0".to_string(),
            reset_pin: None,
            dio0_pin: None,
            low_power_mode: false,
        }
    }
}

impl LoraTransportConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_config(mut self, base: TransportConfig) -> Self {
        self.base = base;
        self
    }

    pub fn with_frequency(mut self, freq: u32) -> Self {
        self.frequency = freq;
        self
    }

    pub fn with_device_id(mut self, id: u8) -> Self {
        self.device_id = id;
        self
    }

    pub fn with_spreading_factor(mut self, sf: LoraSpreadingFactor) -> Self {
        self.spreading_factor = sf;
        self
    }

    pub fn with_bandwidth(mut self, bw: LoraBandwidth) -> Self {
        self.bandwidth = bw;
        self
    }

    pub fn with_coding_rate(mut self, cr: LoraCodingRate) -> Self {
        self.coding_rate = cr;
        self
    }

    pub fn with_tx_power(mut self, power: i8) -> Self {
        self.tx_power_dbm = power.clamp(2, 20);
        self
    }

    pub fn with_preamble_length(mut self, len: u16) -> Self {
        self.preamble_length = len;
        self
    }

    pub fn with_sync_word(mut self, word: u8) -> Self {
        self.sync_word = word;
        self
    }

    pub fn with_crc(mut self, enabled: bool) -> Self {
        self.crc_enabled = enabled;
        self
    }

    pub fn with_implicit_header(mut self, implicit: bool) -> Self {
        self.implicit_header = implicit;
        self
    }

    pub fn with_duty_cycle_percent(mut self, percent: f32) -> Self {
        self.duty_cycle_percent = percent;
        self
    }

    pub fn with_spi_device(mut self, device: &str) -> Self {
        self.spi_device = device.to_string();
        self
    }

    pub fn with_reset_pin(mut self, pin: u8) -> Self {
        self.reset_pin = Some(pin);
        self
    }

    pub fn with_dio0_pin(mut self, pin: u8) -> Self {
        self.dio0_pin = Some(pin);
        self
    }

    pub fn with_low_power_mode(mut self, enabled: bool) -> Self {
        self.low_power_mode = enabled;
        self
    }
}

// ============================================================================
// LORA TRANSPORT
// ============================================================================

/// LoRa transport implementation
pub struct LoraTransport {
    config: LoraTransportConfig,
    state: TransportState,
    connections: HashMap<ConnectionId, ConnectionInfo>,
    events: Vec<TransportEvent>,
    stats: TransportStats,
    current_frequency: u32,
    is_receiving: bool,
    is_sleeping: bool,
    last_rssi: Option<i16>,
    last_snr: Option<f32>,
    last_tx_time: Option<u64>,
}

impl LoraTransport {
    pub fn new(config: LoraTransportConfig) -> Self {
        let freq = config.frequency;
        Self {
            config,
            state: TransportState::Stopped,
            connections: HashMap::new(),
            events: Vec::new(),
            stats: TransportStats::default(),
            current_frequency: freq,
            is_receiving: false,
            is_sleeping: false,
            last_rssi: None,
            last_snr: None,
            last_tx_time: None,
        }
    }

    /// Get current frequency
    pub fn current_frequency(&self) -> u32 {
        self.current_frequency
    }

    /// Set frequency
    pub async fn set_frequency(&mut self, freq: u32) -> Result<(), TransportError> {
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }
        self.current_frequency = freq;
        Ok(())
    }

    /// Set spreading factor
    pub async fn set_spreading_factor(&mut self, sf: LoraSpreadingFactor) -> Result<(), TransportError> {
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }
        self.config.spreading_factor = sf;
        Ok(())
    }

    /// Set transmit power
    pub async fn set_tx_power(&mut self, power: i8) -> Result<(), TransportError> {
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }
        self.config.tx_power_dbm = power.clamp(2, 20);
        Ok(())
    }

    /// Start receive mode
    pub async fn start_receive(&mut self) -> Result<(), TransportError> {
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }
        self.is_receiving = true;
        self.is_sleeping = false;
        Ok(())
    }

    /// Enter standby mode
    pub async fn standby(&mut self) -> Result<(), TransportError> {
        self.is_receiving = false;
        self.is_sleeping = false;
        Ok(())
    }

    /// Enter sleep mode
    pub async fn sleep(&mut self) -> Result<(), TransportError> {
        self.is_receiving = false;
        self.is_sleeping = true;
        Ok(())
    }

    /// Check if receiving
    pub fn is_receiving(&self) -> bool {
        self.is_receiving
    }

    /// Check if sleeping
    pub fn is_sleeping(&self) -> bool {
        self.is_sleeping
    }

    /// Send to a specific address
    pub async fn send_to(&mut self, address: &PeerAddress, data: &[u8]) -> Result<usize, TransportError> {
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }

        let (device_id, frequency) = match address {
            PeerAddress::Lora { device_id, frequency } => (*device_id, *frequency),
            _ => return Err(TransportError::InvalidAddress("Expected LoRa address".to_string())),
        };

        // Check payload size
        let modulation = LoraModulation::new(
            self.config.spreading_factor,
            self.config.bandwidth,
            self.config.coding_rate,
        );
        if data.len() > modulation.max_payload_size() {
            return Err(TransportError::PayloadTooLarge);
        }

        // Check duty cycle
        if !self.can_transmit() {
            return Err(TransportError::LoraChannelBusy);
        }

        // Create header
        let header = LoraMeshHeader::new(self.config.device_id, device_id, 0, 0);
        let header_bytes = header.to_bytes();

        // In a real implementation, this would:
        // 1. Set frequency if different
        // 2. Transmit header + data
        // 3. Wait for TX done

        self.stats.packets_sent += 1;
        self.stats.bytes_sent += data.len() as u64;
        self.last_tx_time = Some(Self::now());

        Ok(data.len())
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Check channel activity detection
    pub async fn check_channel_activity(&mut self) -> Result<bool, TransportError> {
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }
        // In a real implementation, this would perform CAD
        Ok(false) // Channel is clear
    }

    /// Time until next transmit is allowed (duty cycle)
    pub fn time_until_transmit_ms(&self) -> u64 {
        if let Some(last_tx) = self.last_tx_time {
            let now = Self::now();
            let elapsed = now.saturating_sub(last_tx);
            let required_wait = (1000.0 / self.config.duty_cycle_percent * 100.0) as u64;
            required_wait.saturating_sub(elapsed)
        } else {
            0
        }
    }

    /// Check if can transmit now
    pub fn can_transmit(&self) -> bool {
        self.time_until_transmit_ms() == 0
    }

    /// Get last RSSI reading
    pub fn last_rssi(&self) -> Option<i16> {
        self.last_rssi
    }

    /// Get last SNR reading
    pub fn last_snr(&self) -> Option<f32> {
        self.last_snr
    }

    /// Get battery voltage (if supported)
    pub fn battery_voltage(&self) -> Option<f32> {
        // Platform-specific implementation
        None
    }
}

impl Transport for LoraTransport {
    async fn start(&mut self) -> Result<(), TransportError> {
        if self.state.is_running() {
            return Err(TransportError::AlreadyRunning);
        }

        self.state = TransportState::Starting;

        // In a real implementation, this would:
        // 1. Initialize SPI
        // 2. Reset radio
        // 3. Configure registers
        // 4. Enter standby mode

        self.state = TransportState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), TransportError> {
        if !self.state.is_running() && !matches!(self.state, TransportState::Stopped) {
            return Err(TransportError::NotRunning);
        }

        self.state = TransportState::Stopping;
        self.is_receiving = false;
        self.is_sleeping = false;
        self.connections.clear();
        self.state = TransportState::Stopped;

        Ok(())
    }

    async fn connect(&mut self, address: PeerAddress) -> Result<ConnectionId, TransportError> {
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }

        if !address.is_lora() {
            return Err(TransportError::InvalidAddress("Expected LoRa address".to_string()));
        }

        // LoRa is connectionless, but we track "peers" for convenience
        let mut info = ConnectionInfo::new(address.clone());
        let conn_id = info.id().clone();

        self.connections.insert(conn_id.clone(), info);
        self.stats.connections_active = self.connections.len() as u32;

        Ok(conn_id)
    }

    async fn disconnect(&mut self, connection_id: &ConnectionId) -> Result<(), TransportError> {
        if self.connections.remove(connection_id).is_none() {
            return Err(TransportError::NotConnected);
        }
        self.stats.connections_active = self.connections.len() as u32;
        Ok(())
    }

    async fn send(&mut self, connection_id: &ConnectionId, data: &[u8]) -> Result<usize, TransportError> {
        let address = self.connections.get(connection_id)
            .ok_or(TransportError::NotConnected)?
            .address()
            .clone();

        self.send_to(&address, data).await
    }

    async fn broadcast(&mut self, data: &[u8]) -> Result<u32, TransportError> {
        if !self.state.is_running() {
            return Err(TransportError::NotRunning);
        }

        let addr = PeerAddress::lora_broadcast(self.current_frequency);
        self.send_to(&addr, data).await?;

        Ok(1) // Broadcast is single transmission
    }

    async fn poll_events(&mut self) -> Vec<TransportEvent> {
        std::mem::take(&mut self.events)
    }

    fn state(&self) -> &TransportState {
        &self.state
    }

    fn local_address(&self) -> Option<PeerAddress> {
        if self.state.is_running() {
            Some(PeerAddress::lora(self.config.device_id, self.current_frequency))
        } else {
            None
        }
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
