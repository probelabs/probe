# Integration Plan for Enrichment Failure Tracking

## Problem Summary
The Phase 2 monitor runs every 5 seconds and queries for orphan symbols (symbols without edges). When LSP enrichment fails for a symbol, it remains an orphan and gets re-queried repeatedly, causing:
1. Infinite retry loops for symbols that will never succeed
2. Wasted CPU and LSP server resources
3. Log spam with the same failure messages

## Solution Components

### 1. EnrichmentTracker Module (✅ Implemented)
- Tracks failed enrichment attempts per symbol
- Implements exponential backoff (5s, 10s, 20s, 40s, 80s, 160s, max 320s)
- Limits retry attempts to 7 before marking as permanently skipped
- Provides in-memory tracking with detailed failure reasons

### 2. Persistence Strategy (Deferred)
Originally we planned to persist enrichment state in a dedicated table so retries could survive restarts. We’ve dropped that idea; for now we rely on the actual graph contents: once an operation emits edges (or explicit “none” placeholders) the symbol no longer qualifies as missing data. If we later need crash recovery across daemon restarts, we can revisit a durable tracker.

### 3. Integration Points (TODO)

#### A. IndexingManager Updates
```rust
// Add enrichment tracker to IndexingManager
pub struct IndexingManager {
    // ... existing fields ...
    enrichment_tracker: Arc<EnrichmentTracker>,
}

// In find_orphan_symbols_for_enrichment():
async fn find_orphan_symbols_for_enrichment(&self) -> Result<Vec<SymbolState>> {
    // Get orphan symbols from database
    let mut orphan_symbols = /* existing query */;

    // Filter out symbols that have failed recently
    let tracker = &self.enrichment_tracker;
    orphan_symbols.retain(|symbol| {
        !tracker.has_failed(&symbol.symbol_uid).await
    });

    // Add symbols that are ready for retry
    let retry_symbols = tracker.get_symbols_ready_for_retry().await;
    // ... fetch these symbols and add to list ...

    Ok(orphan_symbols)
}
```

#### B. LspEnrichmentWorker Updates
```rust
// In process_symbol_with_retries():
match Self::process_symbol_once(...).await {
    Ok(_) => {
        // Clear any previous failure tracking
        enrichment_tracker.clear_failure(&queue_item.symbol_uid).await;
        return Ok(());
    }
    Err(e) => {
        if attempt == config.max_retries {
            // Record the failure for backoff tracking
            enrichment_tracker.record_failure(
                queue_item.symbol_uid.clone(),
                e.to_string(),
                queue_item.file_path.display().to_string(),
                queue_item.def_start_line,
                queue_item.language.to_string(),
                queue_item.name.clone(),
                queue_item.kind.clone(),
            ).await;
        }
        // ... existing error handling ...
    }
}
```

#### C. Modified Orphan Query
Update the SQL query in `find_orphan_symbols` to look at the presence of specific LSP-derived edges instead of checking a tracking table. Treat the absence of concrete data (edges or explicit “none” placeholders) as the signal that another LSP pass is required.

### 4. Benefits
- **No more infinite retry loops**: Failed symbols get exponential backoff
- **Better resource usage**: LSP servers aren't hammered with failing requests
- **Cleaner logs**: Each symbol's failures are tracked, not repeated endlessly
- **Persistence**: Tracking survives daemon restarts via database storage
- **Observability**: Can query stats on how many symbols are failing/retrying

### 5. Rollout Plan
1. Deploy EnrichmentTracker module ✅
2. Update `find_orphan_symbols` to consider per-operation edge gaps
3. Adjust LspEnrichmentWorker to emit explicit “none” or “error” edges when operations fail definitively
4. Integrate EnrichmentTracker for in-memory backoff, and consider adding metrics/logging for monitoring

### 6. Testing Strategy
- Unit tests for EnrichmentTracker backoff calculations
- Integration test with mock LSP server that always fails
- Verify symbols don't get re-queued within backoff period
- Test that successful enrichment clears failure tracking
- Test persistence across daemon restarts
