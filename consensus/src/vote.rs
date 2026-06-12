use crate::block::BlockId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── view-change types ─────────────────────────────────────────────────────────

/// Sent by a replica to the next-view leader when it times out waiting for
/// a proposal or QC. Carries the highest QC the sender has seen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewViewMessage {
    pub sender: u64,
    /// The view the sender is abandoning.
    pub view: u64,
    pub high_qc: QuorumCertificate,
}

/// Collects NewView messages from replicas; forms the "highest QC" once
/// 2f+1 messages arrive for the same view.
pub struct NewViewCollector {
    threshold: usize,
    messages: HashMap<u64, Vec<NewViewMessage>>, // view → msgs
}

impl NewViewCollector {
    pub fn new(n: usize, f: usize) -> Self {
        Self { threshold: 2 * f + 1, messages: HashMap::new() }
    }

    /// Add a NewView message; returns the highest `high_qc` seen when
    /// threshold is reached for that view.
    pub fn add(&mut self, msg: NewViewMessage) -> Option<QuorumCertificate> {
        let view = msg.view;
        let entry = self.messages.entry(view).or_default();
        if entry.iter().any(|m| m.sender == msg.sender) {
            return None; // dedup
        }
        entry.push(msg);
        if entry.len() == self.threshold {
            let best = entry.iter().max_by_key(|m| m.high_qc.view).unwrap();
            Some(best.high_qc.clone())
        } else {
            None
        }
    }

    pub fn count(&self, view: u64) -> usize {
        self.messages.get(&view).map(|v| v.len()).unwrap_or(0)
    }
}

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

        if entry.len() == self.threshold {
            let signers = entry.iter().map(|v| v.voter).collect();
            Some(QuorumCertificate { block_id, view, signers })
        } else {
            None
        }
    }
}
