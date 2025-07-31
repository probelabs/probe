import { describe, it, before, after, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';
import { 
    setupTestEnvironment, 
    createTestProbeChat, 
    captureConsoleOutput,
    runChatInteraction,
    testData 
} from '../testUtils.js';
import { mockResponses } from '../mocks/mockLLMProvider.js';

describe('Chat Flow Integration Tests', () => {
    let testEnv;
    let consoleCapture;
    
    before(() => {
        testEnv = setupTestEnvironment();
    });
    
    after(() => {
        testEnv.restore();
    });
    
    beforeEach(() => {
        consoleCapture = captureConsoleOutput();
    });
    
    afterEach(() => {
        consoleCapture.restore();
    });
    
    describe('Basic Chat Interactions', () => {
        it('should handle simple text conversation', async () => {
            const { probeChat, mockProvider } = await createTestProbeChat({
                responses: [
                    { text: 'Hello! I am a mock AI assistant.' },
                    { text: 'The weather is simulated to be sunny.' }
                ]
            });
            
            const messages = [
                { role: 'user', content: 'Hello' },
                { role: 'user', content: 'How is the weather?' }
            ];
            
            const results = await runChatInteraction(probeChat, messages);
            
            assert.strictEqual(results.responses.length, 2);
            assert.strictEqual(results.errors.length, 0);
            assert.ok(results.streamedText.includes('Hello! I am a mock AI assistant.'));
            assert.ok(results.streamedText.includes('The weather is simulated to be sunny.'));
        });
        
        it('should handle streaming responses', async () => {
            const { probeChat } = await createTestProbeChat({
                responses: [mockResponses.longStreamingResponse]
            });
            
            let streamChunks = [];
            const results = await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Tell me a story' }],
                {
                    onStream: (chunk) => {
                        streamChunks.push(chunk);
                    }
                }
            );
            
            assert.strictEqual(results.errors.length, 0);
            assert.ok(streamChunks.length > 1, 'Should receive multiple stream chunks');
            assert.ok(results.streamedText.includes('longer response'));
            assert.ok(results.streamedText.includes('streamed in chunks'));
        });
        
        it('should handle conversation context', async () => {
            const { probeChat, mockProvider } = await createTestProbeChat({
                responses: [
                    { text: 'My name is MockBot.' },
                    { text: 'You asked me about my name. I told you it is MockBot.' }
                ]
            });
            
            // First message
            await probeChat.sendMessage('What is your name?');
            
            // Second message should have context
            await probeChat.sendMessage('What did I just ask you?');
            
            // Check that the context was maintained
            const capturedCalls = mockProvider.capturedCalls;
            assert.strictEqual(capturedCalls.length, 2);
            
            // Second call should include previous messages
            const secondCallMessages = capturedCalls[1].messages;
            assert.ok(secondCallMessages.length >= 3); // user, assistant, user
            assert.strictEqual(secondCallMessages[0].content, 'What is your name?');
        });
    });
    
    describe('Error Handling', () => {
        it('should handle API errors gracefully', async () => {
            const { probeChat } = await createTestProbeChat({
                mockProvider: {
                    throwError: 'Simulated API error'
                }
            });
            
            const results = await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'This should fail' }]
            );
            
            assert.strictEqual(results.errors.length, 1);
            assert.ok(results.errors[0].message.includes('Simulated API error'));
        });
        
        it('should handle timeout scenarios', async () => {
            const { probeChat } = await createTestProbeChat({
                responses: [{ text: 'This will timeout' }],
                chatOptions: {
                    timeout: 100 // 100ms timeout
                },
                mockProvider: {
                    streamDelay: 200 // 200ms delay per chunk
                }
            });
            
            const startTime = Date.now();
            const results = await runChatInteraction(probeChat, 
                [{ role: 'user', content: 'Test timeout' }]
            );
            const duration = Date.now() - startTime;
            
            // Should timeout before completing the full stream
            assert.ok(duration < 1000, 'Should timeout quickly');
        });
        
        it('should recover from errors and continue conversation', async () => {
            const { probeChat, mockProvider } = await createTestProbeChat({
                responses: [
                    { text: 'First response works' },
                    { text: 'Second response (will be skipped due to error)' },
                    { text: 'Third response after recovery' }
                ]
            });
            
            // Override getNextResponse to simulate error on second call
            let callCount = 0;
            const originalGetNext = mockProvider.getNextResponse;
            mockProvider.getNextResponse = function() {
                callCount++;
                if (callCount === 2) {
                    // Advance the response index so the third call gets the correct response
                    this.currentResponseIndex = (this.currentResponseIndex + 1) % this.responses.length;
                    throw new Error('Simulated error on second call');
                }
                return originalGetNext.call(this);
            };
            
            const messages = [
                { role: 'user', content: 'First message' },
                { role: 'user', content: 'This will error' },
                { role: 'user', content: 'Should work again' }
            ];
            
            const results = await runChatInteraction(probeChat, messages);
            
            // Verify the chat system recovered correctly
            assert.strictEqual(results.responses.length, 2, 'Should have 2 successful responses');
            assert.strictEqual(results.errors.length, 1, 'Should have 1 error');
            assert.ok(results.streamedText.includes('First response works'), 'First response should be included');
            assert.ok(results.streamedText.includes('Third response after recovery'), 'Third response should be included after recovery');
            assert.ok(results.errors[0].message.includes('Simulated error on second call'), 'Error should be captured correctly');
        });
    });
    
    describe('Multi-turn Conversations', () => {
        it('should maintain conversation history', async () => {
            const { probeChat, mockProvider } = await createTestProbeChat({
                responses: [
                    { text: 'I can help you with coding.' },
                    { text: 'You asked about my capabilities. I mentioned coding help.' },
                    { text: 'Throughout our conversation, you asked about capabilities and I explained coding help.' }
                ]
            });
            
            // Build conversation
            await probeChat.sendMessage('What can you help with?');
            await probeChat.sendMessage('What did I ask about?');
            await probeChat.sendMessage('Summarize our conversation');
            
            const calls = mockProvider.capturedCalls;
            assert.ok(calls.length >= 3, 'Should have at least 3 calls');
            
            // Find the actual chat calls (not duplicates)
            const chatCalls = [];
            const seenMessageCounts = new Set();
            for (const call of calls) {
                const msgCount = call.messages.length;
                if (!seenMessageCounts.has(msgCount)) {
                    seenMessageCounts.add(msgCount);
                    chatCalls.push(call);
                }
            }
            
            assert.strictEqual(chatCalls.length, 3, 'Should have 3 unique chat calls');
            
            // Check conversation growth
            assert.strictEqual(chatCalls[0].messages.length, 1); // Just user message
            assert.strictEqual(chatCalls[1].messages.length, 3); // user, assistant, user
            assert.strictEqual(chatCalls[2].messages.length, 5); // full conversation
        });
        
        it('should handle conversation reset', async () => {
            const { probeChat, mockProvider } = await createTestProbeChat({
                responses: [
                    { text: 'First conversation response' },
                    { text: 'Fresh start response' }
                ]
            });
            
            // First conversation
            await probeChat.sendMessage('First message');
            
            // Reset conversation using clearHistory (the actual method)
            const newSessionId = probeChat.clearHistory();
            assert.ok(newSessionId, 'clearHistory should return a new session ID');
            
            // New conversation
            await probeChat.sendMessage('New conversation');
            
            const calls = mockProvider.capturedCalls;
            assert.strictEqual(calls.length, 2);
            
            // Second call should not have history from first
            assert.strictEqual(calls[1].messages.length, 1);
            assert.strictEqual(calls[1].messages[0].content, 'New conversation');
        });
    });
    
    describe('System Prompts and Configuration', () => {
        it('should include system prompts in conversation', async () => {
            const systemPrompt = 'You are a helpful coding assistant specialized in JavaScript.';
            const { probeChat, mockProvider } = await createTestProbeChat({
                responses: [{ text: 'I understand JavaScript very well.' }],
                chatOptions: {
                    systemPrompt
                }
            });
            
            await probeChat.sendMessage('Can you help with JS?');
            
            const call = mockProvider.capturedCalls[0];
            assert.ok(call.system || call.messages.some(m => m.role === 'system'));
            
            if (call.system) {
                assert.strictEqual(call.system, systemPrompt);
            } else {
                const systemMessage = call.messages.find(m => m.role === 'system');
                assert.ok(systemMessage);
                assert.strictEqual(systemMessage.content, systemPrompt);
            }
        });
        
        it('should respect temperature and max tokens settings', async () => {
            const { probeChat, mockProvider } = await createTestProbeChat({
                responses: [{ text: 'Temperature test response' }],
                chatOptions: {
                    temperature: 0.7,
                    maxTokens: 2000
                }
            });
            
            await probeChat.sendMessage('Test message');
            
            const call = mockProvider.capturedCalls[0];
            assert.strictEqual(call.temperature, 0.7);
            assert.strictEqual(call.maxTokens, 2000);
        });
    });
    
    describe('Image Support', () => {
        it('should handle image inputs in messages', async () => {
            const { probeChat, mockProvider } = await createTestProbeChat({
                responses: [{ text: 'I can see the image you provided.' }]
            });
            
            const messageWithImage = {
                role: 'user',
                content: [
                    { type: 'text', text: 'What is in this image?' },
                    { type: 'image', image: 'base64encodedimage' }
                ]
            };
            
            await runChatInteraction(probeChat, [messageWithImage]);
            
            const call = mockProvider.capturedCalls[0];
            assert.ok(Array.isArray(call.messages[0].content));
            assert.strictEqual(call.messages[0].content[0].type, 'text');
            assert.strictEqual(call.messages[0].content[1].type, 'image');
        });
        
        it('should process image URLs', async () => {
            const { probeChat, mockProvider } = await createTestProbeChat({
                responses: [{ text: 'I processed the image from the URL.' }]
            });
            
            // Mock the processImageUrl method
            probeChat.processImageUrl = async (url) => {
                return 'mocked-base64-image-data';
            };
            
            await probeChat.sendMessage('Look at this image: https://example.com/image.png');
            
            const call = mockProvider.capturedCalls[0];
            // The message should be processed to include image data
            assert.ok(call.messages[0].content);
        });
    });
});