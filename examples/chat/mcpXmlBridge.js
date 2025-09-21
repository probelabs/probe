/**
 * XML-to-MCP Bridge
 * Allows using MCP tools with XML-like syntax while maintaining JSON parameters
 */

import { connectToMCPServers, loadMCPConfig } from './mcpClient.js';

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
      const paramsContent = paramsMatch[1].trim();

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
    this.mcpClients = null;
    this.xmlDefinitions = {};
  }

  /**
   * Initialize MCP connections and load tools
   * @param {Array<Object>} configs - MCP server configurations
   */
  async initialize(configs = null) {
    const mcpConfigs = configs || loadMCPConfig();

    if (mcpConfigs.length === 0) {
      if (this.debug) {
        console.error('No MCP servers configured');
      }
      return;
    }

    try {
      const { tools, clients } = await connectToMCPServers(mcpConfigs);
      this.mcpTools = tools;
      this.mcpClients = clients;

      // Generate XML definitions for all tools
      for (const [name, tool] of Object.entries(tools)) {
        this.xmlDefinitions[name] = mcpToolToXmlDefinition(name, tool);
      }

      if (this.debug) {
        console.error(`Loaded ${Object.keys(tools).length} MCP tools`);
      }
    } catch (error) {
      console.error('Failed to initialize MCP connections:', error);
    }
  }

  /**
   * Get all XML tool definitions for inclusion in system prompt
   * @returns {string} Combined XML tool definitions
   */
  getXmlToolDefinitions() {
    return Object.values(this.xmlDefinitions).join('\n\n');
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
      throw new Error('No valid MCP tool call found in XML');
    }

    const { toolName, params } = parsed;

    if (this.debug) {
      console.error(`Executing MCP tool: ${toolName} with params:`, params);
    }

    const tool = this.mcpTools[toolName];
    if (!tool) {
      throw new Error(`Unknown MCP tool: ${toolName}`);
    }

    try {
      const result = await tool.execute(params);
      return {
        success: true,
        toolName,
        result
      };
    } catch (error) {
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
    if (this.mcpClients) {
      await this.mcpClients.disconnect();
    }
  }
}

/**
 * Enhanced XML parser that handles both native and MCP tools
 * @param {string} xmlString - XML string to parse
 * @param {Array<string>} nativeTools - List of native tool names
 * @param {MCPXmlBridge} mcpBridge - MCP bridge instance
 * @returns {Object|null} Parsed tool call
 */
export function parseHybridXmlToolCall(xmlString, nativeTools = [], mcpBridge = null) {
  // First try native tools with standard XML parsing
  for (const toolName of nativeTools) {
    const nativeResult = parseNativeXmlTool(xmlString, toolName);
    if (nativeResult) {
      return { ...nativeResult, type: 'native' };
    }
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