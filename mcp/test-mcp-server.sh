#!/bin/bash

# Test script for the probe MCP server

# Define JSON-RPC request for listing tools
LIST_TOOLS_REQUEST='{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "listTools",
  "params": {}
}'

# Define JSON-RPC request for calling the search_code tool
SEARCH_CODE_REQUEST='{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "callTool",
  "params": {
    "name": "search_code",
    "arguments": {
      "path": "'"$PWD"'",
      "query": "search",
      "maxResults": 5
    }
  }
}'

echo "Starting MCP server test..."
echo "1. Testing listTools request..."
echo "$LIST_TOOLS_REQUEST" | node build/index.js

echo "2. Testing callTool request for search_code..."
echo "$SEARCH_CODE_REQUEST" | node build/index.js

echo "Test completed."
