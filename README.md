# p2pmesh

Offline-first P2P payment mesh with UTXO-based double-spend prevention

## Overview

p2pmesh is a decentralized, offline-first payment system that enables secure financial transactions in environments with limited or no internet connectivity. The system uses a mesh network architecture to facilitate peer-to-peer payments without requiring traditional network infrastructure.

## Features

- **Offline-First**: Works without internet connectivity using mesh networking
- **UTXO Model**: Prevents double-spending using Unspent Transaction Output model
- **Multiple Transport Protocols**: Supports TCP, Bluetooth Low Energy (BLE), and LoRa
- **Distributed Ledger**: Uses Conflict-free Replicated Data Types (CRDTs) for consistency
- **Secure Identity**: Implements Decentralized Identifiers (DIDs) with Ed25519 cryptography
- **Cross-Platform**: Rust core with Kotlin/Swift bindings via UniFFI
- **Microtransactions**: Efficient handling of small-value transactions

## Architecture

The system is composed of several key modules:

- **Identity**: Manages cryptographic keys and DIDs
- **IOU**: Handles payment packets (I Owe You) representing payment intents
- **Ledger**: Maintains distributed state and conflict detection
- **Storage**: Persistent storage for transaction history
- **Sync**: Gossip protocol for state synchronization
- **Transport**: Multiple communication protocols (TCP, BLE, LoRa)
- **Vault**: Tracks user balances and UTXOs
- **Gateway**: Bridges to external financial systems

## Getting Started

### Prerequisites

- Rust 1.70+
- Cargo

### Building

```bash
cargo build
```

### Running

```bash
cargo run
```

## Usage

The system can be used programmatically through the Rust API or via the UniFFI-generated bindings for Kotlin/Swift.

## UniFFI Bridge

The project includes a bridge crate that exposes the Rust core functionality to Kotlin and Swift applications:

```bash
cd bridge
cargo build
```

This generates the necessary bindings for mobile applications.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with Rust for memory safety and performance
- Uses libp2p for networking capabilities
- Implements CRDTs for distributed state management
- Features UTXO model similar to Bitcoin for preventing double-spending