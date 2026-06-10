use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, info, warn};

// ── peer state machine ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum PeerState {
    Connecting,
    Connected,
    Disconnected,
    Banned,
}

#[derive(Debug, Clone)]
pub struct PeerRecord {
    pub addr: SocketAddr,
    pub state: PeerState,
    pub connected_at: Option<Instant>,
    pub last_seen: Instant,
    pub reconnect_attempts: u32,
    pub score: i32,
    pub messages_rx: u64,
    pub messages_tx: u64,
}

impl PeerRecord {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            state: PeerState::Connecting,
            connected_at: None,
            last_seen: Instant::now(),
            reconnect_attempts: 0,
            score: 0,
            messages_rx: 0,
            messages_tx: 0,
        }
    }

    /// Time connected, or zero if not yet connected.
    pub fn uptime(&self) -> Duration {
        self.connected_at
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO)
    }
}

// ── scoring constants ─────────────────────────────────────────────────────────

const SCORE_GOOD_MESSAGE: i32 = 1;
const SCORE_BAD_MESSAGE: i32 = -10;
const SCORE_TIMEOUT: i32 = -5;
const BAN_THRESHOLD: i32 = -50;

// ── peer manager ─────────────────────────────────────────────────────────────

pub struct PeerManager {
    peers: HashMap<SocketAddr, PeerRecord>,
    max_peers: usize,
    reconnect_limit: u32,
    reconnect_backoff_base: Duration,
}

impl PeerManager {
    pub fn new(max_peers: usize) -> Self {
        Self {
            peers: HashMap::new(),
            max_peers,
            reconnect_limit: 5,
            reconnect_backoff_base: Duration::from_secs(2),
        }
    }

    pub fn add_peer(&mut self, addr: SocketAddr) -> bool {
        if self.peers.len() >= self.max_peers {
            warn!(%addr, "peer limit reached, rejecting");
            return false;
        }
        self.peers.entry(addr).or_insert_with(|| PeerRecord::new(addr));
        true
    }

    pub fn on_connected(&mut self, addr: SocketAddr) {
        if let Some(p) = self.peers.get_mut(&addr) {
            p.state = PeerState::Connected;
            p.connected_at = Some(Instant::now());
            p.last_seen = Instant::now();
            // do not reset reconnect_attempts — cumulative counter for limit enforcement
            info!(%addr, "peer connected");
        }
    }

    pub fn on_disconnected(&mut self, addr: SocketAddr) {
        if let Some(p) = self.peers.get_mut(&addr) {
            p.state = PeerState::Disconnected;
            p.reconnect_attempts += 1;
            warn!(%addr, attempts = p.reconnect_attempts, "peer disconnected");
        }
    }

    pub fn on_message_received(&mut self, addr: &SocketAddr, valid: bool) {
        if let Some(p) = self.peers.get_mut(addr) {
            p.messages_rx += 1;
            p.last_seen = Instant::now();
            if valid {
                p.score += SCORE_GOOD_MESSAGE;
            } else {
                p.score += SCORE_BAD_MESSAGE;
                if p.score <= BAN_THRESHOLD {
                    p.state = PeerState::Banned;
                    warn!(addr = %p.addr, score = p.score, "peer banned");
                }
            }
        }
    }

    pub fn on_timeout(&mut self, addr: &SocketAddr) {
        if let Some(p) = self.peers.get_mut(addr) {
            p.score += SCORE_TIMEOUT;
            warn!(%addr, score = p.score, "peer timeout");
        }
    }

    /// Backoff duration before next reconnect attempt (exponential, capped at 5 min).
    pub fn reconnect_delay(&self, addr: &SocketAddr) -> Option<Duration> {
        self.peers.get(addr).map(|p| {
            let exp = self.reconnect_backoff_base
                .checked_mul(2u32.saturating_pow(p.reconnect_attempts))
                .unwrap_or(Duration::from_secs(300));
            exp.min(Duration::from_secs(300))
        })
    }

    pub fn should_reconnect(&self, addr: &SocketAddr) -> bool {
        self.peers.get(addr).map_or(false, |p| {
            p.state == PeerState::Disconnected
                && p.reconnect_attempts < self.reconnect_limit
                && p.score > BAN_THRESHOLD
        })
    }

    pub fn remove_peer(&mut self, addr: &SocketAddr) {
        self.peers.remove(addr);
    }

    pub fn connected_peers(&self) -> impl Iterator<Item = &PeerRecord> {
        self.peers.values().filter(|p| p.state == PeerState::Connected)
    }

    pub fn connected_count(&self) -> usize {
        self.connected_peers().count()
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn is_banned(&self, addr: &SocketAddr) -> bool {
        self.peers.get(addr).map_or(false, |p| p.state == PeerState::Banned)
    }

    /// Evict peers idle longer than `idle_threshold`.
    pub fn evict_idle(&mut self, idle_threshold: Duration) {
        self.peers.retain(|_, p| {
            if p.state == PeerState::Connected && p.last_seen.elapsed() > idle_threshold {
                info!(addr = %p.addr, "evicting idle peer");
                false
            } else {
                true
            }
        });
    }
}

// ── raw TCP peer handler (low-level, used by networking binary) ───────────────

pub async fn handle_peer(mut stream: TcpStream, addr: SocketAddr) {
    let mut buf = vec![0u8; 4096];
    loop {
        match stream.read(&mut buf).await {
            Ok(0) => {
                info!(peer = %addr, "disconnected");
                break;
            }
            Ok(n) => {
                debug!(peer = %addr, bytes = n, "received");
                let _ = stream.write_all(&buf[..n]).await;
            }
            Err(e) => {
                debug!(peer = %addr, err = %e, "read error");
                break;
            }
        }
    }
}
