//! 进程内事件总线，用于模块间解耦通信。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub struct Event {
    pub topic: String,
    pub payload: serde_json::Value,
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
    _subs: Arc<Mutex<HashMap<String, ()>>>,
}

impl Default for EventBus {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            tx,
            _subs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl EventBus {
    pub fn publish(&self, topic: impl Into<String>, payload: serde_json::Value) {
        let _ = self.tx.send(Event {
            topic: topic.into(),
            payload,
        });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}
