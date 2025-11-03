/**
 * Tests for Edit and Create tools
 */

import { describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { editTool, createTool } from '../../src/tools/edit.js';
import { promises as fs } from 'fs';
import { resolve, join } from 'path';
import { existsSync } from 'fs';
import { tmpdir } from 'os';
import { randomUUID } from 'crypto';

describe('Edit and Create Tools', () => {
  let testDir;
  let testFile;

  beforeEach(async () => {
    // Create a temporary directory for testing
    testDir = join(tmpdir(), `probe-test-${randomUUID()}`);
    await fs.mkdir(testDir, { recursive: true });
    testFile = join(testDir, 'test.txt');
  });

  afterEach(async () => {
    // Clean up test directory
    if (existsSync(testDir)) {
      await fs.rm(testDir, { recursive: true, force: true });
    }
  });

  describe('editTool', () => {
    test('should edit a file with exact string replacement', async () => {
      // Create a test file
      const originalContent = 'Hello, world!\nThis is a test file.\nGoodbye!';
      await fs.writeFile(testFile, originalContent);

      // Create the edit tool
      const edit = editTool({
        debug: false,
        allowedFolders: [testDir]
      });

      // Edit the file
      const result = await edit.execute({
        file_path: testFile,
        old_string: 'This is a test file.',
        new_string: 'This is an edited file.'
      });

      expect(result.success).toBe(true);
      expect(result.replacements).toBe(1);

      // Verify the file was edited
      const newContent = await fs.readFile(testFile, 'utf-8');
      expect(newContent).toBe('Hello, world!\nThis is an edited file.\nGoodbye!');
    });

    test('should handle replace_all option', async () => {
      // Create a test file with repeated content
      const originalContent = 'foo bar foo baz foo';
      await fs.writeFile(testFile, originalContent);

      // Create the edit tool
      const edit = editTool({
        allowedFolders: [testDir]
      });

      // Edit with replace_all
      const result = await edit.execute({
        file_path: testFile,
        old_string: 'foo',
        new_string: 'FOO',
        replace_all: true
      });

      expect(result.success).toBe(true);
      expect(result.replacements).toBe(3);

      // Verify all occurrences were replaced
      const newContent = await fs.readFile(testFile, 'utf-8');
      expect(newContent).toBe('FOO bar FOO baz FOO');
    });

    test('should fail when old_string is not unique without replace_all', async () => {
      // Create a test file with repeated content
      const originalContent = 'foo bar foo baz';
      await fs.writeFile(testFile, originalContent);

      // Create the edit tool
      const edit = editTool({
        allowedFolders: [testDir]
      });

      // Try to edit without replace_all
      const result = await edit.execute({
        file_path: testFile,
        old_string: 'foo',
        new_string: 'FOO',
        replace_all: false
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('Multiple occurrences found');
      expect(result.error).toContain('2 times');
    });

    test('should fail when old_string is not found', async () => {
      // Create a test file
      const originalContent = 'Hello, world!';
      await fs.writeFile(testFile, originalContent);

      // Create the edit tool
      const edit = editTool({
        allowedFolders: [testDir]
      });

      // Try to edit with non-existent string
      const result = await edit.execute({
        file_path: testFile,
        old_string: 'does not exist',
        new_string: 'replacement'
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('String not found in file');
    });

    test('should fail when file does not exist', async () => {
      // Create the edit tool
      const edit = editTool({
        allowedFolders: [testDir]
      });

      // Try to edit non-existent file
      const result = await edit.execute({
        file_path: join(testDir, 'nonexistent.txt'),
        old_string: 'foo',
        new_string: 'bar'
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('File not found');
    });

    test('should respect allowed folders restriction', async () => {
      // Create a test file
      await fs.writeFile(testFile, 'content');

      // Create the edit tool with different allowed folder
      const edit = editTool({
        allowedFolders: ['/some/other/path']
      });

      // Try to edit file outside allowed folders
      const result = await edit.execute({
        file_path: testFile,
        old_string: 'content',
        new_string: 'new content'
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('Permission denied');
    });

    test('should handle whitespace in strings correctly', async () => {
      // Create a test file with indented code
      const originalContent = `function test() {
  const message = "Hello";
  console.log(message);
}`;
      await fs.writeFile(testFile, originalContent);

      // Create the edit tool
      const edit = editTool({
        allowedFolders: [testDir]
      });

      // Edit with exact whitespace matching
      const result = await edit.execute({
        file_path: testFile,
        old_string: '  const message = "Hello";',
        new_string: '  const message = "Goodbye";'
      });

      expect(result.success).toBe(true);

      // Verify the edit
      const newContent = await fs.readFile(testFile, 'utf-8');
      expect(newContent).toContain('const message = "Goodbye"');
      expect(newContent).not.toContain('const message = "Hello"');
    });
  });

  describe('createTool', () => {
    test('should create a new file', async () => {
      const newFile = join(testDir, 'new.txt');
      const content = 'This is new file content';

      // Create the create tool
      const create = createTool({
        allowedFolders: [testDir]
      });

      // Create the file
      const result = await create.execute({
        file_path: newFile,
        content: content
      });

      expect(result.success).toBe(true);
      expect(result.message).toContain('Successfully created');
      expect(result.bytes_written).toBe(Buffer.byteLength(content));

      // Verify the file was created
      expect(existsSync(newFile)).toBe(true);
      const fileContent = await fs.readFile(newFile, 'utf-8');
      expect(fileContent).toBe(content);
    });

    test('should create parent directories if they do not exist', async () => {
      const newFile = join(testDir, 'nested', 'deep', 'file.txt');
      const content = 'Nested file content';

      // Create the create tool
      const create = createTool({
        allowedFolders: [testDir]
      });

      // Create the file
      const result = await create.execute({
        file_path: newFile,
        content: content
      });

      expect(result.success).toBe(true);

      // Verify the file and directories were created
      expect(existsSync(newFile)).toBe(true);
      const fileContent = await fs.readFile(newFile, 'utf-8');
      expect(fileContent).toBe(content);
    });

    test('should fail when file exists without overwrite', async () => {
      // Create an existing file
      await fs.writeFile(testFile, 'existing content');

      // Create the create tool
      const create = createTool({
        allowedFolders: [testDir]
      });

      // Try to create over existing file
      const result = await create.execute({
        file_path: testFile,
        content: 'new content',
        overwrite: false
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('File already exists');

      // Verify original content is preserved
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toBe('existing content');
    });

    test('should overwrite existing file when overwrite is true', async () => {
      // Create an existing file
      await fs.writeFile(testFile, 'existing content');

      // Create the create tool
      const create = createTool({
        allowedFolders: [testDir]
      });

      // Create with overwrite
      const result = await create.execute({
        file_path: testFile,
        content: 'overwritten content',
        overwrite: true
      });

      expect(result.success).toBe(true);
      expect(result.message).toContain('overwrote');

      // Verify file was overwritten
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toBe('overwritten content');
    });

    test('should respect allowed folders restriction', async () => {
      // Create the create tool with different allowed folder
      const create = createTool({
        allowedFolders: ['/some/other/path']
      });

      // Try to create file outside allowed folders
      const result = await create.execute({
        file_path: join(testDir, 'restricted.txt'),
        content: 'content'
      });

      expect(result.success).toBe(false);
      expect(result.error).toContain('Permission denied');
    });

    test('should handle multi-line content correctly', async () => {
      const newFile = join(testDir, 'multiline.js');
      const content = `// JavaScript file
export function hello(name) {
  console.log(\`Hello, \${name}!\`);
  return name;
}

export default hello;`;

      // Create the create tool
      const create = createTool({
        allowedFolders: [testDir]
      });

      // Create the file
      const result = await create.execute({
        file_path: newFile,
        content: content
      });

      expect(result.success).toBe(true);

      // Verify the content
      const fileContent = await fs.readFile(newFile, 'utf-8');
      expect(fileContent).toBe(content);
      expect(fileContent.split('\n').length).toBe(7); // 7 lines
    });
  });

  describe('Integration with relative paths', () => {
    test('should handle relative paths with defaultPath', async () => {
      // Create the edit tool with a default path
      const edit = editTool({
        defaultPath: testDir,
        allowedFolders: [testDir]
      });

      // Create a file
      await fs.writeFile(testFile, 'original');

      // Edit using relative path
      const result = await edit.execute({
        file_path: 'test.txt', // relative path
        old_string: 'original',
        new_string: 'modified'
      });

      expect(result.success).toBe(true);

      // Verify the edit
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toBe('modified');
    });

    test('should handle absolute paths regardless of defaultPath', async () => {
      // Create the create tool with a default path
      const create = createTool({
        defaultPath: '/some/other/path',
        allowedFolders: [testDir]
      });

      const absolutePath = join(testDir, 'absolute.txt');

      // Create using absolute path
      const result = await create.execute({
        file_path: absolutePath,
        content: 'absolute path content'
      });

      expect(result.success).toBe(true);
      expect(existsSync(absolutePath)).toBe(true);
    });
  });
});