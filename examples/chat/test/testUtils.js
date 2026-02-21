import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { ProbeChat } from '../probeChat.js';
import { MockLLMProvider, createMockStreamText } from './mocks/mockLLMProvider.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Test environment setup
export function setupTestEnvironment() {
    // Store original environment
    const originalEnv = { ...process.env };
    
    // Clear API keys to ensure mock providers are used
    delete process.env.ANTHROPIC_API_KEY;
    delete process.env.OPENAI_API_KEY;
    delete process.env.GOOGLE_API_KEY;
    
    // Set test mode
    process.env.NODE_ENV = 'test';
    process.env.PROBE_TEST_MODE = 'true';
    
    return {
        restore: () => {
            Object.keys(process.env).forEach(key => {
                if (!(key in originalEnv)) {
                    delete process.env[key];
                }
            });
            Object.assign(process.env, originalEnv);
        }
    };
}

// Create a test instance of ProbeChat with mocks
export async function createTestProbeChat(options = {}) {
    const mockProvider = options.mockProvider instanceof MockLLMProvider ? 
        options.mockProvider : 
        new MockLLMProvider({
            responses: options.responses || [],
            ...(typeof options.mockProvider === 'object' ? options.mockProvider : {})
        });
    
    const mockStreamText = createMockStreamText(mockProvider);
    
    // Create instance with mocks injected
    const probeChat = new ProbeChat({
        modelName: 'mock-model',
        provider: 'mock',
        ...options.chatOptions
    });
    
    // Set temperature and maxTokens if provided
    if (options.chatOptions?.temperature !== undefined) {
        probeChat.temperature = options.chatOptions.temperature;
    }
    if (options.chatOptions?.maxTokens !== undefined) {
        probeChat.maxTokens = options.chatOptions.maxTokens;
    }
    if (options.chatOptions?.systemPrompt !== undefined) {
        probeChat.customPrompt = options.chatOptions.systemPrompt;
    }
    
    // Override the streamText function
    probeChat.streamText = mockStreamText;
    
    // Override model initialization to use our mock
    probeChat.initializeModel = function() {
        this.model = mockProvider.createMockModel();
        this.modelId = 'mock-model';
        this.modelInfo = {
            provider: 'mock',
            contextWindow: 100000,
            maxOutput: 4096,
            supportsImages: true,
            supportsPromptCaching: false
        };
        this.isInitialized = true;
        this.noApiKeysMode = false; // Disable API key mode for tests
        this.apiType = 'mock';
    };
    
    // Initialize the mock model
    probeChat.initializeModel();
    
    // Create mock tool instances
    const mockTools = {
        probe_search: {
            execute: async (args) => {
                console.log('Mock probe_search called with:', args);
                return createMockProbeResults();
            },
            validate: (args) => {
                if (!args.query) {
                    throw new Error('Missing required field: query');
                }
            }
        },
        probe_query: {
            execute: async (args) => {
                console.log('Mock probe_query called with:', args);
                return {
                    matches: [{
                        file: 'test.js',
                        matches: [{ text: 'mock match' }]
                    }]
                };
            }
        },
        probe_extract: {
            execute: async (args) => {
                console.log('Mock probe_extract called with:', args);
                return {
                    content: 'mock extracted content',
                    language: 'javascript',
                    start_line: 1,
                    end_line: 10
                };
            }
        },
    };
    
    // Make tools accessible for test overrides
    probeChat.tools = mockTools;
    
    // Store mock provider reference for test access
    probeChat.mockProvider = mockProvider;
    
    // Add missing ProbeChat methods for tests
    probeChat.messages = [];
    probeChat.history = probeChat.messages; // Alias for compatibility with real implementation
    probeChat.sendMessage = async function(message) {
        return await this.chat(message);
    };
    
    probeChat.chat = async function(message, options = {}) {
        let userMessage;
        if (typeof message === 'string') {
            userMessage = { role: 'user', content: message };
        } else if (message.role && message.content) {
            userMessage = message;
        } else {
            userMessage = { role: 'user', content: message.content || message };
        }
        this.messages.push(userMessage);
        
        // Call the mock provider with streaming support
        const response = await mockStreamText({
            model: this.model,
            messages: this.messages,
            system: this.customPrompt || 'You are a helpful assistant.',
            temperature: this.temperature,
            maxTokens: this.maxTokens
        });
        
        let fullResponse = '';
        
        // Stream chunks if callback provided
        if (options.onStream) {
            for await (const chunk of response.textStream) {
                options.onStream(chunk);
                fullResponse += chunk;
            }
        } else {
            // Otherwise just collect the full response
            for await (const chunk of response.textStream) {
                fullResponse += chunk;
            }
        }
        
        // Check if the mock provider returned tool calls and convert them to XML format
        const currentResponseIndex = this.mockProvider.currentResponseIndex - 1;
        const mockResponse = this.mockProvider.responses[currentResponseIndex];
        if (mockResponse?.toolCalls) {
            // Convert AI SDK tool calls to XML format that ProbeChat expects
            for (const toolCall of mockResponse.toolCalls) {
                const xmlToolCall = this.convertToolCallToXml(toolCall);
                fullResponse += '\n\n' + xmlToolCall;
            }
        }
        
        const assistantMessage = { role: 'assistant', content: fullResponse };
        this.messages.push(assistantMessage);
        
        return fullResponse;
    };
    
    // Helper to convert AI SDK tool calls to XML format
    probeChat.convertToolCallToXml = function(toolCall) {
        const { toolName, args } = toolCall;
        let xmlContent = `<${toolName}>`;
        
        if (args) {
            for (const [key, value] of Object.entries(args)) {
                xmlContent += `\n<${key}>${value}</${key}>`;
            }
        }
        
        xmlContent += `\n</${toolName}>`;
        return xmlContent;
    };
    
    probeChat.resetConversation = function() {
        this.messages = [];
    };
    
    probeChat.clearHistory = function() {
        this.messages = [];
        this.history = this.messages; // Alias for compatibility
        return 'mock-session-id'; // Return a mock session ID like the real implementation
    };
    
    probeChat.handleToolError = function(error, toolName) {
        console.error(`Tool error in ${toolName}:`, error);
        return `Error executing ${toolName}: ${error.message}`;
    };
    
    probeChat.processImageUrl = async function(url) {
        return 'mock-base64-image-data';
    };
    
    return { probeChat, mockProvider };
}

// Helper to capture console output
export function captureConsoleOutput() {
    const originalLog = console.log;
    const originalError = console.error;
    const output = [];
    const errors = [];
    
    console.log = (...args) => {
        output.push(args.join(' '));
    };
    
    console.error = (...args) => {
        errors.push(args.join(' '));
    };
    
    return {
        output,
        errors,
        restore: () => {
            console.log = originalLog;
            console.error = originalError;
        },
        getOutput: () => output.join('\n'),
        getErrors: () => errors.join('\n')
    };
}

// Helper to create temporary test files
export function createTempTestFiles(files) {
    const tempDir = path.join(__dirname, 'temp-test-' + Date.now());
    fs.mkdirSync(tempDir, { recursive: true });
    
    const createdFiles = [];
    
    for (const [filePath, content] of Object.entries(files)) {
        const fullPath = path.join(tempDir, filePath);
        const dir = path.dirname(fullPath);
        fs.mkdirSync(dir, { recursive: true });
        fs.writeFileSync(fullPath, content);
        createdFiles.push(fullPath);
    }
    
    return {
        tempDir,
        files: createdFiles,
        cleanup: () => {
            fs.rmSync(tempDir, { recursive: true, force: true });
        }
    };
}

// Helper to run a chat interaction and capture results
export async function runChatInteraction(probeChat, messages, options = {}) {
    const results = {
        responses: [],
        toolCalls: [],
        errors: [],
        streamedText: ''
    };
    
    // Override chat to handle tool calls from mock responses
    const originalChat = probeChat.chat.bind(probeChat);
    probeChat.chat = async function(message, opts = {}) {
        const response = await originalChat(message, {
            ...opts,
            onStream: (text) => {
                results.streamedText += text;
                if (opts.onStream) opts.onStream(text);
            }
        });
        
        // Check if the mock provider returned tool calls and execute them
        const lastCall = probeChat.mockProvider?.capturedCalls[probeChat.mockProvider.capturedCalls.length - 1];
        if (lastCall) {
            const mockResponse = probeChat.mockProvider.responses[probeChat.mockProvider.currentResponseIndex - 1];
            if (mockResponse?.toolCalls) {
                for (const toolCall of mockResponse.toolCalls) {
                    results.toolCalls.push(toolCall);
                    
                    // Actually execute the tool to trigger the test's tracking
                    const toolName = toolCall.toolName;
                    if (probeChat.tools && probeChat.tools[toolName]) {
                        try {
                            // Validate tool arguments if validate method exists
                            if (typeof probeChat.tools[toolName].validate === 'function') {
                                probeChat.tools[toolName].validate(toolCall.args);
                            }
                            
                            await probeChat.tools[toolName].execute(toolCall.args);
                        } catch (error) {
                            // Tool execution errors are expected in some tests
                            console.warn(`Tool execution error for ${toolName}:`, error.message);
                            
                            // Call the global error handler if it exists (for testing)
                            if (typeof probeChat.handleToolError === 'function') {
                                probeChat.handleToolError(error, toolName);
                            }
                            
                            // Call the tool-specific error handler if it exists (for testing)
                            if (typeof probeChat.tools[toolName].handleError === 'function') {
                                probeChat.tools[toolName].handleError(error);
                            }
                        }
                    }
                    
                    if (opts.onToolCall) opts.onToolCall(toolCall);
                }
            }
        }
        
        return response;
    };
    
    // Store reference to mock provider in probeChat for access
    if (probeChat.model?.mockProvider) {
        probeChat.mockProvider = probeChat.model.mockProvider;
    }
    
    for (const message of messages) {
        try {
            let currentResponse = await probeChat.chat(message, options);
            results.responses.push(currentResponse);
            
            // Continue the conversation if there are more responses with tool calls
            // This simulates multi-turn tool calling conversations
            let maxTurns = 10; // Prevent infinite loops
            while (maxTurns > 0 && probeChat.mockProvider && 
                   probeChat.mockProvider.currentResponseIndex < probeChat.mockProvider.responses.length) {
                
                const nextResponseIndex = probeChat.mockProvider.currentResponseIndex;
                const nextResponse = probeChat.mockProvider.responses[nextResponseIndex];
                
                // If the next response has tool calls, continue the conversation
                if (nextResponse?.toolCalls) {
                    // Add a synthetic assistant message to continue the flow
                    const continuationResponse = await probeChat.chat({
                        role: 'assistant',
                        content: nextResponse.text || 'Continuing with next tool...'
                    }, options);
                    results.responses.push(continuationResponse);
                } else {
                    // No more tool calls, conversation ends
                    break;
                }
                
                maxTurns--;
            }
        } catch (error) {
            results.errors.push(error);
        }
    }
    
    // Restore original chat
    probeChat.chat = originalChat;
    
    return results;
}

// Helper to create test probe search results
export function createMockProbeResults(options = {}) {
    const defaultResult = {
        path: options.path || 'test/file.js',
        matches: options.matches || [{
            line: 1,
            column: 0,
            match: options.match || 'test match',
            context: options.context || 'function test() { return "test match"; }'
        }],
        score: options.score || 0.95,
        excerpt: options.excerpt || 'function test() { return "test match"; }',
        symbols: options.symbols || [{
            name: 'test',
            kind: 'function',
            line: 1
        }]
    };
    
    return {
        results: options.results || [defaultResult],
        total_matches: options.total_matches || 1,
        search_time_ms: options.search_time_ms || 50,
        bytes_processed: options.bytes_processed || 1000,
        files_searched: options.files_searched || 10
    };
}

// Helper to wait for async operations
export function waitFor(condition, timeout = 5000) {
    return new Promise((resolve, reject) => {
        const startTime = Date.now();
        const interval = setInterval(() => {
            if (condition()) {
                clearInterval(interval);
                resolve();
            } else if (Date.now() - startTime > timeout) {
                clearInterval(interval);
                reject(new Error('Timeout waiting for condition'));
            }
        }, 100);
    });
}

// Test assertion helpers
export const assert = {
    includesText: (actual, expected, message) => {
        if (!actual.includes(expected)) {
            throw new Error(message || `Expected "${actual}" to include "${expected}"`);
        }
    },
    
    toolCallMade: (toolCalls, toolName, message) => {
        const found = toolCalls.some(call => call.toolName === toolName);
        if (!found) {
            throw new Error(message || `Expected tool call "${toolName}" to be made`);
        }
    },
    
    noErrors: (errors, message) => {
        if (errors.length > 0) {
            throw new Error(message || `Expected no errors but got: ${errors.map(e => e.message).join(', ')}`);
        }
    },
    
    responseCount: (responses, expected, message) => {
        if (responses.length !== expected) {
            throw new Error(message || `Expected ${expected} responses but got ${responses.length}`);
        }
    }
};

// Export test data
export const testData = {
    sampleCode: {
        javascript: `function fibonacci(n) {
    if (n <= 1) return n;
    return fibonacci(n - 1) + fibonacci(n - 2);
}`,
        python: `def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)`,
        rust: `fn fibonacci(n: u32) -> u32 {
    match n {
        0 | 1 => n,
        _ => fibonacci(n - 1) + fibonacci(n - 2)
    }
}`
    },
    
    sampleQueries: {
        simple: 'function fibonacci',
        complex: 'function AND (recursive OR recursion)',
        withPath: { query: 'fibonacci', path: './src' },
        withOptions: { 
            query: 'fibonacci', 
            maxResults: 5,
            maxTokens: 1000
        }
    }
};