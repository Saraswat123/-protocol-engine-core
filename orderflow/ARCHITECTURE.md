# Orderflow

## Design

EIP-1559 mempool + greedy block builder + live RPC fetcher.

```
Ethereum node (Geth/Reth)
        │  txpool_content (JSON-RPC)
        ▼
  EthRpcClient (rpc.rs)
        │  Vec<Transaction>
        ▼
  Mempool (BTreeMap, tip-sorted)
        │  top(N)
        ▼
  Builder.build(pool, base_fee)
        │
        ▼
  Block { txs, gas_used, base_fee }
        │
  Builder::next_base_fee()  →  next block's base fee
```

## Modules

| File | Role |
|------|------|
| `mempool.rs` | `Mempool` — BTreeMap keyed by `(Reverse(tip), nonce, hash)` |
| `builder.rs` | `Builder` — greedy gas-aware block construction + EIP-1559 base fee calc |
| `rpc.rs` | `EthRpcClient` — JSON-RPC `txpool_content` + `eth_getBlockByNumber` |

## Ordering key

```
effective_tip = min(max_fee_per_gas - base_fee, gas_price)
key = (Reverse(tip), nonce, hash)   // highest tip first; tie-break by nonce
```

`update_base_fee()` re-sorts entire pool because tips change relative to new base fee.

## RPC usage

```rust
let client = EthRpcClient::new("http://localhost:8545");
let base_fee = client.base_fee().await?;
let txs = client.pending_txs(500).await?;
let mut pool = Mempool::new(base_fee);
for tx in txs { pool.insert(tx); }
let block = Builder::new(block_num, 30_000_000).build(&pool, base_fee);
```

Works with any Geth/Reth/Erigon node exposing `txpool_content`. Holesky/Sepolia endpoints work out of the box.
