# Chained HotStuff Consensus Analysis

Analysis of the 2-chain variant of the HotStuff consensus protocol, focusing on the implementation details found in `asonnino/hotstuff`.

## 🏗️ Architectural Components

### 1. Consensus Core (`consensus/src/core.rs`)
The state machine that handles high-level consensus logic.
- **2-Chain Rule:** Simplifies the standard 3-chain HotStuff by using a pipelined approach where every block carries a Quorum Certificate (QC) for the previous block.
- **View Management:** Handled via the `Synchronizer`, ensuring nodes stay in the same "view" or round.
- **Safety Rule:** A block is committed if it has a direct parent with a QC, and the current node has seen a QC for that parent.

### 2. Mempool & Batching (`mempool/src/batch_maker.rs`)
- High-efficiency transaction ingestion.
- **Batch Maker:** Aggregates transactions into batches before proposing them to the consensus layer.
- **Quorum Waiter:** Ensures that a batch is replicated across a quorum of nodes before being included in a proposal.

### 3. Networking & RPC (`node/src/node.rs`)
- Built on top of `tokio` for asynchronous task management.
- Uses `dalek` cryptography for VRF and threshold signature simulations.
- Implements a custom wire protocol for fast QC propagation.

## 🚀 Benchmarking Strategy
The implementation includes a Python-based benchmarking suite (`benchmark/`) designed to:
- Deploy nodes across multiple AWS instances (via `fabric` and `boto3`).
- Measure throughput (TPS) and latency under varying committee sizes and faults.
- Test "View Change" performance during leader timeouts.

## 🔬 Key Differences (vs. Production HotStuff)
- **Minimalist Design:** Optimized for easy modification and academic benchmarking.
- **2-Chain Pipeline:** Reduces complexity compared to the original HotStuff while maintaining safety and liveness.
- **Storage:** Uses `rocksdb` for persistent ledger state.
