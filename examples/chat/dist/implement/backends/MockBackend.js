import BaseBackend from './BaseBackend.js';

/**
 * Mock backend for testing the implement tool functionality
 */
class MockBackend extends BaseBackend {
    constructor() {
        super();
        this.name = 'mock';
        this.displayName = 'Mock Backend';
        this.description = 'A mock backend for testing purposes';
        this.supportedLanguages = ['javascript', 'typescript', 'python', 'rust', 'go', 'java', 'cpp', 'csharp', 'ruby', 'php', 'swift'];
        this.supportedFeatures = {
            streaming: true,
            rollback: true,
            directFileEdit: true,
            gitIntegration: true,
            webSearch: true,
            multiFile: true,
            planning: true,
            testing: true
        };
        
        // Test configuration
        this.responses = [];
        this.currentResponseIndex = 0;
        this.simulateErrors = false;
        this.simulateTimeout = false;
        this.responseDelay = 100; // ms
        this.capturedRequests = [];
        this.sessionCounter = 0;
    }

    // Test helpers
    reset() {
        this.responses = [];
        this.currentResponseIndex = 0;
        this.simulateErrors = false;
        this.simulateTimeout = false;
        this.capturedRequests = [];
        this.sessionCounter = 0;
    }

    addResponse(response) {
        this.responses.push(response);
    }

    setResponses(responses) {
        this.responses = responses;
        this.currentResponseIndex = 0;
    }

    getNextResponse() {
        if (this.responses.length === 0) {
            return this.generateDefaultResponse();
        }
        const response = this.responses[this.currentResponseIndex];
        this.currentResponseIndex = (this.currentResponseIndex + 1) % this.responses.length;
        return response;
    }

    generateDefaultResponse() {
        return {
            type: 'progress',
            message: 'Mock backend processing request',
            details: {
                status: 'complete',
                filesModified: ['mock-file.js'],
                summary: 'Successfully processed mock request'
            }
        };
    }

    async checkAvailability() {
        // Always available unless configured otherwise
        if (this.simulateErrors) {
            return {
                available: false,
                reason: 'Mock backend configured to simulate unavailability'
            };
        }
        return {
            available: true,
            version: '1.0.0-mock'
        };
    }

    async checkHealth() {
        if (this.simulateErrors) {
            return {
                healthy: false,
                status: 'error',
                message: 'Mock backend configured to simulate unhealthy state'
            };
        }
        return {
            healthy: true,
            status: 'ready',
            details: {
                mockSessions: this.sessionCounter,
                capturedRequests: this.capturedRequests.length
            }
        };
    }

    validateRequest(request) {
        const errors = [];
        
        if (!request.request || typeof request.request !== 'string') {
            errors.push('Request must include a "request" field with the implementation request');
        }
        
        if (this.simulateErrors && request.request && request.request.includes('error')) {
            errors.push('Mock backend configured to reject requests containing "error"');
        }
        
        return errors;
    }

    async implement(request, options = {}) {
        this.capturedRequests.push({ request, options, timestamp: Date.now() });
        
        // Simulate timeout
        if (this.simulateTimeout) {
            await new Promise(resolve => setTimeout(resolve, 60000));
            throw new Error('Mock backend timeout');
        }
        
        // Simulate errors
        if (this.simulateErrors) {
            throw new Error('Mock backend configured to simulate error');
        }
        
        const sessionId = `mock-session-${++this.sessionCounter}`;
        const response = this.getNextResponse();
        
        // Handle dry run
        if (options.dryRun) {
            return {
                sessionId,
                status: 'dry-run',
                plan: response.plan || 'Mock implementation plan:\n1. Analyze request\n2. Generate code\n3. Apply changes',
                estimatedFiles: response.estimatedFiles || ['mock-file.js'],
                estimatedComplexity: response.estimatedComplexity || 'low'
            };
        }
        
        // Simulate streaming responses
        if (options.onProgress && response.stream) {
            for (const chunk of response.stream) {
                await new Promise(resolve => setTimeout(resolve, this.responseDelay));
                options.onProgress(chunk);
            }
        } else if (options.onProgress) {
            // Default streaming behavior
            await new Promise(resolve => setTimeout(resolve, this.responseDelay));
            options.onProgress({
                type: 'start',
                message: 'Mock backend starting implementation'
            });
            
            await new Promise(resolve => setTimeout(resolve, this.responseDelay));
            options.onProgress({
                type: 'progress',
                message: 'Analyzing request and generating code'
            });
            
            await new Promise(resolve => setTimeout(resolve, this.responseDelay));
            options.onProgress({
                type: 'file_update',
                file: response.file || 'mock-file.js',
                action: 'modified',
                content: response.content || '// Mock generated code\nconsole.log("Hello from mock backend!");'
            });
            
            await new Promise(resolve => setTimeout(resolve, this.responseDelay));
            options.onProgress({
                type: 'complete',
                message: 'Implementation complete'
            });
        }
        
        // Return final result
        return {
            sessionId,
            status: 'success',
            filesModified: response.filesModified || ['mock-file.js'],
            summary: response.summary || 'Mock implementation completed successfully',
            details: response.details || {
                linesAdded: 10,
                linesRemoved: 2,
                filesCreated: 0,
                filesModified: 1
            }
        };
    }

    async cancelSession(sessionId) {
        return {
            sessionId,
            status: 'cancelled',
            message: 'Mock session cancelled'
        };
    }

    async getSessionStatus(sessionId) {
        return {
            sessionId,
            status: 'completed',
            progress: 100,
            message: 'Mock session completed'
        };
    }

    // Test-specific methods
    getLastRequest() {
        return this.capturedRequests[this.capturedRequests.length - 1];
    }

    getAllRequests() {
        return this.capturedRequests;
    }

    setErrorMode(enabled) {
        this.simulateErrors = enabled;
    }

    setTimeoutMode(enabled) {
        this.simulateTimeout = enabled;
    }

    setResponseDelay(delay) {
        this.responseDelay = delay;
    }
}

export default MockBackend;