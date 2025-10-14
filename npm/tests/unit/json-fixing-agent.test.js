/**
 * Test JsonFixingAgent with separate session isolation
 */

import { describe, test, expect } from '@jest/globals';
import { JsonFixingAgent, validateJsonResponse, cleanSchemaResponse } from '../../src/agent/schemaUtils.js';

describe('JsonFixingAgent', () => {
  describe('constructor', () => {
    test('should create JsonFixingAgent with default options', () => {
      const agent = new JsonFixingAgent();

      expect(agent.options.sessionId).toMatch(/^json-fixer-\d+-\d+$/);
      expect(agent.options.allowEdit).toBe(false);
      expect(agent.ProbeAgent).toBeNull();
      expect(agent.agent).toBeUndefined();
    });

    test('should create JsonFixingAgent with custom options', () => {
      const options = {
        sessionId: 'custom-session-123',
        path: '/custom/path',
        provider: 'anthropic',
        model: 'claude-3-5-sonnet-20241022',
        debug: true
      };

      const agent = new JsonFixingAgent(options);

      expect(agent.options.sessionId).toBe('custom-session-123');
      expect(agent.options.path).toBe('/custom/path');
      expect(agent.options.provider).toBe('anthropic');
      expect(agent.options.model).toBe('claude-3-5-sonnet-20241022');
      expect(agent.options.debug).toBe(true);
      expect(agent.options.allowEdit).toBe(false); // Always false for safety
    });
  });

  describe('getJsonFixingPrompt', () => {
    test('should return specialized JSON fixing prompt', () => {
      const agent = new JsonFixingAgent();
      const prompt = agent.getJsonFixingPrompt();

      // Check for key elements of the prompt
      expect(prompt).toContain('JSON syntax correction specialist');
      expect(prompt).toContain('CORE RESPONSIBILITIES');
      expect(prompt).toContain('JSON SYNTAX RULES');
      expect(prompt).toContain('COMMON ERRORS TO FIX');
      expect(prompt).toContain('FIXING METHODOLOGY');
      expect(prompt).toContain('CRITICAL RULES');

      // Check for specific syntax rules
      expect(prompt).toContain('Property names');
      expect(prompt).toContain('double quotes');
      expect(prompt).toContain('No trailing commas');

      // Check for common errors
      expect(prompt).toContain('Unquoted property names');
      expect(prompt).toContain('Single quotes');
      expect(prompt).toContain('Trailing commas');

      // Check for critical rules
      expect(prompt).toContain('NEVER add explanations');
      expect(prompt).toContain('NEVER wrap in markdown');
      expect(prompt).toContain('PRESERVE the original data structure');
    });
  });

  describe('initializeAgent', () => {
    test('should initialize ProbeAgent lazily', async () => {
      const agent = new JsonFixingAgent({
        sessionId: 'test-session',
        debug: false
      });

      expect(agent.ProbeAgent).toBeNull();
      expect(agent.agent).toBeUndefined();

      // Initialize the agent
      await agent.initializeAgent();

      // Check that ProbeAgent was imported and agent was created
      expect(agent.ProbeAgent).not.toBeNull();
      expect(agent.agent).toBeDefined();
      expect(agent.agent.sessionId).toBe('test-session');
      expect(agent.agent.disableJsonValidation).toBe(true); // CRITICAL for preventing recursion
    });

    test('should not reinitialize agent if already initialized', async () => {
      const agent = new JsonFixingAgent();

      await agent.initializeAgent();
      const firstAgent = agent.agent;

      await agent.initializeAgent();
      const secondAgent = agent.agent;

      // Should be the same instance
      expect(secondAgent).toBe(firstAgent);
    });

    test('should create agent with disableJsonValidation=true', async () => {
      const agent = new JsonFixingAgent({
        debug: false
      });

      await agent.initializeAgent();

      // This is CRITICAL to prevent infinite recursion
      expect(agent.agent.disableJsonValidation).toBe(true);
    });

    test('should create agent with allowEdit=false', async () => {
      const agent = new JsonFixingAgent();

      await agent.initializeAgent();

      // Agent should not be able to edit files (only fix JSON syntax)
      expect(agent.agent.allowEdit).toBe(false);
    });

    test('should create agent with maxIterations=5', async () => {
      const agent = new JsonFixingAgent();

      await agent.initializeAgent();

      expect(agent.agent.maxIterations).toBe(5);
    });
  });

  describe('session isolation', () => {
    test('should use unique session ID per instance', async () => {
      const agent1 = new JsonFixingAgent();
      // Small delay to ensure different timestamp
      await new Promise(resolve => setTimeout(resolve, 1));
      const agent2 = new JsonFixingAgent();

      expect(agent1.options.sessionId).not.toBe(agent2.options.sessionId);
    });

    test('should create separate session from parent agent', async () => {
      const agent = new JsonFixingAgent({
        sessionId: 'json-fixer-test-123'
      });

      await agent.initializeAgent();

      // Session ID should be the custom one
      expect(agent.agent.sessionId).toBe('json-fixer-test-123');

      // Agent should be completely isolated (no shared history with parent)
      expect(agent.agent.history).toEqual([]);
    });
  });

  describe('cancel and cleanup', () => {
    test('should have cancel method', () => {
      const agent = new JsonFixingAgent();

      expect(typeof agent.cancel).toBe('function');

      // Should not throw when agent not initialized
      expect(() => agent.cancel()).not.toThrow();
    });

    test('should cancel initialized agent', async () => {
      const agent = new JsonFixingAgent();
      await agent.initializeAgent();

      expect(() => agent.cancel()).not.toThrow();
    });

    test('should have getTokenUsage method', () => {
      const agent = new JsonFixingAgent();

      expect(typeof agent.getTokenUsage).toBe('function');
      expect(agent.getTokenUsage()).toBeNull(); // No agent initialized
    });
  });

  describe('integration with validation flow', () => {
    test('should follow clean-then-validate workflow', () => {
      // Step 1: AI returns JSON with markdown
      const aiResponse = '```json\n{"name": "test", "id": 42}\n```';

      // Step 2: Clean the response
      const cleaned = cleanSchemaResponse(aiResponse);
      expect(cleaned).toBe('{"name": "test", "id": 42}');

      // Step 3: Validate
      const validation = validateJsonResponse(cleaned);
      expect(validation.isValid).toBe(true);

      // If invalid, JsonFixingAgent would be used (tested separately with mocks)
    });

    test('should provide enhanced error for fixing', () => {
      const invalidJson = '{"name": "test", "value": invalid}';

      const validation = validateJsonResponse(invalidJson);

      expect(validation.isValid).toBe(false);
      expect(validation.enhancedError).toBeDefined();
      expect(validation.errorContext).toBeDefined();

      // JsonFixingAgent.fixJson() would receive this enhanced error
      expect(validation.enhancedError).toContain('Error location');
      expect(validation.enhancedError).toContain('^ here');
    });
  });

  describe('architectural consistency with MermaidFixingAgent', () => {
    test('should follow same pattern as MermaidFixingAgent', async () => {
      const jsonAgent = new JsonFixingAgent({ sessionId: 'json-test' });
      await jsonAgent.initializeAgent();

      // Both should have:
      // 1. Separate session
      expect(jsonAgent.agent.sessionId).toBe('json-test');

      // 2. allowEdit=false
      expect(jsonAgent.agent.allowEdit).toBe(false);

      // 3. Specialized prompt
      expect(jsonAgent.agent.customPrompt).toBeDefined();
      expect(jsonAgent.agent.customPrompt).toContain('specialist');

      // 4. Recursion prevention flag
      expect(jsonAgent.agent.disableJsonValidation).toBe(true); // JSON equivalent of disableMermaidValidation

      // 5. Cancel method
      expect(typeof jsonAgent.cancel).toBe('function');

      // 6. Token usage method
      expect(typeof jsonAgent.getTokenUsage).toBe('function');
    });
  });
});
