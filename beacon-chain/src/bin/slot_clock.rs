/// Beacon slot clock: drives BeaconState forward on a real Tokio interval.
/// Simulates Ethereum's 12-second slot with configurable tick rate for testing.
use beacon_chain::{
    epoch::process_epoch,
    focil::{ILAggregator, ILEnforcer, InclusionList},
    state::{BeaconState, Validator, SLOTS_PER_EPOCH},
};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone)]
pub enum SlotEvent {
    SlotStart { slot: u64, epoch: u64, proposer: u64 },
    EpochBoundary { epoch: u64, finalized: u64, justified: u64 },
    ILQuorumReached { slot: u64 },
}

pub struct SlotClock {
    state: BeaconState,
    il_agg: ILAggregator,
    il_enforcer: ILEnforcer,
    slot_duration: Duration,
    event_tx: mpsc::Sender<SlotEvent>,
}

impl SlotClock {
    pub fn new(
        validators: usize,
        slot_duration: Duration,
        event_tx: mpsc::Sender<SlotEvent>,
    ) -> Self {
        let vals: Vec<Validator> = (0..validators as u8)
            .map(|i| Validator::new(vec![i; 48], 32_000_000_000))
            .collect();
        Self {
            state: BeaconState::genesis(vals),
            il_agg: ILAggregator::new(validators),
            il_enforcer: ILEnforcer::new(30_000_000),
            slot_duration,
            event_tx,
        }
    }

    pub async fn run(&mut self, max_slots: u64) {
        let mut interval = tokio::time::interval(self.slot_duration);

        for _ in 0..max_slots {
            interval.tick().await;
            self.tick().await;
        }
    }

    async fn tick(&mut self) {
        self.state.advance_slot();
        let slot = self.state.slot;
        let epoch = self.state.current_epoch();
        let proposer = self.state.proposer_index();

        info!(slot, epoch, proposer, "slot start");
        let _ = self.event_tx.send(SlotEvent::SlotStart { slot, epoch, proposer }).await;

        // simulate: each validator submits a minimal IL for this slot
        let n = self.state.validators.len();
        for vid in 0..n as u64 {
            let hash = {
                let mut h = [0u8; 32];
                h[0] = (slot % 256) as u8;
                h[1] = vid as u8;
                h
            };
            self.il_agg.add(InclusionList {
                slot,
                validator_index: vid,
                tx_hashes: vec![hash],
            });
        }

        if self.il_agg.has_quorum(slot) {
            info!(slot, "IL quorum reached");
            let _ = self.event_tx.send(SlotEvent::ILQuorumReached { slot }).await;
        }

        // epoch boundary
        if slot % SLOTS_PER_EPOCH == 0 && slot > 0 {
            process_epoch(&mut self.state);
            let event = SlotEvent::EpochBoundary {
                epoch,
                finalized: self.state.finalized_checkpoint_epoch,
                justified: self.state.justified_checkpoint_epoch,
            };
            info!(
                epoch,
                finalized = self.state.finalized_checkpoint_epoch,
                justified = self.state.justified_checkpoint_epoch,
                "epoch boundary"
            );
            let _ = self.event_tx.send(event).await;
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    // Default: 12s slots (real Ethereum timing). Override with SLOT_MS env var.
    let slot_ms: u64 = std::env::var("SLOT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12_000);

    let max_slots: u64 = std::env::var("MAX_SLOTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(64); // 2 epochs

    info!(slot_ms, max_slots, "beacon slot clock starting");

    let (tx, mut rx) = mpsc::channel(64);
    let mut clock = SlotClock::new(16, Duration::from_millis(slot_ms), tx);

    let printer = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match &event {
                SlotEvent::EpochBoundary { epoch, finalized, justified } => {
                    println!("EPOCH {epoch} | justified={justified} finalized={finalized}");
                }
                SlotEvent::ILQuorumReached { slot } => {
                    println!("SLOT {slot} | IL quorum reached");
                }
                _ => {}
            }
        }
    });

    clock.run(max_slots).await;
    drop(clock);
    let _ = printer.await;
    info!("slot clock done");
}
