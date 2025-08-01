# Cross-Encoder Tokenization Guide for Rust Implementation

## Critical Points for Correct Implementation

### 1. **Use `encode_pair()` NOT Manual Concatenation**

❌ **WRONG** (Manual concatenation):
```rust
let text = format!("{} [SEP] {}", query, document);
let encoding = tokenizer.encode(text, true)?;
```

✅ **CORRECT** (Tokenizer pair encoding):
```rust
let encoding = tokenizer.encode((query, document), true)?;
```

### 2. **Why This Matters**

When you use `encode_pair()`, the tokenizer:
- Automatically adds [CLS] at the start
- Adds [SEP] after the query
- Adds [SEP] at the end (for BERT)
- **Correctly sets token_type_ids**: 0 for query, 1 for document
- Handles special tokens properly

Manual concatenation will:
- Add extra [SEP] tokens (you get [SEP] [SEP] in the middle)
- Set ALL token_type_ids to 0 (incorrect!)
- Produce different tokenization due to whitespace handling

### 3. **Expected Token Structure**

For input: 
- Query: "how does authentication work"
- Document: "Authentication is the process..."

The correct tokenization should be:
```
[CLS] how does authentication work [SEP] authentication is the process ... [SEP]
  0    0   0        0            0    0          1          1   1    1  ...  1
```

Token type IDs:
- 0 = Query segment (including [CLS] and first [SEP])
- 1 = Document segment (including final [SEP])

### 4. **Special Token IDs (for BERT)**

```
[CLS] = 101
[SEP] = 102
[PAD] = 0
```

### 5. **Verification in Rust**

```rust
// After tokenization, check:
let token_ids = encoding.get_ids();
let type_ids = encoding.get_type_ids();

// First token should be [CLS] (101)
assert_eq!(token_ids[0], 101);

// Look for [SEP] tokens (102)
let sep_positions: Vec<_> = token_ids.iter()
    .enumerate()
    .filter(|(_, &id)| id == 102)
    .map(|(i, _)| i)
    .collect();

// Should have 2 [SEP] tokens for pair encoding
assert_eq!(sep_positions.len(), 2);

// Check token type IDs switch from 0 to 1 after first [SEP]
if let Some(first_sep) = sep_positions.first() {
    // Tokens before first [SEP] should have type 0
    // Tokens after should have type 1
}
```

### 6. **Common Issues**

1. **Using wrong tokenizer**: Make sure you load tokenizer.json from the same model directory
2. **Not using pair encoding**: Always use `encode_pair()` for cross-encoders
3. **Missing token type IDs**: These are crucial for BERT to understand query vs document

### 7. **Score Differences**

If you see different scores between Python and Rust:
1. First check tokenization matches exactly (same token IDs)
2. Check token type IDs are correct (0 for query, 1 for document)
3. Verify attention masks are the same
4. Ensure model weights loaded correctly

### 8. **Debug Output**

Add this to your Rust code to debug:
```rust
println!("Token IDs: {:?}", encoding.get_ids());
println!("Type IDs: {:?}", encoding.get_type_ids()); 
println!("Attention mask: {:?}", encoding.get_attention_mask());
```

Compare with Python:
```python
print(f"Token IDs: {encoding['input_ids'][0].tolist()}")
print(f"Type IDs: {encoding['token_type_ids'][0].tolist()}")
print(f"Attention: {encoding['attention_mask'][0].tolist()}")
```

## Summary

The key issue is likely that our Rust implementation was using manual concatenation instead of proper pair encoding. This would result in:
- Wrong token type IDs (all 0s instead of 0s and 1s)
- Extra [SEP] tokens
- Different tokenization

Fixing this should improve the model's ability to distinguish between query and document, leading to better discrimination between relevant and irrelevant queries.