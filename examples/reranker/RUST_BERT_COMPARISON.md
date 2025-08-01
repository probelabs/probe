# Rust-BERT vs Candle for Cross-Encoders

## Summary

After investigating rust-bert for cross-encoder support, here are the key findings:

### rust-bert Limitations for Cross-Encoders

1. **No Native Cross-Encoder Support**: rust-bert doesn't have a dedicated cross-encoder pipeline
2. **Classification Focus**: The sequence classification pipeline expects discrete labels (POSITIVE/NEGATIVE), not continuous relevance scores
3. **Model Format**: Requires TorchScript (.ot) format, not standard PyTorch .bin files
4. **Architecture Mismatch**: Cross-encoders output a single score, but rust-bert's classification expects label probabilities

### Our Candle Implementation Advantages

1. **Direct PyTorch Support**: Loads .bin files directly from HuggingFace
2. **Custom Architecture**: We implement the exact cross-encoder architecture
3. **Raw Scores**: Returns raw logits for scoring, which is what cross-encoders need
4. **Flexibility**: Full control over tokenization and model behavior

## Model Availability

The MS-MARCO models on HuggingFace include:
- PyTorch formats: `pytorch_model.bin`, `model.safetensors`
- ONNX formats: Multiple optimized ONNX versions
- No TorchScript (.ot) versions available

## Conversion Options

### 1. PyTorch to TorchScript
```python
# See convert_to_torchscript.py
traced_model = torch.jit.trace(model, example_inputs)
traced_model.save("rust_model.ot")
```

### 2. Use ONNX Runtime
Instead of rust-bert, consider using ONNX Runtime with Rust bindings:
```toml
[dependencies]
ort = "1.16"  # ONNX Runtime for Rust
```

### 3. Continue with Candle
Our current Candle implementation is actually well-suited for cross-encoders.

## Recommendation

**Stay with Candle** for cross-encoder implementation because:

1. It already works correctly with HuggingFace models
2. No conversion needed
3. Better control over the scoring pipeline
4. The issue isn't with Candle - it's that TinyBERT (4M params) is too small

**To improve results:**
1. Switch to a larger model (MiniLM-L-6-v2 with 85M params)
2. Make the model configurable via CLI
3. Consider adding ONNX support as an alternative backend

## Code Comparison

### rust-bert Approach (Would Require Modifications)
```rust
// rust-bert expects classification, not scoring
let config = SequenceClassificationConfig { ... };
let model = SequenceClassificationModel::new(config)?;
let output = model.predict(&[text]); // Returns Label with probability
```

### Our Candle Approach (Current)
```rust
// Direct cross-encoder implementation
let bert_outputs = self.bert.forward(&input_ids, &attention_mask, token_type_ids.as_ref())?;
let cls_output = bert_outputs.i((.., 0, ..))?;
let logits = self.classifier.forward(&cls_output)?;
let score = logits.i((0, 0))?.to_scalar::<f32>()?; // Raw relevance score
```

## Conclusion

rust-bert isn't suitable for cross-encoder models without significant modifications. Our Candle implementation is the right approach. The scoring issues are due to model size (TinyBERT), not the implementation framework.