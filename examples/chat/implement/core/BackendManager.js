/**
 * Backend manager for handling multiple implementation backends
 * @module BackendManager
 */

const { BackendError, ErrorTypes, ErrorHandler, RetryHandler } = require('./utils.js');

/**
 * Manages registration, selection, and execution of implementation backends
 * @class
 */
class BackendManager {
  /**
   * @param {Object} config - Backend manager configuration
   * @param {string} config.defaultBackend - Default backend name
   * @param {string[]} [config.fallbackBackends] - Fallback backend names
   * @param {string} [config.selectionStrategy='auto'] - Backend selection strategy
   * @param {number} [config.maxConcurrentSessions=3] - Maximum concurrent sessions
   * @param {number} [config.timeout=300000] - Default timeout in milliseconds
   * @param {number} [config.retryAttempts=2] - Number of retry attempts
   */
  constructor(config) {
    this.config = {
      defaultBackend: config.defaultBackend || 'aider',
      fallbackBackends: config.fallbackBackends || [],
      selectionStrategy: config.selectionStrategy || 'auto',
      maxConcurrentSessions: config.maxConcurrentSessions || 3,
      timeout: config.timeout || 300000,
      retryAttempts: config.retryAttempts || 2,
      ...config
    };
    
    this.backends = new Map();
    this.activeSessionCount = 0;
    this.sessionBackendMap = new Map();
    this.initialized = false;
  }

  /**
   * Initialize the backend manager
   * @returns {Promise<void>}
   */
  async initialize() {
    if (this.initialized) return;
    
    // Initialize any pre-registered backends
    for (const [name, backend] of this.backends) {
      try {
        if (!backend.initialized) {
          await backend.initialize(this.config.backends?.[name] || {});
        }
      } catch (error) {
        console.warn(`Failed to initialize backend '${name}':`, error.message);
      }
    }
    
    this.initialized = true;
  }

  /**
   * Register a new backend
   * @param {import('../backends/BaseBackend')} backend - Backend instance
   * @returns {Promise<void>}
   */
  async registerBackend(backend) {
    if (!backend || !backend.name) {
      throw new Error('Invalid backend: must have a name property');
    }
    
    // Check if backend already registered
    if (this.backends.has(backend.name)) {
      console.warn(`Backend '${backend.name}' is already registered, replacing...`);
    }
    
    this.backends.set(backend.name, backend);
    
    // Initialize if manager is already initialized
    if (this.initialized && !backend.initialized) {
      try {
        await backend.initialize(this.config.backends?.[backend.name] || {});
      } catch (error) {
        console.warn(`Failed to initialize backend '${backend.name}':`, error.message);
      }
    }
  }

  /**
   * Unregister a backend
   * @param {string} name - Backend name
   * @returns {Promise<void>}
   */
  async unregisterBackend(name) {
    const backend = this.backends.get(name);
    if (backend) {
      await backend.cleanup();
      this.backends.delete(name);
    }
  }

  /**
   * Get list of available backend names
   * @returns {string[]}
   */
  getAvailableBackends() {
    return Array.from(this.backends.keys());
  }

  /**
   * Get backend instance by name
   * @param {string} name - Backend name
   * @returns {import('../backends/BaseBackend')|null}
   */
  getBackend(name) {
    return this.backends.get(name) || null;
  }

  /**
   * Get backend information
   * @param {string} name - Backend name
   * @returns {import('../types/BackendTypes').BackendInfo|null}
   */
  async getBackendInfo(name) {
    const backend = this.backends.get(name);
    if (!backend) return null;
    
    const info = backend.getInfo();
    info.available = await backend.isAvailable();
    
    return info;
  }

  /**
   * Select appropriate backend for request
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @returns {Promise<string>} Selected backend name
   */
  async selectBackend(request) {
    switch (this.config.selectionStrategy) {
      case 'preference':
        return this.selectByPreference(request);
      case 'capability':
        return this.selectByCapability(request);
      case 'auto':
      default:
        return this.selectAuto(request);
    }
  }

  /**
   * Select backend using auto strategy
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @returns {Promise<string>}
   * @private
   */
  async selectAuto(request) {
    // First try explicit backend if specified
    if (request.options?.backend && this.backends.has(request.options.backend)) {
      const backend = this.backends.get(request.options.backend);
      if (await backend.isAvailable()) {
        return request.options.backend;
      }
    }
    
    // Try default backend
    if (this.backends.has(this.config.defaultBackend)) {
      const backend = this.backends.get(this.config.defaultBackend);
      if (await backend.isAvailable()) {
        return this.config.defaultBackend;
      }
    }
    
    // Try fallback backends
    for (const backendName of this.config.fallbackBackends) {
      if (this.backends.has(backendName)) {
        const backend = this.backends.get(backendName);
        if (await backend.isAvailable()) {
          return backendName;
        }
      }
    }
    
    // Try any available backend
    for (const [name, backend] of this.backends) {
      if (await backend.isAvailable()) {
        return name;
      }
    }
    
    throw new BackendError(
      'No available backends found',
      ErrorTypes.BACKEND_NOT_FOUND,
      'NO_AVAILABLE_BACKENDS'
    );
  }

  /**
   * Select backend by user preference
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @returns {Promise<string>}
   * @private
   */
  async selectByPreference(request) {
    const preference = request.options?.backend || this.config.defaultBackend;
    
    if (!this.backends.has(preference)) {
      throw new BackendError(
        `Preferred backend '${preference}' not found`,
        ErrorTypes.BACKEND_NOT_FOUND,
        'PREFERRED_BACKEND_NOT_FOUND'
      );
    }
    
    const backend = this.backends.get(preference);
    if (!(await backend.isAvailable())) {
      throw new BackendError(
        `Preferred backend '${preference}' is not available`,
        ErrorTypes.BACKEND_NOT_FOUND,
        'PREFERRED_BACKEND_UNAVAILABLE'
      );
    }
    
    return preference;
  }

  /**
   * Select backend by capability matching
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @returns {Promise<string>}
   * @private
   */
  async selectByCapability(request) {
    const candidates = [];
    
    for (const [name, backend] of this.backends) {
      if (!(await backend.isAvailable())) continue;
      
      const capabilities = backend.getCapabilities();
      let score = 0;
      
      // Score based on language support
      if (request.context?.language) {
        if (capabilities.supportsLanguages.includes(request.context.language)) {
          score += 10;
        } else if (capabilities.supportsLanguages.includes('all')) {
          score += 5;
        }
      }
      
      // Score based on requested features
      if (request.options?.generateTests && capabilities.supportsTestGeneration) {
        score += 5;
      }
      
      if (request.options?.streaming && capabilities.supportsStreaming) {
        score += 3;
      }
      
      // Prefer backends with higher concurrent session limits
      score += Math.min(capabilities.maxConcurrentSessions, 5);
      
      candidates.push({ name, score });
    }
    
    if (candidates.length === 0) {
      throw new BackendError(
        'No capable backends found',
        ErrorTypes.BACKEND_NOT_FOUND,
        'NO_CAPABLE_BACKENDS'
      );
    }
    
    // Sort by score and return highest
    candidates.sort((a, b) => b.score - a.score);
    return candidates[0].name;
  }

  /**
   * Execute implementation request
   * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
   * @returns {Promise<import('../types/BackendTypes').ImplementResult>}
   */
  async executeImplementation(request) {
    // Ensure manager is initialized
    await this.initialize();
    
    // Check concurrent session limit
    if (this.activeSessionCount >= this.config.maxConcurrentSessions) {
      throw new BackendError(
        'Maximum concurrent sessions reached',
        ErrorTypes.QUOTA_EXCEEDED,
        'MAX_SESSIONS_REACHED',
        { limit: this.config.maxConcurrentSessions, current: this.activeSessionCount }
      );
    }
    
    // Select backend
    const backendName = await this.selectBackend(request);
    const backend = this.backends.get(backendName);
    
    if (!backend) {
      throw new BackendError(
        `Backend '${backendName}' not found`,
        ErrorTypes.BACKEND_NOT_FOUND,
        'BACKEND_NOT_FOUND'
      );
    }
    
    // Track session
    this.activeSessionCount++;
    this.sessionBackendMap.set(request.sessionId, backendName);
    
    try {
      // Apply timeout if not specified
      if (!request.options?.timeout) {
        request.options = request.options || {};
        request.options.timeout = this.config.timeout;
      }
      
      // Execute with retry logic
      const result = await RetryHandler.withRetry(
        () => backend.execute(request),
        {
          maxAttempts: this.config.retryAttempts + 1,
          shouldRetry: (error) => {
            // Don't retry user cancellations or validation errors
            if (error instanceof BackendError) {
              if (error.type === ErrorTypes.CANCELLATION || 
                  error.type === ErrorTypes.VALIDATION_ERROR) {
                return false;
              }
            }
            return ErrorHandler.isRetryable(error);
          }
        }
      );
      
      // Add backend information to result
      result.backend = backendName;
      
      return result;
      
    } catch (error) {
      // Try fallback backends if configured
      if (this.config.fallbackBackends.length > 0) {
        console.warn(`Backend '${backendName}' failed, trying fallbacks...`);
        
        for (const fallbackName of this.config.fallbackBackends) {
          if (fallbackName === backendName) continue;
          
          const fallbackBackend = this.backends.get(fallbackName);
          if (!fallbackBackend || !(await fallbackBackend.isAvailable())) {
            continue;
          }
          
          try {
            console.log(`Trying fallback backend: ${fallbackName}`);
            this.sessionBackendMap.set(request.sessionId, fallbackName);
            
            const result = await fallbackBackend.execute(request);
            result.backend = fallbackName;
            result.fallback = true;
            
            return result;
          } catch (fallbackError) {
            console.warn(`Fallback backend '${fallbackName}' also failed:`, fallbackError.message);
          }
        }
      }
      
      // All backends failed
      throw error;
      
    } finally {
      this.activeSessionCount--;
      this.sessionBackendMap.delete(request.sessionId);
    }
  }

  /**
   * Cancel an implementation session
   * @param {string} sessionId - Session ID
   * @returns {Promise<void>}
   */
  async cancelImplementation(sessionId) {
    const backendName = this.sessionBackendMap.get(sessionId);
    if (!backendName) {
      throw new BackendError(
        `Session '${sessionId}' not found`,
        ErrorTypes.SESSION_NOT_FOUND,
        'SESSION_NOT_FOUND'
      );
    }
    
    const backend = this.backends.get(backendName);
    if (backend) {
      await backend.cancel(sessionId);
    }
    
    this.sessionBackendMap.delete(sessionId);
    this.activeSessionCount = Math.max(0, this.activeSessionCount - 1);
  }

  /**
   * Get session status
   * @param {string} sessionId - Session ID
   * @returns {Promise<import('../types/BackendTypes').BackendStatus>}
   */
  async getSessionStatus(sessionId) {
    const backendName = this.sessionBackendMap.get(sessionId);
    if (!backendName) {
      return {
        status: 'unknown',
        message: 'Session not found'
      };
    }
    
    const backend = this.backends.get(backendName);
    if (!backend) {
      return {
        status: 'error',
        message: 'Backend not found'
      };
    }
    
    const status = await backend.getStatus(sessionId);
    status.backend = backendName;
    
    return status;
  }

  /**
   * Validate configuration
   * @returns {Promise<import('../types/BackendTypes').ValidationResult>}
   */
  async validateConfiguration() {
    const errors = [];
    const warnings = [];
    
    // Check default backend exists
    if (!this.backends.has(this.config.defaultBackend)) {
      errors.push(`Default backend '${this.config.defaultBackend}' not registered`);
    }
    
    // Check fallback backends exist
    for (const fallback of this.config.fallbackBackends) {
      if (!this.backends.has(fallback)) {
        warnings.push(`Fallback backend '${fallback}' not registered`);
      }
    }
    
    // Check at least one backend is available
    let hasAvailable = false;
    for (const [name, backend] of this.backends) {
      if (await backend.isAvailable()) {
        hasAvailable = true;
        break;
      }
    }
    
    if (!hasAvailable) {
      errors.push('No backends are available');
    }
    
    return {
      valid: errors.length === 0,
      errors,
      warnings
    };
  }

  /**
   * Check health of all backends
   * @param {string} [name] - Specific backend to check, or all if not specified
   * @returns {Promise<Object>}
   */
  async checkBackendHealth(name = null) {
    const results = {};
    
    const backendsToCheck = name 
      ? [name]
      : Array.from(this.backends.keys());
    
    for (const backendName of backendsToCheck) {
      const backend = this.backends.get(backendName);
      if (!backend) continue;
      
      try {
        const available = await backend.isAvailable();
        const info = backend.getInfo();
        
        results[backendName] = {
          status: available ? 'healthy' : 'unavailable',
          available,
          version: info.version,
          capabilities: info.capabilities,
          dependencies: info.dependencies
        };
      } catch (error) {
        results[backendName] = {
          status: 'error',
          available: false,
          error: error.message
        };
      }
    }
    
    return results;
  }

  /**
   * Get recommended backend for a given context
   * @param {Object} context - Request context
   * @returns {string|null}
   */
  getRecommendedBackend(context) {
    // Simple recommendation logic
    if (context.language) {
      for (const [name, backend] of this.backends) {
        const capabilities = backend.getCapabilities();
        if (capabilities.supportsLanguages.includes(context.language)) {
          return name;
        }
      }
    }
    
    return this.config.defaultBackend;
  }

  /**
   * Check if a backend can handle a request
   * @param {string} backendName - Backend name
   * @param {import('../types/BackendTypes').ImplementRequest} request - Request to check
   * @returns {boolean}
   */
  canHandleRequest(backendName, request) {
    const backend = this.backends.get(backendName);
    if (!backend) return false;
    
    const validation = backend.validateRequest(request);
    return validation.valid;
  }

  /**
   * Clean up all backends
   * @returns {Promise<void>}
   */
  async cleanup() {
    const cleanupPromises = [];
    
    for (const [name, backend] of this.backends) {
      cleanupPromises.push(
        backend.cleanup().catch(error => {
          console.error(`Error cleaning up backend '${name}':`, error);
        })
      );
    }
    
    await Promise.all(cleanupPromises);
    
    this.backends.clear();
    this.sessionBackendMap.clear();
    this.activeSessionCount = 0;
    this.initialized = false;
  }
}

module.exports = BackendManager;