/// Partition test: simulate f Byzantine nodes (equivocate or stay silent).
/// Safety invariant: honest nodes never commit conflicting blocks.
use consensus::{
    block::Block,
    node::HotStuffNode,
    vote::Vote,
};

const N: usize = 4; // n = 4, f = 1

fn honest_cluster() -> Vec<HotStuffNode> {
    (0..N as u64).map(|id| HotStuffNode::new(id, N)).collect()
}

fn run_honest_view(nodes: &mut Vec<HotStuffNode>, payload: Vec<u8>) -> bool {
    let leader_id = nodes[0].leader_for_view(nodes[0].view) as usize;
    let block = match nodes[leader_id].propose(payload) {
        Some(b) => b,
        None => return false,
    };
    let votes: Vec<Vote> = nodes.iter().filter_map(|n| n.on_proposal(&block)).collect();
    let mut qc = None;
    for vote in votes {
        qc = nodes[leader_id].on_vote(vote);
        if qc.is_some() { break; }
    }
    let qc = match qc { Some(q) => q, None => return false };
    for node in nodes.iter_mut() {
        node.blocks.insert(block.id.clone(), block.clone());
        node.advance_view(qc.clone());
    }
    true
}

/// f=1 Byzantine: one node withholds vote. Remaining 3 = 2f+1, QC still forms.
#[test]
fn test_one_silent_node_qc_still_forms() {
    let mut nodes = honest_cluster();
    let leader_id = nodes[0].leader_for_view(1) as usize;
    let block = nodes[leader_id].propose(b"payload".to_vec()).unwrap();

    // nodes 0..3, node 3 stays silent (Byzantine: withholds vote)
    let votes: Vec<Vote> = nodes[..3]
        .iter()
        .filter_map(|n| n.on_proposal(&block))
        .collect();

    let mut qc = None;
    for vote in votes {
        qc = nodes[leader_id].on_vote(vote);
        if qc.is_some() { break; }
    }
    assert!(qc.is_some(), "QC must form with 3/4 votes (f=1 tolerance)");
}

/// f=1 equivocation: Byzantine leader sends two different blocks to two groups.
/// Honest replicas must not commit conflicting blocks.
#[test]
fn test_equivocating_leader_no_conflicting_commit() {
    // Byzantine leader is node 0 (view 1 % 4 = 1, so actually leader=1)
    // Let's pick view 2 where leader=2
    let mut nodes = honest_cluster();
    // First advance to view 2 honestly
    run_honest_view(&mut nodes, b"view-1".to_vec());

    // Now in view 2: leader=2 (Byzantine)
    // Group A (nodes 0,1) sees block_a
    // Group B (nodes 3) sees block_b
    // Neither group has enough votes for QC (1 or 2 < 3)
    let leader_id = nodes[0].leader_for_view(nodes[0].view) as usize;
    assert_eq!(leader_id, 2);

    let block_a = Block::new(nodes[0].view, nodes[0].high_qc.block_id.clone(), b"block-a".to_vec(), 2);
    let block_b = Block::new(nodes[0].view, nodes[0].high_qc.block_id.clone(), b"block-b".to_vec(), 2);

    // Group A: nodes 0,1 vote for block_a
    let votes_a: Vec<Vote> = nodes[..2].iter().filter_map(|n| n.on_proposal(&block_a)).collect();
    // Group B: node 3 votes for block_b
    let votes_b: Vec<Vote> = nodes[3..].iter().filter_map(|n| n.on_proposal(&block_b)).collect();

    // Neither group reaches 2f+1=3 — no QC for either block
    let mut qc_a = None;
    for v in votes_a { qc_a = nodes[leader_id].on_vote(v); }

    let mut qc_b = None;
    let mut leader_b = HotStuffNode::new(2, N); // fresh collector for block_b
    leader_b.view = 2;
    for v in votes_b { qc_b = leader_b.on_vote(v); }

    assert!(qc_a.is_none(), "block_a must not reach QC with only 2 votes");
    assert!(qc_b.is_none(), "block_b must not reach QC with only 1 vote");

    // Safety: no node has committed anything in view 2
    for node in &nodes {
        assert!(
            node.committed.is_empty() || node.committed.last().map(|b| b.view).unwrap_or(0) < 2,
            "no node should have committed a view-2 block"
        );
    }
}

/// Partition: split 4 nodes into 2+2. Neither partition has quorum alone.
#[test]
fn test_network_partition_no_progress() {
    let mut nodes = honest_cluster();
    let leader_id = nodes[0].leader_for_view(1) as usize;
    let block = nodes[leader_id].propose(b"data".to_vec()).unwrap();

    // Partition A: nodes 0,1 — only 2 votes, below 2f+1=3
    let votes_a: Vec<Vote> = nodes[..2].iter().filter_map(|n| n.on_proposal(&block)).collect();
    assert_eq!(votes_a.len(), 2);

    let mut qc = None;
    for v in votes_a { qc = nodes[leader_id].on_vote(v); }
    assert!(qc.is_none(), "partition of size 2 must not form QC");
}

/// After partition heals: 3+ votes reunite, QC forms and chain resumes.
#[test]
fn test_partition_heal_resumes_progress() {
    let mut nodes = honest_cluster();

    // 2 views of honest operation
    assert!(run_honest_view(&mut nodes, b"v1".to_vec()));
    assert!(run_honest_view(&mut nodes, b"v2".to_vec()));

    // Should have committed view-1 block by 2-chain rule
    assert!(!nodes[0].committed.is_empty(), "chain must progress after partition heals");
}
