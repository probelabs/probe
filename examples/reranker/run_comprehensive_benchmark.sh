#!/bin/bash

echo "🚀 COMPREHENSIVE RERANKER PERFORMANCE ANALYSIS"
echo "=============================================="
echo ""

# Build first
echo "Building release binary..."
cargo build --release
echo ""

# Test different document counts
echo "=== SCALABILITY ANALYSIS ==="
echo ""

echo "Testing with 100 documents:"
./target/release/benchmark --demo --query "search algorithm implementation" --num-docs 100 --iterations 5 --batch-size 20

echo ""
echo "Testing with 500 documents:"
./target/release/benchmark --demo --query "search algorithm implementation" --num-docs 500 --iterations 5 --batch-size 50

echo ""
echo "Testing with 1000 documents:"
./target/release/benchmark --demo --query "search algorithm implementation" --num-docs 1000 --iterations 5 --batch-size 100

echo ""
echo "Testing with 2000 documents:"
./target/release/benchmark --demo --query "search algorithm implementation" --num-docs 2000 --iterations 3 --batch-size 200

echo ""
echo "=== QUERY COMPLEXITY ANALYSIS ==="
echo ""

echo "Simple query (2 words):"
./target/release/benchmark --demo --query "rust async" --num-docs 500 --iterations 5 --batch-size 50

echo ""
echo "Medium query (4 words):"
./target/release/benchmark --demo --query "vector search embedding similarity" --num-docs 500 --iterations 5 --batch-size 50

echo ""
echo "Complex query (8 words):"
./target/release/benchmark --demo --query "distributed search engine indexing algorithm optimization performance tuning" --num-docs 500 --iterations 5 --batch-size 50

echo ""
echo "=== BATCH SIZE OPTIMIZATION ==="
echo ""

echo "Batch size 10:"
./target/release/benchmark --demo --query "machine learning model" --num-docs 500 --iterations 3 --batch-size 10

echo ""
echo "Batch size 50:"
./target/release/benchmark --demo --query "machine learning model" --num-docs 500 --iterations 3 --batch-size 50

echo ""
echo "Batch size 100:"
./target/release/benchmark --demo --query "machine learning model" --num-docs 500 --iterations 3 --batch-size 100

echo ""
echo "Batch size 250:"
./target/release/benchmark --demo --query "machine learning model" --num-docs 500 --iterations 3 --batch-size 250

echo ""
echo "=== FILE TYPE ANALYSIS ==="
echo ""

echo "Only Rust files:"
./target/release/benchmark --demo --query "struct impl trait" --num-docs 200 --iterations 5 --batch-size 40 --extensions rs

echo ""
echo "Only JavaScript/TypeScript files:"
./target/release/benchmark --demo --query "async function promise" --num-docs 200 --iterations 5 --batch-size 40 --extensions js --extensions ts

echo ""
echo "Multiple file types:"
./target/release/benchmark --demo --query "algorithm optimization" --num-docs 200 --iterations 5 --batch-size 40 --extensions rs --extensions js --extensions ts --extensions go --extensions py --extensions java

echo ""
echo "=============================================="
echo "✅ COMPREHENSIVE BENCHMARK COMPLETE"
echo "=============================================="