/**
 * Unit tests for MCP Configuration management
 */

import { jest } from '@jest/globals';
import {
  loadMCPConfiguration,
  parseEnabledServers,
  createSampleConfig,
  saveConfig
} from '../../src/agent/mcp/config.js';
import { join } from 'path';
import { mkdtemp, writeFile, rm } from 'fs/promises';
import { tmpdir } from 'os';

describe('MCP Configuration', () => {
  let tempDir;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'mcp-config-test-'));
  });

  afterEach(async () => {
    if (tempDir) {
      await rm(tempDir, { recursive: true, force: true });
    }
    // Clean up environment variables
    delete process.env.MCP_CONFIG_PATH;
    delete process.env.MCP_SERVERS_TEST_COMMAND;
    delete process.env.MCP_SERVERS_TEST_ARGS;
    delete process.env.MCP_SERVERS_TEST_ENABLED;
  });

  describe('Sample Configuration', () => {
    test('should create valid sample configuration', () => {
      const config = createSampleConfig();

      expect(config).toHaveProperty('mcpServers');
      expect(config).toHaveProperty('settings');
      expect(config.mcpServers).toHaveProperty('probe');
      expect(config.mcpServers).toHaveProperty('filesystem');
      expect(config.mcpServers).toHaveProperty('github');
      expect(config.mcpServers).toHaveProperty('postgres');
      expect(config.mcpServers).toHaveProperty('custom-http');
      expect(config.mcpServers).toHaveProperty('custom-websocket');

      // Check probe server configuration
      const probeServer = config.mcpServers.probe;
      expect(probeServer.command).toBe('npx');
      expect(probeServer.args).toContain('-y');
      expect(probeServer.args).toContain('@probelabs/probe@latest');
      expect(probeServer.transport).toBe('stdio');
      expect(probeServer.enabled).toBe(true);

      // Check HTTP server configuration
      const httpServer = config.mcpServers['custom-http'];
      expect(httpServer.url).toBe('http://localhost:3000/mcp');
      expect(httpServer.transport).toBe('http');
      expect(httpServer.enabled).toBe(false);

      // Check WebSocket server configuration
      const wsServer = config.mcpServers['custom-websocket'];
      expect(wsServer.url).toBe('ws://localhost:8080');
      expect(wsServer.transport).toBe('websocket');
      expect(wsServer.enabled).toBe(false);
    });
  });

  describe('Server Parsing', () => {
    test('should parse enabled servers correctly', () => {
      const config = {
        mcpServers: {
          'enabled-server': {
            command: 'node',
            args: ['test.js'],
            transport: 'stdio',
            enabled: true
          },
          'disabled-server': {
            command: 'node',
            args: ['test2.js'],
            transport: 'stdio',
            enabled: false
          },
          'default-enabled': {
            command: 'node',
            args: ['test3.js'],
            transport: 'stdio'
            // No enabled property - should be included
          }
        }
      };

      const servers = parseEnabledServers(config);

      expect(servers).toHaveLength(2);
      expect(servers[0].name).toBe('enabled-server');
      expect(servers[1].name).toBe('default-enabled');
    });

    test('should infer transport from URL', () => {
      const config = {
        mcpServers: {
          'ws-server': {
            url: 'ws://localhost:8080',
            enabled: true
          },
          'wss-server': {
            url: 'wss://localhost:8080',
            enabled: true
          },
          'sse-server': {
            url: 'http://localhost:3000/sse',
            enabled: true
          },
          'http-server': {
            url: 'http://localhost:3000/mcp',
            enabled: true
          }
        }
      };

      const servers = parseEnabledServers(config);

      expect(servers).toHaveLength(4);
      expect(servers.find(s => s.name === 'ws-server').transport).toBe('websocket');
      expect(servers.find(s => s.name === 'wss-server').transport).toBe('websocket');
      expect(servers.find(s => s.name === 'sse-server').transport).toBe('sse');
      expect(servers.find(s => s.name === 'http-server').transport).toBe('http');
    });

    test('should use default stdio transport', () => {
      const config = {
        mcpServers: {
          'stdio-server': {
            command: 'node',
            args: ['test.js'],
            enabled: true
          }
        }
      };

      const servers = parseEnabledServers(config);

      expect(servers).toHaveLength(1);
      expect(servers[0].transport).toBe('stdio');
    });

    test('should skip servers with missing required fields', () => {
      const config = {
        mcpServers: {
          'missing-command': {
            transport: 'stdio',
            enabled: true
            // Missing command
          },
          'missing-url': {
            transport: 'websocket',
            enabled: true
            // Missing URL
          },
          'valid-server': {
            command: 'node',
            args: ['test.js'],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const servers = parseEnabledServers(config);

      expect(servers).toHaveLength(1);
      expect(servers[0].name).toBe('valid-server');
    });

    test('should handle empty configuration', () => {
      expect(parseEnabledServers(null)).toEqual([]);
      expect(parseEnabledServers({})).toEqual([]);
      expect(parseEnabledServers({ mcpServers: {} })).toEqual([]);
    });
  });

  describe('Environment Variable Integration', () => {
    test('should merge environment variables', () => {
      // Set environment variables
      process.env.MCP_SERVERS_TEST_COMMAND = 'npm';
      process.env.MCP_SERVERS_TEST_ARGS = 'start,--verbose';
      process.env.MCP_SERVERS_TEST_ENABLED = 'true';
      process.env.MCP_SERVERS_TEST_URL = 'http://localhost:4000';

      const config = loadMCPConfiguration();

      expect(config.mcpServers).toHaveProperty('test');
      expect(config.mcpServers.test.command).toBe('npm');
      expect(config.mcpServers.test.args).toEqual(['start', '--verbose']);
      expect(config.mcpServers.test.enabled).toBe(true);
      expect(config.mcpServers.test.url).toBe('http://localhost:4000');
    });

    test('should handle boolean environment variables', () => {
      process.env.MCP_SERVERS_BOOL_TEST_ENABLED = '1';
      process.env.MCP_SERVERS_BOOL_TEST2_ENABLED = 'false';

      const config = loadMCPConfiguration();

      expect(config.mcpServers['bool-test'].enabled).toBe(true);
      expect(config.mcpServers['bool-test2'].enabled).toBe(false);
    });

    test('should normalize server names from environment', () => {
      process.env.MCP_SERVERS_MY_CUSTOM_SERVER_COMMAND = 'node';
      process.env.MCP_SERVERS_MY_CUSTOM_SERVER_ENABLED = 'true';

      const config = loadMCPConfiguration();

      expect(config.mcpServers).toHaveProperty('my-custom-server');
      expect(config.mcpServers['my-custom-server'].command).toBe('node');
    });
  });

  describe('Configuration Loading', () => {
    test('should load configuration from file', async () => {
      const configData = {
        mcpServers: {
          'file-server': {
            command: 'node',
            args: ['server.js'],
            transport: 'stdio',
            enabled: true
          }
        }
      };

      const configPath = join(tempDir, 'mcp.json');
      await writeFile(configPath, JSON.stringify(configData, null, 2));

      // Set environment variable to point to our test config
      process.env.MCP_CONFIG_PATH = configPath;

      const config = loadMCPConfiguration();

      expect(config.mcpServers).toHaveProperty('file-server');
      expect(config.mcpServers['file-server'].command).toBe('node');
    });

    test('should handle invalid JSON in config file', async () => {
      const configPath = join(tempDir, 'invalid.json');
      await writeFile(configPath, '{ invalid json }');

      process.env.MCP_CONFIG_PATH = configPath;

      // Should not throw, should fall back to default
      const config = loadMCPConfiguration();
      expect(config).toBeDefined();
    });

    test('should use default configuration when no file exists', () => {
      process.env.MCP_CONFIG_PATH = join(tempDir, 'nonexistent.json');

      const config = loadMCPConfiguration();

      expect(config).toBeDefined();
      expect(config.mcpServers).toBeDefined();
    });
  });

  describe('Configuration Saving', () => {
    test('should save configuration to file', async () => {
      const config = createSampleConfig();
      const configPath = join(tempDir, 'saved-config.json');

      // Mock console.log to capture output
      const originalLog = console.log;
      console.log = jest.fn();

      try {
        saveConfig(config, configPath);

        // Read the file back
        const { readFile } = await import('fs/promises');
        const savedContent = await readFile(configPath, 'utf8');
        const parsedConfig = JSON.parse(savedContent);

        expect(parsedConfig).toEqual(config);
        expect(console.log).toHaveBeenCalledWith(`[MCP] Configuration saved to: ${configPath}`);
      } finally {
        console.log = originalLog;
      }
    });

    test('should create directory if it does not exist', async () => {
      const config = createSampleConfig();
      const nestedPath = join(tempDir, 'nested', 'config.json');

      saveConfig(config, nestedPath);

      // Verify file was created
      const { readFile } = await import('fs/promises');
      const savedContent = await readFile(nestedPath, 'utf8');
      const parsedConfig = JSON.parse(savedContent);

      expect(parsedConfig).toEqual(config);
    });
  });

  describe('Configuration Validation', () => {
    test('should handle configuration with custom environment variables', () => {
      const config = {
        mcpServers: {
          'custom-env': {
            command: 'node',
            args: ['server.js'],
            transport: 'stdio',
            enabled: true,
            env: {
              CUSTOM_VAR: 'value',
              DATABASE_URL: 'postgres://localhost/test'
            }
          }
        }
      };

      const servers = parseEnabledServers(config);

      expect(servers).toHaveLength(1);
      expect(servers[0].env).toEqual({
        CUSTOM_VAR: 'value',
        DATABASE_URL: 'postgres://localhost/test'
      });
    });

    test('should handle environment variable for custom env settings', () => {
      process.env.MCP_SERVERS_ENV_TEST_ENV = '{"API_KEY": "secret", "DEBUG": "true"}';
      process.env.MCP_SERVERS_ENV_TEST_COMMAND = 'node';
      process.env.MCP_SERVERS_ENV_TEST_ENABLED = 'true';

      const config = loadMCPConfiguration();

      expect(config.mcpServers['env-test'].env).toEqual({
        API_KEY: 'secret',
        DEBUG: 'true'
      });
    });

    test('should handle invalid JSON in environment env variable', () => {
      process.env.MCP_SERVERS_BAD_ENV_ENV = 'invalid json';
      process.env.MCP_SERVERS_BAD_ENV_COMMAND = 'node';
      process.env.MCP_SERVERS_BAD_ENV_ENABLED = 'true';

      const config = loadMCPConfiguration();

      // Should default to the property name and value
      expect(config.mcpServers['bad-env'].env).toEqual({
        ENV: 'invalid json'
      });
    });
  });
});