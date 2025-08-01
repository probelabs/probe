# Rust-BERT Cross-Encoder Test

This example tests cross-encoder functionality using rust-bert to compare with our Candle implementation.

## Setup

1. Install libtorch (required by rust-bert):
   - macOS: `brew install pytorch`
   - Linux: Download from https://pytorch.org/get-started/locally/

2. Set environment variables:
   ```bash
   export LIBTORCH=/usr/local/opt/pytorch  # macOS with Homebrew
   # or
   export LIBTORCH=/path/to/libtorch       # Linux/custom installation
   ```

3. Build and run:
   ```bash
   cargo run --release
   ```

## Model Conversion

To use the TinyBERT model with rust-bert, you need to convert it to the .ot format:

```python
# convert_model.py
import torch
from transformers import AutoModelForSequenceClassification

model = AutoModelForSequenceClassification.from_pretrained('cross-encoder/ms-marco-TinyBERT-L-2-v2')
traced = torch.jit.trace(model, (torch.zeros(1, 512, dtype=torch.long),))
traced.save("rust_model.ot")
```

## Notes

- rust-bert expects models in TorchScript format (.ot files)
- The sequence classification pipeline is designed for classification, not regression
- For true cross-encoder scoring, you may need to modify the pipeline
- This example demonstrates the approach but may not give identical results to Python

## Comparison with Candle

Our Candle implementation:
- Loads PyTorch .bin files directly
- Implements cross-encoder architecture manually
- Returns raw logits for scoring

rust-bert approach:
- Uses TorchScript format
- Provides high-level pipelines
- Returns classification labels with confidence scores