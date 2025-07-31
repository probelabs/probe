/**
 * Abstract base class for all implementation backends
 * @module BaseBackend
 */

import { BackendError, ErrorTypes } from '../core/utils.js';

/**
 * Base class that all implementation backends must extend
 * @class
 */
class BaseBackend {
  /**
   * @param {string} name - Backend name
   * @param {string} version - Backend version
   */
  constructor(name, version) {
    if (new.target === BaseBackend) {
      throw new Error('BaseBackend is an abstract class and cannot be instantiated directly');
    }
    
    this.name = name;
    this.version = version;
    this.initialized = false;
    this.activeSessions = new Map();
  }

  /**
   * Initialize the backend with configuration
   * @param {import('../types/BackendTypes').BackendConfig} config - Backend-specific configuration
   * @returns {Promise<void>}
   * @abstract
   */
  async initialize(config) {
    throw new Error('initialize() must be implemented by subclass');
  }

  /**
   * Check if backend is available and properly configured
   * @returns {Promise<boolean>}
   * @abstract
   */
  async isAvailable() {
    throw new Error('isAvailable() must be implemented by subclass');
  }

  /**
   * Get required dependencies for this backend
   * @returns {import('../types/BackendTypes').Dependency[]}
   * @abstract
   */
  getRequiredDependencies() {
    throw new Error('getRequiredDependencies() must be implemented by subclass');
  }

  /**
   * Execute implementation task
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @returns {Promise<import('../types/BackendTypes').ImplementResult>}
   * @abstract
   */
  async execute(request) {
    throw new Error('execute() must be implemented by subclass');
  }

  /**
   * Cancel an active implementation session
   * @param {string} sessionId - Session to cancel
   * @returns {Promise<void>}
   */
  async cancel(sessionId) {
    const session = this.activeSessions.get(sessionId);
    if (session && session.cancel) {
      await session.cancel();
    }
    this.activeSessions.delete(sessionId);
  }

  /**
   * Get status of an implementation session
   * @param {string} sessionId - Session ID
   * @returns {Promise<import('../types/BackendTypes').BackendStatus>}
   */
  async getStatus(sessionId) {
    const session = this.activeSessions.get(sessionId);
    if (!session) {
      return { 
        status: 'unknown',
        message: 'Session not found'
      };
    }
    
    return {
      status: session.status || 'running',
      progress: session.progress,
      message: session.message,
      details: session.details
    };
  }

  /**
   * Clean up backend resources
   * @returns {Promise<void>}
   */
  async cleanup() {
    // Cancel all active sessions
    const sessionIds = Array.from(this.activeSessions.keys());
    await Promise.all(sessionIds.map(id => this.cancel(id)));
    
    this.activeSessions.clear();
    this.initialized = false;
  }

  /**
   * Get backend capabilities
   * @returns {import('../types/BackendTypes').BackendCapabilities}
   */
  getCapabilities() {
    return {
      supportsLanguages: [],
      supportsStreaming: false,
      supportsRollback: false,
      supportsDirectFileEdit: false,
      supportsPlanGeneration: false,
      supportsTestGeneration: false,
      maxConcurrentSessions: 1
    };
  }

  /**
   * Get backend information
   * @returns {import('../types/BackendTypes').BackendInfo}
   */
  getInfo() {
    return {
      name: this.name,
      version: this.version,
      description: this.getDescription(),
      available: false,
      capabilities: this.getCapabilities(),
      dependencies: this.getRequiredDependencies()
    };
  }

  /**
   * Get backend description
   * @returns {string}
   */
  getDescription() {
    return 'Implementation backend';
  }

  /**
   * Validate implementation request
   * @param {import('../types/BackendTypes').ImplementRequest} request - Request to validate
   * @returns {import('../types/BackendTypes').ValidationResult}
   */
  validateRequest(request) {
    const errors = [];
    const warnings = [];
    
    // Required fields
    if (!request.sessionId) {
      errors.push('sessionId is required');
    }
    
    if (!request.task || request.task.trim().length === 0) {
      errors.push('task description is required');
    }
    
    // Check for active sessions limit
    if (this.activeSessions.size >= this.getCapabilities().maxConcurrentSessions) {
      errors.push(`Maximum concurrent sessions (${this.getCapabilities().maxConcurrentSessions}) reached`);
    }
    
    // Language support check
    if (request.context?.language) {
      const supportedLanguages = this.getCapabilities().supportsLanguages;
      if (supportedLanguages.length > 0 && !supportedLanguages.includes(request.context.language)) {
        warnings.push(`Language '${request.context.language}' may not be fully supported`);
      }
    }
    
    // Option validation
    if (request.options?.generateTests && !this.getCapabilities().supportsTestGeneration) {
      warnings.push('Test generation requested but not supported by this backend');
    }
    
    return {
      valid: errors.length === 0,
      errors,
      warnings
    };
  }

  /**
   * Create a session info object
   * @param {string} sessionId - Session ID
   * @returns {Object}
   * @protected
   */
  createSessionInfo(sessionId) {
    return {
      sessionId,
      startTime: Date.now(),
      status: 'pending',
      progress: 0,
      message: 'Initializing',
      cancel: null,
      details: {}
    };
  }

  /**
   * Update session status
   * @param {string} sessionId - Session ID
   * @param {Partial<import('../types/BackendTypes').BackendStatus>} update - Status update
   * @protected
   */
  updateSessionStatus(sessionId, update) {
    const session = this.activeSessions.get(sessionId);
    if (session) {
      Object.assign(session, update);
    }
  }

  /**
   * Check if backend is initialized
   * @throws {Error} If backend is not initialized
   * @protected
   */
  checkInitialized() {
    if (!this.initialized) {
      throw new BackendError(
        `Backend '${this.name}' is not initialized`,
        ErrorTypes.INITIALIZATION_FAILED,
        'BACKEND_NOT_INITIALIZED'
      );
    }
  }

  /**
   * Log message with backend context
   * @param {string} level - Log level
   * @param {string} message - Log message
   * @param {Object} [data] - Additional data
   * @protected
   */
  log(level, message, data = {}) {
    const logMessage = `[${this.name}] ${message}`;
    const logData = { backend: this.name, ...data };
    
    switch (level) {
      case 'debug':
        console.debug(logMessage, logData);
        break;
      case 'info':
        console.log(logMessage, logData);
        break;
      case 'warn':
        console.warn(logMessage, logData);
        break;
      case 'error':
        console.error(logMessage, logData);
        break;
      default:
        console.log(logMessage, logData);
    }
  }
}

export default BaseBackend;