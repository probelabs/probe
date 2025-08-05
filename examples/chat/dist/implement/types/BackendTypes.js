/**
 * Type definitions for implementation backends
 * @module BackendTypes
 */

/**
 * @typedef {Object} BackendCapabilities
 * @property {string[]} supportsLanguages - Supported programming languages
 * @property {boolean} supportsStreaming - Whether backend supports streaming output
 * @property {boolean} supportsRollback - Whether backend supports rollback operations
 * @property {boolean} supportsDirectFileEdit - Whether backend can directly edit files
 * @property {boolean} supportsPlanGeneration - Whether backend can generate implementation plans
 * @property {boolean} supportsTestGeneration - Whether backend can generate tests
 * @property {number} maxConcurrentSessions - Maximum concurrent implementation sessions
 */

/**
 * @typedef {Object} ImplementRequest
 * @property {string} sessionId - Unique session identifier
 * @property {string} task - Implementation task description
 * @property {Object} [context] - Optional context information
 * @property {string} [context.workingDirectory] - Working directory for implementation
 * @property {string[]} [context.allowedFiles] - List of files allowed to be modified
 * @property {string} [context.language] - Primary programming language
 * @property {string} [context.additionalContext] - Additional context information
 * @property {Object} [options] - Optional implementation options
 * @property {boolean} [options.autoCommit] - Whether to auto-commit changes
 * @property {boolean} [options.generateTests] - Whether to generate tests
 * @property {boolean} [options.dryRun] - Whether to perform a dry run
 * @property {number} [options.maxTokens] - Maximum tokens to use
 * @property {number} [options.temperature] - Model temperature
 * @property {Object} [callbacks] - Optional callback functions
 * @property {Function} [callbacks.onProgress] - Progress update callback
 * @property {Function} [callbacks.onFileChange] - File change callback
 * @property {Function} [callbacks.onError] - Error callback
 */

/**
 * @typedef {Object} ImplementResult
 * @property {boolean} success - Whether implementation was successful
 * @property {string} sessionId - Session identifier
 * @property {string} output - Implementation output
 * @property {FileChange[]} [changes] - List of file changes
 * @property {Object} [metrics] - Implementation metrics
 * @property {number} [metrics.executionTime] - Execution time in milliseconds
 * @property {number} [metrics.tokensUsed] - Number of tokens used
 * @property {number} [metrics.filesModified] - Number of files modified
 * @property {number} [metrics.linesChanged] - Number of lines changed
 * @property {BackendError} [error] - Error information if failed
 * @property {Object} [metadata] - Additional metadata
 */

/**
 * @typedef {Object} FileChange
 * @property {string} path - File path
 * @property {string} type - Change type: 'created', 'modified', 'deleted'
 * @property {string} [description] - Description of change
 * @property {number} [linesAdded] - Number of lines added
 * @property {number} [linesRemoved] - Number of lines removed
 * @property {string} [diff] - Diff content
 */

/**
 * @typedef {Object} BackendStatus
 * @property {string} status - Status: 'pending', 'running', 'completed', 'failed', 'cancelled'
 * @property {number} [progress] - Progress percentage (0-100)
 * @property {string} [message] - Status message
 * @property {Object} [details] - Additional status details
 */

/**
 * @typedef {Object} ProgressUpdate
 * @property {string} sessionId - Session identifier
 * @property {string} message - Progress message
 * @property {string} [type] - Message type: 'stdout', 'stderr', 'info'
 * @property {number} [progress] - Progress percentage
 * @property {Object} [data] - Additional data
 */

/**
 * @typedef {Object} BackendError
 * @property {string} message - Error message
 * @property {string} type - Error type
 * @property {string} [code] - Error code
 * @property {Object} [details] - Additional error details
 * @property {string} timestamp - Error timestamp
 */

/**
 * @typedef {Object} Dependency
 * @property {string} name - Dependency name
 * @property {string} type - Dependency type: 'npm', 'pip', 'environment', 'system'
 * @property {string} [version] - Required version
 * @property {string} [description] - Dependency description
 * @property {string} [installCommand] - Installation command
 */

/**
 * @typedef {Object} BackendConfig
 * @property {string} [apiKey] - API key for cloud-based backends
 * @property {string} [model] - Model to use
 * @property {number} [timeout] - Timeout in milliseconds
 * @property {Object} [environment] - Environment variables
 * @property {Object} [additional] - Additional backend-specific configuration
 */

/**
 * @typedef {Object} BackendInfo
 * @property {string} name - Backend name
 * @property {string} version - Backend version
 * @property {string} description - Backend description
 * @property {boolean} available - Whether backend is available
 * @property {BackendCapabilities} capabilities - Backend capabilities
 * @property {Dependency[]} dependencies - Required dependencies
 */

/**
 * @typedef {Object} ValidationResult
 * @property {boolean} valid - Whether validation passed
 * @property {string[]} errors - List of validation errors
 * @property {string[]} [warnings] - List of validation warnings
 */

module.exports = {
  // Type exports for JSDoc references
};