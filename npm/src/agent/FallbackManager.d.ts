/**
 * TypeScript type definitions for FallbackManager
 */

/**
 * Fallback strategies
 */
export const FALLBACK_STRATEGIES: {
  /** Try same model on different providers */
  SAME_MODEL: 'same-model';
  /** Try different models on same provider */
  SAME_PROVIDER: 'same-provider';
  /** Try any available provider/model */
  ANY: 'any';
  /** Use custom provider list */
  CUSTOM: 'custom';
};

export type FallbackStrategy =
  | 'same-model'
  | 'same-provider'
  | 'any'
  | 'custom';

/**
 * Provider configuration
 */
export interface ProviderConfig {
  /** Provider name */
  provider: 'anthropic' | 'openai' | 'google' | 'bedrock';
  /** Model name (uses provider default if omitted) */
  model?: string;
  /** API key for the provider */
  apiKey?: string;
  /** Custom API endpoint */
  baseURL?: string;
  /** Max retries for this provider (overrides global) */
  maxRetries?: number;

  // AWS Bedrock specific
  /** AWS region (for Bedrock) */
  region?: string;
  /** AWS access key ID (for Bedrock) */
  accessKeyId?: string;
  /** AWS secret access key (for Bedrock) */
  secretAccessKey?: string;
  /** AWS session token (for Bedrock) */
  sessionToken?: string;
}

/**
 * Fallback configuration options
 */
export interface FallbackOptions {
  /** Fallback strategy */
  strategy?: FallbackStrategy;
  /** List of models for same-provider fallback */
  models?: string[];
  /** List of provider configurations for custom fallback */
  providers?: ProviderConfig[];
  /** Stop on first successful response */
  stopOnSuccess?: boolean;
  /** Continue to fallback on non-retryable errors */
  continueOnNonRetryableError?: boolean;
  /** Maximum total attempts across all providers (1-100) */
  maxTotalAttempts?: number;
  /** Enable debug logging */
  debug?: boolean;
}

/**
 * Fallback statistics
 */
export interface FallbackStats {
  /** Total number of provider attempts */
  totalAttempts: number;
  /** Number of attempts per provider */
  providerAttempts: Record<string, number>;
  /** Name of the successful provider */
  successfulProvider: string | null;
  /** List of failed providers with error details */
  failedProviders: Array<{
    provider: string;
    error: {
      message: string;
      type: string;
      statusCode?: number;
    };
  }>;
}

/**
 * Auto-fallback provider build options
 */
export interface BuildFallbackOptions {
  /** Primary provider to try first */
  primaryProvider?: string;
  /** Primary model to use */
  primaryModel?: string;
}

/**
 * FallbackManager class for handling provider and model fallback
 */
export class FallbackManager {
  /** Fallback strategy */
  strategy: FallbackStrategy;
  /** List of models for same-provider fallback */
  models: string[];
  /** List of provider configurations */
  providers: ProviderConfig[];
  /** Stop on first success */
  stopOnSuccess: boolean;
  /** Continue on non-retryable errors */
  continueOnNonRetryableError: boolean;
  /** Maximum total attempts */
  maxTotalAttempts: number;
  /** Debug logging enabled */
  debug: boolean;
  /** Fallback statistics */
  stats: FallbackStats;

  /**
   * Create a new FallbackManager
   * @param options - Fallback configuration options
   */
  constructor(options?: FallbackOptions);

  /**
   * Execute a function with fallback support
   * @param fn - Function that takes (provider, model, config) and returns a Promise
   * @returns Result from the function
   * @throws Error if all fallbacks are exhausted
   */
  executeWithFallback<T>(
    fn: (
      provider: any,
      model: string,
      config: ProviderConfig
    ) => Promise<T>
  ): Promise<T>;

  /**
   * Get fallback statistics
   * @returns Statistics object (copy)
   */
  getStats(): FallbackStats;

  /**
   * Reset statistics
   */
  resetStats(): void;
}

/**
 * Create a FallbackManager from environment variables
 * @param debug - Enable debug logging
 * @returns Configured FallbackManager instance or null if no config found
 */
export function createFallbackManagerFromEnv(
  debug?: boolean
): FallbackManager | null;

/**
 * Build a fallback provider list from current environment
 * @param options - Options for building the list
 * @returns List of provider configurations
 */
export function buildFallbackProvidersFromEnv(
  options?: BuildFallbackOptions
): ProviderConfig[];

/**
 * Default model mappings for each provider
 */
export const DEFAULT_MODELS: Record<string, string>;
