use consensus::{
    node::HotStuffNode,
    vote::{Vote, VoteCollector},
};
use proptest::prelude::*;

fn run_view(nodes: &mut Vec<HotStuffNode>, payload: Vec<u8>) -> bool {
    let leader_id = nodes[0].leader_for_view(nodes[0].view) as usize;

    let block = match nodes[leader_id].propose(payload) {
        Some(b) => b,
        None => return false,
    };

    let votes: Vec<Vote> = nodes
        .iter()
        .filter_map(|n| n.on_proposal(&block))
        .collect();

    let mut qc = None;
    for vote in votes {
        qc = nodes[leader_id].on_vote(vote);
        if qc.is_some() {
            break;
        }
    }

    let qc = match qc {
        Some(q) => q,
        None => return false,
    };

    for node in nodes.iter_mut() {
        node.blocks.insert(block.id.clone(), block.clone());
        node.advance_view(qc.clone());
    }
    true
}

proptest! {
    /// Safety: committed blocks are never contradicted across nodes.
    #[test]
    fn prop_committed_blocks_consistent(views in 2usize..8) {
        let n = 4;
        let mut nodes: Vec<HotStuffNode> = (0..n as u64).map(|id| HotStuffNode::new(id, n)).collect();

        for v in 0..views {
            let payload = format!("payload-{v}").into_bytes();
            run_view(&mut nodes, payload);
        }

        // All nodes must agree on committed blocks (same order, same content)
        let reference = &nodes[0].committed;
        for node in &nodes[1..] {
            prop_assert_eq!(
                node.committed.len(), reference.len(),
                "committed chain length must match across all nodes"
            );
            for (a, b) in node.committed.iter().zip(reference.iter()) {
                prop_assert_eq!(&a.id, &b.id, "committed block ids must match");
                prop_assert_eq!(&a.payload, &b.payload, "committed payloads must match");
            }
        }
    }

    /// Liveness: with n=4 (f=1), every view forms a QC.
    #[test]
    fn prop_every_view_forms_qc(views in 1usize..6) {
        let n = 4;
        let mut nodes: Vec<HotStuffNode> = (0..n as u64).map(|id| HotStuffNode::new(id, n)).collect();

        for v in 0..views {
            let ok = run_view(&mut nodes, format!("v{v}").into_bytes());
            prop_assert!(ok, "QC must form in every view with n=4");
        }
    }

    /// View counter monotonically increases across all nodes.
    #[test]
    fn prop_view_monotone(views in 1usize..6) {
        let n = 4;
        let mut nodes: Vec<HotStuffNode> = (0..n as u64).map(|id| HotStuffNode::new(id, n)).collect();

        let initial_view = nodes[0].view;
        for v in 0..views {
            run_view(&mut nodes, format!("v{v}").into_bytes());
        }

        for node in &nodes {
            prop_assert_eq!(node.view, initial_view + views as u64);
        }
    }

    /// Duplicate votes from same voter never inflate QC.
    #[test]
    fn prop_no_duplicate_voter_in_qc(f in 0usize..3) {
        let n = 3 * f + 1;
        if n == 0 { return Ok(()); }
        let mut collector = VoteCollector::new(n, f);
        let block_id = consensus::block::BlockId([42u8; 32]);

        // submit same voter 10 times
        for _ in 0..10 {
            collector.add(Vote { block_id: block_id.clone(), view: 1, voter: 0 });
        }

        // submit remaining voters to form QC
        let mut qc = None;
        for voter in 1..=(2 * f) as u64 {
            qc = collector.add(Vote { block_id: block_id.clone(), view: 1, voter });
        }

        if let Some(q) = qc {
            // no duplicates in signers
            let mut seen = std::collections::HashSet::new();
            for s in &q.signers {
                prop_assert!(seen.insert(s), "duplicate signer in QC");
            }
        }
    }
}
