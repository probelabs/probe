use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

use crate::bert_simulator::{BertPerformanceStats, BertSimulator};
use crate::parallel_reranker::ParallelBertReranker;
use crate::reranker::BertReranker;

#[derive(Parser, Clone)]
#[command(name = "benchmark")]
#[command(about = "Performance benchmarking tool for BERT reranker")]
pub struct BenchmarkArgs {
    /// Query to use for reranking
    #[arg(short, long, default_value = "search algorithm implementation")]
    query: String,

    /// Path to collect source files from
    #[arg(short, long, default_value = "../..")]
    source_path: PathBuf,

    /// Number of documents to test with
    #[arg(short, long, default_value = "100")]
    num_docs: usize,

    /// Number of benchmark iterations
    #[arg(short, long, default_value = "5")]
    iterations: usize,

    /// Maximum file size to include (in KB)
    #[arg(long, default_value = "50")]
    max_file_size_kb: usize,

    /// File extensions to include
    #[arg(long, default_values = &["rs", "js", "ts", "py", "go", "java"])]
    extensions: Vec<String>,

    /// Use demo mode (mock reranker)
    #[arg(long)]
    demo: bool,

    /// Use BERT simulator (realistic BERT performance simulation)
    #[arg(long)]
    simulate: bool,

    /// Use parallel BERT processing across CPU cores
    #[arg(long)]
    parallel: bool,

    /// Number of threads for parallel processing (auto-detected if not specified)
    #[arg(long)]
    num_threads: Option<usize>,

    /// Compare parallel vs sequential performance
    #[arg(long)]
    compare_modes: bool,

    /// Compare multiple BERT models (TinyBERT, MiniLM-L-2, MiniLM-L-6)
    #[arg(long)]
    compare_models: bool,

    /// Model to use for reranking
    #[arg(short, long, default_value = "cross-encoder/ms-marco-MiniLM-L-2-v2")]
    model: String,

    /// Batch size for processing
    #[arg(short, long, default_value = "10")]
    batch_size: usize,
}

pub struct Document {
    pub path: PathBuf,
    pub content: String,
    pub size_bytes: usize,
}

pub struct BenchmarkResult {
    pub total_time: Duration,
    pub docs_processed: usize,
    pub docs_per_second: f64,
    pub avg_time_per_doc: Duration,
    pub model_loading_time: Duration,
    pub actual_reranking_time: Duration,
}

impl BenchmarkResult {
    pub fn print_summary(&self) {
        println!("\n=== RERANKER PERFORMANCE BENCHMARK ===");
        println!("Documents processed: {}", self.docs_processed);
        println!("Total time: {:.2}s", self.total_time.as_secs_f64());
        println!(
            "Model loading time: {:.2}s",
            self.model_loading_time.as_secs_f64()
        );
        println!(
            "Actual reranking time: {:.2}s",
            self.actual_reranking_time.as_secs_f64()
        );
        println!(
            "Average time per document: {:.2}ms",
            self.avg_time_per_doc.as_millis()
        );
        println!("Throughput: {:.2} docs/second", self.docs_per_second);
        println!("=======================================");
    }
}

pub fn collect_source_files(args: &BenchmarkArgs) -> Result<Vec<Document>> {
    println!("Collecting source files from: {:?}", args.source_path);
    let mut documents = Vec::new();
    let max_bytes = args.max_file_size_kb * 1024;

    for entry in WalkDir::new(&args.source_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        // Check file extension
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                if !args.extensions.contains(&ext_str.to_string()) {
                    continue;
                }
            } else {
                continue;
            }
        } else {
            continue;
        }

        // Check file size
        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.len() > max_bytes as u64 {
            continue;
        }

        // Read file content
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Skip empty files
        if content.trim().is_empty() {
            continue;
        }

        documents.push(Document {
            path: path.to_path_buf(),
            content,
            size_bytes: metadata.len() as usize,
        });

        if documents.len() >= args.num_docs {
            break;
        }
    }

    println!("Collected {} source files", documents.len());
    if documents.is_empty() {
        anyhow::bail!("No source files found matching criteria");
    }

    Ok(documents)
}

pub async fn run_benchmark(args: BenchmarkArgs) -> Result<BenchmarkResult> {
    // Handle multi-model comparison
    if args.compare_models {
        return run_multi_model_comparison(args).await;
    }

    // Collect documents
    let documents = collect_source_files(&args)?;
    let total_docs = documents.len().min(args.num_docs);

    println!("\nStarting benchmark with {} documents...", total_docs);
    println!("Query: '{}'", args.query);
    println!("Model: {}", args.model);
    println!("Batch size: {}", args.batch_size);
    println!("Iterations: {}", args.iterations);

    // Initialize reranker
    let model_load_start = Instant::now();
    let (reranker, simulator, parallel_reranker) = if args.demo {
        println!("Using demo mode (mock reranker)");
        (None, None, None)
    } else if args.simulate {
        println!("Using BERT simulator (realistic performance simulation)");
        let simulator = BertSimulator::new();

        // Show performance characteristics
        let stats = if args.model.contains("L-6") {
            BertPerformanceStats::minilm_l6_cpu()
        } else {
            BertPerformanceStats::minilm_l2_cpu()
        };
        stats.print_comparison();

        (None, Some(simulator), None)
    } else if args.parallel || args.compare_modes {
        println!("Loading parallel BERT model...");
        let parallel_reranker = ParallelBertReranker::new(&args.model, args.num_threads)
            .await
            .context("Failed to load parallel BERT model")?;
        (None, None, Some(parallel_reranker))
    } else {
        println!("Loading BERT model...");
        let reranker = BertReranker::new(&args.model)
            .await
            .context("Failed to load BERT model")?;
        (Some(reranker), None, None)
    };
    let model_loading_time = model_load_start.elapsed();

    println!("Model loaded in {:.2}s", model_loading_time.as_secs_f64());

    let mut iteration_times = Vec::new();
    let mut total_reranking_time = Duration::new(0, 0);

    // Run benchmark iterations
    for iteration in 1..=args.iterations {
        println!("\nRunning iteration {}/{}", iteration, args.iterations);

        let iteration_start = Instant::now();
        let rerank_start = Instant::now();

        if let Some(ref parallel_reranker) = parallel_reranker {
            // Parallel BERT reranking
            let docs: Vec<&str> = documents[..total_docs]
                .iter()
                .map(|d| d.content.as_str())
                .collect();

            if args.compare_modes {
                println!(
                    "Running comparison: parallel vs sequential (iteration {}/{})",
                    iteration, args.iterations
                );

                // Run sequential first
                let seq_start = Instant::now();
                let _seq_results = parallel_reranker
                    .rerank_sequential(&args.query, &docs)
                    .context("Failed to rerank documents sequentially")?;
                let seq_time = seq_start.elapsed();

                // Run parallel
                let par_start = Instant::now();
                let _par_results = parallel_reranker
                    .rerank_parallel(&args.query, &docs)
                    .context("Failed to rerank documents in parallel")?;
                let par_time = par_start.elapsed();

                println!(
                    "  Sequential: {:.2}s ({:.1} docs/sec)",
                    seq_time.as_secs_f64(),
                    total_docs as f64 / seq_time.as_secs_f64()
                );
                println!(
                    "  Parallel:   {:.2}s ({:.1} docs/sec) - {:.1}x speedup",
                    par_time.as_secs_f64(),
                    total_docs as f64 / par_time.as_secs_f64(),
                    seq_time.as_secs_f64() / par_time.as_secs_f64()
                );
            } else {
                // Just run parallel
                let _results = parallel_reranker
                    .rerank_parallel(&args.query, &docs)
                    .context("Failed to rerank documents in parallel")?;
            }
        } else if let Some(ref reranker) = reranker {
            // Real BERT reranking - process in batches
            let mut batch_start = 0;
            while batch_start < total_docs {
                let batch_end = (batch_start + args.batch_size).min(total_docs);
                let batch_docs: Vec<&str> = documents[batch_start..batch_end]
                    .iter()
                    .map(|d| d.content.as_str())
                    .collect();

                let _results = reranker
                    .rerank(&args.query, &batch_docs)
                    .await
                    .context("Failed to rerank documents")?;

                batch_start = batch_end;
            }
        } else if let Some(ref simulator) = simulator {
            // BERT simulator - realistic performance simulation
            let docs: Vec<&str> = documents[..total_docs]
                .iter()
                .map(|d| d.content.as_str())
                .collect();

            let _results = simulator.rerank(&args.query, &docs);
        } else {
            // Demo mode - mock reranking
            let docs: Vec<&str> = documents[..total_docs]
                .iter()
                .map(|d| d.content.as_str())
                .collect();

            let _results = mock_rerank(&args.query, &docs);
        }

        let rerank_time = rerank_start.elapsed();
        total_reranking_time += rerank_time;

        let iteration_time = iteration_start.elapsed();
        iteration_times.push(iteration_time);

        println!(
            "Iteration {} completed in {:.2}s (reranking: {:.2}s)",
            iteration,
            iteration_time.as_secs_f64(),
            rerank_time.as_secs_f64()
        );
    }

    // Calculate results
    let avg_iteration_time = Duration::from_nanos(
        iteration_times
            .iter()
            .map(|d| d.as_nanos() as u64)
            .sum::<u64>()
            / args.iterations as u64,
    );

    let avg_reranking_time = total_reranking_time / args.iterations as u32;
    let docs_per_second = total_docs as f64 / avg_reranking_time.as_secs_f64();
    let avg_time_per_doc = avg_reranking_time / total_docs as u32;

    Ok(BenchmarkResult {
        total_time: avg_iteration_time,
        docs_processed: total_docs,
        docs_per_second,
        avg_time_per_doc,
        model_loading_time,
        actual_reranking_time: avg_reranking_time,
    })
}

// Mock reranker for demo mode
fn mock_rerank(query: &str, documents: &[&str]) -> Vec<(usize, f32)> {
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();

    let mut scores: Vec<(usize, f32)> = documents
        .iter()
        .enumerate()
        .map(|(idx, doc)| {
            let doc_lower = doc.to_lowercase();
            let score = query_words
                .iter()
                .map(|word| {
                    if doc_lower.contains(word) {
                        // Count occurrences and boost score
                        doc_lower.matches(word).count() as f32 * 0.1
                    } else {
                        0.0
                    }
                })
                .sum::<f32>();

            (idx, score)
        })
        .collect();

    // Sort by score descending
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

pub fn print_document_stats(documents: &[Document]) {
    if documents.is_empty() {
        return;
    }

    let total_bytes: usize = documents.iter().map(|d| d.size_bytes).sum();
    let avg_bytes = total_bytes / documents.len();
    let max_bytes = documents.iter().map(|d| d.size_bytes).max().unwrap_or(0);
    let min_bytes = documents.iter().map(|d| d.size_bytes).min().unwrap_or(0);

    println!("\n=== DOCUMENT STATISTICS ===");
    println!("Total documents: {}", documents.len());
    println!("Total size: {:.2} KB", total_bytes as f64 / 1024.0);
    println!("Average size: {:.2} KB", avg_bytes as f64 / 1024.0);
    println!(
        "Size range: {:.2} KB - {:.2} KB",
        min_bytes as f64 / 1024.0,
        max_bytes as f64 / 1024.0
    );

    // Show file type distribution
    let mut extensions = std::collections::HashMap::new();
    for doc in documents {
        if let Some(ext) = doc.path.extension() {
            if let Some(ext_str) = ext.to_str() {
                *extensions.entry(ext_str.to_string()).or_insert(0) += 1;
            }
        }
    }

    println!("File types:");
    for (ext, count) in extensions {
        println!("  .{}: {} files", ext, count);
    }
    println!("===========================");
}

pub async fn run_multi_model_comparison(args: BenchmarkArgs) -> Result<BenchmarkResult> {
    println!("🔬 MULTI-MODEL BERT COMPARISON");
    println!("==============================");

    let models = vec![
        (
            "cross-encoder/ms-marco-TinyBERT-L-2-v2",
            "TinyBERT-L2 (~4M params, fastest)",
        ),
        (
            "cross-encoder/ms-marco-MiniLM-L-2-v2",
            "MiniLM-L2 (~22M params, balanced)",
        ),
        (
            "cross-encoder/ms-marco-MiniLM-L-6-v2",
            "MiniLM-L6 (~85M params, most accurate)",
        ),
    ];

    // Collect documents
    let documents = collect_source_files(&args)?;
    let total_docs = documents.len().min(args.num_docs);

    print_document_stats(&documents);

    println!(
        "\nComparing {} models with {} documents...",
        models.len(),
        total_docs
    );
    println!("Query: '{}'", args.query);
    println!("Iterations: {}", args.iterations);

    let mut all_results = Vec::new();

    for (model_name, model_desc) in models {
        println!("\n🧠 Testing {}", model_desc);
        println!("Model: {}", model_name);
        println!("{}=", "=".repeat(60));

        // Create args for this model
        let mut model_args = args.clone();
        model_args.model = model_name.to_string();
        model_args.compare_models = false; // Prevent infinite recursion

        // Run benchmark for this model
        let result = run_single_model_benchmark(model_args).await?;

        println!("\n📊 {} Results:", model_desc);
        println!("  Throughput: {:.2} docs/second", result.docs_per_second);
        println!(
            "  Avg time per doc: {:.0}ms",
            result.avg_time_per_doc.as_millis()
        );
        println!(
            "  Model loading: {:.2}s",
            result.model_loading_time.as_secs_f64()
        );
        println!(
            "  Total time: {:.2}s",
            result.actual_reranking_time.as_secs_f64()
        );

        all_results.push((model_name, model_desc, result));
    }

    // Print comparison summary
    println!("\n🏆 MULTI-MODEL PERFORMANCE COMPARISON");
    println!("=====================================");
    println!(
        "{:<25} {:<15} {:<15} {:<15}",
        "Model", "Throughput", "Per-Doc Time", "Loading Time"
    );
    println!("{}", "-".repeat(75));

    for (model_name, model_desc, result) in &all_results {
        println!(
            "{:<25} {:<15.2} {:<15.0} {:<15.2}",
            model_desc.split(' ').next().unwrap_or(model_name),
            result.docs_per_second,
            result.avg_time_per_doc.as_millis(),
            result.model_loading_time.as_secs_f64()
        );
    }

    // Find fastest model
    let fastest = all_results
        .iter()
        .max_by(|a, b| {
            a.2.docs_per_second
                .partial_cmp(&b.2.docs_per_second)
                .unwrap()
        })
        .unwrap();

    println!(
        "\n🥇 WINNER: {} ({:.2} docs/sec)",
        fastest.1.split(' ').next().unwrap(),
        fastest.2.docs_per_second
    );

    // Return the first result (just for consistency)
    Ok(all_results.into_iter().next().unwrap().2)
}

async fn run_single_model_benchmark(args: BenchmarkArgs) -> Result<BenchmarkResult> {
    // This is the same as run_benchmark but without the multi-model check
    let documents = collect_source_files(&args)?;
    let total_docs = documents.len().min(args.num_docs);

    // Initialize reranker
    let model_load_start = Instant::now();
    let (reranker, simulator, parallel_reranker) = if args.demo {
        println!("Using demo mode (mock reranker)");
        (None, None, None)
    } else if args.simulate {
        println!("Using BERT simulator (realistic performance simulation)");
        let simulator = BertSimulator::new();
        (None, Some(simulator), None)
    } else if args.parallel || args.compare_modes {
        let parallel_reranker = ParallelBertReranker::new(&args.model, args.num_threads)
            .await
            .context("Failed to load parallel BERT model")?;
        (None, None, Some(parallel_reranker))
    } else {
        let reranker = BertReranker::new(&args.model)
            .await
            .context("Failed to load BERT model")?;
        (Some(reranker), None, None)
    };
    let model_loading_time = model_load_start.elapsed();

    let mut iteration_times = Vec::new();
    let mut total_reranking_time = Duration::new(0, 0);

    // Run benchmark iterations
    for iteration in 1..=args.iterations {
        let iteration_start = Instant::now();
        let rerank_start = Instant::now();

        // Process documents (simplified version without detailed logging)
        if let Some(ref parallel_reranker) = parallel_reranker {
            let docs: Vec<&str> = documents[..total_docs]
                .iter()
                .map(|d| d.content.as_str())
                .collect();
            let _results = parallel_reranker
                .rerank_parallel(&args.query, &docs)
                .context("Failed to rerank documents in parallel")?;
        } else if let Some(ref reranker) = reranker {
            let mut batch_start = 0;
            while batch_start < total_docs {
                let batch_end = (batch_start + args.batch_size).min(total_docs);
                let batch_docs: Vec<&str> = documents[batch_start..batch_end]
                    .iter()
                    .map(|d| d.content.as_str())
                    .collect();
                let _results = reranker
                    .rerank(&args.query, &batch_docs)
                    .await
                    .context("Failed to rerank documents")?;
                batch_start = batch_end;
            }
        } else if let Some(ref simulator) = simulator {
            let docs: Vec<&str> = documents[..total_docs]
                .iter()
                .map(|d| d.content.as_str())
                .collect();
            let _results = simulator.rerank(&args.query, &docs);
        } else {
            let docs: Vec<&str> = documents[..total_docs]
                .iter()
                .map(|d| d.content.as_str())
                .collect();
            let _results = mock_rerank(&args.query, &docs);
        }

        let rerank_time = rerank_start.elapsed();
        total_reranking_time += rerank_time;

        let iteration_time = iteration_start.elapsed();
        iteration_times.push(iteration_time);
    }

    // Calculate results
    let avg_iteration_time = Duration::from_nanos(
        iteration_times
            .iter()
            .map(|d| d.as_nanos() as u64)
            .sum::<u64>()
            / args.iterations as u64,
    );

    let avg_reranking_time = total_reranking_time / args.iterations as u32;
    let docs_per_second = total_docs as f64 / avg_reranking_time.as_secs_f64();
    let avg_time_per_doc = avg_reranking_time / total_docs as u32;

    Ok(BenchmarkResult {
        total_time: avg_iteration_time,
        docs_processed: total_docs,
        docs_per_second,
        avg_time_per_doc,
        model_loading_time,
        actual_reranking_time: avg_reranking_time,
    })
}
