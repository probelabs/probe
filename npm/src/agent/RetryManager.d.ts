/**
 * TypeScript type definitions for RetryManager
 */

/**
 * Retry configuration options
 */
export interface RetryOptions {
  /** Maximum number of retry attempts (0-100) */
  maxRetries?: number;
  /** Initial delay in milliseconds (0-60000) */
  initialDelay?: number;
  /** Maximum delay in milliseconds (0-300000) */
  maxDelay?: number;
  /** Exponential backoff multiplier (1-10) */
  backoffFactor?: number;
  /** List of retryable error patterns */
  retryableErrors?: string[];
  /** Enable debug logging */
  debug?: boolean;
  /** Add random jitter to delays (default: true) */
  jitter?: boolean;
}

/**
 * Context information for retry operations
 */
export interface RetryContext {
  /** Provider name for logging */
  provider?: string;
  /** Model name for logging */
  model?: string;
  /** AbortSignal for cancellation */
  signal?: AbortSignal;
  /** Additional context data */
  [key: string]: any;
}

/**
 * Retry statistics
 */
export interface RetryStats {
  /** Total number of attempts made */
  totalAttempts: number;
  /** Total number of retries (excluding initial attempts) */
  totalRetries: number;
  /** Number of successful retries */
  successfulRetries: number;
  /** Number of failed retries (exhausted max retries) */
  failedRetries: number;
}

/**
 * Error information extracted from an error object
 */
export interface ErrorInfo {
  /** Error message */
  message: string;
  /** Error type or constructor name */
  type: string;
  /** Error code if available */
  code?: string;
  /** HTTP status code if available */
  statusCode?: number;
  /** Provider name if available */
  provider?: string;
  /** Whether the error is retryable */
  isRetryable: boolean;
}

/**
 * RetryManager class for handling retry logic with exponential backoff
 */
export class RetryManager {
  /** Maximum retry attempts */
  maxRetries: number;
  /** Initial delay in milliseconds */
  initialDelay: number;
  /** Maximum delay in milliseconds */
  maxDelay: number;
  /** Exponential backoff multiplier */
  backoffFactor: number;
  /** List of retryable error patterns */
  retryableErrors: string[];
  /** Debug logging enabled */
  debug: boolean;
  /** Jitter enabled */
  jitter: boolean;
  /** Retry statistics */
  stats: RetryStats;

  /**
   * Create a new RetryManager
   * @param options - Retry configuration options
   */
  constructor(options?: RetryOptions);

  /**
   * Execute a function with retry logic
   * @param fn - Async function to execute
   * @param context - Context information for logging
   * @returns Result from the function
   * @throws Error if all retries are exhausted or operation is aborted
   */
  executeWithRetry<T>(
    fn: () => Promise<T>,
    context?: RetryContext
  ): Promise<T>;

  /**
   * Check if an error is retryable
   * @param error - The error to check
   * @returns True if error should be retried
   */
  isRetryable(error: Error): boolean;

  /**
   * Get retry statistics
   * @returns Statistics object (copy)
   */
  getStats(): RetryStats;

  /**
   * Reset statistics
   */
  resetStats(): void;
}

/**
 * Check if an error is retryable based on error patterns
 * @param error - The error to check
 * @param retryableErrors - List of retryable error patterns
 * @returns True if error should be retried
 */
export function isRetryableError(
  error: Error,
  retryableErrors?: string[]
): boolean;

/**
 * Extract meaningful error information for logging
 * @param error - The error to extract info from
 * @returns Error information object
 */
export function extractErrorInfo(error: Error): ErrorInfo;

/**
 * Create a RetryManager from environment variables
 * @param debug - Enable debug logging
 * @returns Configured RetryManager instance
 */
export function createRetryManagerFromEnv(debug?: boolean): RetryManager;

/**
 * Default retryable error patterns
 */
export const DEFAULT_RETRYABLE_ERRORS: string[];
