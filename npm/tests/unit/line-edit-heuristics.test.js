/**
 * Tests for lineEditHeuristics.js module.
 */

import { describe, test, expect } from '@jest/globals';
import {
  stripEchoedBoundaries,
  restoreIndentation,
  cleanNewString
} from '../../src/tools/lineEditHeuristics.js';

describe('stripEchoedBoundaries', () => {
  const fileLines = [
    'function foo() {',    // line 1
    '  const x = 1;',      // line 2
    '  const y = 2;',      // line 3
    '  return x + y;',     // line 4
    '}'                    // line 5
  ];

  describe('replace mode', () => {
    test('should strip echoed line before range', () => {
      // LLM echoes line 1 at the start of replacement for lines 2-3
      const newStr = 'function foo() {\n  const x = 10;\n  const y = 20;';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLines, 2, 3);
      expect(result).toBe('  const x = 10;\n  const y = 20;');
      expect(modifications).toContain('stripped echoed line before range');
    });

    test('should strip echoed line after range', () => {
      // LLM echoes line 4 at the end of replacement for lines 2-3
      const newStr = '  const x = 10;\n  const y = 20;\n  return x + y;';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLines, 2, 3);
      expect(result).toBe('  const x = 10;\n  const y = 20;');
      expect(modifications).toContain('stripped echoed line after range');
    });

    test('should strip both echoed boundary lines', () => {
      // Replacing line 2 only. Line before (1) = 'function foo() {', line after (3) = '  const y = 2;'
      const newStr = 'function foo() {\n  const x = 10;\n  const y = 2;';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLines, 2, 2);
      expect(result).toBe('  const x = 10;');
      expect(modifications.length).toBe(2);
    });

    test('should not strip when first line is different', () => {
      const newStr = '  const x = 10;\n  const y = 20;';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLines, 2, 3);
      expect(result).toBe(newStr);
      expect(modifications.length).toBe(0);
    });

    test('should not strip blank lines (coincidental match)', () => {
      const fileLinesWithBlank = ['', '  code here', ''];
      const newStr = '\n  new code\n';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLinesWithBlank, 2, 2);
      expect(result).toBe(newStr);
      expect(modifications.length).toBe(0);
    });

    test('should handle first line of file (no line before)', () => {
      const newStr = '  new first line';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLines, 1, 1);
      expect(result).toBe(newStr);
      expect(modifications.length).toBe(0);
    });

    test('should handle last line of file (no line after)', () => {
      const newStr = 'new last line';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLines, 5, 5);
      expect(result).toBe(newStr);
      expect(modifications.length).toBe(0);
    });

    test('should match with different indentation (.trim() comparison)', () => {
      const newStr = '  function foo() {\n  const x = 10;';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLines, 2, 2);
      expect(result).toBe('  const x = 10;');
      expect(modifications).toContain('stripped echoed line before range');
    });
  });

  describe('insert-after mode', () => {
    test('should strip echoed anchor line', () => {
      // LLM echoes the anchor line (line 3) at the start when inserting after line 3
      const newStr = '  const y = 2;\n  const z = 3;';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLines, 3, 3, 'after');
      expect(result).toBe('  const z = 3;');
      expect(modifications).toContain('stripped echoed anchor line (insert-after)');
    });

    test('should not strip when anchor is not echoed', () => {
      const newStr = '  const z = 3;';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLines, 3, 3, 'after');
      expect(result).toBe(newStr);
      expect(modifications.length).toBe(0);
    });
  });

  describe('insert-before mode', () => {
    test('should strip echoed anchor line at end', () => {
      // LLM echoes the anchor line (line 3) at the end when inserting before line 3
      const newStr = '  const z = 0;\n  const y = 2;';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLines, 3, 3, 'before');
      expect(result).toBe('  const z = 0;');
      expect(modifications).toContain('stripped echoed anchor line (insert-before)');
    });

    test('should not strip when anchor is not echoed', () => {
      const newStr = '  const z = 0;';
      const { result, modifications } = stripEchoedBoundaries(newStr, fileLines, 3, 3, 'before');
      expect(result).toBe(newStr);
      expect(modifications.length).toBe(0);
    });
  });

  test('should handle empty new_string', () => {
    const { result, modifications } = stripEchoedBoundaries('', fileLines, 2, 3);
    expect(result).toBe('');
    expect(modifications.length).toBe(0);
  });
});

describe('restoreIndentation', () => {
  test('should reindent when base indent differs', () => {
    const newStr = 'const x = 10;\nconst y = 20;';
    const originalLines = ['  const x = 1;', '  const y = 2;'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    expect(result).toBe('  const x = 10;\n  const y = 20;');
    expect(modifications.length).toBe(1);
    expect(modifications[0]).toContain('reindented');
  });

  test('should not reindent when indentation matches', () => {
    const newStr = '  const x = 10;\n  const y = 20;';
    const originalLines = ['  const x = 1;', '  const y = 2;'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    expect(result).toBe(newStr);
    expect(modifications.length).toBe(0);
  });

  test('should handle deeper-to-shallower indent change', () => {
    const newStr = '      deeply();\n      nested();';
    const originalLines = ['  shallow();', '  code();'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    expect(result).toBe('  deeply();\n  nested();');
    expect(modifications.length).toBe(1);
  });

  test('should handle empty newStr', () => {
    const { result, modifications } = restoreIndentation('', ['  code();']);
    expect(result).toBe('');
    expect(modifications.length).toBe(0);
  });

  test('should handle empty originalLines', () => {
    const { result, modifications } = restoreIndentation('code();', []);
    expect(result).toBe('code();');
    expect(modifications.length).toBe(0);
  });

  test('should handle null newStr', () => {
    const { result, modifications } = restoreIndentation(null, ['  code();']);
    expect(result).toBe('');
    expect(modifications.length).toBe(0);
  });

  test('should preserve relative indentation within block', () => {
    const newStr = 'function foo() {\n  return 1;\n}';
    const originalLines = ['    function bar() {', '      return 2;', '    }'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    expect(result).toBe('    function foo() {\n      return 1;\n    }');
    expect(modifications.length).toBe(1);
  });

  test('should reject reindent when tab indent differs by >1 level (issue #507)', () => {
    // New code at 4-tab indent, original at 1-tab → diff = 3 tabs > 1 allowed
    const newStr = '\t\t\t\tends := i + batchSize';
    const originalLines = ['\tends := i + batchSize'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    // Should NOT reindent — diff too large, likely wrong scope
    expect(result).toBe(newStr);
    expect(modifications.length).toBe(0);
  });

  test('should allow reindent when tab indent differs by exactly 1 level', () => {
    const newStr = '\t\treturn x;';
    const originalLines = ['\treturn x;'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    // Diff = 1 tab, within tolerance → should reindent
    expect(result).toBe('\treturn x;');
    expect(modifications.length).toBe(1);
  });

  test('should reject reindent when space indent differs by >4 chars', () => {
    // 8-space indent original, 0-space new → diff = 8 > 4
    const newStr = 'return x;';
    const originalLines = ['        return x;'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    expect(result).toBe(newStr);
    expect(modifications.length).toBe(0);
  });

  test('should allow reindent at exactly 4-space boundary', () => {
    // 4-space original, 0-space new → diff = 4, exactly at max
    const newStr = 'return x;\nreturn y;';
    const originalLines = ['    return x;', '    return y;'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    expect(result).toBe('    return x;\n    return y;');
    expect(modifications.length).toBe(1);
  });

  test('should reject reindent at 5-space diff (just over boundary)', () => {
    // 5-space original, 0-space new → diff = 5 > 4
    const newStr = 'return x;';
    const originalLines = ['     return x;'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    expect(result).toBe(newStr);
    expect(modifications.length).toBe(0);
  });

  test('should allow zero indent to 1-tab reindent', () => {
    const newStr = 'return x;';
    const originalLines = ['\treturn x;'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    expect(result).toBe('\treturn x;');
    expect(modifications.length).toBe(1);
  });

  test('should reject zero indent to 2-tab reindent', () => {
    const newStr = 'return x;';
    const originalLines = ['\t\treturn x;'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    expect(result).toBe(newStr);
    expect(modifications.length).toBe(0);
  });

  test('should reject tab reindent for multi-line Go block (issue #507)', () => {
    // Realistic scenario: replacement code at 4-tab indent, original at 1-tab
    const newStr = '\t\t\t\tends := i + batchSize\n\t\t\t\tif ends > len(keys) {\n\t\t\t\t\tends = len(keys)\n\t\t\t\t}';
    const originalLines = [
      '\tends := i + batchSize',
      '\tif ends > len(keys) {',
      '\t\tends = len(keys)',
      '\t}',
    ];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    // Diff = 3 tabs > 1 max — should NOT reindent
    expect(result).toBe(newStr);
    expect(modifications.length).toBe(0);
  });

  test('should allow 2-space to 4-space reindent (diff=2, within space limit)', () => {
    const newStr = '  if (x) {\n    return 1;\n  }';
    const originalLines = ['    if (x) {', '      return 1;', '    }'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    expect(result).toBe('    if (x) {\n      return 1;\n    }');
    expect(modifications.length).toBe(1);
  });

  test('should handle both sides being tab-indented at different levels', () => {
    // 3-tab new vs 2-tab original → diff = 1 tab, allowed
    const newStr = '\t\t\treturn x;';
    const originalLines = ['\t\treturn x;'];
    const { result, modifications } = restoreIndentation(newStr, originalLines);
    expect(result).toBe('\t\treturn x;');
    expect(modifications.length).toBe(1);
  });
});

describe('cleanNewString', () => {
  const fileLines = [
    'function foo() {',    // line 1
    '  const x = 1;',      // line 2
    '  const y = 2;',      // line 3
    '  return x + y;',     // line 4
    '}'                    // line 5
  ];

  test('should run full pipeline for replace mode', () => {
    // Contains prefix + echoed boundary + wrong indent
    const newStr = '2:ab |   const x = 10;\n3:cd |   const y = 20;';
    const { cleaned, modifications } = cleanNewString(newStr, fileLines, 2, 3);
    expect(modifications).toContain('stripped line-number prefixes');
    // Should have cleaned prefixes
    expect(cleaned).not.toContain('|');
  });

  test('should handle clean input with no issues', () => {
    const newStr = '  const x = 10;\n  const y = 20;';
    const { cleaned, modifications } = cleanNewString(newStr, fileLines, 2, 3);
    expect(cleaned).toBe(newStr);
    expect(modifications.length).toBe(0);
  });

  test('should handle insert-after mode', () => {
    const newStr = '  const y = 2;\n  const z = 3;';
    const { cleaned, modifications } = cleanNewString(newStr, fileLines, 3, 3, 'after');
    expect(modifications).toContain('stripped echoed anchor line (insert-after)');
    expect(cleaned).toBe('  const z = 3;');
  });

  test('should handle insert-before mode', () => {
    const newStr = '  const z = 0;\n  const y = 2;';
    const { cleaned, modifications } = cleanNewString(newStr, fileLines, 3, 3, 'before');
    expect(modifications).toContain('stripped echoed anchor line (insert-before)');
    expect(cleaned).toBe('  const z = 0;');
  });

  test('should skip indentation restoration for insert mode', () => {
    const newStr = 'const z = 3;';  // no indent, but original has 2-space indent
    const { cleaned, modifications } = cleanNewString(newStr, fileLines, 3, 3, 'after');
    // Should NOT reindent for insert mode
    expect(cleaned).toBe('const z = 3;');
    expect(modifications.filter(m => m.includes('reindent')).length).toBe(0);
  });

  test('should handle null input', () => {
    const { cleaned, modifications } = cleanNewString(null, fileLines, 2, 3);
    expect(cleaned).toBe('');
    expect(modifications.length).toBe(0);
  });

  test('should handle empty string input', () => {
    const { cleaned, modifications } = cleanNewString('', fileLines, 2, 3);
    expect(cleaned).toBe('');
    expect(modifications.length).toBe(0);
  });
});
