/**
 * FallbackManager - Handles provider and model fallback configuration
 *
 * Provides flexible fallback strategies for AI API calls:
 * - Same model across different providers (Azure Claude → Bedrock Claude)
 * - Different models on same provider (Claude 3.7 → Claude 3.5)
 * - Cross-provider fallback (Anthropic → OpenAI → Google)
 * - Custom fallback chains with full configuration
 */

import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { createAmazonBedrock } from '@ai-sdk/amazon-bedrock';

/**
 * Fallback strategies
 */
export const FALLBACK_STRATEGIES = {
  SAME_MODEL: 'same-model',       // Try same model on different providers
  SAME_PROVIDER: 'same-provider', // Try different models on same provider
  ANY: 'any',                     // Try any available provider/model
  CUSTOM: 'custom'                // Use custom provider list
};

/**
 * Provider configuration schema
 * @typedef {Object} ProviderConfig
 * @property {string} provider - Provider name: 'anthropic', 'openai', 'google', 'bedrock'
 * @property {string} [model] - Model name
 * @property {string} [apiKey] - API key
 * @property {string} [baseURL] - Custom API endpoint
 * @property {number} [maxRetries] - Max retries for this provider (overrides global)
 * @property {string} [region] - AWS region (for Bedrock)
 * @property {string} [accessKeyId] - AWS access key ID (for Bedrock)
 * @property {string} [secretAccessKey] - AWS secret access key (for Bedrock)
 * @property {string} [sessionToken] - AWS session token (for Bedrock)
 */

/**
 * Default model mappings for each provider
 */
const DEFAULT_MODELS = {
  anthropic: 'claude-sonnet-4-5-20250929',
  openai: 'gpt-4o',
  google: 'gemini-2.0-flash-exp',
  bedrock: 'anthropic.claude-sonnet-4-20250514-v1:0'
};

/**
 * FallbackManager class for handling provider and model fallback
 */
export class FallbackManager {
  /**
   * Create a new FallbackManager
   * @param {Object} options - Configuration options
   * @param {string} [options.strategy='any'] - Fallback strategy
   * @param {Array<string>} [options.models] - List of models for same-provider fallback
   * @param {Array<ProviderConfig>} [options.providers] - List of provider configurations
   * @param {boolean} [options.stopOnSuccess=true] - Stop on first success
   * @param {boolean} [options.continueOnNonRetryableError=false] - Continue to fallback on non-retryable errors
   * @param {number} [options.maxTotalAttempts=10] - Maximum total attempts across all providers
   * @param {boolean} [options.debug=false] - Enable debug logging
   */
  constructor(options = {}) {
    this.strategy = options.strategy || FALLBACK_STRATEGIES.ANY;
    this.models = Array.isArray(options.models) ? options.models : [];
    this.providers = Array.isArray(options.providers) ? options.providers : [];
    this.stopOnSuccess = options.stopOnSuccess ?? true;
    this.continueOnNonRetryableError = options.continueOnNonRetryableError ?? false;
    this.debug = options.debug ?? false;

    // Validate maxTotalAttempts
    const maxAttempts = options.maxTotalAttempts ?? 10;
    if (typeof maxAttempts !== 'number' || isNaN(maxAttempts) || maxAttempts < 1 || maxAttempts > 100) {
      throw new Error(`FallbackManager: maxTotalAttempts must be a number between 1 and 100, got: ${maxAttempts}`);
    }
    this.maxTotalAttempts = maxAttempts;

    // Statistics
    this.stats = {
      totalAttempts: 0,
      providerAttempts: {},
      successfulProvider: null,
      failedProviders: []
    };

    // Validate configuration
    this._validateConfiguration();
  }

  /**
   * Validate the fallback configuration
   * @private
   */
  _validateConfiguration() {
    if (this.strategy === FALLBACK_STRATEGIES.SAME_PROVIDER && this.models.length === 0) {
      throw new Error('FallbackManager: strategy "same-provider" requires models list');
    }

    if (this.strategy === FALLBACK_STRATEGIES.CUSTOM && this.providers.length === 0) {
      throw new Error('FallbackManager: strategy "custom" requires providers list');
    }

    // Validate provider configurations
    for (const config of this.providers) {
      if (!config.provider) {
        throw new Error('FallbackManager: Each provider config must have a "provider" field');
      }

      if (!['anthropic', 'openai', 'google', 'bedrock'].includes(config.provider)) {
        throw new Error(`FallbackManager: Invalid provider "${config.provider}". Must be: anthropic, openai, google, or bedrock`);
      }

      // Validate Bedrock configuration
      if (config.provider === 'bedrock') {
        const hasCredentials = config.accessKeyId && config.secretAccessKey && config.region;
        const hasApiKey = config.apiKey;

        if (!hasCredentials && !hasApiKey) {
          throw new Error('FallbackManager: Bedrock provider requires either (accessKeyId, secretAccessKey, region) or apiKey');
        }
      } else {
        // Other providers require apiKey
        if (!config.apiKey) {
          throw new Error(`FallbackManager: Provider "${config.provider}" requires apiKey`);
        }
      }
    }
  }

  /**
   * Create a provider instance from configuration
   * @param {ProviderConfig} config - Provider configuration
   * @returns {Object} - Provider instance
   * @throws {Error} - If provider creation fails
   * @private
   */
  _createProviderInstance(config) {
    try {
      switch (config.provider) {
        case 'anthropic':
          return createAnthropic({
            apiKey: config.apiKey,
            ...(config.baseURL && { baseURL: config.baseURL })
          });

        case 'openai':
          return createOpenAI({
            compatibility: 'strict',
            apiKey: config.apiKey,
            ...(config.baseURL && { baseURL: config.baseURL })
          });

        case 'google':
          return createGoogleGenerativeAI({
            apiKey: config.apiKey,
            ...(config.baseURL && { baseURL: config.baseURL })
          });

        case 'bedrock': {
          const bedrockConfig = {};

          if (config.apiKey) {
            bedrockConfig.apiKey = config.apiKey;
          } else if (config.accessKeyId && config.secretAccessKey) {
            bedrockConfig.accessKeyId = config.accessKeyId;
            bedrockConfig.secretAccessKey = config.secretAccessKey;
            if (config.sessionToken) {
              bedrockConfig.sessionToken = config.sessionToken;
            }
          }

          if (config.region) {
            bedrockConfig.region = config.region;
          }

          if (config.baseURL) {
            bedrockConfig.baseURL = config.baseURL;
          }

          return createAmazonBedrock(bedrockConfig);
        }

        default:
          throw new Error(`FallbackManager: Unknown provider "${config.provider}"`);
      }
    } catch (error) {
      // Re-throw with more context
      const providerName = this._getProviderDisplayName(config);
      throw new Error(`Failed to create provider instance for ${providerName}: ${error.message}`);
    }
  }

  /**
   * Get the model name for a provider configuration
   * @param {ProviderConfig} config - Provider configuration
   * @returns {string} - Model name
   * @private
   */
  _getModelName(config) {
    return config.model || DEFAULT_MODELS[config.provider];
  }

  /**
   * Get provider display name for logging
   * @param {ProviderConfig} config - Provider configuration
   * @returns {string} - Display name
   * @private
   */
  _getProviderDisplayName(config) {
    const model = this._getModelName(config);
    const provider = config.provider;
    const url = config.baseURL ? ` (${config.baseURL})` : '';
    return `${provider}/${model}${url}`;
  }

  /**
   * Execute a function with fallback support
   * @param {Function} fn - Function that takes (provider, model, config) and returns a Promise
   * @returns {Promise<*>} - Result from the function
   * @throws {Error} - If all fallbacks are exhausted
   */
  async executeWithFallback(fn) {
    if (this.providers.length === 0) {
      throw new Error('FallbackManager: No providers configured for fallback');
    }

    let lastError = null;
    let totalAttempts = 0;

    for (const config of this.providers) {
      if (totalAttempts >= this.maxTotalAttempts) {
        if (this.debug) {
          console.log(`[FallbackManager] ⚠️  Max total attempts (${this.maxTotalAttempts}) reached`);
        }
        break;
      }

      totalAttempts++;
      this.stats.totalAttempts++;

      const providerName = this._getProviderDisplayName(config);
      this.stats.providerAttempts[providerName] = (this.stats.providerAttempts[providerName] || 0) + 1;

      try {
        if (this.debug) {
          console.log(`[FallbackManager] Attempting provider: ${providerName} (attempt ${totalAttempts}/${this.maxTotalAttempts})`);
        }

        const provider = this._createProviderInstance(config);
        const model = this._getModelName(config);

        const result = await fn(provider, model, config);

        // Success!
        this.stats.successfulProvider = providerName;

        if (this.debug) {
          console.log(`[FallbackManager] ✅ Success with provider: ${providerName}`);
        }

        return result;

      } catch (error) {
        lastError = error;
        const errorInfo = {
          message: error.message || error.toString(),
          type: error.type || error.constructor.name,
          statusCode: error.statusCode || error.status
        };

        this.stats.failedProviders.push({
          provider: providerName,
          error: errorInfo
        });

        if (this.debug) {
          console.log(`[FallbackManager] ❌ Failed with provider: ${providerName}`, errorInfo);
        }

        // Check if we should continue to next provider
        // If error is not retryable and continueOnNonRetryableError is false, stop
        if (!this.continueOnNonRetryableError && error.nonRetryable) {
          if (this.debug) {
            console.log(`[FallbackManager] Non-retryable error, stopping fallback chain`);
          }
          throw error;
        }

        // Continue to next provider
        if (this.debug) {
          const remaining = this.providers.length - (this.providers.indexOf(config) + 1);
          console.log(`[FallbackManager] Trying next provider (${remaining} remaining)...`);
        }
      }
    }

    // All providers failed
    if (this.debug) {
      console.log(`[FallbackManager] ❌ All providers exhausted. Total attempts: ${totalAttempts}`);
    }

    // Enhance error with fallback context
    const fallbackError = new Error(
      `All provider fallbacks exhausted after ${totalAttempts} attempts. Last error: ${lastError?.message || 'Unknown error'}`
    );
    fallbackError.cause = lastError;
    fallbackError.stats = this.getStats();
    fallbackError.allProvidersFailed = true;

    throw fallbackError;
  }

  /**
   * Get fallback statistics
   * @returns {Object} - Statistics object
   */
  getStats() {
    return {
      ...this.stats,
      providerAttempts: { ...this.stats.providerAttempts },
      failedProviders: [...this.stats.failedProviders]
    };
  }

  /**
   * Reset statistics
   */
  resetStats() {
    this.stats = {
      totalAttempts: 0,
      providerAttempts: {},
      successfulProvider: null,
      failedProviders: []
    };
  }
}

/**
 * Create a FallbackManager from environment variables
 * @param {boolean} [debug=false] - Enable debug logging
 * @returns {FallbackManager|null} - Configured FallbackManager instance or null if no fallback config
 */
export function createFallbackManagerFromEnv(debug = false) {
  const fallbackProvidersEnv = process.env.FALLBACK_PROVIDERS;
  const fallbackModelsEnv = process.env.FALLBACK_MODELS;

  // If no fallback configuration, return null
  if (!fallbackProvidersEnv && !fallbackModelsEnv) {
    return null;
  }

  let providers = [];
  let models = [];
  let strategy = FALLBACK_STRATEGIES.ANY;

  // Parse providers configuration
  if (fallbackProvidersEnv) {
    try {
      providers = JSON.parse(fallbackProvidersEnv);
      strategy = FALLBACK_STRATEGIES.CUSTOM;
    } catch (error) {
      console.error('[FallbackManager] Failed to parse FALLBACK_PROVIDERS:', error.message);
      return null;
    }
  }

  // Parse models configuration
  if (fallbackModelsEnv) {
    try {
      models = JSON.parse(fallbackModelsEnv);
      strategy = FALLBACK_STRATEGIES.SAME_PROVIDER;
    } catch (error) {
      console.error('[FallbackManager] Failed to parse FALLBACK_MODELS:', error.message);
      return null;
    }
  }

  const maxTotalAttempts = process.env.FALLBACK_MAX_TOTAL_ATTEMPTS
    ? parseInt(process.env.FALLBACK_MAX_TOTAL_ATTEMPTS, 10)
    : 10;

  return new FallbackManager({
    strategy,
    providers,
    models,
    maxTotalAttempts,
    debug
  });
}

/**
 * Build a fallback provider list from current environment
 * @param {Object} options - Options for building the list
 * @param {string} [options.primaryProvider] - Primary provider to try first
 * @param {string} [options.primaryModel] - Primary model to use
 * @returns {Array<ProviderConfig>} - List of provider configurations
 */
export function buildFallbackProvidersFromEnv(options = {}) {
  const providers = [];

  // Get all available API keys from environment
  const anthropicApiKey = process.env.ANTHROPIC_API_KEY || process.env.ANTHROPIC_AUTH_TOKEN;
  const openaiApiKey = process.env.OPENAI_API_KEY;
  const googleApiKey = process.env.GOOGLE_GENERATIVE_AI_API_KEY || process.env.GOOGLE_API_KEY;
  const awsAccessKeyId = process.env.AWS_ACCESS_KEY_ID;
  const awsSecretAccessKey = process.env.AWS_SECRET_ACCESS_KEY;
  const awsRegion = process.env.AWS_REGION;
  const awsApiKey = process.env.AWS_BEDROCK_API_KEY;

  // Get custom URLs
  const llmBaseUrl = process.env.LLM_BASE_URL;
  const anthropicApiUrl = process.env.ANTHROPIC_API_URL || process.env.ANTHROPIC_BASE_URL || llmBaseUrl;
  const openaiApiUrl = process.env.OPENAI_API_URL || llmBaseUrl;
  const googleApiUrl = process.env.GOOGLE_API_URL || llmBaseUrl;
  const awsBedrockBaseUrl = process.env.AWS_BEDROCK_BASE_URL || llmBaseUrl;

  // Build primary provider config
  const primaryProvider = options.primaryProvider?.toLowerCase();
  const primaryModel = options.primaryModel;

  // Add primary provider first if specified
  if (primaryProvider === 'anthropic' && anthropicApiKey) {
    providers.push({
      provider: 'anthropic',
      apiKey: anthropicApiKey,
      ...(anthropicApiUrl && { baseURL: anthropicApiUrl }),
      ...(primaryModel && { model: primaryModel })
    });
  } else if (primaryProvider === 'openai' && openaiApiKey) {
    providers.push({
      provider: 'openai',
      apiKey: openaiApiKey,
      ...(openaiApiUrl && { baseURL: openaiApiUrl }),
      ...(primaryModel && { model: primaryModel })
    });
  } else if (primaryProvider === 'google' && googleApiKey) {
    providers.push({
      provider: 'google',
      apiKey: googleApiKey,
      ...(googleApiUrl && { baseURL: googleApiUrl }),
      ...(primaryModel && { model: primaryModel })
    });
  } else if (primaryProvider === 'bedrock' && ((awsAccessKeyId && awsSecretAccessKey && awsRegion) || awsApiKey)) {
    const config = { provider: 'bedrock' };

    if (awsApiKey) {
      config.apiKey = awsApiKey;
    } else {
      config.accessKeyId = awsAccessKeyId;
      config.secretAccessKey = awsSecretAccessKey;
      config.region = awsRegion;
      if (process.env.AWS_SESSION_TOKEN) {
        config.sessionToken = process.env.AWS_SESSION_TOKEN;
      }
    }

    if (awsBedrockBaseUrl) config.baseURL = awsBedrockBaseUrl;
    if (primaryModel) config.model = primaryModel;

    providers.push(config);
  }

  // Add remaining available providers as fallbacks
  if (anthropicApiKey && primaryProvider !== 'anthropic') {
    providers.push({
      provider: 'anthropic',
      apiKey: anthropicApiKey,
      ...(anthropicApiUrl && { baseURL: anthropicApiUrl })
    });
  }

  if (openaiApiKey && primaryProvider !== 'openai') {
    providers.push({
      provider: 'openai',
      apiKey: openaiApiKey,
      ...(openaiApiUrl && { baseURL: openaiApiUrl })
    });
  }

  if (googleApiKey && primaryProvider !== 'google') {
    providers.push({
      provider: 'google',
      apiKey: googleApiKey,
      ...(googleApiUrl && { baseURL: googleApiUrl })
    });
  }

  if (((awsAccessKeyId && awsSecretAccessKey && awsRegion) || awsApiKey) && primaryProvider !== 'bedrock') {
    const config = { provider: 'bedrock' };

    if (awsApiKey) {
      config.apiKey = awsApiKey;
    } else {
      config.accessKeyId = awsAccessKeyId;
      config.secretAccessKey = awsSecretAccessKey;
      config.region = awsRegion;
      if (process.env.AWS_SESSION_TOKEN) {
        config.sessionToken = process.env.AWS_SESSION_TOKEN;
      }
    }

    if (awsBedrockBaseUrl) config.baseURL = awsBedrockBaseUrl;

    providers.push(config);
  }

  return providers;
}
