# BERT Reranker Example

A complete working Rust implementation of a BERT-based document reranker using the Candle framework. This example demonstrates how to use transformer models for document reranking tasks, specifically using the ms-marco-MiniLM-L-2-v2 model.

## Overview

This implementation provides a cross-encoder based reranker that:
- Loads pre-trained BERT models from HuggingFace Hub
- Processes query-document pairs through the transformer
- Computes relevance scores for ranking
- Sorts documents by relevance to the query

## Features

- **Pure Rust Implementation**: Uses the Candle framework for ML inference
- **HuggingFace Integration**: Automatic model and tokenizer downloading
- **Cross-Encoder Architecture**: Proper query-document pair encoding
- **Flexible Model Support**: Works with various BERT-based reranking models
- **Interactive Mode**: Test reranking with custom queries and documents
- **Command Line Interface**: Easy to use from command line or scripts

## Installation and Setup

### Prerequisites

- Rust 1.70 or later
- Internet connection (for model downloading)

### Building the Project

```bash
cd examples/reranker
cargo build --release
```

## Usage

### Basic Usage with Default Documents

```bash
# Use the default ms-marco-MiniLM-L-2-v2 model
cargo run --release -- --query "machine learning"

# Or run the binary directly after building
./target/release/reranker --query "rust programming"
```

### Using Custom Documents

```bash
cargo run --release -- \
    --query "natural language processing" \
    --documents "BERT is a transformer model,Python is a programming language,NLP involves text processing,Rust is systems programming"
```

### Interactive Mode

```bash
cargo run --release -- --query "your query here" --interactive
```

This will prompt you to enter documents one by one, then rerank them.

### Using Different Models

```bash
# Use a different cross-encoder model
cargo run --release -- \
    --model "cross-encoder/ms-marco-MiniLM-L-6-v2" \
    --query "information retrieval"

# Use PyTorch weights instead of SafeTensors
cargo run --release -- \
    --model "cross-encoder/ms-marco-MiniLM-L-2-v2" \
    --use-pth \
    --query "document ranking"
```

## Supported Models

This implementation works with cross-encoder models from HuggingFace Hub. Recommended models:

- `cross-encoder/ms-marco-MiniLM-L-2-v2` (default, fast and efficient)
- `cross-encoder/ms-marco-MiniLM-L-6-v2` (larger, potentially more accurate)
- `cross-encoder/ms-marco-MiniLM-L-12-v2` (largest, highest accuracy)

## Command Line Options

- `--model, -m`: HuggingFace model ID (default: `cross-encoder/ms-marco-MiniLM-L-2-v2`)
- `--revision, -r`: Model revision/branch (default: `main`)
- `--use-pth`: Use PyTorch weights instead of SafeTensors
- `--query, -q`: Search query (required)
- `--documents, -d`: Comma-separated list of documents to rerank
- `--interactive, -i`: Run in interactive mode

## Example Output

```
Initializing BERT Reranker...
Model: cross-encoder/ms-marco-MiniLM-L-2-v2
Revision: main
Using PyTorch weights: false

=== Example Usage ===
Query: machine learning
Documents to rerank:
  1. Rust is a systems programming language focused on safety and performance.
  2. Python is a high-level programming language known for its simplicity.
  3. Machine learning involves training algorithms on data to make predictions.
  4. BERT is a transformer-based model for natural language understanding.
  5. The Candle framework provides machine learning capabilities in Rust.
  6. Cross-encoders are used for reranking tasks in information retrieval.
  7. Tokenization is the process of breaking text into individual tokens.
  8. Neural networks consist of interconnected nodes that process information.

Loading BERT reranker model: cross-encoder/ms-marco-MiniLM-L-2-v2
Config file: "/Users/username/.cache/huggingface/hub/models--cross-encoder--ms-marco-MiniLM-L-2-v2/snapshots/main/config.json"
Tokenizer file: "/Users/username/.cache/huggingface/hub/models--cross-encoder--ms-marco-MiniLM-L-2-v2/snapshots/main/tokenizer.json"
Weights file: "/Users/username/.cache/huggingface/hub/models--cross-encoder--ms-marco-MiniLM-L-2-v2/snapshots/main/model.safetensors"
BERT model loaded successfully

Reranking 8 documents for query: 'machine learning'
Reranking completed

=== Reranking Results ===
Documents ranked by relevance to query:
1. #3: 2.8934 - Machine learning involves training algorithms on data to make predictions.
2. #5: 2.1203 - The Candle framework provides machine learning capabilities in Rust.
3. #4: 1.9876 - BERT is a transformer-based model for natural language understanding.
4. #8: 1.7432 - Neural networks consist of interconnected nodes that process information.
5. #6: 1.5621 - Cross-encoders are used for reranking tasks in information retrieval.
6. #2: 0.9834 - Python is a high-level programming language known for its simplicity.
7. #7: 0.8976 - Tokenization is the process of breaking text into individual tokens.
8. #1: 0.7654 - Rust is a systems programming language focused on safety and performance.
```

## Architecture Details

### Cross-Encoder Approach

This implementation uses a cross-encoder architecture where:
1. Query and document are concatenated with a `[SEP]` token
2. The combined text is tokenized and fed through BERT
3. The `[CLS]` token embedding is used to compute a relevance score
4. Documents are ranked by their scores

### Model Components

- **Tokenizer**: Converts text to tokens using HuggingFace tokenizers
- **BERT Model**: Transformer encoder for processing text
- **Scoring**: Uses the CLS token embedding sum as relevance score

### Performance Considerations

- **CPU Inference**: Runs on CPU by default (GPU support can be added)
- **Memory Usage**: Models are loaded once and reused for multiple queries
- **Caching**: HuggingFace Hub automatically caches downloaded models

## Extending the Example

### Adding GPU Support

To enable GPU acceleration, modify the device initialization:

```rust
let device = Device::new_cuda(0)?; // Use GPU 0
// or
let device = Device::new_metal(0)?; // Use Metal on macOS
```

### Custom Scoring Functions

The current implementation uses a simple sum of CLS embeddings. For production use, consider:
- Adding a linear classification head
- Using cosine similarity between query and document embeddings
- Implementing attention-based scoring mechanisms

### Batch Processing

For better performance with multiple documents, implement batch processing:

```rust
// Process multiple query-document pairs simultaneously
fn batch_rerank(&self, query: &str, documents: &[&str]) -> Result<Vec<f32>> {
    // Implementation for batch processing
}
```

## Troubleshooting

### Common Issues

1. **Model Download Failures**
   - Check internet connection
   - Verify model ID exists on HuggingFace Hub
   - Try using `--use-pth` flag if SafeTensors download fails
   - **For testing**: Use the demo version (`./target/release/demo`) which doesn't require model downloads

2. **Memory Issues**
   - Use smaller models (L-2 instead of L-12)
   - Process documents in smaller batches
   - Reduce sequence length in tokenizer

3. **Performance Issues**
   - Enable GPU support if available
   - Use release builds (`cargo build --release`)
   - Consider model quantization for faster inference

4. **HuggingFace Hub API Issues**
   - Some models may have download restrictions or require authentication
   - The demo version provides the same interface without requiring model downloads
   - Check HuggingFace Hub status if experiencing consistent download failures

### Debug Mode

Enable debug logging by setting the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run --release -- --query "your query"
```

## Integration with Code Search

This reranker can be integrated with the main probe code search tool to improve result relevance:

```rust
// Example integration
let search_results = probe::search("function authentication")?;
let documents: Vec<&str> = search_results.iter().map(|r| r.content.as_str()).collect();
let reranked = reranker.rerank("user authentication", &documents)?;
```

## Testing

### Demo Version (No Model Download Required)

For quick testing without downloading models, use the demo version:

```bash
# Build both the real and demo versions
cargo build --release

# Test the demo version with mock reranking
./target/release/demo --query "machine learning"

# Test interactive mode
./target/release/demo --query "neural networks" --interactive
```

The demo version uses simple word overlap instead of BERT models and demonstrates the complete interface.

### Testing with Real Models

Run the test suite:

```bash
cargo test
```

For integration tests with actual models:

```bash
cargo test --release -- --ignored
```

## Contributing

This is an example implementation demonstrating BERT reranking with Candle. For production use, consider:
- Adding comprehensive error handling
- Implementing proper cross-encoder head architecture
- Adding support for different similarity metrics
- Optimizing for batch processing and GPU acceleration

## Python Cross-Encoder Testing

For debugging and comparing Python vs Rust implementations, several Python testing tools are provided:

### Comprehensive Testing Script

```bash
# Run comprehensive cross-encoder testing
./test_cross_encoder.sh

# Or run Python script directly (requires dependencies)
python3 test_cross_encoder.py
```

This script:
- Tests both `transformers` and `sentence-transformers` libraries
- Shows detailed tokenization analysis (token IDs, attention masks, special tokens)
- Compares scores between relevant and irrelevant queries
- Saves results to JSON for further analysis
- Provides debugging recommendations for Rust implementation

### Quick Debugging Script

```bash
# Run focused debugging tests
python3 debug_scoring.py
```

This minimal script:
- Tests hardcoded query-document pairs
- Shows raw logits and final scores
- Easy to modify for specific test cases
- Highlights score differences and discrimination quality

### Dependencies

Install Python dependencies:

```bash
pip3 install -r requirements.txt
```

Required packages:
- `torch` - PyTorch for model inference
- `transformers` - HuggingFace transformers library
- `sentence-transformers` - Cross-encoder wrapper (optional but recommended)
- `numpy` - Numerical operations

### Test Cases

Both scripts test these scenarios by default:
- **Relevant Query**: "how does authentication work" 
- **Irrelevant Query**: "foobar random nonsense gibberish"
- **Document**: Authentication-related text snippet

Expected behavior:
- Relevant query should score >0.5 (high relevance)
- Irrelevant query should score <0.5 (low relevance)
- Score difference should be significant (>0.1)

### Debugging Rust Implementation

Use these Python scripts to debug Rust cross-encoder issues:

1. **Compare tokenization**: Check if token IDs match between Python and Rust
2. **Compare raw logits**: Verify model outputs before activation functions
3. **Compare final scores**: Check if score calculation methods are identical
4. **Model configuration**: Ensure same model version and weights are loaded

The Python scripts provide detailed output to help identify where discrepancies occur.

## License

This example follows the same license as the main probe project.