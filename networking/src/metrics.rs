use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct LatencySample {
    pub peer: String,
    pub rtt: Duration,
    pub recorded_at: Instant,
}

pub struct MetricsSink {
    pub samples: Vec<LatencySample>,
    pub connection_counts: HashMap<String, u64>,
}

impl MetricsSink {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            connection_counts: HashMap::new(),
        }
    }

    pub fn record_latency(&mut self, peer: &str, rtt: Duration) {
        self.samples.push(LatencySample {
            peer: peer.to_string(),
            rtt,
            recorded_at: Instant::now(),
        });
    }

    pub fn record_connection(&mut self, peer: &str) {
        *self.connection_counts.entry(peer.to_string()).or_insert(0) += 1;
    }

    pub fn avg_rtt(&self) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }
        let total: Duration = self.samples.iter().map(|s| s.rtt).sum();
        Some(total / self.samples.len() as u32)
    }

    pub fn p99_rtt(&self) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }
        let mut rtts: Vec<Duration> = self.samples.iter().map(|s| s.rtt).collect();
        rtts.sort();
        let idx = (rtts.len() as f64 * 0.99) as usize;
        Some(rtts[idx.min(rtts.len() - 1)])
    }
}

impl Default for MetricsSink {
    fn default() -> Self {
        Self::new()
    }
}
