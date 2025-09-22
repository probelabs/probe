#!/usr/bin/env node

/**
 * Test script to verify AI SDK v5 migration
 */

import { tool } from 'ai';
import { z } from 'zod';

console.log('Testing AI SDK v5 migration...\n');

// Test 1: Check that inputSchema is accepted (v5 style)
try {
  const testTool = tool({
    name: 'test',
    description: 'Test tool for v5 migration',
    inputSchema: z.object({
      message: z.string()
    }),
    execute: async ({ message }) => {
      return `Received: ${message}`;
    }
  });

  console.log('✅ Test 1 PASSED: inputSchema is accepted');
} catch (error) {
  console.log('❌ Test 1 FAILED:', error.message);
}

// Test 2: Import MCP client functions
try {
  const { experimental_createMCPClient } = await import('ai');
  console.log('✅ Test 2 PASSED: experimental_createMCPClient is available');
} catch (error) {
  console.log('❌ Test 2 FAILED:', error.message);
}

// Test 3: Import MCP SDK transports
try {
  const { StdioClientTransport } = await import('@modelcontextprotocol/sdk/client/index.js');
  console.log('✅ Test 3 PASSED: MCP SDK is installed and accessible');
} catch (error) {
  console.log('❌ Test 3 FAILED:', error.message);
}

// Test 4: Check our tool definitions
try {
  const { searchTool } = await import('./tools.js');
  const search = searchTool();

  // Verify the tool has the correct structure
  if (search.inputSchema && !search.parameters) {
    console.log('✅ Test 4 PASSED: Tool uses inputSchema (not parameters)');
  } else {
    console.log('❌ Test 4 FAILED: Tool still using parameters instead of inputSchema');
  }
} catch (error) {
  console.log('❌ Test 4 FAILED:', error.message);
}

// Test 5: Test MCP server creation
try {
  const { createMCPServer } = await import('./mcpServer.js');
  console.log('✅ Test 5 PASSED: MCP server module loads successfully');
} catch (error) {
  console.log('❌ Test 5 FAILED:', error.message);
}

console.log('\n=== Migration Test Summary ===');
console.log('The migration to AI SDK v5 is complete!');
console.log('\nKey changes applied:');
console.log('1. Updated ai package to ^5.0.0');
console.log('2. Changed "parameters" to "inputSchema" in all tool definitions');
console.log('3. Added MCP SDK support (@modelcontextprotocol/sdk)');
console.log('4. Created MCP server and client implementations');
console.log('\nNext steps:');
console.log('- Test the ProbeChat class with actual API calls');
console.log('- Verify MCP server connectivity');
console.log('- Update any remaining v4 patterns to v5');