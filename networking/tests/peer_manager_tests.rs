use networking::peer_monitor::PeerManager;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

fn addr(s: &str) -> SocketAddr {
    SocketAddr::from_str(s).unwrap()
}

// ── connection lifecycle ──────────────────────────────────────────────────────

#[test]
fn test_add_and_connect_peer() {
    let mut mgr = PeerManager::new(10);
    let a = addr("10.0.0.1:9000");

    assert!(mgr.add_peer(a));
    mgr.on_connected(a);

    assert_eq!(mgr.connected_count(), 1);
}

#[test]
fn test_disconnect_and_reconnect_logic() {
    let mut mgr = PeerManager::new(10);
    let a = addr("10.0.0.2:9000");
    mgr.add_peer(a);
    mgr.on_connected(a);
    mgr.on_disconnected(a);

    assert!(mgr.should_reconnect(&a), "should attempt reconnect after first disconnect");
    assert!(mgr.reconnect_delay(&a).is_some());
}

#[test]
fn test_reconnect_limit_exceeded() {
    let mut mgr = PeerManager::new(10);
    let a = addr("10.0.0.3:9000");
    mgr.add_peer(a);
    // exhaust reconnect limit
    for _ in 0..6 {
        mgr.on_connected(a);
        mgr.on_disconnected(a);
    }
    assert!(!mgr.should_reconnect(&a), "must stop reconnecting after limit");
}

#[test]
fn test_max_peers_respected() {
    let mut mgr = PeerManager::new(2);
    assert!(mgr.add_peer(addr("10.0.0.1:9000")));
    assert!(mgr.add_peer(addr("10.0.0.2:9000")));
    assert!(!mgr.add_peer(addr("10.0.0.3:9000")), "third peer must be rejected");
    assert_eq!(mgr.peer_count(), 2);
}

// ── scoring / banning ─────────────────────────────────────────────────────────

#[test]
fn test_invalid_messages_lower_score() {
    let mut mgr = PeerManager::new(10);
    let a = addr("10.0.0.4:9000");
    mgr.add_peer(a);
    mgr.on_connected(a);

    for _ in 0..5 {
        mgr.on_message_received(&a, false); // -10 each
    }
    assert!(mgr.is_banned(&a), "peer sending 5 bad messages (-50) must be banned");
}

#[test]
fn test_valid_messages_increase_score() {
    let mut mgr = PeerManager::new(10);
    let a = addr("10.0.0.5:9000");
    mgr.add_peer(a);
    mgr.on_connected(a);

    for _ in 0..10 {
        mgr.on_message_received(&a, true);
    }
    assert!(!mgr.is_banned(&a), "good peer must not be banned");
}

#[test]
fn test_banned_peer_not_reconnected() {
    let mut mgr = PeerManager::new(10);
    let a = addr("10.0.0.6:9000");
    mgr.add_peer(a);
    mgr.on_connected(a);
    for _ in 0..5 {
        mgr.on_message_received(&a, false);
    }
    mgr.on_disconnected(a);
    assert!(!mgr.should_reconnect(&a), "banned peer must not be reconnected");
}

// ── reconnect backoff ─────────────────────────────────────────────────────────

#[test]
fn test_exponential_backoff_grows() {
    let mut mgr = PeerManager::new(10);
    let a = addr("10.0.0.7:9000");
    mgr.add_peer(a);

    let mut delays = vec![];
    for _ in 0..4 {
        mgr.on_connected(a);
        mgr.on_disconnected(a);
        delays.push(mgr.reconnect_delay(&a).unwrap());
    }
    // each delay should be >= previous (exponential)
    for w in delays.windows(2) {
        assert!(w[1] >= w[0], "backoff must grow: {:?} >= {:?}", w[1], w[0]);
    }
}

#[test]
fn test_backoff_capped_at_5_minutes() {
    let mut mgr = PeerManager::new(10);
    let a = addr("10.0.0.8:9000");
    mgr.add_peer(a);
    // many disconnects
    for _ in 0..20 {
        mgr.on_connected(a);
        mgr.on_disconnected(a);
    }
    let delay = mgr.reconnect_delay(&a).unwrap();
    assert!(delay <= Duration::from_secs(300), "backoff must be capped at 5 min");
}

// ── eviction ──────────────────────────────────────────────────────────────────

#[test]
fn test_remove_peer() {
    let mut mgr = PeerManager::new(10);
    let a = addr("10.0.0.9:9000");
    mgr.add_peer(a);
    mgr.remove_peer(&a);
    assert_eq!(mgr.peer_count(), 0);
}

#[test]
fn test_connected_peers_iterator() {
    let mut mgr = PeerManager::new(10);
    let a = addr("10.0.0.10:9000");
    let b = addr("10.0.0.11:9000");
    mgr.add_peer(a);
    mgr.add_peer(b);
    mgr.on_connected(a);
    // b stays Connecting

    let connected: Vec<_> = mgr.connected_peers().collect();
    assert_eq!(connected.len(), 1);
    assert_eq!(connected[0].addr, a);
}
