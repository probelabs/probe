use anyhow::Result;
use bert_reranker::benchmark::{
    collect_source_files, print_document_stats, run_benchmark, BenchmarkArgs,
};
use clap::Parser;

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
