/**
 * Mock AI provider for testing purposes
 * This provider simulates AI responses without making actual API calls
 */

export function createMockProvider() {
  return {
    languageModel: (modelName) => ({
      modelId: `mock-${modelName}`,
      provider: 'mock',

      // Mock the doGenerate method used by Vercel AI SDK
      doGenerate: async ({ messages, tools }) => {
        // Simulate processing time
        await new Promise(resolve => setTimeout(resolve, 10));

        // Return a mock response
        return {
          text: 'This is a mock response for testing',
          toolCalls: [],
          usage: {
            promptTokens: 10,
            completionTokens: 5,
            totalTokens: 15
          }
        };
      },

      // Mock the doStream method for streaming responses
      doStream: async function* ({ messages, tools }) {
        // Simulate streaming response
        yield {
          type: 'text-delta',
          textDelta: 'Mock streaming response'
        };

        yield {
          type: 'finish',
          usage: {
            promptTokens: 10,
            completionTokens: 5,
            totalTokens: 15
          }
        };
      }
    })
  };
}

export function createMockModel(modelName = 'mock-model') {
  const provider = createMockProvider();
  return provider.languageModel(modelName);
}