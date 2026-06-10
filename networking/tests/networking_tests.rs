use networking::{
    flow_tracker::FlowTracker,
    metrics::MetricsSink,
};
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

fn addr(s: &str) -> SocketAddr {
    SocketAddr::from_str(s).unwrap()
}

// ── flow tracker ──────────────────────────────────────────────────────────────

#[test]
fn test_record_and_count_connections() {
    let mut tracker = FlowTracker::new();
    assert_eq!(tracker.active_count(), 0);

    tracker.record_connection(addr("10.0.0.1:9000"));
    tracker.record_connection(addr("10.0.0.2:9000"));
    assert_eq!(tracker.active_count(), 2);
}

#[test]
fn test_remove_connection() {
    let mut tracker = FlowTracker::new();
    let peer = addr("10.0.0.1:9000");
    tracker.record_connection(peer);
    tracker.remove(&peer);
    assert_eq!(tracker.active_count(), 0);
}

#[test]
fn test_bytes_rx_tx_accumulate() {
    let mut tracker = FlowTracker::new();
    let peer = addr("10.0.0.1:9000");
    tracker.record_connection(peer);

    tracker.record_rx(&peer, 1024);
    tracker.record_rx(&peer, 512);
    tracker.record_tx(&peer, 256);

    let flow = tracker.flows.get(&peer).unwrap();
    assert_eq!(flow.bytes_rx, 1536);
    assert_eq!(flow.bytes_tx, 256);
}

#[test]
fn test_unknown_peer_record_no_panic() {
    let mut tracker = FlowTracker::new();
    tracker.record_rx(&addr("1.2.3.4:8000"), 100);
    tracker.record_tx(&addr("1.2.3.4:8000"), 100);
    // no panic, no entry created
    assert_eq!(tracker.active_count(), 0);
}

// ── metrics sink ──────────────────────────────────────────────────────────────

#[test]
fn test_avg_rtt_empty() {
    let sink = MetricsSink::new();
    assert!(sink.avg_rtt().is_none());
    assert!(sink.p99_rtt().is_none());
}

#[test]
fn test_avg_rtt_single_sample() {
    let mut sink = MetricsSink::new();
    sink.record_latency("peer-1", Duration::from_millis(50));
    assert_eq!(sink.avg_rtt().unwrap(), Duration::from_millis(50));
}

#[test]
fn test_avg_rtt_multiple_samples() {
    let mut sink = MetricsSink::new();
    sink.record_latency("p", Duration::from_millis(10));
    sink.record_latency("p", Duration::from_millis(20));
    sink.record_latency("p", Duration::from_millis(30));
    assert_eq!(sink.avg_rtt().unwrap(), Duration::from_millis(20));
}

#[test]
fn test_p99_rtt() {
    let mut sink = MetricsSink::new();
    for ms in 1u64..=100 {
        sink.record_latency("p", Duration::from_millis(ms));
    }
    let p99 = sink.p99_rtt().unwrap();
    // p99 of [1..100ms] should be ≥ 98ms
    assert!(p99 >= Duration::from_millis(98));
}

#[test]
fn test_connection_counter() {
    let mut sink = MetricsSink::new();
    sink.record_connection("peer-a");
    sink.record_connection("peer-a");
    sink.record_connection("peer-b");
    assert_eq!(*sink.connection_counts.get("peer-a").unwrap(), 2);
    assert_eq!(*sink.connection_counts.get("peer-b").unwrap(), 1);
}
