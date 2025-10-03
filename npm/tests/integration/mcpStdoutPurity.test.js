/**
 * Tests to verify MCP servers only output JSON-RPC messages to stdout
 * and all diagnostic/debug messages go to stderr
 */

import { spawn } from 'child_process';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { writeFile, rm, mkdir } from 'fs/promises';
import { tmpdir } from 'os';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe('MCP Stdout Purity Tests', () => {
  let tempDir;

  beforeEach(async () => {
    tempDir = join(tmpdir(), `mcp-stdout-test-${Date.now()}`);
    await mkdir(tempDir, { recursive: true });
  });

  afterEach(async () => {
    if (tempDir) {
      await rm(tempDir, { recursive: true, force: true });
    }
  });

  /**
   * Helper to validate that output is valid JSON-RPC
   */
  function isValidJsonRpc(line) {
    try {
      const parsed = JSON.parse(line);
      // Must have jsonrpc field
      if (parsed.jsonrpc !== '2.0') return false;
      // Must have either method (request) or result/error (response)
      if (!parsed.method && !('result' in parsed) && !('error' in parsed)) return false;
      return true;
    } catch {
      return false;
    }
  }

  /**
   * Helper to spawn MCP server and collect stdout/stderr
   */
  function spawnMcpServer(serverPath, args = []) {
    return new Promise((resolve, reject) => {
      const stdoutLines = [];
      const stderrLines = [];
      let stdoutBuffer = '';
      let stderrBuffer = '';

      const proc = spawn('node', [serverPath, ...args], {
        stdio: ['pipe', 'pipe', 'pipe']
      });

      proc.stdout.on('data', (data) => {
        stdoutBuffer += data.toString();
        const lines = stdoutBuffer.split('\n');
        stdoutBuffer = lines.pop(); // Keep incomplete line in buffer
        stdoutLines.push(...lines.filter(line => line.trim()));
      });

      proc.stderr.on('data', (data) => {
        stderrBuffer += data.toString();
        const lines = stderrBuffer.split('\n');
        stderrBuffer = lines.pop(); // Keep incomplete line in buffer
        stderrLines.push(...lines.filter(line => line.trim()));
      });

      proc.on('error', reject);

      // Send initialize request
      const initRequest = JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'initialize',
        params: {
          protocolVersion: '2024-11-05',
          capabilities: {},
          clientInfo: { name: 'test-client', version: '1.0.0' }
        }
      });

      proc.stdin.write(initRequest + '\n');

      // Wait for response
      setTimeout(() => {
        proc.kill();
        resolve({ stdoutLines, stderrLines });
      }, 2000);
    });
  }

  describe('Probe MCP Server Stdout Purity', () => {
    test('should only output JSON-RPC messages to stdout', async () => {
      const mcpServerPath = join(__dirname, '../../build/mcp/index.js');

      const { stdoutLines, stderrLines } = await spawnMcpServer(mcpServerPath);

      // Verify ALL stdout lines are valid JSON-RPC
      stdoutLines.forEach((line, index) => {
        const isValid = isValidJsonRpc(line);
        if (!isValid) {
          console.error(`Invalid JSON-RPC on stdout line ${index}:`, line);
        }
        expect(isValid).toBe(true);
      });

      // Stderr can contain diagnostic messages (this is OK)
      console.log(`Probe MCP server stderr lines: ${stderrLines.length}`);
      console.log('Sample stderr output:', stderrLines.slice(0, 3));
    }, 10000);

    test('should handle --help flag without polluting stdout', async () => {
      const mcpServerPath = join(__dirname, '../../build/mcp/index.js');

      return new Promise((resolve, reject) => {
        const stdoutLines = [];
        const stderrLines = [];

        const proc = spawn('node', [mcpServerPath, '--help'], {
          stdio: ['pipe', 'pipe', 'pipe']
        });

        proc.stdout.on('data', (data) => {
          data.toString().split('\n').forEach(line => {
            if (line.trim()) stdoutLines.push(line);
          });
        });

        proc.stderr.on('data', (data) => {
          data.toString().split('\n').forEach(line => {
            if (line.trim()) stderrLines.push(line);
          });
        });

        proc.on('close', () => {
          // Help message should go to stderr, not stdout
          expect(stdoutLines.length).toBe(0);
          expect(stderrLines.length).toBeGreaterThan(0);

          // Verify help content is in stderr
          const helpText = stderrLines.join('\n');
          expect(helpText).toContain('Probe MCP Server');
          expect(helpText).toContain('Usage:');

          resolve();
        });

        proc.on('error', reject);
      });
    });
  });

  describe('Probe Agent MCP Server Stdout Purity', () => {
    test('should only output JSON-RPC messages to stdout', async () => {
      const agentMcpPath = join(__dirname, '../../src/agent/index.js');

      const { stdoutLines, stderrLines } = await spawnMcpServer(agentMcpPath, ['--mcp']);

      // Verify ALL stdout lines are valid JSON-RPC
      stdoutLines.forEach((line, index) => {
        const isValid = isValidJsonRpc(line);
        if (!isValid) {
          console.error(`Invalid JSON-RPC on stdout line ${index}:`, line);
        }
        expect(isValid).toBe(true);
      });

      // Stderr can contain diagnostic messages (this is OK)
      console.log(`Probe Agent MCP server stderr lines: ${stderrLines.length}`);
      console.log('Sample stderr output:', stderrLines.slice(0, 3));
    }, 10000);

    test('should handle --help flag without polluting stdout when in MCP mode', async () => {
      const agentPath = join(__dirname, '../../src/agent/index.js');

      return new Promise((resolve, reject) => {
        const stdoutLines = [];
        const stderrLines = [];

        const proc = spawn('node', [agentPath, '--help'], {
          stdio: ['pipe', 'pipe', 'pipe']
        });

        proc.stdout.on('data', (data) => {
          data.toString().split('\n').forEach(line => {
            if (line.trim()) stdoutLines.push(line);
          });
        });

        proc.stderr.on('data', (data) => {
          data.toString().split('\n').forEach(line => {
            if (line.trim()) stderrLines.push(line);
          });
        });

        proc.on('close', () => {
          // Help message should go to stderr, not stdout
          expect(stdoutLines.length).toBe(0);
          expect(stderrLines.length).toBeGreaterThan(0);

          // Verify help content is in stderr
          const helpText = stderrLines.join('\n');
          expect(helpText).toContain('probe agent');
          expect(helpText).toContain('Usage:');

          resolve();
        });

        proc.on('error', reject);
      });
    });
  });

  describe('Binary Wrapper Stdout Purity', () => {
    test('should only output JSON-RPC when calling probe mcp', async () => {
      const probeBinPath = join(__dirname, '../../bin/probe');

      const { stdoutLines, stderrLines } = await spawnMcpServer(probeBinPath, ['mcp']);

      // Verify ALL stdout lines are valid JSON-RPC
      stdoutLines.forEach((line, index) => {
        const isValid = isValidJsonRpc(line);
        if (!isValid) {
          console.error(`Invalid JSON-RPC on stdout line ${index}:`, line);
        }
        expect(isValid).toBe(true);
      });

      // Binary download messages should go to stderr
      console.log(`Probe binary wrapper stderr lines: ${stderrLines.length}`);
      console.log('Sample stderr output:', stderrLines.slice(0, 3));
    }, 10000);
  });

  describe('Error Scenarios', () => {
    test('should keep error messages on stderr even during failures', async () => {
      const mcpServerPath = join(__dirname, '../../build/mcp/index.js');

      return new Promise((resolve, reject) => {
        const stdoutLines = [];
        const stderrLines = [];

        const proc = spawn('node', [mcpServerPath], {
          stdio: ['pipe', 'pipe', 'pipe']
        });

        proc.stdout.on('data', (data) => {
          data.toString().split('\n').forEach(line => {
            if (line.trim()) stdoutLines.push(line);
          });
        });

        proc.stderr.on('data', (data) => {
          data.toString().split('\n').forEach(line => {
            if (line.trim()) stderrLines.push(line);
          });
        });

        // Send malformed request
        proc.stdin.write('invalid json\n');

        setTimeout(() => {
          proc.kill();

          // Even with errors, stdout should only contain JSON-RPC messages
          stdoutLines.forEach((line, index) => {
            if (line.trim()) {
              const isValid = isValidJsonRpc(line);
              if (!isValid) {
                console.error(`Invalid JSON-RPC on stdout line ${index}:`, line);
              }
              expect(isValid).toBe(true);
            }
          });

          // Error messages can be on stderr
          console.log('Error scenario stderr:', stderrLines.slice(0, 5));

          resolve();
        }, 2000);

        proc.on('error', reject);
      });
    }, 10000);
  });

  describe('Regression Tests', () => {
    test('should not leak package.json discovery logs to stdout', async () => {
      const mcpServerPath = join(__dirname, '../../build/mcp/index.js');

      const { stdoutLines, stderrLines } = await spawnMcpServer(mcpServerPath);

      // Check that package.json messages don't leak to stdout
      stdoutLines.forEach(line => {
        expect(line).not.toContain('Found package.json at:');
        expect(line).not.toContain('Using version from package.json:');
        expect(line).not.toContain('Bin directory:');
      });

      // These messages should be in stderr instead
      const stderrText = stderrLines.join('\n');
      // Note: These might not always appear depending on installation
      // but if they do, they should be in stderr
      if (stderrText.includes('package.json')) {
        console.log('✓ Package.json discovery logs correctly sent to stderr');
      }
    }, 10000);

    test('should not leak binary download messages to stdout', async () => {
      // This test verifies the fix in npm/bin/probe
      const probeBinPath = join(__dirname, '../../bin/probe');

      return new Promise((resolve, reject) => {
        const stdoutLines = [];
        const stderrLines = [];

        const proc = spawn('node', [probeBinPath, 'mcp'], {
          stdio: ['pipe', 'pipe', 'pipe']
        });

        proc.stdout.on('data', (data) => {
          data.toString().split('\n').forEach(line => {
            if (line.trim()) stdoutLines.push(line);
          });
        });

        proc.stderr.on('data', (data) => {
          data.toString().split('\n').forEach(line => {
            if (line.trim()) stderrLines.push(line);
          });
        });

        setTimeout(() => {
          proc.kill();

          // Binary download messages should NOT be in stdout
          stdoutLines.forEach(line => {
            expect(line).not.toContain('Probe binary not found');
            expect(line).not.toContain('downloading');
            expect(line).not.toContain('Binary downloaded successfully');
          });

          resolve();
        }, 2000);

        proc.on('error', reject);
      });
    }, 10000);
  });
});
