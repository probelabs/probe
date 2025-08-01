# MS-MARCO Cross-Encoder Models

## Available Models

### 1. TinyBERT-L-2-v2 (`ms-marco-tinybert`)
- **Parameters**: 4.4M
- **Layers**: 2
- **Hidden Size**: 128
- **Performance**: Fast but limited discrimination
- **Use Case**: Quick reranking when speed is critical

### 2. MiniLM-L-6-v2 (`ms-marco-minilm-l6`)
- **Parameters**: 22.7M
- **Layers**: 6  
- **Hidden Size**: 384
- **Performance**: Good balance of speed and accuracy
- **Use Case**: Recommended for most applications

### 3. MiniLM-L-12-v2 (`ms-marco-minilm-l12`)
- **Parameters**: 33.4M
- **Layers**: 12
- **Hidden Size**: 384
- **Performance**: Best accuracy, slower
- **Use Case**: When accuracy is more important than speed

## Performance Comparison

Based on MS MARCO evaluation:

| Model | MRR@10 | Params | Speed (V100) |
|-------|--------|--------|--------------|
| TinyBERT-L-2 | 0.312 | 4.4M | ~9000 docs/sec |
| MiniLM-L-6 | 0.384 | 22.7M | ~2800 docs/sec |
| MiniLM-L-12 | 0.391 | 33.4M | ~960 docs/sec |

## Usage

```bash
# Download models
./download_models.sh

# Use in probe
probe search "query" . --reranker ms-marco-minilm-l6 --question "natural language question"
```

## Model Architecture

All models use the same cross-encoder architecture:
1. Input: `[CLS] query [SEP] document [SEP]`
2. BERT encoder processes the concatenated input
3. [CLS] token representation is passed through a linear classifier
4. Output: Single relevance score (raw logit)

## Recommendations

- **Start with MiniLM-L-6**: It provides much better discrimination than TinyBERT while still being reasonably fast
- **Use TinyBERT only if**: You need maximum speed and can tolerate lower accuracy
- **Use MiniLM-L-12 when**: You need the best possible ranking quality

## Token Limits

All models support up to 512 tokens, which is split between:
- Query: typically 10-50 tokens
- Document: remaining tokens (460-500)

Documents are truncated if they exceed the limit.