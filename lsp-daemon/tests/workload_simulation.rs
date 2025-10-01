#![cfg(feature = "legacy-tests")]
//! Real-world workload simulation for the null edge caching system
//!
//! Simulates realistic development scenarios with mixed cache hits/misses,
//! temporal locality, and different usage patterns to validate the system
//! under production-like conditions.

use anyhow::Result;
use lsp_daemon::database::sqlite_backend::SQLiteBackend;
use lsp_daemon::database::{
    create_none_call_hierarchy_edges, create_none_definition_edges,
    create_none_implementation_edges, create_none_reference_edges, DatabaseBackend, DatabaseConfig,
};
use rand::prelude::*;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Realistic project structure simulation
#[derive(Debug)]
pub struct ProjectStructure {
    pub modules: Vec<String>,
    pub functions_per_module: usize,
    pub classes_per_module: usize,
    pub methods_per_class: usize,
}

impl ProjectStructure {
    pub fn new_rust_project() -> Self {
        ProjectStructure {
            modules: vec![
                "src/main.rs".to_string(),
                "src/lib.rs".to_string(),
                "src/database/mod.rs".to_string(),
                "src/database/sqlite.rs".to_string(),
                "src/lsp/daemon.rs".to_string(),
                "src/lsp/protocol.rs".to_string(),
                "src/analyzer/mod.rs".to_string(),
                "src/cache/mod.rs".to_string(),
            ],
            functions_per_module: 10,
            classes_per_module: 2,
            methods_per_class: 5,
        }
    }

    pub fn generate_symbols(&self) -> Vec<String> {
        let mut symbols = Vec::new();

        for module in &self.modules {
            // Generate functions
            for i in 0..self.functions_per_module {
                symbols.push(format!("{}:function_{}:{}", module, i, (i * 10) + 5));
            }

            // Generate classes and methods
            for class_id in 0..self.classes_per_module {
                let class_symbol = format!("{}:Class{}:{}", module, class_id, (class_id * 20) + 50);
                symbols.push(class_symbol);

                for method_id in 0..self.methods_per_class {
                    symbols.push(format!(
                        "{}:Class{}::method_{}:{}",
                        module,
                        class_id,
                        method_id,
                        (method_id * 5) + 75
                    ));
                }
            }
        }

        symbols
    }
}

/// Metrics collection for workflow analysis
#[derive(Debug)]
pub struct WorkflowMetrics {
    pub workflow_name: String,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub cache_hit_times: Vec<Duration>,
    pub cache_miss_times: Vec<Duration>,
    pub total_duration: Duration,
}

impl WorkflowMetrics {
    pub fn new(workflow_name: &str) -> Self {
        WorkflowMetrics {
            workflow_name: workflow_name.to_string(),
            cache_hits: 0,
            cache_misses: 0,
            cache_hit_times: Vec::new(),
            cache_miss_times: Vec::new(),
            total_duration: Duration::from_nanos(0),
        }
    }

    pub fn record_cache_hit(&mut self) {
        self.cache_hits += 1;
    }

    pub fn record_cache_hit_with_time(&mut self, duration: Duration) {
        self.cache_hits += 1;
        self.cache_hit_times.push(duration);
    }

    pub fn record_cache_miss(&mut self, duration: Duration) {
        self.cache_misses += 1;
        self.cache_miss_times.push(duration);
    }

    pub fn cache_hit_rate(&self) -> f64 {
        if self.cache_hits + self.cache_misses == 0 {
            0.0
        } else {
            self.cache_hits as f64 / (self.cache_hits + self.cache_misses) as f64
        }
    }

    pub fn operations_per_second(&self) -> f64 {
        let total_ops = self.cache_hits + self.cache_misses;
        if self.total_duration.as_secs_f64() == 0.0 {
            0.0
        } else {
            total_ops as f64 / self.total_duration.as_secs_f64()
        }
    }

    pub fn print_report(&self) {
        println!("\\n📋 Workflow Report: {}", self.workflow_name);
        println!(
            "   Total operations:     {}",
            self.cache_hits + self.cache_misses
        );
        println!("   Cache hits:           {}", self.cache_hits);
        println!("   Cache misses:         {}", self.cache_misses);
        println!(
            "   Cache hit rate:       {:.1}%",
            self.cache_hit_rate() * 100.0
        );
        println!("   Duration:             {:?}", self.total_duration);
        println!(
            "   Operations per sec:   {:.1}",
            self.operations_per_second()
        );
    }
}

/// Real-world workload simulator
pub struct WorkloadSimulator {
    database: SQLiteBackend,
    workspace_id: i64,
    temp_dir: TempDir,
    project_symbols: Vec<String>,
    rng: StdRng,
}

impl WorkloadSimulator {
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("workload_simulation.db");

        let config = DatabaseConfig {
            path: Some(db_path),
            temporary: false,
            cache_capacity: 10 * 1024 * 1024, // 10MB for realistic simulation
            ..Default::default()
        };

        let database = SQLiteBackend::new(config).await?;
        let workspace_id = database
            .create_workspace("real_world_sim", 1, Some("main"))
            .await?;

        let project = ProjectStructure::new_rust_project();
        let project_symbols = project.generate_symbols();

        println!(
            "Generated {} realistic project symbols",
            project_symbols.len()
        );

        Ok(WorkloadSimulator {
            database,
            workspace_id,
            temp_dir,
            project_symbols,
            rng: StdRng::seed_from_u64(42), // Reproducible randomness
        })
    }

    /// Simulate debugging session with repeated queries
    pub async fn simulate_debugging_session(
        &mut self,
        focus_symbols: usize,
        repetitions: usize,
    ) -> Result<WorkflowMetrics> {
        println!(
            "🐛 Simulating debugging session: {} focus symbols, {} repetitions",
            focus_symbols, repetitions
        );

        let mut metrics = WorkflowMetrics::new("Debugging Session");
        let start_time = Instant::now();

        // Select symbols to focus on
        let focus_set: Vec<_> = (0..focus_symbols)
            .map(|_| self.rng.gen_range(0..self.project_symbols.len()))
            .collect();

        // First pass - cache misses
        for &symbol_idx in &focus_set {
            let symbol_uid = &self.project_symbols[symbol_idx];

            let query_start = Instant::now();
            let result = self
                .database
                .get_call_hierarchy_for_symbol(self.workspace_id, symbol_uid)
                .await?;
            let query_duration = query_start.elapsed();

            if result.is_none() {
                metrics.record_cache_miss(query_duration);
                let none_edges = create_none_call_hierarchy_edges(symbol_uid);
                self.database.store_edges(&none_edges).await?;
            }
        }

        // Repeated queries (debugging pattern)
        for _ in 0..repetitions {
            for &symbol_idx in &focus_set {
                let symbol_uid = &self.project_symbols[symbol_idx];

                let query_start = Instant::now();
                let result = self
                    .database
                    .get_call_hierarchy_for_symbol(self.workspace_id, symbol_uid)
                    .await?;
                let query_duration = query_start.elapsed();

                if result.is_some() {
                    metrics.record_cache_hit_with_time(query_duration);
                } else {
                    metrics.record_cache_miss(query_duration);
                }
            }
        }

        metrics.total_duration = start_time.elapsed();
        Ok(metrics)
    }
}

#[tokio::test]
async fn test_debugging_session_workflow() -> Result<()> {
    let mut simulator = WorkloadSimulator::new().await?;

    let metrics = simulator.simulate_debugging_session(10, 15).await?;
    metrics.print_report();

    // Validate debugging characteristics
    assert!(
        metrics.cache_hit_rate() > 0.5,
        "Debugging should have high cache hit rate due to repetition"
    );
    assert!(
        metrics.operations_per_second() > 100.0,
        "Should be faster due to cache hits"
    );

    println!("✅ Debugging session workflow test passed");
    Ok(())
}

#[tokio::test]
async fn test_mixed_realistic_workload() -> Result<()> {
    println!("🌍 Comprehensive Real-World Workload Simulation");

    let mut simulator = WorkloadSimulator::new().await?;
    let overall_start = Instant::now();

    // Simulate debugging session
    let debugging_metrics = simulator.simulate_debugging_session(8, 10).await?;

    let overall_duration = overall_start.elapsed();

    // Print comprehensive report
    println!("\\n🎯 Comprehensive Real-World Workload Results:");
    debugging_metrics.print_report();

    // Calculate aggregate metrics
    let total_operations = debugging_metrics.cache_hits + debugging_metrics.cache_misses;
    let total_hits = debugging_metrics.cache_hits;

    let overall_hit_rate = total_hits as f64 / total_operations as f64;
    let overall_throughput = total_operations as f64 / overall_duration.as_secs_f64();

    println!("\\n🏆 Aggregate Real-World Performance:");
    println!("   Total operations:      {}", total_operations);
    println!("   Overall hit rate:      {:.1}%", overall_hit_rate * 100.0);
    println!("   Overall duration:      {:?}", overall_duration);
    println!(
        "   Overall throughput:    {:.1} ops/sec",
        overall_throughput
    );

    // Validate realistic performance expectations
    assert!(
        total_operations > 100,
        "Should generate substantial realistic workload"
    );
    assert!(
        overall_hit_rate > 0.3,
        "Should achieve reasonable cache efficiency in mixed workload"
    );
    assert!(
        overall_throughput > 50.0,
        "Should maintain good performance under realistic load"
    );

    println!("✅ Comprehensive real-world workload simulation passed");
    Ok(())
}
