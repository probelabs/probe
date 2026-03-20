/**
 * Shared provider/model creation utilities.
 * Single source of truth for AI SDK provider instantiation.
 * Used by FallbackManager, ProbeAgent, and lightweight LLM calls (e.g., dedup checker).
 * @module utils/provider
 */

import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { createAmazonBedrock } from '@ai-sdk/amazon-bedrock';

export const DEFAULT_MODELS = {
	anthropic: 'claude-sonnet-4-6',
	openai: 'gpt-5.2',
	google: 'gemini-2.5-flash',
	bedrock: 'anthropic.claude-sonnet-4-6'
};

/**
 * Create a provider instance from a config object.
 * @param {{ provider: string, apiKey?: string, baseURL?: string, region?: string, accessKeyId?: string, secretAccessKey?: string, sessionToken?: string }} config
 * @returns {object} AI SDK provider instance
 */
export function createProviderInstance(config) {
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
			if (config.region) bedrockConfig.region = config.region;
			if (config.baseURL) bedrockConfig.baseURL = config.baseURL;
			return createAmazonBedrock(bedrockConfig);
		}

		default:
			throw new Error(`Unknown provider "${config.provider}"`);
	}
}

/**
 * Resolve API key for a provider from environment variables.
 * @param {string} providerName - 'anthropic' | 'openai' | 'google' | 'bedrock'
 * @returns {string|undefined}
 */
export function resolveApiKey(providerName) {
	switch (providerName) {
		case 'anthropic':
			return process.env.ANTHROPIC_API_KEY || process.env.ANTHROPIC_AUTH_TOKEN;
		case 'openai':
			return process.env.OPENAI_API_KEY;
		case 'google':
			return process.env.GOOGLE_GENERATIVE_AI_API_KEY || process.env.GOOGLE_API_KEY || process.env.GEMINI_API_KEY;
		case 'bedrock':
			return process.env.AWS_BEDROCK_API_KEY;
		default:
			return undefined;
	}
}

/**
 * Create a language model instance from provider name + model name.
 * Resolves API keys from environment automatically.
 * Returns null on failure (graceful degradation for optional features).
 * @param {string} providerName - 'anthropic' | 'openai' | 'google' | 'bedrock'
 * @param {string} modelName - Model identifier (e.g., 'gemini-2.0-flash')
 * @returns {Promise<object|null>} AI SDK model instance, or null
 */
export async function createLanguageModel(providerName, modelName) {
	if (!providerName) return null;
	const resolvedModel = modelName || DEFAULT_MODELS[providerName];
	if (!resolvedModel) return null;
	try {
		const apiKey = resolveApiKey(providerName);
		const provider = createProviderInstance({ provider: providerName, ...(apiKey ? { apiKey } : {}) });
		return provider(resolvedModel);
	} catch {
		return null;
	}
}
