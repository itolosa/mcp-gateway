use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

pub struct BroadcastLayer {
    sender: broadcast::Sender<String>,
}

impl BroadcastLayer {
    pub fn new(sender: broadcast::Sender<String>) -> Self {
        Self { sender }
    }
}

struct FieldVisitor {
    message: String,
}

impl FieldVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
        }
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        }
    }
}

impl<S: Subscriber> Layer<S> for BroadcastLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);
        let level = event.metadata().level();
        let target = event.metadata().target();
        let line = format!("{level} {target}: {}", visitor.message);
        let _ = self.sender.send(line);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn should_capture_event_when_subscriber_active() {
        let (sender, mut receiver) = broadcast::channel::<String>(16);
        let layer = BroadcastLayer::new(sender);
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "test_target", "hello world");
        });
        let msg = receiver.try_recv().unwrap();
        assert!(msg.contains("INFO"));
        assert!(msg.contains("test_target"));
        assert!(msg.contains("hello world"));
    }

    #[test]
    fn should_not_panic_when_no_receivers() {
        let (sender, receiver) = broadcast::channel::<String>(16);
        drop(receiver);
        let layer = BroadcastLayer::new(sender);
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("no receivers");
        });
    }

    #[test]
    fn should_format_level_target_message() {
        let (sender, mut receiver) = broadcast::channel::<String>(16);
        let layer = BroadcastLayer::new(sender);
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(target: "my_module", "something happened");
        });
        let msg = receiver.try_recv().unwrap();
        assert_eq!(msg, "WARN my_module: something happened");
    }
}
