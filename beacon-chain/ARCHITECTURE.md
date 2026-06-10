# Beacon Chain

## Design

Simplified Ethereum consensus layer: slot/epoch state machine, LMD-GHOST fork choice, attestation pool, and FOCIL censorship resistance.

```
Slot N                 Epoch boundary
  │                       │
  ├─ proposer_index()     ├─ process_epoch()
  ├─ advance_slot()       │   ├─ justify checkpoint
  │                       │   └─ finalize epoch-1
  │  Attestations         │
  ├─ pool.add(att)        FOCIL (EIP-7805)
  └─ fork_choice.head()   ├─ ILAggregator.add(il)
                          ├─ aggregate(slot)
                          └─ ILEnforcer.verify(block)
```

## Modules

| File | Role |
|------|------|
| `state.rs` | `BeaconState`, `Validator` lifecycle, slot/epoch counters |
| `attestation.rs` | `AttestationPool` — latest-message tracking for LMD-GHOST |
| `fork_choice.rs` | LMD-GHOST: greedy child selection by vote weight |
| `epoch.rs` | Epoch transition: justification + finalization |
| `focil.rs` | EIP-7805 — IL collection, aggregation, block enforcement |

## FOCIL flow

```
Committee validators each submit InclusionList(slot, tx_hashes)
          │
    ILAggregator.add()
          │
    aggregate(slot) → AggregateIL (union of all tx_hashes)
          │
    ILEnforcer.verify(aggregate, block_txs, gas_used)
          ├─ Satisfied          — all IL txs present
          ├─ ExemptBlockFull    — block ≥ 90% full, proposer excused
          └─ Violated           — censorship detected
```

## Key design decisions

- **Latest-message only**: attestation pool keeps one entry per validator; older attestations for same validator are discarded. Matches LMD-GHOST spec.
- **Fork choice starts at justified root**: avoids re-scanning pre-finalized history.
- **FOCIL full-block threshold 90%**: simplified from EIP-7805's exact gas accounting; sufficient for testing the enforcement logic.
