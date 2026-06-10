use engine_sync::{SyncEngine, SyncEvent, SyncTarget};
use tokio::sync::mpsc;

fn target(name: &str, endpoint: &str) -> SyncTarget {
    SyncTarget {
        name: name.to_string(),
        endpoint: endpoint.to_string(),
        last_synced: 0,
    }
}

#[tokio::test]
async fn test_add_and_count_targets() {
    let (tx, _rx) = mpsc::channel(16);
    let mut engine = SyncEngine::new(tx);

    engine.add_target(target("reth", "http://localhost:8545"));
    engine.add_target(target("lighthouse", "http://localhost:5052"));
    assert_eq!(engine.target_count(), 2);
}

#[tokio::test]
async fn test_sync_all_emits_started_events() {
    let (tx, mut rx) = mpsc::channel(16);
    let mut engine = SyncEngine::new(tx);

    engine.add_target(target("reth", "http://localhost:8545"));
    engine.add_target(target("lighthouse", "http://localhost:5052"));
    engine.sync_all().await;

    let mut started = vec![];
    while let Ok(event) = rx.try_recv() {
        if let SyncEvent::Started { target } = event {
            started.push(target);
        }
    }
    assert_eq!(started.len(), 2);
}

#[tokio::test]
async fn test_report_block_known_target() {
    let (tx, mut rx) = mpsc::channel(16);
    let mut engine = SyncEngine::new(tx);
    engine.add_target(target("reth", "http://localhost:8545"));

    engine.report_block("reth", 100).await;

    let event = rx.try_recv().expect("should have event");
    match event {
        SyncEvent::BlockReceived { target, block_number } => {
            assert_eq!(target, "reth");
            assert_eq!(block_number, 100);
        }
        _ => panic!("expected BlockReceived"),
    }
}

#[tokio::test]
async fn test_report_block_unknown_target_no_event() {
    let (tx, mut rx) = mpsc::channel(16);
    let mut engine = SyncEngine::new(tx);
    // no targets registered

    engine.report_block("unknown", 50).await;
    assert!(rx.try_recv().is_err(), "unknown target must not emit events");
}

#[tokio::test]
async fn test_sync_simulation_block_sequence() {
    let (tx, mut rx) = mpsc::channel(64);
    let mut engine = SyncEngine::new(tx);
    engine.add_target(target("reth", "http://localhost:8545"));

    engine.sync_all().await;
    for block in 1u64..=10 {
        engine.report_block("reth", block).await;
    }

    let events: Vec<SyncEvent> = {
        let mut v = vec![];
        while let Ok(e) = rx.try_recv() {
            v.push(e);
        }
        v
    };

    let block_events: Vec<u64> = events
        .iter()
        .filter_map(|e| {
            if let SyncEvent::BlockReceived { block_number, .. } = e {
                Some(*block_number)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(block_events, (1u64..=10).collect::<Vec<_>>());
}
