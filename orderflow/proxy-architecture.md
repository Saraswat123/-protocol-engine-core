# FlowProxy: Orderflow Ingestion & Multiplexing

Architectural breakdown of `BuilderNet/FlowProxy`, a high-performance engine for MEV orderflow management.

## 🏗️ Technical Architecture

### 1. Ingress Layer (`src/ingress/`)
- Handles incoming JSON-RPC requests from searchers and users.
- Implements efficient validation logic to filter out malformed or invalid transactions before they enter the pipeline.

### 2. Priority & Forwarding (`src/priority/`, `src/forwarder/`)
- **Priority Queueing:** Implements sophisticated worker patterns and priority channels to manage high-volume orderflow.
- **Multiplexing:** Capable of proxying orderflow to multiple downstream builder hubs simultaneously.
- **Transport Support:** Native support for both HTTP and TCP forwarding to minimize overhead.

### 3. Caching & Indexing (`src/cache.rs`, `src/indexer/`)
- **State Caching:** High-speed in-memory cache for transaction tracking.
- **Persistent Indexing:** Specialized indexers for ClickHouse and Parquet to enable long-term analytical study of orderflow patterns.

## 🚀 Performance Optimization
- **Rate Limiting:** Granular control over request rates per searcher/IP to prevent DoS attacks on builder infra.
- **Latency Monitoring:** Integrated metrics tracking at each stage of the ingestion pipeline.
- **Async Runtime:** Fully leverages **Tokio** for massive concurrency.

## 🔬 MEV Infrastructure Role
FlowProxy acts as the "front door" for block builders, ensuring that:
- Orderflow is ingested with minimal latency.
- Valid transactions are prioritized and multiplexed to the right execution engines.
- Data is logged and indexed for post-block performance analysis.
