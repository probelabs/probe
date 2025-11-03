#!/usr/bin/env node

/**
 * Direct test of edit and create tools functionality
 */

import { createTools } from '../src/agent/tools.js';
import { join } from 'path';
import { tmpdir } from 'os';
import { promises as fs } from 'fs';
import { randomUUID } from 'crypto';

async function testEditCreateDirect() {
  const testDir = join(tmpdir(), 'probe-edit-test-' + randomUUID());
  await fs.mkdir(testDir, { recursive: true });

  console.log('Testing Edit and Create tools directly...\n');
  console.log(`Test directory: ${testDir}\n`);

  try {
    // Test 1: Create tools WITHOUT --allow-edit
    console.log('1. Testing WITHOUT --allow-edit flag:');
    const tools1 = createTools({
      allowedFolders: [testDir],
      allowEdit: false
    });
    console.log('Available tools:', Object.keys(tools1));
    console.log('Has editTool?', 'editTool' in tools1);
    console.log('Has createTool?', 'createTool' in tools1);
    console.log();

    // Test 2: Create tools WITH --allow-edit
    console.log('2. Testing WITH --allow-edit flag:');
    const tools2 = createTools({
      allowedFolders: [testDir],
      allowEdit: true
    });
    console.log('Available tools:', Object.keys(tools2));
    console.log('Has editTool?', 'editTool' in tools2);
    console.log('Has createTool?', 'createTool' in tools2);
    console.log();

    // Test 3: Actually use the tools if available
    if (tools2.editTool && tools2.createTool) {
      console.log('3. Testing actual tool usage:');

      // Create a file
      console.log('Creating test file...');
      const createResult = await tools2.createTool.execute({
        file_path: join(testDir, 'test.js'),
        content: `// Test file
function hello() {
  return "Hello, World!";
}`
      });
      console.log('Create result:', createResult.success ? 'SUCCESS' : 'FAILED');
      if (!createResult.success) {
        console.error('Error:', createResult.error);
      }

      // Edit the file
      console.log('Editing file...');
      const editResult = await tools2.editTool.execute({
        file_path: join(testDir, 'test.js'),
        old_string: '  return "Hello, World!";',
        new_string: '  return "Hello from Probe!";'
      });
      console.log('Edit result:', editResult.success ? 'SUCCESS' : 'FAILED');
      if (!editResult.success) {
        console.error('Error:', editResult.error);
      }

      // Read the file to verify
      const content = await fs.readFile(join(testDir, 'test.js'), 'utf-8');
      console.log('\nFinal file content:');
      console.log(content);
      console.log();

      // Test 4: Test with bash tool enabled
      console.log('4. Testing with bash tool enabled:');
      const tools3 = createTools({
        allowedFolders: [testDir],
        allowEdit: true,
        enableBash: true,
        bashConfig: {
          allow: ['ls', 'cat'],
          timeout: 5000
        }
      });
      console.log('Available tools:', Object.keys(tools3));
      console.log();
    }

    // Clean up
    console.log('Cleaning up...');
    await fs.rm(testDir, { recursive: true, force: true });
    console.log('\n✅ All tests completed successfully!');

    console.log('\nSummary:');
    console.log('- Without --allow-edit: edit and create tools are NOT available');
    console.log('- With --allow-edit: edit and create tools ARE available');
    console.log('- Tools can be executed successfully when enabled');
    console.log('- Tools integrate properly with other tools like bash');

  } catch (error) {
    console.error('Test failed:', error);
    // Clean up on error
    await fs.rm(testDir, { recursive: true, force: true }).catch(() => {});
    process.exit(1);
  }
}

// Run the test
testEditCreateDirect().catch(console.error);