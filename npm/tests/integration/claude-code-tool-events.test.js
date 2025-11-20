#!/usr/bin/env node

/**
 * Test that Claude Code tool events are properly extracted and emitted
 */

import { ProbeAgent } from './src/agent/ProbeAgent.js';

async function testToolEvents() {
  console.log('Testing Tool Event Extraction from Claude Code\n');
  console.log('='*60 + '\n');

  const agent = new ProbeAgent({
    allowedFolders: [process.cwd()],
    debug: true,  // Enable debug to see tool event emissions
    provider: 'claude-code'
  });

  // Track tool events
  const toolEvents = [];

  try {
    await agent.initialize();

    // Listen for tool events
    if (agent.events) {
      agent.events.on('toolCall', (event) => {
        toolEvents.push(event);
        console.log('\n📊 Tool Event Captured:');
        console.log('  - Name:', event.name);
        console.log('  - Status:', event.status);
        console.log('  - Timestamp:', event.timestamp);
        if (event.args) {
          console.log('  - Args:', JSON.stringify(event.args).substring(0, 100));
        }
        if (event.resultPreview) {
          console.log('  - Result:', event.resultPreview.substring(0, 100));
        }
      });
    }

    // Query that should trigger tool use
    console.log('📝 Query: "List the main JavaScript files in this directory"\n');
    console.log('This query should trigger tool use (listFiles or similar)\n');

    const result = await agent.answer(
      'List the main JavaScript files in this directory',
      [],
      {
        onStream: (chunk) => {
          // Silent streaming to focus on events
        }
      }
    );

    console.log('\n' + '='*60 + '\n');
    console.log('📋 Response Summary:');
    console.log('Response length:', result.length, 'characters');
    console.log('First 300 chars:', result.substring(0, 300) + '...\n');

    console.log('='*60 + '\n');
    console.log('🔍 Tool Event Analysis:\n');

    if (toolEvents.length > 0) {
      console.log(`✅ Captured ${toolEvents.length} tool events`);

      // Group by status
      const started = toolEvents.filter(e => e.status === 'started');
      const completed = toolEvents.filter(e => e.status === 'completed');
      const errors = toolEvents.filter(e => e.status === 'error');

      console.log(`  - Started: ${started.length}`);
      console.log(`  - Completed: ${completed.length}`);
      console.log(`  - Errors: ${errors.length}`);

      // List unique tool names
      const toolNames = [...new Set(toolEvents.map(e => e.name))];
      console.log('\n📦 Tools used:');
      toolNames.forEach(name => {
        const count = toolEvents.filter(e => e.name === name).length;
        console.log(`  - ${name} (${count} events)`);
      });

      // Check if MCP tools were used
      const mcpTools = toolEvents.filter(e => e.name && e.name.startsWith('mcp__'));
      if (mcpTools.length > 0) {
        console.log('\n✅ MCP tools detected:', mcpTools.map(e => e.name).join(', '));
      }

    } else {
      console.log('⚠️  No tool events captured');
      console.log('This could mean:');
      console.log('  1. Claude Code didn\'t use any tools for this query');
      console.log('  2. Tool event extraction needs adjustment');
    }

    // Clean up
    if (agent.engine && agent.engine.close) {
      await agent.engine.close();
    }

  } catch (error) {
    console.error('Test failed:', error.message);
  }
}

async function testSimpleQuery() {
  console.log('\n' + '='*60);
  console.log('Testing Simple Query (no tools expected)\n');

  const agent = new ProbeAgent({
    allowedFolders: [process.cwd()],
    debug: false,
    provider: 'claude-code'
  });

  const toolEvents = [];

  try {
    await agent.initialize();

    if (agent.events) {
      agent.events.on('toolCall', (event) => {
        toolEvents.push(event);
      });
    }

    console.log('📝 Query: "What is 2+2?"\n');
    const result = await agent.answer('What is 2+2?');

    if (toolEvents.length === 0) {
      console.log('✅ No tool events for simple query (expected)');
    } else {
      console.log(`⚠️  ${toolEvents.length} tool events for simple query (unexpected)`);
    }

    if (agent.engine && agent.engine.close) {
      await agent.engine.close();
    }

  } catch (error) {
    console.error('Simple test failed:', error.message);
  }
}

async function main() {
  console.log('🧪 Claude Code Tool Event Extraction Test\n');

  // Test with query that should trigger tools
  await testToolEvents();

  // Test with simple query (no tools expected)
  await testSimpleQuery();

  console.log('\n' + '='*60);
  console.log('🏁 Tool Event Test Complete\n');
  console.log('Key Points:');
  console.log('- Tool events are extracted from Claude Code\'s internal operations');
  console.log('- Events are emitted as a batch after response completes');
  console.log('- This maintains event compatibility with regular ProbeAgent');
}

main().catch(console.error);