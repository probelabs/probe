/**
 * MCP Bridge - manages MCP tool connections and provides Vercel-compatible tools
 */

import { MCPClientManager } from './client.js';
import { loadMCPConfiguration } from './config.js';

/**
 * Convert MCP tool to description string (for debug logging)
 * @param {string} name - Tool name
 * @param {Object} tool - MCP tool object
 * @returns {string} Description of the tool
 */
export function mcpToolToDescription(name, tool) {
  const description = tool.description || 'MCP tool';
  const inputSchema = tool.inputSchema || tool.parameters || {};

  let paramDocs = '';
  if (inputSchema.properties) {
    paramDocs = '\nParameters:';
    for (const [paramName, paramSchema] of Object.entries(inputSchema.properties)) {
      const required = inputSchema.required?.includes(paramName) ? ' (required)' : ' (optional)';
      const desc = paramSchema.description || '';
      const type = paramSchema.type || 'any';
      paramDocs += `\n- ${paramName}: ${type}${required} - ${desc}`;
    }
  }

  return `## ${name}\nDescription: ${description}${paramDocs}`;
}

/**
 * MCP Bridge - manages MCP connections and provides native Vercel AI SDK tools
 */
export class MCPXmlBridge {
  constructor(options = {}) {
    this.debug = options.debug || false;
    this.tracer = options.tracer || null;
    this.agentEvents = options.agentEvents || null;
    this.mcpTools = {};
    this.mcpManager = null;
    this.toolDescriptions = {};
  }

  /**
   * Initialize MCP connections and load tools
   * @param {Object|Array<Object>} config - MCP configuration object or server configurations (deprecated)
   */
  async initialize(config = null) {
    let mcpConfigs = null;

    if (!config) {
      if (this.debug) {
        console.error('[MCP DEBUG] No config provided, attempting auto-discovery...');
      }
      mcpConfigs = loadMCPConfiguration();

      if (!mcpConfigs || !mcpConfigs.mcpServers || Object.keys(mcpConfigs.mcpServers).length === 0) {
        console.error('[MCP WARNING] MCP enabled but no configuration found');
        console.error('[MCP INFO] To use MCP, provide configuration via:');
        console.error('[MCP INFO]   - mcpConfig option when creating ProbeAgent');
        console.error('[MCP INFO]   - mcpConfigPath option pointing to a config file');
        console.error('[MCP INFO]   - Config file in standard locations (~/.mcp/config.json, etc.)');
        console.error('[MCP INFO]   - Environment variable MCP_CONFIG_PATH');
      }
    } else if (Array.isArray(config)) {
      if (this.debug) {
        console.error('[MCP DEBUG] Using deprecated array config format (consider using mcpConfig object)');
      }
      mcpConfigs = { mcpServers: config };
    } else {
      if (this.debug) {
        console.error('[MCP DEBUG] Using provided MCP config object');
      }
      mcpConfigs = config;
    }

    if (!mcpConfigs || !mcpConfigs.mcpServers || Object.keys(mcpConfigs.mcpServers).length === 0) {
      console.error('[MCP INFO] 0 MCP tools available');
      return;
    }

    try {
      if (this.debug) {
        console.error('[MCP DEBUG] Initializing MCP client manager...');
      }

      this.mcpManager = new MCPClientManager({ debug: this.debug, tracer: this.tracer, agentEvents: this.agentEvents });
      const result = await this.mcpManager.initialize(mcpConfigs);

      // Get tools from the manager (already in Vercel format)
      const vercelTools = this.mcpManager.getVercelTools();
      this.mcpTools = vercelTools;
      const toolCount = Object.keys(vercelTools).length;

      // Generate descriptions for debug logging
      for (const [name, tool] of Object.entries(vercelTools)) {
        this.toolDescriptions[name] = mcpToolToDescription(name, tool);
      }

      if (toolCount === 0) {
        console.error('[MCP INFO] MCP initialization complete: 0 tools loaded');
      } else {
        console.error(`[MCP INFO] MCP initialization complete: ${toolCount} tool${toolCount !== 1 ? 's' : ''} loaded from ${result.connected} server${result.connected !== 1 ? 's' : ''}`);
      }
    } catch (error) {
      console.error('[MCP ERROR] Failed to initialize MCP connections:', error.message);
      if (this.debug) {
        console.error('[MCP DEBUG] Full error details:', error);
      }
    }
  }

  /**
   * Get Vercel AI SDK compatible tools for use with streamText
   * @param {Array<string>|null} filterToolNames - Optional list of tool names to include
   * @returns {Object} Map of tool name to Vercel tool object
   */
  getVercelTools(filterToolNames = null) {
    if (filterToolNames === null) {
      return { ...this.mcpTools };
    }
    const filtered = {};
    for (const name of filterToolNames) {
      if (this.mcpTools[name]) {
        filtered[name] = this.mcpTools[name];
      }
    }
    return filtered;
  }

  /**
   * Get list of MCP tool names
   * @returns {Array<string>} Tool names
   */
  getToolNames() {
    return Object.keys(this.mcpTools);
  }

  /**
   * Check if a tool call is an MCP tool
   * @param {string} toolName - Tool name to check
   * @returns {boolean} True if it's an MCP tool
   */
  isMcpTool(toolName) {
    return toolName in this.mcpTools;
  }

  /**
   * Call graceful_stop on all MCP servers that expose it.
   * @returns {Promise<Array>}
   */
  async callGracefulStopAll() {
    if (this.mcpManager) {
      return this.mcpManager.callGracefulStopAll();
    }
    return [];
  }

  /**
   * Clean up MCP connections
   */
  async cleanup() {
    if (this.mcpManager) {
      await this.mcpManager.disconnect();
    }
  }
}

export default MCPXmlBridge;
