#!/bin/bash

# Script to download MS-MARCO cross-encoder models for local use

set -e

echo "=== MS-MARCO Model Downloader ==="
echo

# Base directory for models
MODEL_DIR="models"
mkdir -p "$MODEL_DIR"

# Function to download a model
download_model() {
    local model_name=$1
    local model_dir=$2
    
    echo "Downloading $model_name..."
    mkdir -p "$MODEL_DIR/$model_dir"
    
    # Download essential files
    FILES=(
        "config.json"
        "tokenizer.json"
        "tokenizer_config.json"
        "vocab.txt"
        "pytorch_model.bin"
        "special_tokens_map.json"
    )
    
    for file in "${FILES[@]}"; do
        if [ -f "$MODEL_DIR/$model_dir/$file" ]; then
            echo "  ✓ $file already exists"
        else
            echo "  ⬇ Downloading $file..."
            curl -L -o "$MODEL_DIR/$model_dir/$file" \
                "https://huggingface.co/$model_name/resolve/main/$file" 2>/dev/null || {
                echo "  ⚠ $file not found (might be optional)"
            }
        fi
    done
    
    echo "✓ $model_name download complete"
    echo
}

# Download models
echo "Downloading cross-encoder models..."
echo

# TinyBERT (4M params) - already have this
if [ -d "$MODEL_DIR/ms-marco-TinyBERT-L-2-v2" ]; then
    echo "✓ TinyBERT model already exists"
else
    download_model "cross-encoder/ms-marco-TinyBERT-L-2-v2" "ms-marco-TinyBERT-L-2-v2"
fi

# MiniLM-L6 (22M params)
download_model "cross-encoder/ms-marco-MiniLM-L-6-v2" "ms-marco-MiniLM-L-6-v2"

# MiniLM-L12 (33M params)
download_model "cross-encoder/ms-marco-MiniLM-L-12-v2" "ms-marco-MiniLM-L-12-v2"

echo "=== Download Complete ==="
echo
echo "Models available in $MODEL_DIR/:"
ls -la "$MODEL_DIR/"
echo
echo "You can now use these rerankers:"
echo "  --reranker ms-marco-tinybert    (4M params, fastest)"
echo "  --reranker ms-marco-minilm-l6   (22M params, balanced)"
echo "  --reranker ms-marco-minilm-l12  (33M params, most accurate)"