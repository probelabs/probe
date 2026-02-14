import { describe, test, expect } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('Gemini Built-in Tools', () => {
  describe('_initializeGeminiBuiltinTools', () => {
    test('should enable gemini_google_search and gemini_url_context when provider is google', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        provider: 'google',
      });

      // If GOOGLE_API_KEY is set, provider will be google and tools should be enabled
      // If not, apiType may be 'uninitialized' and tools should be disabled
      if (agent.apiType === 'google') {
        expect(agent._geminiToolsEnabled.googleSearch).toBe(true);
        expect(agent._geminiToolsEnabled.urlContext).toBe(true);
      } else {
        // No Google API key available, tools should be disabled
        expect(agent._geminiToolsEnabled.googleSearch).toBe(false);
        expect(agent._geminiToolsEnabled.urlContext).toBe(false);
      }
    });

    test('should disable gemini tools when provider is not google', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        provider: 'anthropic',
      });

      // Regardless of API key availability, non-google providers should not have gemini tools
      if (agent.apiType !== 'google') {
        expect(agent._geminiToolsEnabled.googleSearch).toBe(false);
        expect(agent._geminiToolsEnabled.urlContext).toBe(false);
      }
    });

    test('should respect allowedTools filtering for gemini tools', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        provider: 'google',
        allowedTools: ['search', 'gemini_google_search'],  // gemini_url_context not allowed
      });

      if (agent.apiType === 'google') {
        expect(agent._geminiToolsEnabled.googleSearch).toBe(true);
        expect(agent._geminiToolsEnabled.urlContext).toBe(false);
      }
    });

    test('should disable gemini tools when all tools are disabled', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        provider: 'google',
        disableTools: true,
      });

      expect(agent._geminiToolsEnabled.googleSearch).toBe(false);
      expect(agent._geminiToolsEnabled.urlContext).toBe(false);
    });

    test('should disable gemini tools via exclusion pattern', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        provider: 'google',
        allowedTools: ['*', '!gemini_google_search'],
      });

      if (agent.apiType === 'google') {
        expect(agent._geminiToolsEnabled.googleSearch).toBe(false);
        expect(agent._geminiToolsEnabled.urlContext).toBe(true);
      }
    });
  });

  describe('_buildGeminiProviderTools', () => {
    test('should return undefined when provider is not google', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        provider: 'anthropic',
      });

      expect(agent._buildGeminiProviderTools()).toBeUndefined();
    });

    test('should return undefined when no gemini tools are enabled', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        provider: 'google',
        disableTools: true,
      });

      expect(agent._buildGeminiProviderTools()).toBeUndefined();
    });

    test('should return tools object when google provider has tools enabled', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        provider: 'google',
      });

      if (agent.apiType === 'google' && agent.provider?.tools) {
        const tools = agent._buildGeminiProviderTools();
        expect(tools).toBeDefined();
        // Keys in streamText tools use the SDK names (google_search, url_context)
        expect(tools.google_search).toBeDefined();
        expect(tools.url_context).toBeDefined();
      }
    });
  });

  describe('system message integration', () => {
    test('should include gemini tool definitions in system message when enabled', async () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        provider: 'google',
      });

      if (agent.apiType === 'google' && agent._geminiToolsEnabled.googleSearch) {
        const systemMessage = await agent.getSystemMessage();
        expect(systemMessage).toContain('gemini_google_search');
        expect(systemMessage).toContain('Gemini Built-in');
        expect(systemMessage).toContain('Web search powered by Google');
      }
    });

    test('should not include gemini tools in system message for non-google providers', async () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        provider: 'anthropic',
      });

      if (agent.apiType !== 'google') {
        const systemMessage = await agent.getSystemMessage();
        expect(systemMessage).not.toContain('Gemini Built-in');
      }
    });

    test('should include gemini tools in available tools list when enabled', async () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        provider: 'google',
      });

      if (agent.apiType === 'google' && agent._geminiToolsEnabled.googleSearch) {
        const systemMessage = await agent.getSystemMessage();
        expect(systemMessage).toContain('gemini_google_search: (auto)');
        expect(systemMessage).toContain('gemini_url_context: (auto)');
      }
    });
  });
});
