//! LSP Enrichment Queue Module
//!
//! This module provides a priority queue system for queuing symbols that need LSP enrichment.
//! It's part of Phase 2 of the LSP enrichment system that finds orphan symbols (symbols without
//! edges) and enriches them with LSP data using the existing server manager infrastructure.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;

use crate::language_detector::Language;

/// Priority levels for LSP enrichment processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EnrichmentPriority {
    /// Highest priority - functions and methods
    High = 3,
    /// Medium priority - classes and structs
    Medium = 2,
    /// Low priority - other symbol types
    Low = 1,
}

impl EnrichmentPriority {
    /// Get priority from symbol kind string
    pub fn from_symbol_kind(kind: &str) -> Self {
        match kind {
            "function" | "method" => Self::High,
            "class" | "struct" | "enum" => Self::Medium,
            _ => Self::Low,
        }
    }
}

/// Individual LSP enrichment operations that can be executed for a symbol
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EnrichmentOperation {
    References,
    Implementations,
    CallHierarchy,
}

/// Item in the LSP enrichment queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    /// Unique identifier for this symbol
    pub symbol_uid: String,
    /// File path where the symbol is defined
    pub file_path: PathBuf,
    /// Line number of symbol definition
    pub def_start_line: u32,
    /// Character position of symbol definition
    pub def_start_char: u32,
    /// Symbol name
    pub name: String,
    /// Programming language
    pub language: Language,
    /// Symbol kind (function, class, etc)
    pub kind: String,
    /// Processing priority
    pub priority: EnrichmentPriority,
    /// Pending enrichment operations for this symbol
    pub operations: Vec<EnrichmentOperation>,
}

impl QueueItem {
    /// Create a new queue item
    pub fn new(
        symbol_uid: String,
        file_path: PathBuf,
        def_start_line: u32,
        def_start_char: u32,
        name: String,
        language: Language,
        kind: String,
    ) -> Self {
        let priority = EnrichmentPriority::from_symbol_kind(&kind);

        Self {
            symbol_uid,
            file_path,
            def_start_line,
            def_start_char,
            name,
            language,
            kind,
            priority,
            operations: Vec::new(),
        }
    }

    /// Attach pending operations to this queue item
    pub fn with_operations(mut self, operations: Vec<EnrichmentOperation>) -> Self {
        self.operations = operations;
        self
    }
}

/// Wrapper for priority queue ordering
#[derive(Debug, Clone)]
struct PriorityQueueItem {
    item: QueueItem,
    /// Timestamp for FIFO ordering within same priority
    timestamp: u64,
}

impl PriorityQueueItem {
    fn new(item: QueueItem) -> Self {
        Self {
            item,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }
}

impl PartialEq for PriorityQueueItem {
    fn eq(&self, other: &Self) -> bool {
        self.item.priority == other.item.priority && self.timestamp == other.timestamp
    }
}

impl Eq for PriorityQueueItem {}

impl PartialOrd for PriorityQueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityQueueItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then earlier timestamp (FIFO within same priority)
        match self.item.priority.cmp(&other.item.priority) {
            Ordering::Equal => other.timestamp.cmp(&self.timestamp), // Earlier timestamp first
            other => other,                                          // Higher priority first
        }
    }
}

/// LSP Enrichment Queue
///
/// A thread-safe priority queue for managing symbols that need LSP enrichment.
/// Provides high-priority processing for functions/methods and lower priority
/// for other symbol types.
pub struct LspEnrichmentQueue {
    /// Internal priority queue
    queue: Arc<Mutex<BinaryHeap<PriorityQueueItem>>>,
}

impl LspEnrichmentQueue {
    /// Create a new empty enrichment queue
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
        }
    }

    /// Add a symbol to the enrichment queue
    pub async fn add_symbol(&self, item: QueueItem) -> Result<()> {
        debug!(
            "Adding symbol to enrichment queue: {} ({}:{}) - priority: {:?}",
            item.name,
            item.file_path.display(),
            item.def_start_line,
            item.priority
        );

        let mut queue = self.queue.lock().await;
        queue.push(PriorityQueueItem::new(item));

        Ok(())
    }

    /// Pop the next highest priority symbol from the queue
    pub async fn pop_next(&self) -> Option<QueueItem> {
        let mut queue = self.queue.lock().await;
        queue.pop().map(|wrapper| {
            debug!(
                "Popped symbol from enrichment queue: {} - priority: {:?}",
                wrapper.item.name, wrapper.item.priority
            );
            wrapper.item
        })
    }

    /// Check if the queue is empty
    pub async fn is_empty(&self) -> bool {
        let queue = self.queue.lock().await;
        queue.is_empty()
    }

    /// Get the current size of the queue
    pub async fn size(&self) -> usize {
        let queue = self.queue.lock().await;
        queue.len()
    }

    /// Get queue statistics by priority level
    pub async fn get_stats(&self) -> EnrichmentQueueStats {
        let queue = self.queue.lock().await;
        let mut high_count = 0;
        let mut medium_count = 0;
        let mut low_count = 0;
        let mut total_operations = 0;
        let mut references_operations = 0;
        let mut implementations_operations = 0;
        let mut call_hierarchy_operations = 0;

        for item in queue.iter() {
            match item.item.priority {
                EnrichmentPriority::High => high_count += 1,
                EnrichmentPriority::Medium => medium_count += 1,
                EnrichmentPriority::Low => low_count += 1,
            }

            total_operations += item.item.operations.len();
            for op in &item.item.operations {
                match op {
                    EnrichmentOperation::References => references_operations += 1,
                    EnrichmentOperation::Implementations => implementations_operations += 1,
                    EnrichmentOperation::CallHierarchy => call_hierarchy_operations += 1,
                }
            }
        }

        EnrichmentQueueStats {
            total_items: queue.len(),
            high_priority_items: high_count,
            medium_priority_items: medium_count,
            low_priority_items: low_count,
            total_operations,
            references_operations,
            implementations_operations,
            call_hierarchy_operations,
        }
    }

    /// Clear all items from the queue
    pub async fn clear(&self) -> Result<()> {
        let mut queue = self.queue.lock().await;
        queue.clear();
        debug!("Cleared LSP enrichment queue");
        Ok(())
    }
}

impl Default for LspEnrichmentQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the enrichment queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentQueueStats {
    /// Total number of items in queue
    pub total_items: usize,
    /// Number of high priority items
    pub high_priority_items: usize,
    /// Number of medium priority items
    pub medium_priority_items: usize,
    /// Number of low priority items
    pub low_priority_items: usize,
    /// Total pending operations across all queue items
    pub total_operations: usize,
    /// Pending reference operations
    pub references_operations: usize,
    /// Pending implementation operations
    pub implementations_operations: usize,
    /// Pending call hierarchy operations
    pub call_hierarchy_operations: usize,
}

impl EnrichmentQueueStats {
    /// Check if the queue has any items
    pub fn has_items(&self) -> bool {
        self.total_items > 0
    }

    /// Get percentage of high priority items
    pub fn high_priority_percentage(&self) -> f64 {
        if self.total_items == 0 {
            0.0
        } else {
            (self.high_priority_items as f64 / self.total_items as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_queue_basic_operations() {
        let queue = LspEnrichmentQueue::new();

        // Test empty queue
        assert!(queue.is_empty().await);
        assert_eq!(queue.size().await, 0);
        assert!(queue.pop_next().await.is_none());

        // Add an item
        let item = QueueItem::new(
            "test_uid".to_string(),
            PathBuf::from("test.rs"),
            10,
            5,
            "test_function".to_string(),
            Language::Rust,
            "function".to_string(),
        );

        queue.add_symbol(item.clone()).await.unwrap();

        // Test non-empty queue
        assert!(!queue.is_empty().await);
        assert_eq!(queue.size().await, 1);

        // Pop the item
        let popped = queue.pop_next().await.unwrap();
        assert_eq!(popped.symbol_uid, item.symbol_uid);
        assert_eq!(popped.name, item.name);
        assert_eq!(popped.priority, EnrichmentPriority::High);

        // Test empty again
        assert!(queue.is_empty().await);
        assert_eq!(queue.size().await, 0);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let queue = LspEnrichmentQueue::new();

        // Add items with different priorities
        let low_item = QueueItem::new(
            "low_uid".to_string(),
            PathBuf::from("test.rs"),
            10,
            5,
            "variable".to_string(),
            Language::Rust,
            "variable".to_string(),
        );

        let high_item = QueueItem::new(
            "high_uid".to_string(),
            PathBuf::from("test.rs"),
            20,
            10,
            "function".to_string(),
            Language::Rust,
            "function".to_string(),
        );

        let medium_item = QueueItem::new(
            "medium_uid".to_string(),
            PathBuf::from("test.rs"),
            30,
            15,
            "MyClass".to_string(),
            Language::Rust,
            "class".to_string(),
        );

        // Add in random order
        queue.add_symbol(low_item).await.unwrap();
        queue.add_symbol(high_item).await.unwrap();
        queue.add_symbol(medium_item).await.unwrap();

        // Should pop in priority order: High, Medium, Low
        let first = queue.pop_next().await.unwrap();
        assert_eq!(first.priority, EnrichmentPriority::High);
        assert_eq!(first.name, "function");

        let second = queue.pop_next().await.unwrap();
        assert_eq!(second.priority, EnrichmentPriority::Medium);
        assert_eq!(second.name, "MyClass");

        let third = queue.pop_next().await.unwrap();
        assert_eq!(third.priority, EnrichmentPriority::Low);
        assert_eq!(third.name, "variable");
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let queue = LspEnrichmentQueue::new();

        // Add items of different priorities
        for i in 0..5 {
            queue
                .add_symbol(QueueItem::new(
                    format!("high_{}", i),
                    PathBuf::from("test.rs"),
                    i as u32,
                    0,
                    format!("func_{}", i),
                    Language::Rust,
                    "function".to_string(),
                ))
                .await
                .unwrap();
        }

        for i in 0..3 {
            queue
                .add_symbol(QueueItem::new(
                    format!("medium_{}", i),
                    PathBuf::from("test.rs"),
                    i as u32,
                    0,
                    format!("class_{}", i),
                    Language::Rust,
                    "class".to_string(),
                ))
                .await
                .unwrap();
        }

        for i in 0..2 {
            queue
                .add_symbol(QueueItem::new(
                    format!("low_{}", i),
                    PathBuf::from("test.rs"),
                    i as u32,
                    0,
                    format!("var_{}", i),
                    Language::Rust,
                    "variable".to_string(),
                ))
                .await
                .unwrap();
        }

        let stats = queue.get_stats().await;
        assert_eq!(stats.total_items, 10);
        assert_eq!(stats.high_priority_items, 5);
        assert_eq!(stats.medium_priority_items, 3);
        assert_eq!(stats.low_priority_items, 2);
        assert!(stats.has_items());
        assert_eq!(stats.high_priority_percentage(), 50.0);
    }

    #[tokio::test]
    async fn test_priority_from_symbol_kind() {
        assert_eq!(
            EnrichmentPriority::from_symbol_kind("function"),
            EnrichmentPriority::High
        );
        assert_eq!(
            EnrichmentPriority::from_symbol_kind("method"),
            EnrichmentPriority::High
        );
        assert_eq!(
            EnrichmentPriority::from_symbol_kind("class"),
            EnrichmentPriority::Medium
        );
        assert_eq!(
            EnrichmentPriority::from_symbol_kind("struct"),
            EnrichmentPriority::Medium
        );
        assert_eq!(
            EnrichmentPriority::from_symbol_kind("enum"),
            EnrichmentPriority::Medium
        );
        assert_eq!(
            EnrichmentPriority::from_symbol_kind("variable"),
            EnrichmentPriority::Low
        );
        assert_eq!(
            EnrichmentPriority::from_symbol_kind("unknown"),
            EnrichmentPriority::Low
        );
    }

    #[tokio::test]
    async fn test_clear_queue() {
        let queue = LspEnrichmentQueue::new();

        // Add some items
        for i in 0..3 {
            queue
                .add_symbol(QueueItem::new(
                    format!("test_{}", i),
                    PathBuf::from("test.rs"),
                    i as u32,
                    0,
                    format!("item_{}", i),
                    Language::Rust,
                    "function".to_string(),
                ))
                .await
                .unwrap();
        }

        assert_eq!(queue.size().await, 3);

        // Clear the queue
        queue.clear().await.unwrap();

        assert!(queue.is_empty().await);
        assert_eq!(queue.size().await, 0);
    }
}
