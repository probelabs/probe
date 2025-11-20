/**
 * Vercel AI SDK Engine - wraps existing ProbeAgent logic
 * This maintains full backward compatibility
 */

import { streamText } from 'ai';

/**
 * Create a Vercel AI SDK engine
 * @param {Object} agent - The ProbeAgent instance
 * @returns {Object} Engine interface
 */
export function createVercelEngine(agent) {
  return {
    /**
     * Query the model using existing Vercel AI SDK implementation
     * @param {string} prompt - The prompt to send
     * @param {Object} options - Additional options
     * @returns {AsyncIterable} Response stream
     */
    async *query(prompt, options = {}) {
      // Build messages array
      const messages = [
        ...agent.history,
        { role: 'user', content: prompt }
      ];

      // Use existing streamText with retry and fallback
      const result = await agent.streamTextWithRetryAndFallback({
        model: agent.provider(agent.model),
        messages,
        maxTokens: options.maxTokens || agent.maxResponseTokens,
        temperature: options.temperature,
        tools: options.tools,
        toolChoice: options.toolChoice,
        experimental_telemetry: options.telemetry
      });

      // Stream the response
      for await (const chunk of result.textStream) {
        yield { type: 'text', content: chunk };
      }

      // Handle tool calls if any
      if (result.toolCalls && result.toolCalls.length > 0) {
        yield { type: 'tool_calls', toolCalls: result.toolCalls };
      }

      // Handle finish reason
      if (result.finishReason) {
        yield { type: 'finish', reason: result.finishReason };
      }
    },

    /**
     * Optional cleanup
     */
    async close() {
      // Nothing to cleanup for Vercel AI
    }
  };
}