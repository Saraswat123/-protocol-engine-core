use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, info};

pub type Topic = String;

#[derive(Debug, Clone)]
pub struct Message {
    pub topic: Topic,
    pub payload: Vec<u8>,
    pub sender: u64,
}

// ── subscription record ───────────────────────────────────────────────────────

struct Subscription {
    tx: mpsc::Sender<Message>,
    /// Optional prefix — if set, matches any topic that starts with this string.
    prefix: Option<String>,
}

impl Subscription {
    fn exact(tx: mpsc::Sender<Message>) -> Self {
        Self { tx, prefix: None }
    }

    fn wildcard(tx: mpsc::Sender<Message>, prefix: String) -> Self {
        Self { tx, prefix: Some(prefix) }
    }

    fn matches(&self, topic: &str, registered_topic: &str) -> bool {
        match &self.prefix {
            Some(p) => topic.starts_with(p.as_str()),
            None => topic == registered_topic,
        }
    }
}

// ── broker ────────────────────────────────────────────────────────────────────

pub struct Broker {
    /// exact-topic subscribers
    subscribers: HashMap<Topic, Vec<Subscription>>,
    /// wildcard subscribers keyed by prefix
    wildcard_subs: Vec<(String, mpsc::Sender<Message>)>,
    /// per-topic publish counter
    pub publish_counts: HashMap<Topic, u64>,
}

impl Broker {
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
            wildcard_subs: Vec::new(),
            publish_counts: HashMap::new(),
        }
    }

    /// Subscribe to an exact topic.
    pub fn subscribe(&mut self, topic: &str) -> mpsc::Receiver<Message> {
        let (tx, rx) = mpsc::channel(64);
        self.subscribers
            .entry(topic.to_string())
            .or_default()
            .push(Subscription::exact(tx));
        info!(topic, "new subscriber");
        rx
    }

    /// Subscribe to all topics whose name starts with `prefix`.
    /// e.g., `subscribe_prefix("blocks/")` matches `blocks/mainnet`, `blocks/holesky`.
    pub fn subscribe_prefix(&mut self, prefix: &str) -> mpsc::Receiver<Message> {
        let (tx, rx) = mpsc::channel(64);
        self.wildcard_subs.push((prefix.to_string(), tx));
        info!(prefix, "new wildcard subscriber");
        rx
    }

    /// Publish a message to all matching exact and wildcard subscribers.
    pub async fn publish(&mut self, msg: Message) {
        let topic = msg.topic.clone();
        *self.publish_counts.entry(topic.clone()).or_insert(0) += 1;

        // exact subscribers
        if let Some(subs) = self.subscribers.get_mut(&topic) {
            subs.retain(|sub| {
                matches!(
                    sub.tx.try_send(msg.clone()),
                    Ok(()) | Err(mpsc::error::TrySendError::Full(_))
                )
            });
            debug!(topic, subscribers = subs.len(), "published (exact)");
        }

        // wildcard subscribers
        self.wildcard_subs.retain(|(prefix, tx)| {
            if topic.starts_with(prefix.as_str()) {
                matches!(
                    tx.try_send(msg.clone()),
                    Ok(()) | Err(mpsc::error::TrySendError::Full(_))
                )
            } else {
                true // keep — topic didn't match, but sender may still be alive
            }
        });

        info!(topic, "published");
    }

    pub fn subscriber_count(&self, topic: &str) -> usize {
        self.subscribers.get(topic).map(|s| s.len()).unwrap_or(0)
    }

    pub fn wildcard_subscriber_count(&self) -> usize {
        self.wildcard_subs.len()
    }

    pub fn total_published(&self, topic: &str) -> u64 {
        *self.publish_counts.get(topic).unwrap_or(&0)
    }
}

impl Default for Broker {
    fn default() -> Self {
        Self::new()
    }
}

// ── filtered broker ───────────────────────────────────────────────────────────

use crate::filter::Filter;

/// A broker that applies a per-subscriber filter before delivery.
pub struct FilteredBroker {
    inner: Broker,
    filters: HashMap<Topic, Vec<Box<dyn Filter>>>,
}

impl FilteredBroker {
    pub fn new() -> Self {
        Self {
            inner: Broker::new(),
            filters: HashMap::new(),
        }
    }

    pub fn subscribe_filtered(
        &mut self,
        topic: &str,
        filter: Box<dyn Filter>,
    ) -> mpsc::Receiver<Message> {
        self.filters.entry(topic.to_string()).or_default().push(filter);
        self.inner.subscribe(topic)
    }

    pub async fn publish(&mut self, msg: Message) {
        let topic = msg.topic.clone();
        if let Some(filters) = self.filters.get(&topic) {
            if !filters.iter().all(|f| f.allow(&msg)) {
                debug!(topic, "message dropped by filter");
                return;
            }
        }
        self.inner.publish(msg).await;
    }

    pub fn subscriber_count(&self, topic: &str) -> usize {
        self.inner.subscriber_count(topic)
    }
}

impl Default for FilteredBroker {
    fn default() -> Self {
        Self::new()
    }
}
