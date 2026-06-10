use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use orderflow::{builder::Builder, mempool::{Mempool, Transaction}};

fn make_tx(i: u8, gas_price: u64, max_fee: u64) -> Transaction {
    Transaction {
        hash: [i; 32],
        sender: [0u8; 20],
        nonce: i as u64,
        gas_price,
        max_fee_per_gas: max_fee,
        gas_limit: 21_000,
        data: vec![],
    }
}

/// Mempool insert throughput at varying pool sizes.
fn bench_mempool_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("mempool_insert");

    for size in [10usize, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut pool = Mempool::new(black_box(10));
                for i in 0..size as u8 {
                    pool.insert(make_tx(i, 15 + i as u64, 25 + i as u64));
                }
                black_box(pool.len())
            });
        });
    }
    group.finish();
}

/// Block building: greedy selection from pools of varying depth.
fn bench_block_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_build");
    let base_fee = 10u64;
    let gas_limit = 30_000_000u64;

    for tx_count in [50usize, 200, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(tx_count), &tx_count, |b, &tx_count| {
            let mut pool = Mempool::new(base_fee);
            for i in 0..tx_count as u8 {
                pool.insert(make_tx(i, 15 + i as u64, 25 + i as u64));
            }
            let builder = Builder::new(1, gas_limit);

            b.iter(|| {
                let block = builder.build(black_box(&pool), base_fee);
                black_box(block.gas_used)
            });
        });
    }
    group.finish();
}

/// base_fee update cost: re-sort entire pool.
fn bench_base_fee_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("base_fee_update");

    for size in [50usize, 200, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched(
                || {
                    let mut pool = Mempool::new(10);
                    for i in 0..size as u8 {
                        pool.insert(make_tx(i, 15 + i as u64, 25 + i as u64));
                    }
                    pool
                },
                |mut pool| {
                    pool.update_base_fee(black_box(15));
                    black_box(pool.len())
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_mempool_insert, bench_block_build, bench_base_fee_update);
criterion_main!(benches);
