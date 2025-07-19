// In app/src/tracing_layer.rs

use chrono::Utc;
use tokio::sync::broadcast;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use events::{WsLogMessage, WsMessage};
type WsCache = std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<events::WsMessage>>>;

pub struct WsBroadcastLayer {
    tx: broadcast::Sender<WsMessage>,
    cache: WsCache, // <-- Add this
}

impl WsBroadcastLayer {
    pub fn new(tx: broadcast::Sender<WsMessage>, cache: WsCache) -> Self { // <-- Update signature
        Self { tx, cache }
    }
}

impl<S> Layer<S> for WsBroadcastLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        // Create a visitor to extract the message from the event's fields.
        let mut visitor = LogMessageVisitor::new();
        event.record(&mut visitor);
        let log_message = WsLogMessage {
            timestamp: Utc::now(),
            level: event.metadata().level().to_string(),
            message: visitor.message,
        };
        let msg = WsMessage::Log(log_message);
        // Send to live clients
        let _ = self.tx.send(msg.clone());
        // Also add to the replay cache
        let mut cache = self.cache.lock().unwrap();
        if cache.len() >= 200 { // WS_CACHE_SIZE
            cache.pop_front();
        }
        cache.push_back(msg);
    }
}

// A simple visitor to capture the `message` field of a log event.
struct LogMessageVisitor {
    message: String,
}

impl LogMessageVisitor {
    fn new() -> Self {
        Self { message: String::new() }
    }
}

impl tracing::field::Visit for LogMessageVisitor {
    fn record_debug(&mut self, _field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.message = format!("{:?}", value);
    }
}