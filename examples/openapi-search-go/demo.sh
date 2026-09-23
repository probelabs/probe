#!/bin/bash

# Demo script for OpenAPI Search Engine
# Shows various search examples

echo "========================================="
echo "OpenAPI Search Engine Demo"
echo "Based on probe's search architecture"
echo "========================================="
echo ""

echo "1. Searching for 'weather API'..."
echo "-----------------------------------"
go run main.go "weather API"
echo ""
echo ""

echo "2. Searching for 'JWT authentication'..."
echo "-----------------------------------"
go run main.go "JWT authentication"
echo ""
echo ""

echo "3. Searching for 'refund payment'..."
echo "-----------------------------------"
go run main.go "refund payment"
echo ""
echo ""

echo "4. Searching for 'create user'..."
echo "-----------------------------------"
go run main.go "create user"
echo ""
echo ""

echo "5. Searching for 'delete' (limiting to 3 results)..."
echo "-----------------------------------"
go run main.go -max 3 "delete"
echo ""
echo ""

echo "========================================="
echo "Demo complete!"
echo "========================================="
