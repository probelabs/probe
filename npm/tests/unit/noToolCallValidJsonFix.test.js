/**
 * Test for issue #443: error.no_tool_call rejects valid schema-matching JSON
 *
 * When an AI returns valid JSON matching the expected schema as plain text
 * (e.g., inside markdown code fences) rather than via attempt_completion,
 * the Probe agent should accept it as valid result instead of triggering error.no_tool_call.
 *
 * This test file first verifies the bug exists, then will pass after the fix.
 */

import { jest, describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { cleanSchemaResponse, validateJsonResponse } from '../../src/agent/schemaUtils.js';
import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe('Issue #443: error.no_tool_call rejects valid schema-matching JSON', () => {

  describe('Core schema response cleaning', () => {
    test('should extract JSON from markdown code fences', () => {
      const responseWithCodeFences = '```json\n{"projects": ["tyk-analytics", "portal"]}\n```';
      const cleaned = cleanSchemaResponse(responseWithCodeFences);

      expect(cleaned).toBe('{"projects": ["tyk-analytics", "portal"]}');

      // Should be valid JSON
      const validation = validateJsonResponse(cleaned);
      expect(validation.isValid).toBe(true);
      expect(validation.parsed).toEqual({ projects: ['tyk-analytics', 'portal'] });
    });

    test('should extract JSON from code fences with surrounding text', () => {
      // Real-world scenario: AI explains before providing JSON
      const responseWithText = `Based on my analysis of the available repositories, I've selected the most relevant ones:

\`\`\`json
{
  "projects": ["tyk-analytics", "portal"]
}
\`\`\`

These repositories contain the code related to your query about dashboard features.`;

      const cleaned = cleanSchemaResponse(responseWithText);

      // Should extract just the JSON
      expect(cleaned).toBe('{\n  "projects": ["tyk-analytics", "portal"]\n}');

      const validation = validateJsonResponse(cleaned);
      expect(validation.isValid).toBe(true);
    });

    test('should handle plain JSON without code fences', () => {
      const plainJson = '{"status": "completed", "result": 42}';
      const cleaned = cleanSchemaResponse(plainJson);

      expect(cleaned).toBe(plainJson);

      const validation = validateJsonResponse(cleaned);
      expect(validation.isValid).toBe(true);
    });

    test('should handle JSON with extra whitespace', () => {
      const jsonWithWhitespace = `
        {
          "message": "success",
          "count": 5
        }
      `;
      const cleaned = cleanSchemaResponse(jsonWithWhitespace);

      const validation = validateJsonResponse(cleaned);
      expect(validation.isValid).toBe(true);
    });
  });

  describe('Proposed fix validation helper', () => {
    /**
     * This function represents the proposed fix logic that should be added
     * to ProbeAgent.js when no tool call is detected but a schema is expected.
     *
     * Before triggering error.no_tool_call, we should:
     * 1. Strip markdown fences
     * 2. Validate the response as JSON
     * 3. If valid, accept it as the result
     */
    function tryExtractValidJson(responseText) {
      // Strip markdown code fences and validate
      const stripped = responseText
        .replace(/^```(?:json)?\s*\n?/m, '')
        .replace(/\n?```\s*$/m, '')
        .trim();

      try {
        JSON.parse(stripped);
        const validation = validateJsonResponse(stripped);
        if (validation.isValid) {
          return { valid: true, result: stripped };
        }
      } catch {
        // Not valid JSON
      }

      // Also try with cleanSchemaResponse for more robust cleaning
      const cleaned = cleanSchemaResponse(responseText);
      try {
        JSON.parse(cleaned);
        const validation = validateJsonResponse(cleaned);
        if (validation.isValid) {
          return { valid: true, result: cleaned };
        }
      } catch {
        // Still not valid JSON
      }

      return { valid: false, result: null };
    }

    test('should accept JSON in code fences as valid result', () => {
      const aiResponse = '```json\n{"projects": ["repo1", "repo2"]}\n```';

      const result = tryExtractValidJson(aiResponse);

      expect(result.valid).toBe(true);
      expect(result.result).toBe('{"projects": ["repo1", "repo2"]}');
    });

    test('should accept plain JSON as valid result', () => {
      const aiResponse = '{"status": "ok", "value": 100}';

      const result = tryExtractValidJson(aiResponse);

      expect(result.valid).toBe(true);
      expect(JSON.parse(result.result)).toEqual({ status: 'ok', value: 100 });
    });

    test('should accept JSON with surrounding explanation text', () => {
      const aiResponse = `After analyzing the request, here is my response:

\`\`\`json
{
  "selected": ["project-a", "project-b"],
  "reason": "These match the criteria"
}
\`\`\`

Let me know if you need more details.`;

      const result = tryExtractValidJson(aiResponse);

      expect(result.valid).toBe(true);
      expect(JSON.parse(result.result).selected).toEqual(['project-a', 'project-b']);
    });

    test('should reject non-JSON text', () => {
      const aiResponse = 'I cannot find the repositories you are looking for. Please check the paths.';

      const result = tryExtractValidJson(aiResponse);

      expect(result.valid).toBe(false);
    });

    test('should reject malformed JSON', () => {
      const aiResponse = '```json\n{"broken: json}\n```';

      const result = tryExtractValidJson(aiResponse);

      expect(result.valid).toBe(false);
    });

    test('should reject empty code blocks', () => {
      const aiResponse = '```json\n\n```';

      const result = tryExtractValidJson(aiResponse);

      expect(result.valid).toBe(false);
    });
  });

  describe('Real-world scenarios from issue #443', () => {
    test('should accept route-projects response with two repos selected', () => {
      // This is the actual response from the issue that was incorrectly rejected
      const aiResponse = `Based on the user's question about dashboard customization, I've selected the relevant repositories:

\`\`\`json
{
  "projects": ["tyk-analytics", "portal"]
}
\`\`\`

These repositories contain the dashboard and portal code that handle the customization features.`;

      // Strip markdown fences and validate
      const stripped = aiResponse
        .replace(/^```(?:json)?\s*\n?/m, '')
        .replace(/\n?```\s*$/m, '')
        .trim();

      // Use cleanSchemaResponse for robust cleaning
      const cleaned = cleanSchemaResponse(aiResponse);

      // Should be valid JSON after cleaning
      const validation = validateJsonResponse(cleaned);
      expect(validation.isValid).toBe(true);
      expect(validation.parsed.projects).toEqual(['tyk-analytics', 'portal']);
    });

    test('should accept JSON response even when thinking tags are present', () => {
      // Some models include thinking before the JSON
      const aiResponse = `<thinking>
The user is asking about dashboard features. I should select tyk-analytics and portal.
</thinking>

\`\`\`json
{"projects": ["tyk-analytics", "portal"]}
\`\`\``;

      // First remove thinking tags
      const withoutThinking = aiResponse
        .replace(/<thinking>[\s\S]*?<\/thinking>/gi, '')
        .trim();

      const cleaned = cleanSchemaResponse(withoutThinking);

      const validation = validateJsonResponse(cleaned);
      expect(validation.isValid).toBe(true);
      expect(validation.parsed.projects).toEqual(['tyk-analytics', 'portal']);
    });
  });

  describe('Edge cases', () => {
    test('should handle nested JSON objects', () => {
      const aiResponse = '```json\n{"config": {"nested": {"deep": "value"}}, "array": [1, 2, 3]}\n```';

      const cleaned = cleanSchemaResponse(aiResponse);
      const validation = validateJsonResponse(cleaned);

      expect(validation.isValid).toBe(true);
      expect(validation.parsed.config.nested.deep).toBe('value');
    });

    test('should handle JSON arrays at root level', () => {
      const aiResponse = '```json\n["item1", "item2", "item3"]\n```';

      const cleaned = cleanSchemaResponse(aiResponse);
      const validation = validateJsonResponse(cleaned);

      expect(validation.isValid).toBe(true);
      expect(validation.parsed).toEqual(['item1', 'item2', 'item3']);
    });

    test('should handle JSON with special characters in strings', () => {
      const aiResponse = '```json\n{"message": "Hello\\nWorld", "path": "C:\\\\Users\\\\test"}\n```';

      const cleaned = cleanSchemaResponse(aiResponse);
      const validation = validateJsonResponse(cleaned);

      expect(validation.isValid).toBe(true);
      expect(validation.parsed.message).toBe('Hello\nWorld');
    });

    test('should handle JSON with unicode characters', () => {
      const aiResponse = '```json\n{"greeting": "Hello, \u4e16\u754c!", "emoji": "\ud83d\ude00"}\n```';

      const cleaned = cleanSchemaResponse(aiResponse);
      const validation = validateJsonResponse(cleaned);

      expect(validation.isValid).toBe(true);
    });

    test('should handle multiple code blocks - extract first JSON block', () => {
      const aiResponse = `Here's the result:

\`\`\`json
{"primary": "result"}
\`\`\`

And here's some additional info:

\`\`\`text
Not JSON
\`\`\``;

      const cleaned = cleanSchemaResponse(aiResponse);
      const validation = validateJsonResponse(cleaned);

      expect(validation.isValid).toBe(true);
      expect(validation.parsed.primary).toBe('result');
    });
  });

  describe('Test cases from issue specification', () => {
    // These are the exact test cases specified in the issue

    test('JSON in code fences, matches schema -> Accept as valid result', () => {
      const response = '```json\n{"result": "success", "count": 42}\n```';
      const cleaned = cleanSchemaResponse(response);
      const validation = validateJsonResponse(cleaned);

      expect(validation.isValid).toBe(true);
      // This should be accepted, not trigger error.no_tool_call
    });

    test('Plain JSON, matches schema -> Accept as valid result', () => {
      const response = '{"result": "success", "count": 42}';
      const cleaned = cleanSchemaResponse(response);
      const validation = validateJsonResponse(cleaned);

      expect(validation.isValid).toBe(true);
      // This should be accepted, not trigger error.no_tool_call
    });

    test('Non-JSON plain text -> Should trigger error.no_tool_call', () => {
      const response = 'I found the results you were looking for in the repository.';
      const cleaned = cleanSchemaResponse(response);
      const validation = validateJsonResponse(cleaned);

      expect(validation.isValid).toBe(false);
      // This SHOULD trigger error.no_tool_call
    });

    test('JSON not matching schema (simulation) -> Should trigger error.no_tool_call', () => {
      // Malformed JSON that doesn't match any schema
      const response = '```json\n{incomplete json';
      const cleaned = cleanSchemaResponse(response);
      const validation = validateJsonResponse(cleaned);

      expect(validation.isValid).toBe(false);
      // This SHOULD trigger error.no_tool_call
    });
  });

  describe('ProbeAgent.js structural verification', () => {
    test('should have the fix for issue #443 in ProbeAgent.js', () => {
      const probeAgentPath = join(__dirname, '../../src/agent/ProbeAgent.js');
      const sourceCode = readFileSync(probeAgentPath, 'utf-8');

      // Verify the fix comment is present
      expect(sourceCode).toContain('Issue #443');
      expect(sourceCode).toContain('Check if response contains valid schema-matching JSON');

      // Verify the fix logic pattern is present
      expect(sourceCode).toContain('cleanSchemaResponse(contentToCheck)');
      expect(sourceCode).toContain('validateJsonResponse(cleanedJson');

      // Verify it's in the "no tool call found" section
      expect(sourceCode).toContain('// No tool call found');
    });

    test('should check options.schema before accepting JSON without attempt_completion', () => {
      const probeAgentPath = join(__dirname, '../../src/agent/ProbeAgent.js');
      const sourceCode = readFileSync(probeAgentPath, 'utf-8');

      // The fix should only apply when options.schema is set
      // Find the section with Issue #443 and verify it checks options.schema
      const issue443Pattern = /Issue #443[\s\S]*?if \(options\.schema\)/;
      expect(sourceCode).toMatch(issue443Pattern);
    });

    test('should remove thinking tags before checking for valid JSON', () => {
      const probeAgentPath = join(__dirname, '../../src/agent/ProbeAgent.js');
      const sourceCode = readFileSync(probeAgentPath, 'utf-8');

      // The fix should remove thinking tags before validating JSON
      // This is important for models that include <thinking> before JSON
      const thinkingRemovalPattern = /Issue #443[\s\S]*?<thinking>[\s\S]*?<\/thinking>/;
      expect(sourceCode).toMatch(thinkingRemovalPattern);
    });
  });
});
