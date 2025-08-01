#!/usr/bin/env python3
"""
Debug script to understand exactly how the Python implementation works
so we can ensure our Rust implementation matches it perfectly.
"""

import os
os.environ['TOKENIZERS_PARALLELISM'] = 'false'

from transformers import AutoTokenizer, AutoModelForSequenceClassification
import torch
import json

print("="*80)
print("TOKENIZATION AND MODEL LOADING DEBUG")
print("="*80)

# Load model and tokenizer
model_name = 'cross-encoder/ms-marco-TinyBERT-L-2-v2'
print(f"Loading model: {model_name}")

tokenizer = AutoTokenizer.from_pretrained(model_name)
model = AutoModelForSequenceClassification.from_pretrained(model_name)

# Print tokenizer info
print("\n--- TOKENIZER INFO ---")
print(f"Tokenizer class: {type(tokenizer).__name__}")
print(f"Vocab size: {tokenizer.vocab_size}")
print(f"Model max length: {tokenizer.model_max_length}")
print(f"Padding token: '{tokenizer.pad_token}' (ID: {tokenizer.pad_token_id})")
print(f"SEP token: '{tokenizer.sep_token}' (ID: {tokenizer.sep_token_id})")
print(f"CLS token: '{tokenizer.cls_token}' (ID: {tokenizer.cls_token_id})")

# Test inputs
query = "how does authentication work"
document = "Authentication is the process of verifying the identity of a user."

print(f"\nQuery: '{query}'")
print(f"Document: '{document}'")

# Method 1: Tokenize as pair (CORRECT for cross-encoder)
print("\n--- METHOD 1: Tokenize as pair (query, document) ---")
encoding = tokenizer(
    query,
    document,
    padding=True,
    truncation=True,
    max_length=512,
    return_tensors="pt"
)

print(f"Keys in encoding: {list(encoding.keys())}")
print(f"Input IDs shape: {encoding['input_ids'].shape}")
print(f"Input IDs: {encoding['input_ids'][0].tolist()}")

# Decode to see what was tokenized
decoded = tokenizer.decode(encoding['input_ids'][0])
print(f"\nDecoded text: '{decoded}'")

# Show token type IDs if present
if 'token_type_ids' in encoding:
    print(f"\nToken type IDs: {encoding['token_type_ids'][0].tolist()}")
    # Find where document starts (token type switches from 0 to 1)
    token_types = encoding['token_type_ids'][0].tolist()
    for i, (token_id, token_type) in enumerate(zip(encoding['input_ids'][0].tolist(), token_types)):
        if i < 30:  # Show first 30 tokens
            token_text = tokenizer.decode([token_id])
            print(f"  [{i}] '{token_text}' (ID: {token_id}, Type: {token_type})")

# Method 2: Manual concatenation (WRONG - for comparison)
print("\n--- METHOD 2: Manual concatenation (WRONG approach) ---")
manual_text = f"{query} [SEP] {document}"
encoding2 = tokenizer(
    manual_text,
    padding=True,
    truncation=True,
    max_length=512,
    return_tensors="pt"
)

print(f"Input IDs: {encoding2['input_ids'][0].tolist()}")
if 'token_type_ids' in encoding2:
    print(f"Token type IDs: {encoding2['token_type_ids'][0].tolist()}")

# Compare the two methods
print("\n--- COMPARISON ---")
ids1 = encoding['input_ids'][0].tolist()
ids2 = encoding2['input_ids'][0].tolist()

if ids1 == ids2:
    print("✓ Both methods produce SAME token IDs")
else:
    print("❌ Methods produce DIFFERENT token IDs!")
    print(f"  Length difference: {len(ids1)} vs {len(ids2)}")
    # Find first difference
    for i, (t1, t2) in enumerate(zip(ids1, ids2)):
        if t1 != t2:
            print(f"  First difference at position {i}: {t1} vs {t2}")
            break

# Test model forward pass
print("\n--- MODEL FORWARD PASS ---")
model.eval()

# Show model configuration
print(f"Model config:")
print(f"  Hidden size: {model.config.hidden_size}")
print(f"  Num labels: {model.config.num_labels}")
print(f"  Problem type: {getattr(model.config, 'problem_type', 'Not specified')}")

# Test both encodings
with torch.no_grad():
    # Correct tokenization
    output1 = model(**encoding)
    logits1 = output1.logits[0][0].item()
    
    # Manual concatenation
    output2 = model(**encoding2)
    logits2 = output2.logits[0][0].item()

print(f"\nResults:")
print(f"  Correct tokenization score: {logits1:.6f}")
print(f"  Manual concatenation score: {logits2:.6f}")
print(f"  Difference: {abs(logits1 - logits2):.6f}")

if abs(logits1 - logits2) > 0.01:
    print("  ⚠️  Significant difference! Tokenization method matters!")

# Save tokenizer info for Rust comparison
print("\n--- SAVING DEBUG INFO ---")
debug_info = {
    "model_name": model_name,
    "tokenizer_class": type(tokenizer).__name__,
    "vocab_size": tokenizer.vocab_size,
    "special_tokens": {
        "pad": {"token": tokenizer.pad_token, "id": tokenizer.pad_token_id},
        "sep": {"token": tokenizer.sep_token, "id": tokenizer.sep_token_id},
        "cls": {"token": tokenizer.cls_token, "id": tokenizer.cls_token_id},
    },
    "test_case": {
        "query": query,
        "document": document,
        "correct_input_ids": ids1,
        "correct_token_types": encoding['token_type_ids'][0].tolist() if 'token_type_ids' in encoding else None,
        "correct_score": logits1,
        "manual_concat_score": logits2,
    }
}

with open("tokenizer_debug_info.json", "w") as f:
    json.dump(debug_info, f, indent=2)

print("Debug info saved to tokenizer_debug_info.json")
print("\n✅ Use this info to verify your Rust implementation matches exactly!")