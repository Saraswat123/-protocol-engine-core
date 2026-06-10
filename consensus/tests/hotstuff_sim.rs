use consensus::{
    block::Block,
    node::HotStuffNode,
};

fn make_cluster(n: usize) -> Vec<HotStuffNode> {
    (0..n as u64).map(|id| HotStuffNode::new(id, n)).collect()
}

/// Run one full view: leader proposes, all replicas vote, leader collects QC, all advance.
fn run_view(nodes: &mut Vec<HotStuffNode>, payload: Vec<u8>) {
    let n = nodes.len();
    let leader_id = nodes[0].leader_for_view(nodes[0].view);

    // Leader proposes
    let block: Block = nodes[leader_id as usize].propose(payload).expect("leader must propose");

    // All nodes (including leader) vote
    let mut votes = vec![];
    for node in nodes.iter() {
        if let Some(vote) = node.on_proposal(&block) {
            votes.push(vote);
        }
    }

    // Leader collects votes, forms QC
    let mut qc = None;
    for vote in votes {
        qc = nodes[leader_id as usize].on_vote(vote);
        if qc.is_some() {
            break;
        }
    }
    let qc = qc.expect("quorum must form with n=4, f=1");

    // All nodes advance view
    for node in nodes.iter_mut() {
        node.blocks.insert(block.id.clone(), block.clone());
        node.advance_view(qc.clone());
    }
}

#[test]
fn test_4node_two_views_commit() {
    let mut nodes = make_cluster(4);

    // View 1: propose + QC
    run_view(&mut nodes, b"tx-batch-1".to_vec());
    assert_eq!(nodes[0].view, 2);

    // View 2: propose + QC — triggers 2-chain commit of view-1 block
    run_view(&mut nodes, b"tx-batch-2".to_vec());
    assert_eq!(nodes[0].view, 3);

    // Leader should have committed the view-1 block
    let leader = &nodes[0];
    assert_eq!(leader.committed.len(), 1);
    assert_eq!(leader.committed[0].payload, b"tx-batch-1");
}

#[test]
fn test_quorum_requires_2f_plus_1() {
    let mut nodes = make_cluster(4); // f=1, need 3 votes
    let leader_id = nodes[0].leader_for_view(1);
    let block = nodes[leader_id as usize].propose(b"payload".to_vec()).unwrap();

    let mut collector_node = HotStuffNode::new(99, 4);
    collector_node.view = 1;

    // Only 2 votes — no QC
    for id in 0..2u64 {
        let vote = consensus::vote::Vote { block_id: block.id.clone(), view: 1, voter: id };
        let result = collector_node.on_vote(vote);
        assert!(result.is_none(), "should not form QC with only 2 votes");
    }

    // 3rd vote — QC forms
    let vote = consensus::vote::Vote { block_id: block.id.clone(), view: 1, voter: 2 };
    let qc = collector_node.on_vote(vote);
    assert!(qc.is_some(), "QC must form at 2f+1=3 votes");
}

#[test]
fn test_safety_locked_block() {
    let mut nodes = make_cluster(4);

    // Complete view 1 to set locked_qc
    run_view(&mut nodes, b"block-1".to_vec());

    // Node with locked_qc should refuse proposal that doesn't extend it
    let node = &nodes[0];
    let bad_block = consensus::block::Block::new(
        node.view,
        consensus::block::BlockId::genesis(), // doesn't extend locked
        b"conflicting".to_vec(),
        0,
    );

    // Safety: vote only if block view > locked_qc.view + 1 (or extends lock)
    // Since locked_qc.view=1 and bad_block.view=2 with wrong parent, this may still pass
    // depending on the liveness rule. The key is locked blocks cannot be orphaned.
    let vote = node.on_proposal(&bad_block);
    // vote is None because safety rule refuses
    assert!(vote.is_none());
}
