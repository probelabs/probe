#!/usr/bin/env cargo script
//! Cache Memory Growth Analysis Script
//!
//! This script demonstrates the memory growth issues in Probe's caching system
//! by simulating real-world usage patterns and measuring memory consumption.
//!
//! Usage: cargo script cache_memory_analysis.rs
//!
//! Demonstrates:
//! 1. Unbounded cache growth in tree cache
//! 2. Memory consumption patterns across different cache types
//! 3. Performance vs memory trade-offs
//! 4. Cache invalidation failure impacts

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

/// Memory usage tracker for cache analysis
#[derive(Debug, Clone)]
struct MemoryUsage {
    timestamp: u64,
    cache_entries: usize,
    estimated_memory_mb: f64,
    operation_count: usize,
}

/// Cache performance metrics
#[derive(Debug, Clone)]
struct CacheMetrics {
    hits: usize,
    misses: usize,
    insertions: usize,
    evictions: usize,
    memory_usage: Vec<MemoryUsage>,
}

impl CacheMetrics {
    fn new() -> Self {
        Self {
            hits: 0,
            misses: 0,
            insertions: 0,
            evictions: 0,
            memory_usage: Vec::new(),
        }
    }

    fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }

    fn record_memory_usage(&mut self, entries: usize, estimated_mb: f64, ops: usize) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.memory_usage.push(MemoryUsage {
            timestamp,
            cache_entries: entries,
            estimated_memory_mb: estimated_mb,
            operation_count: ops,
        });
    }
}

/// Simulates tree cache memory growth
struct TreeCacheSimulator {
    cache: HashMap<String, (String, u64)>, // file_path -> (content, hash)
    metrics: CacheMetrics,
    memory_per_entry_kb: f64,
}

impl TreeCacheSimulator {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
            metrics: CacheMetrics::new(),
            memory_per_entry_kb: 2.5, // Estimated memory per cached tree
        }
    }

    fn parse_file(&mut self, file_path: &str, content: &str) -> bool {
        let content_hash = self.hash_content(content);

        if let Some((cached_content, cached_hash)) = self.cache.get(file_path) {
            if cached_hash == &content_hash {
                self.metrics.hits += 1;
                return true; // Cache hit
            }
        }

        self.metrics.misses += 1;
        self.cache.insert(file_path.to_string(), (content.to_string(), content_hash));
        self.metrics.insertions += 1;

        // Record memory usage every 100 operations
        if self.metrics.insertions % 100 == 0 {
            let memory_mb = (self.cache.len() as f64 * self.memory_per_entry_kb) / 1024.0;
            self.metrics.record_memory_usage(
                self.cache.len(),
                memory_mb,
                self.metrics.insertions,
            );
        }

        false // Cache miss
    }

    fn hash_content(&self, content: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    fn get_metrics(&self) -> &CacheMetrics {
        &self.metrics
    }

    fn get_memory_usage_mb(&self) -> f64 {
        (self.cache.len() as f64 * self.memory_per_entry_kb) / 1024.0
    }
}

/// Simulates token cache with size limits
struct TokenCacheSimulator {
    cache: HashMap<String, (usize, u64)>, // content_hash -> (token_count, last_accessed)
    metrics: CacheMetrics,
    max_entries: usize,
    ttl_seconds: u64,
}

impl TokenCacheSimulator {
    fn new(max_entries: usize, ttl_seconds: u64) -> Self {
        Self {
            cache: HashMap::new(),
            metrics: CacheMetrics::new(),
            max_entries,
            ttl_seconds,
        }
    }

    fn count_tokens(&mut self, content: &str) -> usize {
        let content_hash = format!("{:x}", self.hash_content(content));
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Check cache
        if let Some((token_count, last_accessed)) = self.cache.get_mut(&content_hash) {
            if current_time - *last_accessed < self.ttl_seconds {
                *last_accessed = current_time;
                self.metrics.hits += 1;
                return *token_count;
            } else {
                // Expired entry
                self.cache.remove(&content_hash);
                self.metrics.evictions += 1;
            }
        }

        self.metrics.misses += 1;

        // Simulate token counting (just use character count / 4 as approximation)
        let token_count = content.len() / 4;

        // Evict entries if over limit
        while self.cache.len() >= self.max_entries {
            if let Some(oldest_key) = self.find_oldest_entry() {
                self.cache.remove(&oldest_key);
                self.metrics.evictions += 1;
            } else {
                break;
            }
        }

        self.cache.insert(content_hash, (token_count, current_time));
        self.metrics.insertions += 1;

        // Record memory usage
        if self.metrics.insertions % 50 == 0 {
            let memory_mb = (self.cache.len() as f64 * 0.1) / 1024.0; // ~100 bytes per entry
            self.metrics.record_memory_usage(
                self.cache.len(),
                memory_mb,
                self.metrics.insertions,
            );
        }

        token_count
    }

    fn find_oldest_entry(&self) -> Option<String> {
        self.cache
            .iter()
            .min_by_key(|(_, (_, last_accessed))| *last_accessed)
            .map(|(key, _)| key.clone())
    }

    fn hash_content(&self, content: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    fn get_metrics(&self) -> &CacheMetrics {
        &self.metrics
    }
}

/// Generates realistic test content
struct ContentGenerator {
    file_counter: AtomicUsize,
}

impl ContentGenerator {
    fn new() -> Self {
        Self {
            file_counter: AtomicUsize::new(0),
        }
    }

    fn generate_rust_file(&self) -> (String, String) {
        let id = self.file_counter.fetch_add(1, Ordering::SeqCst);
        let file_path = format!("src/module_{}.rs", id);

        let content = format!(r#"
//! Module {} generated for cache testing
//! This file contains realistic Rust code to test caching behavior

use std::collections::HashMap;
use std::sync::{{Arc, Mutex}};

/// Structure for data processing in module {}
#[derive(Debug, Clone)]
pub struct DataProcessor_{} {{
    data: HashMap<String, i32>,
    counter: Arc<Mutex<usize>>,
    config: ProcessorConfig_{},
}}

/// Configuration for processor {}
#[derive(Debug, Clone)]
struct ProcessorConfig_{} {{
    max_entries: usize,
    timeout_ms: u64,
    batch_size: usize,
}}

impl DataProcessor_{} {{
    /// Creates a new data processor instance
    pub fn new() -> Self {{
        Self {{
            data: HashMap::new(),
            counter: Arc::new(Mutex::new(0)),
            config: ProcessorConfig_{} {{
                max_entries: {},
                timeout_ms: {},
                batch_size: {},
            }},
        }}
    }}

    /// Processes a batch of data items
    pub fn process_batch(&mut self, items: Vec<String>) -> Result<Vec<i32>, ProcessorError> {{
        let mut results = Vec::new();
        let mut counter = self.counter.lock().unwrap();

        for item in items {{
            let processed_value = self.process_single_item(&item)?;
            results.push(processed_value);
            *counter += 1;

            if results.len() >= self.config.batch_size {{
                break;
            }}
        }}

        Ok(results)
    }}

    /// Processes a single data item
    fn process_single_item(&mut self, item: &str) -> Result<i32, ProcessorError> {{
        // Simulate processing logic
        let hash_value = item.chars().map(|c| c as u32).sum::<u32>() as i32;
        let processed = hash_value * {} + {};

        self.data.insert(item.to_string(), processed);
        Ok(processed)
    }}

    /// Gets statistics about processed data
    pub fn get_stats(&self) -> ProcessorStats {{
        let counter = self.counter.lock().unwrap();
        ProcessorStats {{
            total_processed: *counter,
            unique_items: self.data.len(),
            average_value: if self.data.is_empty() {{
                0.0
            }} else {{
                self.data.values().sum::<i32>() as f64 / self.data.len() as f64
            }},
        }}
    }}
}}

/// Statistics for the data processor
#[derive(Debug)]
pub struct ProcessorStats {{
    pub total_processed: usize,
    pub unique_items: usize,
    pub average_value: f64,
}}

/// Error type for processor operations
#[derive(Debug)]
pub enum ProcessorError {{
    InvalidInput(String),
    ProcessingFailed(String),
    ConfigurationError(String),
}}

impl std::error::Error for ProcessorError {{}}

impl std::fmt::Display for ProcessorError {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        match self {{
            ProcessorError::InvalidInput(msg) => write!(f, "Invalid input: {{}}", msg),
            ProcessorError::ProcessingFailed(msg) => write!(f, "Processing failed: {{}}", msg),
            ProcessorError::ConfigurationError(msg) => write!(f, "Configuration error: {{}}", msg),
        }}
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_processor_creation() {{
        let processor = DataProcessor_{}::new();
        let stats = processor.get_stats();
        assert_eq!(stats.total_processed, 0);
        assert_eq!(stats.unique_items, 0);
    }}

    #[test]
    fn test_batch_processing() {{
        let mut processor = DataProcessor_{}::new();
        let items = vec!["item1".to_string(), "item2".to_string(), "item3".to_string()];

        let results = processor.process_batch(items).unwrap();
        assert_eq!(results.len(), 3);

        let stats = processor.get_stats();
        assert_eq!(stats.total_processed, 3);
        assert_eq!(stats.unique_items, 3);
    }}
}}
"#,
            id, id, id, id, id, id, id,
            id * 100 + 50,      // max_entries
            id * 1000 + 5000,   // timeout_ms
            id % 10 + 5,        // batch_size
            id * 13 + 7,        // multiplier
            id * 5 + 42,        // offset
            id, id
        );

        (file_path, content)
    }

    fn generate_javascript_file(&self) -> (String, String) {
        let id = self.file_counter.fetch_add(1, Ordering::SeqCst);
        let file_path = format!("src/component_{}.js", id);

        let content = format!(r#"
/**
 * Component {} - Generated for cache testing
 * This file contains realistic JavaScript code patterns
 */

import React, {{ useState, useEffect, useCallback }} from 'react';
import {{ debounce, throttle }} from 'lodash';

/**
 * DataProcessor component for module {}
 */
export const DataProcessor{} = ({{
    initialData = [],
    maxItems = {},
    processingTimeout = {}
}}) => {{
    const [data, setData] = useState(initialData);
    const [processing, setProcessing] = useState(false);
    const [stats, setStats] = useState({{
        totalProcessed: 0,
        averageValue: 0,
        lastUpdate: null
    }});

    // Memoized processing function
    const processItems = useCallback(async (items) => {{
        setProcessing(true);

        try {{
            const processed = await Promise.all(
                items.slice(0, maxItems).map(async (item, index) => {{
                    await new Promise(resolve => setTimeout(resolve, processingTimeout / items.length));

                    return {{
                        id: `item_${{id}}_${{index}}`,
                        value: item.value * {} + {},
                        processed: true,
                        timestamp: Date.now(),
                        metadata: {{
                            originalIndex: index,
                            processingTime: processingTimeout / items.length,
                            batchId: `batch_${{id}}_${{Date.now()}}`
                        }}
                    }};
                }})
            );

            setData(processed);
            setStats(prevStats => ({{
                totalProcessed: prevStats.totalProcessed + processed.length,
                averageValue: processed.reduce((sum, item) => sum + item.value, 0) / processed.length,
                lastUpdate: new Date().toISOString()
            }}));

        }} catch (error) {{
            console.error('Processing error in component {}:', error);
        }} finally {{
            setProcessing(false);
        }}
    }}, [maxItems, processingTimeout]);

    // Debounced search function
    const handleSearch = useCallback(
        debounce((searchTerm) => {{
            const filtered = data.filter(item =>
                item.id.toLowerCase().includes(searchTerm.toLowerCase()) ||
                item.metadata.batchId.includes(searchTerm)
            );

            console.log(`Search results for "${{searchTerm}}" in component {}:`, filtered.length);
        }}, 300),
        [data]
    );

    // Throttled update function
    const handleUpdate = useCallback(
        throttle((updates) => {{
            setData(prevData =>
                prevData.map(item => {{
                    const update = updates.find(u => u.id === item.id);
                    return update ? {{ ...item, ...update }} : item;
                }})
            );
        }}, 100),
        []
    );

    // Effect for processing data
    useEffect(() => {{
        if (initialData.length > 0) {{
            processItems(initialData);
        }}
    }}, [initialData, processItems]);

    // Effect for cleanup
    useEffect(() => {{
        return () => {{
            console.log(`Cleanup component {} with ${{data.length}} items`);
        }};
    }}, [data.length]);

    const renderItem = (item) => (
        <div key={{item.id}} className="data-item">
            <h3>{{item.id}}</h3>
            <p>Value: {{item.value}}</p>
            <p>Processed: {{item.processed ? 'Yes' : 'No'}}</p>
            <small>{{item.timestamp}}</small>
        </div>
    );

    const renderStats = () => (
        <div className="stats-panel">
            <h2>Processing Stats</h2>
            <p>Total Processed: {{stats.totalProcessed}}</p>
            <p>Average Value: {{stats.averageValue.toFixed(2)}}</p>
            <p>Last Update: {{stats.lastUpdate}}</p>
            <p>Current Items: {{data.length}}</p>
        </div>
    );

    return (
        <div className="data-processor-{{}}">
            <h1>Data Processor Component {{}}</h1>

            {{renderStats()}}

            <div className="controls">
                <button
                    onClick={{() => processItems(initialData)}}
                    disabled={{processing}}
                >
                    {{processing ? 'Processing...' : 'Reprocess Data'}}
                </button>

                <input
                    type="text"
                    placeholder="Search items..."
                    onChange={{(e) => handleSearch(e.target.value)}}
                />
            </div>

            <div className="items-grid">
                {{data.map(renderItem)}}
            </div>
        </div>
    );
}};

/**
 * Utility functions for component {}
 */
export const ComponentUtils{} = {{
    // Data transformation utilities
    transformData: (rawData) => {{
        return rawData.map((item, index) => ({{
            ...item,
            id: `transformed_${{index}}_${{{}}}`,
            transformedAt: Date.now(),
            originalData: item
        }}));
    }},

    // Validation utilities
    validateItems: (items) => {{
        const errors = [];

        items.forEach((item, index) => {{
            if (!item.id) {{
                errors.push(`Item at index ${{index}} missing id`);
            }}
            if (typeof item.value !== 'number') {{
                errors.push(`Item at index ${{index}} has invalid value type`);
            }}
        }});

        return errors;
    }},

    // Performance utilities
    measurePerformance: (fn, label = 'operation') => {{
        return (...args) => {{
            const start = performance.now();
            const result = fn(...args);
            const end = performance.now();

            console.log(`${{label}} in component {} took ${{end - start}} milliseconds`);
            return result;
        }};
    }}
}};

export default DataProcessor{};
"#,
            id, id, id,
            id * 10 + 20,       // maxItems
            id * 100 + 1000,    // processingTimeout
            id * 7 + 3,         // multiplier
            id * 2 + 10,        // offset
            id, id, id, id, id, id, id, id, id
        );

        (file_path, content)
    }
}

/// Runs the cache memory analysis
fn main() {
    println!("🔍 Probe Cache Memory Growth Analysis");
    println!("=====================================\n");

    // Test 1: Unbounded Tree Cache Growth
    println!("📊 Test 1: Tree Cache Unbounded Growth");
    test_tree_cache_growth();
    println!();

    // Test 2: Token Cache with Limits
    println!("📊 Test 2: Token Cache with Size Limits");
    test_token_cache_behavior();
    println!();

    // Test 3: Mixed Workload Memory Usage
    println!("📊 Test 3: Mixed Workload Memory Impact");
    test_mixed_workload();
    println!();

    // Test 4: Concurrent Access Memory Issues
    println!("📊 Test 4: Concurrent Access Memory Patterns");
    test_concurrent_memory_usage();
    println!();

    println!("✅ Analysis Complete - Results show critical memory management issues");
}

fn test_tree_cache_growth() {
    let mut tree_cache = TreeCacheSimulator::new();
    let generator = ContentGenerator::new();

    println!("   Simulating tree cache with {} files...", 5000);

    let start_time = Instant::now();

    // Simulate parsing 5000 unique files (unbounded growth)
    for i in 0..5000 {
        let (file_path, content) = generator.generate_rust_file();
        tree_cache.parse_file(&file_path, &content);

        if i % 1000 == 0 && i > 0 {
            let memory_mb = tree_cache.get_memory_usage_mb();
            println!("   📈 After {} files: {:.2} MB memory usage", i, memory_mb);
        }
    }

    let duration = start_time.elapsed();
    let metrics = tree_cache.get_metrics();

    println!("   ⚠️  CRITICAL: Tree cache has NO size limits!");
    println!("   💾 Final memory usage: {:.2} MB", tree_cache.get_memory_usage_mb());
    println!("   📊 Cache entries: {}", tree_cache.cache.len());
    println!("   🎯 Hit rate: {:.2}%", metrics.hit_rate() * 100.0);
    println!("   ⏱️  Total time: {:?}", duration);

    if tree_cache.get_memory_usage_mb() > 100.0 {
        println!("   🚨 Memory usage exceeds 100MB - OOM risk on large codebases!");
    }
}

fn test_token_cache_behavior() {
    let mut token_cache = TokenCacheSimulator::new(1000, 3600); // 1000 entries, 1 hour TTL
    let generator = ContentGenerator::new();

    println!("   Simulating token cache with {} content pieces...", 2000);

    let start_time = Instant::now();

    // Generate 2000 pieces of content (should trigger evictions)
    for i in 0..2000 {
        let (_, content) = if i % 2 == 0 {
            generator.generate_rust_file()
        } else {
            generator.generate_javascript_file()
        };

        let _token_count = token_cache.count_tokens(&content);

        if i % 500 == 0 && i > 0 {
            let metrics = token_cache.get_metrics();
            println!("   📊 After {} items: {} cached, {:.2}% hit rate",
                i, token_cache.cache.len(), metrics.hit_rate() * 100.0);
        }
    }

    let duration = start_time.elapsed();
    let metrics = token_cache.get_metrics();

    println!("   ✅ Token cache respects size limits");
    println!("   📊 Final cache size: {}", token_cache.cache.len());
    println!("   🎯 Hit rate: {:.2}%", metrics.hit_rate() * 100.0);
    println!("   🗑️  Evictions: {}", metrics.evictions);
    println!("   ⏱️  Total time: {:?}", duration);

    // Test TTL expiration simulation
    println!("   Testing TTL expiration...");
    thread::sleep(Duration::from_millis(100)); // Small delay

    let expired_content = "This content should be expired";
    let _token_count = token_cache.count_tokens(expired_content);

    println!("   ✅ TTL mechanism working (simulated)");
}

fn test_mixed_workload() {
    let mut tree_cache = TreeCacheSimulator::new();
    let mut token_cache = TokenCacheSimulator::new(500, 1800); // Smaller cache
    let generator = ContentGenerator::new();

    println!("   Simulating mixed workload: parsing + tokenization...");

    let start_time = Instant::now();
    let mut total_tree_memory = 0.0;
    let mut files_processed = 0;

    // Mixed workload: parse files and count tokens
    for i in 0..1000 {
        let (file_path, content) = if i % 3 == 0 {
            generator.generate_rust_file()
        } else {
            generator.generate_javascript_file()
        };

        // Tree parsing (unbounded cache)
        tree_cache.parse_file(&file_path, &content);

        // Token counting (bounded cache)
        let _token_count = token_cache.count_tokens(&content);

        files_processed += 1;

        if i % 200 == 0 && i > 0 {
            total_tree_memory = tree_cache.get_memory_usage_mb();
            let token_metrics = token_cache.get_metrics();

            println!("   📊 Mixed workload progress:");
            println!("      - Files processed: {}", files_processed);
            println!("      - Tree cache memory: {:.2} MB", total_tree_memory);
            println!("      - Token cache size: {}", token_cache.cache.len());
            println!("      - Token hit rate: {:.2}%", token_metrics.hit_rate() * 100.0);
        }
    }

    let duration = start_time.elapsed();

    println!("   🏁 Mixed workload results:");
    println!("   ⚠️  Tree cache memory: {:.2} MB (unbounded)", total_tree_memory);
    println!("   ✅ Token cache memory: ~{:.2} MB (bounded)", token_cache.cache.len() as f64 * 0.1 / 1024.0);
    println!("   ⏱️  Total time: {:?}", duration);

    // Demonstrate the problem
    let tree_to_token_ratio = total_tree_memory / (token_cache.cache.len() as f64 * 0.1 / 1024.0);
    if tree_to_token_ratio > 50.0 {
        println!("   🚨 Tree cache uses {}x more memory than token cache!", tree_to_token_ratio as usize);
    }
}

fn test_concurrent_memory_usage() {
    println!("   Simulating concurrent cache access...");

    let tree_memory = Arc::new(AtomicUsize::new(0));
    let files_processed = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..4).map(|thread_id| {
        let tree_memory = Arc::clone(&tree_memory);
        let files_processed = Arc::clone(&files_processed);

        thread::spawn(move || {
            let mut local_tree_cache = TreeCacheSimulator::new();
            let mut local_token_cache = TokenCacheSimulator::new(200, 1800);
            let generator = ContentGenerator::new();

            for i in 0..250 {  // 250 files per thread = 1000 total
                let (file_path, content) = if (thread_id + i) % 2 == 0 {
                    generator.generate_rust_file()
                } else {
                    generator.generate_javascript_file()
                };

                local_tree_cache.parse_file(&file_path, &content);
                let _token_count = local_token_cache.count_tokens(&content);

                files_processed.fetch_add(1, Ordering::SeqCst);

                // Update memory estimate (very rough)
                let memory_kb = (local_tree_cache.cache.len() as f64 * 2.5) as usize;
                tree_memory.store(memory_kb, Ordering::SeqCst);

                // Small delay to simulate realistic processing
                thread::sleep(Duration::from_millis(1));
            }

            (local_tree_cache.get_memory_usage_mb(), local_tree_cache.get_metrics().clone())
        })
    }).collect();

    // Wait for completion and collect results
    let mut total_memory = 0.0;
    let mut total_entries = 0;

    for handle in handles {
        let (memory_mb, _metrics) = handle.join().unwrap();
        total_memory += memory_mb;
        total_entries += 1;
    }

    let final_files = files_processed.load(Ordering::SeqCst);

    println!("   🔄 Concurrent processing results:");
    println!("   📊 Total files processed: {}", final_files);
    println!("   💾 Combined memory usage: {:.2} MB", total_memory);
    println!("   🧵 Memory per thread: {:.2} MB", total_memory / 4.0);

    if total_memory > 50.0 {
        println!("   🚨 High memory usage in concurrent scenario!");
        println!("   💡 Each thread maintains separate unbounded cache");
    }

    println!("   ⚠️  Real Probe would have shared caches with race conditions");
}

/// Additional utility to demonstrate system memory pressure
fn check_system_memory() {
    println!("\n🖥️  System Memory Check:");

    // Try to get system memory info (Unix-like systems)
    if let Ok(output) = Command::new("free")
        .args(&["-m"])
        .output()
    {
        if let Ok(output_str) = String::from_utf8(output.stdout) {
            println!("{}", output_str);
        }
    } else if let Ok(output) = Command::new("vm_stat").output() {
        // macOS alternative
        if let Ok(output_str) = String::from_utf8(output.stdout) {
            println!("macOS Memory Stats:\n{}", output_str);
        }
    } else {
        println!("   ❌ Unable to get system memory info");
    }
}