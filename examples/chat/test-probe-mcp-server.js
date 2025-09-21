#!/usr/bin/env node

/**
 * Test script to connect to probe MCP server and explore its capabilities
 */

import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

async function testProbeMcpServer() {
  console.log('Testing Probe MCP Server...\n');

  // Create transport to probe MCP server
  const transport = new StdioClientTransport({
    command: 'npx',
    args: ['-y', '@probelabs/probe@latest', 'mcp']
  });

  const client = new Client(
    {
      name: 'test-client',
      version: '1.0.0'
    },
    {
      capabilities: {}
    }
  );

  try {
    // Connect to the server
    console.log('Connecting to Probe MCP server...');
    await client.connect(transport);
    console.log('✅ Connected successfully\n');

    // List available tools
    console.log('Available tools:');
    const tools = await client.listTools();

    for (const tool of tools.tools) {
      console.log(`\n📦 Tool: ${tool.name}`);
      console.log(`   Description: ${tool.description}`);

      if (tool.inputSchema) {
        console.log('   Input Schema:');
        console.log(JSON.stringify(tool.inputSchema, null, 4));
      }
    }

    // Test a simple search
    console.log('\n\nTesting search_code tool...');
    const searchResult = await client.callTool({
      name: 'search_code',
      arguments: {
        query: 'function',
        path: '/home/buger/projects/probe/examples/chat',
        max_results: 3
      }
    });

    console.log('\nSearch result (truncated):');
    if (searchResult.content && searchResult.content[0]) {
      const content = searchResult.content[0].text;
      console.log(content.substring(0, 500) + '...');
    }

    // Test query_code tool
    console.log('\n\nTesting query_code tool...');
    const queryResult = await client.callTool({
      name: 'query_code',
      arguments: {
        pattern: 'function $NAME($$$PARAMS) { $$$BODY }',
        path: '/home/buger/projects/probe/examples/chat',
        language: 'javascript',
        max_results: 2
      }
    });

    console.log('\nQuery result (truncated):');
    if (queryResult.content && queryResult.content[0]) {
      const content = queryResult.content[0].text;
      console.log(content.substring(0, 500) + '...');
    }

    // Test extract_code tool
    console.log('\n\nTesting extract_code tool...');
    const extractResult = await client.callTool({
      name: 'extract_code',
      arguments: {
        files: ['/home/buger/projects/probe/examples/chat/probeChat.js:230-250']
      }
    });

    console.log('\nExtract result (truncated):');
    if (extractResult.content && extractResult.content[0]) {
      const content = extractResult.content[0].text;
      console.log(content.substring(0, 500) + '...');
    }

    // Close the connection
    await client.close();
    console.log('\n✅ All tests completed successfully!');

  } catch (error) {
    console.error('Error during testing:', error);
    process.exit(1);
  }
}

// Run the test
testProbeMcpServer().catch(console.error);