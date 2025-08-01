#!/bin/bash

# Cross-encoder testing script setup and runner
# This script sets up the Python environment and runs the cross-encoder tests

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== Cross-Encoder Model Testing Setup ==="
echo "Working directory: $SCRIPT_DIR"

# Check if Python 3 is available
if ! command -v python3 &> /dev/null; then
    echo "❌ Python 3 is required but not found"
    exit 1
fi

echo "✓ Python 3 found: $(python3 --version)"

# Check if pip is available
if ! command -v pip3 &> /dev/null; then
    echo "❌ pip3 is required but not found"
    exit 1
fi

echo "✓ pip3 found"

# Install or check requirements
echo ""
echo "Checking Python dependencies..."

# Function to check if a package is installed
check_package() {
    python3 -c "import $1" 2>/dev/null && return 0 || return 1
}

# Check required packages
REQUIRED_PACKAGES=("torch" "transformers" "numpy")
MISSING_PACKAGES=()

for package in "${REQUIRED_PACKAGES[@]}"; do
    if check_package "$package"; then
        echo "✓ $package is installed"
    else
        echo "❌ $package is missing"
        MISSING_PACKAGES+=("$package")
    fi
done

# Check optional package
if check_package "sentence_transformers"; then
    echo "✓ sentence-transformers is installed"
else
    echo "⚠️  sentence-transformers is missing (optional but recommended)"
    MISSING_PACKAGES+=("sentence-transformers")
fi

# Install missing packages if any
if [ ${#MISSING_PACKAGES[@]} -gt 0 ]; then
    echo ""
    echo "Installing missing packages..."
    pip3 install "${MISSING_PACKAGES[@]}"
    echo "✓ Dependencies installed"
else
    echo "✓ All required dependencies are installed"
fi

echo ""
echo "=== Running Cross-Encoder Tests ==="
echo ""

# Run the test script
python3 test_cross_encoder.py

echo ""
echo "=== Test Complete ==="
echo "Check the output above for score comparisons and debugging information"
echo "Results have been saved to cross_encoder_test_results.json"