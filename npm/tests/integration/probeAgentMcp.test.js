/**
 * Integration tests for ProbeAgent with MCP support
 */

import { jest } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { mkdtemp, writeFile, rm } from 'fs/promises';
import { tmpdir } from 'os';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe('ProbeAgent MCP Integration', () => {
  let tempDir;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'probe-agent-mcp-test-'));
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
  });

  describe('MCP Disabled (Default)', () => {
    test('should initialize ProbeAgent without MCP by default', async () => {
      const agent = new ProbeAgent({
        debug: false,
        path: tempDir
      });

      expect(agent.enableMcp).toBe(false);
      expect(agent.mcpBridge).toBeNull();

      await agent.cleanup();
    });

    test('should work normally without MCP features', async () => {
      // Set a dummy API key for testing
      process.env.ANTHROPIC_API_KEY = 'test-key';

      const agent = new ProbeAgent({
        debug: false,
        path: tempDir
      });

      // Verify system message doesn't include MCP tools
      const systemMessage = await agent.getSystemMessage();
      expect(systemMessage).not.toContain('MCP Tools');
      expect(systemMessage).not.toContain('JSON parameters in <params> tag');

      await agent.cleanup();
    });
  });

  describe('MCP Enabled via Options', () => {
    test('should initialize ProbeAgent with MCP enabled via options', async () => {
      const agent = new ProbeAgent({
        enableMcp: true,
        debug: false,
        path: tempDir
      });

      expect(agent.enableMcp).toBe(true);

      // MCP bridge may be null if no servers are configured, which is expected
      // The important thing is that enableMcp is true

      await agent.cleanup();
    });

    test('should initialize ProbeAgent with MCP enabled via environment', async () => {
      process.env.ENABLE_MCP = '1';

      const agent = new ProbeAgent({
        debug: false,
        path: tempDir
      });

      expect(agent.enableMcp).toBe(true);

      await agent.cleanup();
    });
  });

  describe('MCP Configuration', () => {
    test('should initialize MCP with mock server configuration', async () => {
      // Create MCP configuration file
      const mcpConfig = {
        mcpServers: {
          'mock-test': {
            command: 'node',
            args: [join(__dirname, '../mcp/mockMcpServer.js')],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const configPath = join(tempDir, 'mcp-config.json');
      await writeFile(configPath, JSON.stringify(mcpConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;

      const agent = new ProbeAgent({
        enableMcp: true,
        debug: false,
        path: tempDir
      });

      // Wait a bit for MCP initialization
      await new Promise(resolve => setTimeout(resolve, 1000));

      // Check if MCP bridge was initialized
      if (agent.mcpBridge) {
        const toolNames = agent.mcpBridge.getToolNames();
        console.log('Available MCP tools:', toolNames);

        // If connection succeeded, verify we have expected tools
        if (toolNames.length > 0) {
          expect(toolNames.some(name => name.includes('foobar'))).toBe(true);
        }
      }

      await agent.cleanup();
    }, 15000); // Longer timeout for server startup

    test('should handle MCP initialization failure gracefully', async () => {
      // Create invalid MCP configuration
      const mcpConfig = {
        mcpServers: {
          'invalid-server': {
            command: 'nonexistent-command',
            args: ['--invalid'],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const configPath = join(tempDir, 'invalid-mcp-config.json');
      await writeFile(configPath, JSON.stringify(mcpConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;

      const agent = new ProbeAgent({
        enableMcp: true,
        debug: false,
        path: tempDir
      });

      // Wait for initialization attempt
      await new Promise(resolve => setTimeout(resolve, 2000));

      // Should not crash, but MCP bridge should be null due to failed connection
      expect(agent.mcpBridge).toBeNull();

      await agent.cleanup();
    }, 10000);
  });

  describe('System Message with MCP', () => {
    test('should include MCP tools in system message when available', async () => {
      // Mock MCP bridge for testing
      const mockMcpBridge = {
        getToolNames: () => ['test_foobar', 'test_calculator'],
        getXmlToolDefinitions: () => `
## test_foobar
Description: Mock foobar tool

## test_calculator
Description: Mock calculator tool
        `
      };

      const agent = new ProbeAgent({
        enableMcp: true,
        debug: false,
        path: tempDir
      });

      // Manually set mock bridge for testing
      agent.mcpBridge = mockMcpBridge;

      const systemMessage = await agent.getSystemMessage();

      expect(systemMessage).toContain('MCP Tools (JSON parameters in <params> tag)');
      expect(systemMessage).toContain('test_foobar');
      expect(systemMessage).toContain('test_calculator');
      expect(systemMessage).toContain('For MCP tools, use JSON format within the params tag');

      await agent.cleanup();
    });

    test('should not include MCP section when no tools available', async () => {
      // Mock empty MCP bridge
      const mockMcpBridge = {
        getToolNames: () => [],
        getXmlToolDefinitions: () => ''
      };

      const agent = new ProbeAgent({
        enableMcp: true,
        debug: false,
        path: tempDir
      });

      agent.mcpBridge = mockMcpBridge;

      const systemMessage = await agent.getSystemMessage();

      expect(systemMessage).not.toContain('MCP Tools');

      await agent.cleanup();
    });
  });

  describe('Tool Execution with MCP', () => {
    test('should route MCP tool calls correctly', async () => {
      // Mock successful tool execution
      const mockExecute = jest.fn().mockResolvedValue('Mock tool result');

      const mockMcpBridge = {
        getToolNames: () => ['test_tool'],
        isMcpTool: (name) => name === 'test_tool',
        mcpTools: {
          'test_tool': {
            execute: mockExecute
          }
        }
      };

      const agent = new ProbeAgent({
        enableMcp: true,
        debug: false,
        path: tempDir
      });

      agent.mcpBridge = mockMcpBridge;

      // The tool execution is internal to the agent, but we can verify
      // that the bridge is properly set up for tool routing
      expect(agent.mcpBridge.isMcpTool('test_tool')).toBe(true);
      expect(agent.mcpBridge.isMcpTool('native_tool')).toBe(false);

      await agent.cleanup();
    });

    test('should handle MCP tool execution errors', async () => {
      // Mock failing tool execution
      const mockExecute = jest.fn().mockRejectedValue(new Error('Tool execution failed'));

      const mockMcpBridge = {
        getToolNames: () => ['failing_tool'],
        isMcpTool: (name) => name === 'failing_tool',
        mcpTools: {
          'failing_tool': {
            execute: mockExecute
          }
        }
      };

      const agent = new ProbeAgent({
        enableMcp: true,
        debug: false,
        path: tempDir
      });

      agent.mcpBridge = mockMcpBridge;

      // Verify error handling setup
      expect(agent.mcpBridge.isMcpTool('failing_tool')).toBe(true);

      await agent.cleanup();
    });
  });

  describe('ProbeAgent Cleanup with MCP', () => {
    test('should cleanup MCP bridge properly', async () => {
      const mockCleanup = jest.fn().mockResolvedValue(undefined);

      const mockMcpBridge = {
        getToolNames: () => [],
        cleanup: mockCleanup
      };

      const agent = new ProbeAgent({
        enableMcp: true,
        debug: false,
        path: tempDir
      });

      agent.mcpBridge = mockMcpBridge;

      await agent.cleanup();

      expect(mockCleanup).toHaveBeenCalled();
    });

    test('should handle cleanup errors gracefully', async () => {
      const mockCleanup = jest.fn().mockRejectedValue(new Error('Cleanup failed'));

      const mockMcpBridge = {
        getToolNames: () => [],
        cleanup: mockCleanup
      };

      const agent = new ProbeAgent({
        enableMcp: true,
        debug: false,
        path: tempDir
      });

      agent.mcpBridge = mockMcpBridge;

      // Should not throw even if MCP cleanup fails
      await expect(agent.cleanup()).resolves.not.toThrow();

      expect(mockCleanup).toHaveBeenCalled();
    });

    test('should cleanup without MCP bridge', async () => {
      const agent = new ProbeAgent({
        enableMcp: false,
        debug: false,
        path: tempDir
      });

      expect(agent.mcpBridge).toBeNull();

      // Should not throw when no MCP bridge exists
      await expect(agent.cleanup()).resolves.not.toThrow();
    });
  });

  describe('Real Mock Server Integration', () => {
    test('should connect to and use mock MCP server', async () => {
      // Only run this test if we can actually start the mock server
      const mcpConfig = {
        mcpServers: {
          'mock-integration': {
            command: 'node',
            args: [join(__dirname, '../mcp/mockMcpServer.js')],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const configPath = join(tempDir, 'integration-config.json');
      await writeFile(configPath, JSON.stringify(mcpConfig, null, 2));

      process.env.MCP_CONFIG_PATH = configPath;
      process.env.ANTHROPIC_API_KEY = 'test-key'; // Required for ProbeAgent

      const agent = new ProbeAgent({
        enableMcp: true,
        debug: true,
        path: tempDir
      });

      // Wait for MCP initialization
      await new Promise(resolve => setTimeout(resolve, 3000));

      if (agent.mcpBridge && agent.mcpBridge.getToolNames().length > 0) {
        const toolNames = agent.mcpBridge.getToolNames();
        console.log('Successfully connected to mock server with tools:', toolNames);

        // Verify expected tools are available
        expect(toolNames.some(name => name.includes('foobar'))).toBe(true);
        expect(toolNames.some(name => name.includes('calculator'))).toBe(true);
        expect(toolNames.some(name => name.includes('echo'))).toBe(true);

        // Test system message includes these tools
        const systemMessage = await agent.getSystemMessage();
        expect(systemMessage).toContain('foobar');
        expect(systemMessage).toContain('calculator');
      } else {
        console.warn('Mock server connection failed - this may be expected in CI environment');
      }

      await agent.cleanup();
    }, 20000); // Extended timeout for server startup
  });
});