/**
 * Tests for Edit and Create tools
 */

import { describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { editTool, createTool } from '../../src/tools/edit.js';
import { FileTracker } from '../../src/tools/fileTracker.js';
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

      expect(result).toBe('Successfully edited ' + testFile + ' (1 replacement)');

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

      expect(result).toBe('Successfully edited ' + testFile + ' (3 replacements)');

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

      expect(result).toContain('Error editing file: Multiple occurrences found');
      expect(result).toContain('2 times');
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

      expect(result).toContain('Error editing file: String not found');
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

      expect(result).toContain('Error editing file: File not found');
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

      expect(result).toContain('Error editing file: Permission denied');
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

      expect(result).toBe('Successfully edited ' + testFile + ' (1 replacement)');

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

      expect(result).toContain('Successfully created');
      expect(result).toContain(`(${Buffer.byteLength(content)} bytes)`);

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

      expect(result).toContain('Successfully created');

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

      expect(result).toContain('Error creating file: File already exists');

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

      expect(result).toContain('Successfully overwrote');

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

      expect(result).toContain('Error creating file: Permission denied');
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

      expect(result).toContain('Successfully created');

      // Verify the content
      const fileContent = await fs.readFile(newFile, 'utf-8');
      expect(fileContent).toBe(content);
      expect(fileContent.split('\n').length).toBe(7); // 7 lines
    });
  });

  describe('Integration with relative paths', () => {
    test('should handle relative paths with cwd', async () => {
      // Create the edit tool with a working directory
      const edit = editTool({
        cwd: testDir,
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

      expect(result).toBe('Successfully edited test.txt (1 replacement)');

      // Verify the edit
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toBe('modified');
    });

    test('should handle absolute paths regardless of cwd', async () => {
      // Create the create tool with a working directory
      const create = createTool({
        cwd: '/some/other/path',
        allowedFolders: [testDir]
      });

      const absolutePath = join(testDir, 'absolute.txt');

      // Create using absolute path
      const result = await create.execute({
        file_path: absolutePath,
        content: 'absolute path content'
      });

      expect(result).toContain('Successfully created');
      expect(result).toContain('bytes)');
      expect(existsSync(absolutePath)).toBe(true);
    });
  });

  describe('Input Validation', () => {
    describe('editTool validation', () => {
      test('should handle invalid file_path', async () => {
        const edit = editTool({ allowedFolders: [testDir] });

        // Empty string
        let result = await edit.execute({
          file_path: '',
          old_string: 'foo',
          new_string: 'bar'
        });
        expect(result).toContain('Error editing file: Invalid file_path');

        // Null
        result = await edit.execute({
          file_path: null,
          old_string: 'foo',
          new_string: 'bar'
        });
        expect(result).toContain('Error editing file: Invalid file_path');

        // Whitespace only
        result = await edit.execute({
          file_path: '   ',
          old_string: 'foo',
          new_string: 'bar'
        });
        expect(result).toContain('Error editing file: Invalid file_path');
      });

      test('should handle invalid old_string', async () => {
        const edit = editTool({ allowedFolders: [testDir] });
        await fs.writeFile(testFile, 'test content');

        // Undefined - should prompt to provide old_string or symbol
        let result = await edit.execute({
          file_path: testFile,
          old_string: undefined,
          new_string: 'bar'
        });
        expect(result).toContain('Error editing file: Must provide either old_string');

        // Null - should prompt to provide old_string or symbol
        result = await edit.execute({
          file_path: testFile,
          old_string: null,
          new_string: 'bar'
        });
        expect(result).toContain('Error editing file: Must provide either old_string');
      });

      test('should handle invalid new_string', async () => {
        const edit = editTool({ allowedFolders: [testDir] });
        await fs.writeFile(testFile, 'test content');

        // Undefined
        let result = await edit.execute({
          file_path: testFile,
          old_string: 'test',
          new_string: undefined
        });
        expect(result).toContain('Error editing file: Invalid new_string');

        // Null
        result = await edit.execute({
          file_path: testFile,
          old_string: 'test',
          new_string: null
        });
        expect(result).toContain('Error editing file: Invalid new_string');
      });

      test('should handle empty strings in old_string and new_string', async () => {
        const edit = editTool({ allowedFolders: [testDir] });
        await fs.writeFile(testFile, 'test  content'); // Double space

        // Empty old_string matches everywhere, so it will find multiple occurrences
        let result = await edit.execute({
          file_path: testFile,
          old_string: '',
          new_string: 'inserted'
        });
        // Empty string will match at every position, causing multiple occurrences error
        expect(result).toContain('Error editing file: Multiple occurrences found');

        // Empty new_string (valid - can replace with empty)
        await fs.writeFile(testFile, 'test content');
        result = await edit.execute({
          file_path: testFile,
          old_string: ' ',
          new_string: ''
        });
        expect(result).toContain('Successfully edited');
      });
    });

    describe('createTool validation', () => {
      test('should handle invalid file_path', async () => {
        const create = createTool({ allowedFolders: [testDir] });

        // Empty string
        let result = await create.execute({
          file_path: '',
          content: 'test'
        });
        expect(result).toContain('Error creating file: Invalid file_path');

        // Null
        result = await create.execute({
          file_path: null,
          content: 'test'
        });
        expect(result).toContain('Error creating file: Invalid file_path');
      });

      test('should handle invalid content', async () => {
        const create = createTool({ allowedFolders: [testDir] });

        // Undefined
        let result = await create.execute({
          file_path: join(testDir, 'test.txt'),
          content: undefined
        });
        expect(result).toContain('Error creating file: Invalid content');

        // Null
        result = await create.execute({
          file_path: join(testDir, 'test.txt'),
          content: null
        });
        expect(result).toContain('Error creating file: Invalid content');
      });

      test('should handle empty content', async () => {
        const create = createTool({ allowedFolders: [testDir] });

        // Empty string content is valid (creates empty file)
        const result = await create.execute({
          file_path: join(testDir, 'empty.txt'),
          content: ''
        });
        expect(result).toContain('Successfully created');
        expect(result).toContain('(0 bytes)');

        const content = await fs.readFile(join(testDir, 'empty.txt'), 'utf-8');
        expect(content).toBe('');
      });
    });
  });

  describe('Edge Cases', () => {
    test('should handle very large files', async () => {
      // Create a large file (1MB)
      const largeContent = 'x'.repeat(1024 * 1024);
      const largeFile = join(testDir, 'large.txt');
      await fs.writeFile(largeFile, largeContent);

      const edit = editTool({ allowedFolders: [testDir] });
      const result = await edit.execute({
        file_path: largeFile,
        old_string: 'x'.repeat(100),
        new_string: 'y'.repeat(100),
        replace_all: true  // Need replace_all since pattern repeats many times
      });

      expect(result).toContain('Successfully edited');
    });

    test('should handle files with special characters in path', async () => {
      const specialFile = join(testDir, 'file with spaces & special.txt');

      const create = createTool({ allowedFolders: [testDir] });
      let result = await create.execute({
        file_path: specialFile,
        content: 'content'
      });
      expect(result).toContain('Successfully created');

      const edit = editTool({ allowedFolders: [testDir] });
      result = await edit.execute({
        file_path: specialFile,
        old_string: 'content',
        new_string: 'new content'
      });
      expect(result).toContain('Successfully edited');
    });

    test('should handle Unicode content correctly', async () => {
      const unicodeFile = join(testDir, 'unicode.txt');
      const unicodeContent = '你好世界 🌍 émojis ñ';

      const create = createTool({ allowedFolders: [testDir] });
      const createResult = await create.execute({
        file_path: unicodeFile,
        content: unicodeContent
      });
      expect(createResult).toContain('Successfully created');

      const edit = editTool({ allowedFolders: [testDir] });
      const editResult = await edit.execute({
        file_path: unicodeFile,
        old_string: '你好世界',
        new_string: '再见世界'
      });
      expect(editResult).toContain('Successfully edited');

      const content = await fs.readFile(unicodeFile, 'utf-8');
      expect(content).toContain('再见世界');
      expect(content).toContain('🌍');
    });

    test('should handle line endings correctly', async () => {
      const lineEndingFile = join(testDir, 'lineending.txt');

      // Test with different line endings
      const windowsContent = 'line1\r\nline2\r\nline3';
      const unixContent = 'line1\nline2\nline3';

      const create = createTool({ allowedFolders: [testDir] });
      await create.execute({
        file_path: lineEndingFile,
        content: windowsContent
      });

      const edit = editTool({ allowedFolders: [testDir] });
      const result = await edit.execute({
        file_path: lineEndingFile,
        old_string: 'line2',
        new_string: 'modified'
      });
      expect(result).toContain('Successfully edited');

      const content = await fs.readFile(lineEndingFile, 'utf-8');
      expect(content).toContain('modified');
    });
  });

  describe('Fuzzy Matching Integration', () => {
    test('should fall back to line-trimmed matching when exact match fails', async () => {
      // File has specific indentation
      const originalContent = `function greet() {
  const name = "World";
  console.log("Hello, " + name);
}`;
      await fs.writeFile(testFile, originalContent);

      const edit = editTool({ allowedFolders: [testDir] });

      // Search string has different leading whitespace per line (no indent)
      const result = await edit.execute({
        file_path: testFile,
        old_string: 'const name = "World";\nconsole.log("Hello, " + name);',
        new_string: '  const name = "Universe";\n  console.log("Hello, " + name);'
      });

      expect(result).toContain('Successfully edited');
      expect(result).toContain('matched via line-trimmed');

      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toContain('const name = "Universe"');
      expect(content).not.toContain('const name = "World"');
    });

    test('should fall back to whitespace-normalized matching', async () => {
      // File has single spaces
      const originalContent = 'const x = foo(a, b, c);';
      await fs.writeFile(testFile, originalContent);

      const edit = editTool({ allowedFolders: [testDir] });

      // Search string has extra spaces (double space after comma)
      const result = await edit.execute({
        file_path: testFile,
        old_string: 'const x = foo(a,  b,  c);',
        new_string: 'const x = bar(a, b, c);'
      });

      expect(result).toContain('Successfully edited');
      expect(result).toContain('matched via whitespace-normalized');

      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toBe('const x = bar(a, b, c);');
    });

    test('should fall back to fuzzy matching for different indentation', async () => {
      // File uses 4-space indentation
      const originalContent = `function test() {
    const a = 1;
    const b = 2;
    return a + b;
}`;
      await fs.writeFile(testFile, originalContent);

      const edit = editTool({ allowedFolders: [testDir] });

      // Search string uses 2-space indentation — line-trimmed will match first
      // (it's more permissive than indent-flexible in the cascade)
      const result = await edit.execute({
        file_path: testFile,
        old_string: '  const a = 1;\n  const b = 2;\n  return a + b;',
        new_string: '    const a = 10;\n    const b = 20;\n    return a + b;'
      });

      expect(result).toContain('Successfully edited');
      expect(result).toMatch(/matched via (line-trimmed|indent-flexible)/);

      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toContain('const a = 10');
      expect(content).toContain('const b = 20');
    });

    test('should use fuzzy matching with replace_all', async () => {
      // File has two identical multiline blocks with 2-space indentation
      const originalContent = `function a() {
  doStuff();
  log("done");
}

function b() {
  doStuff();
  log("done");
}`;
      await fs.writeFile(testFile, originalContent);

      const edit = editTool({ allowedFolders: [testDir] });

      // Multiline search without indentation — NOT a substring because
      // content has '\n  log' but search has '\nlog', forcing fuzzy fallback
      const result = await edit.execute({
        file_path: testFile,
        old_string: 'doStuff();\nlog("done");',
        new_string: '  doOtherStuff();\n  log("complete");',
        replace_all: true
      });

      expect(result).toContain('Successfully edited');
      expect(result).toContain('matched via line-trimmed');
      expect(result).toContain('2 replacements');

      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).not.toContain('doStuff()');
      expect(content).toContain('doOtherStuff()');
    });

    test('should error on multiple fuzzy matches without replace_all', async () => {
      // File has two identical multiline blocks with 4-space indentation
      const originalContent = `function a() {
    processData();
    saveResult();
}

function b() {
    processData();
    saveResult();
}`;
      await fs.writeFile(testFile, originalContent);

      const edit = editTool({ allowedFolders: [testDir] });

      // Multiline search without indentation — NOT a substring because
      // content has '\n    saveResult' but search has '\nsaveResult'
      const result = await edit.execute({
        file_path: testFile,
        old_string: 'processData();\nsaveResult();',
        new_string: '    processOther();\n    saveOther();'
      });

      expect(result).toContain('Error editing file: Multiple occurrences found');
      expect(result).toContain('2 times');
    });

    test('should return string not found when no fuzzy strategy matches', async () => {
      const originalContent = 'function hello() { return 1; }';
      await fs.writeFile(testFile, originalContent);

      const edit = editTool({ allowedFolders: [testDir] });

      const result = await edit.execute({
        file_path: testFile,
        old_string: 'this text does not exist anywhere at all',
        new_string: 'replacement'
      });

      expect(result).toContain('Error editing file: String not found');
    });

    test('should prefer exact match over fuzzy match', async () => {
      const originalContent = 'const x = 1;\nconst x = 1;';
      await fs.writeFile(testFile, originalContent);

      const edit = editTool({ allowedFolders: [testDir] });

      // This is an exact match (appears twice), so fuzzy matching is NOT triggered
      const result = await edit.execute({
        file_path: testFile,
        old_string: 'const x = 1;',
        new_string: 'const x = 2;'
      });

      // Exact match finds 2 occurrences → multiple occurrences error
      expect(result).toContain('Error editing file: Multiple occurrences found');
    });

    test('fuzzy match success message includes strategy name', async () => {
      const originalContent = '  const   value  =  42;';
      await fs.writeFile(testFile, originalContent);

      const edit = editTool({ allowedFolders: [testDir] });

      const result = await edit.execute({
        file_path: testFile,
        old_string: 'const value = 42;',
        new_string: 'const value = 99;'
      });

      expect(result).toContain('Successfully edited');
      expect(result).toMatch(/matched via (line-trimmed|whitespace-normalized|indent-flexible)/);
    });

    test('realistic LLM scenario: code extracted then edited with stripped indentation', async () => {
      // Simulates real usage: LLM sees indented code from extract output,
      // strips the outer function indentation when referencing inner code
      const originalContent = `function validateUser(user) {
    if (!user.name) {
        throw new Error('Name required');
    }
    if (!user.email) {
        throw new Error('Email required');
    }
    return true;
}`;
      await fs.writeFile(testFile, originalContent);

      const edit = editTool({ allowedFolders: [testDir] });

      // LLM references the inner code but strips the 4-space outer indent,
      // using its own 0-space base. Multiline ensures NOT a substring.
      const result = await edit.execute({
        file_path: testFile,
        old_string: `if (!user.name) {
    throw new Error('Name required');
}
if (!user.email) {
    throw new Error('Email required');
}`,
        new_string: `    if (!user.name || !user.email) {
        throw new Error('Name and email required');
    }`
      });

      expect(result).toContain('Successfully edited');
      expect(result).toContain('matched via line-trimmed');

      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toContain("throw new Error('Name and email required')");
      expect(content).not.toContain("throw new Error('Name required')");
      expect(content).toContain('return true;'); // Surrounding code preserved
    });

    test('realistic LLM scenario: extra spaces in operator expressions', async () => {
      // LLM types code from memory with slightly different spacing around operators
      const originalContent = `const result = items.filter(x => x.active).map(x => x.name);`;
      await fs.writeFile(testFile, originalContent);

      const edit = editTool({ allowedFolders: [testDir] });

      // LLM adds spaces around arrows — NOT an exact substring
      const result = await edit.execute({
        file_path: testFile,
        old_string: 'const result = items.filter(x  =>  x.active).map(x  =>  x.name);',
        new_string: 'const result = items.filter(x => x.enabled).map(x => x.label);'
      });

      expect(result).toContain('Successfully edited');
      expect(result).toContain('matched via whitespace-normalized');

      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toContain('x.enabled');
      expect(content).toContain('x.label');
    });
  });

  // ─── FileTracker Integration ───

  describe('fileTracker integration', () => {
    test('edit succeeds when file was seen', async () => {
      const originalContent = 'Hello, world!\nThis is a test.';
      await fs.writeFile(testFile, originalContent);

      const tracker = new FileTracker();
      tracker.markFileSeen(testFile);

      const edit = editTool({
        allowedFolders: [testDir],
        fileTracker: tracker
      });

      const result = await edit.execute({
        file_path: testFile,
        old_string: 'This is a test.',
        new_string: 'This was edited.'
      });

      expect(result).toContain('Successfully edited');
    });

    test('edit fails with untracked message when file not seen', async () => {
      const originalContent = 'Hello, world!';
      await fs.writeFile(testFile, originalContent);

      const tracker = new FileTracker();
      // Do NOT mark file as seen

      const edit = editTool({
        allowedFolders: [testDir],
        fileTracker: tracker
      });

      const result = await edit.execute({
        file_path: testFile,
        old_string: 'Hello',
        new_string: 'Goodbye'
      });

      expect(result).toContain('not been read yet');
      expect(result).toContain('extract');

      // File should be unchanged
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toBe(originalContent);
    });

    test('edit succeeds even when file modified externally (seen-check only)', async () => {
      // Key behavioral change: file-level mtime no longer blocks edits
      await fs.writeFile(testFile, 'original content here');

      const tracker = new FileTracker();
      tracker.markFileSeen(testFile);

      // Modify the file externally
      await fs.writeFile(testFile, 'modified content here that is now different length');

      const edit = editTool({
        allowedFolders: [testDir],
        fileTracker: tracker
      });

      const result = await edit.execute({
        file_path: testFile,
        old_string: 'modified',
        new_string: 'changed'
      });

      // Should succeed — file was seen, content matching handles the rest
      expect(result).toContain('Successfully edited');
    });

    test('chained edits succeed with text mode', async () => {
      await fs.writeFile(testFile, 'line 1\nline 2\nline 3');

      const tracker = new FileTracker();
      tracker.markFileSeen(testFile);

      const edit = editTool({
        allowedFolders: [testDir],
        fileTracker: tracker
      });

      // First edit
      const result1 = await edit.execute({
        file_path: testFile,
        old_string: 'line 2',
        new_string: 'line TWO'
      });
      expect(result1).toContain('Successfully edited');

      // Second edit — file was modified by first edit but seen-check passes
      const result2 = await edit.execute({
        file_path: testFile,
        old_string: 'line 3',
        new_string: 'line THREE'
      });
      expect(result2).toContain('Successfully edited');

      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toBe('line 1\nline TWO\nline THREE');
    });

    test('standalone editTool without fileTracker works unchanged', async () => {
      await fs.writeFile(testFile, 'standalone test');

      // No fileTracker in options
      const edit = editTool({
        allowedFolders: [testDir]
      });

      const result = await edit.execute({
        file_path: testFile,
        old_string: 'standalone test',
        new_string: 'still works'
      });

      expect(result).toContain('Successfully edited');
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toBe('still works');
    });

    test('createTool updates tracker after write', async () => {
      const newFile = join(testDir, 'created.js');
      const tracker = new FileTracker();

      const create = createTool({
        allowedFolders: [testDir],
        fileTracker: tracker
      });

      const result = await create.execute({
        file_path: newFile,
        content: 'new file content'
      });

      expect(result).toContain('Successfully');
      expect(tracker.isTracked(newFile)).toBe(true);
    });

    test('symbol edit updates tracker after write', async () => {
      const jsFile = join(testDir, 'sym.js');
      await fs.writeFile(jsFile, 'function hello() {\n  return "hi";\n}\n');

      const tracker = new FileTracker();
      tracker.markFileSeen(jsFile);

      const edit = editTool({
        allowedFolders: [testDir],
        fileTracker: tracker
      });

      const result = await edit.execute({
        file_path: jsFile,
        symbol: 'hello',
        new_string: 'function hello() {\n  return "world";\n}'
      });

      expect(result).toContain('Successfully replaced symbol');
      // Tracker should be updated — second edit should work
      const check = tracker.checkBeforeEdit(jsFile);
      expect(check.ok).toBe(true);
    });

    test('line-targeted edit updates tracker after write', async () => {
      const lineFile = join(testDir, 'lines.js');
      await fs.writeFile(lineFile, 'line one\nline two\nline three\n');

      const tracker = new FileTracker();
      tracker.markFileSeen(lineFile);

      const edit = editTool({
        allowedFolders: [testDir],
        fileTracker: tracker
      });

      const result = await edit.execute({
        file_path: lineFile,
        start_line: '2',
        new_string: 'line TWO'
      });

      expect(result).toContain('Successfully edited');
      // Tracker should be updated
      const check = tracker.checkBeforeEdit(lineFile);
      expect(check.ok).toBe(true);
    });
  });
});