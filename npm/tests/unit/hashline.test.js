/**
 * Tests for hashline.js utility module.
 */

import { describe, test, expect } from '@jest/globals';
import {
  computeLineHash,
  parseLineRef,
  validateLineHash,
  annotateOutputWithHashes,
  stripHashlinePrefixes
} from '../../src/tools/hashline.js';

describe('computeLineHash', () => {
  test('should return a 2-char hex string', () => {
    const hash = computeLineHash('function foo() {}');
    expect(hash).toMatch(/^[0-9a-f]{2}$/);
  });

  test('should be deterministic', () => {
    const hash1 = computeLineHash('const x = 42;');
    const hash2 = computeLineHash('const x = 42;');
    expect(hash1).toBe(hash2);
  });

  test('should be whitespace-agnostic', () => {
    const hash1 = computeLineHash('  const x = 42;');
    const hash2 = computeLineHash('const x = 42;');
    const hash3 = computeLineHash('    const   x   =   42;');
    expect(hash1).toBe(hash2);
    expect(hash2).toBe(hash3);
  });

  test('should produce different hashes for different content', () => {
    const hash1 = computeLineHash('const x = 1;');
    const hash2 = computeLineHash('const x = 2;');
    expect(hash1).not.toBe(hash2);
  });

  test('should handle empty string', () => {
    const hash = computeLineHash('');
    expect(hash).toMatch(/^[0-9a-f]{2}$/);
  });

  test('should handle null/undefined', () => {
    const hash1 = computeLineHash(null);
    const hash2 = computeLineHash(undefined);
    expect(hash1).toMatch(/^[0-9a-f]{2}$/);
    expect(hash2).toMatch(/^[0-9a-f]{2}$/);
    // Both null and undefined produce same hash as empty string
    expect(hash1).toBe(hash2);
  });

  test('should handle whitespace-only line', () => {
    const hash = computeLineHash('     ');
    // Whitespace-only = same as empty after stripping
    expect(hash).toBe(computeLineHash(''));
  });

  test('should handle special characters', () => {
    const hash = computeLineHash('// comment: @#$%^&*()');
    expect(hash).toMatch(/^[0-9a-f]{2}$/);
  });

  test('should handle very long lines', () => {
    const longLine = 'a'.repeat(10000);
    const hash = computeLineHash(longLine);
    expect(hash).toMatch(/^[0-9a-f]{2}$/);
  });

  test('should handle unicode content', () => {
    const hash = computeLineHash('const name = "世界";');
    expect(hash).toMatch(/^[0-9a-f]{2}$/);
  });

  test('tab vs spaces are considered different (after stripping whitespace)', () => {
    // Both get stripped, so they hash the same
    const hash1 = computeLineHash('\t\tfoo();');
    const hash2 = computeLineHash('    foo();');
    expect(hash1).toBe(hash2);
  });
});

describe('parseLineRef', () => {
  test('should parse plain line number string', () => {
    const result = parseLineRef('42');
    expect(result).toEqual({ line: 42, hash: null });
  });

  test('should parse line with hash', () => {
    const result = parseLineRef('42:ab');
    expect(result).toEqual({ line: 42, hash: 'ab' });
  });

  test('should handle number input (XML coercion)', () => {
    const result = parseLineRef(42);
    expect(result).toEqual({ line: 42, hash: null });
  });

  test('should normalize hash to lowercase', () => {
    const result = parseLineRef('42:AB');
    expect(result).toEqual({ line: 42, hash: 'ab' });
  });

  test('should handle line 1', () => {
    const result = parseLineRef('1');
    expect(result).toEqual({ line: 1, hash: null });
  });

  test('should handle large line numbers', () => {
    const result = parseLineRef('99999');
    expect(result).toEqual({ line: 99999, hash: null });
  });

  test('should handle line with hash "00"', () => {
    const result = parseLineRef('1:00');
    expect(result).toEqual({ line: 1, hash: '00' });
  });

  test('should handle line with hash "ff"', () => {
    const result = parseLineRef('1:ff');
    expect(result).toEqual({ line: 1, hash: 'ff' });
  });

  test('should return null for line 0', () => {
    const result = parseLineRef('0');
    expect(result).toBeNull();
  });

  test('should return null for negative number', () => {
    const result = parseLineRef('-1');
    expect(result).toBeNull();
  });

  test('should return null for empty string', () => {
    const result = parseLineRef('');
    expect(result).toBeNull();
  });

  test('should return null for null', () => {
    const result = parseLineRef(null);
    expect(result).toBeNull();
  });

  test('should return null for undefined', () => {
    const result = parseLineRef(undefined);
    expect(result).toBeNull();
  });

  test('should return null for non-numeric string', () => {
    const result = parseLineRef('abc');
    expect(result).toBeNull();
  });

  test('should return null for invalid hash format (too long)', () => {
    const result = parseLineRef('42:abc');
    expect(result).toBeNull();
  });

  test('should return null for invalid hash format (too short)', () => {
    const result = parseLineRef('42:a');
    expect(result).toBeNull();
  });

  test('should return null for invalid hash characters', () => {
    const result = parseLineRef('42:zz');
    expect(result).toBeNull();
  });

  test('should return null for floating point', () => {
    const result = parseLineRef('42.5');
    expect(result).toBeNull();
  });

  test('should return null for string with spaces', () => {
    const result = parseLineRef('42 ab');
    expect(result).toBeNull();
  });

  test('should handle whitespace-padded input', () => {
    const result = parseLineRef('  42  ');
    expect(result).toEqual({ line: 42, hash: null });
  });

  test('should return null for number 0', () => {
    const result = parseLineRef(0);
    expect(result).toBeNull();
  });

  test('should handle number 1', () => {
    const result = parseLineRef(1);
    expect(result).toEqual({ line: 1, hash: null });
  });
});

describe('validateLineHash', () => {
  const fileLines = [
    'function foo() {',    // line 1
    '  return 42;',        // line 2
    '}'                    // line 3
  ];

  test('should validate correct hash', () => {
    const expectedHash = computeLineHash(fileLines[0]);
    const result = validateLineHash(1, expectedHash, fileLines);
    expect(result.valid).toBe(true);
    expect(result.actualHash).toBe(expectedHash);
    expect(result.actualContent).toBe('function foo() {');
  });

  test('should invalidate wrong hash', () => {
    const result = validateLineHash(1, 'zz', fileLines);
    expect(result.valid).toBe(false);
    expect(result.actualContent).toBe('function foo() {');
  });

  test('should handle line out of range (too high)', () => {
    const result = validateLineHash(10, 'ab', fileLines);
    expect(result.valid).toBe(false);
    expect(result.actualHash).toBe('');
    expect(result.actualContent).toBe('');
  });

  test('should handle line 0 (out of range)', () => {
    const result = validateLineHash(0, 'ab', fileLines);
    expect(result.valid).toBe(false);
  });

  test('should handle negative line (out of range)', () => {
    const result = validateLineHash(-1, 'ab', fileLines);
    expect(result.valid).toBe(false);
  });

  test('should be case-insensitive', () => {
    const expectedHash = computeLineHash(fileLines[0]);
    const upperHash = expectedHash.toUpperCase();
    const result = validateLineHash(1, upperHash, fileLines);
    expect(result.valid).toBe(true);
  });

  test('should validate last line', () => {
    const expectedHash = computeLineHash(fileLines[2]);
    const result = validateLineHash(3, expectedHash, fileLines);
    expect(result.valid).toBe(true);
  });

  test('should validate empty file', () => {
    const result = validateLineHash(1, 'ab', []);
    expect(result.valid).toBe(false);
  });
});

describe('annotateOutputWithHashes', () => {
  test('should annotate standard probe output lines', () => {
    const input = '  42 | function foo() {}\n  43 |   return 1;\n  44 | }';
    const result = annotateOutputWithHashes(input);
    const lines = result.split('\n');
    // Each line should have LINE:HASH | format
    expect(lines[0]).toMatch(/^\s*42:[0-9a-f]{2}\s*\| function foo\(\) \{\}$/);
    expect(lines[1]).toMatch(/^\s*43:[0-9a-f]{2}\s*\|   return 1;$/);
    expect(lines[2]).toMatch(/^\s*44:[0-9a-f]{2}\s*\| \}$/);
  });

  test('should not modify non-matching lines', () => {
    const input = 'File: src/main.js\nResults found: 3';
    const result = annotateOutputWithHashes(input);
    expect(result).toBe(input);
  });

  test('should handle mixed output', () => {
    const input = 'File: test.js\n  10 | const x = 1;\nMatches: 1';
    const result = annotateOutputWithHashes(input);
    const lines = result.split('\n');
    expect(lines[0]).toBe('File: test.js');
    expect(lines[1]).toMatch(/^\s*10:[0-9a-f]{2}\s*\| const x = 1;$/);
    expect(lines[2]).toBe('Matches: 1');
  });

  test('should handle empty input', () => {
    expect(annotateOutputWithHashes('')).toBe('');
  });

  test('should handle null input', () => {
    expect(annotateOutputWithHashes(null)).toBeNull();
  });

  test('should handle undefined input', () => {
    expect(annotateOutputWithHashes(undefined)).toBeUndefined();
  });

  test('should handle single-digit line numbers', () => {
    const input = '  1 | first line';
    const result = annotateOutputWithHashes(input);
    expect(result).toMatch(/^\s*1:[0-9a-f]{2}\s*\| first line$/);
  });

  test('should handle 4+ digit line numbers', () => {
    const input = '  1234 | some code';
    const result = annotateOutputWithHashes(input);
    expect(result).toMatch(/^\s*1234:[0-9a-f]{2}\s*\| some code$/);
  });

  test('should produce consistent hashes for same content', () => {
    const input1 = '  42 | function foo() {}';
    const input2 = '  42 | function foo() {}';
    expect(annotateOutputWithHashes(input1)).toBe(annotateOutputWithHashes(input2));
  });

  test('should handle line with empty content after pipe', () => {
    const input = '  42 |';
    const result = annotateOutputWithHashes(input);
    expect(result).toMatch(/^\s*42:[0-9a-f]{2}\s*\|$/);
  });

  test('should handle pipe in content (only first pipe matters)', () => {
    const input = '  42 | x = a | b;';
    const result = annotateOutputWithHashes(input);
    expect(result).toMatch(/^\s*42:[0-9a-f]{2}\s*\| x = a \| b;$/);
  });
});

describe('stripHashlinePrefixes', () => {
  test('should strip line:hash prefixes from all lines', () => {
    const input = '42:ab | function foo() {\n43:cd |   return 1;\n44:ef | }';
    const { cleaned, stripped } = stripHashlinePrefixes(input);
    expect(stripped).toBe(true);
    expect(cleaned).toBe('function foo() {\n  return 1;\n}');
  });

  test('should strip plain line number prefixes', () => {
    const input = '42 | function foo() {\n43 |   return 1;\n44 | }';
    const { cleaned, stripped } = stripHashlinePrefixes(input);
    expect(stripped).toBe(true);
    expect(cleaned).toBe('function foo() {\n  return 1;\n}');
  });

  test('should not strip when minority of lines have prefixes', () => {
    const input = 'function foo() {\n42 | return 1;\n}';
    const { cleaned, stripped } = stripHashlinePrefixes(input);
    expect(stripped).toBe(false);
    expect(cleaned).toBe(input);
  });

  test('should handle empty string', () => {
    const { cleaned, stripped } = stripHashlinePrefixes('');
    expect(stripped).toBe(false);
    expect(cleaned).toBe('');
  });

  test('should handle null', () => {
    const { cleaned, stripped } = stripHashlinePrefixes(null);
    expect(stripped).toBe(false);
    expect(cleaned).toBe('');
  });

  test('should handle undefined', () => {
    const { cleaned, stripped } = stripHashlinePrefixes(undefined);
    expect(stripped).toBe(false);
    expect(cleaned).toBe('');
  });

  test('should preserve empty lines between prefixed lines', () => {
    const input = '42:ab | foo();\n\n44:cd | bar();';
    const { cleaned, stripped } = stripHashlinePrefixes(input);
    expect(stripped).toBe(true);
    expect(cleaned).toBe('foo();\n\nbar();');
  });

  test('should handle content that looks like but is not a prefix', () => {
    const input = 'port 8080 is used\nport 3000 is free';
    const { cleaned, stripped } = stripHashlinePrefixes(input);
    expect(stripped).toBe(false);
    expect(cleaned).toBe(input);
  });

  test('should strip prefixes with varying whitespace', () => {
    const input = '  42:ab | foo();\n  43:cd | bar();';
    const { cleaned, stripped } = stripHashlinePrefixes(input);
    expect(stripped).toBe(true);
    expect(cleaned).toBe('foo();\nbar();');
  });

  test('should handle single line with prefix', () => {
    const input = '42:ab | return true;';
    const { cleaned, stripped } = stripHashlinePrefixes(input);
    expect(stripped).toBe(true);
    expect(cleaned).toBe('return true;');
  });
});
