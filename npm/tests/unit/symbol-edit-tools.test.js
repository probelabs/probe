/**
 * Tests for Symbol Edit capabilities integrated into the unified edit tool.
 *
 * The symbol editing features (replace by symbol name, insert before/after symbol)
 * are now part of the edit tool. AST-based extract cannot run in unit tests,
 * so we test:
 *   1. Schema includes symbol/position parameters
 *   2. Input validation for symbol mode (rejects bad parameters before calling extract)
 *   3. File-level guards (non-existent file, path outside allowed dirs)
 *   4. Helper functions from symbolEdit.js (detectBaseIndent, reindent)
 */

import { describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { promises as fs } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

import { editTool, editSchema, editDescription, editToolDefinition } from '../../src/tools/edit.js';
import { findSymbol, detectBaseIndent, reindent } from '../../src/tools/symbolEdit.js';

describe('Unified Edit Tool - Symbol Mode', () => {
  describe('Schema', () => {
    test('should have file_path and new_string as required fields', () => {
      expect(editSchema.required).toEqual(['file_path', 'new_string']);
    });

    test('should include symbol property', () => {
      expect(editSchema.properties.symbol).toBeDefined();
      expect(editSchema.properties.symbol.type).toBe('string');
      expect(editSchema.properties.symbol.description).toBeDefined();
    });

    test('should include position property with before/after enum', () => {
      expect(editSchema.properties.position).toBeDefined();
      expect(editSchema.properties.position.type).toBe('string');
      expect(editSchema.properties.position.enum).toEqual(['before', 'after']);
    });

    test('should not require old_string (it is optional for symbol mode)', () => {
      expect(editSchema.required).not.toContain('old_string');
    });

    test('should still include old_string property for text mode', () => {
      expect(editSchema.properties.old_string).toBeDefined();
      expect(editSchema.properties.old_string.type).toBe('string');
    });

    test('should have type object', () => {
      expect(editSchema.type).toBe('object');
    });
  });

  describe('Description and Definition', () => {
    test('editDescription should mention AST-aware capabilities', () => {
      expect(typeof editDescription).toBe('string');
      expect(editDescription).toContain('AST');
      expect(editDescription).toContain('symbol');
    });

    test('editToolDefinition should contain symbol mode documentation', () => {
      expect(editToolDefinition).toContain('## edit');
      expect(editToolDefinition).toContain('Symbol replace');
      expect(editToolDefinition).toContain('Symbol insert');
      expect(editToolDefinition).toContain('<symbol>');
      expect(editToolDefinition).toContain('<position>');
    });

    test('editToolDefinition should contain text mode documentation', () => {
      expect(editToolDefinition).toContain('<old_string>');
      expect(editToolDefinition).toContain('<new_string>');
    });

    test('editToolDefinition should include symbol mode examples', () => {
      expect(editToolDefinition).toContain('Symbol replace');
      expect(editToolDefinition).toContain('calculateTotal');
      expect(editToolDefinition).toContain('calculateTax');
    });
  });

  describe('Symbol Mode - Input Validation', () => {
    let tempDir;
    let tool;

    beforeEach(async () => {
      tempDir = await fs.mkdtemp(join(tmpdir(), 'edit-symbol-test-'));
      tool = editTool({ cwd: tempDir, allowedFolders: [tempDir] });
    });

    afterEach(async () => {
      await fs.rm(tempDir, { recursive: true, force: true });
    });

    test('should return error for empty symbol', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, 'function foo() { return 1; }');
      const result = await tool.execute({ file_path: testFile, symbol: '', new_string: 'bar' });
      expect(result).toContain('Error editing file');
      expect(result).toContain('Invalid symbol');
    });

    test('should return error for whitespace-only symbol', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, 'function foo() { return 1; }');
      const result = await tool.execute({ file_path: testFile, symbol: '   ', new_string: 'bar' });
      expect(result).toContain('Error editing file');
      expect(result).toContain('Invalid symbol');
    });

    test('should return error for invalid position value', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, 'function foo() {}');
      const result = await tool.execute({
        file_path: testFile,
        symbol: 'foo',
        new_string: 'bar',
        position: 'middle'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('Invalid position');
    });

    test('should accept "before" as a valid position', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, 'function foo() {}');
      const result = await tool.execute({
        file_path: testFile,
        symbol: 'foo',
        new_string: 'bar',
        position: 'before'
      });
      // Should get past validation - will fail at findSymbol because extract is unavailable
      expect(result).not.toContain('Invalid position');
    });

    test('should accept "after" as a valid position', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, 'function foo() {}');
      const result = await tool.execute({
        file_path: testFile,
        symbol: 'foo',
        new_string: 'bar',
        position: 'after'
      });
      expect(result).not.toContain('Invalid position');
    });

    test('should return error for non-existent file in symbol mode', async () => {
      const result = await tool.execute({
        file_path: join(tempDir, 'nonexistent.js'),
        symbol: 'foo',
        new_string: 'bar'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('File not found');
    });

    test('should return error for path outside allowed directories in symbol mode', async () => {
      const result = await tool.execute({
        file_path: '/etc/passwd',
        symbol: 'foo',
        new_string: 'bar'
      });
      expect(result).toContain('Error editing file');
      expect(result).toContain('Permission denied');
    });

    test('should return error when neither old_string nor symbol is provided', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, 'function foo() { return 1; }');
      const result = await tool.execute({ file_path: testFile, new_string: 'bar' });
      expect(result).toContain('Error editing file');
      expect(result).toContain('Must provide either old_string');
    });

    test('should accept empty string as valid new_string in symbol mode', async () => {
      const testFile = join(tempDir, 'test.js');
      await fs.writeFile(testFile, 'function foo() { return 1; }');
      const result = await tool.execute({ file_path: testFile, symbol: 'foo', new_string: '' });
      // Should pass new_string validation - will fail at findSymbol because extract is unavailable
      expect(result).not.toContain('Invalid new_string');
    });
  });
});

describe('symbolEdit.js Helper Functions', () => {
  describe('detectBaseIndent', () => {
    test('should detect leading whitespace of first non-empty line', () => {
      expect(detectBaseIndent('    function foo() {}')).toBe('    ');
    });

    test('should return empty string for unindented code', () => {
      expect(detectBaseIndent('function foo() {}')).toBe('');
    });

    test('should skip empty lines', () => {
      expect(detectBaseIndent('\n\n  function foo() {}')).toBe('  ');
    });

    test('should handle tab indentation', () => {
      expect(detectBaseIndent('\tfunction foo() {}')).toBe('\t');
    });

    test('should return empty string for empty input', () => {
      expect(detectBaseIndent('')).toBe('');
    });

    test('should return empty string for whitespace-only input', () => {
      expect(detectBaseIndent('   \n   \n   ')).toBe('');
    });
  });

  describe('reindent', () => {
    test('should reindent content to target level', () => {
      const input = 'function foo() {\n  return 1;\n}';
      const result = reindent(input, '    ');
      expect(result).toBe('    function foo() {\n      return 1;\n    }');
    });

    test('should handle content already at target indent', () => {
      const input = '  foo();\n  bar();';
      const result = reindent(input, '  ');
      expect(result).toBe('  foo();\n  bar();');
    });

    test('should strip indent and apply new one', () => {
      const input = '      deeply();\n      indented();';
      const result = reindent(input, '  ');
      expect(result).toBe('  deeply();\n  indented();');
    });

    test('should handle empty lines', () => {
      const input = 'foo();\n\nbar();';
      const result = reindent(input, '  ');
      expect(result).toBe('  foo();\n\n  bar();');
    });

    test('should handle single line input', () => {
      const input = 'return 42;';
      const result = reindent(input, '    ');
      expect(result).toBe('    return 42;');
    });
  });
});
