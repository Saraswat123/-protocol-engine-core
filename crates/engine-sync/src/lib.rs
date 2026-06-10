use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{info, warn};

// ── types ─────────────────────────────────────────────────────────────────────

pub type BlockHash = [u8; 32];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PayloadStatus {
    Valid,
    Invalid { validation_error: String },
    Syncing,
    Accepted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkchoiceState {
    pub head_block_hash: BlockHash,
    pub safe_block_hash: BlockHash,
    pub finalized_block_hash: BlockHash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPayload {
    pub block_hash: BlockHash,
    pub parent_hash: BlockHash,
    pub block_number: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    pub transactions: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkchoiceUpdatedResult {
    pub payload_status: PayloadStatus,
    pub payload_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineApiEvent {
    NewPayloadReceived { block_number: u64, block_hash: BlockHash, status: PayloadStatus },
    ForkchoiceUpdated { head: BlockHash, finalized: BlockHash, status: PayloadStatus },
    ChainReorg { old_head: BlockHash, new_head: BlockHash, depth: u64 },
}

// ── execution layer state ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub hash: BlockHash,
    pub parent_hash: BlockHash,
    pub number: u64,
    pub gas_used: u64,
}

pub struct ExecutionState {
    /// All known valid blocks by hash.
    pub blocks: HashMap<BlockHash, BlockHeader>,
    pub head: BlockHash,
    pub safe: BlockHash,
    pub finalized: BlockHash,
    /// Depth of canonical chain from genesis.
    pub height: u64,
}

impl ExecutionState {
    pub fn genesis(genesis_hash: BlockHash) -> Self {
        let genesis = BlockHeader {
            hash: genesis_hash,
            parent_hash: [0u8; 32],
            number: 0,
            gas_used: 0,
        };
        let mut blocks = HashMap::new();
        blocks.insert(genesis_hash, genesis);
        Self {
            blocks,
            head: genesis_hash,
            safe: genesis_hash,
            finalized: genesis_hash,
            height: 0,
        }
    }

    pub fn insert_block(&mut self, header: BlockHeader) {
        self.height = self.height.max(header.number);
        self.blocks.insert(header.hash, header);
    }

    pub fn is_known(&self, hash: &BlockHash) -> bool {
        self.blocks.contains_key(hash)
    }

    /// Walk from `hash` back to genesis via parent_hash links; return chain depth.
    pub fn chain_depth(&self, hash: &BlockHash) -> u64 {
        let mut depth = 0u64;
        let mut current = *hash;
        while let Some(b) = self.blocks.get(&current) {
            if b.number == 0 { break; }
            current = b.parent_hash;
            depth += 1;
        }
        depth
    }
}

// ── engine api handler ────────────────────────────────────────────────────────

pub struct EngineApi {
    pub exec: ExecutionState,
    gas_limit: u64,
    event_tx: mpsc::Sender<EngineApiEvent>,
}

impl EngineApi {
    pub fn new(genesis_hash: BlockHash, gas_limit: u64, event_tx: mpsc::Sender<EngineApiEvent>) -> Self {
        Self {
            exec: ExecutionState::genesis(genesis_hash),
            gas_limit,
            event_tx,
        }
    }

    /// engine_newPayload: validate and import an execution payload.
    pub async fn new_payload(&mut self, payload: ExecutionPayload) -> PayloadStatus {
        // basic validation
        if payload.gas_used > payload.gas_limit {
            let err = format!("gas_used {} > gas_limit {}", payload.gas_used, payload.gas_limit);
            warn!(block = payload.block_number, "{}", err);
            let status = PayloadStatus::Invalid { validation_error: err };
            let _ = self.event_tx.send(EngineApiEvent::NewPayloadReceived {
                block_number: payload.block_number,
                block_hash: payload.block_hash,
                status: status.clone(),
            }).await;
            return status;
        }
        if payload.gas_limit != self.gas_limit {
            let err = format!(
                "gas_limit mismatch: got {} expected {}",
                payload.gas_limit, self.gas_limit
            );
            warn!(block = payload.block_number, "{}", err);
            let status = PayloadStatus::Invalid { validation_error: err };
            let _ = self.event_tx.send(EngineApiEvent::NewPayloadReceived {
                block_number: payload.block_number,
                block_hash: payload.block_hash,
                status: status.clone(),
            }).await;
            return status;
        }
        // parent must be known
        if !self.exec.is_known(&payload.parent_hash) {
            info!(block = payload.block_number, "parent unknown, syncing");
            let status = PayloadStatus::Syncing;
            let _ = self.event_tx.send(EngineApiEvent::NewPayloadReceived {
                block_number: payload.block_number,
                block_hash: payload.block_hash,
                status: status.clone(),
            }).await;
            return status;
        }

        let header = BlockHeader {
            hash: payload.block_hash,
            parent_hash: payload.parent_hash,
            number: payload.block_number,
            gas_used: payload.gas_used,
        };
        self.exec.insert_block(header);
        info!(block = payload.block_number, hash = ?payload.block_hash, "payload accepted");

        let status = PayloadStatus::Valid;
        let _ = self.event_tx.send(EngineApiEvent::NewPayloadReceived {
            block_number: payload.block_number,
            block_hash: payload.block_hash,
            status: status.clone(),
        }).await;
        status
    }

    /// engine_forkchoiceUpdated: update head/safe/finalized pointers.
    pub async fn forkchoice_updated(&mut self, fcs: ForkchoiceState) -> ForkchoiceUpdatedResult {
        if !self.exec.is_known(&fcs.head_block_hash) {
            warn!(head = ?fcs.head_block_hash, "forkchoice head unknown");
            return ForkchoiceUpdatedResult {
                payload_status: PayloadStatus::Syncing,
                payload_id: None,
            };
        }

        let old_head = self.exec.head;
        let reorg_depth = if old_head != fcs.head_block_hash {
            self.reorg_depth(old_head, fcs.head_block_hash)
        } else {
            0
        };

        self.exec.head = fcs.head_block_hash;
        if self.exec.is_known(&fcs.safe_block_hash) {
            self.exec.safe = fcs.safe_block_hash;
        }
        if self.exec.is_known(&fcs.finalized_block_hash) {
            self.exec.finalized = fcs.finalized_block_hash;
        }

        if old_head != fcs.head_block_hash {
            let _ = self.event_tx.send(EngineApiEvent::ChainReorg {
                old_head,
                new_head: fcs.head_block_hash,
                depth: reorg_depth,
            }).await;
            info!(depth = reorg_depth, "chain reorg detected");
        }

        let _ = self.event_tx.send(EngineApiEvent::ForkchoiceUpdated {
            head: fcs.head_block_hash,
            finalized: fcs.finalized_block_hash,
            status: PayloadStatus::Valid,
        }).await;

        info!(head = ?self.exec.head, finalized = ?self.exec.finalized, "forkchoice updated");
        ForkchoiceUpdatedResult {
            payload_status: PayloadStatus::Valid,
            payload_id: None,
        }
    }

    /// Estimate reorg depth between two forks by walking to common ancestor.
    fn reorg_depth(&self, old_head: BlockHash, new_head: BlockHash) -> u64 {
        let old_num = self.exec.blocks.get(&old_head).map(|b| b.number).unwrap_or(0);
        let new_num = self.exec.blocks.get(&new_head).map(|b| b.number).unwrap_or(0);
        // simplified: depth is difference in block numbers (assumes linear chain)
        old_num.abs_diff(new_num)
    }
}

// ── legacy SyncEngine (kept for existing tests) ───────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTarget {
    pub name: String,
    pub endpoint: String,
    pub last_synced: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncEvent {
    Started { target: String },
    BlockReceived { target: String, block_number: u64 },
    Completed { target: String, final_block: u64 },
    Failed { target: String, reason: String },
}

pub struct SyncEngine {
    targets: HashMap<String, SyncTarget>,
    event_tx: mpsc::Sender<SyncEvent>,
}

impl SyncEngine {
    pub fn new(event_tx: mpsc::Sender<SyncEvent>) -> Self {
        Self { targets: HashMap::new(), event_tx }
    }

    pub fn add_target(&mut self, target: SyncTarget) {
        info!(name = %target.name, endpoint = %target.endpoint, "registered sync target");
        self.targets.insert(target.name.clone(), target);
    }

    pub async fn sync_all(&self) {
        for target in self.targets.values() {
            let _ = self.event_tx.send(SyncEvent::Started {
                target: target.name.clone(),
            }).await;
            info!(target = %target.name, "sync started");
        }
    }

    pub async fn report_block(&self, target_name: &str, block_number: u64) {
        if self.targets.contains_key(target_name) {
            let _ = self.event_tx.send(SyncEvent::BlockReceived {
                target: target_name.to_string(),
                block_number,
            }).await;
        } else {
            warn!(target = target_name, "unknown sync target");
        }
    }

    pub fn target_count(&self) -> usize {
        self.targets.len()
    }
}
