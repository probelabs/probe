/**
 * Enhanced Vercel AI SDK Engine with proper tool and prompt support
 */

import { streamText } from 'ai';

/**
 * Create an enhanced Vercel AI SDK engine with full tool support
 * @param {Object} agent - The ProbeAgent instance
 * @returns {Object} Engine interface
 */
export function createEnhancedVercelEngine(agent) {
  return {
    /**
     * Query the model using existing Vercel AI SDK implementation
     * @param {string} prompt - The prompt to send
     * @param {Object} options - Additional options
     * @returns {AsyncIterable} Response stream
     */
    async *query(prompt, options = {}) {
      // Get the system message with tools embedded (existing behavior)
      const systemMessage = await agent.getSystemMessage();

      // Build messages array with system prompt
      const messages = [
        { role: 'system', content: systemMessage },
        ...agent.history,
        { role: 'user', content: prompt }
      ];

      // Use existing streamText with retry and fallback
      const result = await agent.streamTextWithRetryAndFallback({
        model: agent.provider(agent.model),
        messages,
        maxTokens: options.maxTokens || agent.maxResponseTokens,
        temperature: options.temperature || 0.3,
        // Note: Vercel AI SDK doesn't use structured tools for XML format
        // The tools are embedded in the system prompt
        experimental_telemetry: options.telemetry
      });

      // Stream the response
      let fullContent = '';
      for await (const chunk of result.textStream) {
        fullContent += chunk;
        yield { type: 'text', content: chunk };
      }

      // Parse XML tool calls from the response if any
      // This maintains compatibility with existing XML tool format
      const toolCalls = agent.parseXmlToolCalls ? agent.parseXmlToolCalls(fullContent) : null;
      if (toolCalls && toolCalls.length > 0) {
        yield { type: 'tool_calls', toolCalls };
      }

      // Handle finish reason
      if (result.finishReason) {
        yield { type: 'finish', reason: result.finishReason };
      }
    },

    /**
     * Get available tools for this engine
     */
    getTools() {
      return agent.toolImplementations || {};
    },

    /**
     * Get system prompt for this engine
     */
    async getSystemPrompt() {
      return agent.getSystemMessage();
    },

    /**
     * Optional cleanup
     */
    async close() {
      // Nothing to cleanup for Vercel AI
    }
  };
}