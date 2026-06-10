use serde::{Deserialize, Serialize};
use tracing::{debug, info};

pub const SLOTS_PER_EPOCH: u64 = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    pub pubkey: Vec<u8>,
    pub effective_balance: u64,
    pub slashed: bool,
    pub activation_epoch: u64,
    pub exit_epoch: u64,
}

impl Validator {
    pub fn new(pubkey: Vec<u8>, balance: u64) -> Self {
        Self {
            pubkey,
            effective_balance: balance,
            slashed: false,
            activation_epoch: 0,
            exit_epoch: u64::MAX,
        }
    }

    pub fn is_active(&self, epoch: u64) -> bool {
        self.activation_epoch <= epoch && epoch < self.exit_epoch && !self.slashed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaconBlockHeader {
    pub slot: u64,
    pub proposer_index: u64,
    pub parent_root: [u8; 32],
    pub state_root: [u8; 32],
    pub body_root: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaconState {
    pub slot: u64,
    pub validators: Vec<Validator>,
    pub balances: Vec<u64>,
    pub latest_block_header: BeaconBlockHeader,
    pub finalized_checkpoint_epoch: u64,
    pub justified_checkpoint_epoch: u64,
}

impl BeaconState {
    pub fn genesis(validators: Vec<Validator>) -> Self {
        let balances = validators.iter().map(|v| v.effective_balance).collect();
        let genesis_header = BeaconBlockHeader {
            slot: 0,
            proposer_index: 0,
            parent_root: [0u8; 32],
            state_root: [0u8; 32],
            body_root: [0u8; 32],
        };
        Self {
            slot: 0,
            validators,
            balances,
            latest_block_header: genesis_header,
            finalized_checkpoint_epoch: 0,
            justified_checkpoint_epoch: 0,
        }
    }

    pub fn current_epoch(&self) -> u64 {
        self.slot / SLOTS_PER_EPOCH
    }

    pub fn active_validator_count(&self) -> usize {
        let epoch = self.current_epoch();
        self.validators.iter().filter(|v| v.is_active(epoch)).count()
    }

    pub fn advance_slot(&mut self) {
        self.slot += 1;
        debug!(slot = self.slot, epoch = self.current_epoch(), "slot advanced");
    }

    pub fn proposer_index(&self) -> u64 {
        let epoch = self.current_epoch();
        let active: Vec<_> = self
            .validators
            .iter()
            .enumerate()
            .filter(|(_, v)| v.is_active(epoch))
            .map(|(i, _)| i as u64)
            .collect();
        if active.is_empty() {
            return 0;
        }
        active[(self.slot as usize) % active.len()]
    }
}
