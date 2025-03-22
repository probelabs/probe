#!/bin/bash

# Simple script to test provider forcing in the mcp-agent
echo "Testing provider forcing in mcp-agent"
echo "====================================="

# Make sure we have both API keys for testing
export ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY:-dummy-anthropic-key}
export GOOGLE_API_KEY=${GOOGLE_API_KEY:-dummy-google-key}

# Test with --google flag
echo -e "\nTesting with --google flag:"
node src/index.js --google &

# Store the PID
PID=$!

# Wait for a few seconds to see the output
sleep 3

# Kill the process
kill $PID 2>/dev/null

echo -e "\nTest completed. Check the output above to verify that:"
echo "1. 'Forcing provider: google' is displayed"
echo "2. 'Raw FORCE_PROVIDER env value: \"google\"' is displayed"
echo "3. 'Parsed forceProvider config value: \"google\"' is displayed"
echo "4. 'Provider forced to: \"google\"' is displayed"
echo "5. 'Using Google provider as forced' is displayed"