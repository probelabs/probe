/**
 * MCP Configuration Manager
 * Handles loading and parsing MCP server configurations similar to Claude
 */

import { readFileSync, existsSync, mkdirSync, writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { homedir } from 'os';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

/**
 * Timeout configuration constants
 */
export const DEFAULT_TIMEOUT = 30000; // 30 seconds
export const MAX_TIMEOUT = (() => {
  if (process.env.MCP_MAX_TIMEOUT) {
    const parsed = parseInt(process.env.MCP_MAX_TIMEOUT, 10);
    if (!isNaN(parsed) && parsed >= 30000 && parsed <= 7200000) return parsed;
  }
  return 1800000; // 30 minutes default - workflow tools (code checkouts, AI exploration) need time
})();

/**
 * Validate and normalize a timeout value
 * @param {*} value - The timeout value to validate
 * @returns {number|undefined} Validated timeout in ms, or undefined if invalid
 */
export function validateTimeout(value) {
  if (value === undefined || value === null) return undefined;
  const num = Number(value);
  if (!Number.isFinite(num) || num < 0) return undefined; // Invalid, use fallback
  return Math.min(num, MAX_TIMEOUT); // Cap at max timeout
}

/**
 * Validate and normalize method filter configuration
 * @param {Object} serverConfig - Server configuration
 * @param {string} serverName - Server name for logging
 * @returns {Object} Object with allowedMethods and blockedMethods (null if not configured)
 */
export function validateMethodFilter(serverConfig, serverName = 'unknown') {
  const result = { allowedMethods: null, blockedMethods: null };
  const debug = process.env.DEBUG === '1' || process.env.DEBUG_MCP === '1';

  // Check if both are specified - allowedMethods takes precedence
  if (serverConfig.allowedMethods && serverConfig.blockedMethods) {
    console.error(`[MCP WARN] Server '${serverName}' has both allowedMethods and blockedMethods - using allowedMethods only`);
  }

  // Process allowedMethods
  if (serverConfig.allowedMethods) {
    if (!Array.isArray(serverConfig.allowedMethods)) {
      console.error(`[MCP WARN] Server '${serverName}' allowedMethods must be an array, ignoring`);
    } else {
      const validMethods = serverConfig.allowedMethods.filter(m => typeof m === 'string' && m.length > 0);
      if (validMethods.length !== serverConfig.allowedMethods.length) {
        console.error(`[MCP WARN] Server '${serverName}' allowedMethods contains non-string values, skipping those`);
      }
      if (validMethods.length > 0) {
        result.allowedMethods = validMethods;
        if (debug) {
          console.error(`[MCP DEBUG] Server '${serverName}' allowedMethods: ${validMethods.join(', ')}`);
        }
      }
    }
    return result; // If allowedMethods is specified (even if invalid), don't process blockedMethods
  }

  // Process blockedMethods (only if allowedMethods not specified)
  if (serverConfig.blockedMethods) {
    if (!Array.isArray(serverConfig.blockedMethods)) {
      console.error(`[MCP WARN] Server '${serverName}' blockedMethods must be an array, ignoring`);
    } else {
      const validMethods = serverConfig.blockedMethods.filter(m => typeof m === 'string' && m.length > 0);
      if (validMethods.length !== serverConfig.blockedMethods.length) {
        console.error(`[MCP WARN] Server '${serverName}' blockedMethods contains non-string values, skipping those`);
      }
      if (validMethods.length > 0) {
        result.blockedMethods = validMethods;
        if (debug) {
          console.error(`[MCP DEBUG] Server '${serverName}' blockedMethods: ${validMethods.join(', ')}`);
        }
      }
    }
  }

  return result;
}

/**
 * Default MCP configuration structure
 */
const DEFAULT_CONFIG = {
  mcpServers: {
    // Example probe server configuration
    'probe-local': {
      command: 'node',
      args: [join(__dirname, '../../../examples/chat/mcpServer.js')],
      transport: 'stdio',
      enabled: false
    },
    'probe-npm': {
      command: 'npx',
      args: ['-y', '@probelabs/probe@latest', 'mcp'],
      transport: 'stdio',
      enabled: false
    }
  }
};

/**
 * Load MCP configuration from a specific file path
 * @param {string} configPath - Path to MCP configuration file
 * @returns {Object} Configuration object
 * @throws {Error} If file doesn't exist or is invalid
 */
export function loadMCPConfigurationFromPath(configPath) {
  if (!configPath) {
    throw new Error('Config path is required');
  }

  if (!existsSync(configPath)) {
    throw new Error(`MCP configuration file not found: ${configPath}`);
  }

  try {
    const content = readFileSync(configPath, 'utf8');
    const config = JSON.parse(content);

    if (process.env.DEBUG === '1' || process.env.DEBUG_MCP === '1') {
      console.error(`[MCP DEBUG] Loaded configuration from: ${configPath}`);
    }

    // Merge with environment variable overrides
    return mergeWithEnvironment(config);
  } catch (error) {
    throw new Error(`Failed to parse MCP config from ${configPath}: ${error.message}`);
  }
}

/**
 * Load MCP configuration from various sources (DEPRECATED - use loadMCPConfigurationFromPath for explicit paths)
 * Priority order:
 * 1. Environment variable MCP_CONFIG_PATH
 * 2. Local project .mcp/config.json
 * 3. Home directory ~/.config/probe/mcp.json
 * 4. Home directory ~/.mcp/config.json (Claude compatible)
 * 5. Default configuration
 * @deprecated Use loadMCPConfigurationFromPath for explicit path loading or pass config directly
 */
export function loadMCPConfiguration() {
  const configPaths = [
    // Environment variable path
    process.env.MCP_CONFIG_PATH,
    // Local project paths
    join(process.cwd(), '.mcp', 'config.json'),
    join(process.cwd(), 'mcp.config.json'),
    // Home directory paths
    join(homedir(), '.config', 'probe', 'mcp.json'),
    join(homedir(), '.mcp', 'config.json'),
    // Claude-style config location
    join(homedir(), 'Library', 'Application Support', 'Claude', 'mcp_config.json'),
  ].filter(Boolean);

  let config = null;

  // Try to load configuration from paths
  for (const configPath of configPaths) {
    if (existsSync(configPath)) {
      try {
        const content = readFileSync(configPath, 'utf8');
        config = JSON.parse(content);
        if (process.env.DEBUG === '1' || process.env.DEBUG_MCP === '1') {
          console.error(`[MCP DEBUG] Loaded configuration from: ${configPath}`);
        }
        break;
      } catch (error) {
        console.error(`[MCP ERROR] Failed to parse config from ${configPath}:`, error.message);
      }
    }
  }

  // Merge with environment variable overrides
  config = mergeWithEnvironment(config || DEFAULT_CONFIG);

  return config;
}

/**
 * Merge configuration with environment variables
 * Supports:
 * - MCP_SERVERS_<NAME>_COMMAND: Command for server
 * - MCP_SERVERS_<NAME>_ARGS: Comma-separated args
 * - MCP_SERVERS_<NAME>_TRANSPORT: Transport type
 * - MCP_SERVERS_<NAME>_URL: URL for HTTP/WebSocket transports
 * - MCP_SERVERS_<NAME>_ENABLED: Enable/disable server
 */
function mergeWithEnvironment(config) {
  const serverPattern = /^MCP_SERVERS_([A-Z0-9_]+)_(.+)$/;

  for (const [key, value] of Object.entries(process.env)) {
    const match = key.match(serverPattern);
    if (match) {
      const [, serverName, property] = match;
      const normalizedName = serverName.toLowerCase().replace(/_/g, '-');

      if (!config.mcpServers) {
        config.mcpServers = {};
      }

      if (!config.mcpServers[normalizedName]) {
        config.mcpServers[normalizedName] = {};
      }

      switch (property) {
        case 'COMMAND':
          config.mcpServers[normalizedName].command = value;
          break;
        case 'ARGS':
          config.mcpServers[normalizedName].args = value.split(',').map(arg => arg.trim());
          break;
        case 'TRANSPORT':
          config.mcpServers[normalizedName].transport = value.toLowerCase();
          break;
        case 'URL':
          config.mcpServers[normalizedName].url = value;
          break;
        case 'ENABLED':
          config.mcpServers[normalizedName].enabled = value === 'true' || value === '1';
          break;
        case 'ENV':
          // Support custom environment variables for the server
          try {
            config.mcpServers[normalizedName].env = JSON.parse(value);
          } catch {
            config.mcpServers[normalizedName].env = { [property]: value };
          }
          break;
        case 'TIMEOUT':
          // Per-server timeout in milliseconds with validation
          const validatedTimeout = validateTimeout(value);
          if (validatedTimeout !== undefined) {
            config.mcpServers[normalizedName].timeout = validatedTimeout;
          } else {
            console.error(`[MCP WARN] Invalid timeout value for ${normalizedName}: ${value}`);
          }
          break;
        case 'ALLOWLIST':
          // Comma-separated list of allowed method names (supports wildcards)
          // e.g., MCP_SERVERS_MYSERVER_ALLOWLIST=method1,method2,prefix_*
          config.mcpServers[normalizedName].allowedMethods = value.split(',').map(m => m.trim()).filter(Boolean);
          break;
        case 'BLOCKLIST':
          // Comma-separated list of blocked method names (supports wildcards)
          // e.g., MCP_SERVERS_MYSERVER_BLOCKLIST=dangerous_*,risky_method
          config.mcpServers[normalizedName].blockedMethods = value.split(',').map(m => m.trim()).filter(Boolean);
          break;
      }
    }
  }

  return config;
}

/**
 * Parse MCP server configuration to extract enabled servers
 * @param {Object} config - Full MCP configuration
 * @returns {Array} Array of server configurations ready for connection
 */
export function parseEnabledServers(config) {
  const servers = [];

  if (!config || !config.mcpServers) {
    return servers;
  }

  for (const [name, serverConfig] of Object.entries(config.mcpServers)) {
    // Skip disabled servers
    if (serverConfig.enabled === false) {
      continue;
    }

    const server = {
      name,
      ...serverConfig
    };

    // Set default transport if not specified
    if (!server.transport) {
      if (server.url) {
        // Infer transport from URL
        if (server.url.startsWith('ws://') || server.url.startsWith('wss://')) {
          server.transport = 'websocket';
        } else if (server.url.includes('/sse')) {
          server.transport = 'sse';
        } else {
          server.transport = 'http';
        }
      } else {
        server.transport = 'stdio';
      }
    }

    // Validate required fields based on transport
    if (server.transport === 'stdio') {
      if (!server.command) {
        console.error(`[MCP ERROR] Server ${name} missing required 'command' for stdio transport`);
        continue;
      }
    } else if (['websocket', 'sse', 'http'].includes(server.transport)) {
      if (!server.url) {
        console.error(`[MCP ERROR] Server ${name} missing required 'url' for ${server.transport} transport`);
        continue;
      }
    }

    // Validate and normalize timeout if present (fail-fast validation)
    if (server.timeout !== undefined) {
      const validatedTimeout = validateTimeout(server.timeout);
      if (validatedTimeout === undefined) {
        console.error(`[MCP ERROR] Server ${name} has invalid timeout value: ${server.timeout}`);
        continue;
      }
      server.timeout = validatedTimeout;
    }

    // Validate and normalize method filter configuration
    const methodFilter = validateMethodFilter(serverConfig, name);
    server.allowedMethods = methodFilter.allowedMethods;
    server.blockedMethods = methodFilter.blockedMethods;

    servers.push(server);
  }

  return servers;
}

/**
 * Create a sample MCP configuration file
 */
export function createSampleConfig() {
  return {
    mcpServers: {
      'probe': {
        command: 'npx',
        args: ['-y', '@probelabs/probe@latest', 'mcp'],
        transport: 'stdio',
        enabled: true,
        description: 'Probe code search MCP server'
      },
      'filesystem': {
        command: 'npx',
        args: ['-y', '@modelcontextprotocol/server-filesystem', process.cwd()],
        transport: 'stdio',
        enabled: false,
        description: 'Filesystem operations MCP server'
      },
      'github': {
        command: 'npx',
        args: ['-y', '@modelcontextprotocol/server-github'],
        transport: 'stdio',
        enabled: false,
        description: 'GitHub API MCP server',
        env: {
          GITHUB_TOKEN: 'your-github-token'
        }
      },
      'postgres': {
        command: 'npx',
        args: ['-y', '@modelcontextprotocol/server-postgres'],
        transport: 'stdio',
        enabled: false,
        description: 'PostgreSQL database MCP server',
        env: {
          DATABASE_URL: 'postgresql://user:pass@localhost/db'
        }
      },
      'custom-http': {
        url: 'http://localhost:3000/mcp',
        transport: 'http',
        enabled: false,
        description: 'Custom HTTP MCP server'
      },
      'custom-websocket': {
        url: 'ws://localhost:8080',
        transport: 'websocket',
        enabled: false,
        description: 'Custom WebSocket MCP server'
      },
      'slow-server-example': {
        command: 'node',
        args: ['path/to/slow-server.js'],
        transport: 'stdio',
        enabled: false,
        timeout: 120000,
        description: 'Example server with custom 2-minute timeout (overrides global setting)'
      },
      'filtered-server-example': {
        command: 'npx',
        args: ['-y', '@example/mcp-server'],
        transport: 'stdio',
        enabled: false,
        allowedMethods: ['safe_read', 'safe_query'],
        description: 'Example server with method allowlist - only safe_read and safe_query are available'
      },
      'blocklist-server-example': {
        command: 'npx',
        args: ['-y', '@example/mcp-server'],
        transport: 'stdio',
        enabled: false,
        blockedMethods: ['dangerous_delete', 'dangerous_*'],
        description: 'Example server with method blocklist - all methods except dangerous ones (supports wildcards)'
      }
    },
    // Global settings (apply to all servers unless overridden per-server)
    settings: {
      timeout: 30000,
      retryCount: 3,
      debug: false
    }
  };
}

/**
 * Save configuration to file
 * @param {Object} config - Configuration to save
 * @param {string} path - Path to save to
 */
export function saveConfig(config, path) {
  const dir = dirname(path);

  // Create directory if it doesn't exist
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }

  writeFileSync(path, JSON.stringify(config, null, 2), 'utf8');
  console.error(`[MCP INFO] Configuration saved to: ${path}`);
}

export default {
  loadMCPConfiguration,
  loadMCPConfigurationFromPath,
  parseEnabledServers,
  createSampleConfig,
  saveConfig,
  validateTimeout,
  validateMethodFilter,
  DEFAULT_TIMEOUT,
  MAX_TIMEOUT
};