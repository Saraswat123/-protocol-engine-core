use beacon_chain::{
    attestation::{Attestation, AttestationPool, Checkpoint},
    epoch::advance_epoch,
    fork_choice::ForkChoice,
    state::{BeaconState, Validator, SLOTS_PER_EPOCH},
};

fn make_validator(id: u8) -> Validator {
    Validator::new(vec![id; 48], 32_000_000_000)
}

fn make_state(n: usize) -> BeaconState {
    let validators = (0..n as u8).map(make_validator).collect();
    BeaconState::genesis(validators)
}

// ── state ────────────────────────────────────────────────────────────────────

#[test]
fn test_genesis_state() {
    let state = make_state(4);
    assert_eq!(state.slot, 0);
    assert_eq!(state.current_epoch(), 0);
    assert_eq!(state.active_validator_count(), 4);
}

#[test]
fn test_slot_and_epoch_advance() {
    let mut state = make_state(4);
    for _ in 0..SLOTS_PER_EPOCH {
        state.advance_slot();
    }
    assert_eq!(state.slot, SLOTS_PER_EPOCH);
    assert_eq!(state.current_epoch(), 1);
}

#[test]
fn test_proposer_rotates_each_slot() {
    let state = make_state(4);
    let mut proposers: Vec<u64> = (0..8)
        .map(|slot| {
            let mut s = state.clone();
            s.slot = slot;
            s.proposer_index()
        })
        .collect();
    proposers.dedup();
    // at least 2 distinct proposers across 8 slots
    assert!(proposers.len() >= 2);
}

#[test]
fn test_slashed_validator_not_active() {
    let mut state = make_state(4);
    state.validators[0].slashed = true;
    assert_eq!(state.active_validator_count(), 3);
}

// ── attestation pool ──────────────────────────────────────────────────────────

#[test]
fn test_attestation_pool_vote_weight() {
    let root = [1u8; 32];
    let other = [2u8; 32];
    let checkpoint = Checkpoint { epoch: 0, root: [0u8; 32] };

    let mut pool = AttestationPool::new();
    for i in 0..3u64 {
        pool.add(Attestation {
            slot: 1,
            validator_index: i,
            beacon_block_root: root,
            source: checkpoint.clone(),
            target: checkpoint.clone(),
        });
    }
    pool.add(Attestation {
        slot: 1,
        validator_index: 3,
        beacon_block_root: other,
        source: checkpoint.clone(),
        target: checkpoint.clone(),
    });

    assert_eq!(pool.vote_weight(&root), 3);
    assert_eq!(pool.vote_weight(&other), 1);
}

#[test]
fn test_attestation_pool_latest_wins() {
    let old_root = [1u8; 32];
    let new_root = [2u8; 32];
    let checkpoint = Checkpoint { epoch: 0, root: [0u8; 32] };

    let mut pool = AttestationPool::new();
    pool.add(Attestation {
        slot: 1,
        validator_index: 0,
        beacon_block_root: old_root,
        source: checkpoint.clone(),
        target: checkpoint.clone(),
    });
    // same validator, newer slot → replaces old
    pool.add(Attestation {
        slot: 2,
        validator_index: 0,
        beacon_block_root: new_root,
        source: checkpoint.clone(),
        target: checkpoint.clone(),
    });

    assert_eq!(pool.vote_weight(&new_root), 1);
    assert_eq!(pool.vote_weight(&old_root), 0);
}

// ── fork choice (LMD-GHOST) ───────────────────────────────────────────────────

#[test]
fn test_fork_choice_single_chain() {
    let genesis = [0u8; 32];
    let block_a = [1u8; 32];
    let block_b = [2u8; 32];

    let mut fc = ForkChoice::new(genesis);
    fc.on_block(block_a, genesis);
    fc.on_block(block_b, block_a);

    // no votes: head = genesis (no children of justified with votes)
    // with votes pointing to block_b:
    let checkpoint = Checkpoint { epoch: 0, root: genesis };
    fc.pool.add(Attestation {
        slot: 1,
        validator_index: 0,
        beacon_block_root: block_b,
        source: checkpoint.clone(),
        target: checkpoint.clone(),
    });

    assert_eq!(fc.head(), block_b);
}

#[test]
fn test_fork_choice_picks_heavier_fork() {
    let genesis = [0u8; 32];
    let fork_a = [1u8; 32]; // 1 vote
    let fork_b = [2u8; 32]; // 2 votes — should win

    let mut fc = ForkChoice::new(genesis);
    fc.on_block(fork_a, genesis);
    fc.on_block(fork_b, genesis);

    let checkpoint = Checkpoint { epoch: 0, root: genesis };
    fc.pool.add(Attestation {
        slot: 1,
        validator_index: 0,
        beacon_block_root: fork_a,
        source: checkpoint.clone(),
        target: checkpoint.clone(),
    });
    for i in 1..3u64 {
        fc.pool.add(Attestation {
            slot: 1,
            validator_index: i,
            beacon_block_root: fork_b,
            source: checkpoint.clone(),
            target: checkpoint.clone(),
        });
    }

    assert_eq!(fc.head(), fork_b);
}

// ── epoch transition ──────────────────────────────────────────────────────────

#[test]
fn test_epoch_justification() {
    let mut state = make_state(4);
    advance_epoch(&mut state);
    assert_eq!(state.current_epoch(), 1);
    assert_eq!(state.justified_checkpoint_epoch, 1);
}

#[test]
fn test_epoch_finalization_after_two_epochs() {
    let mut state = make_state(4);
    advance_epoch(&mut state); // epoch 1: justified
    advance_epoch(&mut state); // epoch 2: justified, epoch 1 finalized
    assert_eq!(state.finalized_checkpoint_epoch, 1);
}
