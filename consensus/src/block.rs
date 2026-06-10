use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BlockId(pub [u8; 32]);

impl BlockId {
    pub fn genesis() -> Self {
        Self([0u8; 32])
    }
}

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.0[..4]))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: BlockId,
    pub view: u64,
    pub parent_id: BlockId,
    pub payload: Vec<u8>,
    pub proposer: u64,
}

impl Block {
    pub fn new(view: u64, parent_id: BlockId, payload: Vec<u8>, proposer: u64) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(view.to_le_bytes());
        hasher.update(&parent_id.0);
        hasher.update(&payload);
        hasher.update(proposer.to_le_bytes());
        let id = BlockId(hasher.finalize().into());
        Self { id, view, parent_id, payload, proposer }
    }

    pub fn genesis() -> Self {
        Self {
            id: BlockId::genesis(),
            view: 0,
            parent_id: BlockId::genesis(),
            payload: vec![],
            proposer: 0,
        }
    }
}
