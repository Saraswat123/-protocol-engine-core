/// FOCIL — Fork-Choice Enforced Inclusion Lists (EIP-7805 simplified)
///
/// At each slot a committee of validators submits inclusion lists (ILs).
/// The block proposer must include all txs in the aggregate IL unless the
/// block is already full (gas limit reached).  This breaks proposer censorship.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{info, warn};

pub type TxHash = [u8; 32];

/// One validator's inclusion list for a specific slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InclusionList {
    pub slot: u64,
    pub validator_index: u64,
    pub tx_hashes: Vec<TxHash>,
}

/// Aggregate IL = union of all committee ILs for a slot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregateIL {
    pub slot: u64,
    pub tx_hashes: HashSet<TxHash>,
    pub contributor_count: usize,
}

/// Collects per-validator ILs and aggregates them.
pub struct ILAggregator {
    /// slot → (validator_index → IL)
    lists: HashMap<u64, HashMap<u64, InclusionList>>,
    committee_size: usize,
}

impl ILAggregator {
    pub fn new(committee_size: usize) -> Self {
        Self { lists: HashMap::new(), committee_size }
    }

    pub fn add(&mut self, il: InclusionList) {
        info!(
            slot = il.slot,
            validator = il.validator_index,
            txs = il.tx_hashes.len(),
            "IL received"
        );
        self.lists
            .entry(il.slot)
            .or_default()
            .insert(il.validator_index, il);
    }

    pub fn aggregate(&self, slot: u64) -> AggregateIL {
        let slot_lists = match self.lists.get(&slot) {
            Some(m) => m,
            None => return AggregateIL { slot, ..Default::default() },
        };

        let mut tx_hashes = HashSet::new();
        for il in slot_lists.values() {
            tx_hashes.extend(il.tx_hashes.iter().copied());
        }

        info!(slot, txs = tx_hashes.len(), contributors = slot_lists.len(), "IL aggregated");
        AggregateIL {
            slot,
            tx_hashes,
            contributor_count: slot_lists.len(),
        }
    }

    /// True once at least (committee_size / 2 + 1) ILs received for slot.
    pub fn has_quorum(&self, slot: u64) -> bool {
        let count = self.lists.get(&slot).map(|m| m.len()).unwrap_or(0);
        count >= self.committee_size / 2 + 1
    }
}

/// Verifies a proposed block satisfies the aggregate IL.
pub struct ILEnforcer {
    pub gas_limit: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ILVerdict {
    /// Block includes all required txs.
    Satisfied,
    /// Block is full — proposer exempted from including remaining txs.
    ExemptBlockFull { gas_used: u64, gas_limit: u64 },
    /// Block censors txs without justification.
    Violated { missing: Vec<TxHash> },
}

impl ILEnforcer {
    pub fn new(gas_limit: u64) -> Self {
        Self { gas_limit }
    }

    /// Check block's included txs against the aggregate IL.
    pub fn verify(
        &self,
        aggregate: &AggregateIL,
        block_txs: &[TxHash],
        block_gas_used: u64,
    ) -> ILVerdict {
        let block_set: HashSet<TxHash> = block_txs.iter().copied().collect();
        let missing: Vec<TxHash> = aggregate
            .tx_hashes
            .iter()
            .filter(|h| !block_set.contains(*h))
            .copied()
            .collect();

        if missing.is_empty() {
            return ILVerdict::Satisfied;
        }

        // Proposer exempt if block is ≥ 90% full (simplified full-block exception)
        let fill_ratio = block_gas_used as f64 / self.gas_limit as f64;
        if fill_ratio >= 0.9 {
            info!(
                slot = aggregate.slot,
                missing = missing.len(),
                gas_used = block_gas_used,
                "IL: block full exemption granted"
            );
            return ILVerdict::ExemptBlockFull {
                gas_used: block_gas_used,
                gas_limit: self.gas_limit,
            };
        }

        warn!(
            slot = aggregate.slot,
            missing = missing.len(),
            "IL VIOLATION: proposer censored txs"
        );
        ILVerdict::Violated { missing }
    }
}
