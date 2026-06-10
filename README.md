# Protocol Engine Core

Multi-crate Rust workspace for distributed systems, BFT consensus protocols, and Ethereum protocol research.

## Workspace Architecture

### [networking/](networking/) — eBPF P2P Observer
- **Engine:** [Aya](https://aya-rs.dev/) eBPF loader (Linux kernel ≥ 5.8).
- **Core logic:** TCP flow tracking and RTT probing via kernel programs; userspace reads perf event maps.
- **Target:** Passive observation of Geth/Lighthouse p2p traffic without modifying node processes.

### [consensus/](consensus/) — HotStuff BFT
- **Engine:** 2-chain HotStuff state machine (`HotStuffNode`, `VoteCollector`).
- **Core logic:** Vote dedup, 2f+1 quorum formation, pipelined commit rule (lock → commit on consecutive QCs).
- **Tests:** 4-node cluster — honest views, partition, equivocating leader, fault injection (dropped votes, duplicates, replay, out-of-order delivery).

### [beacon-chain/](beacon-chain/) — Ethereum Beacon Chain
- **Engine:** Simplified beacon node — `BeaconState`, epoch transitions, attestation pool.
- **Core logic:** Slot and epoch advancement, LMD-GHOST fork choice, FOCIL (EIP-7805) inclusion list enforcement.
- **Binary:** `beacon-slot-clock` — drives state forward on a Tokio interval; configurable via `SLOT_MS` / `MAX_SLOTS`.

### [messaging/](messaging/) — Pub/Sub Broker
- **Engine:** Tokio-native pub/sub with topic filtering.
- **Core logic:** `Broker` fan-out, `mpsc`-backed subscriber channels, high-throughput topic matching.

### [orderflow/](orderflow/) — EIP-1559 Block Builder
- **Engine:** Priority mempool + JSON-RPC client + block builder pipeline.
- **Core logic:** `BTreeMap`-ordered mempool by effective tip, `EthRpcClient` for `eth_getBlockByNumber` / `txpool_content`, greedy gas-fill builder.
- **Tests:** Wiremock mock-server tests for base fee parsing, EIP-1559/legacy tx ingestion, RPC error propagation.
- **Binary:** `block-builder` — fetches live pending txs, builds block, prints summary. Configurable via `ETHEREUM_RPC` / `TX_LIMIT` / `GAS_LIMIT`.

### [crates/engine-sync/](crates/engine-sync/) — Engine API Sync
- **Engine:** Simulates the consensus↔execution Engine API handshake.
- **Core logic:** `forkchoiceUpdated` / `newPayload` message sequencing over Tokio channels.

## Repository Structure

```
protocol-engine-core/
├── Cargo.toml                          # workspace manifest
│
├── networking/                         # eBPF P2P observer
│   └── src/
│       ├── main.rs                     # eBPF loader entry point
│       ├── flow_tracker.rs             # TCP flow state machine
│       ├── peer_monitor.rs             # peer connection tracking
│       ├── metrics.rs                  # RTT / bandwidth counters
│       └── lib.rs
│
├── consensus/                          # HotStuff BFT
│   ├── src/
│   │   ├── block.rs                    # Block, BlockId
│   │   ├── node.rs                     # HotStuffNode state machine
│   │   ├── vote.rs                     # Vote, VoteCollector, QuorumCertificate
│   │   ├── network.rs                  # async Tokio channel simulation
│   │   └── lib.rs
│   ├── tests/
│   │   ├── hotstuff_sim.rs             # honest multi-view simulation
│   │   ├── consensus_proptest.rs       # property-based fuzz (proptest)
│   │   ├── partition_test.rs           # Byzantine / network partition
│   │   └── fault_injection_test.rs     # dropped votes, replay, out-of-order
│   └── benches/
│       └── hotstuff_bench.rs           # criterion throughput benchmark
│
├── beacon-chain/                       # Ethereum beacon node (simplified)
│   ├── src/
│   │   ├── state.rs                    # BeaconState, Validator, slot/epoch logic
│   │   ├── epoch.rs                    # process_epoch (rewards, finality)
│   │   ├── attestation.rs              # AttestationPool
│   │   ├── fork_choice.rs              # LMD-GHOST
│   │   ├── focil.rs                    # EIP-7805 inclusion list enforcement
│   │   ├── bin/slot_clock.rs           # runnable: beacon-slot-clock binary
│   │   └── lib.rs
│   └── tests/
│       ├── beacon_tests.rs             # state transitions, epoch boundary
│       └── focil_tests.rs              # ILAggregator, ILEnforcer
│
├── messaging/                          # pub/sub broker
│   ├── src/
│   │   ├── broker.rs                   # Broker fan-out
│   │   ├── filter.rs                   # topic filter / matcher
│   │   └── lib.rs
│   └── tests/
│       └── broker_tests.rs
│
├── orderflow/                          # EIP-1559 block builder
│   ├── src/
│   │   ├── mempool.rs                  # BTreeMap priority mempool
│   │   ├── builder.rs                  # greedy gas-fill block builder
│   │   ├── rpc.rs                      # EthRpcClient (JSON-RPC)
│   │   ├── bin/block_builder.rs        # runnable: block-builder binary
│   │   └── lib.rs
│   ├── tests/
│   │   ├── orderflow_tests.rs          # mempool unit tests
│   │   └── rpc_tests.rs                # wiremock mock-server RPC tests
│   └── benches/
│       └── mempool_bench.rs            # criterion insert/drain benchmark
│
└── crates/
    └── engine-sync/                    # Engine API handshake simulation
        ├── src/lib.rs                  # forkchoiceUpdated / newPayload logic
        └── tests/sync_tests.rs
```

## Getting Started

```bash
# Build workspace
cargo build --release

# Run all tests
cargo test --workspace

# Beacon slot clock (simulated 12-second slots, 2 epochs)
MAX_SLOTS=64 SLOT_MS=12000 cargo run -p beacon-chain --bin beacon-slot-clock

# Block builder (requires Ethereum RPC endpoint)
ETHEREUM_RPC=https://holesky.drpc.org TX_LIMIT=500 cargo run -p orderflow --bin block-builder

# eBPF networking tracker (requires Linux + root)
sudo cargo run -p networking --bin networking --release
```

## Crate Test Count

| Crate | Tests |
|-------|-------|
| consensus | 14 (sim + proptest + partition + fault injection) |
| beacon-chain | 9 (state, focil, epoch) |
| orderflow | 9 (mempool) + 8 (rpc wiremock) |
| messaging | 5 |
| engine-sync | 4 |
| networking | 3 |
