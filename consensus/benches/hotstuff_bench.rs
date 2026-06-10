use consensus::node::HotStuffNode;
use consensus::vote::Vote;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn make_cluster(n: usize) -> Vec<HotStuffNode> {
    (0..n as u64).map(|id| HotStuffNode::new(id, n)).collect()
}

fn run_view(nodes: &mut Vec<HotStuffNode>, payload: Vec<u8>) {
    let leader_id = nodes[0].leader_for_view(nodes[0].view) as usize;
    let block = nodes[leader_id].propose(payload).unwrap();
    let votes: Vec<Vote> = nodes.iter().filter_map(|n| n.on_proposal(&block)).collect();
    let mut qc = None;
    for vote in votes {
        qc = nodes[leader_id].on_vote(vote);
        if qc.is_some() { break; }
    }
    let qc = qc.unwrap();
    for node in nodes.iter_mut() {
        node.blocks.insert(block.id.clone(), block.clone());
        node.advance_view(qc.clone());
    }
}

/// Throughput: how many views/sec can the consensus engine process?
fn bench_views_per_second(c: &mut Criterion) {
    let mut group = c.benchmark_group("hotstuff_views");

    for n in [4usize, 7, 10] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let mut nodes = make_cluster(n);
                for v in 0..10 {
                    run_view(&mut nodes, black_box(format!("payload-{v}").into_bytes()));
                }
                black_box(nodes[0].committed.len())
            });
        });
    }
    group.finish();
}

/// Quorum formation: time to collect 2f+1 votes and form QC.
fn bench_qc_formation(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc_formation");

    for n in [4usize, 7, 10, 16] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let mut nodes = make_cluster(n);
                let leader_id = 0;
                let block = nodes[leader_id].propose(b"bench-payload".to_vec()).unwrap();
                let votes: Vec<Vote> = nodes.iter().filter_map(|nd| nd.on_proposal(&block)).collect();
                let mut qc = None;
                for vote in votes {
                    qc = nodes[leader_id].on_vote(vote);
                    if qc.is_some() { break; }
                }
                black_box(qc)
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_views_per_second, bench_qc_formation);
criterion_main!(benches);
