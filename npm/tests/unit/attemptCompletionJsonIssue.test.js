import { describe, test, expect } from '@jest/globals';
import { cleanSchemaResponse } from '../../src/agent/schemaUtils.js';

describe('attempt_completion JSON parsing issue', () => {
  test('should reproduce the issue where attempt_completion result with markdown causes JSON parsing errors', () => {
    // This is the actual result format from the failure log
    const attemptCompletionResult = `\`\`\`json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "code-review",
  "title": "Code Review",
  "description": "Structured format for code review issues",
  "type": "object",
  "required": [
    "issues"
  ],
  "properties": {
    "issues": {
      "type": "array",
      "description": "List of issues found during code review",
      "items": [
        {
          "file": "docs/failure-conditions-implementation.md",
          "line": 1,
          "ruleId": "documentation/inconsistency",
          "message": "The documentation and examples consistently refer to JEXL as the expression language, but the implementation uses sandboxed JavaScript. This is highly misleading for users trying to write conditions.",
          "severity": "error",
          "category": "documentation",
          "suggestion": "Update all documentation, including markdown files and YAML examples, to correctly refer to JavaScript expressions. Remove all mentions of JEXL to avoid confusion and ensure users are writing valid conditions based on the actual implementation."
        }
      ]
    }
  }
}
\`\`\``;

    // Test that this fails when parsed directly as JSON
    expect(() => JSON.parse(attemptCompletionResult)).toThrow();

    // Test that cleanSchemaResponse would handle this correctly
    const cleaned = cleanSchemaResponse(attemptCompletionResult);
    
    // After cleaning, it should be valid JSON
    expect(() => JSON.parse(cleaned)).not.toThrow();
    
    const parsed = JSON.parse(cleaned);
    expect(parsed).toHaveProperty('$schema');
    expect(parsed).toHaveProperty('properties');
    expect(parsed.properties).toHaveProperty('issues');
  });

  test('should handle attempt_completion result that is plain text', () => {
    const plainTextResult = "Hello! I've reviewed the pull request and focused on performance aspects as requested. The introduction of conditional execution is a powerful feature.";

    const cleaned = cleanSchemaResponse(plainTextResult);
    
    // Plain text should remain unchanged
    expect(cleaned).toBe(plainTextResult);
  });

  test('should handle attempt_completion result with mixed content', () => {
    const mixedResult = `Here is my analysis:

\`\`\`json
{
  "summary": "The code looks good",
  "issues": []
}
\`\`\`

Additional notes: The implementation is solid.`;

    const cleaned = cleanSchemaResponse(mixedResult);
    
    // Should extract just the JSON part
    expect(cleaned.trim()).toBe('{\n  "summary": "The code looks good",\n  "issues": []\n}');
    expect(() => JSON.parse(cleaned)).not.toThrow();
  });
});