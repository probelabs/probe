import { streamText as originalStreamText } from 'ai';
import { z } from 'zod';

// Mock LLM Provider for testing
export class MockLLMProvider {
    constructor(options = {}) {
        this.responses = options.responses || [];
        this.currentResponseIndex = 0;
        this.failAfter = options.failAfter || Infinity;
        this.callCount = 0;
        this.capturedCalls = [];
        this.streamDelay = options.streamDelay || 10; // ms between chunks
        this.throwError = options.throwError || null;
    }

    // Reset state between tests
    reset() {
        this.currentResponseIndex = 0;
        this.callCount = 0;
        this.capturedCalls = [];
    }

    // Add a response to the queue
    addResponse(response) {
        this.responses.push(response);
    }

    // Get the next response
    getNextResponse() {
        if (this.callCount >= this.failAfter) {
            throw new Error('Mock provider configured to fail after ' + this.failAfter + ' calls');
        }

        if (this.throwError) {
            throw new Error(this.throwError);
        }

        const response = this.responses[this.currentResponseIndex];
        this.currentResponseIndex = (this.currentResponseIndex + 1) % this.responses.length;
        return response;
    }

    // Create a mock model that can be used with the AI SDK
    createMockModel() {
        const provider = this;
        
        return {
            doStream: async function*(params) {
                provider.callCount++;
                // Deep copy params to avoid reference issues
                provider.capturedCalls.push({
                    ...params,
                    messages: params.messages ? JSON.parse(JSON.stringify(params.messages)) : []
                });

                const response = provider.getNextResponse();
                
                // Simulate streaming with chunks
                if (response.text) {
                    // Split into words for more realistic streaming
                    const words = response.text.split(' ');
                    const chunkSize = Math.max(1, Math.floor(words.length / 5)); // At least 5 chunks
                    
                    for (let i = 0; i < words.length; i += chunkSize) {
                        const chunk = words.slice(i, i + chunkSize).join(' ');
                        if (i + chunkSize < words.length) {
                            // Add space after chunk if not last
                            await new Promise(resolve => setTimeout(resolve, provider.streamDelay));
                            yield {
                                type: 'text-delta',
                                textDelta: chunk + ' '
                            };
                        } else {
                            // Last chunk, no trailing space
                            await new Promise(resolve => setTimeout(resolve, provider.streamDelay));
                            yield {
                                type: 'text-delta',
                                textDelta: chunk
                            };
                        }
                    }
                }

                // Handle tool calls
                if (response.toolCalls) {
                    for (const toolCall of response.toolCalls) {
                        yield {
                            type: 'tool-call',
                            toolCallId: toolCall.toolCallId || 'mock-tool-call-' + Date.now(),
                            toolName: toolCall.toolName,
                            args: toolCall.args
                        };
                    }
                }

                // Send finish reason
                yield {
                    type: 'finish',
                    finishReason: response.finishReason || 'stop',
                    usage: {
                        promptTokens: response.promptTokens || 100,
                        completionTokens: response.completionTokens || 50
                    }
                };
            },

            // Support for generateText (non-streaming)
            doGenerate: async function(params) {
                provider.callCount++;
                // Deep copy params to avoid reference issues
                provider.capturedCalls.push({
                    ...params, 
                    messages: params.messages ? JSON.parse(JSON.stringify(params.messages)) : []
                });

                const response = provider.getNextResponse();
                
                return {
                    text: response.text || '',
                    toolCalls: response.toolCalls || [],
                    finishReason: response.finishReason || 'stop',
                    usage: {
                        promptTokens: response.promptTokens || 100,
                        completionTokens: response.completionTokens || 50
                    }
                };
            }
        };
    }
}

// Mock the AI SDK's streamText function
export function createMockStreamText(provider) {
    return async function mockStreamText(options) {
        const { model, messages, tools, toolChoice, maxTokens, temperature, system } = options;
        
        // Create a mock stream similar to the AI SDK
        const mockModel = provider.createMockModel();
        
        // Call doStream once and collect all chunks
        const params = { messages, tools, toolChoice, maxTokens, temperature, system };
        const chunks = [];
        for await (const chunk of mockModel.doStream(params)) {
            chunks.push(chunk);
        }

        // Create mock helper functions that replay the collected chunks
        const textStream = (async function*() {
            for (const chunk of chunks) {
                if (chunk.type === 'text-delta') {
                    yield chunk.textDelta;
                }
            }
        })();

        const fullStream = (async function*() {
            for (const chunk of chunks) {
                yield chunk;
            }
        })();

        return {
            textStream,
            fullStream,
            toAIStreamResponse: () => {
                // Mock response for testing
                return new Response('mock stream response');
            }
        };
    };
}

// Predefined response scenarios for common test cases
export const mockResponses = {
    // Simple text response
    simpleText: {
        text: "This is a simple text response from the mock LLM."
    },

    // Response with tool call
    withToolCall: {
        text: "Let me search for that information.",
        toolCalls: [{
            toolName: 'probe_search',
            args: {
                query: 'test query',
                path: './src'
            }
        }]
    },

    // Multiple tool calls
    multipleToolCalls: {
        text: "I'll help you with multiple operations.",
        toolCalls: [
            {
                toolName: 'probe_search',
                args: {
                    query: 'function definition',
                    path: './src'
                }
            },
            {
                toolName: 'probe_extract',
                args: {
                    location: 'src/main.rs:42'
                }
            }
        ]
    },

    // Error response
    errorResponse: {
        text: "I encountered an error processing your request.",
        finishReason: 'error'
    },

    // Long streaming response
    longStreamingResponse: {
        text: "This is a longer response that will be streamed in chunks. " +
              "It simulates how a real LLM would stream content back to the user. " +
              "Each chunk arrives with a small delay to mimic network latency. " +
              "This helps test the streaming functionality of the chat system."
    }
};

// Helper to create a mock provider with predefined responses
export function createMockProvider(scenario = 'simple', options = {}) {
    const responses = [];
    
    switch (scenario) {
        case 'simple':
            responses.push(mockResponses.simpleText);
            break;
        case 'tools':
            responses.push(mockResponses.withToolCall);
            responses.push(mockResponses.simpleText);
            break;
        case 'error':
            return new MockLLMProvider({ ...options, throwError: 'Simulated API error' });
        case 'mixed':
            responses.push(mockResponses.simpleText);
            responses.push(mockResponses.withToolCall);
            responses.push(mockResponses.multipleToolCalls);
            break;
        default:
            responses.push(mockResponses.simpleText);
    }
    
    return new MockLLMProvider({ ...options, responses });
}