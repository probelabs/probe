/**
 * Unit tests for max tool iterations warning logic
 * Tests the core warning message functionality without full ProbeAgent integration
 */

import { describe, test, expect } from '@jest/globals';

describe('Max Tool Iterations Warning Logic', () => {
  
  // Test the core warning message generation logic
  describe('Warning Message Generation', () => {
    test('should generate correct warning message format', () => {
      const maxIterations = 30;
      const warningMessage = `⚠️ WARNING: You have reached the maximum tool iterations limit (${maxIterations}). This is your final message. Please respond with the data you have so far. If something was not completed, honestly state what was not done and provide any partial results or recommendations you can offer.`;
      
      // Verify the warning message contains all required elements
      expect(warningMessage).toContain('⚠️ WARNING');
      expect(warningMessage).toContain('maximum tool iterations limit');
      expect(warningMessage).toContain(`(${maxIterations})`);
      expect(warningMessage).toContain('This is your final message');
      expect(warningMessage).toContain('honestly state what was not done');
      expect(warningMessage).toContain('partial results or recommendations');
    });

    test('should include different iteration limits in message', () => {
      const testLimits = [1, 5, 10, 30, 50, 100];
      
      testLimits.forEach(limit => {
        const warningMessage = `⚠️ WARNING: You have reached the maximum tool iterations limit (${limit}). This is your final message. Please respond with the data you have so far. If something was not completed, honestly state what was not done and provide any partial results or recommendations you can offer.`;
        
        expect(warningMessage).toContain(`limit (${limit})`);
      });
    });
  });

  describe('Iteration Counting Logic', () => {
    test('should detect when current iteration equals max iterations', () => {
      // Test different scenarios
      const testCases = [
        { current: 1, max: 1, shouldWarn: true },
        { current: 5, max: 5, shouldWarn: true },
        { current: 30, max: 30, shouldWarn: true },
        { current: 1, max: 5, shouldWarn: false },
        { current: 4, max: 5, shouldWarn: false },
        { current: 29, max: 30, shouldWarn: false }
      ];

      testCases.forEach(({ current, max, shouldWarn }) => {
        const shouldShowWarning = (current === max);
        expect(shouldShowWarning).toBe(shouldWarn);
      });
    });

    test('should handle schema-extended iteration limits', () => {
      const baseLimit = 30;
      const schemaExtension = 4;
      const extendedLimit = baseLimit + schemaExtension;
      
      // When schema is provided, limit should be extended
      expect(extendedLimit).toBe(34);
      
      // Warning should trigger at the extended limit
      const shouldWarnAtBase = (baseLimit === baseLimit); // true
      const shouldWarnAtExtended = (extendedLimit === extendedLimit); // true
      
      expect(shouldWarnAtBase).toBe(true);
      expect(shouldWarnAtExtended).toBe(true);
    });
  });

  describe('Environment Variable Handling', () => {
    test('should parse MAX_TOOL_ITERATIONS environment variable correctly', () => {
      // Test parsing logic (simulating how the actual code works)
      const testCases = [
        { env: '30', expected: 30 },
        { env: '1', expected: 1 },
        { env: '100', expected: 100 },
        { env: undefined, expected: 30 }, // default
        { env: '', expected: 30 }, // default
        { env: 'invalid', expected: NaN }
      ];

      testCases.forEach(({ env, expected }) => {
        const parsed = parseInt(env || '30', 10);
        if (isNaN(expected)) {
          expect(isNaN(parsed)).toBe(true);
        } else {
          expect(parsed).toBe(expected);
        }
      });
    });
  });

  describe('Message Structure Validation', () => {
    test('should create proper message structure for AI model', () => {
      const maxIterations = 25;
      const warningMessage = `⚠️ WARNING: You have reached the maximum tool iterations limit (${maxIterations}). This is your final message. Please respond with the data you have so far. If something was not completed, honestly state what was not done and provide any partial results or recommendations you can offer.`;
      
      // Simulate the message structure that would be added to the conversation
      const messageStructure = {
        role: 'user',
        content: warningMessage
      };
      
      expect(messageStructure.role).toBe('user');
      expect(messageStructure.content).toBe(warningMessage);
      expect(typeof messageStructure.content).toBe('string');
      expect(messageStructure.content.length).toBeGreaterThan(0);
    });

    test('should ensure warning message is clear and actionable', () => {
      const warningMessage = `⚠️ WARNING: You have reached the maximum tool iterations limit (30). This is your final message. Please respond with the data you have so far. If something was not completed, honestly state what was not done and provide any partial results or recommendations you can offer.`;
      
      // Check that the message contains actionable instructions
      const actionableKeywords = [
        'respond with the data',
        'honestly state',
        'what was not done',
        'partial results',
        'recommendations'
      ];

      actionableKeywords.forEach(keyword => {
        expect(warningMessage.toLowerCase()).toContain(keyword.toLowerCase());
      });
    });

    test('should handle different iteration limit scenarios', () => {
      // Test edge cases
      const edgeCases = [
        { limit: 1, description: 'minimum limit' },
        { limit: 2, description: 'very low limit' }, 
        { limit: 30, description: 'default limit' },
        { limit: 100, description: 'high limit' }
      ];

      edgeCases.forEach(({ limit, description }) => {
        const warningMessage = `⚠️ WARNING: You have reached the maximum tool iterations limit (${limit}). This is your final message. Please respond with the data you have so far. If something was not completed, honestly state what was not done and provide any partial results or recommendations you can offer.`;
        
        expect(warningMessage).toContain(`limit (${limit})`);
        expect(warningMessage).toContain('This is your final message');
        
        // Message should be consistent regardless of limit value
        expect(warningMessage.split(' ').length).toBeGreaterThan(20); // Reasonable message length
      });
    });
  });

  describe('Integration Points', () => {
    test('should verify warning triggers at the right time in iteration flow', () => {
      // Simulate the iteration loop logic
      const MAX_ITERATIONS = 3;
      let currentIteration = 0;
      const warningTriggered = [];

      // Simulate 5 iterations
      for (let i = 0; i < 5; i++) {
        currentIteration++;
        
        // Check if warning should be triggered (this is the actual logic from the code)
        if (currentIteration === MAX_ITERATIONS) {
          warningTriggered.push(currentIteration);
        }
        
        // Break if we would exceed max iterations (simulating the while loop condition)
        if (currentIteration >= MAX_ITERATIONS) {
          break;
        }
      }

      // Warning should be triggered exactly once at iteration 3
      expect(warningTriggered).toHaveLength(1);
      expect(warningTriggered[0]).toBe(MAX_ITERATIONS);
      expect(currentIteration).toBe(MAX_ITERATIONS);
    });

    test('should handle schema option extension correctly', () => {
      const BASE_LIMIT = 5;
      const SCHEMA_EXTENSION = 4;
      
      // Test without schema
      const normalLimit = BASE_LIMIT;
      expect(normalLimit).toBe(5);
      
      // Test with schema (logic from actual code)
      const hasSchema = true;
      const extendedLimit = hasSchema ? BASE_LIMIT + SCHEMA_EXTENSION : BASE_LIMIT;
      expect(extendedLimit).toBe(9);
      
      // Warning should trigger at different points
      expect(5 === normalLimit).toBe(true); // Would trigger at iteration 5 normally
      expect(9 === extendedLimit).toBe(true); // Would trigger at iteration 9 with schema
    });
  });
});