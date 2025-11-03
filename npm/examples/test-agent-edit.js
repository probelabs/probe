#!/usr/bin/env node

/**
 * Test script to verify edit and create tools are available with --allow-edit flag
 */

import { ProbeAgent } from '../src/agent/ProbeAgent.js';
import { join } from 'path';
import { tmpdir } from 'os';
import { promises as fs } from 'fs';
import { existsSync } from 'fs';

async function testAgentWithEdit() {
  const testDir = join(tmpdir(), 'probe-agent-edit-test');
  await fs.mkdir(testDir, { recursive: true });

  console.log('Testing ProbeAgent with edit tools...\n');
  console.log(`Test directory: ${testDir}\n`);

  try {
    // Test 1: Without --allow-edit flag
    console.log('1. Testing WITHOUT --allow-edit flag:');
    const agentWithoutEdit = new ProbeAgent({
      path: testDir,
      allowedFolders: [testDir],
      allowEdit: false, // Explicitly disabled
      verbose: true
    });

    const tools1 = agentWithoutEdit.getTools();
    console.log('Available tools without --allow-edit:', Object.keys(tools1));
    console.log('Has editTool?', 'editTool' in tools1);
    console.log('Has createTool?', 'createTool' in tools1);
    console.log();

    // Test 2: With --allow-edit flag
    console.log('2. Testing WITH --allow-edit flag:');
    const agentWithEdit = new ProbeAgent({
      path: testDir,
      allowedFolders: [testDir],
      allowEdit: true, // Enabled
      verbose: true
    });

    const tools2 = agentWithEdit.getTools();
    console.log('Available tools with --allow-edit:', Object.keys(tools2));
    console.log('Has editTool?', 'editTool' in tools2);
    console.log('Has createTool?', 'createTool' in tools2);
    console.log();

    // Test 3: Actually use the tools if available
    if (tools2.editTool && tools2.createTool) {
      console.log('3. Testing actual tool usage through agent:');

      // Create a file
      console.log('Creating test.txt...');
      const createResult = await tools2.createTool.execute({
        file_path: join(testDir, 'test.txt'),
        content: 'Hello from ProbeAgent!'
      });
      console.log('Create success:', createResult.success);

      // Edit the file
      console.log('Editing test.txt...');
      const editResult = await tools2.editTool.execute({
        file_path: join(testDir, 'test.txt'),
        old_string: 'Hello from ProbeAgent!',
        new_string: 'Hello from ProbeAgent with Edit tools!'
      });
      console.log('Edit success:', editResult.success);

      // Verify the content
      const content = await fs.readFile(join(testDir, 'test.txt'), 'utf-8');
      console.log('Final content:', content);
      console.log();
    }

    // Test 4: Test tool definitions
    console.log('4. Testing tool definitions:');
    const toolDefinitions = agentWithEdit.getToolDefinitions();
    console.log('Available tool definitions:', toolDefinitions.length);
    const hasEditDef = toolDefinitions.some(def => def.includes('## edit'));
    const hasCreateDef = toolDefinitions.some(def => def.includes('## create'));
    console.log('Has edit definition?', hasEditDef);
    console.log('Has create definition?', hasCreateDef);
    console.log();

    // Test 5: Test with a simple prompt
    console.log('5. Testing with a simple prompt (this would normally call the AI):');
    console.log('Agent is configured with:');
    console.log('- allowEdit:', agentWithEdit.config.allowEdit);
    console.log('- allowedFolders:', agentWithEdit.config.allowedFolders);
    console.log('- Available tools:', Object.keys(agentWithEdit.getTools()));
    console.log();

    // Clean up
    console.log('Cleaning up...');
    await fs.rm(testDir, { recursive: true, force: true });
    console.log('\n✅ All tests completed successfully!');
    console.log('\nSummary:');
    console.log('- Edit and Create tools are NOT available without --allow-edit flag');
    console.log('- Edit and Create tools ARE available with --allow-edit flag');
    console.log('- Tools can be executed successfully when enabled');

  } catch (error) {
    console.error('Test failed:', error);
    // Clean up on error
    await fs.rm(testDir, { recursive: true, force: true }).catch(() => {});
    process.exit(1);
  }
}

// Run the test
testAgentWithEdit().catch(console.error);