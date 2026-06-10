use beacon_chain::focil::{AggregateIL, ILAggregator, ILEnforcer, ILVerdict, InclusionList};

fn il(slot: u64, validator: u64, hashes: Vec<[u8; 32]>) -> InclusionList {
    InclusionList { slot, validator_index: validator, tx_hashes: hashes }
}

fn h(b: u8) -> [u8; 32] { [b; 32] }

// ── aggregator ────────────────────────────────────────────────────────────────

#[test]
fn test_aggregate_union_of_all_ils() {
    let mut agg = ILAggregator::new(4);
    agg.add(il(1, 0, vec![h(1), h(2)]));
    agg.add(il(1, 1, vec![h(2), h(3)])); // h(2) duplicate → deduped
    agg.add(il(1, 2, vec![h(4)]));

    let result = agg.aggregate(1);
    assert_eq!(result.tx_hashes.len(), 4);
    assert!(result.tx_hashes.contains(&h(1)));
    assert!(result.tx_hashes.contains(&h(4)));
}

#[test]
fn test_aggregate_empty_slot() {
    let agg = ILAggregator::new(4);
    let result = agg.aggregate(99);
    assert_eq!(result.tx_hashes.len(), 0);
    assert_eq!(result.contributor_count, 0);
}

#[test]
fn test_quorum_detection() {
    let mut agg = ILAggregator::new(4); // quorum = 4/2+1 = 3
    assert!(!agg.has_quorum(1));
    agg.add(il(1, 0, vec![]));
    agg.add(il(1, 1, vec![]));
    assert!(!agg.has_quorum(1));
    agg.add(il(1, 2, vec![]));
    assert!(agg.has_quorum(1));
}

#[test]
fn test_same_validator_il_deduplicated() {
    let mut agg = ILAggregator::new(4);
    agg.add(il(1, 0, vec![h(1)]));
    agg.add(il(1, 0, vec![h(2)])); // same validator again
    // only one entry per validator
    let result = agg.aggregate(1);
    assert_eq!(result.contributor_count, 1);
}

// ── enforcer ─────────────────────────────────────────────────────────────────

#[test]
fn test_il_satisfied() {
    let enforcer = ILEnforcer::new(30_000_000);
    let aggregate = AggregateIL {
        slot: 1,
        tx_hashes: [h(1), h(2), h(3)].into_iter().collect(),
        contributor_count: 3,
    };
    let block_txs = vec![h(1), h(2), h(3), h(4)]; // superset
    assert_eq!(
        enforcer.verify(&aggregate, &block_txs, 15_000_000),
        ILVerdict::Satisfied
    );
}

#[test]
fn test_il_violated_censorship() {
    let enforcer = ILEnforcer::new(30_000_000);
    let aggregate = AggregateIL {
        slot: 1,
        tx_hashes: [h(1), h(2), h(3)].into_iter().collect(),
        contributor_count: 3,
    };
    let block_txs = vec![h(1)]; // missing h(2) and h(3)
    match enforcer.verify(&aggregate, &block_txs, 5_000_000) {
        ILVerdict::Violated { missing } => {
            assert_eq!(missing.len(), 2);
        }
        other => panic!("expected Violated, got {other:?}"),
    }
}

#[test]
fn test_il_exempt_block_full() {
    let gas_limit = 30_000_000u64;
    let enforcer = ILEnforcer::new(gas_limit);
    let aggregate = AggregateIL {
        slot: 1,
        tx_hashes: [h(1), h(2)].into_iter().collect(),
        contributor_count: 2,
    };
    let block_txs = vec![]; // missing everything, but block is 95% full
    let verdict = enforcer.verify(&aggregate, &block_txs, (gas_limit as f64 * 0.95) as u64);
    assert!(
        matches!(verdict, ILVerdict::ExemptBlockFull { .. }),
        "full block must be exempt from IL enforcement"
    );
}

#[test]
fn test_empty_il_always_satisfied() {
    let enforcer = ILEnforcer::new(30_000_000);
    let aggregate = AggregateIL {
        slot: 1,
        tx_hashes: Default::default(),
        contributor_count: 0,
    };
    assert_eq!(
        enforcer.verify(&aggregate, &[], 0),
        ILVerdict::Satisfied
    );
}
