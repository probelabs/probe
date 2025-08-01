#!/usr/bin/env python3
"""
Cross-encoder model testing script for debugging Rust vs Python score differences.

This script tests the cross-encoder/ms-marco-TinyBERT-L-2-v2 model with hardcoded
query-document pairs to compare exact scores with the Rust implementation.

Usage:
    python test_cross_encoder.py

Requirements:
    pip install transformers sentence-transformers torch numpy

Author: Generated for debugging cross-encoder scoring differences
"""

import os
import sys
import numpy as np
from typing import List, Tuple, Dict, Any
import json

# Try to import required libraries with helpful error messages
try:
    import torch
    print(f"✓ PyTorch version: {torch.__version__}")
except ImportError:
    print("❌ PyTorch not found. Install with: pip install torch")
    sys.exit(1)

try:
    from transformers import AutoTokenizer, AutoModelForSequenceClassification
    print(f"✓ Transformers library imported successfully")
except ImportError:
    print("❌ Transformers not found. Install with: pip install transformers")
    sys.exit(1)

try:
    from sentence_transformers import CrossEncoder
    print(f"✓ Sentence-transformers library imported successfully")
    HAS_SENTENCE_TRANSFORMERS = True
except ImportError:
    print("⚠️  Sentence-transformers not found. Install with: pip install sentence-transformers")
    print("    (Will still test with transformers directly)")
    HAS_SENTENCE_TRANSFORMERS = False

# Model configuration
MODEL_NAME = "cross-encoder/ms-marco-TinyBERT-L-2-v2"
MAX_LENGTH = 512

# Test data - same as used in Rust implementation debugging
TEST_QUERIES = [
    "how does authentication work",
    "foobar random nonsense gibberish"
]

# Sample authentication-related document
SAMPLE_DOCUMENT = """
Authentication is the process of verifying the identity of a user, device, or system. 
In web applications, authentication typically involves checking credentials like usernames 
and passwords against a database. Common authentication methods include:

1. Password-based authentication
2. Multi-factor authentication (MFA)
3. OAuth and OpenID Connect
4. JSON Web Tokens (JWT)
5. Certificate-based authentication

The authentication process usually follows these steps:
- User provides credentials
- System validates credentials against stored data
- If valid, system grants access and creates a session
- Session token is used for subsequent requests

Modern authentication systems often implement additional security measures
like password hashing, salt, and rate limiting to prevent attacks.
"""

def print_separator(title: str):
    """Print a formatted separator with title."""
    print("\n" + "="*80)
    print(f" {title}")
    print("="*80)

def print_subsection(title: str):
    """Print a formatted subsection header."""
    print(f"\n--- {title} ---")

def analyze_tokenization(tokenizer, query: str, document: str, max_length: int = MAX_LENGTH):
    """Analyze tokenization process in detail."""
    print_subsection("Tokenization Analysis")
    
    # Create the input text (query + [SEP] + document)
    input_text = f"{query} [SEP] {document}"
    print(f"Input text length: {len(input_text)} characters")
    print(f"Input text preview: {input_text[:200]}...")
    
    # Tokenize
    encoded = tokenizer(
        query, 
        document,
        truncation=True,
        padding=True,
        max_length=max_length,
        return_tensors="pt",
        return_attention_mask=True,
        return_token_type_ids=True
    )
    
    # Print tokenization details
    input_ids = encoded['input_ids'][0]
    attention_mask = encoded['attention_mask'][0]
    token_type_ids = encoded['token_type_ids'][0] if 'token_type_ids' in encoded else None
    
    print(f"Number of tokens: {len(input_ids)}")
    print(f"Max length limit: {max_length}")
    print(f"Attention mask sum: {attention_mask.sum().item()}")
    
    # Decode tokens to see what they look like
    tokens = tokenizer.convert_ids_to_tokens(input_ids)
    
    print(f"\nFirst 20 tokens:")
    for i, (token_id, token, attention, token_type) in enumerate(zip(
        input_ids[:20], 
        tokens[:20], 
        attention_mask[:20],
        token_type_ids[:20] if token_type_ids is not None else [None]*20
    )):
        type_str = f" (type: {token_type.item()})" if token_type is not None else ""
        print(f"  {i:2d}: {token_id.item():5d} -> '{token}' (att: {attention.item()}){type_str}")
    
    if len(tokens) > 20:
        print(f"  ... ({len(tokens) - 20} more tokens)")
    
    # Find [SEP] tokens
    sep_positions = [i for i, token in enumerate(tokens) if token == '[SEP]']
    print(f"\n[SEP] token positions: {sep_positions}")
    
    return encoded

def test_with_transformers_direct(query: str, document: str) -> Dict[str, Any]:
    """Test using transformers library directly."""
    print_subsection("Testing with Transformers (Direct)")
    
    try:
        # Load model and tokenizer
        print("Loading model and tokenizer...")
        tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
        model = AutoModelForSequenceClassification.from_pretrained(MODEL_NAME)
        
        print(f"Model config: {model.config}")
        print(f"Number of labels: {model.config.num_labels}")
        print(f"Tokenizer vocab size: {tokenizer.vocab_size}")
        
        # Analyze tokenization
        encoded = analyze_tokenization(tokenizer, query, document)
        
        # Run inference
        print_subsection("Model Inference")
        model.eval()
        with torch.no_grad():
            outputs = model(**encoded)
            logits = outputs.logits
            
        print(f"Raw logits shape: {logits.shape}")
        print(f"Raw logits: {logits}")
        
        # Apply softmax to get probabilities
        probabilities = torch.softmax(logits, dim=-1)
        print(f"Probabilities: {probabilities}")
        
        # Get the relevance score (assuming binary classification with relevant=1, irrelevant=0)
        if logits.shape[-1] == 1:
            # Single output (regression-style)
            relevance_score = torch.sigmoid(logits[0, 0]).item()
            print(f"Relevance score (sigmoid): {relevance_score}")
        else:
            # Multiple outputs (classification-style)
            relevance_score = probabilities[0, 1].item() if probabilities.shape[-1] > 1 else probabilities[0, 0].item()
            print(f"Relevance score (softmax): {relevance_score}")
        
        return {
            'method': 'transformers_direct',
            'raw_logits': logits.cpu().numpy().tolist(),
            'probabilities': probabilities.cpu().numpy().tolist(),
            'relevance_score': relevance_score,
            'model_config': str(model.config),
            'tokenizer_info': {
                'vocab_size': tokenizer.vocab_size,
                'model_max_length': tokenizer.model_max_length,
                'pad_token': tokenizer.pad_token,
                'sep_token': tokenizer.sep_token,
                'cls_token': tokenizer.cls_token
            }
        }
        
    except Exception as e:
        print(f"❌ Error with transformers direct: {e}")
        return {'method': 'transformers_direct', 'error': str(e)}

def test_with_sentence_transformers(query: str, document: str) -> Dict[str, Any]:
    """Test using sentence-transformers library."""
    print_subsection("Testing with Sentence-Transformers")
    
    if not HAS_SENTENCE_TRANSFORMERS:
        return {'method': 'sentence_transformers', 'error': 'sentence-transformers not available'}
    
    try:
        # Load cross-encoder
        print("Loading CrossEncoder...")
        cross_encoder = CrossEncoder(MODEL_NAME)
        
        # Score the query-document pair
        pairs = [(query, document)]
        scores = cross_encoder.predict(pairs)
        score = scores[0] if isinstance(scores, (list, np.ndarray)) else scores
        
        print(f"Cross-encoder score: {score}")
        print(f"Score type: {type(score)}")
        
        return {
            'method': 'sentence_transformers',
            'score': float(score),
            'model_name': MODEL_NAME
        }
        
    except Exception as e:
        print(f"❌ Error with sentence-transformers: {e}")
        return {'method': 'sentence_transformers', 'error': str(e)}

def compare_queries(queries: List[str], document: str):
    """Compare multiple queries against the same document."""
    print_separator("QUERY COMPARISON ANALYSIS")
    
    results = {}
    
    for i, query in enumerate(queries, 1):
        print_separator(f"QUERY {i}: '{query}'")
        
        # Test with both methods
        transformers_result = test_with_transformers_direct(query, document)
        sentence_transformers_result = test_with_sentence_transformers(query, document)
        
        results[query] = {
            'transformers': transformers_result,
            'sentence_transformers': sentence_transformers_result
        }
    
    # Summary comparison
    print_separator("SUMMARY COMPARISON")
    
    print("Query Relevance Scores:")
    print(f"{'Query':<40} {'Transformers':<15} {'Sentence-T':<15} {'Difference':<15}")
    print("-" * 85)
    
    for query in queries:
        trans_score = results[query]['transformers'].get('relevance_score', 'Error')
        sent_score = results[query]['sentence_transformers'].get('score', 'Error')
        
        if isinstance(trans_score, (int, float)) and isinstance(sent_score, (int, float)):
            diff = abs(trans_score - sent_score)
            print(f"{query:<40} {trans_score:<15.6f} {sent_score:<15.6f} {diff:<15.6f}")
        else:
            print(f"{query:<40} {str(trans_score):<15} {str(sent_score):<15} {'N/A':<15}")
    
    # Expected behavior analysis
    print_separator("EXPECTED BEHAVIOR ANALYSIS")
    
    print("Expected:")
    print(f"- Query 1 ('{queries[0]}') should have HIGH relevance score (> 0.5)")
    print(f"- Query 2 ('{queries[1]}') should have LOW relevance score (< 0.5)")
    print()
    
    for query in queries:
        trans_score = results[query]['transformers'].get('relevance_score')
        sent_score = results[query]['sentence_transformers'].get('score')
        
        print(f"Query: '{query}'")
        if isinstance(trans_score, (int, float)):
            relevance = "HIGH" if trans_score > 0.5 else "LOW"
            print(f"  Transformers: {trans_score:.6f} ({relevance})")
        
        if isinstance(sent_score, (int, float)):
            relevance = "HIGH" if sent_score > 0.5 else "LOW"
            print(f"  Sentence-T:   {sent_score:.6f} ({relevance})")
        print()
    
    return results

def save_results(results: Dict[str, Any], filename: str = "cross_encoder_test_results.json"):
    """Save results to JSON file for further analysis."""
    try:
        with open(filename, 'w') as f:
            json.dump(results, f, indent=2, default=str)
        print(f"✓ Results saved to {filename}")
    except Exception as e:
        print(f"❌ Failed to save results: {e}")

def main():
    """Main function to run all tests."""
    print_separator("CROSS-ENCODER MODEL TESTING")
    print(f"Model: {MODEL_NAME}")
    print(f"Max length: {MAX_LENGTH}")
    print(f"PyTorch device: {'cuda' if torch.cuda.is_available() else 'cpu'}")
    
    print("\nTest Document Preview:")
    print(SAMPLE_DOCUMENT[:300] + "..." if len(SAMPLE_DOCUMENT) > 300 else SAMPLE_DOCUMENT)
    
    print(f"\nTest Queries:")
    for i, query in enumerate(TEST_QUERIES, 1):
        print(f"  {i}. '{query}'")
    
    # Run comparison tests
    results = compare_queries(TEST_QUERIES, SAMPLE_DOCUMENT)
    
    # Save results
    save_results(results)
    
    print_separator("DEBUGGING RECOMMENDATIONS")
    print("To debug Rust vs Python differences:")
    print("1. Compare tokenization - check token IDs and attention masks")
    print("2. Compare model outputs - check raw logits before activation")
    print("3. Check model weights - ensure same model version is loaded")
    print("4. Verify input preprocessing - truncation, padding, special tokens")
    print("5. Check activation functions - sigmoid vs softmax vs raw logits")
    print()
    print("Key files to check in Rust implementation:")
    print("- Tokenization logic and special token handling")
    print("- Model loading and weight initialization")
    print("- Input preprocessing and tensor creation")
    print("- Output post-processing and score calculation")

if __name__ == "__main__":
    main()