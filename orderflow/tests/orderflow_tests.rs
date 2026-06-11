use orderflow::{
    builder::Builder,
    mempool::{Mempool, Transaction},
};
use proptest::prelude::*;

fn tx(hash_byte: u8, gas_price: u64, max_fee: u64, gas_limit: u64) -> Transaction {
    Transaction {
        hash: [hash_byte; 32],
        sender: [0u8; 20],
        nonce: 0,
        gas_price,
        max_fee_per_gas: max_fee,
        gas_limit,
        data: vec![],
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[test]
fn test_mempool_orders_by_tip() {
    let base_fee = 10;
    let mut pool = Mempool::new(base_fee, 0);

    // tip = min(max_fee - base_fee, gas_price)
    pool.insert(tx(1, 5, 15, 21_000)); // tip = min(5, 5) = 5
    pool.insert(tx(2, 20, 30, 21_000)); // tip = min(20, 20) = 20
    pool.insert(tx(3, 10, 20, 21_000)); // tip = min(10, 10) = 10

    let top = pool.top(3);
    assert_eq!(top[0].hash[0], 2, "highest tip first");
    assert_eq!(top[1].hash[0], 3);
    assert_eq!(top[2].hash[0], 1, "lowest tip last");
}

#[test]
fn test_mempool_remove() {
    let mut pool = Mempool::new(10, 0);
    pool.insert(tx(1, 20, 30, 21_000));
    pool.insert(tx(2, 15, 25, 21_000));
    assert_eq!(pool.len(), 2);

    pool.remove(&[1u8; 32]);
    assert_eq!(pool.len(), 1);
    assert_eq!(pool.top(1)[0].hash[0], 2);
}

#[test]
fn test_mempool_base_fee_update_reorders() {
    // tx1: tight max_fee cap — tip shrinks fast as base_fee rises
    // tx2: low gas_price cap — tip is always capped by gas_price=5
    //
    // base_fee=5: tx1 tip=min(7,10)=7  tx2 tip=min(45,5)=5  → tx1 wins
    // base_fee=10: tx1 tip=min(2,10)=2  tx2 tip=min(40,5)=5  → tx2 wins
    let mut pool = Mempool::new(5, 0);
    pool.insert(tx(1, 10, 12, 21_000));
    pool.insert(tx(2,  5, 50, 21_000));

    assert_eq!(pool.top(1)[0].hash[0], 1, "tx1 should lead at base_fee=5");

    pool.update_base_fee(10);
    assert_eq!(pool.top(1)[0].hash[0], 2, "tx2 should lead after base_fee rises to 10");
}

#[test]
fn test_builder_respects_gas_limit() {
    let base_fee = 10;
    let mut pool = Mempool::new(base_fee, 0);
    let gas_limit = 100_000u64;

    // 6 txs of 21_000 gas each — only 4 fit (84_000 < 100_000 < 105_000)
    for i in 0..6u8 {
        pool.insert(tx(i, 20, 30, 21_000));
    }

    let builder = Builder::new(1, gas_limit);
    let block = builder.build(&pool, base_fee);

    assert!(block.gas_used <= gas_limit, "gas_used must not exceed limit");
    assert_eq!(block.transactions.len(), 4);
}

#[test]
fn test_builder_eip1559_base_fee_increases_on_full_block() {
    let base_fee = 100u64;
    let gas_limit = 30_000_000u64;
    // full block: gas_used = gas_limit
    let block = orderflow::builder::Block {
        number: 1,
        base_fee,
        transactions: vec![],
        gas_used: gas_limit,
        gas_limit,
    };
    let next = Builder::next_base_fee(&block);
    assert!(next > base_fee, "base fee must increase on full block");
}

#[test]
fn test_builder_eip1559_base_fee_decreases_on_empty_block() {
    let base_fee = 100u64;
    let gas_limit = 30_000_000u64;
    let block = orderflow::builder::Block {
        number: 1,
        base_fee,
        transactions: vec![],
        gas_used: 0,
        gas_limit,
    };
    let next = Builder::next_base_fee(&block);
    assert!(next < base_fee, "base fee must decrease on empty block");
}

// ── eviction / max-size cap ───────────────────────────────────────────────────

#[test]
fn test_eviction_keeps_highest_tip_txs() {
    let base_fee = 10;
    let mut pool = Mempool::new(base_fee, 5);

    // insert 10 txs with tips 1..10 (tip = gas_price since max_fee = gas_price + base_fee)
    for i in 1u8..=10 {
        pool.insert(tx(i, i as u64, i as u64 + base_fee, 21_000));
    }

    // only top-5 by tip should survive
    assert_eq!(pool.len(), 5, "pool must be capped at 5");
    let top = pool.top(5);
    // lowest surviving tip must be 6 (tips 1..5 evicted)
    let min_tip = top.iter().map(|t| t.effective_tip(base_fee)).min().unwrap();
    assert!(min_tip >= 6, "lowest surviving tip must be >= 6, got {min_tip}");
}

#[test]
fn test_rejected_tx_is_lowest_priority() {
    let base_fee = 10;
    let mut pool = Mempool::new(base_fee, 3);

    // fill pool with tip=20 txs
    pool.insert(tx(1, 20, 30, 21_000));
    pool.insert(tx(2, 20, 30, 21_000));
    pool.insert(tx(3, 20, 30, 21_000));

    // insert tx with tip=1 — should be rejected immediately (it's the lowest)
    let accepted = pool.insert(tx(4, 1, 11, 21_000));
    assert!(!accepted, "low-tip tx must be rejected when pool is full");
    assert_eq!(pool.len(), 3, "pool must stay at cap");
    assert!(pool.top(3).iter().all(|t| t.hash[0] != 4), "rejected tx must not be in pool");
}

#[test]
fn test_accepted_tx_evicts_lowest() {
    let base_fee = 10;
    let mut pool = Mempool::new(base_fee, 3);

    pool.insert(tx(1, 5, 15, 21_000));  // tip=5
    pool.insert(tx(2, 10, 20, 21_000)); // tip=10
    pool.insert(tx(3, 15, 25, 21_000)); // tip=15

    // insert tx with tip=20 — must evict tip=5 tx
    let accepted = pool.insert(tx(4, 20, 30, 21_000));
    assert!(accepted, "high-tip tx must be accepted");
    assert_eq!(pool.len(), 3, "pool stays at cap");
    assert!(pool.top(3).iter().all(|t| t.hash[0] != 1), "tip=5 tx must be evicted");
}

#[test]
fn test_unbounded_pool_never_evicts() {
    let mut pool = Mempool::new(10, 0); // 0 = unbounded
    for i in 0..200u8 {
        pool.insert(tx(i, 5, 15, 21_000));
    }
    assert_eq!(pool.len(), 200, "unbounded pool must accept all txs");
}

#[test]
fn test_eviction_after_base_fee_update() {
    // 5-slot pool, base_fee=5
    // tx1 tip@5 = min(15,10)=10, tx2 tip@5 = min(5, 5)=5 (barely eligible)
    // After base_fee→12: tx2 tip = min(5-12,5) = 0 → worthless
    let mut pool = Mempool::new(5, 3);
    pool.insert(tx(1, 10, 20, 21_000)); // tip@5 = 10
    pool.insert(tx(2, 8,  18, 21_000)); // tip@5 = 8
    pool.insert(tx(3, 5,  10, 21_000)); // tip@5 = 5

    // raise base_fee to 12 — now insert a high-tip tx
    pool.update_base_fee(12);
    // tx3: effective_tip@12 = min(10-12=0, 5)=0 → it becomes the lowest
    let accepted = pool.insert(tx(4, 20, 35, 21_000)); // tip@12 = min(23,20)=20
    assert!(accepted);
    // tx3 (tip=0) must be evicted
    assert!(pool.top(3).iter().all(|t| t.hash[0] != 3), "zero-tip tx must be evicted after base_fee rise");
}

// ── proptest fuzz ─────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_top_never_exceeds_pool_size(
        count in 1usize..20,
        base_fee in 1u64..100,
    ) {
        let mut pool = Mempool::new(base_fee, 0);
        for i in 0..count as u8 {
            pool.insert(tx(i, base_fee + 5, base_fee + 10, 21_000));
        }
        let top = pool.top(count + 100);
        prop_assert!(top.len() <= count);
    }

    #[test]
    fn prop_builder_gas_invariant(
        tx_count in 1usize..30,
        gas_limit in 21_000u64..1_000_000,
        base_fee in 1u64..50,
    ) {
        let mut pool = Mempool::new(base_fee, 0);
        for i in 0..tx_count as u8 {
            pool.insert(tx(i, base_fee + 5, base_fee + 10, 21_000));
        }
        let builder = Builder::new(1, gas_limit);
        let block = builder.build(&pool, base_fee);

        // invariant: gas_used never exceeds block gas_limit
        prop_assert!(block.gas_used <= gas_limit);
        // invariant: every tx in block fits within gas_limit individually
        for tx in &block.transactions {
            prop_assert!(tx.gas_limit <= gas_limit);
        }
    }

    #[test]
    fn prop_mempool_ordering_consistent(
        tips in prop::collection::vec(1u64..200, 2..10),
        base_fee in 1u64..50,
    ) {
        let mut pool = Mempool::new(base_fee, 0);
        for (i, tip) in tips.iter().enumerate() {
            pool.insert(tx(i as u8, *tip, tip + base_fee, 21_000));
        }
        let top = pool.top(tips.len());
        // verify descending tip order
        for window in top.windows(2) {
            let tip_a = window[0].effective_tip(base_fee);
            let tip_b = window[1].effective_tip(base_fee);
            prop_assert!(tip_a >= tip_b, "ordering violated: {} < {}", tip_a, tip_b);
        }
    }

    #[test]
    fn prop_capped_pool_never_exceeds_max_size(
        count in 1usize..50,
        cap in 1usize..20,
        base_fee in 1u64..50,
    ) {
        let mut pool = Mempool::new(base_fee, cap);
        for i in 0..count as u8 {
            pool.insert(tx(i, (i as u64 % 30) + 1, (i as u64 % 30) + 1 + base_fee, 21_000));
        }
        prop_assert!(
            pool.len() <= cap,
            "pool len {} exceeded cap {}", pool.len(), cap
        );
    }
}
