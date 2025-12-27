// Transport module - THE WIRE (abstract)
// Provides abstract transport layer for TCP, BLE, and LoRa communications

mod traits;
mod tcp;
mod ble;
mod lora;

pub use traits::{
    // Core trait
    Transport,
    // Configuration
    TransportConfig,
    // Connection types
    ConnectionId, ConnectionInfo, ConnectionState,
    // Address types
    PeerAddress,
    // Events and errors
    TransportEvent, TransportError, TransportState,
    // Statistics
    TransportStats,
};

pub use tcp::{TcpTransport, TcpTransportConfig};

pub use ble::{
    BleTransport, BleTransportConfig,
    BleService, BleCharacteristic,
};

pub use lora::{
    LoraTransport, LoraTransportConfig,
    LoraModulation, LoraSpreadingFactor, LoraBandwidth, LoraCodingRate,
    LoraMeshHeader,
};
