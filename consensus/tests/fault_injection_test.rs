/// Fault injection tests: simulate message drops, delays, duplicates, and
/// invalid votes arriving at the leader mid-view.
use consensus::{
    block::Block,
    node::HotStuffNode,
    vote::{Vote, VoteCollector},
};

const N: usize = 4;

fn cluster() -> Vec<HotStuffNode> {
    (0..N as u64).map(|id| HotStuffNode::new(id, N)).collect()
}

fn run_view(nodes: &mut Vec<HotStuffNode>, payload: Vec<u8>) -> bool {
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

/// Drop every other vote: only 2 of 4 arrive at leader.
/// 2 < 2f+1=3 → no QC.
#[test]
fn test_dropped_votes_no_qc() {
    let mut nodes = cluster();
    let leader_id = nodes[0].leader_for_view(nodes[0].view) as usize;
    let block = nodes[leader_id].propose(b"drop-test".to_vec()).unwrap();

    // Only nodes 0,1 deliver votes (nodes 2,3 messages dropped)
    let delivered: Vec<Vote> = nodes[..2]
        .iter()
        .filter_map(|n| n.on_proposal(&block))
        .collect();

    let mut qc = None;
    for vote in delivered {
        qc = nodes[leader_id].on_vote(vote);
    }
    assert!(qc.is_none(), "2 delivered votes < threshold 3, no QC");
}

/// All 4 votes arrive but one is a duplicate (same voter, re-sent).
/// Dedup must prevent double-counting: still only 4 distinct voters → QC forms.
#[test]
fn test_duplicate_vote_not_double_counted() {
    let mut nodes = cluster();
    let leader_id = nodes[0].leader_for_view(nodes[0].view) as usize;
    let block = nodes[leader_id].propose(b"dup-test".to_vec()).unwrap();

    let mut votes: Vec<Vote> = nodes.iter().filter_map(|n| n.on_proposal(&block)).collect();
    // inject duplicate of vote[0] at the end
    votes.push(votes[0].clone());

    let mut collector = VoteCollector::new(N, 1);
    let mut qc_count = 0;
    for vote in votes {
        if collector.add(vote).is_some() {
            qc_count += 1;
        }
    }
    // QC should form exactly once (not twice due to duplicate)
    assert_eq!(qc_count, 1, "QC must form exactly once despite duplicate vote");
}

/// Votes arrive in reverse order (delayed delivery). QC still forms at threshold.
#[test]
fn test_out_of_order_votes_qc_forms() {
    let mut nodes = cluster();
    let leader_id = nodes[0].leader_for_view(nodes[0].view) as usize;
    let block = nodes[leader_id].propose(b"reorder".to_vec()).unwrap();

    let mut votes: Vec<Vote> = nodes.iter().filter_map(|n| n.on_proposal(&block)).collect();
    votes.reverse(); // simulate LIFO network delivery

    let mut qc = None;
    for vote in votes {
        let result = nodes[leader_id].on_vote(vote);
        if result.is_some() { qc = result; }
    }
    assert!(qc.is_some(), "QC must form regardless of vote arrival order");
}

/// Replay attack: leader receives the same QC-worth of votes twice.
/// Second round of identical votes must NOT produce a second QC.
#[test]
fn test_vote_replay_idempotent() {
    let mut nodes = cluster();
    let leader_id = nodes[0].leader_for_view(nodes[0].view) as usize;
    let block = nodes[leader_id].propose(b"replay".to_vec()).unwrap();

    let votes: Vec<Vote> = nodes.iter().filter_map(|n| n.on_proposal(&block)).collect();

    let mut collector = VoteCollector::new(N, 1);
    let mut qc_count = 0;

    // first pass — should get exactly 1 QC
    for vote in votes.iter().cloned() {
        if collector.add(vote).is_some() { qc_count += 1; }
    }
    // second pass — all duplicates, collector must reject them
    for vote in votes.iter().cloned() {
        if collector.add(vote).is_some() { qc_count += 1; }
    }
    assert_eq!(qc_count, 1, "replayed votes must not produce second QC");
}

/// Wrong-view vote injected mid-collection: must be silently ignored.
/// Honest nodes still produce a valid QC.
#[test]
fn test_wrong_view_vote_ignored() {
    let mut nodes = cluster();
    let leader_id = nodes[0].leader_for_view(nodes[0].view) as usize;
    let block = nodes[leader_id].propose(b"stale-vote".to_vec()).unwrap();

    let votes: Vec<Vote> = nodes.iter().filter_map(|n| n.on_proposal(&block)).collect();

    // stale vote from a past view
    let stale = Vote {
        voter: 99,
        view: 0,
        block_id: block.id.clone(),
    };

    let mut collector = VoteCollector::new(N, 1);
    collector.add(stale); // inject stale vote

    let mut qc = None;
    for vote in votes {
        let r = collector.add(vote);
        if r.is_some() { qc = r; }
    }
    assert!(qc.is_some(), "honest QC must still form despite stale-view injection");
}

/// Two-view progress with fault: view 1 has one dropped message, view 2 is clean.
/// Block from view 1 must eventually commit at view 2 boundary.
#[test]
fn test_recovery_after_degraded_view() {
    let mut nodes = cluster();

    // view 1: one vote missing (just barely forms QC with 3)
    {
        let leader_id = nodes[0].leader_for_view(nodes[0].view) as usize;
        let block = nodes[leader_id].propose(b"degraded".to_vec()).unwrap();
        // only 3 of 4 votes (node 3 dropped)
        let votes: Vec<Vote> = nodes[..3].iter().filter_map(|n| n.on_proposal(&block)).collect();
        let mut qc = None;
        for vote in votes {
            qc = nodes[leader_id].on_vote(vote);
            if qc.is_some() { break; }
        }
        let qc = qc.expect("3/4 votes should form QC");
        for node in nodes.iter_mut() {
            node.blocks.insert(block.id.clone(), block.clone());
            node.advance_view(qc.clone());
        }
    }

    // view 2: fully honest
    assert!(run_view(&mut nodes, b"recovery".to_vec()), "view 2 must succeed");

    // By 2-chain rule: view-1 block commits after view-2 QC
    for node in &nodes {
        assert!(
            !node.committed.is_empty(),
            "node {} must have committed after recovery",
            node.id
        );
    }
}
