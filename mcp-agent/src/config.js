
// Configuration for the probe-agent-mcp server
import zod from 'zod';
import dotenv from 'dotenv';
import { existsSync } from 'fs';
import path from 'path';

// Load environment variables
dotenv.config();

// Define configuration schema
const configSchema = zod.object({
	// API keys
	anthropicApiKey: zod.string().optional(),
	openaiApiKey: zod.string().optional(),
	googleApiKey: zod.string().optional(),

	// API URLs
	anthropicApiUrl: zod.string().default('https://api.anthropic.com/v1'),
	openaiApiUrl: zod.string().default('https://api.openai.com/v1'),
	googleApiUrl: zod.string().default('https://generativelanguage.googleapis.com'),

	// Model configuration
	modelName: zod.string().optional(),
	defaultAnthropicModel: zod.string().default('claude-3-7-sonnet-latest'),
	defaultOpenAIModel: zod.string().default('gpt-4o-2024-05-13'),
	defaultGoogleModel: zod.string().default('gemini-1.5-pro-latest'),

	// Force specific provider
	forceProvider: zod.enum(['anthropic', 'openai', 'google']).optional(),

	// Token limits
	maxTokens: zod.number().default(4000),
	maxHistoryMessages: zod.number().default(20),

	// Allowed folders
	allowedFolders: zod.array(zod.string()).default([]),

	// Debug mode
	debug: zod.boolean().default(false)
});

// Parse and validate allowed folders from environment variable
const allowedFolders = process.env.ALLOWED_FOLDERS
	? process.env.ALLOWED_FOLDERS.split(',').map(folder => folder.trim()).filter(Boolean)
	: [];

// Validate folders exist
console.error('Configured search folders:');
for (const folder of allowedFolders) {
	const exists = existsSync(folder);
	console.error(`- ${folder} ${exists ? '✓' : '✗ (not found)'}`);
	if (!exists) {
		console.error(`Warning: Folder "${folder}" does not exist or is not accessible`);
	}
}

// Create configuration object
export const config = configSchema.parse({
	anthropicApiKey: process.env.ANTHROPIC_API_KEY,
	openaiApiKey: process.env.OPENAI_API_KEY,
	googleApiKey: process.env.GOOGLE_API_KEY,
	anthropicApiUrl: process.env.ANTHROPIC_API_URL,
	openaiApiUrl: process.env.OPENAI_API_URL,
	googleApiUrl: process.env.GOOGLE_API_URL,
	forceProvider: process.env.FORCE_PROVIDER,
	modelName: process.env.MODEL_NAME,
	maxTokens: process.env.MAX_TOKENS ? parseInt(process.env.MAX_TOKENS) : undefined,
	maxHistoryMessages: process.env.MAX_HISTORY_MESSAGES ? parseInt(process.env.MAX_HISTORY_MESSAGES) : undefined,
	allowedFolders,
	debug: process.env.DEBUG === 'true' || process.env.DEBUG === '1'
});

// Validate that at least one API key is provided
if (!config.anthropicApiKey && !config.openaiApiKey && !config.googleApiKey) {
	throw new Error('No API key provided. Please set ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY environment variable.');
}

// Validate forced provider has matching API key
if (config.forceProvider) {
	if (config.forceProvider === 'anthropic' && !config.anthropicApiKey) {
		throw new Error('Forced provider "anthropic" selected but ANTHROPIC_API_KEY is not set.');
	}
	if (config.forceProvider === 'openai' && !config.openaiApiKey) {
		throw new Error('Forced provider "openai" selected but OPENAI_API_KEY is not set.');
	}
	if (config.forceProvider === 'google' && !config.googleApiKey) {
		throw new Error('Forced provider "google" selected but GOOGLE_API_KEY is not set.');
	}
}

export default config;