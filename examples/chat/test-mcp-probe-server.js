#!/usr/bin/env node

/**
 * Test direct connection to Probe MCP server
 * This tests the MCP server without requiring AI API keys
 */

import { MCPClientManager } from '@probelabs/probe/agent/mcp';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

async function testProbeMCP() {
  console.log('=== Testing Probe MCP Server Connection ===\n');

  // Create configuration for Probe MCP server
  const config = {
    mcpServers: {
      'probe': {
        command: 'npx',
        args: ['-y', '@probelabs/probe@latest', 'mcp'],
        transport: 'stdio',
        enabled: true
      }
    }
  };

  const manager = new MCPClientManager({ debug: true });

  try {
    // Step 1: Connect to server
    console.log('📡 Connecting to Probe MCP server...\n');
    const result = await manager.initialize(config);

    console.log(`\n✅ Connected successfully!`);
    console.log(`📊 Connection summary:`);
    console.log(`   - Servers connected: ${result.connected}/${result.total}`);
    console.log(`   - Total tools available: ${result.tools.length}`);
    console.log(`   - Tool names: ${result.tools.join(', ')}\n`);

    // Step 2: Get tool details
    const tools = manager.getTools();
    console.log('🛠️  Tool Details:');
    console.log('─'.repeat(60));

    for (const [name, tool] of Object.entries(tools)) {
      console.log(`\n📦 ${name}`);
      console.log(`   Server: ${tool.serverName}`);
      console.log(`   Description: ${tool.description}`);

      if (tool.inputSchema) {
        console.log('   Parameters:');
        const props = tool.inputSchema.properties || {};
        for (const [param, schema] of Object.entries(props)) {
          const required = tool.inputSchema.required?.includes(param) ? ' (required)' : '';
          console.log(`     - ${param}: ${schema.type}${required} - ${schema.description || 'No description'}`);
        }
      }
    }

    console.log('\n' + '─'.repeat(60));

    // Step 3: Test actual tool calls
    console.log('\n🧪 Testing Tool Execution:\n');

    // Test search_code
    console.log('1. Testing probe_search_code...');
    try {
      const searchResult = await manager.callTool('probe_search_code', {
        query: 'MCP',
        path: process.cwd(),
        max_results: 2
      });

      if (searchResult.content && searchResult.content[0]) {
        const content = searchResult.content[0].text;
        const lines = content.split('\n').slice(0, 10);
        console.log('   ✅ Search successful! First 10 lines of results:');
        lines.forEach(line => console.log(`      ${line}`));
      }
    } catch (error) {
      console.log(`   ❌ Search failed: ${error.message}`);
    }

    // Test query_code
    console.log('\n2. Testing probe_query_code...');
    try {
      const queryResult = await manager.callTool('probe_query_code', {
        pattern: 'class $NAME',
        path: process.cwd(),
        language: 'javascript',
        max_results: 2
      });

      if (queryResult.content && queryResult.content[0]) {
        const content = queryResult.content[0].text;
        const lines = content.split('\n').slice(0, 10);
        console.log('   ✅ Query successful! First 10 lines of results:');
        lines.forEach(line => console.log(`      ${line}`));
      }
    } catch (error) {
      console.log(`   ❌ Query failed: ${error.message}`);
    }

    // Test extract_code
    console.log('\n3. Testing probe_extract_code...');
    try {
      const extractResult = await manager.callTool('probe_extract_code', {
        files: [`${join(dirname(fileURLToPath(import.meta.url)), 'probeChat.js')}:1-20`]
      });

      if (extractResult.content && extractResult.content[0]) {
        const content = extractResult.content[0].text;
        const lines = content.split('\n').slice(0, 10);
        console.log('   ✅ Extract successful! First 10 lines:');
        lines.forEach(line => console.log(`      ${line}`));
      }
    } catch (error) {
      console.log(`   ❌ Extract failed: ${error.message}`);
    }

    // Step 4: Test for Vercel AI SDK compatibility
    console.log('\n\n🔄 Testing Vercel AI SDK Compatibility:\n');
    const vercelTools = manager.getVercelTools();
    console.log(`✅ ${Object.keys(vercelTools).length} tools converted for Vercel AI SDK`);

    // Test executing a tool through Vercel wrapper
    const searchTool = vercelTools['probe_search_code'];
    if (searchTool) {
      console.log('\nTesting Vercel-wrapped tool execution...');
      try {
        const result = await searchTool.execute({
          query: 'export',
          path: process.cwd(),
          max_results: 1
        });

        console.log('   ✅ Vercel wrapper works! Result preview:');
        console.log(`      ${result.substring(0, 200)}...`);
      } catch (error) {
        console.log(`   ❌ Vercel wrapper failed: ${error.message}`);
      }
    }

    // Cleanup
    console.log('\n\n🔌 Disconnecting...');
    await manager.disconnect();
    console.log('✅ Disconnected successfully');

    console.log('\n' + '='.repeat(60));
    console.log('✨ All tests completed successfully!');
    console.log('='.repeat(60));

  } catch (error) {
    console.error('\n❌ Test failed:', error);
    await manager.disconnect().catch(() => {});
    process.exit(1);
  }
}

// Run the test
testProbeMCP().catch(console.error);