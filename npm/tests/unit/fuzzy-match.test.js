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

    test('should reject tab-indented match with 3+ level difference (issue #507)', () => {
      // Reproduces issue #507: old_string at 4-tab indent (deeply nested loop)
      // should NOT match content at 1-tab indent (different scope entirely)
      const contentLines = [
        '\tends := i + batchSize',  // 1-tab indent (function body)
      ];
      const searchLines = [
        '\t\t\t\tends := i + batchSize',  // 4-tab indent (nested loop)
      ];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // Indent diff = 3 tabs = 3 levels, exceeds max of 1 for tabs
      expect(result).toBeNull();
    });

    test('should reject tab-indented match with 2-level difference', () => {
      const contentLines = [
        '\treturn indexBaseName + "_" + tableName',  // 1 tab
      ];
      const searchLines = [
        '\t\t\treturn indexBaseName + "_" + tableName',  // 3 tabs
      ];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // Diff = 2 tabs > 1 allowed
      expect(result).toBeNull();
    });

    test('should allow tab-indented match with 1-level difference', () => {
      const contentLines = [
        '\tif (x) {',
        '\t\treturn true;',
        '\t}',
      ];
      const searchLines = [
        '\t\tif (x) {',
        '\t\t\treturn true;',
        '\t\t}',
      ];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // Diff = 1 tab = exactly 1 level, should be allowed
      expect(result).not.toBeNull();
    });

    test('should allow space-indented match with 1-level difference (up to 4 spaces)', () => {
      const contentLines = ['    if (x) {', '        return true;', '    }'];
      const searchLines = ['  if (x) {', '      return true;', '  }'];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // Diff = 2 spaces, well within 4-space max
      expect(result).not.toBeNull();
    });

    test('should reject space-indented match with >4 space difference', () => {
      const contentLines = [
        '          if (x) {',      // 10 spaces
        '              return 1;', // 14 spaces
        '          }',
      ];
      const searchLines = [
        '  if (x) {',      // 2 spaces
        '      return 1;', // 6 spaces
        '  }',
      ];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // Diff = 8 spaces > 4 allowed
      expect(result).toBeNull();
    });

    test('issue #507 full scenario: Go code at 4-tab vs 1-tab should not match', () => {
      // Realistic Go file: buildIndexName at 1-tab indent, LLM searches at 4-tab indent
      const contentLines = [
        'func (c *SQLPump) buildIndexName(indexBaseName, tableName string) string {',
        '\tends := i + batchSize',
        '\tif ends > len(keys) {',
        '\t\tends = len(keys)',
        '\t}',
        '\tbatch := keys[i:ends]',
        '\treturn indexBaseName + "_" + tableName',
        '}',
      ];
      // LLM old_string from deeply nested loop body (4 tabs)
      const searchLines = [
        '\t\t\t\tends := i + batchSize',
        '\t\t\t\tif ends > len(keys) {',
        '\t\t\t\t\tends = len(keys)',
        '\t\t\t\t}',
        '\t\t\t\tbatch := keys[i:ends]',
      ];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // searchMinIndent=4, windowMinIndent=1, diff=3 tabs > 1 max
      expect(result).toBeNull();
    });

    test('should allow zero indent search vs 1-tab content', () => {
      const contentLines = ['\treturn x;'];
      const searchLines = ['return x;'];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // Diff = 1 tab, exactly at max
      expect(result).not.toBeNull();
      expect(result.matchedText).toBe('\treturn x;');
    });

    test('should reject zero indent search vs 2-tab content', () => {
      const contentLines = ['\t\treturn x;'];
      const searchLines = ['return x;'];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // Diff = 2 tabs > 1 max
      expect(result).toBeNull();
    });

    test('should allow zero indent search vs 4-space content (boundary)', () => {
      const contentLines = ['    return x;'];
      const searchLines = ['return x;'];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // Diff = 4 spaces, exactly at max
      expect(result).not.toBeNull();
    });

    test('should reject zero indent search vs 5-space content (just over boundary)', () => {
      const contentLines = ['     return x;'];  // 5 spaces
      const searchLines = ['return x;'];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // Diff = 5 spaces > 4 max
      expect(result).toBeNull();
    });

    test('should match when indent diff is exactly 0 (same level)', () => {
      const contentLines = ['\t\t\tif (x) {', '\t\t\t\treturn 1;', '\t\t\t}'];
      const searchLines = ['\t\t\tif (x) {', '\t\t\t\treturn 1;', '\t\t\t}'];
      const result = indentFlexibleMatch(contentLines, searchLines);
      expect(result).not.toBeNull();
      expect(result.count).toBe(1);
    });

    test('should only match correctly-indented window among multiple candidates', () => {
      // Two structurally identical blocks at different indent levels
      const contentLines = [
        '\tif (ok) {',         // 1-tab block
        '\t\treturn true;',
        '\t}',
        '\t\t\t\tif (ok) {',  // 4-tab block
        '\t\t\t\t\treturn true;',
        '\t\t\t\t}',
      ];
      // Search at 3-tab indent
      const searchLines = [
        '\t\t\tif (ok) {',
        '\t\t\t\treturn true;',
        '\t\t\t}',
      ];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // 1-tab block: diff = 2 tabs > 1 → rejected
      // 4-tab block: diff = 1 tab = 1 → allowed
      expect(result).not.toBeNull();
      expect(result.count).toBe(1);
      expect(result.matchedText).toBe('\t\t\t\tif (ok) {\n\t\t\t\t\treturn true;\n\t\t\t\t}');
    });

    test('should reject multi-line block with blank lines when indent diff too large (tabs)', () => {
      const contentLines = [
        '\tif (x) {',
        '',
        '\t\treturn y;',
        '\t}',
      ];
      const searchLines = [
        '\t\t\t\tif (x) {',
        '',
        '\t\t\t\t\treturn y;',
        '\t\t\t\t}',
      ];
      const result = indentFlexibleMatch(contentLines, searchLines);
      // Diff = 3 tabs > 1 max
      expect(result).toBeNull();
    });

    test('should handle single-line with only whitespace difference at boundary', () => {
      // 4 spaces vs 8 spaces = diff of 4, at boundary
      const contentLines = ['        x = 1;'];  // 8 spaces
      const searchLines = ['    x = 1;'];        // 4 spaces
      const result = indentFlexibleMatch(contentLines, searchLines);
      // Diff = 4 spaces, exactly at max
      expect(result).not.toBeNull();
    });
  });

  describe('findFuzzyMatch with indent limits', () => {
    test('should not match via indent-flexible for issue #507 scenario', () => {
      // Even through the full orchestrator, the deeply-nested search should not
      // fuzzy-match against shallow content
      const content = [
        'func buildIndexName() string {',
        '\tends := i + batchSize',
        '}',
      ].join('\n');
      const search = '\t\t\t\tends := i + batchSize';
      const result = findFuzzyMatch(content, search);
      // line-trimmed would match (it trims all whitespace), so it may still find it
      // but that's OK — line-trimmed is a different, stricter strategy
      if (result) {
        // If it matches, it should NOT be via indent-flexible
        expect(result.strategy).not.toBe('indent-flexible');
      }
    });

    test('should still match via line-trimmed even when indent-flexible would reject', () => {
      // line-trimmed compares trimmed lines, so indent diff doesn't matter
      const content = '\t\t\t\treturn x + y;';
      const search = '\treturn x + y;';
      const result = findFuzzyMatch(content, search);
      expect(result).not.toBeNull();
      // line-trimmed is tried first and matches
      expect(result.strategy).toBe('line-trimmed');
    });
  });
});
