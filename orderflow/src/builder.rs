use crate::mempool::{Mempool, Transaction};

pub struct Block {
    pub number: u64,
    pub base_fee: u64,
    pub transactions: Vec<Transaction>,
    pub gas_used: u64,
    pub gas_limit: u64,
}

pub struct Builder {
    pub block_number: u64,
    pub gas_limit: u64,
}

impl Builder {
    pub fn new(block_number: u64, gas_limit: u64) -> Self {
        Self { block_number, gas_limit }
    }

    /// Build a block: greedily pick highest-tip txs until gas limit reached.
    pub fn build(&self, mempool: &Mempool, base_fee: u64) -> Block {
        let mut txs = vec![];
        let mut gas_used = 0u64;

        for tx in mempool.top(usize::MAX) {
            if gas_used + tx.gas_limit > self.gas_limit {
                continue; // skip if doesn't fit
            }
            gas_used += tx.gas_limit;
            txs.push(tx.clone());
        }

        Block {
            number: self.block_number,
            base_fee,
            transactions: txs,
            gas_used,
            gas_limit: self.gas_limit,
        }
    }

    /// EIP-1559 base fee adjustment for next block.
    pub fn next_base_fee(block: &Block) -> u64 {
        let target = block.gas_limit / 2;
        if block.gas_used == target {
            return block.base_fee;
        }
        let delta = block.base_fee / 8;
        if block.gas_used > target {
            block.base_fee + delta.max(1)
        } else {
            block.base_fee.saturating_sub(delta)
        }
    }
}
