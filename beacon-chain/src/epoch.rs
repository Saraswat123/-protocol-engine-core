use crate::state::{BeaconState, SLOTS_PER_EPOCH};

/// Process end-of-epoch transitions: update justified/finalized checkpoints.
pub fn process_epoch(state: &mut BeaconState) {
    let epoch = state.current_epoch();

    // Justify current epoch if enough validators attested (simplified: always justify)
    if epoch > state.justified_checkpoint_epoch {
        state.justified_checkpoint_epoch = epoch;
    }

    // Finalize previous epoch once current is justified (1-epoch finality for simplicity)
    if state.justified_checkpoint_epoch > 0
        && state.justified_checkpoint_epoch == epoch
        && epoch > state.finalized_checkpoint_epoch + 1
    {
        state.finalized_checkpoint_epoch = epoch - 1;
    }
}

/// Advance state by one full epoch (SLOTS_PER_EPOCH slots).
pub fn advance_epoch(state: &mut BeaconState) {
    for _ in 0..SLOTS_PER_EPOCH {
        state.advance_slot();
    }
    process_epoch(state);
}
