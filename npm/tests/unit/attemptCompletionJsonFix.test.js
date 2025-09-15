import { describe, test, expect, jest, beforeEach, afterEach } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('attempt_completion JSON schema fix', () => {
  let mockStreamText;
  let mockCreateAnthropic;
  let originalEnv;

  beforeEach(() => {
    // Save original environment
    originalEnv = process.env.NODE_ENV;
    process.env.NODE_ENV = 'test';

    // Mock the AI SDK
    mockStreamText = jest.fn();
    mockCreateAnthropic = jest.fn();

    // Mock the modules
    jest.unstable_mockModule('@ai-sdk/anthropic', () => ({
      createAnthropic: mockCreateAnthropic
    }));

    jest.unstable_mockModule('ai', () => ({
      streamText: mockStreamText
    }));
  });

  afterEach(() => {
    // Restore environment
    process.env.NODE_ENV = originalEnv;
    jest.clearAllMocks();
  });

  test('should apply JSON correction when attempt_completion returns markdown', async () => {
    // Setup mock responses
    let callCount = 0;
    mockStreamText.mockImplementation(async () => {
      callCount++;
      
      if (callCount === 1) {
        // First call - agent uses attempt_completion tool with markdown response
        return {
          text: 'I will help you count the words.',
          toolCalls: [
            {
              type: 'tool-call',
              toolCallId: 'call_1',
              toolName: 'attempt_completion',
              args: {
                result: 'Based on the text "Hello world test", I can count that there are 3 words.\n\nHere is the analysis:\n- "Hello": 1 word\n- "world": 1 word  \n- "test": 1 word\n\nTotal: 3 words'
              }
            }
          ],
          toolResults: [
            {
              type: 'tool-result',
              toolCallId: 'call_1',
              result: 'Task completed successfully'
            }
          ]
        };
      } else if (callCount === 2) {
        // Second call - JSON correction prompt returns valid JSON
        return {
          text: '{"message": "There are 3 words in the text Hello world test", "count": 3}',
          toolCalls: [],
          toolResults: []
        };
      }
      
      throw new Error('Unexpected call count: ' + callCount);
    });

    mockCreateAnthropic.mockReturnValue({
      // Mock Anthropic client
    });

    // Create agent instance
    const agent = new ProbeAgent({
      sessionId: 'test-session',
      debug: true,
      provider: 'anthropic'
    });

    // Define JSON schema
    const jsonSchema = `{
  "type": "object",
  "properties": {
    "message": {
      "type": "string",
      "description": "A descriptive message about the word count"
    },
    "count": {
      "type": "number",
      "description": "The actual number of words counted"
    }
  },
  "required": ["message", "count"]
}`;

    // Test the agent with schema
    const result = await agent.answer(
      'Count words in "Hello world test" and return JSON with message and count fields.',
      [],
      { schema: jsonSchema }
    );

    // Verify the result
    expect(result).toBeTruthy();
    
    // Should be valid JSON
    expect(() => JSON.parse(result)).not.toThrow();
    
    // Parse and verify structure
    const parsed = JSON.parse(result);
    expect(parsed).toHaveProperty('message');
    expect(parsed).toHaveProperty('count');
    expect(typeof parsed.message).toBe('string');
    expect(typeof parsed.count).toBe('number');
    expect(parsed.count).toBe(3);

    // Verify the correction flow was triggered
    expect(mockStreamText).toHaveBeenCalledTimes(2);
    
    // Verify the second call contains correction prompt
    const secondCallArgs = mockStreamText.mock.calls[1][0];
    expect(secondCallArgs.messages[0].content).toMatch(/CRITICAL JSON ERROR|URGENT|FINAL ATTEMPT/);
  });

  test('should handle schema definition confusion in attempt_completion', async () => {
    let callCount = 0;
    mockStreamText.mockImplementation(async () => {
      callCount++;
      
      if (callCount === 1) {
        // First call - agent returns schema definition instead of data via attempt_completion
        return {
          text: 'I will provide the JSON schema structure.',
          toolCalls: [
            {
              type: 'tool-call',
              toolCallId: 'call_1',
              toolName: 'attempt_completion',
              args: {
                result: '```json\n{\n  "$schema": "http://json-schema.org/draft-07/schema#",\n  "type": "object",\n  "properties": {\n    "message": {"type": "string"},\n    "count": {"type": "number"}\n  },\n  "required": ["message", "count"]\n}\n```'
              }
            }
          ],
          toolResults: [
            {
              type: 'tool-result',
              toolCallId: 'call_1',
              result: 'Task completed successfully'
            }
          ]
        };
      } else if (callCount === 2) {
        // Second call - schema definition correction returns actual data
        return {
          text: '{"message": "Word count for Hello world test is 3", "count": 3}',
          toolCalls: [],
          toolResults: []
        };
      }
      
      throw new Error('Unexpected call count: ' + callCount);
    });

    mockCreateAnthropic.mockReturnValue({});

    const agent = new ProbeAgent({
      sessionId: 'test-schema-confusion',
      debug: true,
      provider: 'anthropic'
    });

    const jsonSchema = `{
  "type": "object",
  "properties": {
    "message": {"type": "string"},
    "count": {"type": "number"}
  },
  "required": ["message", "count"]
}`;

    const result = await agent.answer(
      'Count words in "Hello world test".',
      [],
      { schema: jsonSchema }
    );

    // Should be valid JSON data, not schema definition
    expect(() => JSON.parse(result)).not.toThrow();
    
    const parsed = JSON.parse(result);
    expect(parsed).toHaveProperty('message');
    expect(parsed).toHaveProperty('count');
    
    // Should NOT have schema definition properties
    expect(parsed).not.toHaveProperty('$schema');
    expect(parsed).not.toHaveProperty('type');
    expect(parsed).not.toHaveProperty('properties');

    // Verify correction was triggered
    expect(mockStreamText).toHaveBeenCalledTimes(2);
    
    // Verify schema definition correction prompt was used
    const secondCallArgs = mockStreamText.mock.calls[1][0];
    expect(secondCallArgs.messages[0].content).toMatch(/CRITICAL MISUNDERSTANDING|URGENT - WRONG RESPONSE TYPE|FINAL ATTEMPT - SCHEMA VS DATA CONFUSION/);
  });
});