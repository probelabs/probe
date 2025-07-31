import { describe, it, before, after, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';
import path from 'path';
import { 
    setupTestEnvironment, 
    createTestProbeChat, 
    captureConsoleOutput,
    runChatInteraction,
    createTempTestFiles,
    createMockProbeResults,
    testData 
} from '../testUtils.js';
import { mockResponses } from '../mocks/mockLLMProvider.js';

describe('Tool Calling Integration Tests', () => {
    let testEnv;
    let tempFiles;
    
    before(() => {
        testEnv = setupTestEnvironment();
    });
    
    after(() => {
        testEnv.restore();
        if (tempFiles) {
            tempFiles.cleanup();
        }
    });
    
    beforeEach(() => {
        // Create temporary test files
        tempFiles = createTempTestFiles({
            'src/math.js': testData.sampleCode.javascript,
            'src/math.py': testData.sampleCode.python,
            'src/math.rs': testData.sampleCode.rust,
            'test/math.test.js': 'describe("math tests", () => {});'
        });
    });
    
    afterEach(() => {
        if (tempFiles) {
            tempFiles.cleanup();
            tempFiles = null;
        }
    });
    
    describe('Probe Search Tool', () => {
        it('should handle search tool calls', async () => {
            const { probeChat, mockProvider } = await createTestProbeChat({
                responses: [
                    mockResponses.withToolCall,
                    { text: 'I found the search results for your query.' }
                ]
            });
            
            // Mock the probe search tool
            probeChat.tools.probe_search.execute = async (args) => {
                return createMockProbeResults({
                    path: path.join(tempFiles.tempDir, 'src/math.js'),
                    match: 'fibonacci',
                    context: testData.sampleCode.javascript
                });
            };
            
            const results = await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Search for fibonacci functions' }]
            );
            
            assert.strictEqual(results.toolCalls.length, 1);
            assert.strictEqual(results.toolCalls[0].toolName, 'probe_search');
            assert.deepStrictEqual(results.toolCalls[0].args, {
                query: 'test query',
                path: './src'
            });
            assert.strictEqual(results.errors.length, 0);
        });
        
        it('should handle multiple search tool calls', async () => {
            const { probeChat } = await createTestProbeChat({
                responses: [
                    mockResponses.multipleToolCalls,
                    { text: 'I completed both search and extract operations.' }
                ]
            });
            
            // Mock the tools
            let toolExecutions = [];
            probeChat.tools.probe_search.execute = async (args) => {
                toolExecutions.push({ tool: 'search', args });
                return createMockProbeResults();
            };
            
            probeChat.tools.probe_extract.execute = async (args) => {
                toolExecutions.push({ tool: 'extract', args });
                return {
                    content: testData.sampleCode.javascript,
                    language: 'javascript',
                    symbols: []
                };
            };
            
            await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Search and extract code' }]
            );
            
            assert.strictEqual(toolExecutions.length, 2);
            assert.strictEqual(toolExecutions[0].tool, 'search');
            assert.strictEqual(toolExecutions[1].tool, 'extract');
        });
        
        it('should handle search with advanced options', async () => {
            const { probeChat } = await createTestProbeChat({
                responses: [{
                    text: 'Let me search with those specific parameters.',
                    toolCalls: [{
                        toolName: 'probe_search',
                        args: {
                            query: 'async function',
                            path: './src',
                            maxResults: 10,
                            maxTokens: 2000
                        }
                    }]
                }, { text: 'Found results with your criteria.' }]
            });
            
            let capturedArgs;
            probeChat.tools.probe_search.execute = async (args) => {
                capturedArgs = args;
                return createMockProbeResults();
            };
            
            await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Search for async functions in src, limit to 10 results' }]
            );
            
            assert.deepStrictEqual(capturedArgs, {
                query: 'async function',
                path: './src',
                maxResults: 10,
                maxTokens: 2000
            });
        });
    });
    
    describe('Implement Tool', () => {
        it('should handle implement tool calls', async () => {
            const { probeChat, mockBackend } = await createTestProbeChat({
                responses: [
                    mockResponses.implementToolCall,
                    { text: 'The implementation is complete!' }
                ],
                useMockBackend: true,
                mockBackendResponses: [{
                    filesModified: ['src/math.js'],
                    summary: 'Added fibonacci function'
                }]
            });
            
            const results = await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Add a fibonacci function to math.js' }]
            );
            
            assert.strictEqual(results.toolCalls.length, 1);
            assert.strictEqual(results.toolCalls[0].toolName, 'implement');
            assert.strictEqual(mockBackend.capturedRequests.length, 1);
            
            const request = mockBackend.getLastRequest();
            assert.strictEqual(request.request.request, 'Add a new function to calculate fibonacci numbers');
            assert.deepStrictEqual(request.request.files, ['src/math.js']);
            assert.strictEqual(request.request.backend, 'mock');
        });
        
        it('should handle implement tool with streaming', async () => {
            const { probeChat, mockBackend } = await createTestProbeChat({
                responses: [
                    mockResponses.implementToolCall,
                    { text: 'Implementation completed with streaming!' }
                ],
                useMockBackend: true,
                mockBackendResponses: [{
                    stream: [
                        { type: 'start', message: 'Starting implementation' },
                        { type: 'progress', message: 'Generating code' },
                        { type: 'file_update', file: 'src/math.js', action: 'modified' },
                        { type: 'complete', message: 'Done' }
                    ],
                    filesModified: ['src/math.js']
                }]
            });
            
            mockBackend.setResponseDelay(50); // Fast streaming for tests
            
            let streamEvents = [];
            const originalImplementExecute = probeChat.tools.implement.execute;
            probeChat.tools.implement.execute = async function(args) {
                return originalImplementExecute.call(this, args, {
                    onProgress: (event) => {
                        streamEvents.push(event);
                    }
                });
            };
            
            await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Implement fibonacci with streaming' }]
            );
            
            assert.ok(streamEvents.length >= 4, 'Should receive multiple stream events');
            assert.strictEqual(streamEvents[0].type, 'start');
            assert.strictEqual(streamEvents[streamEvents.length - 1].type, 'complete');
        });
        
        it('should handle backend selection', async () => {
            const { probeChat, mockBackend } = await createTestProbeChat({
                responses: [{
                    text: 'I will use the mock backend.',
                    toolCalls: [{
                        toolName: 'implement',
                        args: {
                            request: 'Test backend selection',
                            backend: 'mock'
                        }
                    }]
                }, { text: 'Done with mock backend.' }],
                useMockBackend: true
            });
            
            await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Implement something using mock backend' }]
            );
            
            const request = mockBackend.getLastRequest();
            assert.ok(request, 'Should capture the request');
            assert.strictEqual(request.request.backend, 'mock');
        });
        
        it('should handle backend errors', async () => {
            const { probeChat, mockBackend } = await createTestProbeChat({
                responses: [
                    mockResponses.implementToolCall,
                    { text: 'The implementation failed, but I handled it gracefully.' }
                ],
                useMockBackend: true
            });
            
            mockBackend.setErrorMode(true);
            
            let toolError;
            probeChat.tools.implement.handleError = (error) => {
                toolError = error;
            };
            
            const results = await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Try to implement with error' }]
            );
            
            // The tool should handle the error gracefully
            assert.strictEqual(results.errors.length, 0, 'Chat should not error');
            assert.ok(toolError, 'Tool should capture the error');
        });
    });
    
    describe('Query Tool', () => {
        it('should handle semantic query tool calls', async () => {
            const { probeChat } = await createTestProbeChat({
                responses: [{
                    text: 'Let me query for that pattern.',
                    toolCalls: [{
                        toolName: 'probe_query',
                        args: {
                            pattern: 'function $name($args) { $body }',
                            path: './src'
                        }
                    }]
                }, { text: 'Found matching patterns.' }]
            });
            
            let queryArgs;
            probeChat.tools.probe_query.execute = async (args) => {
                queryArgs = args;
                return {
                    matches: [{
                        file: 'src/math.js',
                        matches: [{ text: 'function fibonacci(n) { ... }' }]
                    }]
                };
            };
            
            await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Find all function definitions' }]
            );
            
            assert.deepStrictEqual(queryArgs, {
                pattern: 'function $name($args) { $body }',
                path: './src'
            });
        });
    });
    
    describe('Extract Tool', () => {
        it('should handle code extraction', async () => {
            const { probeChat } = await createTestProbeChat({
                responses: [{
                    text: 'Let me extract that code section.',
                    toolCalls: [{
                        toolName: 'probe_extract',
                        args: {
                            location: 'src/math.js:1-5'
                        }
                    }]
                }, { text: 'Here is the extracted code.' }]
            });
            
            let extractArgs;
            probeChat.tools.probe_extract.execute = async (args) => {
                extractArgs = args;
                return {
                    content: testData.sampleCode.javascript,
                    language: 'javascript',
                    start_line: 1,
                    end_line: 5
                };
            };
            
            await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Extract lines 1-5 from math.js' }]
            );
            
            assert.deepStrictEqual(extractArgs, {
                location: 'src/math.js:1-5'
            });
        });
        
        it('should handle symbol extraction', async () => {
            const { probeChat } = await createTestProbeChat({
                responses: [{
                    text: 'Extracting the fibonacci function.',
                    toolCalls: [{
                        toolName: 'probe_extract',
                        args: {
                            location: 'src/math.js:fibonacci'
                        }
                    }]
                }, { text: 'Extracted the function successfully.' }]
            });
            
            probeChat.tools.probe_extract.execute = async (args) => {
                assert.strictEqual(args.location, 'src/math.js:fibonacci');
                return {
                    content: testData.sampleCode.javascript,
                    language: 'javascript',
                    symbol: 'fibonacci'
                };
            };
            
            await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Extract the fibonacci function from math.js' }]
            );
        });
    });
    
    describe('Tool Error Handling', () => {
        it('should handle tool execution errors', async () => {
            const { probeChat } = await createTestProbeChat({
                responses: [
                    mockResponses.withToolCall,
                    { text: 'I encountered an error but will continue.' }
                ]
            });
            
            // Make the tool throw an error
            probeChat.tools.probe_search.execute = async () => {
                throw new Error('Search tool error');
            };
            
            let capturedToolError;
            probeChat.handleToolError = (error, toolName) => {
                capturedToolError = { error, toolName };
                return 'Tool error handled';
            };
            
            const results = await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Search for something' }]
            );
            
            assert.ok(capturedToolError);
            assert.strictEqual(capturedToolError.toolName, 'probe_search');
            assert.ok(capturedToolError.error.message.includes('Search tool error'));
        });
        
        it('should handle invalid tool arguments', async () => {
            const { probeChat } = await createTestProbeChat({
                responses: [{
                    text: 'Calling tool with invalid args.',
                    toolCalls: [{
                        toolName: 'probe_search',
                        args: {} // Missing required 'query' field
                    }]
                }, { text: 'Handling the validation error.' }]
            });
            
            let validationError;
            probeChat.tools.probe_search.validate = (args) => {
                if (!args.query) {
                    validationError = new Error('Missing required field: query');
                    throw validationError;
                }
            };
            
            await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Search without query' }]
            );
            
            assert.ok(validationError);
            assert.ok(validationError.message.includes('Missing required field'));
        });
    });
    
    describe('Complex Tool Sequences', () => {
        it('should handle search → extract → implement workflow', async () => {
            const { probeChat, mockBackend } = await createTestProbeChat({
                responses: [
                    {
                        text: 'Let me search for existing implementations.',
                        toolCalls: [{
                            toolName: 'probe_search',
                            args: { query: 'fibonacci', path: './src' }
                        }]
                    },
                    {
                        text: 'Now extracting the current implementation.',
                        toolCalls: [{
                            toolName: 'probe_extract',
                            args: { location: 'src/math.js:fibonacci' }
                        }]
                    },
                    {
                        text: 'Implementing the optimized version.',
                        toolCalls: [{
                            toolName: 'implement',
                            args: { 
                                request: 'Optimize fibonacci with memoization',
                                files: ['src/math.js'],
                                backend: 'mock'
                            }
                        }]
                    },
                    { text: 'Successfully optimized the fibonacci function!' }
                ],
                useMockBackend: true
            });
            
            // Mock tool implementations
            probeChat.tools.probe_search.execute = async () => createMockProbeResults();
            probeChat.tools.probe_extract.execute = async () => ({
                content: testData.sampleCode.javascript,
                language: 'javascript'
            });
            
            const results = await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Optimize the fibonacci function' }]
            );
            
            assert.strictEqual(results.toolCalls.length, 3);
            assert.strictEqual(results.toolCalls[0].toolName, 'probe_search');
            assert.strictEqual(results.toolCalls[1].toolName, 'probe_extract');
            assert.strictEqual(results.toolCalls[2].toolName, 'implement');
            assert.strictEqual(mockBackend.capturedRequests.length, 1);
        });
    });
});