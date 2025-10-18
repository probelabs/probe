/**
 * Integration tests for MCP Server .env file loading
 */

import { jest } from '@jest/globals';
import { spawn } from 'child_process';
import { join } from 'path';
import { mkdtemp, writeFile, rm } from 'fs/promises';
import { tmpdir } from 'os';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe('MCP Server .env Support', () => {
  let tempDir;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'mcp-dotenv-test-'));
  });

  afterEach(async () => {
    if (tempDir) {
      await rm(tempDir, { recursive: true, force: true });
    }
  });

  test('should load environment variables from .env file', async () => {
    // Create a .env file in the temp directory
    const envContent = `
TEST_VAR=from_dotenv
PROBE_PATH=/custom/probe/path
DEBUG=true
    `.trim();

    await writeFile(join(tempDir, '.env'), envContent);

    // Test using dotenv directly (simulating what MCP server does)
    const dotenv = await import('dotenv');
    const result = dotenv.config({ path: join(tempDir, '.env') });

    expect(result.error).toBeUndefined();
    expect(result.parsed).toEqual({
      TEST_VAR: 'from_dotenv',
      PROBE_PATH: '/custom/probe/path',
      DEBUG: 'true'
    });
  });

  test('should not fail if .env file does not exist', async () => {
    // Test using dotenv with non-existent file (simulating what MCP server does)
    const dotenv = await import('dotenv');
    const result = dotenv.config({ path: join(tempDir, '.env') });

    // dotenv.config() should not throw when file doesn't exist
    // It returns an error object but doesn't throw
    expect(result.error).toBeDefined();
    expect(result.error.code).toBe('ENOENT');
    // When there's an error, parsed is an empty object
    expect(result.parsed).toEqual({});
  });

  test('should prioritize existing environment variables over .env', async () => {
    // Create a .env file
    const envContent = `
TEST_PRIORITY=from_dotenv
    `.trim();

    await writeFile(join(tempDir, '.env'), envContent);

    // Set an environment variable
    const originalValue = process.env.TEST_PRIORITY;
    process.env.TEST_PRIORITY = 'from_environment';

    try {
      // Test using dotenv
      const dotenv = await import('dotenv');
      const result = dotenv.config({ path: join(tempDir, '.env') });

      expect(result.error).toBeUndefined();
      // dotenv by default does not override existing env vars
      expect(process.env.TEST_PRIORITY).toBe('from_environment');
    } finally {
      // Clean up
      if (originalValue !== undefined) {
        process.env.TEST_PRIORITY = originalValue;
      } else {
        delete process.env.TEST_PRIORITY;
      }
    }
  });

  test('MCP server should have dotenv loaded at startup', async () => {
    // This test verifies that the MCP server index.ts has dotenv import
    const { readFile } = await import('fs/promises');
    const mcpIndexPath = join(__dirname, '..', '..', 'src', 'mcp', 'index.ts');

    const content = await readFile(mcpIndexPath, 'utf8');

    // Check that dotenv is imported
    expect(content).toContain("import { config } from 'dotenv'");

    // Check that config() is called
    expect(content).toContain('config()');

    // Check that the dotenv import is near the top (before other imports)
    const lines = content.split('\n');
    const dotenvImportLine = lines.findIndex(line => line.includes("import { config } from 'dotenv'"));
    const configCallLine = lines.findIndex(line => line.includes('config()'));
    const firstRegularImport = lines.findIndex(line =>
      line.includes('import') &&
      !line.includes('dotenv') &&
      !line.includes('#!/usr/bin/env node')
    );

    // dotenv should be imported and called before other imports
    expect(dotenvImportLine).toBeGreaterThan(-1);
    expect(configCallLine).toBeGreaterThan(-1);
    expect(dotenvImportLine).toBeLessThan(firstRegularImport);
  });

  test('ProbeAgent should have dotenv loaded at startup', async () => {
    // This test verifies that ProbeAgent.js has dotenv import
    const { readFile } = await import('fs/promises');
    const probeAgentPath = join(__dirname, '..', '..', 'src', 'agent', 'ProbeAgent.js');

    const content = await readFile(probeAgentPath, 'utf8');

    // Check that dotenv is imported
    expect(content).toContain("import dotenv from 'dotenv'");

    // Check that config() is called
    expect(content).toContain('dotenv.config()');

    // Check that the dotenv import is near the top (before other imports)
    const lines = content.split('\n');
    const dotenvImportLine = lines.findIndex(line => line.includes("import dotenv from 'dotenv'"));
    const configCallLine = lines.findIndex(line => line.includes('dotenv.config()'));

    // dotenv should be imported and called at the top
    expect(dotenvImportLine).toBeGreaterThan(-1);
    expect(configCallLine).toBeGreaterThan(-1);
    expect(dotenvImportLine).toBeLessThan(10); // Should be in first 10 lines
  });

  test('Main package index should have dotenv loaded at startup', async () => {
    // This test verifies that index.js has dotenv import
    const { readFile } = await import('fs/promises');
    const indexPath = join(__dirname, '..', '..', 'src', 'index.js');

    const content = await readFile(indexPath, 'utf8');

    // Check that dotenv is imported
    expect(content).toContain("import dotenv from 'dotenv'");

    // Check that config() is called
    expect(content).toContain('dotenv.config()');
  });

  test('Agent CLI index should have dotenv loaded at startup', async () => {
    // This test verifies that agent/index.js has dotenv import
    const { readFile } = await import('fs/promises');
    const agentIndexPath = join(__dirname, '..', '..', 'src', 'agent', 'index.js');

    const content = await readFile(agentIndexPath, 'utf8');

    // Check that dotenv is imported
    expect(content).toContain("import dotenv from 'dotenv'");

    // Check that config() is called
    expect(content).toContain('dotenv.config()');
  });
});
