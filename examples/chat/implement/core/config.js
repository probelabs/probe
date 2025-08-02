/**
 * Configuration management for the implementation tool
 * @module config
 */

import fs from 'fs';
import path from 'path';
import { promisify } from 'util';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const readFile = promisify(fs.readFile);
const writeFile = promisify(fs.writeFile);
const exists = promisify(fs.exists);

/**
 * Configuration manager for implementation backends
 * @class
 */
class ConfigManager {
  constructor() {
    this.config = null;
    this.configPath = null;
    this.watchers = new Map();
    this.changeCallbacks = [];
  }

  /**
   * Initialize configuration
   * @param {string} [configPath] - Path to configuration file
   * @returns {Promise<void>}
   */
  async initialize(configPath = null) {
    // Determine config path
    this.configPath = this.resolveConfigPath(configPath);
    
    // Load configuration
    await this.loadConfig();
    
    // Apply environment overrides
    this.applyEnvironmentOverrides();
    
    // Set up file watching
    if (this.configPath && fs.existsSync(this.configPath)) {
      this.setupWatcher();
    }
  }

  /**
   * Resolve configuration file path
   * @param {string} [providedPath] - User-provided path
   * @returns {string|null}
   * @private
   */
  resolveConfigPath(providedPath) {
    // Priority order:
    // 1. Provided path
    // 2. Environment variable
    // 3. Local config file
    // 4. Default config file
    
    if (providedPath && fs.existsSync(providedPath)) {
      return providedPath;
    }
    
    if (process.env.IMPLEMENT_TOOL_CONFIG_PATH) {
      const envPath = process.env.IMPLEMENT_TOOL_CONFIG_PATH;
      if (fs.existsSync(envPath)) {
        return envPath;
      }
    }
    
    // Check for local config in current directory
    const localConfig = path.join(process.cwd(), 'implement-config.json');
    if (fs.existsSync(localConfig)) {
      return localConfig;
    }
    
    // Fall back to default config
    const defaultConfig = path.join(__dirname, '..', 'config', 'default.json');
    if (fs.existsSync(defaultConfig)) {
      return defaultConfig;
    }
    
    return null;
  }

  /**
   * Load configuration from file
   * @returns {Promise<void>}
   * @private
   */
  async loadConfig() {
    if (this.configPath && fs.existsSync(this.configPath)) {
      try {
        const configData = await readFile(this.configPath, 'utf8');
        this.config = JSON.parse(configData);
        console.error(`Loaded configuration from: ${this.configPath}`);
      } catch (error) {
        console.error(`Failed to load configuration from ${this.configPath}:`, error.message);
        this.config = this.getDefaultConfig();
      }
    } else {
      console.error('Using default configuration');
      this.config = this.getDefaultConfig();
    }
  }

  /**
   * Get default configuration
   * @returns {Object}
   * @private
   */
  getDefaultConfig() {
    return {
      implement: {
        defaultBackend: 'aider',
        fallbackBackends: ['claude-code'],
        selectionStrategy: 'auto',
        maxConcurrentSessions: 3,
        timeout: 300000,
        retryAttempts: 2,
        retryDelay: 5000
      },
      backends: {
        aider: {
          command: 'aider',
          timeout: 300000,
          maxOutputSize: 10485760,
          additionalArgs: [],
          environment: {},
          autoCommit: false,
          modelSelection: 'auto'
        },
        'claude-code': {
          timeout: 300000,
          maxTokens: 8000,
          temperature: 0.3,
          model: 'claude-3-5-sonnet-20241022',
          systemPrompt: null,
          tools: ['edit', 'search', 'bash'],
          maxTurns: 100
        }
      }
    };
  }

  /**
   * Apply environment variable overrides
   * @private
   */
  applyEnvironmentOverrides() {
    // Backend selection
    if (process.env.IMPLEMENT_TOOL_BACKEND) {
      this.config.implement.defaultBackend = process.env.IMPLEMENT_TOOL_BACKEND;
      console.error(`[ImplementConfig] Setting default backend from env: ${process.env.IMPLEMENT_TOOL_BACKEND}`);
    }
    
    if (process.env.IMPLEMENT_TOOL_FALLBACKS) {
      this.config.implement.fallbackBackends = process.env.IMPLEMENT_TOOL_FALLBACKS
        .split(',')
        .map(s => s.trim())
        .filter(Boolean);
    }
    
    if (process.env.IMPLEMENT_TOOL_SELECTION_STRATEGY) {
      this.config.implement.selectionStrategy = process.env.IMPLEMENT_TOOL_SELECTION_STRATEGY;
    }
    
    if (process.env.IMPLEMENT_TOOL_TIMEOUT) {
      // Convert seconds to milliseconds for backend compatibility
      this.config.implement.timeout = parseInt(process.env.IMPLEMENT_TOOL_TIMEOUT, 10) * 1000;
    }
    
    // Aider backend configuration
    if (process.env.AIDER_MODEL) {
      this.config.backends.aider = this.config.backends.aider || {};
      this.config.backends.aider.model = process.env.AIDER_MODEL;
    }
    
    if (process.env.AIDER_TIMEOUT) {
      this.config.backends.aider = this.config.backends.aider || {};
      this.config.backends.aider.timeout = parseInt(process.env.AIDER_TIMEOUT, 10);
    }
    
    if (process.env.AIDER_AUTO_COMMIT) {
      this.config.backends.aider = this.config.backends.aider || {};
      this.config.backends.aider.autoCommit = process.env.AIDER_AUTO_COMMIT === 'true';
    }
    
    if (process.env.AIDER_ADDITIONAL_ARGS) {
      this.config.backends.aider = this.config.backends.aider || {};
      this.config.backends.aider.additionalArgs = process.env.AIDER_ADDITIONAL_ARGS
        .split(',')
        .map(s => s.trim())
        .filter(Boolean);
    }
    
    // Claude Code backend configuration
    if (process.env.CLAUDE_CODE_MODEL) {
      this.config.backends['claude-code'] = this.config.backends['claude-code'] || {};
      this.config.backends['claude-code'].model = process.env.CLAUDE_CODE_MODEL;
    }
    
    if (process.env.CLAUDE_CODE_MAX_TOKENS) {
      this.config.backends['claude-code'] = this.config.backends['claude-code'] || {};
      this.config.backends['claude-code'].maxTokens = parseInt(process.env.CLAUDE_CODE_MAX_TOKENS, 10);
    }
    
    if (process.env.CLAUDE_CODE_TEMPERATURE) {
      this.config.backends['claude-code'] = this.config.backends['claude-code'] || {};
      this.config.backends['claude-code'].temperature = parseFloat(process.env.CLAUDE_CODE_TEMPERATURE);
    }
    
    if (process.env.CLAUDE_CODE_MAX_TURNS) {
      this.config.backends['claude-code'] = this.config.backends['claude-code'] || {};
      this.config.backends['claude-code'].maxTurns = parseInt(process.env.CLAUDE_CODE_MAX_TURNS, 10);
    }
  }

  /**
   * Set up file watcher for configuration changes
   * @private
   */
  setupWatcher() {
    if (!this.configPath) return;
    
    fs.watchFile(this.configPath, { interval: 2000 }, async (curr, prev) => {
      if (curr.mtime !== prev.mtime) {
        console.error('Configuration file changed, reloading...');
        await this.reloadConfig();
      }
    });
  }

  /**
   * Reload configuration from file
   * @returns {Promise<void>}
   */
  async reloadConfig() {
    try {
      const oldConfig = JSON.stringify(this.config);
      await this.loadConfig();
      this.applyEnvironmentOverrides();
      
      const newConfig = JSON.stringify(this.config);
      if (oldConfig !== newConfig) {
        this.notifyChangeCallbacks();
      }
    } catch (error) {
      console.error('Failed to reload configuration:', error);
    }
  }

  /**
   * Register a callback for configuration changes
   * @param {Function} callback - Callback function
   */
  onChange(callback) {
    this.changeCallbacks.push(callback);
  }

  /**
   * Notify all change callbacks
   * @private
   */
  notifyChangeCallbacks() {
    for (const callback of this.changeCallbacks) {
      try {
        callback(this.config);
      } catch (error) {
        console.error('Error in configuration change callback:', error);
      }
    }
  }

  /**
   * Get configuration value by path
   * @param {string} [path] - Dot-separated path (e.g., 'implement.defaultBackend')
   * @returns {*}
   */
  get(path = null) {
    if (!path) {
      return this.config;
    }
    
    const parts = path.split('.');
    let value = this.config;
    
    for (const part of parts) {
      if (value && typeof value === 'object' && part in value) {
        value = value[part];
      } else {
        return undefined;
      }
    }
    
    return value;
  }

  /**
   * Set configuration value by path
   * @param {string} path - Dot-separated path
   * @param {*} value - Value to set
   */
  set(path, value) {
    const parts = path.split('.');
    const lastPart = parts.pop();
    
    let target = this.config;
    for (const part of parts) {
      if (!(part in target) || typeof target[part] !== 'object') {
        target[part] = {};
      }
      target = target[part];
    }
    
    target[lastPart] = value;
  }

  /**
   * Save configuration to file
   * @param {string} [path] - Path to save to (defaults to current config path)
   * @returns {Promise<void>}
   */
  async save(path = null) {
    const savePath = path || this.configPath;
    if (!savePath) {
      throw new Error('No configuration file path specified');
    }
    
    try {
      const configData = JSON.stringify(this.config, null, 2);
      await writeFile(savePath, configData, 'utf8');
      console.error(`Configuration saved to: ${savePath}`);
    } catch (error) {
      throw new Error(`Failed to save configuration: ${error.message}`);
    }
  }

  /**
   * Get backend-specific configuration
   * @param {string} backendName - Backend name
   * @returns {Object}
   */
  getBackendConfig(backendName) {
    return this.config.backends?.[backendName] || {};
  }

  /**
   * Get implementation tool configuration
   * @returns {Object}
   */
  getImplementConfig() {
    return this.config.implement || {};
  }

  /**
   * Validate configuration
   * @returns {Object} Validation result
   */
  validate() {
    const errors = [];
    const warnings = [];
    
    // Check required fields
    if (!this.config.implement?.defaultBackend) {
      errors.push('implement.defaultBackend is required');
    }
    
    // Check backend configurations exist
    const defaultBackend = this.config.implement?.defaultBackend;
    if (defaultBackend && !this.config.backends?.[defaultBackend]) {
      warnings.push(`Configuration for default backend '${defaultBackend}' not found`);
    }
    
    // Check fallback backends
    const fallbackBackends = this.config.implement?.fallbackBackends || [];
    for (const backend of fallbackBackends) {
      if (!this.config.backends?.[backend]) {
        warnings.push(`Configuration for fallback backend '${backend}' not found`);
      }
    }
    
    // Validate selection strategy
    const validStrategies = ['auto', 'preference', 'capability'];
    const strategy = this.config.implement?.selectionStrategy;
    if (strategy && !validStrategies.includes(strategy)) {
      errors.push(`Invalid selection strategy: ${strategy}. Must be one of: ${validStrategies.join(', ')}`);
    }
    
    return {
      valid: errors.length === 0,
      errors,
      warnings
    };
  }

  /**
   * Clean up resources
   */
  cleanup() {
    if (this.configPath) {
      fs.unwatchFile(this.configPath);
    }
    
    this.changeCallbacks = [];
    this.watchers.clear();
  }
}

// Create singleton instance
const configManager = new ConfigManager();

export {
  ConfigManager,
  configManager
};