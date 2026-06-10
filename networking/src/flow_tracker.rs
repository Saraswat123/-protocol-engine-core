use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

// ── RTT probe ─────────────────────────────────────────────────────────────────

/// In-flight probe: ping sent, waiting for pong.
pub struct RttProbe {
    pub sent_at: Instant,
    pub seq: u64,
}

// ── per-flow state ────────────────────────────────────────────────────────────

pub struct FlowEntry {
    pub addr: SocketAddr,
    pub connected_at: Instant,
    pub bytes_rx: u64,
    pub bytes_tx: u64,
    /// Completed RTT samples (most recent first, capped at 64).
    pub rtt_samples: Vec<Duration>,
    /// Pending outgoing probe (seq → sent_at).
    pending_probe: Option<RttProbe>,
}

impl FlowEntry {
    fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            connected_at: Instant::now(),
            bytes_rx: 0,
            bytes_tx: 0,
            rtt_samples: Vec::new(),
            pending_probe: None,
        }
    }

    pub fn avg_rtt(&self) -> Option<Duration> {
        if self.rtt_samples.is_empty() { return None; }
        let total: Duration = self.rtt_samples.iter().sum();
        Some(total / self.rtt_samples.len() as u32)
    }

    pub fn min_rtt(&self) -> Option<Duration> {
        self.rtt_samples.iter().copied().min()
    }

    pub fn max_rtt(&self) -> Option<Duration> {
        self.rtt_samples.iter().copied().max()
    }

    pub fn uptime(&self) -> Duration {
        self.connected_at.elapsed()
    }
}

// ── tracker ───────────────────────────────────────────────────────────────────

pub struct FlowTracker {
    pub flows: HashMap<SocketAddr, FlowEntry>,
    next_seq: u64,
}

impl FlowTracker {
    pub fn new() -> Self {
        Self { flows: HashMap::new(), next_seq: 1 }
    }

    pub fn record_connection(&mut self, addr: SocketAddr) {
        self.flows.insert(addr, FlowEntry::new(addr));
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

    /// Send a latency probe; returns the sequence number to embed in the ping.
    pub fn send_probe(&mut self, addr: &SocketAddr) -> Option<u64> {
        let entry = self.flows.get_mut(addr)?;
        let seq = self.next_seq;
        self.next_seq += 1;
        entry.pending_probe = Some(RttProbe { sent_at: Instant::now(), seq });
        Some(seq)
    }

    /// Record pong response; resolves RTT for the matching probe.
    /// Returns measured RTT if seq matches, None if stale or unexpected.
    pub fn record_pong(&mut self, addr: &SocketAddr, seq: u64) -> Option<Duration> {
        let entry = self.flows.get_mut(addr)?;
        let probe = entry.pending_probe.take()?;
        if probe.seq != seq {
            // stale pong — put probe back, report nothing
            entry.pending_probe = Some(probe);
            return None;
        }
        let rtt = probe.sent_at.elapsed();
        entry.rtt_samples.insert(0, rtt);
        entry.rtt_samples.truncate(64);
        Some(rtt)
    }

    pub fn remove(&mut self, addr: &SocketAddr) {
        self.flows.remove(addr);
    }

    pub fn active_count(&self) -> usize {
        self.flows.len()
    }

    /// Aggregate average RTT across all tracked flows.
    pub fn global_avg_rtt(&self) -> Option<Duration> {
        let samples: Vec<Duration> = self.flows.values()
            .flat_map(|e| e.rtt_samples.iter().copied())
            .collect();
        if samples.is_empty() { return None; }
        let total: Duration = samples.iter().sum();
        Some(total / samples.len() as u32)
    }

    /// Flows idle (no RX/TX update) — does not track last-activity directly;
    /// callers use this to enumerate flows and cross-reference with MetricsSink.
    pub fn all_addrs(&self) -> impl Iterator<Item = &SocketAddr> {
        self.flows.keys()
    }
}

impl Default for FlowTracker {
    fn default() -> Self {
        Self::new()
    }
}
