use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub hash: [u8; 32],
    pub sender: [u8; 20],
    pub nonce: u64,
    pub gas_price: u64,
    pub max_fee_per_gas: u64,
    pub gas_limit: u64,
    pub data: Vec<u8>,
}

impl Transaction {
    pub fn effective_tip(&self, base_fee: u64) -> u64 {
        self.max_fee_per_gas.saturating_sub(base_fee).min(self.gas_price)
    }
}

/// Priority mempool: ordered by effective tip (EIP-1559 ordering).
pub struct Mempool {
    /// (tip, nonce, hash) → tx — BTreeMap gives us sorted order for free
    ordered: BTreeMap<(std::cmp::Reverse<u64>, u64, [u8; 32]), Transaction>,
    base_fee: u64,
}

impl Mempool {
    pub fn new(base_fee: u64) -> Self {
        Self { ordered: BTreeMap::new(), base_fee }
    }

    pub fn insert(&mut self, tx: Transaction) {
        let tip = tx.effective_tip(self.base_fee);
        let key = (std::cmp::Reverse(tip), tx.nonce, tx.hash);
        self.ordered.insert(key, tx);
    }

    pub fn remove(&mut self, hash: &[u8; 32]) {
        self.ordered.retain(|(_, _, h), _| h != hash);
    }

    /// Return up to `limit` txs sorted by highest tip first.
    pub fn top(&self, limit: usize) -> Vec<&Transaction> {
        self.ordered.values().take(limit).collect()
    }

    pub fn len(&self) -> usize {
        self.ordered.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ordered.is_empty()
    }

    pub fn update_base_fee(&mut self, base_fee: u64) {
        let txs: Vec<_> = self.ordered.values().cloned().collect();
        self.ordered.clear();
        self.base_fee = base_fee;
        for tx in txs {
            self.insert(tx);
        }
    }
}
