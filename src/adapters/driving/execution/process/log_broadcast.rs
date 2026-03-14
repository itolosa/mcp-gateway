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
