#!/usr/bin/env python3
"""
Convert MS-MARCO TinyBERT model to TorchScript format for rust-bert
"""

import torch
from transformers import AutoModelForSequenceClassification, AutoTokenizer
import os
import sys

def convert_to_torchscript(model_name="cross-encoder/ms-marco-TinyBERT-L-2-v2", output_dir="models/ms-marco-TinyBERT-L-2-v2"):
    print(f"Converting {model_name} to TorchScript format...")
    
    # Create output directory
    os.makedirs(output_dir, exist_ok=True)
    
    # Load model and tokenizer
    print("Loading model...")
    model = AutoModelForSequenceClassification.from_pretrained(model_name)
    tokenizer = AutoTokenizer.from_pretrained(model_name)
    
    # Set to eval mode
    model.eval()
    
    # Create dummy inputs for tracing
    dummy_text = "What is machine learning?"
    dummy_inputs = tokenizer(
        dummy_text, 
        return_tensors="pt",
        padding=True,
        truncation=True,
        max_length=512
    )
    
    # Get the input tensors
    input_ids = dummy_inputs["input_ids"]
    attention_mask = dummy_inputs["attention_mask"]
    token_type_ids = dummy_inputs.get("token_type_ids", torch.zeros_like(input_ids))
    
    print(f"Input shapes:")
    print(f"  input_ids: {input_ids.shape}")
    print(f"  attention_mask: {attention_mask.shape}")
    print(f"  token_type_ids: {token_type_ids.shape}")
    
    # Trace the model
    print("\nTracing model...")
    try:
        # Method 1: Trace with all inputs
        traced_model = torch.jit.trace(
            model, 
            (input_ids, attention_mask, token_type_ids),
            strict=False
        )
        print("✓ Model traced successfully with all inputs")
    except Exception as e:
        print(f"Failed to trace with all inputs: {e}")
        # Method 2: Try with just input_ids
        try:
            traced_model = torch.jit.trace(model, input_ids)
            print("✓ Model traced with input_ids only")
        except Exception as e2:
            print(f"Failed to trace with input_ids only: {e2}")
            return False
    
    # Save the traced model
    output_path = os.path.join(output_dir, "rust_model.ot")
    traced_model.save(output_path)
    print(f"\n✓ Saved TorchScript model to: {output_path}")
    
    # Also save the tokenizer vocab
    tokenizer.save_pretrained(output_dir)
    print(f"✓ Saved tokenizer files to: {output_dir}")
    
    # Test the traced model
    print("\nTesting traced model...")
    with torch.no_grad():
        original_output = model(input_ids, attention_mask, token_type_ids)
        traced_output = traced_model(input_ids, attention_mask, token_type_ids)
        
        orig_logits = original_output.logits[0][0].item()
        traced_logits = traced_output.logits[0][0].item()
        
        print(f"Original model logits: {orig_logits:.6f}")
        print(f"Traced model logits: {traced_logits:.6f}")
        print(f"Difference: {abs(orig_logits - traced_logits):.6f}")
        
        if abs(orig_logits - traced_logits) < 1e-5:
            print("✓ Models produce identical results!")
        else:
            print("⚠ Warning: Models produce different results")
    
    return True

def main():
    # Convert TinyBERT
    if convert_to_torchscript():
        print("\n" + "="*60)
        print("Conversion successful!")
        print("="*60)
        print("\nTo use with rust-bert:")
        print("1. Copy the rust_model.ot file to your rust project")
        print("2. Use LocalResource to load the model")
        print("3. The vocab files are also saved for tokenization")
    else:
        print("\nConversion failed!")
        sys.exit(1)

if __name__ == "__main__":
    main()