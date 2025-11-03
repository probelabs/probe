#!/usr/bin/env node

/**
 * Example script to test the Edit and Create tools
 */

import { editTool, createTool } from '../src/tools/edit.js';
import { join } from 'path';
import { tmpdir } from 'os';
import { promises as fs } from 'fs';

async function testEditCreateTools() {
  const testDir = join(tmpdir(), 'probe-edit-test');

  console.log('Testing Edit and Create tools...\n');
  console.log(`Working directory: ${testDir}\n`);

  // Create test directory
  await fs.mkdir(testDir, { recursive: true });

  // Initialize tools with the test directory as allowed
  const create = createTool({
    allowedFolders: [testDir],
    debug: true
  });

  const edit = editTool({
    allowedFolders: [testDir],
    debug: true
  });

  try {
    // Test 1: Create a new file
    console.log('1. Creating a new file...');
    const createResult = await create.execute({
      file_path: join(testDir, 'hello.js'),
      content: `// Hello World Example
function greet(name) {
  return "Hello, " + name + "!";
}

console.log(greet("World"));`
    });
    console.log('Create result:', createResult);
    console.log();

    // Test 2: Edit the file
    console.log('2. Editing the file...');
    const editResult = await edit.execute({
      file_path: join(testDir, 'hello.js'),
      old_string: '  return "Hello, " + name + "!";',
      new_string: '  return "Greetings, " + name + "!";'
    });
    console.log('Edit result:', editResult);
    console.log();

    // Test 3: Read and display the edited file
    console.log('3. Reading the edited file:');
    const content = await fs.readFile(join(testDir, 'hello.js'), 'utf-8');
    console.log('--- File Content ---');
    console.log(content);
    console.log('--- End Content ---\n');

    // Test 4: Try to create an existing file without overwrite (should fail)
    console.log('4. Trying to create existing file without overwrite...');
    const failResult = await create.execute({
      file_path: join(testDir, 'hello.js'),
      content: 'new content',
      overwrite: false
    });
    console.log('Expected failure:', failResult);
    console.log();

    // Test 5: Create with overwrite
    console.log('5. Creating with overwrite...');
    const overwriteResult = await create.execute({
      file_path: join(testDir, 'hello.js'),
      content: `// Overwritten file
console.log("This file was overwritten!");`,
      overwrite: true
    });
    console.log('Overwrite result:', overwriteResult);
    console.log();

    // Test 6: Multiple replacements
    console.log('6. Creating file with multiple occurrences...');
    await create.execute({
      file_path: join(testDir, 'multi.txt'),
      content: 'foo bar foo baz foo',
      overwrite: true
    });

    console.log('Replacing all occurrences of "foo" with "FOO"...');
    const multiEditResult = await edit.execute({
      file_path: join(testDir, 'multi.txt'),
      old_string: 'foo',
      new_string: 'FOO',
      replace_all: true
    });
    console.log('Multi-edit result:', multiEditResult);

    const multiContent = await fs.readFile(join(testDir, 'multi.txt'), 'utf-8');
    console.log('Final content:', multiContent);
    console.log();

    // Clean up
    console.log('Cleaning up test directory...');
    await fs.rm(testDir, { recursive: true, force: true });
    console.log('Test completed successfully! ✅');

  } catch (error) {
    console.error('Test failed:', error);
    // Clean up on error
    await fs.rm(testDir, { recursive: true, force: true }).catch(() => {});
    process.exit(1);
  }
}

// Run the test
testEditCreateTools().catch(console.error);