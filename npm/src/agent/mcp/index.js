/**
 * MCP (Model Context Protocol) integration for ProbeAgent
 *
 * This module provides:
 * - MCP client management for connecting to MCP servers
 * - XML/JSON hybrid tool interface
 * - Configuration management
 */

// Re-export main classes and functions
export { MCPClientManager, createMCPManager, createTransport } from './client.js';
export {
  loadMCPConfiguration,
  loadMCPConfigurationFromPath,
  parseEnabledServers,
  createSampleConfig,
  saveConfig
} from './config.js';
export {
  MCPXmlBridge,
  mcpToolToXmlDefinition,
  parseXmlMcpToolCall,
  parseHybridXmlToolCall,
  createHybridSystemMessage
} from './xmlBridge.js';

// Import for default export
import { MCPClientManager, createMCPManager, createTransport } from './client.js';
import {
  loadMCPConfiguration,
  loadMCPConfigurationFromPath,
  parseEnabledServers,
  createSampleConfig,
  saveConfig
} from './config.js';
import {
  MCPXmlBridge,
  mcpToolToXmlDefinition,
  parseXmlMcpToolCall,
  parseHybridXmlToolCall,
  createHybridSystemMessage
} from './xmlBridge.js';

// Default export for convenience
export default {
  // Client
  MCPClientManager,
  createMCPManager,
  createTransport,

  // Config
  loadMCPConfiguration,
  loadMCPConfigurationFromPath,
  parseEnabledServers,
  createSampleConfig,
  saveConfig,

  // XML Bridge
  MCPXmlBridge,
  mcpToolToXmlDefinition,
  parseXmlMcpToolCall,
  parseHybridXmlToolCall,
  createHybridSystemMessage
};