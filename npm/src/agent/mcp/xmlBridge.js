/**
 * XML-to-MCP Bridge
 * Allows using MCP tools with XML-like syntax while maintaining JSON parameters
 */

import { MCPClientManager } from './client.js';
import { loadMCPConfiguration } from './config.js';
import { processXmlWithThinkingAndRecovery } from '../xmlParsingUtils.js';

/**
 * Convert MCP tool to XML definition format
 * @param {string} name - Tool name
 * @param {Object} tool - MCP tool object
 * @returns {string} XML-formatted tool definition
 */
export function mcpToolToXmlDefinition(name, tool) {
  const description = tool.description || 'MCP tool';
  const inputSchema = tool.inputSchema || tool.parameters || {};

  // Build parameter documentation
  let paramDocs = '';
  if (inputSchema.properties) {
    paramDocs = '\n\nParameters (provide as JSON object):';
    for (const [paramName, paramSchema] of Object.entries(inputSchema.properties)) {
      const required = inputSchema.required?.includes(paramName) ? ' (required)' : ' (optional)';
      const desc = paramSchema.description || '';
      const type = paramSchema.type || 'any';
      paramDocs += `\n- ${paramName}: ${type}${required} - ${desc}`;

      if (paramSchema.enum) {
        paramDocs += ` [choices: ${paramSchema.enum.join(', ')}]`;
      }
    }
  }

  return `## ${name}
Description: ${description}${paramDocs}

Usage:
<${name}>
<params>
{
  "param1": "value1",
  "param2": "value2"
}
</params>
</${name}>

Or for simple single parameter:
<${name}>
<params>value</params>
</${name}>`;
}

/**
 * Parse XML tool call with JSON parameters
 * Handles both JSON object parameters and simple string parameters
 * @param {string} xmlString - XML string containing tool call
 * @param {Array<string>} mcpToolNames - List of available MCP tool names
 * @returns {Object|null} Parsed tool call with name and params
 */
export function parseXmlMcpToolCall(xmlString, mcpToolNames = []) {
  // Clean the XML string
  const cleanedXml = xmlString.replace(/<thinking>[\s\S]*?<\/thinking>/g, '').trim();

  for (const toolName of mcpToolNames) {
    // Look for the tool in XML format
    const openTag = `<${toolName}>`;
    const closeTag = `</${toolName}>`;

    const openIndex = cleanedXml.indexOf(openTag);
    if (openIndex === -1) continue;

    const closeIndex = cleanedXml.indexOf(closeTag, openIndex);
    if (closeIndex === -1) continue;

    // Extract content between tags
    const contentStart = openIndex + openTag.length;
    const content = cleanedXml.substring(contentStart, closeIndex).trim();

    // Look for params tag
    const paramsMatch = content.match(/<params>([\s\S]*?)<\/params>/);

    let params = {};
    if (paramsMatch) {
      let paramsContent = paramsMatch[1].trim();

      // Handle CDATA sections
      const cdataMatch = paramsContent.match(/^<!\[CDATA\[([\s\S]*?)\]\]>$/);
      if (cdataMatch) {
        paramsContent = cdataMatch[1];
      }

      // Try to parse as JSON first
      try {
        // Handle JSON object
        if (paramsContent.startsWith('{')) {
          params = JSON.parse(paramsContent);
        } else {
          // Handle simple string parameter
          // For backwards compatibility with simple XML params
          params = { value: paramsContent };
        }
      } catch (e) {
        // If JSON parsing fails, treat as simple string
        params = { value: paramsContent };
      }
    } else {
      // Legacy format: parse individual XML parameters
      const paramPattern = /<(\w+)>([\s\S]*?)<\/\1>/g;
      let match;
      while ((match = paramPattern.exec(content)) !== null) {
        const [, paramName, paramValue] = match;
        params[paramName] = paramValue.trim();
      }
    }

    return { toolName, params };
  }

  return null;
}

/**
 * MCP Tool Manager that bridges XML and MCP
 */
export class MCPXmlBridge {
  constructor(options = {}) {
    this.debug = options.debug || false;
    this.mcpTools = {};
    this.mcpManager = null;
    this.xmlDefinitions = {};
  }

  /**
   * Initialize MCP connections and load tools
   * @param {Object|Array<Object>} config - MCP configuration object or server configurations (deprecated)
   */
  async initialize(config = null) {
    let mcpConfigs = null;

    if (!config) {
      // No config provided - fall back to auto-discovery for backward compatibility
      if (this.debug) {
        console.error('[MCP DEBUG] No config provided, attempting auto-discovery...');
      }
      mcpConfigs = loadMCPConfiguration();

      // Check if auto-discovery found anything
      if (!mcpConfigs || !mcpConfigs.mcpServers || Object.keys(mcpConfigs.mcpServers).length === 0) {
        console.error('[MCP WARNING] MCP enabled but no configuration found');
        console.error('[MCP INFO] To use MCP, provide configuration via:');
        console.error('[MCP INFO]   - mcpConfig option when creating ProbeAgent');
        console.error('[MCP INFO]   - mcpConfigPath option pointing to a config file');
        console.error('[MCP INFO]   - Config file in standard locations (~/.mcp/config.json, etc.)');
        console.error('[MCP INFO]   - Environment variable MCP_CONFIG_PATH');
      }
    } else if (Array.isArray(config)) {
      // Deprecated: Array of server configs (backward compatibility)
      if (this.debug) {
        console.error('[MCP DEBUG] Using deprecated array config format (consider using mcpConfig object)');
      }
      mcpConfigs = { mcpServers: config };
    } else {
      // New: Full config object provided directly
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

      // Initialize the MCP client manager
      this.mcpManager = new MCPClientManager({ debug: this.debug });
      const result = await this.mcpManager.initialize(mcpConfigs);

      // Get tools from the manager
      const vercelTools = this.mcpManager.getVercelTools();
      this.mcpTools = vercelTools;
      const toolCount = Object.keys(vercelTools).length;

      // Generate XML definitions for all tools
      for (const [name, tool] of Object.entries(vercelTools)) {
        this.xmlDefinitions[name] = mcpToolToXmlDefinition(name, tool);
      }

      if (toolCount === 0) {
        console.error('[MCP INFO] MCP initialization complete: 0 tools loaded');
      } else {
        console.error(`[MCP INFO] MCP initialization complete: ${toolCount} tool${toolCount !== 1 ? 's' : ''} loaded from ${result.connected} server${result.connected !== 1 ? 's' : ''}`);

        if (this.debug) {
          console.error('[MCP DEBUG] Tool definitions generated for XML bridge');
        }
      }
    } catch (error) {
      console.error('[MCP ERROR] Failed to initialize MCP connections:', error.message);
      if (this.debug) {
        console.error('[MCP DEBUG] Full error details:', error);
      }
    }
  }

  /**
   * Get XML tool definitions for inclusion in system prompt
   * @param {Array<string>|null} filterToolNames - Optional list of tool names to include (if null, include all)
   * @returns {string} Combined XML tool definitions
   */
  getXmlToolDefinitions(filterToolNames = null) {
    if (filterToolNames === null) {
      return Object.values(this.xmlDefinitions).join('\n\n');
    }

    // Filter definitions based on provided tool names
    return Object.entries(this.xmlDefinitions)
      .filter(([name]) => filterToolNames.includes(name))
      .map(([, def]) => def)
      .join('\n\n');
  }

  /**
   * Get list of MCP tool names
   * @returns {Array<string>} Tool names
   */
  getToolNames() {
    return Object.keys(this.mcpTools);
  }

  /**
   * Execute an MCP tool from XML call
   * @param {string} xmlString - XML tool call string
   * @returns {Promise<Object>} Tool execution result
   */
  async executeFromXml(xmlString) {
    const parsed = parseXmlMcpToolCall(xmlString, this.getToolNames());

    if (!parsed) {
      console.error('[MCP ERROR] No valid MCP tool call found in XML');
      throw new Error('No valid MCP tool call found in XML');
    }

    const { toolName, params } = parsed;

    if (this.debug) {
      console.error(`[MCP DEBUG] Executing MCP tool: ${toolName}`);
      console.error(`[MCP DEBUG] Parameters:`, JSON.stringify(params, null, 2));
    }

    const tool = this.mcpTools[toolName];
    if (!tool) {
      console.error(`[MCP ERROR] Unknown MCP tool: ${toolName}`);
      console.error(`[MCP ERROR] Available tools: ${this.getToolNames().join(', ')}`);
      throw new Error(`Unknown MCP tool: ${toolName}`);
    }

    try {
      const result = await tool.execute(params);

      if (this.debug) {
        console.error(`[MCP DEBUG] Tool ${toolName} executed successfully`);
      }

      return {
        success: true,
        toolName,
        result
      };
    } catch (error) {
      console.error(`[MCP ERROR] Tool ${toolName} execution failed:`, error.message);
      if (this.debug) {
        console.error(`[MCP DEBUG] Full error details:`, error);
      }

      return {
        success: false,
        toolName,
        error: error.message
      };
    }
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
   * Clean up MCP connections
   */
  async cleanup() {
    if (this.mcpManager) {
      await this.mcpManager.disconnect();
    }
  }
}

/**
 * Enhanced XML parser that handles both native and MCP tools
 * Uses the exact same logic as CLI/SDK mode to ensure consistency
 * @param {string} xmlString - XML string to parse
 * @param {Array<string>} nativeTools - List of native tool names
 * @param {MCPXmlBridge} mcpBridge - MCP bridge instance
 * @returns {Object|null} Parsed tool call
 */
export function parseHybridXmlToolCall(xmlString, nativeTools = [], mcpBridge = null) {
  // First try native tools with the same logic as CLI/SDK mode
  // This includes thinking tag removal and attempt_complete recovery logic
  const nativeResult = parseNativeXmlToolWithThinking(xmlString, nativeTools);
  if (nativeResult) {
    return { ...nativeResult, type: 'native' };
  }

  // Then try MCP tools if bridge is available
  if (mcpBridge) {
    const mcpResult = parseXmlMcpToolCall(xmlString, mcpBridge.getToolNames());
    if (mcpResult) {
      return { ...mcpResult, type: 'mcp' };
    }
  }

  return null;
}

/**
 * Parse native XML tools using the same logic as CLI/SDK mode
 * Now uses shared utilities instead of duplicating code
 * @param {string} xmlString - XML string to parse
 * @param {Array<string>} validTools - List of valid tool names
 * @returns {Object|null} Parsed tool call
 */
function parseNativeXmlToolWithThinking(xmlString, validTools) {
  // Use the shared processing logic
  const { cleanedXmlString, recoveryResult } = processXmlWithThinkingAndRecovery(xmlString, validTools);
  
  // If recovery found an attempt_complete pattern, return it
  if (recoveryResult) {
    return recoveryResult;
  }

  // Use the original parseNativeXmlTool function to parse the cleaned XML string
  for (const toolName of validTools) {
    const result = parseNativeXmlTool(cleanedXmlString, toolName);
    if (result) {
      return result;
    }
  }

  return null;
}

/**
 * Parse native XML tool (existing format)
 * @param {string} xmlString - XML string
 * @param {string} toolName - Tool name to look for
 * @returns {Object|null} Parsed tool call
 */
function parseNativeXmlTool(xmlString, toolName) {
  const openTag = `<${toolName}>`;
  const closeTag = `</${toolName}>`;

  const openIndex = xmlString.indexOf(openTag);
  if (openIndex === -1) return null;

  const closeIndex = xmlString.indexOf(closeTag, openIndex);
  if (closeIndex === -1) return null;

  const contentStart = openIndex + openTag.length;
  const content = xmlString.substring(contentStart, closeIndex).trim();

  // Parse individual XML parameters (native format)
  const params = {};
  const paramPattern = /<(\w+)>([\s\S]*?)<\/\1>/g;
  let match;

  while ((match = paramPattern.exec(content)) !== null) {
    const [, paramName, paramValue] = match;
    // Skip if this is the params tag itself (MCP format)
    if (paramName !== 'params') {
      params[paramName] = paramValue.trim();
    }
  }

  // Only return if we found actual parameters (not MCP format)
  if (Object.keys(params).length > 0) {
    return { toolName, params };
  }

  return null;
}

/**
 * Create a combined system message with both native and MCP tools
 * @param {string} baseSystemMessage - Base system message
 * @param {string} nativeToolDefinitions - Native tool definitions in XML format
 * @param {MCPXmlBridge} mcpBridge - MCP bridge with loaded tools
 * @returns {string} Combined system message
 */
export function createHybridSystemMessage(baseSystemMessage, nativeToolDefinitions, mcpBridge) {
  let message = baseSystemMessage;

  // Add native tools section
  if (nativeToolDefinitions) {
    message += '\n\n=== NATIVE TOOLS ===\n';
    message += 'These tools use standard XML parameter format:\n\n';
    message += nativeToolDefinitions;
  }

  // Add MCP tools section if available
  if (mcpBridge && mcpBridge.getToolNames().length > 0) {
    message += '\n\n=== MCP TOOLS ===\n';
    message += 'These tools use JSON parameters within the params tag:\n\n';
    message += mcpBridge.getXmlToolDefinitions();
  }

  // Add usage instructions
  message += '\n\n=== TOOL USAGE INSTRUCTIONS ===\n';
  message += `
For NATIVE tools, use standard XML format:
<search>
<query>authentication</query>
<path>./src</path>
</search>

For MCP tools, use JSON within params tag:
<mcp_tool_name>
<params>
{
  "param1": "value1",
  "param2": 123
}
</params>
</mcp_tool_name>

IMPORTANT: Always check the tool definition to determine whether it's a native tool (XML params) or MCP tool (JSON params).
`;

  return message;
}

export default MCPXmlBridge;