/**
 * Tests for FileTracker — per-session content-aware file state tracking
 */

import { describe, test, expect, beforeEach, afterEach, jest } from '@jest/globals';
import { FileTracker, computeContentHash } from '../../src/tools/fileTracker.js';
import { promises as fs } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';
import { randomUUID } from 'crypto';
import { existsSync } from 'fs';

describe('FileTracker', () => {
  let testDir;
  let tracker;

  beforeEach(async () => {
    testDir = join(tmpdir(), `probe-tracker-test-${randomUUID()}`);
    await fs.mkdir(testDir, { recursive: true });
    tracker = new FileTracker({ debug: false });
  });

  afterEach(async () => {
    if (existsSync(testDir)) {
      await fs.rm(testDir, { recursive: true, force: true });
    }
  });

  // ─── computeContentHash ───

  describe('computeContentHash', () => {
    test('should produce same hash for same content', () => {
      const hash1 = computeContentHash('function foo() { return 1; }');
      const hash2 = computeContentHash('function foo() { return 1; }');
      expect(hash1).toBe(hash2);
    });

    test('should produce different hash for different content', () => {
      const hash1 = computeContentHash('function foo() { return 1; }');
      const hash2 = computeContentHash('function foo() { return 2; }');
      expect(hash1).not.toBe(hash2);
    });

    test('should return 16-char hex string', () => {
      const hash = computeContentHash('some code');
      expect(hash).toMatch(/^[0-9a-f]{16}$/);
    });

    test('should normalize trailing whitespace', () => {
      const hash1 = computeContentHash('line1  \nline2\t\nline3');
      const hash2 = computeContentHash('line1\nline2\nline3');
      expect(hash1).toBe(hash2);
    });

    test('should handle empty content', () => {
      const hash = computeContentHash('');
      expect(hash).toMatch(/^[0-9a-f]{16}$/);
    });

    test('should handle null/undefined', () => {
      const hash = computeContentHash(null);
      expect(hash).toMatch(/^[0-9a-f]{16}$/);
    });
  });

  // ─── markFileSeen / isFileSeen ───

  describe('markFileSeen and isFileSeen', () => {
    test('should mark a file as seen', () => {
      tracker.markFileSeen('/path/to/file.js');
      expect(tracker.isFileSeen('/path/to/file.js')).toBe(true);
    });

    test('should report unseen files as not seen', () => {
      expect(tracker.isFileSeen('/some/random/path.js')).toBe(false);
    });

    test('isTracked should alias isFileSeen', () => {
      tracker.markFileSeen('/path/to/file.js');
      expect(tracker.isTracked('/path/to/file.js')).toBe(true);
      expect(tracker.isTracked('/other/file.js')).toBe(false);
    });
  });

  // ─── trackSymbolContent / getSymbolRecord ───

  describe('trackSymbolContent and getSymbolRecord', () => {
    test('should store and retrieve symbol content hash', () => {
      const code = 'function calculateTotal(items) {\n  return items.reduce((s, i) => s + i, 0);\n}';
      tracker.trackSymbolContent('/path/file.js', 'calculateTotal', code, 10, 12);

      const record = tracker.getSymbolRecord('/path/file.js', 'calculateTotal');
      expect(record).not.toBeNull();
      expect(record.contentHash).toBe(computeContentHash(code));
      expect(record.startLine).toBe(10);
      expect(record.endLine).toBe(12);
      expect(record.symbolName).toBe('calculateTotal');
      expect(record.source).toBe('extract');
    });

    test('should return null for unknown symbol', () => {
      expect(tracker.getSymbolRecord('/path/file.js', 'unknown')).toBeNull();
    });

    test('should overwrite previous record for same symbol', () => {
      tracker.trackSymbolContent('/path/file.js', 'foo', 'v1', 1, 3);
      tracker.trackSymbolContent('/path/file.js', 'foo', 'v2', 5, 8);

      const record = tracker.getSymbolRecord('/path/file.js', 'foo');
      expect(record.contentHash).toBe(computeContentHash('v2'));
      expect(record.startLine).toBe(5);
    });
  });

  // ─── checkBeforeEdit ───

  describe('checkBeforeEdit — untracked', () => {
    test('should return untracked for files never seen', () => {
      const result = tracker.checkBeforeEdit('/path/to/unread.js');
      expect(result.ok).toBe(false);
      expect(result.reason).toBe('untracked');
      expect(result.message).toContain('not been read yet');
    });
  });

  describe('checkBeforeEdit — seen', () => {
    test('should return ok for seen file', () => {
      tracker.markFileSeen('/path/to/file.js');
      const result = tracker.checkBeforeEdit('/path/to/file.js');
      expect(result.ok).toBe(true);
    });

    test('should return ok regardless of file modification', async () => {
      // This is the key behavioral change: seen-check only, no mtime tracking
      const filePath = join(testDir, 'modified.js');
      await fs.writeFile(filePath, 'original content');
      tracker.markFileSeen(filePath);

      // Modify the file externally
      await fs.writeFile(filePath, 'modified content that is different length');

      // Should still return ok (seen-check only)
      const result = tracker.checkBeforeEdit(filePath);
      expect(result.ok).toBe(true);
    });
  });

  // ─── checkSymbolContent ───

  describe('checkSymbolContent — match', () => {
    test('should return ok when content hash matches', () => {
      const code = 'function foo() { return 42; }';
      tracker.trackSymbolContent('/path/file.js', 'foo', code, 10, 12);

      const result = tracker.checkSymbolContent('/path/file.js', 'foo', code);
      expect(result.ok).toBe(true);
    });

    test('should match despite trailing whitespace differences', () => {
      const original = 'function foo() { return 42; }  \nconst x = 1;\t';
      const current = 'function foo() { return 42; }\nconst x = 1;';
      tracker.trackSymbolContent('/path/file.js', 'foo', original, 10, 12);

      const result = tracker.checkSymbolContent('/path/file.js', 'foo', current);
      expect(result.ok).toBe(true);
    });
  });

  describe('checkSymbolContent — mismatch', () => {
    test('should return stale when content changed', () => {
      tracker.trackSymbolContent('/path/file.js', 'foo', 'function foo() { return 1; }', 10, 12);

      const result = tracker.checkSymbolContent('/path/file.js', 'foo', 'function foo() { return 2; }');
      expect(result.ok).toBe(false);
      expect(result.reason).toBe('stale');
      expect(result.message).toContain('has changed');
    });
  });

  describe('checkSymbolContent — no record', () => {
    test('should return ok when no record exists (first edit)', () => {
      const result = tracker.checkSymbolContent('/path/file.js', 'unknown', 'some code');
      expect(result.ok).toBe(true);
    });
  });

  // ─── File changed but symbol unchanged ───

  describe('file changed but symbol unchanged', () => {
    test('should allow edit when target symbol content is the same', () => {
      // This is the KEY benefit over mtime-based tracking
      const symbolCode = 'function processOrder(order) {\n  return order.total;\n}';
      tracker.trackSymbolContent('/path/file.js', 'processOrder', symbolCode, 40, 42);
      tracker.markFileSeen('/path/file.js');

      // File was modified (other parts changed), but this symbol is unchanged
      // checkBeforeEdit returns ok (file is seen)
      const editCheck = tracker.checkBeforeEdit('/path/file.js');
      expect(editCheck.ok).toBe(true);

      // Symbol content check also passes (content unchanged)
      const symbolCheck = tracker.checkSymbolContent('/path/file.js', 'processOrder', symbolCode);
      expect(symbolCheck.ok).toBe(true);
    });
  });

  // ─── trackSymbolAfterWrite ───

  describe('trackSymbolAfterWrite', () => {
    test('should update hash so subsequent check passes', () => {
      const oldCode = 'function foo() { return 1; }';
      const newCode = 'function foo() { return 2; }';
      tracker.trackSymbolContent('/path/file.js', 'foo', oldCode, 10, 12);

      // Simulate successful edit — update with new content
      tracker.trackSymbolAfterWrite('/path/file.js', 'foo', newCode, 10, 12);

      // Check with new code should pass
      const result = tracker.checkSymbolContent('/path/file.js', 'foo', newCode);
      expect(result.ok).toBe(true);
    });

    test('should handle chained edits to same symbol', () => {
      // Edit 1
      const v1 = 'function foo() { return 1; }';
      tracker.trackSymbolContent('/path/file.js', 'foo', v1, 10, 12);
      tracker.trackSymbolAfterWrite('/path/file.js', 'foo', v1, 10, 12);

      // Edit 2
      const v2 = 'function foo() { return 2; }';
      tracker.trackSymbolAfterWrite('/path/file.js', 'foo', v2, 10, 12);
      expect(tracker.checkSymbolContent('/path/file.js', 'foo', v2).ok).toBe(true);

      // Edit 3
      const v3 = 'function foo() { return 3; }';
      tracker.trackSymbolAfterWrite('/path/file.js', 'foo', v3, 10, 13);
      expect(tracker.checkSymbolContent('/path/file.js', 'foo', v3).ok).toBe(true);
    });
  });

  // ─── invalidateFileRecords ───

  describe('invalidateFileRecords', () => {
    test('should remove all content records for a file', () => {
      tracker.trackSymbolContent('/path/file.js', 'foo', 'code1', 1, 3);
      tracker.trackSymbolContent('/path/file.js', 'bar', 'code2', 5, 8);
      tracker.trackSymbolContent('/path/other.js', 'baz', 'code3', 1, 2);

      tracker.invalidateFileRecords('/path/file.js');

      expect(tracker.getSymbolRecord('/path/file.js', 'foo')).toBeNull();
      expect(tracker.getSymbolRecord('/path/file.js', 'bar')).toBeNull();
      // Other file should not be affected
      expect(tracker.getSymbolRecord('/path/other.js', 'baz')).not.toBeNull();
    });
  });

  // ─── trackFileAfterWrite ───

  describe('trackFileAfterWrite', () => {
    test('should mark file as seen and invalidate records', async () => {
      tracker.trackSymbolContent('/path/file.js', 'foo', 'code', 1, 3);
      tracker.markFileSeen('/path/file.js');

      await tracker.trackFileAfterWrite('/path/file.js');

      // File should still be seen
      expect(tracker.isFileSeen('/path/file.js')).toBe(true);
      // But content records should be invalidated
      expect(tracker.getSymbolRecord('/path/file.js', 'foo')).toBeNull();
    });
  });

  // ─── trackFilesFromExtract ───

  describe('trackFilesFromExtract', () => {
    test('should mark files as seen from simple targets', async () => {
      const file1 = join(testDir, 'a.js');
      const file2 = join(testDir, 'b.js');
      await fs.writeFile(file1, 'a');
      await fs.writeFile(file2, 'b');

      await tracker.trackFilesFromExtract([file1, file2], testDir);

      expect(tracker.isFileSeen(file1)).toBe(true);
      expect(tracker.isFileSeen(file2)).toBe(true);
    });

    test('should mark files as seen from symbol targets', async () => {
      const filePath = join(testDir, 'src.js');
      await fs.writeFile(filePath, 'function foo() {}');

      // findSymbol will be called but may fail (no probe binary in tests)
      // The file should still be marked as seen
      await tracker.trackFilesFromExtract([filePath + '#foo'], testDir);

      expect(tracker.isFileSeen(filePath)).toBe(true);
    });

    test('should mark files as seen from line-range targets', async () => {
      const filePath = join(testDir, 'src.js');
      await fs.writeFile(filePath, 'line1\nline2');

      await tracker.trackFilesFromExtract([filePath + ':1-10'], testDir);

      expect(tracker.isFileSeen(filePath)).toBe(true);
    });

    test('should resolve relative paths against cwd', async () => {
      const filePath = join(testDir, 'rel.js');
      await fs.writeFile(filePath, 'relative');

      await tracker.trackFilesFromExtract(['rel.js'], testDir);

      expect(tracker.isFileSeen(filePath)).toBe(true);
    });

    test('should deduplicate file-seen marking', async () => {
      const filePath = join(testDir, 'dedup.js');
      await fs.writeFile(filePath, 'code');

      // Multiple targets pointing to same file
      await tracker.trackFilesFromExtract([
        filePath + '#foo',
        filePath + '#bar',
        filePath + ':10'
      ], testDir);

      expect(tracker.isFileSeen(filePath)).toBe(true);
    });

    test('should mark non-existent files as seen', async () => {
      // Even non-existent files should be marked as seen (they were in the targets)
      // The file might be created later (e.g., create tool)
      await tracker.trackFilesFromExtract(['/nonexistent/file.js'], testDir);

      expect(tracker.isFileSeen('/nonexistent/file.js')).toBe(true);
    });
  });

  // ─── trackFilesFromOutput ───

  describe('trackFilesFromOutput', () => {
    test('should parse "File: path" headers', async () => {
      const filePath = join(testDir, 'result.js');
      await fs.writeFile(filePath, 'content');

      const output = `File: ${filePath}\n  1 | content\n`;

      await tracker.trackFilesFromOutput(output, testDir);

      expect(tracker.isFileSeen(filePath)).toBe(true);
    });

    test('should parse "--- path ---" separators', async () => {
      const filePath = join(testDir, 'sep.js');
      await fs.writeFile(filePath, 'code');

      const output = `--- ${filePath} ---\n  1 | code\n`;

      await tracker.trackFilesFromOutput(output, testDir);

      expect(tracker.isFileSeen(filePath)).toBe(true);
    });

    test('should handle multiple files in output', async () => {
      const file1 = join(testDir, 'one.js');
      const file2 = join(testDir, 'two.js');
      await fs.writeFile(file1, 'a');
      await fs.writeFile(file2, 'b');

      const output = `File: ${file1}\n  1 | a\n\nFile: ${file2}\n  1 | b\n`;

      await tracker.trackFilesFromOutput(output, testDir);

      expect(tracker.isFileSeen(file1)).toBe(true);
      expect(tracker.isFileSeen(file2)).toBe(true);
    });

    test('should resolve relative paths from output', async () => {
      const filePath = join(testDir, 'relative.js');
      await fs.writeFile(filePath, 'rel');

      const output = `File: relative.js\n  1 | rel\n`;

      await tracker.trackFilesFromOutput(output, testDir);

      expect(tracker.isFileSeen(filePath)).toBe(true);
    });

    test('should skip metadata lines', async () => {
      const output = `Results found: 5\nPage 1 of 2\n`;

      await tracker.trackFilesFromOutput(output, testDir);

      // No files should be seen
      expect(tracker._seenFiles.size).toBe(0);
    });

    test('should handle empty output', async () => {
      await tracker.trackFilesFromOutput('', testDir);
      expect(tracker._seenFiles.size).toBe(0);
    });
  });

  // ─── clear ───

  describe('clear', () => {
    test('should reset all tracking', () => {
      tracker.markFileSeen('/path/file.js');
      tracker.trackSymbolContent('/path/file.js', 'foo', 'code', 1, 3);

      tracker.clear();

      expect(tracker.isFileSeen('/path/file.js')).toBe(false);
      expect(tracker.getSymbolRecord('/path/file.js', 'foo')).toBeNull();
      expect(tracker._seenFiles.size).toBe(0);
      expect(tracker._contentRecords.size).toBe(0);
    });
  });

  // ─── debug logging ───

  describe('debug mode', () => {
    test('should log when debug is enabled', () => {
      const debugTracker = new FileTracker({ debug: true });

      const errors = [];
      const origError = console.error;
      console.error = (...args) => errors.push(args.join(' '));

      try {
        debugTracker.markFileSeen('/path/file.js');
        expect(errors.some(e => e.includes('[FileTracker]'))).toBe(true);
      } finally {
        console.error = origError;
      }
    });

    test('should log symbol tracking when debug is enabled', () => {
      const debugTracker = new FileTracker({ debug: true });

      const errors = [];
      const origError = console.error;
      console.error = (...args) => errors.push(args.join(' '));

      try {
        debugTracker.trackSymbolContent('/path/file.js', 'foo', 'code', 1, 3);
        expect(errors.some(e => e.includes('Tracked symbol'))).toBe(true);
      } finally {
        console.error = origError;
      }
    });
  });
});
