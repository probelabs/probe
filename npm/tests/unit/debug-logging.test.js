import { describe, test, expect, jest, beforeEach, afterEach } from '@jest/globals';
import { debugTruncate, debugLogToolResults } from '../../src/agent/ProbeAgent.js';

describe('Debug logging helpers', () => {
  describe('debugTruncate', () => {
    test('should return short strings unchanged', () => {
      expect(debugTruncate('hello')).toBe('hello');
      expect(debugTruncate('')).toBe('');
      expect(debugTruncate('a'.repeat(200))).toBe('a'.repeat(200));
    });

    test('should truncate long strings showing first and last portions', () => {
      const long = 'A'.repeat(100) + 'B'.repeat(100) + 'C'.repeat(100);
      const result = debugTruncate(long);
      expect(result).toContain('A'.repeat(100));
      expect(result).toContain('C'.repeat(100));
      expect(result).toContain('300 chars');
      expect(result.length).toBeLessThan(long.length);
    });

    test('should respect custom limit', () => {
      const s = 'abcdefghij'; // 10 chars
      const result = debugTruncate(s, 6);
      // half = 3, so first 3 + last 3
      expect(result).toContain('abc');
      expect(result).toContain('hij');
      expect(result).toContain('10 chars');
    });

    test('should not truncate at exactly the limit', () => {
      const s = 'a'.repeat(200);
      expect(debugTruncate(s, 200)).toBe(s);
    });
  });

  describe('debugLogToolResults', () => {
    let consoleSpy;

    beforeEach(() => {
      consoleSpy = jest.spyOn(console, 'log').mockImplementation(() => {});
    });

    afterEach(() => {
      consoleSpy.mockRestore();
    });

    test('should not log anything for empty or null results', () => {
      debugLogToolResults(null);
      debugLogToolResults([]);
      debugLogToolResults(undefined);
      expect(consoleSpy).not.toHaveBeenCalled();
    });

    test('should log tool name and args', () => {
      debugLogToolResults([{
        toolName: 'search',
        args: { query: 'test query', path: '/src' },
        result: 'found 3 results'
      }]);
      expect(consoleSpy).toHaveBeenCalledTimes(1);
      const output = consoleSpy.mock.calls[0][0];
      expect(output).toContain('tool: search');
      expect(output).toContain('test query');
      expect(output).toContain('found 3 results');
    });

    test('should log multiple tool results', () => {
      debugLogToolResults([
        { toolName: 'search', args: { query: 'a' }, result: 'res1' },
        { toolName: 'extract', args: { file: 'b.js' }, result: 'res2' }
      ]);
      expect(consoleSpy).toHaveBeenCalledTimes(2);
      expect(consoleSpy.mock.calls[0][0]).toContain('tool: search');
      expect(consoleSpy.mock.calls[1][0]).toContain('tool: extract');
    });

    test('should truncate long args and results', () => {
      const longArg = 'x'.repeat(500);
      debugLogToolResults([{
        toolName: 'search',
        args: { query: longArg },
        result: 'y'.repeat(500)
      }]);
      const output = consoleSpy.mock.calls[0][0];
      expect(output).toContain('500 chars');
      expect(output.length).toBeLessThan(1000);
    });

    test('should handle non-string results (objects)', () => {
      debugLogToolResults([{
        toolName: 'search',
        args: {},
        result: { files: ['a.js', 'b.js'] }
      }]);
      const output = consoleSpy.mock.calls[0][0];
      expect(output).toContain('a.js');
      expect(output).toContain('b.js');
    });
  });
});
