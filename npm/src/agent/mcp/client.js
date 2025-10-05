/**
 * Enhanced MCP Client with support for all transport types
 * Compatible with Claude's MCP configuration format
 */

import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { SSEClientTransport } from '@modelcontextprotocol/sdk/client/sse.js';
import { WebSocketClientTransport } from '@modelcontextprotocol/sdk/client/websocket.js';
import { loadMCPConfiguration, parseEnabledServers } from './config.js';

/**
 * Create transport based on configuration
 * @param {Object} serverConfig - Server configuration
 * @returns {Object} Transport instance
 */
export function createTransport(serverConfig) {
  const { transport, command, args, url, env } = serverConfig;

  switch (transport) {
    case 'stdio':
      return new StdioClientTransport({
        command,
        args: args || [],
        env: env ? { ...process.env, ...env } : undefined
      });

    case 'sse':
      if (!url) {
        throw new Error('SSE transport requires a URL');
      }
      return new SSEClientTransport(new URL(url));

    case 'websocket':
    case 'ws':
      if (!url) {
        throw new Error('WebSocket transport requires a URL');
      }
      try {
        return new WebSocketClientTransport(new URL(url));
      } catch (error) {
        throw new Error(`Invalid WebSocket URL: ${url}`);
      }

    case 'http':
    case 'streamable':
      // For HTTP, we'll use a custom implementation since the SDK
      // doesn't provide a direct HTTP transport yet
      if (!url) {
        throw new Error('HTTP transport requires a URL');
      }
      // Return a custom HTTP transport wrapper
      return createHttpTransport(url);

    default:
      throw new Error(`Unknown transport type: ${transport}`);
  }
}

/**
 * Create a custom HTTP transport wrapper
 * This simulates MCP over HTTP REST endpoints
 */
function createHttpTransport(url) {
  // This is a simplified HTTP transport
  // In practice, you'd implement the full MCP protocol over HTTP
  return {
    async start() {
      // Initialize HTTP connection
      const response = await fetch(`${url}/initialize`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          protocolVersion: '2024-11-05',
          capabilities: {}
        })
      });

      if (!response.ok) {
        throw new Error(`HTTP initialization failed: ${response.statusText}`);
      }

      return response.json();
    },

    async send(message) {
      const response = await fetch(`${url}/message`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(message)
      });

      if (!response.ok) {
        throw new Error(`HTTP request failed: ${response.statusText}`);
      }

      return response.json();
    },

    async close() {
      // Close HTTP connection
      await fetch(`${url}/close`, {
        method: 'POST'
      }).catch(() => {
        // Ignore close errors
      });
    }
  };
}

/**
 * MCP Client Manager - manages multiple MCP server connections
 */
export class MCPClientManager {
  constructor(options = {}) {
    this.clients = new Map();
    this.tools = new Map();
    this.debug = options.debug || process.env.DEBUG_MCP === '1';
    this.config = null;
  }

  /**
   * Initialize MCP clients from configuration
   * @param {Object} config - Optional configuration override
   */
  async initialize(config = null) {
    // Load configuration
    this.config = config || loadMCPConfiguration();
    const servers = parseEnabledServers(this.config);

    if (this.debug) {
      console.error(`[MCP] Found ${servers.length} enabled servers`);
    }

    // Connect to each enabled server
    const connectionPromises = servers.map(server =>
      this.connectToServer(server).catch(error => {
        console.error(`[MCP] Failed to connect to ${server.name}:`, error.message);
        return null;
      })
    );

    const results = await Promise.all(connectionPromises);
    const connectedCount = results.filter(Boolean).length;

    if (this.debug) {
      console.error(`[MCP] Successfully connected to ${connectedCount}/${servers.length} servers`);
      console.error(`[MCP] Total tools available: ${this.tools.size}`);
    }

    return {
      connected: connectedCount,
      total: servers.length,
      tools: Array.from(this.tools.keys())
    };
  }

  /**
   * Connect to a single MCP server
   * @param {Object} serverConfig - Server configuration
   */
  async connectToServer(serverConfig) {
    const { name } = serverConfig;

    try {
      if (this.debug) {
        console.error(`[MCP] Connecting to ${name} via ${serverConfig.transport}...`);
      }

      // Create transport
      const transport = createTransport(serverConfig);

      // Create client
      const client = new Client(
        {
          name: `probe-client-${name}`,
          version: '1.0.0'
        },
        {
          capabilities: {}
        }
      );

      // Connect
      await client.connect(transport);

      // Store client
      this.clients.set(name, {
        client,
        transport,
        config: serverConfig
      });

      // Fetch and register tools
      const toolsResponse = await client.listTools();

      if (toolsResponse && toolsResponse.tools) {
        for (const tool of toolsResponse.tools) {
          // Add server prefix to avoid conflicts
          const qualifiedName = `${name}_${tool.name}`;
          this.tools.set(qualifiedName, {
            ...tool,
            serverName: name,
            originalName: tool.name
          });

          if (this.debug) {
            console.error(`[MCP]   Registered tool: ${qualifiedName}`);
          }
        }
      }

      if (this.debug) {
        console.error(`[MCP] Connected to ${name} with ${toolsResponse?.tools?.length || 0} tools`);
      }

      return true;
    } catch (error) {
      console.error(`[MCP] Error connecting to ${name}:`, error.message);
      return false;
    }
  }

  /**
   * Call a tool on its respective server
   * @param {string} toolName - Qualified tool name (server_tool)
   * @param {Object} args - Tool arguments
   */
  async callTool(toolName, args) {
    const tool = this.tools.get(toolName);
    if (!tool) {
      throw new Error(`Unknown tool: ${toolName}`);
    }

    const clientInfo = this.clients.get(tool.serverName);
    if (!clientInfo) {
      throw new Error(`Server ${tool.serverName} not connected`);
    }

    try {
      if (this.debug) {
        console.error(`[MCP] Calling ${toolName} with args:`, args);
      }

      // Get timeout from config (default 30 seconds)
      const timeout = this.config?.settings?.timeout || 30000;

      // Create a timeout promise
      const timeoutPromise = new Promise((_, reject) => {
        setTimeout(() => {
          reject(new Error(`MCP tool call timeout after ${timeout}ms`));
        }, timeout);
      });

      // Race between the actual call and timeout
      const result = await Promise.race([
        clientInfo.client.callTool({
          name: tool.originalName,
          arguments: args
        }),
        timeoutPromise
      ]);

      return result;
    } catch (error) {
      console.error(`[MCP] Error calling tool ${toolName}:`, error);
      throw error;
    }
  }

  /**
   * Get all available tools with their schemas
   * @returns {Object} Map of tool name to tool definition
   */
  getTools() {
    const tools = {};
    for (const [name, tool] of this.tools.entries()) {
      tools[name] = {
        description: tool.description,
        inputSchema: tool.inputSchema,
        serverName: tool.serverName
      };
    }
    return tools;
  }

  /**
   * Get tools formatted for Vercel AI SDK
   * @returns {Object} Tools in Vercel AI SDK format
   */
  getVercelTools() {
    const tools = {};

    for (const [name, tool] of this.tools.entries()) {
      // Create a wrapper that calls the MCP tool
      tools[name] = {
        description: tool.description,
        inputSchema: tool.inputSchema,
        execute: async (args) => {
          const result = await this.callTool(name, args);
          // Extract text content from MCP response
          if (result.content && result.content[0]) {
            return result.content[0].text;
          }
          return JSON.stringify(result);
        }
      };
    }

    return tools;
  }

  /**
   * Disconnect all clients
   */
  async disconnect() {
    const disconnectPromises = [];

    for (const [name, clientInfo] of this.clients.entries()) {
      disconnectPromises.push(
        clientInfo.client.close()
          .then(() => {
            if (this.debug) {
              console.error(`[MCP] Disconnected from ${name}`);
            }
          })
          .catch(error => {
            console.error(`[MCP] Error disconnecting from ${name}:`, error);
          })
      );
    }

    await Promise.all(disconnectPromises);
    this.clients.clear();
    this.tools.clear();
  }
}

/**
 * Create and initialize MCP client manager with default configuration
 */
export async function createMCPManager(options = {}) {
  const manager = new MCPClientManager(options);
  await manager.initialize();
  return manager;
}

export default {
  MCPClientManager,
  createMCPManager,
  createTransport
};