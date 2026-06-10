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

pub struct Broker {
    subscribers: HashMap<Topic, Vec<mpsc::Sender<Message>>>,
}

impl Broker {
    pub fn new() -> Self {
        Self { subscribers: HashMap::new() }
    }

    pub fn subscribe(&mut self, topic: &str) -> mpsc::Receiver<Message> {
        let (tx, rx) = mpsc::channel(64);
        self.subscribers.entry(topic.to_string()).or_default().push(tx);
        info!(topic, "new subscriber");
        rx
    }

    pub async fn publish(&mut self, msg: Message) {
        let topic = msg.topic.clone();
        let subs = match self.subscribers.get_mut(&topic) {
            Some(s) => s,
            None => {
                debug!(topic, "no subscribers");
                return;
            }
        };
        // Remove dead senders as we publish
        subs.retain(|tx| {
            matches!(tx.try_send(msg.clone()), Ok(()) | Err(mpsc::error::TrySendError::Full(_)))
        });
        info!(topic, subscribers = subs.len(), "published");
    }

    pub fn subscriber_count(&self, topic: &str) -> usize {
        self.subscribers.get(topic).map(|s| s.len()).unwrap_or(0)
    }
}

impl Default for Broker {
    fn default() -> Self {
        Self::new()
    }
}
