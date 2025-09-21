/**
 * MCP Client implementation for Probe Agent
 * Allows Probe to connect to MCP servers and use their tools
 */

import { experimental_createMCPClient } from 'ai';
import {
  StdioClientTransport
} from '@modelcontextprotocol/sdk/client/stdio.js';
import {
  WebSocketClientTransport
} from '@modelcontextprotocol/sdk/client/websocket.js';

/**
 * Create MCP client from configuration
 * @param {Object} config - MCP client configuration
 * @param {string} config.type - Transport type ('stdio', 'http', 'sse')
 * @param {string} [config.command] - Command to run (for stdio)
 * @param {string[]} [config.args] - Command arguments (for stdio)
 * @param {string} [config.url] - URL for HTTP/SSE transport
 * @returns {Promise<Object>} MCP client with tools
 */
export async function createMCPClient(config) {
  let transport;

  switch (config.type) {
    case 'stdio':
      if (!config.command) {
        throw new Error('Command is required for stdio transport');
      }
      transport = new StdioClientTransport({
        command: config.command,
        args: config.args || []
      });
      break;

    case 'websocket':
    case 'ws':
      if (!config.url) {
        throw new Error('URL is required for WebSocket transport');
      }
      transport = new WebSocketClientTransport(new URL(config.url));
      break;

    default:
      throw new Error(`Unknown transport type: ${config.type}`);
  }

  const client = await experimental_createMCPClient({
    transport,
    name: config.name || 'probe-mcp-client',
    version: '1.0.0'
  });

  return client;
}

/**
 * Connect to multiple MCP servers and combine their tools
 * @param {Array<Object>} configs - Array of MCP server configurations
 * @returns {Promise<Object>} Combined tools from all servers
 */
export async function connectToMCPServers(configs = []) {
  const clients = [];
  const allTools = {};

  for (const config of configs) {
    try {
      console.error(`Connecting to MCP server: ${config.name || config.url || config.command}`);
      const client = await createMCPClient(config);
      clients.push(client);

      // Get tools from this server
      const tools = await client.tools();

      // Add tools with a prefix to avoid conflicts
      const prefix = config.prefix || '';
      for (const [toolName, tool] of Object.entries(tools)) {
        const prefixedName = prefix ? `${prefix}_${toolName}` : toolName;
        allTools[prefixedName] = tool;
      }

      console.error(`Successfully connected to ${config.name || 'MCP server'} with ${Object.keys(tools).length} tools`);
    } catch (error) {
      console.error(`Failed to connect to MCP server ${config.name || config.url || config.command}:`, error.message);
      // Continue with other servers even if one fails
    }
  }

  return {
    tools: allTools,
    clients,
    disconnect: async () => {
      for (const client of clients) {
        try {
          await client.disconnect();
        } catch (error) {
          console.error('Error disconnecting MCP client:', error);
        }
      }
    }
  };
}

/**
 * Load MCP configuration from environment or config file
 * @returns {Array<Object>} Array of MCP server configurations
 */
export function loadMCPConfig() {
  const configs = [];

  // Check environment variable for MCP servers
  if (process.env.MCP_SERVERS) {
    try {
      const serversConfig = JSON.parse(process.env.MCP_SERVERS);
      configs.push(...serversConfig);
    } catch (error) {
      console.error('Failed to parse MCP_SERVERS environment variable:', error);
    }
  }

  // Example default configuration for common MCP servers
  if (process.env.MCP_FILESYSTEM_SERVER) {
    configs.push({
      name: 'filesystem',
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-filesystem', process.cwd()],
      prefix: 'fs'
    });
  }

  if (process.env.MCP_GITHUB_SERVER) {
    configs.push({
      name: 'github',
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-github'],
      prefix: 'gh'
    });
  }

  if (process.env.MCP_POSTGRES_SERVER) {
    configs.push({
      name: 'postgres',
      type: 'stdio',
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-postgres', process.env.DATABASE_URL],
      prefix: 'db'
    });
  }

  // Custom MCP server URLs from environment
  if (process.env.MCP_HTTP_SERVERS) {
    const urls = process.env.MCP_HTTP_SERVERS.split(',');
    urls.forEach((url, index) => {
      configs.push({
        name: `http-server-${index}`,
        type: 'http',
        url: url.trim()
      });
    });
  }

  return configs;
}

/**
 * Example usage function
 */
export async function example() {
  // Load configuration from environment
  const configs = loadMCPConfig();

  // Or manually specify servers
  const manualConfigs = [
    {
      name: 'probe-server',
      type: 'stdio',
      command: 'node',
      args: ['/home/buger/projects/probe/examples/chat/mcpServer.js']
    },
    {
      name: 'custom-http-server',
      type: 'http',
      url: 'http://localhost:3000/mcp'
    }
  ];

  // Connect to all configured MCP servers
  const { tools, disconnect } = await connectToMCPServers(configs.length > 0 ? configs : manualConfigs);

  console.log('Available MCP tools:', Object.keys(tools));

  // Use the tools with AI SDK
  // Example: use with generateText
  /*
  const result = await generateText({
    model: openai('gpt-4'),
    tools,
    messages: [
      {
        role: 'user',
        content: 'Search for authentication code in the project'
      }
    ]
  });
  */

  // Clean up when done
  await disconnect();
}

// Export for use in ProbeChat
export default {
  createMCPClient,
  connectToMCPServers,
  loadMCPConfig
};