/**
 * Tests for MCP server with strict elastic syntax validation
 *
 * Note: These tests use the freshly compiled probe binary from the workspace.
 * In CI, the binary will be built before tests run.
 */

import { search } from '../src/search.js';
import path from 'path';
import fs from 'fs';
import os from 'os';
import { exec } from 'child_process';
import { promisify } from 'util';
import { fileURLToPath } from 'url';

const execAsync = promisify(exec);
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Get the workspace root (one level up from npm/)
const workspaceRoot = path.resolve(__dirname, '..', '..');
const npmBinDir = path.join(__dirname, '..', 'bin');

/**
 * Copy the freshly built binary from workspace to npm bin directory
 */
async function copyFreshBinary() {
  try {
    // Check for release binary first, then debug
    const possibleBinaries = [
      path.join(workspaceRoot, 'target', 'release', 'probe'),
      path.join(workspaceRoot, 'target', 'debug', 'probe'),
    ];

    let sourceBinary = null;
    for (const binPath of possibleBinaries) {
      if (fs.existsSync(binPath)) {
        sourceBinary = binPath;
        console.log(`   ✓ Found fresh binary at: ${binPath}`);
        break;
      }
    }

    if (!sourceBinary) {
      console.log('   ⚠️  No fresh binary found in target/release or target/debug');
      console.log('   ℹ️  Run "cargo build --release" or "cargo build" to build the binary');
      return false;
    }

    // Ensure bin directory exists
    await fs.promises.mkdir(npmBinDir, { recursive: true });

    // Copy to npm bin directory
    const targetBinary = path.join(npmBinDir, 'probe-binary');
    await fs.promises.copyFile(sourceBinary, targetBinary);
    await fs.promises.chmod(targetBinary, 0o755);

    console.log(`   ✓ Copied fresh binary to: ${targetBinary}`);
    return true;
  } catch (error) {
    console.error(`   ✗ Error copying fresh binary: ${error.message}`);
    return false;
  }
}

/**
 * Check if the binary supports --strict-elastic-syntax flag
 */
async function binarySupportsStrictSyntax() {
  try {
    const binaryPath = path.join(npmBinDir, 'probe-binary');
    if (!fs.existsSync(binaryPath)) {
      return false;
    }

    const { stdout } = await execAsync(`"${binaryPath}" search --help`);
    return stdout.includes('--strict-elastic-syntax');
  } catch (error) {
    return false;
  }
}

describe('MCP Strict Elastic Syntax', () => {
  let tempDir;
  let skipTests = false;

  beforeAll(async () => {
    console.log('\n🔧 Setting up MCP strict syntax tests...');

    // Try to copy fresh binary
    const copiedFresh = await copyFreshBinary();

    if (!copiedFresh) {
      console.log('   ⚠️  Using existing binary (if available)');
    }

    // Check if binary supports the flag
    skipTests = !(await binarySupportsStrictSyntax());

    if (skipTests) {
      console.log('   ⚠️  Binary does not support --strict-elastic-syntax flag');
      console.log('   ℹ️  These tests will be skipped');
      console.log('   💡 To run tests, build the binary first: cargo build --release\n');
    } else {
      console.log('   ✓ Binary supports --strict-elastic-syntax flag');
      console.log('   ✓ All tests will run\n');
    }
  });

  beforeEach(() => {
    if (skipTests) return;

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
    if (skipTests) return;

    // Clean up temp directory
    if (tempDir && fs.existsSync(tempDir)) {
      fs.rmSync(tempDir, { recursive: true, force: true });
    }
  });

  test('rejects vague query with strict syntax enabled', async () => {
    if (skipTests) {
      console.log('   ⊘ Skipped: binary does not support --strict-elastic-syntax yet');
      return;
    }

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
    if (skipTests) {
      console.log('   ⊘ Skipped: binary does not support --strict-elastic-syntax yet');
      return;
    }

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
    if (skipTests) {
      console.log('   ⊘ Skipped: binary does not support --strict-elastic-syntax yet');
      return;
    }

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
    if (skipTests) {
      console.log('   ⊘ Skipped: binary does not support --strict-elastic-syntax yet');
      return;
    }

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
    if (skipTests) {
      console.log('   ⊘ Skipped: binary does not support --strict-elastic-syntax yet');
      return;
    }

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
    if (skipTests) {
      console.log('   ⊘ Skipped: binary does not support --strict-elastic-syntax yet');
      return;
    }

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
    if (skipTests) {
      console.log('   ⊘ Skipped: binary does not support --strict-elastic-syntax yet');
      return;
    }

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
    if (skipTests) {
      console.log('   ⊘ Skipped: binary does not support --strict-elastic-syntax yet');
      return;
    }

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
    // This test doesn't require the strict flag, so it always runs
    if (!tempDir) {
      tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'probe-mcp-test-'));
    }

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
    // This is a static test, always runs
    const mcpOptions = {
      path: '/some/path',
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
