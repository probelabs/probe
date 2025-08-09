use crate::protocol::{LogEntry, LogLevel};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// Maximum number of log entries to keep in memory
const MAX_LOG_ENTRIES: usize = 1000;

/// Thread-safe circular buffer for storing log entries
#[derive(Debug, Clone)]
pub struct LogBuffer {
    entries: Arc<Mutex<VecDeque<LogEntry>>>,
}

impl LogBuffer {
    /// Create a new empty log buffer
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Add a log entry to the buffer, removing old entries if needed
    pub fn push(&self, entry: LogEntry) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.push_back(entry);
            
            // Maintain circular buffer behavior by removing old entries
            while entries.len() > MAX_LOG_ENTRIES {
                entries.pop_front();
            }
        }
    }

    /// Get the last N log entries, up to the buffer size
    pub fn get_last(&self, count: usize) -> Vec<LogEntry> {
        if let Ok(entries) = self.entries.lock() {
            let take_count = count.min(entries.len());
            entries
                .iter()
                .rev()
                .take(take_count)
                .rev()
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all log entries currently in the buffer
    pub fn get_all(&self) -> Vec<LogEntry> {
        if let Ok(entries) = self.entries.lock() {
            entries.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Clear all log entries from the buffer
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }

    /// Get the current number of entries in the buffer
    pub fn len(&self) -> usize {
        if let Ok(entries) = self.entries.lock() {
            entries.len()
        } else {
            0
        }
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracing layer that writes log entries to an in-memory buffer
pub struct MemoryLogLayer {
    buffer: LogBuffer,
}

impl MemoryLogLayer {
    /// Create a new memory log layer with the given buffer
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }

    /// Get a reference to the log buffer
    pub fn buffer(&self) -> &LogBuffer {
        &self.buffer
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
        let file = metadata.file().map(|s| s.to_string());
        let line = metadata.line();
        (file, line)
    }

    /// Format the log message from the event
    fn format_message<S>(event: &Event<'_>, _ctx: &Context<'_, S>) -> String
    where
        S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    {
        // Use a visitor to format the message properly
        struct MessageVisitor {
            message: String,
        }
        
        impl tracing::field::Visit for MessageVisitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.message = format!("{:?}", value);
                    // Remove surrounding quotes from debug format
                    if self.message.starts_with('"') && self.message.ends_with('"') {
                        self.message = self.message[1..self.message.len()-1].to_string();
                    }
                }
            }
        }
        
        let mut visitor = MessageVisitor {
            message: String::new(),
        };
        
        event.record(&mut visitor);
        
        // If no message field was found, try to format as a display string
        if visitor.message.is_empty() {
            // Fallback to target if no specific message
            event.metadata().target().to_string()
        } else {
            visitor.message
        }
    }
}

impl<S> Layer<S> for MemoryLogLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let level = Self::convert_level(metadata.level());
        let target = metadata.target().to_string();
        let (file, line) = Self::extract_location(metadata);
        
        // Create timestamp
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f UTC").to_string();
        
        // Format message - this is a simplified version
        // A full implementation would extract the formatted message from the event
        let message = Self::format_message(event, &ctx);
        
        let log_entry = LogEntry {
            timestamp,
            level,
            target,
            message,
            file,
            line,
        };

        self.buffer.push(log_entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_buffer_basic_operations() {
        let buffer = LogBuffer::new();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);

        let entry = LogEntry {
            timestamp: "2024-01-01 12:00:00.000 UTC".to_string(),
            level: LogLevel::Info,
            target: "test".to_string(),
            message: "Test message".to_string(),
            file: None,
            line: None,
        };

        buffer.push(entry.clone());
        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());

        let entries = buffer.get_all();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "Test message");
    }

    #[test]
    fn test_log_buffer_circular_behavior() {
        let buffer = LogBuffer::new();
        
        // Fill buffer beyond capacity
        for i in 0..(MAX_LOG_ENTRIES + 100) {
            let entry = LogEntry {
                timestamp: format!("2024-01-01 12:00:{:02}.000 UTC", i % 60),
                level: LogLevel::Info,
                target: "test".to_string(),
                message: format!("Message {}", i),
                file: None,
                line: None,
            };
            buffer.push(entry);
        }

        // Should not exceed max capacity
        assert_eq!(buffer.len(), MAX_LOG_ENTRIES);
        
        // Should contain the most recent entries
        let entries = buffer.get_all();
        assert!(entries[entries.len() - 1].message.contains(&format!("{}", MAX_LOG_ENTRIES + 99)));
    }

    #[test]
    fn test_get_last_entries() {
        let buffer = LogBuffer::new();
        
        // Add some entries
        for i in 0..10 {
            let entry = LogEntry {
                timestamp: format!("2024-01-01 12:00:{:02}.000 UTC", i),
                level: LogLevel::Info,
                target: "test".to_string(),
                message: format!("Message {}", i),
                file: None,
                line: None,
            };
            buffer.push(entry);
        }

        // Get last 5 entries
        let entries = buffer.get_last(5);
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].message, "Message 5");
        assert_eq!(entries[4].message, "Message 9");
    }

    #[test]
    fn test_level_conversion() {
        assert!(matches!(MemoryLogLayer::convert_level(&tracing::Level::TRACE), LogLevel::Trace));
        assert!(matches!(MemoryLogLayer::convert_level(&tracing::Level::DEBUG), LogLevel::Debug));
        assert!(matches!(MemoryLogLayer::convert_level(&tracing::Level::INFO), LogLevel::Info));
        assert!(matches!(MemoryLogLayer::convert_level(&tracing::Level::WARN), LogLevel::Warn));
        assert!(matches!(MemoryLogLayer::convert_level(&tracing::Level::ERROR), LogLevel::Error));
    }
}