//! Tracing layer that writes to persistent storage

use crate::logging::persistent_log::PersistentLogStorage;
use crate::protocol::{LogEntry, LogLevel};
use std::sync::Arc;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// Tracing layer that writes to persistent log storage
pub struct PersistentLogLayer {
    storage: Arc<PersistentLogStorage>,
}

impl PersistentLogLayer {
    /// Create a new persistent log layer
    pub fn new(storage: Arc<PersistentLogStorage>) -> Self {
        Self { storage }
    }

    /// Convert tracing level to our LogLevel enum
    fn convert_level(level: &tracing::Level) -> LogLevel {
        match *level {
            tracing::Level::TRACE => LogLevel::Trace,
            tracing::Level::DEBUG => LogLevel::Debug,
            tracing::Level::INFO => LogLevel::Info,
            tracing::Level::WARN => LogLevel::Warn,
            tracing::Level::ERROR => LogLevel::Error,
        }
    }

    /// Extract location information from metadata
    fn extract_location(metadata: &tracing::Metadata) -> (Option<String>, Option<u32>) {
        (metadata.file().map(String::from), metadata.line())
    }

    /// Format the log message from the event
    fn format_message(event: &Event, _ctx: &Context<'_, impl Subscriber>) -> String {
        // Use a visitor to extract the message
        struct MessageVisitor {
            message: String,
        }

        impl tracing::field::Visit for MessageVisitor {
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "message" {
                    self.message = value.to_string();
                } else if self.message.is_empty() {
                    // If no 'message' field yet, use any string field
                    self.message = format!("{}: {}", field.name(), value);
                } else {
                    // Append other fields
                    self.message
                        .push_str(&format!(", {}: {}", field.name(), value));
                }
            }

            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.message = format!("{:?}", value);
                } else if self.message.is_empty() {
                    self.message = format!("{}: {:?}", field.name(), value);
                } else {
                    self.message
                        .push_str(&format!(", {}: {:?}", field.name(), value));
                }
            }
        }

        let mut visitor = MessageVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);

        // Truncate very large messages to prevent memory issues
        const MAX_MESSAGE_LENGTH: usize = 4096;
        if visitor.message.len() > MAX_MESSAGE_LENGTH {
            visitor.message.truncate(MAX_MESSAGE_LENGTH);
            visitor.message.push_str("... [TRUNCATED]");
        }

        visitor.message
    }
}

impl<S> Layer<S> for PersistentLogLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        // Extract log information
        let metadata = event.metadata();
        let level = Self::convert_level(metadata.level());
        let target = metadata.target().to_string();
        let (file, line) = Self::extract_location(metadata);

        // Get timestamp
        let timestamp = chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S%.3f UTC")
            .to_string();

        // Format message
        let message = Self::format_message(event, &ctx);

        let log_entry = LogEntry {
            sequence: 0, // Will be set by persistent storage
            timestamp,
            level,
            target,
            message,
            file,
            line,
        };

        // Clone storage for async operation
        let storage = self.storage.clone();

        // Spawn async task to write to persistent storage (non-blocking)
        tokio::spawn(async move {
            if let Err(e) = storage.add_entry(log_entry).await {
                // Can't log this error or we'd have recursion
                eprintln!("Failed to persist log entry: {}", e);
            }
        });
    }
}
