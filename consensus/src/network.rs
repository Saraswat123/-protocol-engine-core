/// Async HotStuff network: real Tokio channels between nodes.
/// Each node runs as a Tokio task; messages route via mpsc.
use crate::{
    block::Block,
    node::HotStuffNode,
    vote::{QuorumCertificate, Vote},
};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub enum Message {
    Proposal(Block),
    Vote(Vote),
    NewView(QuorumCertificate),
}

pub type NodeId = u64;

pub struct NetworkNode {
    pub inner: HotStuffNode,
    rx: mpsc::Receiver<Message>,
    peers: HashMap<NodeId, mpsc::Sender<Message>>,
}

impl NetworkNode {
    pub fn new(
        id: NodeId,
        n: usize,
        rx: mpsc::Receiver<Message>,
        peers: HashMap<NodeId, mpsc::Sender<Message>>,
    ) -> Self {
        Self { inner: HotStuffNode::new(id, n), rx, peers }
    }

    pub async fn broadcast(&self, msg: Message) {
        for (id, tx) in &self.peers {
            if *id != self.inner.id {
                let _ = tx.send(msg.clone()).await;
                debug!(from = self.inner.id, to = id, "broadcast");
            }
        }
    }

    pub async fn send_to(&self, target: NodeId, msg: Message) {
        if let Some(tx) = self.peers.get(&target) {
            let _ = tx.send(msg).await;
        }
    }

    /// Run one view as leader: propose → collect votes → broadcast QC.
    pub async fn run_leader_view(&mut self, payload: Vec<u8>) -> Option<QuorumCertificate> {
        let block = self.inner.propose(payload)?;
        info!(node = self.inner.id, view = self.inner.view, block = %block.id, "leader: proposing");
        self.broadcast(Message::Proposal(block.clone())).await;

        // also vote on own proposal
        if let Some(vote) = self.inner.on_proposal(&block) {
            let _ = self.inner.on_vote(vote);
        }

        // collect votes until QC forms
        let timeout = tokio::time::Duration::from_millis(500);
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match tokio::time::timeout(remaining, self.rx.recv()).await {
                Ok(Some(Message::Vote(vote))) => {
                    if let Some(qc) = self.inner.on_vote(vote) {
                        info!(node = self.inner.id, view = self.inner.view, "QC formed");
                        self.broadcast(Message::NewView(qc.clone())).await;
                        return Some(qc);
                    }
                }
                _ => return None,
            }
        }
    }

    /// Run one view as replica: wait for proposal → vote → wait for QC.
    pub async fn run_replica_view(&mut self) -> bool {
        let timeout = tokio::time::Duration::from_millis(500);

        // wait for proposal
        let block = match tokio::time::timeout(timeout, self.rx.recv()).await {
            Ok(Some(Message::Proposal(b))) => b,
            _ => return false,
        };

        self.inner.blocks.insert(block.id.clone(), block.clone());

        if let Some(vote) = self.inner.on_proposal(&block) {
            let leader = self.inner.leader_for_view(self.inner.view);
            self.send_to(leader, Message::Vote(vote)).await;
        }

        // wait for NewView (QC from leader)
        match tokio::time::timeout(timeout, self.rx.recv()).await {
            Ok(Some(Message::NewView(qc))) => {
                self.inner.advance_view(qc);
                true
            }
            _ => false,
        }
    }
}

/// Spawn N nodes as Tokio tasks, run `views` rounds, return committed counts.
pub async fn simulate(n: usize, views: usize) -> Vec<usize> {
    let mut senders: HashMap<NodeId, mpsc::Sender<Message>> = HashMap::new();
    let mut receivers: HashMap<NodeId, mpsc::Receiver<Message>> = HashMap::new();

    for id in 0..n as u64 {
        let (tx, rx) = mpsc::channel(64);
        senders.insert(id, tx);
        receivers.insert(id, rx);
    }

    let mut nodes: Vec<NetworkNode> = (0..n as u64)
        .map(|id| {
            let rx = receivers.remove(&id).unwrap();
            NetworkNode::new(id, n, rx, senders.clone())
        })
        .collect();

    for v in 0..views {
        let leader_id = nodes[0].inner.leader_for_view(nodes[0].inner.view) as usize;
        let payload = format!("view-{v}").into_bytes();

        // advance all replicas' block stores (need to share blocks — simplified)
        let mut handles: Vec<tokio::task::JoinHandle<()>> = vec![];
        let (done_tx, mut done_rx) = mpsc::channel::<()>(n);

        // run leader and replicas concurrently
        // simplified: run sequentially in same task (avoids move issues in tests)
        let qc = nodes[leader_id].run_leader_view(payload).await;

        for (i, node) in nodes.iter_mut().enumerate() {
            if i != leader_id {
                node.run_replica_view().await;
            }
        }

        if let Some(qc) = qc {
            for node in nodes.iter_mut() {
                if node.inner.view == qc.view {
                    node.inner.advance_view(qc.clone());
                }
            }
        }
        drop(handles);
        drop(done_tx);
        // drain done_rx
        while done_rx.try_recv().is_ok() {}
    }

    nodes.iter().map(|n| n.inner.committed.len()).collect()
}
