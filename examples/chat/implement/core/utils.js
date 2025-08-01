/**
 * Utility functions and classes for the implement tool
 * @module utils
 */

/**
 * Error types for backend operations
 * @enum {string}
 */
const ErrorTypes = {
  INITIALIZATION_FAILED: 'initialization_failed',
  DEPENDENCY_MISSING: 'dependency_missing',
  CONFIGURATION_INVALID: 'configuration_invalid',
  EXECUTION_FAILED: 'execution_failed',
  TIMEOUT: 'timeout',
  CANCELLATION: 'cancellation',
  NETWORK_ERROR: 'network_error',
  API_ERROR: 'api_error',
  FILE_ACCESS_ERROR: 'file_access_error',
  VALIDATION_ERROR: 'validation_error',
  BACKEND_NOT_FOUND: 'backend_not_found',
  SESSION_NOT_FOUND: 'session_not_found',
  QUOTA_EXCEEDED: 'quota_exceeded'
};

/**
 * Standardized error class for backend operations
 * @class
 * @extends Error
 */
class BackendError extends Error {
  /**
   * @param {string} message - Error message
   * @param {string} type - Error type from ErrorTypes
   * @param {string} [code] - Error code
   * @param {Object} [details] - Additional error details
   */
  constructor(message, type, code = null, details = {}) {
    super(message);
    this.name = 'BackendError';
    this.type = type;
    this.code = code;
    this.details = details;
    this.timestamp = new Date().toISOString();
    
    // Capture stack trace
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, BackendError);
    }
  }

  /**
   * Convert error to JSON representation
   * @returns {Object}
   */
  toJSON() {
    return {
      name: this.name,
      message: this.message,
      type: this.type,
      code: this.code,
      details: this.details,
      timestamp: this.timestamp,
      stack: this.stack
    };
  }
}

/**
 * Error handling utilities
 * @class
 */
class ErrorHandler {
  /**
   * Create a new BackendError
   * @param {string} type - Error type
   * @param {string} message - Error message
   * @param {string} [code] - Error code
   * @param {Object} [details] - Additional details
   * @returns {BackendError}
   */
  static createError(type, message, code = null, details = {}) {
    return new BackendError(message, type, code, details);
  }

  /**
   * Check if an error is retryable
   * @param {Error|BackendError} error - Error to check
   * @returns {boolean}
   */
  static isRetryable(error) {
    if (error instanceof BackendError) {
      const retryableTypes = [
        ErrorTypes.NETWORK_ERROR,
        ErrorTypes.TIMEOUT,
        ErrorTypes.API_ERROR
      ];
      return retryableTypes.includes(error.type);
    }
    
    // Check for common retryable error patterns
    const message = error.message.toLowerCase();
    return message.includes('timeout') || 
           message.includes('network') || 
           message.includes('connection');
  }

  /**
   * Get recovery strategy for an error
   * @param {Error|BackendError} error - Error to analyze
   * @returns {string} Recovery strategy
   */
  static getRecoveryStrategy(error) {
    if (!(error instanceof BackendError)) {
      return 'manual_intervention';
    }
    
    switch (error.type) {
      case ErrorTypes.DEPENDENCY_MISSING:
        return 'install_dependencies';
      case ErrorTypes.CONFIGURATION_INVALID:
        return 'fix_configuration';
      case ErrorTypes.TIMEOUT:
        return 'retry_with_longer_timeout';
      case ErrorTypes.NETWORK_ERROR:
        return 'retry_with_backoff';
      case ErrorTypes.QUOTA_EXCEEDED:
        return 'wait_or_upgrade';
      case ErrorTypes.API_ERROR:
        return 'check_api_key';
      default:
        return 'manual_intervention';
    }
  }

  /**
   * Format error for user display
   * @param {Error|BackendError} error - Error to format
   * @returns {string}
   */
  static formatForDisplay(error) {
    if (error instanceof BackendError) {
      let message = `${error.message}`;
      
      if (error.code) {
        message += ` (${error.code})`;
      }
      
      // Add helpful context for specific error types
      const strategy = this.getRecoveryStrategy(error);
      switch (strategy) {
        case 'install_dependencies':
          message += '\n💡 Try installing missing dependencies';
          break;
        case 'fix_configuration':
          message += '\n💡 Check your configuration settings';
          break;
        case 'retry_with_longer_timeout':
          message += '\n💡 Consider increasing the timeout value';
          break;
        case 'check_api_key':
          message += '\n💡 Verify your API key is valid';
          break;
      }
      
      return message;
    }
    
    return error.message;
  }
}

/**
 * Retry utility for handling transient failures
 * @class
 */
class RetryHandler {
  /**
   * Execute a function with retry logic
   * @param {Function} fn - Function to execute
   * @param {Object} [options] - Retry options
   * @param {number} [options.maxAttempts=3] - Maximum retry attempts
   * @param {number} [options.initialDelay=1000] - Initial delay in ms
   * @param {number} [options.maxDelay=30000] - Maximum delay in ms
   * @param {number} [options.backoffFactor=2] - Backoff multiplier
   * @param {Function} [options.shouldRetry] - Custom retry predicate
   * @returns {Promise<*>}
   */
  static async withRetry(fn, options = {}) {
    const {
      maxAttempts = 3,
      initialDelay = 1000,
      maxDelay = 30000,
      backoffFactor = 2,
      shouldRetry = ErrorHandler.isRetryable
    } = options;
    
    let lastError;
    let delay = initialDelay;
    
    for (let attempt = 1; attempt <= maxAttempts; attempt++) {
      try {
        return await fn();
      } catch (error) {
        lastError = error;
        
        if (attempt === maxAttempts || !shouldRetry(error)) {
          throw error;
        }
        
        console.error(`Attempt ${attempt} failed, retrying in ${delay}ms...`);
        await this.sleep(delay);
        
        // Calculate next delay with exponential backoff
        delay = Math.min(delay * backoffFactor, maxDelay);
      }
    }
    
    throw lastError;
  }

  /**
   * Sleep for specified milliseconds
   * @param {number} ms - Milliseconds to sleep
   * @returns {Promise<void>}
   */
  static sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}

/**
 * Progress tracking utility
 * @class
 */
class ProgressTracker {
  /**
   * @param {string} sessionId - Session ID
   * @param {Function} [onProgress] - Progress callback
   */
  constructor(sessionId, onProgress = null) {
    this.sessionId = sessionId;
    this.onProgress = onProgress;
    this.startTime = Date.now();
    this.steps = [];
    this.currentStep = null;
  }

  /**
   * Start a new step
   * @param {string} name - Step name
   * @param {string} [message] - Step message
   */
  startStep(name, message = null) {
    if (this.currentStep) {
      this.endStep();
    }
    
    this.currentStep = {
      name,
      message,
      startTime: Date.now()
    };
    
    this.reportProgress({
      type: 'step_start',
      step: name,
      message
    });
  }

  /**
   * End the current step
   * @param {string} [result] - Step result
   */
  endStep(result = 'completed') {
    if (!this.currentStep) return;
    
    const duration = Date.now() - this.currentStep.startTime;
    this.currentStep.duration = duration;
    this.currentStep.result = result;
    
    this.steps.push(this.currentStep);
    
    this.reportProgress({
      type: 'step_end',
      step: this.currentStep.name,
      result,
      duration
    });
    
    this.currentStep = null;
  }

  /**
   * Report progress update
   * @param {Object} update - Progress update
   */
  reportProgress(update) {
    if (!this.onProgress) return;
    
    const progress = {
      sessionId: this.sessionId,
      timestamp: Date.now(),
      elapsed: Date.now() - this.startTime,
      ...update
    };
    
    try {
      this.onProgress(progress);
    } catch (error) {
      console.error('Progress callback error:', error);
    }
  }

  /**
   * Report a message
   * @param {string} message - Message to report
   * @param {string} [type='info'] - Message type
   */
  reportMessage(message, type = 'info') {
    this.reportProgress({
      type: 'message',
      messageType: type,
      message
    });
  }

  /**
   * Get execution summary
   * @returns {Object}
   */
  getSummary() {
    return {
      sessionId: this.sessionId,
      totalDuration: Date.now() - this.startTime,
      steps: this.steps,
      currentStep: this.currentStep
    };
  }
}

/**
 * Utility for parsing and extracting file changes from output
 * @class
 */
class FileChangeParser {
  /**
   * Parse file changes from command output
   * @param {string} output - Command output
   * @param {string} [workingDir] - Working directory
   * @returns {import('../types/BackendTypes').FileChange[]}
   */
  static parseChanges(output, workingDir = process.cwd()) {
    const changes = [];
    const patterns = {
      created: /(?:created?|new file|added?):?\s+(.+)/gi,
      modified: /(?:modified?|changed?|updated?):?\s+(.+)/gi,
      deleted: /(?:deleted?|removed?):?\s+(.+)/gi,
      diff: /^[-+]{3}\s+(.+)$/gm
    };
    
    // Extract file changes
    for (const [type, pattern] of Object.entries(patterns)) {
      if (type === 'diff') continue;
      
      let match;
      while ((match = pattern.exec(output)) !== null) {
        const filePath = match[1].trim();
        if (filePath && !changes.some(c => c.path === filePath)) {
          changes.push({
            path: filePath,
            type,
            description: `File ${filePath} was ${type}`
          });
        }
      }
    }
    
    // Git status parsing
    const gitStatusPattern = /^([AMD])\s+(.+)$/gm;
    let match;
    while ((match = gitStatusPattern.exec(output)) !== null) {
      const status = match[1];
      const filePath = match[2];
      
      const typeMap = {
        'A': 'created',
        'M': 'modified',
        'D': 'deleted'
      };
      
      if (typeMap[status] && !changes.some(c => c.path === filePath)) {
        changes.push({
          path: filePath,
          type: typeMap[status],
          description: `File ${filePath} was ${typeMap[status]}`
        });
      }
    }
    
    return changes;
  }

  /**
   * Extract diff statistics from output
   * @param {string} output - Command output
   * @returns {Object}
   */
  static extractDiffStats(output) {
    const stats = {
      filesChanged: 0,
      insertions: 0,
      deletions: 0
    };
    
    // Git diff stat pattern
    const statPattern = /(\d+)\s+files?\s+changed(?:,\s+(\d+)\s+insertions?)?(?:,\s+(\d+)\s+deletions?)?/;
    const match = output.match(statPattern);
    
    if (match) {
      stats.filesChanged = parseInt(match[1], 10);
      stats.insertions = match[2] ? parseInt(match[2], 10) : 0;
      stats.deletions = match[3] ? parseInt(match[3], 10) : 0;
    }
    
    return stats;
  }
}

/**
 * Simple token estimation utility
 * @class
 */
class TokenEstimator {
  /**
   * Estimate token count for text
   * @param {string} text - Text to estimate
   * @returns {number} Estimated token count
   */
  static estimate(text) {
    // Simple estimation: ~4 characters per token
    // This is a rough approximation; real tokenizers are more complex
    return Math.ceil(text.length / 4);
  }

  /**
   * Check if text exceeds token limit
   * @param {string} text - Text to check
   * @param {number} limit - Token limit
   * @returns {boolean}
   */
  static exceedsLimit(text, limit) {
    return this.estimate(text) > limit;
  }

  /**
   * Truncate text to fit within token limit
   * @param {string} text - Text to truncate
   * @param {number} limit - Token limit
   * @param {string} [suffix='...'] - Suffix to add
   * @returns {string}
   */
  static truncate(text, limit, suffix = '...') {
    const estimatedChars = limit * 4;
    if (text.length <= estimatedChars) {
      return text;
    }
    
    return text.substring(0, estimatedChars - suffix.length) + suffix;
  }
}

export {
  ErrorTypes,
  BackendError,
  ErrorHandler,
  RetryHandler,
  ProgressTracker,
  FileChangeParser,
  TokenEstimator
};