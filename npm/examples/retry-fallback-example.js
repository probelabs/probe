/**
 * Retry and Fallback Example
 *
 * This example demonstrates the retry and fallback capabilities of ProbeAgent.
 * Run with: node examples/retry-fallback-example.js
 */

import { ProbeAgent } from '../src/agent/index.js';

console.log('=== ProbeAgent Retry and Fallback Example ===\n');

/**
 * Example 1: Basic Retry Configuration
 */
async function example1BasicRetry() {
  console.log('Example 1: Basic Retry Configuration');
  console.log('-------------------------------------');

  const agent = new ProbeAgent({
    path: process.cwd(),
    provider: 'anthropic',
    debug: true,

    // Configure retry behavior
    retry: {
      maxRetries: 5,           // Retry up to 5 times
      initialDelay: 1000,      // Start with 1 second
      maxDelay: 30000,         // Max 30 seconds
      backoffFactor: 2         // Double each time
    }
  });

  console.log('✅ Agent configured with retry support');
  console.log('   - Max retries: 5');
  console.log('   - Initial delay: 1s');
  console.log('   - Backoff factor: 2x\n');

  // API calls will automatically retry on transient failures
  // (Uncomment to test with real API)
  // try {
  //   const result = await agent.answer('What files are in this directory?');
  //   console.log('Result:', result);
  // } catch (error) {
  //   console.error('Error:', error.message);
  // }
}

/**
 * Example 2: Provider Fallback
 */
async function example2ProviderFallback() {
  console.log('\nExample 2: Provider Fallback Configuration');
  console.log('------------------------------------------');

  const agent = new ProbeAgent({
    path: process.cwd(),
    debug: true,

    // Configure multiple providers for fallback
    fallback: {
      strategy: 'custom',
      providers: [
        {
          provider: 'anthropic',
          apiKey: process.env.ANTHROPIC_API_KEY,
          model: 'claude-3-7-sonnet-20250219',
          maxRetries: 5
        },
        {
          provider: 'openai',
          apiKey: process.env.OPENAI_API_KEY,
          model: 'gpt-4o',
          maxRetries: 3
        },
        {
          provider: 'google',
          apiKey: process.env.GOOGLE_API_KEY,
          model: 'gemini-2.0-flash-exp',
          maxRetries: 3
        }
      ],
      maxTotalAttempts: 15
    }
  });

  console.log('✅ Agent configured with 3 fallback providers:');
  console.log('   1. Anthropic Claude 3.7 (5 retries)');
  console.log('   2. OpenAI GPT-4o (3 retries)');
  console.log('   3. Google Gemini 2.0 (3 retries)');
  console.log('   - Max total attempts: 15\n');

  // If Anthropic fails, automatically falls back to OpenAI, then Google
  // (Uncomment to test with real API)
  // try {
  //   const result = await agent.answer('Search for all TypeScript files');
  //   console.log('Result:', result);
  //
  //   // Check which provider succeeded
  //   if (agent.fallbackManager) {
  //     const stats = agent.fallbackManager.getStats();
  //     console.log('\n📊 Fallback Statistics:');
  //     console.log('   Successful provider:', stats.successfulProvider);
  //     console.log('   Total attempts:', stats.totalAttempts);
  //     console.log('   Failed providers:', stats.failedProviders.map(f => f.provider));
  //   }
  // } catch (error) {
  //   console.error('Error:', error.message);
  // }
}

/**
 * Example 3: Azure Claude → Bedrock Claude Fallback
 */
async function example3CrossCloudFallback() {
  console.log('\nExample 3: Cross-Cloud Fallback (Azure → Bedrock)');
  console.log('--------------------------------------------------');

  const agent = new ProbeAgent({
    path: process.cwd(),
    debug: true,

    fallback: {
      strategy: 'custom',
      providers: [
        {
          provider: 'anthropic',
          apiKey: process.env.AZURE_CLAUDE_API_KEY,
          baseURL: process.env.AZURE_CLAUDE_ENDPOINT,
          model: 'claude-3-7-sonnet-20250219',
          maxRetries: 5
        },
        {
          provider: 'bedrock',
          region: 'us-west-2',
          accessKeyId: process.env.AWS_ACCESS_KEY_ID,
          secretAccessKey: process.env.AWS_SECRET_ACCESS_KEY,
          model: 'anthropic.claude-sonnet-4-20250514-v1:0',
          maxRetries: 3
        }
      ]
    }
  });

  console.log('✅ Agent configured for cross-cloud fallback:');
  console.log('   1. Azure Claude (primary)');
  console.log('   2. AWS Bedrock Claude (fallback)\n');

  console.log('💡 Use case: Regional redundancy and cloud provider failover\n');
}

/**
 * Example 4: Auto-Fallback (Use All Available Providers)
 */
async function example4AutoFallback() {
  console.log('\nExample 4: Auto-Fallback');
  console.log('------------------------');

  const agent = new ProbeAgent({
    path: process.cwd(),
    provider: 'anthropic',  // Primary provider
    debug: true,

    fallback: {
      auto: true  // Automatically use all providers from environment
    }
  });

  console.log('✅ Agent configured with auto-fallback');
  console.log('   - Primary: Anthropic (from provider option)');
  console.log('   - Fallback: All other providers with API keys set\n');

  console.log('💡 Set these environment variables to enable auto-fallback:');
  console.log('   ANTHROPIC_API_KEY=xxx');
  console.log('   OPENAI_API_KEY=xxx');
  console.log('   GOOGLE_API_KEY=xxx');
  console.log('   AUTO_FALLBACK=1\n');
}

/**
 * Example 5: Environment Variable Configuration
 */
async function example5EnvVarConfig() {
  console.log('\nExample 5: Environment Variable Configuration');
  console.log('---------------------------------------------');

  console.log('Set these environment variables:\n');

  console.log('# Retry configuration');
  console.log('export MAX_RETRIES=5');
  console.log('export RETRY_INITIAL_DELAY=1000');
  console.log('export RETRY_MAX_DELAY=30000');
  console.log('export RETRY_BACKOFF_FACTOR=2\n');

  console.log('# Fallback providers (JSON)');
  console.log('export FALLBACK_PROVIDERS=\'[');
  console.log('  {');
  console.log('    "provider": "anthropic",');
  console.log('    "apiKey": "sk-ant-xxx",');
  console.log('    "model": "claude-3-7-sonnet-20250219"');
  console.log('  },');
  console.log('  {');
  console.log('    "provider": "openai",');
  console.log('    "apiKey": "sk-xxx",');
  console.log('    "model": "gpt-4o"');
  console.log('  }');
  console.log(']\'');
  console.log('export FALLBACK_MAX_TOTAL_ATTEMPTS=15\n');

  console.log('Then create agent with no config:');
  console.log('const agent = new ProbeAgent({ path: process.cwd() });\n');

  console.log('The retry and fallback will be auto-configured!\n');
}

/**
 * Example 6: Custom Retryable Errors
 */
async function example6CustomRetryableErrors() {
  console.log('\nExample 6: Custom Retryable Errors');
  console.log('-----------------------------------');

  const agent = new ProbeAgent({
    path: process.cwd(),
    debug: true,

    retry: {
      maxRetries: 5,
      // Only retry on these specific errors
      retryableErrors: [
        'Overloaded',
        'rate_limit',
        '429',
        '503',
        'CustomProviderError'  // Your custom error
      ]
    }
  });

  console.log('✅ Agent configured with custom retryable errors');
  console.log('   - Will retry on: Overloaded, rate_limit, 429, 503, CustomProviderError');
  console.log('   - Will NOT retry on: Invalid API key, authentication errors, etc.\n');
}

/**
 * Example 7: Statistics and Monitoring
 */
async function example7Statistics() {
  console.log('\nExample 7: Statistics and Monitoring');
  console.log('------------------------------------');

  const agent = new ProbeAgent({
    path: process.cwd(),
    debug: true,  // Enable debug logs

    retry: { maxRetries: 3 },
    fallback: {
      strategy: 'custom',
      providers: [
        { provider: 'anthropic', apiKey: process.env.ANTHROPIC_API_KEY },
        { provider: 'openai', apiKey: process.env.OPENAI_API_KEY }
      ]
    }
  });

  console.log('✅ Debug mode enabled - you will see detailed logs:\n');
  console.log('[RetryManager] Retry attempt 1/3 { provider: "anthropic", model: "claude-3-7-..." }');
  console.log('[RetryManager] Waiting 1000ms before retry...');
  console.log('[FallbackManager] Attempting provider: anthropic/claude-3-7-... (attempt 1/10)');
  console.log('[FallbackManager] ❌ Failed with provider: anthropic/claude-3-7-...');
  console.log('[FallbackManager] Trying next provider (1 remaining)...');
  console.log('[FallbackManager] ✅ Success with provider: openai/gpt-4o\n');

  console.log('Access statistics programmatically:');
  console.log(`
if (agent.retryManager) {
  const retryStats = agent.retryManager.getStats();
  console.log('Retry Stats:', retryStats);
}

if (agent.fallbackManager) {
  const fallbackStats = agent.fallbackManager.getStats();
  console.log('Fallback Stats:', fallbackStats);
}
  `.trim());
  console.log();
}

// Run all examples
async function main() {
  await example1BasicRetry();
  await example2ProviderFallback();
  await example3CrossCloudFallback();
  await example4AutoFallback();
  await example5EnvVarConfig();
  await example6CustomRetryableErrors();
  await example7Statistics();

  console.log('\n' + '='.repeat(60));
  console.log('For full documentation, see:');
  console.log('  npm/docs/RETRY_AND_FALLBACK.md');
  console.log('='.repeat(60) + '\n');
}

main().catch(console.error);
