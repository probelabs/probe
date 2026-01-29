/**
 * Structured error types for AI-friendly error handling
 * @module utils/error-types
 */

/**
 * Error categories for classification
 */
export const ErrorCategory = {
  PATH_ERROR: 'path_error',           // Path doesn't exist, not a directory, permission denied
  PARAMETER_ERROR: 'parameter_error', // Invalid parameters provided
  TIMEOUT_ERROR: 'timeout_error',     // Operation timed out
  API_ERROR: 'api_error',             // AI provider errors (rate limit, token limit)
  DELEGATION_ERROR: 'delegation_error', // Subagent failures
  INTERNAL_ERROR: 'internal_error'    // Unexpected system errors
};

/**
 * Escape special characters for XML output
 * @param {string} str - String to escape
 * @returns {string} - Escaped string
 */
function escapeXml(str) {
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

/**
 * Base class for structured errors with AI-friendly information
 */
export class ProbeError extends Error {
  /**
   * Create a ProbeError
   * @param {string} message - Error message
   * @param {Object} options - Options
   * @param {string} [options.category] - Error category from ErrorCategory
   * @param {boolean} [options.recoverable] - Whether the error is recoverable by AI action
   * @param {string} [options.suggestion] - Suggested action for recovery
   * @param {Object} [options.details] - Additional error details
   * @param {Error} [options.originalError] - Original error that was wrapped
   */
  constructor(message, options = {}) {
    super(message);
    this.name = 'ProbeError';
    this.category = options.category || ErrorCategory.INTERNAL_ERROR;
    this.recoverable = options.recoverable ?? false;
    this.suggestion = options.suggestion || null;
    this.details = options.details || {};
    this.originalError = options.originalError || null;
  }

  /**
   * Format error as XML for AI consumption
   * @returns {string} - XML-formatted error
   */
  toXml() {
    const parts = [
      `<error type="${this.category}" recoverable="${this.recoverable}">`,
      `<message>${escapeXml(this.message)}</message>`
    ];

    if (this.suggestion) {
      parts.push(`<suggestion>${escapeXml(this.suggestion)}</suggestion>`);
    }

    if (Object.keys(this.details).length > 0) {
      parts.push(`<details>${escapeXml(JSON.stringify(this.details))}</details>`);
    }

    parts.push('</error>');
    return parts.join('\n');
  }

  /**
   * Format error for plain text (backward compatible)
   * @returns {string} - Plain text error
   */
  toString() {
    return `Error: ${this.message}`;
  }
}

/**
 * Path-related errors (not found, not a directory, permission denied)
 */
export class PathError extends ProbeError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      category: ErrorCategory.PATH_ERROR,
      recoverable: options.recoverable ?? true,
      suggestion: options.suggestion || 'Please verify the path exists or try searching in a different directory.'
    });
    this.name = 'PathError';
  }
}

/**
 * Parameter validation errors
 */
export class ParameterError extends ProbeError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      category: ErrorCategory.PARAMETER_ERROR,
      recoverable: options.recoverable ?? true,
      suggestion: options.suggestion || 'Please check and correct the parameter values.'
    });
    this.name = 'ParameterError';
  }
}

/**
 * Timeout errors
 */
export class TimeoutError extends ProbeError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      category: ErrorCategory.TIMEOUT_ERROR,
      recoverable: options.recoverable ?? true,
      suggestion: options.suggestion || 'The operation timed out. Try a more specific query or increase the timeout.'
    });
    this.name = 'TimeoutError';
  }
}

/**
 * API/AI provider errors (rate limit, token limit, etc.)
 */
export class ApiError extends ProbeError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      category: ErrorCategory.API_ERROR,
      recoverable: options.recoverable ?? false,
      suggestion: options.suggestion || 'This is an API provider error. The system will retry automatically if possible.'
    });
    this.name = 'ApiError';
  }
}

/**
 * Delegation/subagent errors
 */
export class DelegationError extends ProbeError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      category: ErrorCategory.DELEGATION_ERROR,
      recoverable: options.recoverable ?? true,
      suggestion: options.suggestion || 'The delegated task failed. Consider breaking down the task further or trying a different approach.'
    });
    this.name = 'DelegationError';
  }
}

/**
 * Categorize any error into a structured ProbeError
 * @param {Error|string} error - Error to categorize
 * @returns {ProbeError} - Structured error with category and suggestions
 */
export function categorizeError(error) {
  // If already a ProbeError, return as-is
  if (error instanceof ProbeError) {
    return error;
  }

  const message = error?.message || String(error);
  const lowerMessage = message.toLowerCase();
  const errorCode = error?.code?.toLowerCase() || '';

  // Path-related errors
  if (lowerMessage.includes('path does not exist') ||
      lowerMessage.includes('no such file or directory') ||
      errorCode === 'enoent') {
    return new PathError(message, {
      originalError: error,
      suggestion: 'The specified path does not exist. Please verify the path or use a different directory.'
    });
  }

  if (lowerMessage.includes('not a directory') || errorCode === 'enotdir') {
    return new PathError(message, {
      originalError: error,
      suggestion: 'The path is not a directory. Please provide a valid directory path.'
    });
  }

  if (lowerMessage.includes('permission denied') || errorCode === 'eacces') {
    return new PathError(message, {
      originalError: error,
      recoverable: false,
      suggestion: 'Permission denied. This is a system-level restriction that cannot be resolved by changing the query.'
    });
  }

  // Timeout errors
  if (lowerMessage.includes('timed out') ||
      lowerMessage.includes('timeout') ||
      errorCode === 'etimedout' ||
      error?.killed === true) {
    return new TimeoutError(message, {
      originalError: error,
      suggestion: 'The operation timed out. Try a more specific query, reduce the search scope, or increase the timeout.'
    });
  }

  // API errors - rate limiting and overload
  const rateLimitPatterns = ['rate_limit', 'rate limit', '429', 'too many requests', 'overloaded'];
  if (rateLimitPatterns.some(p => lowerMessage.includes(p))) {
    return new ApiError(message, {
      originalError: error,
      recoverable: true,
      suggestion: 'API rate limit or overload encountered. The system will retry automatically with backoff.'
    });
  }

  // API errors - server errors
  const serverErrorPatterns = ['500', '502', '503', '504', 'internal server error', 'bad gateway', 'service unavailable'];
  if (serverErrorPatterns.some(p => lowerMessage.includes(p))) {
    return new ApiError(message, {
      originalError: error,
      recoverable: true,
      suggestion: 'API server error encountered. The system will retry automatically.'
    });
  }

  // API errors - context/token limits
  if ((lowerMessage.includes('context') || lowerMessage.includes('token')) &&
      (lowerMessage.includes('limit') || lowerMessage.includes('exceed'))) {
    return new ApiError(message, {
      originalError: error,
      recoverable: true,
      suggestion: 'Context or token limit exceeded. The conversation may be automatically compacted.'
    });
  }

  // API errors - authentication
  if (lowerMessage.includes('invalid') && (lowerMessage.includes('api') || lowerMessage.includes('key') || lowerMessage.includes('auth'))) {
    return new ApiError(message, {
      originalError: error,
      recoverable: false,
      suggestion: 'API authentication error. Please check the API key configuration.'
    });
  }

  // Parameter validation errors
  if (lowerMessage.includes('required') ||
      lowerMessage.includes('must be') ||
      lowerMessage.includes('invalid parameter') ||
      lowerMessage.includes('missing parameter')) {
    return new ParameterError(message, {
      originalError: error
    });
  }

  // Delegation errors
  if (lowerMessage.includes('delegation failed') ||
      lowerMessage.includes('delegate agent') ||
      lowerMessage.includes('subagent')) {
    return new DelegationError(message, {
      originalError: error
    });
  }

  // Network errors
  if (errorCode === 'econnreset' ||
      errorCode === 'econnrefused' ||
      errorCode === 'enotfound' ||
      lowerMessage.includes('network') ||
      lowerMessage.includes('connection')) {
    return new ApiError(message, {
      originalError: error,
      recoverable: true,
      suggestion: 'Network error encountered. The system will retry automatically.'
    });
  }

  // Default: internal error
  return new ProbeError(message, {
    category: ErrorCategory.INTERNAL_ERROR,
    recoverable: false,
    originalError: error,
    suggestion: 'An unexpected error occurred. Please check the error message for details.'
  });
}

/**
 * Format an error for AI consumption as XML
 * Works with both ProbeError and regular Error objects
 * @param {Error} error - Error to format
 * @returns {string} - XML-formatted error string
 */
export function formatErrorForAI(error) {
  const structuredError = categorizeError(error);
  return structuredError.toXml();
}
