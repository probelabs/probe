use anyhow::Result;
use clap::Parser;
use bert_reranker::benchmark::{BenchmarkArgs, run_benchmark, print_document_stats, collect_source_files};

#[tokio::main]
async fn main() -> Result<()> {
    let args = BenchmarkArgs::parse();
    
    println!("🚀 BERT Reranker Performance Benchmark");
    println!("======================================");
    
    // Collect documents first to show stats
    let documents = collect_source_files(&args)?;
    print_document_stats(&documents);
    
    // Run the benchmark
    let result = run_benchmark(args).await?;
    
    // Print results
    result.print_summary();
    
    Ok(())
}