#!/usr/bin/env python3
"""
Test cross-encoder scores to compare with Rust implementation.
This is a minimal script to get raw scores for debugging.
"""

import sys

try:
    from transformers import AutoTokenizer, AutoModelForSequenceClassification
    import torch
except ImportError as e:
    print(f"Error: {e}")
    print("Please install: pip3 install torch transformers")
    sys.exit(1)

# Test inputs - same as in Rust
queries = [
    "how does authentication work",
    "foobar random nonsense gibberish"
]

document = """Authentication is the process of verifying the identity of a user, device, or system. 
In web applications, authentication typically involves checking credentials like usernames 
and passwords against a database. Common authentication methods include:

1. Password-based authentication
2. Multi-factor authentication (MFA)
3. OAuth and social login
4. Biometric authentication
5. Token-based authentication (JWT)

The authentication workflow typically starts when a user provides credentials."""

# Model name - must match Rust
model_name = 'cross-encoder/ms-marco-TinyBERT-L-2-v2'

print(f"Loading model: {model_name}")
try:
    model = AutoModelForSequenceClassification.from_pretrained(model_name)
    tokenizer = AutoTokenizer.from_pretrained(model_name)
    print("Model loaded successfully!")
except Exception as e:
    print(f"Error loading model: {e}")
    sys.exit(1)

# Set to eval mode
model.eval()

print("\n" + "="*80)
print("CROSS-ENCODER SCORING TEST")
print("="*80)

for query in queries:
    print(f"\nQuery: '{query}'")
    print("-" * 40)
    
    # Tokenize as a pair - this is critical for cross-encoders
    inputs = tokenizer(
        query, 
        document, 
        padding=True, 
        truncation=True,
        max_length=512,
        return_tensors="pt"
    )
    
    # Show tokenization info
    print(f"Input shape: {inputs['input_ids'].shape}")
    print(f"First 20 token IDs: {inputs['input_ids'][0][:20].tolist()}")
    
    # Check if token_type_ids exist
    if 'token_type_ids' in inputs:
        print(f"Token type IDs exist: Yes")
        print(f"First 20 type IDs: {inputs['token_type_ids'][0][:20].tolist()}")
    else:
        print(f"Token type IDs exist: No")
    
    # Get model output
    with torch.no_grad():
        outputs = model(**inputs)
        logits = outputs.logits
        
    # Get raw score
    raw_score = logits[0][0].item()
    
    # Apply sigmoid for probability
    sigmoid_score = torch.sigmoid(logits[0][0]).item()
    
    print(f"\nRaw logit score: {raw_score:.6f}")
    print(f"Sigmoid score: {sigmoid_score:.6f}")
    
print("\n" + "="*80)
print("SCORE COMPARISON")
print("="*80)

# Run again to collect scores
scores = {}
for query in queries:
    inputs = tokenizer(query, document, padding=True, truncation=True, return_tensors="pt")
    with torch.no_grad():
        outputs = model(**inputs)
        raw_score = outputs.logits[0][0].item()
        sigmoid_score = torch.sigmoid(outputs.logits[0][0]).item()
    scores[query] = {'raw': raw_score, 'sigmoid': sigmoid_score}

print(f"\nQuery 1 (relevant): {queries[0]}")
print(f"  Raw: {scores[queries[0]]['raw']:.6f}, Sigmoid: {scores[queries[0]]['sigmoid']:.6f}")

print(f"\nQuery 2 (nonsense): {queries[1]}")  
print(f"  Raw: {scores[queries[1]]['raw']:.6f}, Sigmoid: {scores[queries[1]]['sigmoid']:.6f}")

print(f"\nDifference (raw): {abs(scores[queries[0]]['raw'] - scores[queries[1]]['raw']):.6f}")
print(f"Difference (sigmoid): {abs(scores[queries[0]]['sigmoid'] - scores[queries[1]]['sigmoid']):.6f}")

# Analysis
print("\n" + "="*80)
print("ANALYSIS")
print("="*80)

raw_diff = scores[queries[0]]['raw'] - scores[queries[1]]['raw']
if raw_diff > 0.1:
    print("✓ Good discrimination: Relevant query scores higher than nonsense")
elif abs(raw_diff) < 0.1:
    print("⚠ Poor discrimination: Scores are too similar (< 0.1 difference)")
else:
    print("❌ Wrong order: Nonsense query scores higher than relevant query")

print(f"\nExpected behavior:")
print("- Relevant query should have significantly higher score")
print("- Difference should be > 0.5 for good discrimination")
print(f"- Actual difference: {raw_diff:.6f}")