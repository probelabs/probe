# Debug Output Analysis

Based on the debug output, here's exactly what's happening in our Rust implementation:

## 1. Input to score_pair()
```
Query: 'test question'
Document: '// Filename: ./mcp-agent/src/agent.js\n// AI agent implementation\nimport...'
```

## 2. Tokenization (`encode_pair`)
- **Token IDs**: `[101, 3231, 3160, 102, 1013, 1013, 5371, 18442, 1024, ...]`
  - 101 = [CLS]
  - 3231, 3160 = "test question" 
  - 102 = [SEP]
  - Rest = document tokens

- **Token Type IDs**: `[0, 0, 0, 0, 1, 1, 1, 1, 1, 1, ...]`
  - First 4 tokens (including [CLS] and [SEP]) = 0 (query segment)
  - Remaining tokens = 1 (document segment)
  - ✅ This is CORRECT!

- **Structure**: `[CLS] test question [SEP] // Filename: ./mcp-agent/src/agent.js ...`

## 3. Model Input Tensors
- **input_ids**: Shape [1, 512] - padded to max length
- **attention_mask**: Shape [1, 512] - 1s for real tokens, 0s for padding
- **token_type_ids**: Shape [1, 512] - 0s for query, 1s for document

## 4. BERT Processing
- **CLS output**: Shape [1, 128] (hidden size = 128 for TinyBERT)
- **CLS values**: `[-0.041968495, -0.4378377, 0.58510137, 1.540222, ...]`
  - These are the contextualized embeddings for the [CLS] token

## 5. Classifier Output
- **Logits**: Shape [1, 1] - single score
- **Raw score**: 0.833216 (for this example)

## Key Observations

1. **Tokenization is correct**: Using `encode_pair()` properly generates:
   - Correct special tokens ([CLS], [SEP])
   - Correct token type IDs (0 for query, 1 for document)

2. **Model inputs are correct**: All tensors have the right shape and values

3. **BERT is processing correctly**: Getting proper hidden states

4. **Scores are reasonable**: Raw logits in expected range

## The Real Issue

The implementation is correct. The problem is that TinyBERT (4M parameters) produces very similar scores for different queries:
- "test question" → 0.833216
- "how does authentication work" → ~0.85-0.88 (from earlier tests)

The model just isn't discriminating well between relevant and irrelevant queries because it's too small.

## To Verify Further

Add this temporary debug to see exact token-by-token breakdown:
```rust
// After encoding
for (i, (token_id, type_id)) in encoding.get_ids().iter()
    .zip(encoding.get_type_ids().iter())
    .enumerate()
    .take(20) {
    let token_text = self.tokenizer.decode(&[*token_id], false).unwrap_or_default();
    println!("  [{}] '{}' (ID: {}, Type: {})", i, token_text, token_id, type_id);
}
```