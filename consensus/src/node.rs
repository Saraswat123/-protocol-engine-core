use crate::{
    block::{Block, BlockId},
    vote::{QuorumCertificate, Vote, VoteCollector},
};
use std::collections::HashMap;
use tracing::{debug, info};

#[derive(Debug, Clone, PartialEq)]
pub enum NodeRole {
    Leader,
    Replica,
}

pub struct HotStuffNode {
    pub id: u64,
    pub n: usize,
    pub f: usize,
    pub view: u64,
    pub high_qc: QuorumCertificate,
    pub locked_qc: QuorumCertificate,
    pub blocks: HashMap<BlockId, Block>,
    pub votes: VoteCollector,
    pub committed: Vec<Block>,
}

impl HotStuffNode {
    pub fn new(id: u64, n: usize) -> Self {
        let f = (n - 1) / 3;
        let genesis = Block::genesis();
        let mut blocks = HashMap::new();
        blocks.insert(genesis.id.clone(), genesis);

        Self {
            id,
            n,
            f,
            view: 1,
            high_qc: QuorumCertificate::genesis(),
            locked_qc: QuorumCertificate::genesis(),
            blocks,
            votes: VoteCollector::new(n, f),
            committed: vec![],
        }
    }

    pub fn leader_for_view(&self, view: u64) -> u64 {
        (view % self.n as u64) as u64
    }

    pub fn is_leader(&self) -> bool {
        self.leader_for_view(self.view) == self.id
    }

    /// Leader: propose a new block extending high_qc.
    pub fn propose(&mut self, payload: Vec<u8>) -> Option<Block> {
        if !self.is_leader() {
            return None;
        }
        let block = Block::new(self.view, self.high_qc.block_id.clone(), payload, self.id);
        info!(node = self.id, view = self.view, block = %block.id, "proposing block");
        self.blocks.insert(block.id.clone(), block.clone());
        Some(block)
    }

    /// Replica: on receiving a proposal, vote if safe.
    pub fn on_proposal(&self, block: &Block) -> Option<Vote> {
        if block.view != self.view {
            debug!(node = self.id, "ignoring proposal for wrong view");
            return None;
        }
        // Safety rule: only vote if block extends locked_qc or locked_qc is genesis
        let extends_lock = block.parent_id == self.locked_qc.block_id
            || self.locked_qc.view == 0
            || block.view > self.locked_qc.view + 1;

        if extends_lock {
            info!(node = self.id, view = self.view, block = %block.id, "voting");
            Some(Vote {
                block_id: block.id.clone(),
                view: self.view,
                voter: self.id,
            })
        } else {
            debug!(node = self.id, "vote refused: safety rule");
            None
        }
    }

    /// Leader: collect a vote; returns QC if quorum reached.
    pub fn on_vote(&mut self, vote: Vote) -> Option<QuorumCertificate> {
        let qc = self.votes.add(vote)?;
        info!(node = self.id, view = self.view, signers = ?qc.signers, "QC formed");
        if qc.view >= self.high_qc.view {
            self.high_qc = qc.clone();
        }
        Some(qc)
    }

    /// Advance to next view after QC. Commit if 2-chain rule satisfied.
    pub fn advance_view(&mut self, qc: QuorumCertificate) {
        // 2-chain commit: if qc.view == locked_qc.view + 1, commit locked block
        if qc.view == self.locked_qc.view + 1 && self.locked_qc.view > 0 {
            if let Some(block) = self.blocks.get(&self.locked_qc.block_id) {
                info!(node = self.id, view = block.view, block = %block.id, "COMMITTED");
                self.committed.push(block.clone());
            }
        }
        // high_qc must track the highest QC seen — all nodes receive it here
        if qc.view >= self.high_qc.view {
            self.high_qc = qc.clone();
        }
        self.locked_qc = qc;
        self.view += 1;
    }
}
