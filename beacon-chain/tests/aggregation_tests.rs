use beacon_chain::attestation::{AggregateAttestation, Attestation, AttestationPool, Checkpoint};
use beacon_chain::fork_choice::ForkChoice;

fn checkpoint(epoch: u64, root: [u8; 32]) -> Checkpoint {
    Checkpoint { epoch, root }
}

fn att(slot: u64, validator: u64, block_root: [u8; 32], target: [u8; 32]) -> Attestation {
    let cp = checkpoint(slot / 32, target);
    Attestation {
        slot,
        validator_index: validator,
        beacon_block_root: block_root,
        source: cp.clone(),
        target: cp,
    }
}

// ── aggregate ─────────────────────────────────────────────────────────────────

#[test]
fn test_aggregate_collects_all_validators() {
    let root = [1u8; 32];
    let target = [0u8; 32];
    let mut pool = AttestationPool::new();

    for v in 0..8u64 {
        pool.add(att(1, v, root, target));
    }

    let agg = pool.aggregate(1, target);
    assert_eq!(agg.vote_count, 8);
    assert_eq!(agg.participating_validators.len(), 8);
}

#[test]
fn test_aggregate_deduplicates_same_validator() {
    let root = [1u8; 32];
    let target = [0u8; 32];
    let mut pool = AttestationPool::new();

    // validator 0 adds two attestations for same slot (only latest kept by pool)
    pool.add(att(1, 0, root, target));
    pool.add(att(1, 0, root, target)); // same slot — no-op (not newer)
    pool.add(att(1, 1, root, target));

    let agg = pool.aggregate(1, target);
    assert_eq!(agg.vote_count, 2, "duplicate validator must not inflate count");
}

#[test]
fn test_aggregate_only_matching_slot_and_target() {
    let root = [1u8; 32];
    let target_a = [0xaa; 32];
    let target_b = [0xbb; 32];
    let mut pool = AttestationPool::new();

    pool.add(att(1, 0, root, target_a));
    pool.add(att(1, 1, root, target_b)); // different target
    pool.add(att(2, 2, root, target_a)); // different slot

    let agg = pool.aggregate(1, target_a);
    assert_eq!(agg.vote_count, 1, "only (slot=1, target_a) must match");
    assert_eq!(agg.participating_validators, vec![0]);
}

#[test]
fn test_aggregate_empty_result_no_matches() {
    let pool = AttestationPool::new();
    let agg = pool.aggregate(99, [0u8; 32]);
    assert_eq!(agg.vote_count, 0);
    assert!(agg.participating_validators.is_empty());
}

#[test]
fn test_aggregate_validators_sorted() {
    let root = [1u8; 32];
    let target = [0u8; 32];
    let mut pool = AttestationPool::new();

    // add in reverse order
    for v in (0..5u64).rev() {
        pool.add(att(1, v, root, target));
    }

    let agg = pool.aggregate(1, target);
    let sorted = agg.participating_validators.windows(2).all(|w| w[0] < w[1]);
    assert!(sorted, "participating_validators must be sorted ascending");
}

#[test]
fn test_aggregate_includes_check() {
    let root = [1u8; 32];
    let target = [0u8; 32];
    let mut pool = AttestationPool::new();
    pool.add(att(1, 3, root, target));
    pool.add(att(1, 7, root, target));

    let agg = pool.aggregate(1, target);
    assert!(agg.includes(3));
    assert!(agg.includes(7));
    assert!(!agg.includes(0));
    assert!(!agg.includes(99));
}

// ── aggregate_vote_weight (dedup semantics) ───────────────────────────────────

#[test]
fn test_aggregate_vote_weight_matches_distinct_validators() {
    let root = [0xAA; 32];
    let target = [0u8; 32];
    let mut pool = AttestationPool::new();

    // 3 distinct validators vote for root
    for v in 0..3u64 {
        pool.add(att(1, v, root, target));
    }
    // validator 0 updates to newer slot, same root
    pool.add(att(2, 0, root, target));

    // vote_weight and aggregate_vote_weight must agree
    assert_eq!(pool.vote_weight(&root), pool.aggregate_vote_weight(&root));
    assert_eq!(pool.aggregate_vote_weight(&root), 3);
}

// ── fork choice uses aggregate weight ────────────────────────────────────────

#[test]
fn test_fork_choice_picks_heavier_fork_via_aggregate() {
    let genesis = [0u8; 32];
    let fork_a = [0x0a; 32]; // 2 validators
    let fork_b = [0x0b; 32]; // 3 validators — should win

    let mut fc = ForkChoice::new(genesis);
    fc.on_block(fork_a, genesis);
    fc.on_block(fork_b, genesis);

    let target = genesis;

    for v in 0..2u64 {
        fc.pool.add(att(1, v, fork_a, target));
    }
    for v in 2..5u64 {
        fc.pool.add(att(1, v, fork_b, target));
    }

    assert_eq!(fc.head(), fork_b, "heavier fork (3 validators) must win");
}

#[test]
fn test_fork_choice_dedup_prevents_fake_weight() {
    let genesis = [0u8; 32];
    let fork_a = [0x0a; 32];
    let fork_b = [0x0b; 32];

    let mut fc = ForkChoice::new(genesis);
    fc.on_block(fork_a, genesis);
    fc.on_block(fork_b, genesis);

    let target = genesis;
    // fork_a: 1 real validator, submitted 3 times (later slots override)
    fc.pool.add(att(1, 0, fork_a, target));
    fc.pool.add(att(2, 0, fork_a, target)); // overrides slot 1
    fc.pool.add(att(3, 0, fork_a, target)); // overrides slot 2

    // fork_b: 2 distinct validators
    fc.pool.add(att(1, 1, fork_b, target));
    fc.pool.add(att(1, 2, fork_b, target));

    // fork_a weight must be 1 (one validator, not 3)
    assert_eq!(fc.pool.aggregate_vote_weight(&fork_a), 1);
    assert_eq!(fc.pool.aggregate_vote_weight(&fork_b), 2);
    assert_eq!(fc.head(), fork_b, "fork_b must win (2 > 1 real validators)");
}
