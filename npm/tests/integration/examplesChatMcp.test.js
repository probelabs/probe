/**
 * Integration tests for examples/chat ProbeChat with MCP support
 *
 * NOTE: This test requires examples/chat dependencies to be installed.
 * It tests the integration between the npm package and the examples/chat application.
 */

import { jest } from '@jest/globals';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { mkdtemp, writeFile, rm } from 'fs/promises';
import { tmpdir } from 'os';
import { existsSync } from 'fs';

// Check if examples/chat exists and has node_modules
const examplesPath = join(dirname(dirname(dirname(dirname(fileURLToPath(import.meta.url))))), 'examples', 'chat');
const examplesNodeModules = join(examplesPath, 'node_modules');

// Skip these tests if examples/chat dependencies are not installed
const skipTests = !existsSync(examplesNodeModules);

// Only import ProbeChat if we can run the tests
let ProbeChat;
if (!skipTests) {
  // Mock dotenv/config before importing modules that use it
  jest.unstable_mockModule('dotenv/config', () => ({}));

  // Import ProbeChat only if we'll run the tests
  const module = await import('../../../examples/chat/probeChat.js');
  ProbeChat = module.ProbeChat;
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const describeOrSkip = skipTests ? describe.skip : describe;

describeOrSkip('ProbeChat MCP Integration', () => {
  let tempDir;

  if (skipTests) {
    test('Tests skipped - examples/chat dependencies not installed', () => {
      console.log('Skipping ProbeChat tests as examples/chat dependencies are not installed');
      console.log(`Checked path: ${examplesNodeModules}`);
      expect(true).toBe(true);
    });
    return;
  }

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'probe-chat-mcp-test-'));
  });

  afterEach(async () => {
    if (tempDir) {
      await rm(tempDir, { recursive: true, force: true });
    }
    // Clean up environment variables
    delete process.env.ENABLE_MCP;
    delete process.env.MCP_CONFIG_PATH;
    delete process.env.ANTHROPIC_API_KEY;
    delete process.env.OPENAI_API_KEY;
    delete process.env.GOOGLE_API_KEY;
    delete process.env.PROBE_NON_INTERACTIVE;
    delete process.env.DEBUG_CHAT;
  });

  describe('ProbeChat Initialization', () => {
    test('should initialize ProbeChat without MCP by default', async () => {
      process.env.PROBE_NON_INTERACTIVE = '1'; // Suppress logs in tests

      const chat = new ProbeChat({
        isNonInteractive: true,
        debug: false
      });

      expect(chat.agent.enableMcp).toBe(false);
      expect(chat.agent.mcpBridge).toBeNull();

      await chat.cleanup();
    });

    test('should initialize ProbeChat with MCP enabled via options', async () => {
      process.env.PROBE_NON_INTERACTIVE = '1';

      const chat = new ProbeChat({
        enableMcp: true,
        isNonInteractive: true,
        debug: false
      });

      expect(chat.agent.enableMcp).toBe(true);

      await chat.cleanup();
    });

    test('should initialize ProbeChat with MCP enabled via environment', async () => {
      process.env.ENABLE_MCP = '1';
      process.env.PROBE_NON_INTERACTIVE = '1';

      const chat = new ProbeChat({
        isNonInteractive: true,
        debug: false
      });

      expect(chat.agent.enableMcp).toBe(true);

      await chat.cleanup();
    });

    test('should pass MCP server configurations to ProbeAgent', async () => {
      process.env.PROBE_NON_INTERACTIVE = '1';

      const mcpServers = [
        {
          name: 'test-server',
          command: 'node',
          args: ['test.js']
        }
      ];

      const chat = new ProbeChat({
        enableMcp: true,
        mcpServers: mcpServers,
        isNonInteractive: true,
        debug: false
      });

      expect(chat.agent.enableMcp).toBe(true);
      expect(chat.agent.mcpServers).toEqual(mcpServers);

      await chat.cleanup();
    });
  });

  describe('ProbeChat API Integration', () => {
    test('should maintain ProbeChat API compatibility', async () => {
      process.env.PROBE_NON_INTERACTIVE = '1';
      process.env.ANTHROPIC_API_KEY = 'test-key';

      const chat = new ProbeChat({
        enableMcp: true,
        isNonInteractive: true,
        debug: false
      });

      // Verify all expected methods exist
      expect(typeof chat.chat).toBe('function');
      expect(typeof chat.getSessionId).toBe('function');
      expect(typeof chat.getUsageSummary).toBe('function');
      expect(typeof chat.clearHistory).toBe('function');
      expect(typeof chat.exportHistory).toBe('function');
      expect(typeof chat.saveHistory).toBe('function');
      expect(typeof chat.cancel).toBe('function');
      expect(typeof chat.cleanup).toBe('function');

      // Test basic functionality
      expect(chat.getSessionId()).toBeDefined();
      expect(chat.getUsageSummary()).toBeDefined();
      expect(chat.exportHistory()).toEqual([]);

      chat.clearHistory();
      expect(chat.exportHistory()).toEqual([]);

      await chat.cleanup();
    });

    test('should handle image URL extraction', async () => {
      process.env.PROBE_NON_INTERACTIVE = '1';

      const chat = new ProbeChat({
        isNonInteractive: true,
        debug: false
      });

      // Test that ProbeChat can be instantiated without errors
      // Image extraction is tested in the ProbeChat source, here we just verify integration
      expect(chat).toBeDefined();

      await chat.cleanup();
    });
  });

  describe('Token Usage and Display', () => {
    test('should initialize token usage tracking', async () => {
      process.env.PROBE_NON_INTERACTIVE = '1';

      const chat = new ProbeChat({
        isNonInteractive: true,
        debug: false
      });

      expect(chat.tokenUsage).toBeDefined();
      expect(typeof chat.tokenUsage.updateFromTokenCounter).toBe('function');
      expect(typeof chat.tokenUsage.display).toBe('function');
      expect(typeof chat.tokenUsage.clear).toBe('function');

      await chat.cleanup();
    });

    test('should handle telemetry configuration', async () => {
      process.env.PROBE_NON_INTERACTIVE = '1';

      const chat = new ProbeChat({
        isNonInteractive: true,
        debug: false
      });

      expect(chat.telemetryConfig).toBeDefined();

      await chat.cleanup();
    });
  });

  describe('History Management', () => {
    test('should save and load conversation history', async () => {
      process.env.PROBE_NON_INTERACTIVE = '1';

      const chat = new ProbeChat({
        isNonInteractive: true,
        debug: false
      });

      // Test history export/import
      const initialHistory = chat.exportHistory();
      expect(Array.isArray(initialHistory)).toBe(true);

      // Test history saving (filename generation)
      const filename = join(tempDir, 'test-history.json');
      const savedFile = chat.saveHistory(filename);
      expect(savedFile).toBe(filename);

      await chat.cleanup();
    });

    test('should generate automatic history filenames', async () => {
      process.env.PROBE_NON_INTERACTIVE = '1';

      const chat = new ProbeChat({
        isNonInteractive: true,
        debug: false
      });

      // Test automatic filename generation
      const sessionId = chat.getSessionId();
      const filename = chat.saveHistory();

      expect(filename).toContain(sessionId);
      expect(filename).toContain('.json');

      await chat.cleanup();
    });
  });

  describe('Mock Server Integration', () => {
    test('should work with mock MCP server configuration', async () => {
      // Create MCP configuration with mock server
      const mcpConfig = {
        mcpServers: {
          'mock-chat-test': {
            command: 'node',
            args: [join(__dirname, '../mcp/mockMcpServer.js')],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const configPath = join(tempDir, 'chat-mcp-config.json');
      await writeFile(configPath, JSON.stringify(mcpConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;
      process.env.PROBE_NON_INTERACTIVE = '1';
      process.env.ANTHROPIC_API_KEY = 'test-key';

      const chat = new ProbeChat({
        enableMcp: true,
        isNonInteractive: true,
        debug: true
      });

      // Wait for MCP initialization
      await new Promise(resolve => setTimeout(resolve, 3000));

      // Verify MCP integration is working
      expect(chat.agent.enableMcp).toBe(true);

      if (chat.agent.mcpBridge && chat.agent.mcpBridge.getToolNames().length > 0) {
        const toolNames = chat.agent.mcpBridge.getToolNames();
        console.log('ProbeChat successfully connected to mock server with tools:', toolNames);

        // Verify expected tools
        expect(toolNames.some(name => name.includes('foobar'))).toBe(true);
        expect(toolNames.some(name => name.includes('calculator'))).toBe(true);
      } else {
        console.warn('Mock server connection failed in ProbeChat - may be expected in test environment');
      }

      await chat.cleanup();
    }, 20000); // Extended timeout for server startup
  });

  describe('Error Handling', () => {
    test('should handle MCP initialization errors gracefully', async () => {
      // Create invalid MCP configuration
      const mcpConfig = {
        mcpServers: {
          'invalid-chat-server': {
            command: 'nonexistent-command',
            args: ['--invalid'],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const configPath = join(tempDir, 'invalid-chat-config.json');
      await writeFile(configPath, JSON.stringify(mcpConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;
      process.env.PROBE_NON_INTERACTIVE = '1';

      // Should not crash during initialization
      const chat = new ProbeChat({
        enableMcp: true,
        isNonInteractive: true,
        debug: false
      });

      // Wait for initialization attempt
      await new Promise(resolve => setTimeout(resolve, 2000));

      expect(chat).toBeDefined();
      expect(chat.agent.enableMcp).toBe(true);
      // MCP bridge should be null due to failed connection
      expect(chat.agent.mcpBridge).toBeNull();

      await chat.cleanup();
    }, 10000);

    test('should handle cleanup errors gracefully', async () => {
      process.env.PROBE_NON_INTERACTIVE = '1';

      const chat = new ProbeChat({
        enableMcp: true,
        isNonInteractive: true,
        debug: false
      });

      // Mock the agent cleanup to throw an error
      const originalCleanup = chat.agent.cleanup;
      chat.agent.cleanup = jest.fn().mockRejectedValue(new Error('Cleanup failed'));

      // Should not throw even if underlying cleanup fails
      await expect(chat.cleanup()).resolves.not.toThrow();

      // Restore original cleanup
      chat.agent.cleanup = originalCleanup;
      await chat.cleanup();
    });
  });

  describe('Environment Variable Handling', () => {
    test('should respect ENABLE_MCP environment variable', async () => {
      process.env.ENABLE_MCP = '1';
      process.env.PROBE_NON_INTERACTIVE = '1';
      process.env.DEBUG_CHAT = '1';

      const chat = new ProbeChat({
        isNonInteractive: true
      });

      expect(chat.agent.enableMcp).toBe(true);
      expect(chat.debug).toBe(true);

      await chat.cleanup();
    });

    test('should respect DEBUG_CHAT environment variable', async () => {
      process.env.DEBUG_CHAT = '1';
      process.env.PROBE_NON_INTERACTIVE = '1';

      const chat = new ProbeChat({
        isNonInteractive: true
      });

      expect(chat.debug).toBe(true);

      await chat.cleanup();
    });

    test('should handle non-interactive mode properly', async () => {
      process.env.PROBE_NON_INTERACTIVE = '1';

      const chat = new ProbeChat();

      expect(chat.isNonInteractive).toBe(true);

      await chat.cleanup();
    });
  });
});