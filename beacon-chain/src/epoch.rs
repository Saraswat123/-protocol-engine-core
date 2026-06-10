use crate::state::{BeaconState, SLOTS_PER_EPOCH};
use tracing::info;

/// Process end-of-epoch transitions: update justified/finalized checkpoints.
pub fn process_epoch(state: &mut BeaconState) {
    let epoch = state.current_epoch();

    if epoch > state.justified_checkpoint_epoch {
        state.justified_checkpoint_epoch = epoch;
        info!(epoch, "checkpoint justified");
    }

    if state.justified_checkpoint_epoch > 0
        && state.justified_checkpoint_epoch == epoch
        && epoch > state.finalized_checkpoint_epoch + 1
    {
        state.finalized_checkpoint_epoch = epoch - 1;
        info!(finalized_epoch = state.finalized_checkpoint_epoch, "checkpoint finalized");
    }
}

/// Advance state by one full epoch (SLOTS_PER_EPOCH slots).
pub fn advance_epoch(state: &mut BeaconState) {
    for _ in 0..SLOTS_PER_EPOCH {
        state.advance_slot();
    }
    process_epoch(state);
}
