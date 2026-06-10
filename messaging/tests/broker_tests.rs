use messaging::{
    broker::{Broker, Message},
    filter::{ChainFilter, Filter, SenderFilter, SizeFilter},
};

fn msg(topic: &str, sender: u64, payload: &[u8]) -> Message {
    Message {
        topic: topic.to_string(),
        sender,
        payload: payload.to_vec(),
    }
}

// ── broker ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_subscribe_and_receive() {
    let mut broker = Broker::new();
    let mut rx = broker.subscribe("blocks");

    broker.publish(msg("blocks", 0, b"block-1")).await;

    let received = rx.try_recv().expect("should have message");
    assert_eq!(received.payload, b"block-1");
    assert_eq!(received.topic, "blocks");
}

#[tokio::test]
async fn test_multiple_subscribers_same_topic() {
    let mut broker = Broker::new();
    let mut rx1 = broker.subscribe("attestations");
    let mut rx2 = broker.subscribe("attestations");

    broker.publish(msg("attestations", 1, b"att-data")).await;

    assert_eq!(rx1.try_recv().unwrap().payload, b"att-data");
    assert_eq!(rx2.try_recv().unwrap().payload, b"att-data");
}

#[tokio::test]
async fn test_no_cross_topic_delivery() {
    let mut broker = Broker::new();
    let mut rx_blocks = broker.subscribe("blocks");
    let mut _rx_txs = broker.subscribe("transactions");

    broker.publish(msg("transactions", 0, b"tx-data")).await;

    assert!(rx_blocks.try_recv().is_err(), "blocks subscriber must not get tx message");
}

#[tokio::test]
async fn test_subscriber_count() {
    let mut broker = Broker::new();
    assert_eq!(broker.subscriber_count("blocks"), 0);

    let _rx1 = broker.subscribe("blocks");
    let _rx2 = broker.subscribe("blocks");
    assert_eq!(broker.subscriber_count("blocks"), 2);
}

#[tokio::test]
async fn test_publish_no_subscribers_no_panic() {
    let mut broker = Broker::new();
    broker.publish(msg("orphan-topic", 0, b"data")).await;
    // should not panic
}

#[tokio::test]
async fn test_dead_subscriber_removed() {
    let mut broker = Broker::new();
    {
        let _rx = broker.subscribe("blocks"); // drops at end of block
    }
    // publish should not panic and dead sender should be pruned
    broker.publish(msg("blocks", 0, b"data")).await;
    assert_eq!(broker.subscriber_count("blocks"), 0);
}

// ── filters ───────────────────────────────────────────────────────────────────

#[test]
fn test_sender_filter_allow() {
    let f = SenderFilter { allowed: vec![1, 2, 3] };
    assert!(f.allow(&msg("t", 1, b"")));
    assert!(!f.allow(&msg("t", 99, b"")));
}

#[test]
fn test_size_filter() {
    let f = SizeFilter { max_bytes: 10 };
    assert!(f.allow(&msg("t", 0, b"hello")));
    assert!(!f.allow(&msg("t", 0, b"this is too long payload")));
}

#[test]
fn test_chain_filter_all_must_pass() {
    let f = ChainFilter {
        filters: vec![
            Box::new(SenderFilter { allowed: vec![1] }),
            Box::new(SizeFilter { max_bytes: 10 }),
        ],
    };
    assert!(f.allow(&msg("t", 1, b"ok")));
    assert!(!f.allow(&msg("t", 2, b"ok")));           // wrong sender
    assert!(!f.allow(&msg("t", 1, b"too long payload here"))); // too big
}
