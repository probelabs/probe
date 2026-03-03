/**
 * MCP (Model Context Protocol) integration for ProbeAgent
 *
 * This module provides:
 * - MCP client management for connecting to MCP servers
 * - Native Vercel AI SDK tool interface
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
  mcpToolToDescription
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
  mcpToolToDescription
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

  // MCP Bridge
  MCPXmlBridge,
  mcpToolToDescription
};
