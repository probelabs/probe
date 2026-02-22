/**
 * Tests for line-targeted edit mode in the unified edit tool.
 */

import { describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { promises as fs } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

import { editTool, editSchema, editDescription, editToolDefinition } from '../../src/tools/edit.js';

describe('Line-Targeted Edit Mode', () => {
  let tempDir;
  let tool;

  const sampleContent = [
    'function foo() {',        // line 1
    '  const x = 1;',          // line 2
    '  const y = 2;',          // line 3
    '  const z = x + y;',      // line 4
    '  return z;',             // line 5
    '}'                        // line 6
  ].join('\n');

  beforeEach(async () => {
    tempDir = await fs.mkdtemp(join(tmpdir(), 'edit-line-test-'));
    tool = editTool({ cwd: tempDir, allowedFolders: [tempDir] });
  });

  afterEach(async () => {
    await fs.rm(tempDir, { recursive: true, force: true });
  });

  describe('Schema', () => {
    test('should include start_line property', () => {
      expect(editSchema.properties.start_line).toBeDefined();
      expect(editSchema.properties.start_line.type).toBe('string');
    });

    test('should include end_line property', () => {
      expect(editSchema.properties.end_line).toBeDefined();
      expect(editSchema.properties.end_line.type).toBe('string');
    });

    test('editDescription should mention line-targeted', () => {
      expect(editDescription).toContain('line-targeted');
    });

    test('editToolDefinition should document line-targeted mode', () => {
      expect(editToolDefinition).toContain('Line-targeted edit');
      expect(editToolDefinition).toContain('start_line');
      expect(editToolDefinition).toContain('end_line');
    });
  });

  describe('Replace single line', () => {
    test('should replace a single line by number', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '2',
        new_string: '  const x = 100;'
      });
      expect(result).toContain('Successfully edited');
      expect(result).toContain('line 2 replaced');
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toContain('const x = 100;');
      expect(content).not.toContain('const x = 1;');
    });

    test('should handle numeric start_line (XML coercion)', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: 2,
        new_string: '  const x = 100;'
      });
      expect(result).toContain('Successfully edited');
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toContain('const x = 100;');
    });
  });

  describe('Replace line range', () => {
    test('should replace a range of lines', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '2',
        end_line: '4',
        new_string: '  const result = compute();'
      });
      expect(result).toContain('Successfully edited');
      expect(result).toContain('lines 2-4 replaced');
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toContain('const result = compute();');
      expect(content).not.toContain('const x = 1;');
      expect(content).not.toContain('const y = 2;');
      expect(content).not.toContain('const z = x + y;');
      // Lines 1, 5, 6 should still be there
      expect(content).toContain('function foo() {');
      expect(content).toContain('return z;');
      expect(content).toContain('}');
    });

    test('should handle single-line range (start_line == end_line)', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '3',
        end_line: '3',
        new_string: '  const y = 200;'
      });
      expect(result).toContain('Successfully edited');
      expect(result).toContain('line 3 replaced');
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toContain('const y = 200;');
    });
  });

  describe('Insert mode', () => {
    test('should insert after a line', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '3',
        position: 'after',
        new_string: '  const w = 3;'
      });
      expect(result).toContain('Successfully edited');
      expect(result).toContain('inserted after line 3');
      const content = await fs.readFile(testFile, 'utf-8');
      const lines = content.split('\n');
      expect(lines[3]).toBe('  const w = 3;');
      // Original line 3 should still be there
      expect(lines[2]).toBe('  const y = 2;');
    });

    test('should insert before a line', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '3',
        position: 'before',
        new_string: '  const w = 0;'
      });
      expect(result).toContain('Successfully edited');
      expect(result).toContain('inserted before line 3');
      const content = await fs.readFile(testFile, 'utf-8');
      const lines = content.split('\n');
      expect(lines[2]).toBe('  const w = 0;');
      // Original line 3 should be pushed down
      expect(lines[3]).toBe('  const y = 2;');
    });

    test('should insert multiple lines', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '5',
        position: 'before',
        new_string: '  // compute\n  const sum = x + y + z;'
      });
      expect(result).toContain('2 lines inserted');
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toContain('// compute');
      expect(content).toContain('const sum = x + y + z;');
    });
  });

  describe('Delete lines', () => {
    test('should delete lines when new_string is empty', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '3',
        end_line: '4',
        new_string: ''
      });
      expect(result).toContain('Successfully edited');
      expect(result).toContain('deleted');
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).not.toContain('const y = 2;');
      expect(content).not.toContain('const z = x + y;');
      const lines = content.split('\n');
      expect(lines.length).toBe(4); // 6 original - 2 deleted
    });
  });

  describe('Hash verification', () => {
    test('should accept valid hash', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);

      // Compute the actual hash for line 2
      const { computeLineHash } = await import('../../src/tools/hashline.js');
      const hash = computeLineHash('  const x = 1;');

      const result = await tool.execute({
        file_path: testFile,
        start_line: `2:${hash}`,
        new_string: '  const x = 100;'
      });
      expect(result).toContain('Successfully edited');
    });

    test('should reject invalid hash with helpful error', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      // Use a valid hex hash that doesn't match the actual content
      const result = await tool.execute({
        file_path: testFile,
        start_line: '2:ff',
        new_string: '  const x = 100;'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('has changed since last read');
      // Should include the actual hash for recovery
      expect(result).toMatch(/\d+:[0-9a-f]{2}/);
    });

    test('should validate end_line hash', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '2',
        end_line: '4:ff',
        new_string: '  const result = compute();'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('has changed since last read');
    });
  });

  describe('Response format', () => {
    test('should include context lines with hashes', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '3',
        new_string: '  const y = 200;'
      });
      expect(result).toContain('Context:');
      // Should have LINE:HASH | format
      expect(result).toMatch(/\d+:[0-9a-f]{2} \|/);
      // New lines should be marked with >
      expect(result).toContain('>');
    });
  });

  describe('Error handling', () => {
    test('should error on invalid start_line format', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: 'abc',
        new_string: 'foo'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('Invalid start_line');
    });

    test('should error on invalid end_line format', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '2',
        end_line: 'xyz',
        new_string: 'foo'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('Invalid end_line');
    });

    test('should error when end_line < start_line', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '4',
        end_line: '2',
        new_string: 'foo'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('must be >= start_line');
    });

    test('should error when start_line is beyond file length', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '100',
        new_string: 'foo'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('beyond file length');
    });

    test('should error when end_line is beyond file length', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '2',
        end_line: '100',
        new_string: 'foo'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('beyond file length');
    });

    test('should error on invalid position value', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '2',
        position: 'middle',
        new_string: 'foo'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('Invalid position');
    });

    test('should error for non-existent file', async () => {
      const result = await tool.execute({
        file_path: join(tempDir, 'nonexistent.js'),
        start_line: '1',
        new_string: 'foo'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('File not found');
    });

    test('should error for path outside allowed directories', async () => {
      const result = await tool.execute({
        file_path: '/etc/passwd',
        start_line: '1',
        new_string: 'foo'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('Permission denied');
    });
  });

  describe('Routing priority', () => {
    test('symbol should take priority over start_line', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        symbol: 'nonexistent',
        start_line: '2',
        new_string: 'foo'
      });
      // Should attempt symbol mode (and fail at findSymbol), NOT line mode
      expect(result).toContain('Symbol');
    });

    test('start_line should take priority over old_string', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '2',
        old_string: '  const x = 1;',
        new_string: '  const x = 100;'
      });
      // Should use line mode, not text mode
      expect(result).toContain('Successfully edited');
      expect(result).toContain('line 2 replaced');
    });

    test('error message should mention all three modes when none provided', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        new_string: 'foo'
      });
      expect(result).toContain('old_string');
      expect(result).toContain('symbol');
      expect(result).toContain('start_line');
    });
  });

  describe('Heuristic corrections', () => {
    test('should strip hashline prefixes from new_string', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '2',
        end_line: '3',
        new_string: '2:ab |   const x = 10;\n3:cd |   const y = 20;'
      });
      expect(result).toContain('Successfully edited');
      expect(result).toContain('auto-corrected');
      const content = await fs.readFile(testFile, 'utf-8');
      // Should not contain the prefix in the file
      expect(content).not.toContain('2:ab |');
      expect(content).toContain('const x = 10;');
    });

    test('should strip echoed boundary lines', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      // LLM echoes line 1 before the replacement for line 2
      const result = await tool.execute({
        file_path: testFile,
        start_line: '2',
        new_string: 'function foo() {\n  const x = 100;'
      });
      expect(result).toContain('Successfully edited');
      const content = await fs.readFile(testFile, 'utf-8');
      // Line 1 should not be duplicated
      const lines = content.split('\n');
      expect(lines.filter(l => l.trim() === 'function foo() {').length).toBe(1);
    });
  });

  describe('Edge cases', () => {
    test('should handle editing first line', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '1',
        new_string: 'function bar() {'
      });
      expect(result).toContain('Successfully edited');
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content.startsWith('function bar() {')).toBe(true);
    });

    test('should handle editing last line', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '6',
        new_string: '};'
      });
      expect(result).toContain('Successfully edited');
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content.endsWith('};')).toBe(true);
    });

    test('should handle replacing with more lines than original', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, sampleContent);
      const result = await tool.execute({
        file_path: testFile,
        start_line: '2',
        new_string: '  const a = 1;\n  const b = 2;\n  const c = 3;'
      });
      expect(result).toContain('Successfully edited');
      const content = await fs.readFile(testFile, 'utf-8');
      const lines = content.split('\n');
      expect(lines.length).toBe(8); // 6 - 1 + 3
    });

    test('should handle single-line file', async () => {
      const testFile = join(tempDir, 'single.js');
      await fs.writeFile(testFile, 'console.log("hello");');
      const result = await tool.execute({
        file_path: testFile,
        start_line: '1',
        new_string: 'console.log("world");'
      });
      expect(result).toContain('Successfully edited');
      const content = await fs.readFile(testFile, 'utf-8');
      expect(content).toBe('console.log("world");');
    });
  });
});
