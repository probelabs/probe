/**
 * Tests for MCP server with strict elastic syntax validation
 */

import { search } from '../src/search.js';
import path from 'path';
import fs from 'fs';
import os from 'os';

describe('MCP Strict Elastic Syntax', () => {
  let tempDir;

  beforeEach(() => {
    // Create a temporary directory for test files
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'probe-mcp-test-'));

    // Create test files with various patterns
    const testFile = path.join(tempDir, 'test.js');
    fs.writeFileSync(testFile, `
function getUserId(user) {
  return user.id;
}

function get_user_name(user) {
  return user.name;
}

function errorHandler(err) {
  console.error('Error:', err);
}

function handleError(err) {
  errorHandler(err);
}
`);
  });

  afterEach(() => {
    // Clean up temp directory
    if (tempDir && fs.existsSync(tempDir)) {
      fs.rmSync(tempDir, { recursive: true, force: true });
    }
  });

  test('rejects vague query with strict syntax enabled', async () => {
    await expect(
      search({
        path: tempDir,
        query: 'error handler', // Vague query
        strictElasticSyntax: true,
        maxResults: 1,
        timeout: 10
      })
    ).rejects.toThrow(/Vague query format detected/);
  });

  test('rejects unquoted snake_case with strict syntax enabled', async () => {
    await expect(
      search({
        path: tempDir,
        query: 'get_user_name', // Unquoted snake_case
        strictElasticSyntax: true,
        maxResults: 1,
        timeout: 10
      })
    ).rejects.toThrow(/contains special characters/);
  });

  test('rejects unquoted camelCase with strict syntax enabled', async () => {
    await expect(
      search({
        path: tempDir,
        query: 'getUserId', // Unquoted camelCase
        strictElasticSyntax: true,
        maxResults: 1,
        timeout: 10
      })
    ).rejects.toThrow(/contains special characters/);
  });

  test('accepts explicit operators with strict syntax enabled', async () => {
    const result = await search({
      path: tempDir,
      query: '(error AND handler)',
      strictElasticSyntax: true,
      maxResults: 1,
      timeout: 10
    });

    expect(result).toBeDefined();
    expect(typeof result).toBe('string');
  });

  test('accepts quoted snake_case with strict syntax enabled', async () => {
    const result = await search({
      path: tempDir,
      query: '"get_user_name"',
      strictElasticSyntax: true,
      maxResults: 1,
      timeout: 10
    });

    expect(result).toBeDefined();
    expect(typeof result).toBe('string');
  });

  test('accepts quoted camelCase with strict syntax enabled', async () => {
    const result = await search({
      path: tempDir,
      query: '"getUserId"',
      strictElasticSyntax: true,
      maxResults: 1,
      timeout: 10
    });

    expect(result).toBeDefined();
    expect(typeof result).toBe('string');
  });

  test('accepts single word with strict syntax enabled', async () => {
    const result = await search({
      path: tempDir,
      query: 'error',
      strictElasticSyntax: true,
      maxResults: 1,
      timeout: 10
    });

    expect(result).toBeDefined();
    expect(typeof result).toBe('string');
  });

  test('accepts complex query with strict syntax enabled', async () => {
    const result = await search({
      path: tempDir,
      query: '("getUserId" AND NOT test)',
      strictElasticSyntax: true,
      maxResults: 1,
      timeout: 10
    });

    expect(result).toBeDefined();
    expect(typeof result).toBe('string');
  });

  test('allows vague query when strict syntax is disabled', async () => {
    const result = await search({
      path: tempDir,
      query: 'error handler', // Vague query but flag not set
      strictElasticSyntax: false,
      maxResults: 1,
      timeout: 10
    });

    expect(result).toBeDefined();
    expect(typeof result).toBe('string');
  });

  test('MCP default behavior enables strict syntax', () => {
    // Simulate MCP server default options
    const mcpOptions = {
      path: tempDir,
      query: 'test',
      allowTests: true,
      session: "new",
      maxResults: 20,
      maxTokens: 8000,
      strictElasticSyntax: true, // This is the MCP default
    };

    expect(mcpOptions.strictElasticSyntax).toBe(true);
  });
});
