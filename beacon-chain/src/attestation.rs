use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Checkpoint {
    pub epoch: u64,
    pub root: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    pub slot: u64,
    pub validator_index: u64,
    pub beacon_block_root: [u8; 32],
    pub source: Checkpoint,
    pub target: Checkpoint,
}

/// Tracks latest messages for LMD-GHOST and vote weight.
pub struct AttestationPool {
    /// validator_index → latest attestation
    pub latest: HashMap<u64, Attestation>,
}

impl AttestationPool {
    pub fn new() -> Self {
        Self { latest: HashMap::new() }
    }

    pub fn add(&mut self, att: Attestation) {
        let entry = self.latest.entry(att.validator_index).or_insert(att.clone());
        // only update if newer slot
        if att.slot > entry.slot {
            *entry = att;
        }
    }

    /// Weight of a block root = number of validators with latest vote pointing to it.
    pub fn vote_weight(&self, root: &[u8; 32]) -> usize {
        self.latest
            .values()
            .filter(|a| &a.beacon_block_root == root)
            .count()
    }
}

impl Default for AttestationPool {
    fn default() -> Self {
        Self::new()
    }
}
