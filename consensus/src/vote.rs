use crate::block::BlockId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub block_id: BlockId,
    pub view: u64,
    pub voter: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumCertificate {
    pub block_id: BlockId,
    pub view: u64,
    pub signers: Vec<u64>,
}

impl QuorumCertificate {
    pub fn genesis() -> Self {
        Self {
            block_id: BlockId::genesis(),
            view: 0,
            signers: vec![],
        }
    }
}

/// Collects votes for a specific view; forms QC when threshold reached.
pub struct VoteCollector {
    threshold: usize,
    votes: HashMap<u64, Vec<Vote>>, // view → votes
}

impl VoteCollector {
    pub fn new(_n: usize, f: usize) -> Self {
        Self {
            threshold: 2 * f + 1,
            votes: HashMap::new(),
        }
    }

    /// Returns a QC if quorum is reached for this vote's view.
    pub fn add(&mut self, vote: Vote) -> Option<QuorumCertificate> {
        let view = vote.view;
        let block_id = vote.block_id.clone();

        let entry = self.votes.entry(view).or_default();
        if entry.iter().any(|v| v.voter == vote.voter) {
            return None;
        }
        entry.push(vote);

        if entry.len() >= self.threshold {
            let signers = entry.iter().map(|v| v.voter).collect();
            Some(QuorumCertificate { block_id, view, signers })
        } else {
            None
        }
    }
}
