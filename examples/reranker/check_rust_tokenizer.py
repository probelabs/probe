#!/usr/bin/env python3
"""
Check what tokenizer files our Rust implementation is using
and compare with Python tokenizer output.
"""

import json
import os

print("="*80)
print("CHECKING RUST TOKENIZER CONFIGURATION")
print("="*80)

# Check what tokenizer files we have
tokenizer_path = "models/ms-marco-TinyBERT-L-2-v2/tokenizer.json"

if os.path.exists(tokenizer_path):
    print(f"✓ Found tokenizer at: {tokenizer_path}")
    
    # Load and inspect the tokenizer
    with open(tokenizer_path, 'r') as f:
        tokenizer_data = json.load(f)
    
    print("\n--- TOKENIZER STRUCTURE ---")
    print(f"Tokenizer type: {tokenizer_data.get('model', {}).get('type', 'Unknown')}")
    
    # Check for special tokens
    if 'added_tokens' in tokenizer_data:
        print("\nSpecial tokens:")
        for token in tokenizer_data['added_tokens'][:10]:  # Show first 10
            print(f"  {token}")
    
    # Check post-processor (important for BERT!)
    if 'post_processor' in tokenizer_data:
        post_proc = tokenizer_data['post_processor']
        print(f"\nPost-processor type: {post_proc.get('type', 'Unknown')}")
        
        # For BERT, should be TemplateProcessing
        if post_proc.get('type') == 'TemplateProcessing':
            if 'single' in post_proc:
                print(f"Single sequence template: {post_proc['single']}")
            if 'pair' in post_proc:
                print(f"Pair sequence template: {post_proc['pair']}")
else:
    print(f"❌ Tokenizer not found at: {tokenizer_path}")

# Now let's create a test to verify Rust tokenization
print("\n" + "="*80)
print("RUST TOKENIZATION TEST CASES")
print("="*80)

# These should match Python exactly
test_cases = [
    {
        "name": "Simple pair",
        "query": "how does authentication work",
        "document": "Authentication is the process of verifying the identity of a user.",
        "method": "pair"  # tokenizer.encode_pair(query, document)
    },
    {
        "name": "Manual concat (wrong)",
        "text": "how does authentication work [SEP] Authentication is the process of verifying the identity of a user.",
        "method": "single"  # tokenizer.encode(text)
    }
]

print("\nExpected Rust code for correct tokenization:")
print("```rust")
print('// CORRECT: Use encode_pair for cross-encoder')
print('let encoding = tokenizer.encode((query, document), true)?;')
print('')
print('// WRONG: Do not manually concatenate')
print('let text = format!("{} [SEP] {}", query, document);')
print('let encoding = tokenizer.encode(text, true)?;')
print("```")

# Key differences to check
print("\n--- KEY THINGS TO VERIFY IN RUST ---")
print("1. Token IDs match exactly")
print("2. Token type IDs are generated correctly:")
print("   - 0 for query tokens (including [CLS])")
print("   - 0 for first [SEP]") 
print("   - 1 for document tokens")
print("   - 1 for final [SEP] (if present)")
print("3. Special tokens are in the right positions")
print("4. Padding is handled correctly")

# Load Python results if available
if os.path.exists("tokenizer_debug_info.json"):
    with open("tokenizer_debug_info.json", 'r') as f:
        python_info = json.load(f)
    
    print("\n--- PYTHON REFERENCE ---")
    print(f"Query: '{python_info['test_case']['query']}'")
    print(f"Document: '{python_info['test_case']['document']}'")
    print(f"Correct score: {python_info['test_case']['correct_score']:.6f}")
    print(f"Manual concat score: {python_info['test_case']['manual_concat_score']:.6f}")
    
    # Show first 20 tokens
    ids = python_info['test_case']['correct_input_ids'][:20]
    types = python_info['test_case']['correct_token_types'][:20] if python_info['test_case']['correct_token_types'] else None
    
    print("\nFirst 20 tokens (Python):")
    print(f"IDs: {ids}")
    if types:
        print(f"Types: {types}")
    
    print("\n✅ Your Rust implementation should produce these EXACT token IDs and types!")