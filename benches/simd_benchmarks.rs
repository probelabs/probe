use ahash::AHashMap as HashMap;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use probe_code::ranking::{
    rank_documents, rank_documents_simd, rank_documents_simd_simple, RankingParams,
};
use probe_code::simd_ranking::SparseVector;
use rand::prelude::*;

/// Generate synthetic documents for benchmarking
fn generate_synthetic_docs(num_docs: usize, vocab_size: usize) -> Vec<String> {
    let mut rng = rand::thread_rng();
    let mut documents = Vec::with_capacity(num_docs);

    // Create a vocabulary of common programming terms
    let vocab: Vec<String> = (0..vocab_size)
        .map(|i| match i % 20 {
            0..=4 => format!("function_{}", i),
            5..=9 => format!("class_{}", i),
            10..=14 => format!("method_{}", i),
            15..=19 => format!("variable_{}", i),
            _ => format!("token_{}", i),
        })
        .collect();

    for _ in 0..num_docs {
        let doc_length = rng.gen_range(20..200); // Realistic document lengths
        let mut doc_tokens = Vec::with_capacity(doc_length);

        for _ in 0..doc_length {
            let token_idx = rng.gen_range(0..vocab_size);
            doc_tokens.push(vocab[token_idx].clone());
        }

        documents.push(doc_tokens.join(" "));
    }

    documents
}

/// Generate realistic code-like documents
fn generate_code_documents(num_docs: usize) -> Vec<String> {
    let mut rng = rand::thread_rng();
    let mut documents = Vec::with_capacity(num_docs);

    let functions = [
        "process",
        "handle",
        "execute",
        "compute",
        "calculate",
        "transform",
        "validate",
    ];
    let objects = [
        "data",
        "input",
        "output",
        "result",
        "value",
        "parameter",
        "config",
    ];
    let operations = [
        "load", "save", "parse", "format", "convert", "merge", "filter",
    ];

    for _ in 0..num_docs {
        let mut doc = String::new();

        // Add some function definitions
        for _ in 0..rng.gen_range(1..5) {
            let func = functions[rng.gen_range(0..functions.len())];
            let obj = objects[rng.gen_range(0..objects.len())];
            let op = operations[rng.gen_range(0..operations.len())];

            doc.push_str(&format!("function {}{}() {{\n", func, obj));
            doc.push_str(&format!("    let {} = {}();\n", obj, op));
            doc.push_str(&format!("    return {}.{}();\n", obj, op));
            doc.push_str("}\n\n");
        }

        // Add some variable declarations
        for _ in 0..rng.gen_range(2..8) {
            let obj = objects[rng.gen_range(0..objects.len())];
            let val = rng.gen_range(0..1000);
            doc.push_str(&format!("const {} = {};\n", obj, val));
        }

        documents.push(doc);
    }

    documents
}

/// Benchmark sparse vector operations
fn bench_sparse_vector_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_vector_ops");

    // Create test sparse vectors of different sizes
    let sizes = [10, 50, 100, 500];

    for &size in &sizes {
        let mut tf_map1 = HashMap::new();
        let mut tf_map2 = HashMap::new();

        // Create overlapping sparse vectors
        for i in 0..size {
            tf_map1.insert(i as u8, (i % 10) + 1);
            if i % 3 == 0 {
                // 33% overlap
                tf_map2.insert(i as u8, (i % 5) + 1);
            }
        }

        let sparse1 = SparseVector::from_tf_map(&tf_map1);
        let sparse2 = SparseVector::from_tf_map(&tf_map2);

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("dot_product", size),
            &(&sparse1, &sparse2),
            |b, (v1, v2)| b.iter(|| black_box(v1.dot_product(v2))),
        );

        group.bench_with_input(
            BenchmarkId::new("intersection", size),
            &(&sparse1, &sparse2),
            |b, (v1, v2)| b.iter(|| black_box(v1.intersect_indices(v2))),
        );
    }

    group.finish();
}

/// Benchmark traditional vs SIMD ranking on synthetic data
fn bench_ranking_synthetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("ranking_synthetic");

    let doc_counts = [100, 500, 1000, 2000];
    let vocab_size = 1000;

    for &num_docs in &doc_counts {
        let documents = generate_synthetic_docs(num_docs, vocab_size);
        let doc_refs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();

        let queries = [
            "function process data",
            "class method handle",
            "variable input output load",
        ];

        for query in queries {
            let params = RankingParams {
                documents: &doc_refs,
                query,
                pre_tokenized: None,
            };

            group.throughput(Throughput::Elements(num_docs as u64));

            group.bench_with_input(
                BenchmarkId::new(format!("traditional_{}", query.replace(' ', "_")), num_docs),
                &params,
                |b, params| b.iter(|| black_box(rank_documents(params))),
            );

            group.bench_with_input(
                BenchmarkId::new(format!("simd_{}", query.replace(' ', "_")), num_docs),
                &params,
                |b, params| b.iter(|| black_box(rank_documents_simd(params))),
            );

            group.bench_with_input(
                BenchmarkId::new(format!("simd_simple_{}", query.replace(' ', "_")), num_docs),
                &params,
                |b, params| b.iter(|| black_box(rank_documents_simd_simple(params))),
            );
        }
    }

    group.finish();
}

/// Benchmark traditional vs SIMD ranking on realistic code documents
fn bench_ranking_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("ranking_realistic");

    let doc_counts = [50, 200, 500, 1000];

    for &num_docs in &doc_counts {
        let documents = generate_code_documents(num_docs);
        let doc_refs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();

        let queries = [
            "function process data load",
            "class method handle input",
            "variable config parameter validate",
            "execute compute transform merge",
        ];

        for query in queries {
            let params = RankingParams {
                documents: &doc_refs,
                query,
                pre_tokenized: None,
            };

            group.throughput(Throughput::Elements(num_docs as u64));

            group.bench_with_input(
                BenchmarkId::new(format!("traditional_{}", query.replace(' ', "_")), num_docs),
                &params,
                |b, params| b.iter(|| black_box(rank_documents(params))),
            );

            group.bench_with_input(
                BenchmarkId::new(format!("simd_{}", query.replace(' ', "_")), num_docs),
                &params,
                |b, params| b.iter(|| black_box(rank_documents_simd(params))),
            );

            group.bench_with_input(
                BenchmarkId::new(format!("simd_simple_{}", query.replace(' ', "_")), num_docs),
                &params,
                |b, params| b.iter(|| black_box(rank_documents_simd_simple(params))),
            );
        }
    }

    group.finish();
}

/// Benchmark memory usage and allocation patterns
fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");

    let num_docs = 1000;
    let documents = generate_code_documents(num_docs);
    let doc_refs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();

    let query = "function process data load validate";
    let params = RankingParams {
        documents: &doc_refs,
        query,
        pre_tokenized: None,
    };

    group.bench_function("traditional_memory", |b| {
        b.iter(|| {
            let _results = black_box(rank_documents(&params));
            // Memory will be automatically freed here
        })
    });

    group.bench_function("simd_memory", |b| {
        b.iter(|| {
            let _results = black_box(rank_documents_simd(&params));
            // Memory will be automatically freed here
        })
    });

    group.bench_function("simd_simple_memory", |b| {
        b.iter(|| {
            let _results = black_box(rank_documents_simd_simple(&params));
            // Memory will be automatically freed here
        })
    });

    group.finish();
}

/// Benchmark with varying query complexities
fn bench_query_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_complexity");

    let num_docs = 500;
    let documents = generate_code_documents(num_docs);
    let doc_refs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();

    let queries = [
        ("simple", "function"),
        ("medium", "function process data"),
        ("complex", "function process data load validate transform"),
        ("boolean", "+function +process -deprecated"),
        ("mixed", "function OR method AND (data OR input)"),
    ];

    for (complexity, query) in queries {
        let params = RankingParams {
            documents: &doc_refs,
            query,
            pre_tokenized: None,
        };

        group.bench_with_input(
            BenchmarkId::new("traditional", complexity),
            &params,
            |b, params| b.iter(|| black_box(rank_documents(params))),
        );

        group.bench_with_input(
            BenchmarkId::new("simd", complexity),
            &params,
            |b, params| b.iter(|| black_box(rank_documents_simd(params))),
        );

        // Simple SIMD works best with non-boolean queries
        if !query.contains(['+', '-', '(', ')']) {
            group.bench_with_input(
                BenchmarkId::new("simd_simple", complexity),
                &params,
                |b, params| b.iter(|| black_box(rank_documents_simd_simple(params))),
            );
        }
    }

    group.finish();
}

/// Benchmark scalability with increasing document sizes
fn bench_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability");
    group.sample_size(20); // Fewer samples for large datasets

    let doc_counts = [100, 500, 1000, 2500, 5000];
    let query = "function process data load";

    for &num_docs in &doc_counts {
        let documents = generate_synthetic_docs(num_docs, 500);
        let doc_refs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();

        let params = RankingParams {
            documents: &doc_refs,
            query,
            pre_tokenized: None,
        };

        group.throughput(Throughput::Elements(num_docs as u64));

        group.bench_with_input(
            BenchmarkId::new("traditional", num_docs),
            &params,
            |b, params| b.iter(|| black_box(rank_documents(params))),
        );

        group.bench_with_input(BenchmarkId::new("simd", num_docs), &params, |b, params| {
            b.iter(|| black_box(rank_documents_simd(params)))
        });

        group.bench_with_input(
            BenchmarkId::new("simd_simple", num_docs),
            &params,
            |b, params| b.iter(|| black_box(rank_documents_simd_simple(params))),
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_sparse_vector_ops,
    bench_ranking_synthetic,
    bench_ranking_realistic,
    bench_memory_efficiency,
    bench_query_complexity,
    bench_scalability
);

criterion_main!(benches);
