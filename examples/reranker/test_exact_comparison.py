#!/usr/bin/env python3
"""
Exact comparison test with the Python implementation shown in the documentation.
This uses the exact same inputs as our Rust implementation.
"""

import os
os.environ['TOKENIZERS_PARALLELISM'] = 'false'  # Avoid warning

# Test 1: Using transformers directly (as shown in the docs)
print("="*80)
print("TEST 1: Using Transformers Library (Direct)")
print("="*80)

try:
    from transformers import AutoTokenizer, AutoModelForSequenceClassification
    import torch
    
    model = AutoModelForSequenceClassification.from_pretrained('cross-encoder/ms-marco-TinyBERT-L-2-v2')
    tokenizer = AutoTokenizer.from_pretrained('cross-encoder/ms-marco-TinyBERT-L-2-v2')
    
    # Our test inputs - EXACTLY the same as in Rust
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

    print(f"Document length: {len(document)} chars")
    print(f"Queries: {queries}")
    print()
    
    # Process each query with the document
    for query in queries:
        # Method 1: Single pair tokenization
        features = tokenizer(
            query, 
            document,  
            padding=True, 
            truncation=True, 
            max_length=512,
            return_tensors="pt"
        )
        
        print(f"\nQuery: '{query}'")
        print(f"Input IDs shape: {features['input_ids'].shape}")
        print(f"First 20 tokens: {features['input_ids'][0][:20].tolist()}")
        
        # Check token type IDs
        if 'token_type_ids' in features:
            print(f"Token type IDs (first 20): {features['token_type_ids'][0][:20].tolist()}")
        
        model.eval()
        with torch.no_grad():
            outputs = model(**features)
            logits = outputs.logits
            score = logits[0][0].item()  # Get the raw score
            
        print(f"Raw logit score: {score:.6f}")
        
        # Also try sigmoid
        sigmoid_score = torch.sigmoid(logits[0][0]).item()
        print(f"Sigmoid score: {sigmoid_score:.6f}")
    
    # Now test batch processing (as shown in docs)
    print("\n" + "-"*40)
    print("Batch processing test:")
    
    # Create pairs for batch processing
    batch_queries = [queries[0], queries[1]]
    batch_docs = [document, document]
    
    features = tokenizer(
        batch_queries, 
        batch_docs,
        padding=True, 
        truncation=True,
        max_length=512,
        return_tensors="pt"
    )
    
    model.eval()
    with torch.no_grad():
        scores = model(**features).logits
        print(f"\nBatch scores shape: {scores.shape}")
        print(f"Query 1 score: {scores[0][0].item():.6f}")
        print(f"Query 2 score: {scores[1][0].item():.6f}")
        print(f"Score difference: {abs(scores[0][0].item() - scores[1][0].item()):.6f}")

except Exception as e:
    print(f"Error with transformers: {e}")
    import traceback
    traceback.print_exc()

# Test 2: Using sentence-transformers (as shown in the docs)
print("\n" + "="*80)
print("TEST 2: Using Sentence-Transformers (CrossEncoder)")
print("="*80)

try:
    from sentence_transformers import CrossEncoder
    
    model = CrossEncoder('cross-encoder/ms-marco-TinyBERT-L-2-v2', max_length=512)
    
    # Create pairs
    pairs = [
        ("how does authentication work", document),
        ("foobar random nonsense gibberish", document)
    ]
    
    scores = model.predict(pairs)
    
    print(f"CrossEncoder scores: {scores}")
    print(f"Query 1 score: {scores[0]:.6f}")
    print(f"Query 2 score: {scores[1]:.6f}") 
    print(f"Score difference: {abs(scores[0] - scores[1]):.6f}")
    
    # Analysis
    print("\n" + "-"*40)
    print("ANALYSIS:")
    if scores[0] > scores[1] + 0.1:
        print("✓ Good: Relevant query scores significantly higher")
    elif abs(scores[0] - scores[1]) < 0.1:
        print("⚠ Poor discrimination: Scores too similar (< 0.1 difference)")
    else:
        print("❌ Wrong: Nonsense query scores higher")
        
except Exception as e:
    print(f"Error with sentence-transformers: {e}")
    import traceback
    traceback.print_exc()

print("\n" + "="*80)
print("IMPORTANT NOTES FOR RUST IMPLEMENTATION:")
print("="*80)
print("1. The tokenizer should handle query-document pairs properly")
print("2. Token type IDs should mark query vs document segments")
print("3. Raw logits are the direct model output (no activation)")
print("4. CrossEncoder from sentence-transformers may apply post-processing")
print("5. Check if your Rust tokenizer matches the Python tokenizer output")