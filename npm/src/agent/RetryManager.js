/**
 * RetryManager - Handles retry logic with exponential backoff
 *
 * Provides configurable retry behavior for AI API calls with:
 * - Exponential backoff
 * - Configurable retry limits
 * - Error type filtering
 * - Detailed error context
 */

/**
 * Default retryable error patterns
 */
const DEFAULT_RETRYABLE_ERRORS = [
  'Overloaded',
  'overloaded',
  'rate_limit',
  'rate limit',
  '429',
  '500',
  '502',
  '503',
  '504',
  'timeout',
  'ECONNRESET',
  'ETIMEDOUT',
  'ENOTFOUND',
  'api_error'
];

/**
 * Check if an error is retryable based on error patterns
 * @param {Error} error - The error to check
 * @param {Array<string>} retryableErrors - List of retryable error patterns
 * @returns {boolean} - True if error should be retried
 */
function isRetryableError(error, retryableErrors = DEFAULT_RETRYABLE_ERRORS) {
  if (!error) return false;

  const errorString = error.toString().toLowerCase();
  const errorMessage = (error.message || '').toLowerCase();
  const errorCode = (error.code || '').toLowerCase();
  const errorType = (error.type || '').toLowerCase();
  const statusCode = error.statusCode || error.status;

  // Check if error matches any retryable pattern
  for (const pattern of retryableErrors) {
    const lowerPattern = pattern.toLowerCase();

    if (errorString.includes(lowerPattern) ||
        errorMessage.includes(lowerPattern) ||
        errorCode.includes(lowerPattern) ||
        errorType.includes(lowerPattern) ||
        statusCode?.toString() === pattern) {
      return true;
    }
  }

  return false;
}

/**
 * Extract meaningful error information for logging
 * @param {Error} error - The error to extract info from
 * @returns {Object} - Error information object
 */
function extractErrorInfo(error) {
  return {
    message: error.message || error.toString(),
    type: error.type || error.constructor.name,
    code: error.code,
    statusCode: error.statusCode || error.status,
    provider: error.provider,
    isRetryable: isRetryableError(error)
  };
}

/**
 * Sleep for a specified duration
 * @param {number} ms - Milliseconds to sleep
 * @returns {Promise<void>}
 */
function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * RetryManager class for handling retry logic with exponential backoff
 */
export class RetryManager {
  /**
   * Create a new RetryManager
   * @param {Object} options - Configuration options
   * @param {number} [options.maxRetries=3] - Maximum retry attempts
   * @param {number} [options.initialDelay=1000] - Initial delay in ms (1 second)
   * @param {number} [options.maxDelay=30000] - Maximum delay in ms (30 seconds)
   * @param {number} [options.backoffFactor=2] - Exponential backoff multiplier
   * @param {Array<string>} [options.retryableErrors] - List of retryable error patterns
   * @param {boolean} [options.debug=false] - Enable debug logging
   */
  constructor(options = {}) {
    // Validate and set configuration with defaults
    this.maxRetries = this._validateNumber(options.maxRetries, 3, 'maxRetries', 0, 100);
    this.initialDelay = this._validateNumber(options.initialDelay, 1000, 'initialDelay', 0, 60000);
    this.maxDelay = this._validateNumber(options.maxDelay, 30000, 'maxDelay', 0, 300000);
    this.backoffFactor = this._validateNumber(options.backoffFactor, 2, 'backoffFactor', 1, 10);
    this.retryableErrors = options.retryableErrors || DEFAULT_RETRYABLE_ERRORS;
    this.debug = options.debug ?? false;
    this.jitter = options.jitter ?? true; // Add random jitter by default

    // Validate that maxDelay >= initialDelay
    if (this.maxDelay < this.initialDelay) {
      throw new Error('maxDelay must be greater than or equal to initialDelay');
    }

    // Statistics
    this.stats = {
      totalAttempts: 0,
      totalRetries: 0,
      successfulRetries: 0,
      failedRetries: 0
    };
  }

  /**
   * Validate a numeric parameter
   * @param {*} value - Value to validate
   * @param {number} defaultValue - Default if undefined
   * @param {string} name - Parameter name for error messages
   * @param {number} min - Minimum allowed value
   * @param {number} max - Maximum allowed value
   * @returns {number} - Validated number
   * @private
   */
  _validateNumber(value, defaultValue, name, min, max) {
    if (value === undefined || value === null) {
      return defaultValue;
    }

    const num = Number(value);

    if (isNaN(num)) {
      throw new Error(`${name} must be a number, got: ${value}`);
    }

    if (num < min || num > max) {
      throw new Error(`${name} must be between ${min} and ${max}, got: ${num}`);
    }

    return num;
  }

  /**
   * Execute a function with retry logic
   * @param {Function} fn - Async function to execute
   * @param {Object} [context={}] - Context information for logging
   * @param {AbortSignal} [context.signal] - Optional abort signal for cancellation
   * @returns {Promise<*>} - Result from the function
   * @throws {Error} - If all retries are exhausted or operation is aborted
   */
  async executeWithRetry(fn, context = {}) {
    let lastError = null;
    let currentDelay = this.initialDelay;

    for (let attempt = 0; attempt <= this.maxRetries; attempt++) {
      // Check for abort signal
      if (context.signal?.aborted) {
        const abortError = new Error('Operation aborted');
        abortError.name = 'AbortError';
        throw abortError;
      }

      this.stats.totalAttempts++;

      try {
        if (this.debug && attempt > 0) {
          console.log(`[RetryManager] Retry attempt ${attempt}/${this.maxRetries}`, context);
        }

        const result = await fn();

        // Success!
        if (attempt > 0) {
          this.stats.successfulRetries++;
          if (this.debug) {
            console.log(`[RetryManager] ✅ Retry successful on attempt ${attempt + 1}`, context);
          }
        }

        return result;

      } catch (error) {
        lastError = error;
        const errorInfo = extractErrorInfo(error);

        // Check if we should retry
        const shouldRetry = isRetryableError(error, this.retryableErrors);
        const hasRetriesLeft = attempt < this.maxRetries;

        if (this.debug) {
          console.log(`[RetryManager] ❌ Attempt ${attempt + 1}/${this.maxRetries + 1} failed:`, {
            ...context,
            error: errorInfo,
            shouldRetry,
            hasRetriesLeft
          });
        }

        // If this is not a retryable error or no retries left, throw
        if (!shouldRetry) {
          if (this.debug) {
            console.log(`[RetryManager] Error is not retryable, failing immediately`, errorInfo);
          }
          throw error;
        }

        if (!hasRetriesLeft) {
          this.stats.failedRetries++;
          if (this.debug) {
            console.log(`[RetryManager] Max retries (${this.maxRetries}) exhausted`, context);
          }
          throw error;
        }

        // Wait before retrying with exponential backoff
        this.stats.totalRetries++;

        // Add jitter to prevent thundering herd
        let delayWithJitter = currentDelay;
        if (this.jitter) {
          // Add ±25% jitter
          const jitterAmount = currentDelay * 0.25;
          delayWithJitter = currentDelay + (Math.random() * jitterAmount * 2 - jitterAmount);
        }

        if (this.debug) {
          console.log(`[RetryManager] Waiting ${Math.round(delayWithJitter)}ms before retry...`);
        }

        await sleep(delayWithJitter);

        // Calculate next delay with exponential backoff
        currentDelay = Math.min(currentDelay * this.backoffFactor, this.maxDelay);
      }
    }

    // This should never be reached, but just in case
    throw lastError;
  }

  /**
   * Check if an error is retryable
   * @param {Error} error - The error to check
   * @returns {boolean} - True if error should be retried
   */
  isRetryable(error) {
    return isRetryableError(error, this.retryableErrors);
  }

  /**
   * Get retry statistics
   * @returns {Object} - Statistics object
   */
  getStats() {
    return { ...this.stats };
  }

  /**
   * Reset statistics
   */
  resetStats() {
    this.stats = {
      totalAttempts: 0,
      totalRetries: 0,
      successfulRetries: 0,
      failedRetries: 0
    };
  }
}

/**
 * Create a RetryManager from environment variables
 * @param {boolean} [debug=false] - Enable debug logging
 * @returns {RetryManager} - Configured RetryManager instance
 */
export function createRetryManagerFromEnv(debug = false) {
  const options = { debug };

  // Parse and validate environment variables
  if (process.env.MAX_RETRIES) {
    const parsed = parseInt(process.env.MAX_RETRIES, 10);
    if (!isNaN(parsed) && parsed >= 0 && parsed <= 50) {
      options.maxRetries = parsed;
    } else {
      console.warn(`[RetryManager] MAX_RETRIES must be between 0 and 50, using default`);
    }
  }

  if (process.env.RETRY_INITIAL_DELAY) {
    const parsed = parseInt(process.env.RETRY_INITIAL_DELAY, 10);
    if (!isNaN(parsed) && parsed >= 0 && parsed <= 60000) {
      options.initialDelay = parsed;
    } else {
      console.warn(`[RetryManager] RETRY_INITIAL_DELAY must be between 0 and 60000ms, using default`);
    }
  }

  if (process.env.RETRY_MAX_DELAY) {
    const parsed = parseInt(process.env.RETRY_MAX_DELAY, 10);
    if (!isNaN(parsed) && parsed >= 0 && parsed <= 300000) {
      options.maxDelay = parsed;
    } else {
      console.warn(`[RetryManager] RETRY_MAX_DELAY must be between 0 and 300000ms, using default`);
    }
  }

  if (process.env.RETRY_BACKOFF_FACTOR) {
    const parsed = parseFloat(process.env.RETRY_BACKOFF_FACTOR);
    if (!isNaN(parsed) && parsed >= 1 && parsed <= 10) {
      options.backoffFactor = parsed;
    } else {
      console.warn(`[RetryManager] RETRY_BACKOFF_FACTOR must be between 1 and 10, using default`);
    }
  }

  if (process.env.RETRY_JITTER === '0' || process.env.RETRY_JITTER === 'false') {
    options.jitter = false;
  }

  return new RetryManager(options);
}

// Export utility functions
export { isRetryableError, extractErrorInfo, DEFAULT_RETRYABLE_ERRORS };
