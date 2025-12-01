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

    // Always log the number of servers found
    console.error(`[MCP INFO] Found ${servers.length} enabled MCP server${servers.length !== 1 ? 's' : ''}`);

    if (servers.length === 0) {
      console.error('[MCP INFO] No MCP servers configured or enabled');
      console.error('[MCP INFO] 0 MCP tools available');
      return {
        connected: 0,
        total: 0,
        tools: []
      };
    }

    if (this.debug) {
      console.error('[MCP DEBUG] Server details:');
      servers.forEach(server => {
        console.error(`[MCP DEBUG]   - ${server.name} (${server.transport})`);
      });
    }

    // Connect to each enabled server
    const connectionPromises = servers.map(server =>
      this.connectToServer(server).catch(error => {
        console.error(`[MCP ERROR] Failed to connect to ${server.name}:`, error.message);
        return null;
      })
    );

    const results = await Promise.all(connectionPromises);
    const connectedCount = results.filter(Boolean).length;

    // Always log connection results
    if (connectedCount === 0) {
      console.error(`[MCP ERROR] Failed to connect to all ${servers.length} server${servers.length !== 1 ? 's' : ''}`);
      console.error('[MCP INFO] 0 MCP tools available');
    } else if (connectedCount < servers.length) {
      console.error(`[MCP INFO] Successfully connected to ${connectedCount}/${servers.length} servers`);
      console.error(`[MCP INFO] ${this.tools.size} MCP tool${this.tools.size !== 1 ? 's' : ''} available`);
    } else {
      console.error(`[MCP INFO] Successfully connected to all ${connectedCount} server${connectedCount !== 1 ? 's' : ''}`);
      console.error(`[MCP INFO] ${this.tools.size} MCP tool${this.tools.size !== 1 ? 's' : ''} available`);
    }

    if (this.debug && this.tools.size > 0) {
      console.error('[MCP DEBUG] Available tools:');
      Array.from(this.tools.keys()).forEach(toolName => {
        console.error(`[MCP DEBUG]   - ${toolName}`);
      });
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
        console.error(`[MCP DEBUG] Connecting to ${name} via ${serverConfig.transport}...`);
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
      const toolCount = toolsResponse?.tools?.length || 0;

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
            console.error(`[MCP DEBUG]     Registered tool: ${qualifiedName}`);
          }
        }
      }

      console.error(`[MCP INFO] Connected to ${name}: ${toolCount} tool${toolCount !== 1 ? 's' : ''} loaded`);

      return true;
    } catch (error) {
      console.error(`[MCP ERROR] Error connecting to ${name}:`, error.message);
      if (this.debug) {
        console.error(`[MCP DEBUG] Full error details:`, error);
      }
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
        console.error(`[MCP DEBUG] Calling ${toolName} with args:`, JSON.stringify(args, null, 2));
      }

      // Get timeout: per-server timeout takes priority over global timeout (default 30 seconds)
      // Validate timeout values to prevent resource exhaustion
      const DEFAULT_TIMEOUT = 30000;
      const MAX_TIMEOUT = 600000; // 10 minutes max to prevent resource exhaustion

      const validateTimeout = (value) => {
        if (value === undefined || value === null) return undefined;
        const num = Number(value);
        if (!Number.isFinite(num) || num < 0) return undefined; // Invalid, use fallback
        return Math.min(num, MAX_TIMEOUT); // Cap at max timeout
      };

      const serverTimeout = validateTimeout(clientInfo.config?.timeout);
      const globalTimeout = validateTimeout(this.config?.settings?.timeout) ?? DEFAULT_TIMEOUT;
      const timeout = serverTimeout ?? globalTimeout;

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

      if (this.debug) {
        console.error(`[MCP DEBUG] Tool ${toolName} executed successfully`);
      }

      return result;
    } catch (error) {
      console.error(`[MCP ERROR] Error calling tool ${toolName}:`, error.message);
      if (this.debug) {
        console.error(`[MCP DEBUG] Full error details:`, error);
      }
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

    if (this.clients.size === 0) {
      if (this.debug) {
        console.error('[MCP DEBUG] No MCP clients to disconnect');
      }
      return;
    }

    if (this.debug) {
      console.error(`[MCP DEBUG] Disconnecting from ${this.clients.size} MCP server${this.clients.size !== 1 ? 's' : ''}...`);
    }

    for (const [name, clientInfo] of this.clients.entries()) {
      disconnectPromises.push(
        clientInfo.client.close()
          .then(() => {
            if (this.debug) {
              console.error(`[MCP DEBUG] Disconnected from ${name}`);
            }
          })
          .catch(error => {
            console.error(`[MCP ERROR] Error disconnecting from ${name}:`, error.message);
          })
      );
    }

    await Promise.all(disconnectPromises);
    this.clients.clear();
    this.tools.clear();

    if (this.debug) {
      console.error('[MCP DEBUG] All MCP connections closed');
    }
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