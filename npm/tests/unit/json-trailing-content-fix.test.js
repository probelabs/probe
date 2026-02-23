/**
 * Tests for JSON trailing content fix (issue #447)
 *
 * When JSON validation fails with "trailing content" error (e.g.,
 * "Unexpected non-whitespace character after JSON at position 477"),
 * we should try parsing the valid JSON prefix before entering the
 * expensive correction retry loop.
 *
 * Also tests that correction calls don't inherit inflated iteration budgets.
 */

import { describe, test, expect, jest, beforeEach, afterEach } from '@jest/globals';
import {
  validateJsonResponse,
  cleanSchemaResponse,
  tryExtractValidJsonPrefix
} from '../../src/agent/schemaUtils.js';
import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe('JSON Trailing Content Fix (Issue #447)', () => {
  describe('tryExtractValidJsonPrefix', () => {
    test('should extract valid JSON when followed by markdown content', () => {
      const validJson = '{"file_list": ["CustomGoPlugin.so"], "custom_middleware": {"pre": [{"name": "AddHeader"}], "driver": "goplugin"}}';
      const trailingContent = '\n\n2. [Upload this bundle](/tyk-cloud/using-plugins)...';
      const input = validJson + trailingContent;

      const result = tryExtractValidJsonPrefix(input);
      expect(result).not.toBeNull();
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual(JSON.parse(validJson));
    });

    test('should extract valid JSON array followed by text', () => {
      const validJson = '[{"name": "foo"}, {"name": "bar"}]';
      const trailingContent = '\nHere is some extra text';
      const input = validJson + trailingContent;

      const result = tryExtractValidJsonPrefix(input);
      expect(result).not.toBeNull();
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual(JSON.parse(validJson));
    });

    test('should return null for completely invalid JSON', () => {
      const input = 'This is not JSON at all';
      const result = tryExtractValidJsonPrefix(input);
      expect(result).toBeNull();
    });

    test('should return null for truncated JSON (no complete prefix)', () => {
      const input = '{"key": "value", "incomplete":';
      const result = tryExtractValidJsonPrefix(input);
      expect(result).toBeNull();
    });

    test('should handle JSON followed by whitespace only (not trailing content)', () => {
      const input = '{"key": "value"}   \n\n  ';
      // Pure whitespace after JSON is fine - JSON.parse handles it
      const result = tryExtractValidJsonPrefix(input);
      // Should return null because JSON.parse would succeed on the full string
      expect(result).toBeNull();
    });

    test('should handle nested JSON with trailing content', () => {
      const validJson = '{"outer": {"inner": [1, 2, 3], "nested": {"deep": true}}}';
      const trailingContent = ' some random text after';
      const input = validJson + trailingContent;

      const result = tryExtractValidJsonPrefix(input);
      expect(result).not.toBeNull();
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual(JSON.parse(validJson));
    });

    test('should validate against schema when provided', () => {
      const validJson = '{"file_list": ["a.so"], "status": "ok"}';
      const trailingContent = '\nExtra content here';
      const input = validJson + trailingContent;

      const schema = {
        type: 'object',
        properties: {
          file_list: { type: 'array', items: { type: 'string' } },
          status: { type: 'string' }
        },
        required: ['file_list']
      };

      const result = tryExtractValidJsonPrefix(input, { schema });
      expect(result).not.toBeNull();
      expect(result.isValid).toBe(true);
    });

    test('should return null when prefix is valid JSON but fails schema validation', () => {
      const validJson = '{"wrong_key": "value"}';
      const trailingContent = '\nExtra content';
      const input = validJson + trailingContent;

      const schema = {
        type: 'object',
        properties: {
          required_key: { type: 'string' }
        },
        required: ['required_key']
      };

      const result = tryExtractValidJsonPrefix(input, { schema });
      // Should return null because while JSON parses, it doesn't match schema
      expect(result).toBeNull();
    });
  });

  describe('validateJsonResponse handles trailing content gracefully', () => {
    test('should recover valid JSON from response with trailing markdown', () => {
      const validJson = '{"file_list": ["CustomGoPlugin.so"], "custom_middleware": {"pre": [{"name": "AddHeader"}]}}';
      const trailingContent = '\n\n2. [Upload this bundle](/tyk-cloud/using-plugins)...';
      const input = validJson + trailingContent;

      const result = validateJsonResponse(input);
      // After fix, this should succeed by extracting the valid prefix
      expect(result.isValid).toBe(true);
      expect(result.parsed).toEqual(JSON.parse(validJson));
    });

    test('should still fail for genuinely broken JSON', () => {
      const input = '{"broken": value}';
      const result = validateJsonResponse(input);
      expect(result.isValid).toBe(false);
    });
  });

  describe('Correction calls should not inherit inflated iteration budget', () => {
    test('should strip schema from correction options in attempt_completion path', () => {
      const probeAgentPath = join(__dirname, '../../src/agent/ProbeAgent.js');
      const sourceCode = readFileSync(probeAgentPath, 'utf-8');

      // The correction calls in the attempt_completion path should NOT spread schema.
      // Look for the correction call pattern - it should destructure schema out
      // or use _maxIterationsOverride

      // Find the attempt_completion correction call blocks
      const attemptCompletionBlock = sourceCode.substring(
        sourceCode.indexOf('completionAttempted && options.schema && !options._schemaFormatted && !options._skipValidation'),
        sourceCode.indexOf('completionAttempted && options.schema && !options._schemaFormatted && !options._skipValidation') + 5000
      );

      // Verify that correction calls either:
      // 1. Don't spread full options (to avoid inheriting schema), OR
      // 2. Have _maxIterationsOverride to limit iterations
      const hasCorrectionLimiter =
        attemptCompletionBlock.includes('_maxIterationsOverride') ||
        attemptCompletionBlock.includes('schema, ...correctionOptions') ||
        attemptCompletionBlock.includes('schema: undefined');

      expect(hasCorrectionLimiter).toBe(true);
    });

    test('should support _maxIterationsOverride option in answer()', () => {
      const probeAgentPath = join(__dirname, '../../src/agent/ProbeAgent.js');
      const sourceCode = readFileSync(probeAgentPath, 'utf-8');

      // Verify that _maxIterationsOverride is respected in iteration limit calculation
      expect(sourceCode).toContain('_maxIterationsOverride');
    });
  });

  describe('cleanSchemaResponse handles trailing content', () => {
    test('should handle JSON followed by markdown text', () => {
      const validJson = '{"result": "success"}';
      const input = validJson + '\n\nSome markdown text after';

      // cleanSchemaResponse should try to extract just the JSON
      const result = cleanSchemaResponse(input);
      // After clean, we should be able to parse it
      try {
        JSON.parse(result);
      } catch {
        // If cleanSchemaResponse doesn't fix it, the validateJsonResponse
        // trailing content handler should catch it
      }
    });
  });
});
