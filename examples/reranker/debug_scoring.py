#!/usr/bin/env python3
"""
Focused debugging script for cross-encoder scoring issues.

This script provides a minimal, easily modifiable test harness for debugging
specific query-document pairs and comparing with Rust implementation results.

Usage:
    python debug_scoring.py
    
Or modify the test cases in the script and run again.
"""

import sys
import torch
import numpy as np
from transformers import AutoTokenizer, AutoModelForSequenceClassification

# Configuration - MODIFY THESE FOR YOUR TESTS
MODEL_NAME = "cross-encoder/ms-marco-TinyBERT-L-2-v2"
MAX_LENGTH = 512

# Test cases - MODIFY THESE FOR YOUR SPECIFIC DEBUGGING
TEST_CASES = [
    {
        "name": "Relevant Query",
        "query": "how does authentication work",
        "document": """Authentication is the process of verifying the identity of a user, device, or system. 
In web applications, authentication typically involves checking credentials like usernames 
and passwords against a database. The authentication process usually follows these steps:
- User provides credentials
- System validates credentials against stored data
- If valid, system grants access and creates a session"""
    },
    {
        "name": "Irrelevant Query", 
        "query": "foobar random nonsense gibberish",
        "document": """Authentication is the process of verifying the identity of a user, device, or system. 
In web applications, authentication typically involves checking credentials like usernames 
and passwords against a database. The authentication process usually follows these steps:
- User provides credentials
- System validates credentials against stored data
- If valid, system grants access and creates a session"""
    }
]

def debug_single_case(tokenizer, model, query: str, document: str, case_name: str):
    """Debug a single query-document pair with detailed output."""
    print(f"\n{'='*60}")
    print(f"DEBUGGING: {case_name}")
    print(f"{'='*60}")
    print(f"Query: '{query}'")
    print(f"Document: '{document[:100]}...'")
    
    # Tokenize
    encoded = tokenizer(
        query, 
        document,
        truncation=True,
        padding=True,
        max_length=MAX_LENGTH,
        return_tensors="pt",
        return_attention_mask=True,
        return_token_type_ids=True
    )
    
    # Print tokenization info
    input_ids = encoded['input_ids'][0]
    attention_mask = encoded['attention_mask'][0]
    token_type_ids = encoded.get('token_type_ids', [None])[0]
    
    print(f"\nTokenization:")
    print(f"  Input IDs shape: {input_ids.shape}")
    print(f"  Number of tokens: {len(input_ids)}")
    print(f"  Attention mask sum: {attention_mask.sum().item()}")
    
    # Show first few and last few tokens
    tokens = tokenizer.convert_ids_to_tokens(input_ids)
    print(f"  First 10 tokens: {tokens[:10]}")
    print(f"  Last 10 tokens: {tokens[-10:]}")
    
    # Find special tokens
    cls_positions = [i for i, token in enumerate(tokens) if token == '[CLS]']
    sep_positions = [i for i, token in enumerate(tokens) if token == '[SEP]']
    print(f"  [CLS] positions: {cls_positions}")
    print(f"  [SEP] positions: {sep_positions}")
    
    # Model inference
    model.eval()
    with torch.no_grad():
        outputs = model(**encoded)
        logits = outputs.logits
    
    print(f"\nModel Output:")
    print(f"  Raw logits: {logits}")
    print(f"  Logits shape: {logits.shape}")
    
    # Calculate different score interpretations
    if logits.shape[-1] == 1:
        # Single output - treat as regression
        sigmoid_score = torch.sigmoid(logits[0, 0]).item()
        raw_score = logits[0, 0].item()
        print(f"  Raw score: {raw_score}")
        print(f"  Sigmoid score: {sigmoid_score}")
        final_score = sigmoid_score
    else:
        # Multiple outputs - treat as classification
        probabilities = torch.softmax(logits, dim=-1)
        print(f"  Softmax probabilities: {probabilities}")
        if logits.shape[-1] == 2:
            # Binary classification
            final_score = probabilities[0, 1].item()
            print(f"  Relevance probability (class 1): {final_score}")
        else:
            final_score = probabilities[0, 0].item()
            print(f"  First class probability: {final_score}")
    
    print(f"\nFINAL SCORE: {final_score:.6f}")
    
    # Return data for comparison
    return {
        'case_name': case_name,
        'query': query,
        'document_preview': document[:100] + '...',
        'num_tokens': len(input_ids),
        'raw_logits': logits.cpu().numpy().tolist(),
        'final_score': final_score
    }

def main():
    """Run focused debugging tests."""
    print("Cross-Encoder Scoring Debug Tool")
    print(f"Model: {MODEL_NAME}")
    print(f"PyTorch device: {'cuda' if torch.cuda.is_available() else 'cpu'}")
    
    # Load model and tokenizer
    print("\nLoading model...")
    try:
        tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
        model = AutoModelForSequenceClassification.from_pretrained(MODEL_NAME)
        print("✓ Model loaded successfully")
    except Exception as e:
        print(f"❌ Failed to load model: {e}")
        sys.exit(1)
    
    # Run test cases
    results = []
    for test_case in TEST_CASES:
        result = debug_single_case(
            tokenizer, 
            model, 
            test_case["query"], 
            test_case["document"], 
            test_case["name"]
        )
        results.append(result)
    
    # Summary
    print(f"\n{'='*60}")
    print("SUMMARY")
    print(f"{'='*60}")
    
    print(f"{'Case':<20} {'Tokens':<8} {'Score':<12} {'Expected':<12}")
    print("-" * 52)
    
    for result in results:
        expected = "HIGH (>0.5)" if "Relevant" in result['case_name'] else "LOW (<0.5)"
        actual = "HIGH" if result['final_score'] > 0.5 else "LOW"
        status = "✓" if (actual == "HIGH") == ("Relevant" in result['case_name']) else "❌"
        
        print(f"{result['case_name']:<20} {result['num_tokens']:<8} {result['final_score']:<12.6f} {expected:<12} {status}")
    
    # Score difference
    if len(results) >= 2:
        score_diff = abs(results[0]['final_score'] - results[1]['final_score'])
        print(f"\nScore difference: {score_diff:.6f}")
        if score_diff < 0.1:
            print("⚠️  WARNING: Score difference is very small - model may not be discriminating well")
        else:
            print("✓ Good score separation between relevant and irrelevant queries")
    
    print("\nFor Rust debugging, compare:")
    print("1. Token IDs and their order")
    print("2. Raw logits values") 
    print("3. Final score calculation method")
    print("4. Model configuration and weights")

if __name__ == "__main__":
    main()