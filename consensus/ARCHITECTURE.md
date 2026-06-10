# Consensus — HotStuff BFT

## Design

2-chain HotStuff variant. Safety threshold: `2f+1` votes where `f = (n-1)/3`.

```
Producer          Leader            Replicas
   │                │                  │
   │   Proposal     │                  │
   │◄───────────────│                  │
   │                │    Proposal      │
   │                │─────────────────►│
   │                │◄─────────────────│ Vote
   │                │  (2f+1 votes)    │
   │                │   QC formed      │
   │                │─────────────────►│ NewView(QC)
   │                │    advance_view  │
```

## Key invariants

| Invariant | Where enforced |
|-----------|----------------|
| Only vote if block extends `locked_qc` | `node::on_proposal` safety rule |
| QC requires exactly `2f+1` distinct signers | `vote::VoteCollector` dedup check |
| Commit only on 2-chain (consecutive QCs) | `node::advance_view` |
| `high_qc` propagates to all nodes on `advance_view` | prevents leader proposing stale fork |

## Modules

| File | Role |
|------|------|
| `block.rs` | `Block` + `BlockId` (SHA-256 hash of view+parent+payload) |
| `vote.rs` | `Vote`, `QuorumCertificate`, `VoteCollector` |
| `node.rs` | `HotStuffNode` — full state machine |
| `network.rs` | Async Tokio-channel simulation (`NetworkNode`, `simulate`) |

## Test coverage

- 4-node 2-view commit (unit)
- 2f+1 quorum threshold (unit)
- Safety rule rejects conflicting block (unit)
- 4 proptest invariants (committed consistency, liveness, view monotone, no dup signers)
- 4 partition tests (silent node, equivocating leader, 2+2 split, heal)
