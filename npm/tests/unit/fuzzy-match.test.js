import { describe, test, expect } from '@jest/globals';
import {
  findFuzzyMatch,
  lineTrimmedMatch,
  whitespaceNormalizedMatch,
  indentFlexibleMatch
} from '../../src/tools/fuzzyMatch.js';

describe('fuzzyMatch module', () => {
  describe('findFuzzyMatch orchestrator', () => {
    test('should return null for empty search string', () => {
      expect(findFuzzyMatch('some content', '')).toBeNull();
    });

    test('should return null for whitespace-only search string', () => {
      expect(findFuzzyMatch('some content', '   \n  ')).toBeNull();
    });

    test('should return null when no strategy matches', () => {
      expect(findFuzzyMatch('hello world', 'completely different')).toBeNull();
    });

    test('should match via line-trimmed strategy first', () => {
      const content = '    function foo() {\n      return 42;\n    }';
      const search = 'function foo() {\n  return 42;\n}';
      const result = findFuzzyMatch(content, search);
      expect(result).not.toBeNull();
      expect(result.strategy).toBe('line-trimmed');
      expect(result.matchedText).toBe(content);
    });

    test('should fall back to whitespace-normalized when line-trimmed fails', () => {
      const content = 'const x =  1;';
      const search = 'const x = 1;';
      // Line-trimmed won't help since it's single line with different internal spaces
      const result = findFuzzyMatch(content, search);
      expect(result).not.toBeNull();
      expect(result.strategy).toBe('whitespace-normalized');
    });

    test('should match code with different indentation levels via cascade', () => {
      const content = '        if (x) {\n            return true;\n        }';
      const search = '    if (x) {\n        return true;\n    }';
      // Both have same code structure, different indent base.
      // Line-trimmed matches first since it's more permissive (trims all whitespace).
      const result = findFuzzyMatch(content, search);
      expect(result).not.toBeNull();
      expect(result.strategy).toBe('line-trimmed');
      expect(result.matchedText).toBe(content);
    });

    test('should handle \\r\\n line endings', () => {
      const content = '  foo();\r\n  bar();';
      const search = 'foo();\r\nbar();';
      const result = findFuzzyMatch(content, search);
      expect(result).not.toBeNull();
    });

    test('should return count of matches', () => {
      const content = '  x = 1;\n  y = 2;\n  x = 1;';
      const search = 'x = 1;';
      const result = findFuzzyMatch(content, search);
      expect(result).not.toBeNull();
      expect(result.count).toBe(2);
    });
  });

  describe('lineTrimmedMatch', () => {
    test('should return null for empty search lines', () => {
      expect(lineTrimmedMatch(['a', 'b'], [])).toBeNull();
    });

    test('should return null for all-empty search lines', () => {
      expect(lineTrimmedMatch(['a', 'b'], ['', '  '])).toBeNull();
    });

    test('should match lines with different leading/trailing whitespace', () => {
      const contentLines = ['    function foo() {', '      return 42;', '    }'];
      const searchLines = ['function foo() {', '  return 42;', '}'];
      const result = lineTrimmedMatch(contentLines, searchLines);
      expect(result).not.toBeNull();
      expect(result.matchedText).toBe('    function foo() {\n      return 42;\n    }');
      expect(result.count).toBe(1);
    });

    test('should return original content text, not search text', () => {
      const contentLines = ['  hello', '  world'];
      const searchLines = ['hello', 'world'];
      const result = lineTrimmedMatch(contentLines, searchLines);
      expect(result.matchedText).toBe('  hello\n  world');
    });

    test('should find multiple occurrences', () => {
      const contentLines = ['  x = 1;', '  y = 2;', '  x = 1;'];
      const searchLines = ['x = 1;'];
      const result = lineTrimmedMatch(contentLines, searchLines);
      expect(result).not.toBeNull();
      expect(result.count).toBe(2);
      // Returns first match
      expect(result.matchedText).toBe('  x = 1;');
    });

    test('should not match when trimmed content differs', () => {
      const contentLines = ['  hello world'];
      const searchLines = ['hello universe'];
      const result = lineTrimmedMatch(contentLines, searchLines);
      expect(result).toBeNull();
    });
  });

  describe('whitespaceNormalizedMatch', () => {
    test('should return null for empty search', () => {
      expect(whitespaceNormalizedMatch('content', '')).toBeNull();
    });

    test('should return null for whitespace-only search', () => {
      expect(whitespaceNormalizedMatch('content', '   ')).toBeNull();
    });

    test('should match with different whitespace amounts', () => {
      const content = 'const   x  =   1;';
      const search = 'const x = 1;';
      const result = whitespaceNormalizedMatch(content, search);
      expect(result).not.toBeNull();
      expect(result.matchedText).toBe('const   x  =   1;');
    });

    test('should match tabs vs spaces', () => {
      const content = 'const\tx = 1;';
      const search = 'const x = 1;';
      const result = whitespaceNormalizedMatch(content, search);
      expect(result).not.toBeNull();
    });

    test('should preserve newlines as meaningful', () => {
      const content = 'line1\nline2';
      const search = 'line1 line2';
      // Newlines are preserved, not normalized to spaces
      const result = whitespaceNormalizedMatch(content, search);
      expect(result).toBeNull();
    });

    test('should find multiple occurrences', () => {
      const content = 'x  = 1; y = 2; x  = 1;';
      const search = 'x = 1;';
      const result = whitespaceNormalizedMatch(content, search);
      expect(result).not.toBeNull();
      expect(result.count).toBe(2);
    });
  });

  describe('indentFlexibleMatch', () => {
    test('should return null for empty search lines', () => {
      expect(indentFlexibleMatch(['a'], [])).toBeNull();
    });

    test('should return null for all-empty search lines', () => {
      expect(indentFlexibleMatch(['a'], ['', ''])).toBeNull();
    });

    test('should match code with different indentation levels', () => {
      const contentLines = ['        if (x) {', '            return true;', '        }'];
      const searchLines = ['    if (x) {', '        return true;', '    }'];
      const result = indentFlexibleMatch(contentLines, searchLines);
      expect(result).not.toBeNull();
      expect(result.matchedText).toBe('        if (x) {\n            return true;\n        }');
    });

    test('should match with uniform indent offset', () => {
      // Content indented 4 more spaces than search (uniform offset)
      const contentLines = ['      function foo() {', '          return 1;', '      }'];
      const searchLines = ['  function foo() {', '      return 1;', '  }'];
      // Both have min indent stripped: content min=6, search min=2
      // After strip: 'function foo() {', '    return 1;', '}'  for both
      const result = indentFlexibleMatch(contentLines, searchLines);
      expect(result).not.toBeNull();
      expect(result.count).toBe(1);
    });

    test('should handle blank lines in code blocks', () => {
      const contentLines = ['    if (x) {', '', '        return y;', '    }'];
      const searchLines = ['  if (x) {', '', '      return y;', '  }'];
      const result = indentFlexibleMatch(contentLines, searchLines);
      expect(result).not.toBeNull();
    });

    test('should not match when code structure differs', () => {
      const contentLines = ['    if (x) {', '        return true;', '    }'];
      const searchLines = ['  if (y) {', '    return false;', '  }'];
      const result = indentFlexibleMatch(contentLines, searchLines);
      expect(result).toBeNull();
    });

    test('should find multiple occurrences', () => {
      // Content at 4-space indent, search at 0 indent
      // min indent for content windows = 4, min indent for search = 0
      // After stripping both match exactly
      const contentLines = [
        '    if (a) {', '        return 1;', '    }',
        '    if (a) {', '        return 1;', '    }'
      ];
      const searchLines = ['if (a) {', '    return 1;', '}'];
      const result = indentFlexibleMatch(contentLines, searchLines);
      expect(result).not.toBeNull();
      expect(result.count).toBe(2);
    });
  });
});
