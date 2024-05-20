# Ray: Ethereum Beacon Node Study

Technical study of `ackintosh/ray`, an educational Ethereum Beacon Node implementation designed for protocol clarity.

## 🏗️ Technical Architecture

### 1. P2P & Networking (`src/discovery/`, `src/network.rs`)
- **Discovery (discv5):** Implements Node Discovery Protocol v5 for finding peers in the Ethereum network.
- **Identity:** Manages ENR (Ethereum Node Records) and cryptographic identities for network participation.
- **Libp2p Integration:** Uses `rust-libp2p` for Gossipsub and Request-Response protocols.

### 2. Synchronization & Chain Management (`src/sync/`)
- **Range Sync:** Implements block range requests to catch up with the latest head.
- **Chain Collection:** Manages the canonical chain and forks within the Beacon state.
- **Syncing Chain:** Logic for determining the best chain head and handling re-orgs.

### 3. RPC & Protocol Handling (`src/rpc/`)
- **Ethereum Wire Protocol:** Implementation of the SSZ-encoded RPC methods for Beacon Node communication (e.g., Status, Goodbye, BeaconBlocksByRange).
- **Behaviour:** High-level libp2p behaviour management for coordinating different network tasks.

## 🚀 Educational Goals
- **Clarity:** Provides a simplified view of the complex Beacon Chain specifications (Phase 0).
- **Rust Implementation:** Demonstrates idiomatic Rust patterns for protocol development (e.g., using `tokio` for async and `libp2p` for P2P).
- **Simulation:** Ideal for testing consensus logic and peer-to-peer behavior in a controlled environment.

## 🔬 Comparison with Production Clients (Lighthouse/Prysm)
- **Scale:** While production clients are optimized for massive mainnet loads, Ray focus on modularity and readability.
- **Feature Set:** Implements the core consensus logic required for network participation without the extensive resource management needed for validator-heavy nodes.
