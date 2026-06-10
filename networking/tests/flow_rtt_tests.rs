use networking::flow_tracker::FlowTracker;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

fn addr(s: &str) -> SocketAddr {
    SocketAddr::from_str(s).unwrap()
}

#[test]
fn test_rtt_probe_round_trip() {
    let mut tracker = FlowTracker::new();
    let peer = addr("10.0.0.1:9000");
    tracker.record_connection(peer);

    let seq = tracker.send_probe(&peer).expect("probe seq");
    let rtt = tracker.record_pong(&peer, seq).expect("should resolve RTT");
    assert!(rtt < Duration::from_secs(1), "RTT must be sub-second in test");
}

#[test]
fn test_stale_pong_ignored() {
    let mut tracker = FlowTracker::new();
    let peer = addr("10.0.0.2:9000");
    tracker.record_connection(peer);

    let seq = tracker.send_probe(&peer).unwrap();
    // wrong seq
    assert!(tracker.record_pong(&peer, seq + 99).is_none());
    // correct seq still works after stale rejection
    let rtt = tracker.record_pong(&peer, seq);
    assert!(rtt.is_some());
}

#[test]
fn test_avg_rtt_multiple_samples() {
    let mut tracker = FlowTracker::new();
    let peer = addr("10.0.0.3:9000");
    tracker.record_connection(peer);

    for _ in 0..5 {
        let seq = tracker.send_probe(&peer).unwrap();
        tracker.record_pong(&peer, seq).unwrap();
    }

    let flow = tracker.flows.get(&peer).unwrap();
    assert_eq!(flow.rtt_samples.len(), 5);
    assert!(flow.avg_rtt().is_some());
}

#[test]
fn test_rtt_samples_capped_at_64() {
    let mut tracker = FlowTracker::new();
    let peer = addr("10.0.0.4:9000");
    tracker.record_connection(peer);

    for _ in 0..80 {
        let seq = tracker.send_probe(&peer).unwrap();
        tracker.record_pong(&peer, seq).unwrap();
    }

    let flow = tracker.flows.get(&peer).unwrap();
    assert!(flow.rtt_samples.len() <= 64, "samples must be capped at 64");
}

#[test]
fn test_global_avg_rtt_across_flows() {
    let mut tracker = FlowTracker::new();
    for i in 0..3u8 {
        let peer = addr(&format!("10.0.0.{}:9000", i + 1));
        tracker.record_connection(peer);
        let seq = tracker.send_probe(&peer).unwrap();
        tracker.record_pong(&peer, seq).unwrap();
    }
    assert!(tracker.global_avg_rtt().is_some());
}

#[test]
fn test_probe_unknown_peer_returns_none() {
    let mut tracker = FlowTracker::new();
    assert!(tracker.send_probe(&addr("9.9.9.9:1234")).is_none());
}
