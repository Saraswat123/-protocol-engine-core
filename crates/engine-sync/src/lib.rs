use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTarget {
    pub name: String,
    pub endpoint: String,
    pub last_synced: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncEvent {
    Started { target: String },
    BlockReceived { target: String, block_number: u64 },
    Completed { target: String, final_block: u64 },
    Failed { target: String, reason: String },
}

pub struct SyncEngine {
    targets: HashMap<String, SyncTarget>,
    event_tx: mpsc::Sender<SyncEvent>,
}

impl SyncEngine {
    pub fn new(event_tx: mpsc::Sender<SyncEvent>) -> Self {
        Self { targets: HashMap::new(), event_tx }
    }

    pub fn add_target(&mut self, target: SyncTarget) {
        info!(name = %target.name, endpoint = %target.endpoint, "registered sync target");
        self.targets.insert(target.name.clone(), target);
    }

    pub async fn sync_all(&self) {
        for target in self.targets.values() {
            let _ = self.event_tx.send(SyncEvent::Started {
                target: target.name.clone(),
            }).await;
            info!(target = %target.name, "sync started");
        }
    }

    pub async fn report_block(&self, target_name: &str, block_number: u64) {
        if self.targets.contains_key(target_name) {
            let _ = self.event_tx.send(SyncEvent::BlockReceived {
                target: target_name.to_string(),
                block_number,
            }).await;
        } else {
            warn!(target = target_name, "unknown sync target");
        }
    }

    pub fn target_count(&self) -> usize {
        self.targets.len()
    }
}
