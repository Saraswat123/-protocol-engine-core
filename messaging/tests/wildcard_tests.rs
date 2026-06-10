use messaging::{
    broker::{Broker, FilteredBroker, Message},
    filter::{SenderFilter, SizeFilter},
};

fn msg(topic: &str, sender: u64, payload: &[u8]) -> Message {
    Message { topic: topic.to_string(), sender, payload: payload.to_vec() }
}

// ── wildcard subscriptions ────────────────────────────────────────────────────

#[tokio::test]
async fn test_prefix_subscriber_receives_matching_topics() {
    let mut broker = Broker::new();
    let mut rx = broker.subscribe_prefix("blocks/");

    broker.publish(msg("blocks/mainnet", 0, b"data-1")).await;
    broker.publish(msg("blocks/holesky", 0, b"data-2")).await;
    broker.publish(msg("attestations/mainnet", 0, b"skip")).await;

    assert_eq!(rx.try_recv().unwrap().payload, b"data-1");
    assert_eq!(rx.try_recv().unwrap().payload, b"data-2");
    assert!(rx.try_recv().is_err(), "non-matching topic must not arrive");
}

#[tokio::test]
async fn test_exact_and_prefix_subscriber_both_receive() {
    let mut broker = Broker::new();
    let mut rx_exact = broker.subscribe("blocks/mainnet");
    let mut rx_prefix = broker.subscribe_prefix("blocks/");

    broker.publish(msg("blocks/mainnet", 1, b"payload")).await;

    assert!(rx_exact.try_recv().is_ok(), "exact subscriber must get message");
    assert!(rx_prefix.try_recv().is_ok(), "prefix subscriber must get message");
}

#[tokio::test]
async fn test_publish_count_tracked() {
    let mut broker = Broker::new();
    let _rx = broker.subscribe("tx");

    for _ in 0..5 {
        broker.publish(msg("tx", 0, b"x")).await;
    }
    assert_eq!(broker.total_published("tx"), 5);
    assert_eq!(broker.total_published("missing"), 0);
}

#[tokio::test]
async fn test_wildcard_count() {
    let mut broker = Broker::new();
    let _r1 = broker.subscribe_prefix("chain/");
    let _r2 = broker.subscribe_prefix("chain/");
    assert_eq!(broker.wildcard_subscriber_count(), 2);
}

#[tokio::test]
async fn test_prefix_does_not_match_shorter_topic() {
    let mut broker = Broker::new();
    let mut rx = broker.subscribe_prefix("blocks/mainnet/");

    // "blocks/mainnet" does NOT start with "blocks/mainnet/" (no trailing slash)
    broker.publish(msg("blocks/mainnet", 0, b"x")).await;
    assert!(rx.try_recv().is_err());
}

// ── filtered broker ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_filtered_broker_sender_filter() {
    let mut broker = FilteredBroker::new();
    let mut rx = broker.subscribe_filtered(
        "blocks",
        Box::new(SenderFilter { allowed: vec![42] }),
    );

    // allowed sender
    broker.publish(msg("blocks", 42, b"ok")).await;
    // blocked sender
    broker.publish(msg("blocks", 99, b"dropped")).await;

    assert_eq!(rx.try_recv().unwrap().payload, b"ok");
    assert!(rx.try_recv().is_err(), "message from blocked sender must not arrive");
}

#[tokio::test]
async fn test_filtered_broker_size_filter() {
    let mut broker = FilteredBroker::new();
    let mut rx = broker.subscribe_filtered(
        "data",
        Box::new(SizeFilter { max_bytes: 5 }),
    );

    broker.publish(msg("data", 0, b"hi")).await;
    broker.publish(msg("data", 0, b"this is way too large")).await;

    assert_eq!(rx.try_recv().unwrap().payload, b"hi");
    assert!(rx.try_recv().is_err(), "oversized message must be dropped");
}

#[tokio::test]
async fn test_filtered_broker_subscriber_count() {
    let mut broker = FilteredBroker::new();
    let _r1 = broker.subscribe_filtered("t", Box::new(SizeFilter { max_bytes: 100 }));
    let _r2 = broker.subscribe_filtered("t", Box::new(SizeFilter { max_bytes: 100 }));
    assert_eq!(broker.subscriber_count("t"), 2);
}
