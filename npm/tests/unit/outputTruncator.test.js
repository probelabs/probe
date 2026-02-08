/**
 * Tests for outputTruncator module
 */

import { describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { readFile, rm, stat } from 'fs/promises';
import { tmpdir } from 'os';
import { join } from 'path';
import { truncateIfNeeded, getMaxOutputTokens } from '../../src/agent/outputTruncator.js';

// Mock token counter that returns 1 token per 4 characters (same approximation as real implementation)
const createMockTokenCounter = () => ({
  countTokens: (text) => Math.ceil(text.length / 4)
});

describe('getMaxOutputTokens', () => {
  const originalEnv = process.env.PROBE_MAX_OUTPUT_TOKENS;

  afterEach(() => {
    // Restore original env
    if (originalEnv !== undefined) {
      process.env.PROBE_MAX_OUTPUT_TOKENS = originalEnv;
    } else {
      delete process.env.PROBE_MAX_OUTPUT_TOKENS;
    }
  });

  test('should return default value (20000) when no config provided', () => {
    delete process.env.PROBE_MAX_OUTPUT_TOKENS;
    expect(getMaxOutputTokens(undefined)).toBe(20000);
    expect(getMaxOutputTokens(null)).toBe(20000);
  });

  test('should use constructor value when provided', () => {
    process.env.PROBE_MAX_OUTPUT_TOKENS = '5000';
    expect(getMaxOutputTokens(10000)).toBe(10000);
  });

  test('should use environment variable when no constructor value', () => {
    process.env.PROBE_MAX_OUTPUT_TOKENS = '15000';
    expect(getMaxOutputTokens(undefined)).toBe(15000);
  });

  test('should prioritize constructor value over env variable', () => {
    process.env.PROBE_MAX_OUTPUT_TOKENS = '5000';
    expect(getMaxOutputTokens(8000)).toBe(8000);
  });

  test('should handle string numbers in constructor', () => {
    expect(getMaxOutputTokens('12000')).toBe(12000);
  });

  // Edge case tests for invalid inputs
  test('should return default for NaN constructor value', () => {
    delete process.env.PROBE_MAX_OUTPUT_TOKENS;
    expect(getMaxOutputTokens(NaN)).toBe(20000);
    expect(getMaxOutputTokens('invalid')).toBe(20000);
  });

  test('should return default for negative constructor value', () => {
    delete process.env.PROBE_MAX_OUTPUT_TOKENS;
    expect(getMaxOutputTokens(-1)).toBe(20000);
    expect(getMaxOutputTokens(-1000)).toBe(20000);
  });

  test('should return default for zero constructor value', () => {
    delete process.env.PROBE_MAX_OUTPUT_TOKENS;
    expect(getMaxOutputTokens(0)).toBe(20000);
  });

  test('should return default for invalid env variable', () => {
    process.env.PROBE_MAX_OUTPUT_TOKENS = 'invalid';
    expect(getMaxOutputTokens(undefined)).toBe(20000);
  });

  test('should return default for negative env variable', () => {
    process.env.PROBE_MAX_OUTPUT_TOKENS = '-100';
    expect(getMaxOutputTokens(undefined)).toBe(20000);
  });

  test('should return default for zero env variable', () => {
    process.env.PROBE_MAX_OUTPUT_TOKENS = '0';
    expect(getMaxOutputTokens(undefined)).toBe(20000);
  });

  test('should fall through to env when constructor is invalid', () => {
    process.env.PROBE_MAX_OUTPUT_TOKENS = '5000';
    // Invalid constructor should fall through to valid env
    expect(getMaxOutputTokens('invalid')).toBe(5000);
  });
});

describe('truncateIfNeeded', () => {
  const tokenCounter = createMockTokenCounter();
  const sessionId = 'test-session';

  test('should return content unchanged when under limit', async () => {
    const content = 'Short content';
    const result = await truncateIfNeeded(content, tokenCounter, sessionId, 20000);

    expect(result.truncated).toBe(false);
    expect(result.content).toBe(content);
    expect(result.tempFilePath).toBeUndefined();
    expect(result.originalTokens).toBeUndefined();
  });

  test('should truncate content and save to file when over limit (small limit, head-only)', async () => {
    // Create content that's definitely over the limit (100 tokens = 400 chars with our mock)
    // We'll use a limit of 10 tokens = 40 chars (below MIN_LIMIT_FOR_TAIL=2000, so head-only)
    const content = 'A'.repeat(200); // 50 tokens with our mock
    const maxTokens = 10; // Much smaller limit

    const result = await truncateIfNeeded(content, tokenCounter, sessionId, maxTokens);

    expect(result.truncated).toBe(true);
    expect(result.originalTokens).toBe(50); // 200 chars / 4 = 50 tokens
    expect(result.tempFilePath).toBeDefined();
    expect(result.tempFilePath).toContain('probe-output');
    expect(result.tempFilePath).toContain(sessionId);

    // Verify the truncated message format
    expect(result.content).toContain('Output exceeded maximum size');
    expect(result.content).toContain('50 tokens');
    expect(result.content).toContain('limit: 10');
    expect(result.content).toContain('Full output saved to:');
    expect(result.content).toContain('--- Truncated Output');

    // Small limit should NOT use head+tail (below MIN_LIMIT_FOR_TAIL)
    expect(result.content).not.toContain('tokens omitted');

    // Verify the file was created with full content
    const fileContent = await readFile(result.tempFilePath, 'utf8');
    expect(fileContent).toBe(content);

    // Cleanup
    await rm(result.tempFilePath);
  });

  test('should use head+tail truncation when limit is large enough', async () => {
    // Use a limit of 5000 tokens (above MIN_LIMIT_FOR_TAIL=2000)
    // Create content of 10000 tokens = 40000 chars
    const headChar = 'H';
    const middleChar = 'M';
    const tailChar = 'T';
    // Head: first 16000 chars (4000 tokens), Middle: 20000 chars (5000 tokens), Tail: last 4000 chars (1000 tokens)
    const content = headChar.repeat(16000) + middleChar.repeat(20000) + tailChar.repeat(4000);
    // Total: 40000 chars = 10000 tokens
    const maxTokens = 5000;

    const result = await truncateIfNeeded(content, tokenCounter, sessionId, maxTokens);

    expect(result.truncated).toBe(true);
    expect(result.originalTokens).toBe(10000);

    // Should contain the omitted tokens placeholder
    // headTokens = 5000 - 1000 = 4000, tailTokens = 1000
    // omitted = 10000 - 4000 - 1000 = 5000
    expect(result.content).toContain('5000 tokens omitted');

    // Head portion: 4000 tokens = 16000 chars from the start (all H's)
    expect(result.content).toContain(headChar.repeat(16000));

    // Tail portion: 1000 tokens = 4000 chars from the end (all T's)
    expect(result.content).toContain(tailChar.repeat(4000));

    // Cleanup
    await rm(result.tempFilePath);
  });

  test('should use default max tokens when not provided', async () => {
    const content = 'Short content';
    const result = await truncateIfNeeded(content, tokenCounter, sessionId);

    // Short content should not be truncated with default 20000 limit
    expect(result.truncated).toBe(false);
  });

  test('should handle missing sessionId gracefully', async () => {
    const content = 'A'.repeat(200);
    const maxTokens = 10;

    const result = await truncateIfNeeded(content, tokenCounter, undefined, maxTokens);

    expect(result.truncated).toBe(true);
    expect(result.tempFilePath).toContain('unknown');

    // Cleanup
    await rm(result.tempFilePath);
  });

  test('should create temp directory if it does not exist', async () => {
    const content = 'A'.repeat(200);
    const maxTokens = 10;

    const result = await truncateIfNeeded(content, tokenCounter, 'new-session', maxTokens);

    expect(result.truncated).toBe(true);

    // Verify file exists
    const fileStat = await stat(result.tempFilePath);
    expect(fileStat.isFile()).toBe(true);

    // Cleanup
    await rm(result.tempFilePath);
  });

  test('should truncate content to approximately maxTokens characters (head-only for small limits)', async () => {
    // Using limit of 50 which is below MIN_LIMIT_FOR_TAIL=2000, so head-only
    const content = 'A'.repeat(1000); // 250 tokens
    const maxTokens = 50; // Should result in ~200 chars (50 * 4)

    const result = await truncateIfNeeded(content, tokenCounter, sessionId, maxTokens);

    expect(result.truncated).toBe(true);

    // The truncated content in the message should be approximately 200 chars
    // Extract the truncated part from the message (head-only, no tail)
    const truncatedMatch = result.content.match(/--- Truncated Output ---\n(A+)\n--- End/s);
    expect(truncatedMatch).toBeTruthy();
    // The truncated content should be approximately maxTokens * 4 chars
    expect(truncatedMatch[1].length).toBe(200); // 50 tokens * 4 chars/token

    // Cleanup
    await rm(result.tempFilePath);
  });

  test('should handle exact boundary case', async () => {
    // 79999 chars = 19999.75 tokens, rounds to 20000 tokens
    // Actually let's be more precise: 80000 chars / 4 = 20000 tokens exactly
    const content = 'A'.repeat(80000); // Exactly 20000 tokens
    const maxTokens = 20000;

    const result = await truncateIfNeeded(content, tokenCounter, sessionId, maxTokens);

    // Exactly at the limit should NOT be truncated
    expect(result.truncated).toBe(false);
    expect(result.content).toBe(content);
  });

  test('should truncate when just over the limit', async () => {
    const content = 'A'.repeat(80004); // 20001 tokens (just over limit)
    const maxTokens = 20000;

    const result = await truncateIfNeeded(content, tokenCounter, sessionId, maxTokens);

    expect(result.truncated).toBe(true);
    expect(result.originalTokens).toBe(20001);

    // Cleanup
    await rm(result.tempFilePath);
  });

  // Edge case tests for invalid maxTokens
  test('should use default limit for invalid maxTokens (NaN)', async () => {
    const content = 'Short content';
    const result = await truncateIfNeeded(content, tokenCounter, sessionId, NaN);

    // Should use default 20000, so short content should not be truncated
    expect(result.truncated).toBe(false);
  });

  test('should use default limit for negative maxTokens', async () => {
    const content = 'Short content';
    const result = await truncateIfNeeded(content, tokenCounter, sessionId, -100);

    // Should use default 20000, so short content should not be truncated
    expect(result.truncated).toBe(false);
  });

  test('should use default limit for zero maxTokens', async () => {
    const content = 'Short content';
    const result = await truncateIfNeeded(content, tokenCounter, sessionId, 0);

    // Should use default 20000, so short content should not be truncated
    expect(result.truncated).toBe(false);
  });

  test('should not have error field when file write succeeds', async () => {
    const content = 'A'.repeat(200);
    const maxTokens = 10;

    const result = await truncateIfNeeded(content, tokenCounter, sessionId, maxTokens);

    expect(result.truncated).toBe(true);
    expect(result.error).toBeUndefined();

    // Cleanup
    await rm(result.tempFilePath);
  });
});
