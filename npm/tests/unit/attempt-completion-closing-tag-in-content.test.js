import { parseXmlToolCallWithThinking } from '../../src/agent/tools.js';

/**
 * Test for fixing issue where JSON content containing "</attempt_completion>" string
 * causes the XML parser to truncate the content prematurely.
 *
 * GitHub Issue: Content with closing tag string gets truncated
 *
 * Problem: When attempt_completion contains JSON with a suggestion like:
 *   "suggestion": "Use regex: /<attempt_completion>/"
 * The parser finds this "</attempt_completion>" and treats it as the closing tag,
 * truncating the JSON and causing validation errors.
 */
describe('attempt_completion with closing tag in content', () => {
  test('should handle JSON content with escaped closing tag in string', () => {
    const xmlString = `<attempt_completion>
{
  "issues": [
    {
      "file": "npm/src/agent/contextCompactor.js",
      "line": 115,
      "ruleId": "logic/weak-pattern-matching",
      "message": "The attempt_completion detection uses simple string matching",
      "severity": "warning",
      "category": "logic",
      "suggestion": "Use more precise regex pattern to match actual XML tags: /<attempt_completion(?:[^>]*)>/ or /<\\/attempt_completion>/",
      "replacement": "if (/<attempt_completion(?:[^>]*)>|<\\/attempt_completion>/.test(content)) {"
    }
  ]
}
</attempt_completion>`;

    const result = parseXmlToolCallWithThinking(xmlString);

    expect(result).toBeDefined();
    expect(result.toolName).toBe('attempt_completion');
    expect(result.params.result).toBeDefined();

    // Verify the JSON is complete and parseable
    const jsonResult = JSON.parse(result.params.result);
    expect(jsonResult.issues).toHaveLength(1);
    expect(jsonResult.issues[0].suggestion).toContain('</attempt_completion>');
    expect(jsonResult.issues[0].replacement).toContain('</attempt_completion>');
  });

  test('should handle content with closing tag string in plain text', () => {
    const xmlString = `<attempt_completion>
The issue is in the XML parser. It searches for </attempt_completion> using indexOf(),
which finds the first occurrence rather than the actual closing tag. This causes truncation
when the content contains strings like </attempt_completion> in regex patterns or code examples.
</attempt_completion>`;

    const result = parseXmlToolCallWithThinking(xmlString);

    expect(result).toBeDefined();
    expect(result.toolName).toBe('attempt_completion');
    expect(result.params.result).toContain('</attempt_completion>');
    expect(result.params.result).toContain('This causes truncation');
  });

  test('should handle multiple occurrences of closing tag string in content', () => {
    const xmlString = `<attempt_completion>
{
  "patterns": [
    "Pattern 1: </attempt_completion>",
    "Pattern 2: <\\/attempt_completion>",
    "Pattern 3: </attempt_completion>"
  ],
  "note": "All patterns above should be preserved"
}
</attempt_completion>`;

    const result = parseXmlToolCallWithThinking(xmlString);

    expect(result).toBeDefined();
    expect(result.toolName).toBe('attempt_completion');

    const jsonResult = JSON.parse(result.params.result);
    expect(jsonResult.patterns).toHaveLength(3);
    expect(jsonResult.patterns[0]).toContain('</attempt_completion>');
    expect(jsonResult.patterns[1]).toContain('</attempt_completion>');
    expect(jsonResult.patterns[2]).toContain('</attempt_completion>');
    expect(jsonResult.note).toBe('All patterns above should be preserved');
  });

  test('should handle closing tag in markdown code block', () => {
    const xmlString = `<attempt_completion>
## Fix Instructions

The XML parser needs to be updated:

\`\`\`javascript
// Before (buggy):
const closeIndex = xmlString.indexOf('</attempt_completion>');

// After (fixed):
const closeIndex = xmlString.lastIndexOf('</attempt_completion>');
\`\`\`

This ensures we find the actual closing tag, not one in the content.
</attempt_completion>`;

    const result = parseXmlToolCallWithThinking(xmlString);

    expect(result).toBeDefined();
    expect(result.toolName).toBe('attempt_completion');
    expect(result.params.result).toContain('## Fix Instructions');
    expect(result.params.result).toContain('indexOf');
    expect(result.params.result).toContain('lastIndexOf');
    expect(result.params.result).toContain('This ensures we find the actual closing tag');
  });

  test('should handle nested XML-like content', () => {
    const xmlString = `<attempt_completion>
<analysis>
  <finding>The code uses indexOf to find </attempt_completion></finding>
  <recommendation>Use lastIndexOf or proper XML parsing</recommendation>
</analysis>
</attempt_completion>`;

    const result = parseXmlToolCallWithThinking(xmlString);

    expect(result).toBeDefined();
    expect(result.toolName).toBe('attempt_completion');
    expect(result.params.result).toContain('<analysis>');
    expect(result.params.result).toContain('<finding>');
    expect(result.params.result).toContain('</attempt_completion></finding>');
    expect(result.params.result).toContain('</recommendation>');
    expect(result.params.result).toContain('</analysis>');
  });

  test('should reproduce the exact issue from the log', () => {
    // This is the exact content from the bug report log
    const xmlString = `<attempt_completion>
{
  "issues": [
    {
      "file": "npm/src/agent/contextCompactor.js",
      "line": 115,
      "ruleId": "logic/weak-pattern-matching",
      "message": "The attempt_completion detection uses simple string matching which could match content that mentions attempt_completion but isn't actually a completion tag",
      "severity": "warning",
      "category": "logic",
      "suggestion": "Use more precise regex pattern to match actual XML tags: /<attempt_completion(?:[^>]*)>/ or /<\\/attempt_completion>/",
      "replacement": "if (/<attempt_completion(?:[^>]*)>|<\\/attempt_completion>/.test(content)) {"
    },
    {
      "file": "npm/src/agent/contextCompactor.js",
      "line": 88,
      "ruleId": "logic/weak-pattern-matching",
      "message": "Tool result detection uses simple string matching which could have false positives",
      "severity": "warning",
      "category": "logic",
      "suggestion": "Use more precise regex pattern to match actual tool_result tags: /<tool_result>/",
      "replacement": "const isToolResult = /<tool_result>/.test(content);"
    },
    {
      "file": "npm/src/agent/ProbeAgent.js",
      "line": 2480,
      "ruleId": "logic/incomplete-error-handling",
      "message": "Storage error handling only logs but doesn't propagate the error, potentially leaving the system in an inconsistent state",
      "severity": "warning",
      "category": "logic",
      "suggestion": "Either propagate the error or implement a retry mechanism to ensure data consistency",
      "replacement": "    } catch (error) {\\n      console.error(\`[ERROR] Failed to save compacted messages to storage:\`, error);\\n      throw new Error(\`Failed to save compacted history: \${error.message}\`);\\n    }"
    }
  ]
}
</attempt_completion>`;

    const result = parseXmlToolCallWithThinking(xmlString);

    expect(result).toBeDefined();
    expect(result.toolName).toBe('attempt_completion');
    expect(result.params.result).toBeDefined();

    // The result should be valid JSON
    let jsonResult;
    expect(() => {
      jsonResult = JSON.parse(result.params.result);
    }).not.toThrow();

    // Verify the complete structure
    expect(jsonResult.issues).toHaveLength(3);
    expect(jsonResult.issues[0].replacement).toContain('</attempt_completion>');
    expect(jsonResult.issues[2].replacement).toContain('Failed to save compacted history');
  });
});
