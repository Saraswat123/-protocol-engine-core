use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::debug;

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

/// Priority mempool: ordered by effective tip (EIP-1559 ordering), bounded by max_size.
///
/// When full, the lowest-priority transaction is evicted to make room. If the
/// incoming tx is itself the lowest-priority, it is rejected immediately.
pub struct Mempool {
    /// (Reverse(tip), nonce, hash) → tx — BTreeMap gives sorted order for free.
    ordered: BTreeMap<(std::cmp::Reverse<u64>, u64, [u8; 32]), Transaction>,
    base_fee: u64,
    max_size: usize,
}

impl Mempool {
    /// `max_size = 0` means unbounded (legacy behaviour for tests that don't care about eviction).
    pub fn new(base_fee: u64, max_size: usize) -> Self {
        Self { ordered: BTreeMap::new(), base_fee, max_size }
    }

    /// Insert a transaction.
    ///
    /// Returns `true`  — tx accepted (possibly evicted a lower-priority tx).
    /// Returns `false` — tx rejected (it was the lowest-priority and evicted itself).
    pub fn insert(&mut self, tx: Transaction) -> bool {
        let tip = tx.effective_tip(self.base_fee);
        debug!(hash = hex::encode(&tx.hash[..4]), tip, "mempool insert");
        let key = (std::cmp::Reverse(tip), tx.nonce, tx.hash);
        self.ordered.insert(key, tx.clone());

        // enforce cap
        if self.max_size > 0 && self.ordered.len() > self.max_size {
            // last entry = lowest (Reverse tip, so largest key = smallest tip)
            if let Some((&last_key, _)) = self.ordered.iter().next_back() {
                self.ordered.remove(&last_key);
                // if we just removed the tx we inserted → rejected
                if last_key.2 == tx.hash {
                    return false;
                }
            }
        }
        true
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

    pub fn max_size(&self) -> usize {
        self.max_size
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
