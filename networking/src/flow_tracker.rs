use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

pub struct FlowEntry {
    pub addr: SocketAddr,
    pub connected_at: Instant,
    pub bytes_rx: u64,
    pub bytes_tx: u64,
}

pub struct FlowTracker {
    pub flows: HashMap<SocketAddr, FlowEntry>,
}

impl FlowTracker {
    pub fn new() -> Self {
        Self { flows: HashMap::new() }
    }

    pub fn record_connection(&mut self, addr: SocketAddr) {
        self.flows.insert(addr, FlowEntry {
            addr,
            connected_at: Instant::now(),
            bytes_rx: 0,
            bytes_tx: 0,
        });
    }

    pub fn record_rx(&mut self, addr: &SocketAddr, bytes: u64) {
        if let Some(e) = self.flows.get_mut(addr) {
            e.bytes_rx += bytes;
        }
    }

    pub fn record_tx(&mut self, addr: &SocketAddr, bytes: u64) {
        if let Some(e) = self.flows.get_mut(addr) {
            e.bytes_tx += bytes;
        }
    }

    pub fn remove(&mut self, addr: &SocketAddr) {
        self.flows.remove(addr);
    }

    pub fn active_count(&self) -> usize {
        self.flows.len()
    }
}

impl Default for FlowTracker {
    fn default() -> Self {
        Self::new()
    }
}
