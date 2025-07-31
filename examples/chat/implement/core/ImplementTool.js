/**
 * Implementation tool wrapper that integrates with the backend system
 * @module ImplementTool
 */

import BackendManager from './BackendManager.js';
import { createBackend, listBackendNames } from '../backends/registry.js';
import { BackendError, ErrorTypes } from './utils.js';
import { configManager } from './config.js';

/**
 * Implementation tool that uses pluggable backends
 * @class
 */
class ImplementTool {
  /**
   * @param {Object} config - Tool configuration
   * @param {boolean} [config.enabled=false] - Whether the tool is enabled
   * @param {Object} [config.backendConfig] - Backend manager configuration
   */
  constructor(config = {}) {
    this.enabled = config.enabled || false;
    this.backendManager = null;
    this.config = config;
    this.initialized = false;
  }

  /**
   * Initialize the implementation tool
   * @returns {Promise<void>}
   */
  async initialize() {
    if (this.initialized) return;
    
    if (!this.enabled) {
      throw new Error('Implementation tool is not enabled. Use --allow-edit flag to enable.');
    }
    
    // Initialize configuration manager
    await configManager.initialize(this.config.configPath);
    
    // Get configuration from manager
    const implementConfig = configManager.getImplementConfig();
    const backendConfigs = configManager.get('backends') || {};
    
    // Create backend manager with configuration
    const backendManagerConfig = {
      ...implementConfig,
      backends: backendConfigs,
      ...this.config.backendConfig
    };
    
    this.backendManager = new BackendManager(backendManagerConfig);
    
    // Register available backends
    await this.registerBackends();
    
    // Initialize backend manager
    await this.backendManager.initialize();
    
    // Validate configuration
    const configValidation = configManager.validate();
    if (!configValidation.valid) {
      console.error('Configuration errors:', configValidation.errors.join(', '));
      if (configValidation.warnings.length > 0) {
        console.warn('Configuration warnings:', configValidation.warnings.join(', '));
      }
    }
    
    const backendValidation = await this.backendManager.validateConfiguration();
    if (!backendValidation.valid) {
      console.warn('Backend configuration warnings:', backendValidation.errors.join(', '));
    }
    
    // Listen for configuration changes
    configManager.onChange(async (newConfig) => {
      console.error('Configuration changed, reinitializing backends...');
      await this.reinitialize(newConfig);
    });
    
    this.initialized = true;
  }

  /**
   * Register all available backends
   * @private
   */
  async registerBackends() {
    const backendNames = listBackendNames();
    
    for (const name of backendNames) {
      try {
        const backend = createBackend(name);
        if (backend) {
          await this.backendManager.registerBackend(backend);
          console.error(`Registered backend: ${name}`);
        }
      } catch (error) {
        console.warn(`Failed to register backend '${name}':`, error.message);
      }
    }
  }

  /**
   * Reinitialize with new configuration
   * @param {Object} newConfig - New configuration
   * @private
   */
  async reinitialize(newConfig) {
    try {
      // Clean up existing backend manager
      if (this.backendManager) {
        await this.backendManager.cleanup();
      }
      
      // Create new backend manager with updated configuration
      const implementConfig = newConfig.implement || {};
      const backendConfigs = newConfig.backends || {};
      
      const backendManagerConfig = {
        ...implementConfig,
        backends: backendConfigs,
        ...this.config.backendConfig
      };
      
      this.backendManager = new BackendManager(backendManagerConfig);
      
      // Re-register backends
      await this.registerBackends();
      
      // Re-initialize
      await this.backendManager.initialize();
      
      console.error('Backend reinitialization completed');
    } catch (error) {
      console.error('Failed to reinitialize backends:', error);
    }
  }

  /**
   * Get tool definition for AI models
   * @returns {Object}
   */
  getToolDefinition() {
    return {
      name: 'implement',
      description: 'Implement a feature or fix a bug using AI-powered code generation. Only available when --allow-edit is enabled.',
      parameters: {
        type: 'object',
        properties: {
          task: {
            type: 'string',
            description: 'The task description for implementation'
          },
          backend: {
            type: 'string',
            description: 'Optional: Specific backend to use (aider, claude-code)',
            enum: listBackendNames()
          },
          autoCommit: {
            type: 'boolean',
            description: 'Whether to auto-commit changes (default: false)'
          },
          generateTests: {
            type: 'boolean',
            description: 'Whether to generate tests for the implementation'
          },
          dryRun: {
            type: 'boolean',
            description: 'Perform a dry run without making actual changes'
          }
        },
        required: ['task']
      }
    };
  }

  /**
   * Execute implementation task
   * @param {Object} params - Execution parameters
   * @param {string} params.task - Task description
   * @param {string} [params.backend] - Specific backend to use
   * @param {boolean} [params.autoCommit] - Auto-commit changes
   * @param {boolean} [params.generateTests] - Generate tests
   * @param {boolean} [params.dryRun] - Dry run mode
   * @param {string} [params.sessionId] - Session ID
   * @returns {Promise<Object>}
   */
  async execute(params) {
    if (!this.enabled) {
      throw new Error('Implementation tool is not enabled. Use --allow-edit flag to enable.');
    }
    
    // Ensure initialized
    if (!this.initialized) {
      await this.initialize();
    }
    
    const { task, backend, autoCommit, generateTests, dryRun, sessionId, ...rest } = params;
    
    // Build implementation request
    const request = {
      sessionId: sessionId || `implement-${Date.now()}`,
      task,
      context: {
        workingDirectory: process.cwd(),
        ...rest.context
      },
      options: {
        backend,
        autoCommit: autoCommit || false,
        generateTests: generateTests || false,
        dryRun: dryRun || false,
        ...rest.options
      },
      callbacks: {
        onProgress: (update) => {
          // Log progress to stderr for visibility
          if (update.message) {
            const prefix = update.type === 'stderr' ? '[STDERR]' : '[INFO]';
            console.error(`${prefix} ${update.message}`);
          }
        },
        onError: (error) => {
          console.error('[ERROR]', error.message);
        }
      }
    };
    
    try {
      console.error(`Executing implementation task: ${task.substring(0, 100)}${task.length > 100 ? '...' : ''}`);
      console.error(`Using backend selection strategy: ${this.backendManager.config.selectionStrategy}`);
      
      if (backend) {
        console.error(`Requested backend: ${backend}`);
      }
      
      // Execute implementation
      const result = await this.backendManager.executeImplementation(request);
      
      console.error(`Implementation completed using backend: ${result.backend}`);
      
      if (result.fallback) {
        console.error('Note: Used fallback backend due to primary backend failure');
      }
      
      // Format result for compatibility with existing code
      return {
        success: result.success,
        output: result.output,
        error: result.error?.message || null,
        command: `[${result.backend}] ${task}`,
        timestamp: new Date().toISOString(),
        prompt: task,
        backend: result.backend,
        metrics: result.metrics,
        changes: result.changes
      };
      
    } catch (error) {
      console.error(`Implementation failed:`, error.message);
      
      // Format error response
      return {
        success: false,
        output: null,
        error: error.message,
        command: `[failed] ${task}`,
        timestamp: new Date().toISOString(),
        prompt: task,
        backend: null,
        errorDetails: error instanceof BackendError ? error.toJSON() : { message: error.message }
      };
    }
  }

  /**
   * Cancel an implementation session
   * @param {string} sessionId - Session ID to cancel
   * @returns {Promise<void>}
   */
  async cancel(sessionId) {
    if (!this.backendManager) {
      throw new Error('Implementation tool not initialized');
    }
    
    await this.backendManager.cancelImplementation(sessionId);
  }

  /**
   * Get backend information
   * @returns {Promise<Object>}
   */
  async getBackendInfo() {
    if (!this.initialized) {
      await this.initialize();
    }
    
    const health = await this.backendManager.checkBackendHealth();
    const availableBackends = this.backendManager.getAvailableBackends();
    
    return {
      enabled: this.enabled,
      defaultBackend: this.backendManager.config.defaultBackend,
      fallbackBackends: this.backendManager.config.fallbackBackends,
      availableBackends,
      health
    };
  }

  /**
   * Clean up resources
   * @returns {Promise<void>}
   */
  async cleanup() {
    if (this.backendManager) {
      await this.backendManager.cleanup();
    }
    
    // Clean up configuration manager
    configManager.cleanup();
    
    this.initialized = false;
  }
}

/**
 * Create a singleton instance of the implementation tool
 * This maintains compatibility with the existing code structure
 */
function createImplementTool(config = {}) {
  const tool = new ImplementTool(config);
  
  // Return a tool object compatible with the existing interface
  return {
    ...tool.getToolDefinition(),
    execute: async (params) => {
      return await tool.execute(params);
    },
    cancel: async (sessionId) => {
      return await tool.cancel(sessionId);
    },
    getInfo: async () => {
      return await tool.getBackendInfo();
    },
    cleanup: async () => {
      return await tool.cleanup();
    },
    // Expose the tool instance for advanced usage
    instance: tool
  };
}

export { ImplementTool, createImplementTool };