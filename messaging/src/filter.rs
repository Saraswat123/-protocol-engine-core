use crate::broker::Message;

pub trait Filter: Send + Sync {
    fn allow(&self, msg: &Message) -> bool;
}

/// Only pass messages from specific senders.
pub struct SenderFilter {
    pub allowed: Vec<u64>,
}

impl Filter for SenderFilter {
    fn allow(&self, msg: &Message) -> bool {
        self.allowed.contains(&msg.sender)
    }
}

/// Only pass messages with payload below max_size bytes.
pub struct SizeFilter {
    pub max_bytes: usize,
}

impl Filter for SizeFilter {
    fn allow(&self, msg: &Message) -> bool {
        msg.payload.len() <= self.max_bytes
    }
}

/// Chains multiple filters — all must pass.
pub struct ChainFilter {
    pub filters: Vec<Box<dyn Filter>>,
}

impl Filter for ChainFilter {
    fn allow(&self, msg: &Message) -> bool {
        self.filters.iter().all(|f| f.allow(msg))
    }
}
