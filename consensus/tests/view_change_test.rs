/// View-change / timeout protocol tests.
///
/// Scenario: leader for view V fails to propose. Honest replicas detect the
/// timeout, send NewViewMessage to the view-(V+1) leader, which collects
/// 2f+1 messages, adopts the highest QC, and continues the protocol.
use consensus::{
    node::HotStuffNode,
    vote::{NewViewCollector, NewViewMessage, Vote},
};

const N: usize = 4; // n=4, f=1, threshold=3

fn cluster() -> Vec<HotStuffNode> {
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

// ── on_timeout ────────────────────────────────────────────────────────────────

/// on_timeout increments view and produces correct NewViewMessage.
#[test]
fn test_on_timeout_increments_view() {
    let mut node = HotStuffNode::new(0, N);
    assert_eq!(node.view, 1);

    let nv = node.on_timeout();
    assert_eq!(nv.sender, 0);
    assert_eq!(nv.view, 1,  "message carries the view being abandoned");
    assert_eq!(node.view, 2, "node advances to next view");
}

#[test]
fn test_on_timeout_carries_current_high_qc() {
    let mut nodes = cluster();
    // run 1 honest view so nodes have non-genesis high_qc
    run_honest_view(&mut nodes, b"v1".to_vec());

    let pre_high_qc_view = nodes[0].high_qc.view;
    let nv = nodes[0].on_timeout();
    assert_eq!(nv.high_qc.view, pre_high_qc_view, "high_qc propagated in NewView msg");
}

// ── NewViewCollector ──────────────────────────────────────────────────────────

/// Collector returns None until 2f+1=3 messages arrive for the same view.
#[test]
fn test_new_view_collector_threshold() {
    let mut collector = NewViewCollector::new(N, 1);
    let genesis_qc = consensus::vote::QuorumCertificate::genesis();

    for sender in 0..2u64 {
        let result = collector.add(NewViewMessage {
            sender,
            view: 1,
            high_qc: genesis_qc.clone(),
        });
        assert!(result.is_none(), "no quorum with {sender}+1 messages");
    }
    // third message → quorum
    let result = collector.add(NewViewMessage { sender: 2, view: 1, high_qc: genesis_qc });
    assert!(result.is_some(), "quorum must form at 3/4");
}

/// Collector picks the highest QC across all NewView messages.
#[test]
fn test_new_view_collector_picks_highest_qc() {
    let mut nodes = cluster();

    // run 2 honest views so different nodes have different high_qc values
    run_honest_view(&mut nodes, b"v1".to_vec());
    // Only advance some nodes to view 2, others stay at view 1 (simulate partition)
    // Simulate: nodes[0..2] have qc.view=1, nodes[2..3] still have genesis qc
    // We'll fake it by directly constructing messages

    let high_qc_v1 = nodes[0].high_qc.clone(); // view=1
    let genesis_qc = consensus::vote::QuorumCertificate::genesis(); // view=0

    let mut collector = NewViewCollector::new(N, 1);
    collector.add(NewViewMessage { sender: 0, view: 2, high_qc: genesis_qc.clone() });
    collector.add(NewViewMessage { sender: 1, view: 2, high_qc: genesis_qc.clone() });
    let best = collector.add(NewViewMessage { sender: 2, view: 2, high_qc: high_qc_v1.clone() });

    let best = best.expect("quorum must form");
    assert_eq!(best.view, high_qc_v1.view, "must pick highest QC (view=1 > view=0)");
}

/// Duplicate sender is ignored — does not inflate count.
#[test]
fn test_new_view_collector_dedup() {
    let mut collector = NewViewCollector::new(N, 1);
    let genesis_qc = consensus::vote::QuorumCertificate::genesis();
    let msg = NewViewMessage { sender: 0, view: 1, high_qc: genesis_qc };

    assert!(collector.add(msg.clone()).is_none());
    assert!(collector.add(msg.clone()).is_none()); // duplicate — ignored
    assert_eq!(collector.count(1), 1, "duplicate must not increment count");
}

// ── view-change scenario ──────────────────────────────────────────────────────

/// Leader for view V is silent. Replicas call on_timeout and produce
/// NewViewMessages. Next leader collects quorum, adopts highest QC,
/// proposes and drives the chain forward.
#[test]
fn test_view_change_after_silent_leader() {
    let mut nodes = cluster();

    // view 1, leader=1 (1 % 4 = 1). Leader stays silent.
    // Replicas 0, 2, 3 timeout and produce view-change messages.
    let silent_view = nodes[0].view;
    assert_eq!(nodes[0].leader_for_view(silent_view), 1);

    // replicas 0, 2, 3 time out
    let nv0 = nodes[0].on_timeout();
    let nv2 = nodes[2].on_timeout();
    let nv3 = nodes[3].on_timeout();

    // all three are now in view 2
    assert_eq!(nodes[0].view, 2);
    assert_eq!(nodes[2].view, 2);
    assert_eq!(nodes[3].view, 2);

    // next leader = leader_for_view(2) = 2
    let next_leader = nodes[0].leader_for_view(2);
    assert_eq!(next_leader, 2);

    // node 1 (silent leader) also needs to catch up — timeout it too
    let nv1 = nodes[1].on_timeout();
    assert_eq!(nodes[1].view, 2);

    // next leader (node 2) collects NewView messages
    let mut collector = NewViewCollector::new(N, 1);
    let best = [nv0, nv1, nv2, nv3]
        .into_iter()
        .find_map(|nv| collector.add(nv))
        .expect("quorum must form with 4 NewView messages");

    // adopt best QC (genesis — no previous views committed)
    if best.view >= nodes[2].high_qc.view {
        nodes[2].high_qc = best;
    }

    // node 2 is now in view 2 (timed out above), proposes
    let block = nodes[2].propose(b"view-2-recovery".to_vec())
        .expect("next leader must propose");

    // all nodes vote
    let votes: Vec<_> = nodes.iter().filter_map(|n| n.on_proposal(&block)).collect();
    assert_eq!(votes.len(), N, "all 4 nodes must vote on recovery block");

    let mut qc = None;
    for vote in votes {
        qc = nodes[2].on_vote(vote);
        if qc.is_some() { break; }
    }
    assert!(qc.is_some(), "QC must form after view-change recovery");
}

/// After a view-change, the chain continues normally for multiple rounds.
#[test]
fn test_chain_continues_after_view_change() {
    let mut nodes = cluster();

    // view 1: leader 1 silent → all nodes timeout
    for node in nodes.iter_mut() {
        node.on_timeout(); // advances to view 2
    }

    // view 2 onward: honest operation
    assert!(run_honest_view(&mut nodes, b"v2".to_vec()), "view 2 must succeed after view-change");
    assert!(run_honest_view(&mut nodes, b"v3".to_vec()), "view 3 must succeed");

    // by 2-chain rule: view-2 block commits after view-3 QC
    for node in &nodes {
        assert!(
            !node.committed.is_empty(),
            "node {} must have committed after recovery",
            node.id
        );
    }
}

/// Safety invariant: nodes that timed out and nodes that didn't both
/// eventually commit the same block (no fork).
#[test]
fn test_no_safety_violation_across_view_change() {
    let mut nodes = cluster();

    // nodes 0,1 timeout view 1; nodes 2,3 get a proposal but no QC
    let nv0 = nodes[0].on_timeout();
    let nv1 = nodes[1].on_timeout();

    // nodes 2,3 also advance (simulate they got stuck too)
    let nv2 = nodes[2].on_timeout();
    let nv3 = nodes[3].on_timeout();

    // view 2: node 2 is leader, adopts best QC from NewView msgs
    let mut collector = NewViewCollector::new(N, 1);
    let best = [nv0, nv1, nv2, nv3]
        .into_iter()
        .find_map(|nv| collector.add(nv))
        .unwrap();
    nodes[2].high_qc = best;

    // two honest views from view 2
    run_honest_view(&mut nodes, b"recovery-1".to_vec());
    run_honest_view(&mut nodes, b"recovery-2".to_vec());

    // all committed blocks must be identical across nodes
    let reference = nodes[0].committed.iter().map(|b| b.id.clone()).collect::<Vec<_>>();
    for node in &nodes[1..] {
        let committed = node.committed.iter().map(|b| b.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            committed, reference,
            "node {} committed different blocks — safety violation",
            node.id
        );
    }
}
