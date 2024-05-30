# Protocol Engine Core

Advanced multi-crate Rust workspace for high-performance distributed systems, consensus protocols, and Ethereum research.

## 🛠️ Workspace Architecture

### 🔌 [networking/](networking/) — eBPF P2P Observation (2-3 weeks)
- **Engine:** [Aya](https://aya-rs.dev/) based eBPF loader.
- **Core logic:** TCP flow tracking and RTT latency probing at the kernel level.
- **Target:** Observing Geth/Lighthouse traffic without application overhead.

### 🤝 [consensus/](consensus/) — HotStuff Implementation (2 weeks)
- **Engine:** Custom 2-chain HotStuff state machine.
- **Core logic:** Vote collection, quorum verification (2f+1), and pipelined block chaining.
- **Tests:** 4-node simulation with network partition recovery scenarios.

### 🗼 [beacon-chain/](beacon-chain/) — Ethereum Consensus Study (1.5 weeks)
- **Engine:** Simplified Beacon Node implementation.
- **Core logic:** LMD-GHOST fork choice, epoch transition logic, and attestation management.

### 📡 [messaging/](messaging/) — Distributed Messaging (1 week)
- **Engine:** Port of `msg-rs` optimized for Tokio.
- **Core logic:** Pub/Sub broker with high-throughput topic filtering.

### 🏎️ [orderflow/](orderflow/) — MEV Builder Simulation (1 week)
- **Engine:** Block building and bundle ingestion pipeline.
- **Core logic:** Priority mempool based on gas-price ordering and FlowProxy routing.

## 🚀 Getting Started

```bash
# Build the entire workspace
cargo build --release

# Run eBPF networking tracker (requires Linux)
sudo ./target/release/networking --process geth
```

## 📊 Project Timeline (2026)
- **March:** Networking & eBPF Foundation.
- **April:** Consensus & Beacon Chain Logic.
- **May:** Messaging, Orderflow, and CI/CD.
- **June:** Performance Benchmarking & Integration.
