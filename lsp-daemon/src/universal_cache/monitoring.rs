//! Monitoring and observability utilities for universal cache
//!
//! This module provides functions to collect and format cache statistics
//! for inclusion in daemon status responses and monitoring systems.

use super::{CacheStats, LspMethod, MethodStats, UniversalCache};
use crate::protocol::{
    UniversalCacheConfigSummary, UniversalCacheMethodStats, UniversalCacheStats,
    UniversalCacheWorkspaceSummary,
};
use anyhow::Result;
use std::collections::HashMap;

impl UniversalCache {
    /// Get comprehensive universal cache statistics for daemon status
    pub async fn get_daemon_stats(&self) -> Result<UniversalCacheStats> {
        // Get basic cache stats
        let cache_stats = self.get_stats().await?;

        // Convert method stats
        let method_stats = convert_method_stats(&cache_stats.method_stats);

        // Calculate total operations from method stats
        let total_operations = cache_stats
            .method_stats
            .values()
            .map(|s| s.hits + s.misses)
            .sum::<u64>();

        // Cache is enabled if we have any data
        let cache_enabled = cache_stats.total_entries > 0 || total_operations > 0;

        // Get workspace summaries
        let workspace_summaries = get_workspace_summaries(&cache_stats).await?;

        // Get configuration summary
        let config_summary = get_config_summary();

        // Calculate rates (total_operations already calculated above)

        let total_hits = cache_stats
            .method_stats
            .values()
            .map(|s| s.hits)
            .sum::<u64>();

        let total_misses = cache_stats
            .method_stats
            .values()
            .map(|s| s.misses)
            .sum::<u64>();

        let hit_rate = if total_operations > 0 {
            total_hits as f64 / total_operations as f64
        } else {
            0.0
        };

        let miss_rate = if total_operations > 0 {
            total_misses as f64 / total_operations as f64
        } else {
            0.0
        };

        Ok(UniversalCacheStats {
            enabled: true,
            total_entries: cache_stats.total_entries,
            total_size_bytes: cache_stats.total_size_bytes,
            active_workspaces: cache_stats.active_workspaces,
            hit_rate,
            miss_rate,
            total_hits,
            total_misses,
            method_stats,
            cache_enabled,
            workspace_summaries,
            config_summary,
        })
    }
}

/// Convert universal cache method stats to protocol format
fn convert_method_stats(
    method_stats: &HashMap<LspMethod, MethodStats>,
) -> HashMap<String, UniversalCacheMethodStats> {
    method_stats
        .iter()
        .map(|(method, stats)| {
            let total_ops = stats.hits + stats.misses;
            let hit_rate = if total_ops > 0 {
                stats.hits as f64 / total_ops as f64
            } else {
                0.0
            };

            let protocol_stats = UniversalCacheMethodStats {
                method: method.as_str().to_string(),
                enabled: true, // Would check actual policy
                entries: stats.entries,
                size_bytes: stats.size_bytes,
                hits: stats.hits,
                misses: stats.misses,
                hit_rate,
                avg_cache_response_time_us: 100, // Placeholder - would track actual timing
                avg_lsp_response_time_us: 5000,  // Placeholder - would track actual timing
            };

            (method.as_str().to_string(), protocol_stats)
        })
        .collect()
}

/// Get workspace-specific cache summaries
async fn get_workspace_summaries(
    _cache_stats: &CacheStats,
) -> Result<Vec<UniversalCacheWorkspaceSummary>> {
    // Placeholder implementation
    // In reality, this would iterate through workspace caches and collect stats
    let summaries = vec![UniversalCacheWorkspaceSummary {
        workspace_id: "example_workspace_abc123".to_string(),
        workspace_root: "/Users/example/projects/my-project".into(),
        entries: 500,
        size_bytes: 512 * 1024, // 512KB
        hits: 2500,
        misses: 250,
        hit_rate: 0.91,
        last_accessed: chrono::Utc::now().to_rfc3339(),
        languages: vec!["rust".to_string(), "typescript".to_string()],
    }];

    Ok(summaries)
}

/// Get configuration summary for universal cache
fn get_config_summary() -> UniversalCacheConfigSummary {
    // Placeholder implementation
    // In reality, this would read from actual configuration
    UniversalCacheConfigSummary {
        enabled: true,
        max_size_mb: Some(1024),
        custom_method_configs: 3,
        compression_enabled: true,
    }
}

/// Helper to format cache statistics for human-readable output
pub fn format_cache_stats_summary(stats: &UniversalCacheStats) -> String {
    let mut summary = String::new();

    summary.push_str(&format!(
        "Universal Cache Status: {}\n",
        if stats.enabled { "Enabled" } else { "Disabled" }
    ));

    if stats.enabled {
        summary.push_str(&format!("  Total entries: {}\n", stats.total_entries));
        summary.push_str(&format!(
            "  Total size: {:.2} MB\n",
            stats.total_size_bytes as f64 / (1024.0 * 1024.0)
        ));
        summary.push_str(&format!(
            "  Active workspaces: {}\n",
            stats.active_workspaces
        ));
        summary.push_str(&format!(
            "  Overall hit rate: {:.1}%\n",
            stats.hit_rate * 100.0
        ));

        if !stats.method_stats.is_empty() {
            summary.push_str("\n  Method Statistics:\n");
            for (method, method_stats) in &stats.method_stats {
                summary.push_str(&format!(
                    "    {}: {} entries, {:.1}% hit rate\n",
                    method,
                    method_stats.entries,
                    method_stats.hit_rate * 100.0
                ));
            }
        }

        // Simplified cache overview - no layer details
        summary.push_str(&format!(
            "  Cache enabled: {}\n",
            if stats.cache_enabled { "Yes" } else { "No" }
        ));

        if !stats.workspace_summaries.is_empty() {
            summary.push_str(&format!(
                "\n  Active Workspaces: {}\n",
                stats.workspace_summaries.len()
            ));
            for workspace in stats.workspace_summaries.iter().take(3) {
                // Show first 3
                summary.push_str(&format!(
                    "    {}: {} entries, {:.1}% hit rate\n",
                    workspace.workspace_id,
                    workspace.entries,
                    workspace.hit_rate * 100.0
                ));
            }
            if stats.workspace_summaries.len() > 3 {
                summary.push_str(&format!(
                    "    ... and {} more\n",
                    stats.workspace_summaries.len() - 3
                ));
            }
        }
    }

    summary
}

/// Helper to get disabled universal cache stats (when feature is disabled)
pub fn get_disabled_cache_stats() -> UniversalCacheStats {
    UniversalCacheStats {
        enabled: false,
        total_entries: 0,
        total_size_bytes: 0,
        active_workspaces: 0,
        hit_rate: 0.0,
        miss_rate: 0.0,
        total_hits: 0,
        total_misses: 0,
        method_stats: HashMap::new(),
        cache_enabled: false,
        workspace_summaries: Vec::new(),
        config_summary: UniversalCacheConfigSummary {
            enabled: false,
            max_size_mb: None,
            custom_method_configs: 0,
            compression_enabled: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_cache_stats_summary() {
        let stats = get_disabled_cache_stats();
        let summary = format_cache_stats_summary(&stats);

        assert!(summary.contains("Universal Cache Status: Disabled"));
        assert!(!summary.contains("Total entries:"));
    }

    #[test]
    fn test_convert_method_stats() {
        let mut method_stats = HashMap::new();
        method_stats.insert(
            LspMethod::Definition,
            MethodStats {
                entries: 100,
                size_bytes: 1024,
                hits: 80,
                misses: 20,
            },
        );

        let protocol_stats = convert_method_stats(&method_stats);

        assert_eq!(protocol_stats.len(), 1);
        let def_stats = &protocol_stats["textDocument/definition"];
        assert_eq!(def_stats.entries, 100);
        assert_eq!(def_stats.hit_rate, 0.8); // 80/100
        assert!(def_stats.enabled);
    }
}
