//! LSP Enrichment Tracking Module
//!
//! Tracks symbols that have failed LSP enrichment to prevent repeated attempts
//! and implements exponential backoff for retries.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Status of LSP enrichment for a symbol
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EnrichmentStatus {
    /// Not yet attempted
    Pending,
    /// Successfully enriched
    Success,
    /// Failed enrichment (with retry tracking)
    Failed,
    /// Permanently skipped (e.g., unsupported symbol type)
    Skipped,
}

/// Tracking information for LSP enrichment attempts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentTracking {
    pub symbol_uid: String,
    pub last_attempt_at: DateTime<Utc>,
    pub attempt_count: u32,
    pub status: EnrichmentStatus,
    pub failure_reason: Option<String>,
    pub next_retry_after: Option<DateTime<Utc>>,
    pub file_path: String,
    pub line_number: u32,
    pub language: String,
    pub symbol_name: String,
    pub symbol_kind: String,
}

impl EnrichmentTracking {
    /// Create a new failed tracking entry with exponential backoff
    pub fn new_failure(
        symbol_uid: String,
        failure_reason: String,
        attempt_count: u32,
        file_path: String,
        line_number: u32,
        language: String,
        symbol_name: String,
        symbol_kind: String,
    ) -> Self {
        let now = Utc::now();

        // Exponential backoff: 5s, 10s, 20s, 40s, 80s, 160s, 320s (max ~5 minutes)
        let backoff_seconds = std::cmp::min(320, 5 * (1 << attempt_count));
        let next_retry = now + Duration::seconds(backoff_seconds as i64);

        info!(
            "Symbol '{}' ({}:{}) failed enrichment attempt #{}, next retry in {}s",
            symbol_name, file_path, line_number, attempt_count, backoff_seconds
        );

        Self {
            symbol_uid,
            last_attempt_at: now,
            attempt_count,
            status: EnrichmentStatus::Failed,
            failure_reason: Some(failure_reason),
            next_retry_after: Some(next_retry),
            file_path,
            line_number,
            language,
            symbol_name,
            symbol_kind,
        }
    }

    /// Check if this symbol is ready for retry
    pub fn is_ready_for_retry(&self) -> bool {
        match (&self.status, &self.next_retry_after) {
            (EnrichmentStatus::Failed, Some(retry_time)) => Utc::now() >= *retry_time,
            _ => false,
        }
    }

    /// Check if symbol has exceeded max retry attempts (default: 7 attempts)
    pub fn should_skip(&self) -> bool {
        self.attempt_count >= 7
    }
}

/// In-memory cache for enrichment tracking
pub struct EnrichmentTracker {
    /// Set of symbol UIDs that have failed enrichment
    failed_symbols: Arc<RwLock<HashSet<String>>>,
    /// Detailed tracking information for failed symbols
    tracking_info: Arc<RwLock<Vec<EnrichmentTracking>>>,
    /// Maximum number of retry attempts before giving up
    max_retry_attempts: u32,
}

impl EnrichmentTracker {
    pub fn new() -> Self {
        Self {
            failed_symbols: Arc::new(RwLock::new(HashSet::new())),
            tracking_info: Arc::new(RwLock::new(Vec::new())),
            max_retry_attempts: 7,
        }
    }

    /// Record a failed enrichment attempt
    pub async fn record_failure(
        &self,
        symbol_uid: String,
        failure_reason: String,
        file_path: String,
        line_number: u32,
        language: String,
        symbol_name: String,
        symbol_kind: String,
    ) {
        let mut failed_set = self.failed_symbols.write().await;
        failed_set.insert(symbol_uid.clone());

        let mut tracking = self.tracking_info.write().await;

        // Find existing tracking or create new
        let existing_idx = tracking.iter().position(|t| t.symbol_uid == symbol_uid);

        let new_tracking = if let Some(idx) = existing_idx {
            let existing = &tracking[idx];
            EnrichmentTracking::new_failure(
                symbol_uid,
                failure_reason,
                existing.attempt_count + 1,
                file_path,
                line_number,
                language,
                symbol_name,
                symbol_kind,
            )
        } else {
            EnrichmentTracking::new_failure(
                symbol_uid,
                failure_reason,
                1,
                file_path,
                line_number,
                language,
                symbol_name,
                symbol_kind,
            )
        };

        // Check if we should permanently skip this symbol
        if new_tracking.should_skip() {
            warn!(
                "Symbol '{}' has failed {} times, marking as permanently skipped",
                new_tracking.symbol_name, new_tracking.attempt_count
            );
        }

        if let Some(idx) = existing_idx {
            tracking[idx] = new_tracking;
        } else {
            tracking.push(new_tracking);
        }
    }

    /// Check if a symbol has failed enrichment
    pub async fn has_failed(&self, symbol_uid: &str) -> bool {
        let failed_set = self.failed_symbols.read().await;
        failed_set.contains(symbol_uid)
    }

    /// Get symbols that are ready for retry
    pub async fn get_symbols_ready_for_retry(&self) -> Vec<String> {
        let tracking = self.tracking_info.read().await;
        tracking
            .iter()
            .filter(|t| t.is_ready_for_retry() && !t.should_skip())
            .map(|t| t.symbol_uid.clone())
            .collect()
    }

    /// Clear failure record for a symbol (after successful enrichment)
    pub async fn clear_failure(&self, symbol_uid: &str) {
        let mut failed_set = self.failed_symbols.write().await;
        failed_set.remove(symbol_uid);

        let mut tracking = self.tracking_info.write().await;
        tracking.retain(|t| t.symbol_uid != symbol_uid);

        debug!("Cleared failure tracking for symbol: {}", symbol_uid);
    }

    /// Get statistics about failed enrichments
    pub async fn get_stats(&self) -> EnrichmentStats {
        let failed_set = self.failed_symbols.read().await;
        let tracking = self.tracking_info.read().await;

        let permanently_skipped = tracking.iter().filter(|t| t.should_skip()).count();

        let ready_for_retry = tracking
            .iter()
            .filter(|t| t.is_ready_for_retry() && !t.should_skip())
            .count();

        EnrichmentStats {
            total_failed: failed_set.len(),
            permanently_skipped,
            ready_for_retry,
            in_cooldown: failed_set.len() - permanently_skipped - ready_for_retry,
        }
    }
}

/// Statistics about enrichment failures
#[derive(Debug, Clone, Serialize)]
pub struct EnrichmentStats {
    pub total_failed: usize,
    pub permanently_skipped: usize,
    pub ready_for_retry: usize,
    pub in_cooldown: usize,
}

impl Default for EnrichmentTracker {
    fn default() -> Self {
        Self::new()
    }
}
