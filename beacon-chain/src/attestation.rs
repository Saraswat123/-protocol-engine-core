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

// ── aggregate attestation ─────────────────────────────────────────────────────

/// One aggregate covers all individual attestations for a (slot, target_root).
/// Validators are deduplicated — each counted once regardless of how many
/// times they submitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateAttestation {
    pub slot: u64,
    pub target_root: [u8; 32],
    /// Sorted list of validator indices that signed this aggregate.
    pub participating_validators: Vec<u64>,
    pub vote_count: usize,
}

impl AggregateAttestation {
    pub fn empty(slot: u64, target_root: [u8; 32]) -> Self {
        Self { slot, target_root, participating_validators: vec![], vote_count: 0 }
    }

    pub fn includes(&self, validator_index: u64) -> bool {
        self.participating_validators.binary_search(&validator_index).is_ok()
    }
}

// ── pool ──────────────────────────────────────────────────────────────────────

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

    /// Aggregate all attestations for a given (slot, target_root) pair.
    /// Deduplicates by validator index; returns sorted participating set.
    pub fn aggregate(&self, slot: u64, target_root: [u8; 32]) -> AggregateAttestation {
        let mut validators: Vec<u64> = self.latest
            .values()
            .filter(|a| a.slot == slot && a.target.root == target_root)
            .map(|a| a.validator_index)
            .collect();

        validators.sort_unstable();
        validators.dedup();
        let count = validators.len();

        AggregateAttestation {
            slot,
            target_root,
            participating_validators: validators,
            vote_count: count,
        }
    }

    /// Aggregate vote weight for a block root using the aggregated view.
    /// Same result as `vote_weight` but goes through `AggregateAttestation`
    /// to ensure deduplication semantics are enforced.
    pub fn aggregate_vote_weight(&self, root: &[u8; 32]) -> usize {
        // collect distinct validators whose latest message points to root
        let mut validators: Vec<u64> = self.latest
            .values()
            .filter(|a| &a.beacon_block_root == root)
            .map(|a| a.validator_index)
            .collect();
        validators.sort_unstable();
        validators.dedup();
        validators.len()
    }
}

impl Default for AttestationPool {
    fn default() -> Self {
        Self::new()
    }
}
