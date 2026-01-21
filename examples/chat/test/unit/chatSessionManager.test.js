import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert';
import { ChatSessionManager } from '../../ChatSessionManager.js';

// Store original environment
let originalEnv;

describe('ChatSessionManager Tests', () => {
  beforeEach(() => {
    // Store original environment
    originalEnv = { ...process.env };

    // Set test mode
    process.env.NODE_ENV = 'test';
    process.env.USE_MOCK_AI = 'true';
  });

  afterEach(() => {
    // Restore original environment
    Object.keys(process.env).forEach(key => {
      if (!(key in originalEnv)) {
        delete process.env[key];
      }
    });
    Object.assign(process.env, originalEnv);
  });

  describe('initialize()', () => {
    it('should call agent.initialize() when initializing', async () => {
      // Create a ChatSessionManager
      const session = new ChatSessionManager({
        sessionId: 'test-session-123',
        debug: false
      });

      // Track if agent.initialize was called
      let agentInitializeCalled = false;
      const originalInitialize = session.agent.initialize.bind(session.agent);
      session.agent.initialize = async function() {
        agentInitializeCalled = true;
        return originalInitialize();
      };

      // Initialize the session
      await session.initialize();

      // Verify agent.initialize was called
      assert.strictEqual(agentInitializeCalled, true, 'agent.initialize() should be called during ChatSessionManager.initialize()');
    });

    it('should only initialize once even if called multiple times', async () => {
      const session = new ChatSessionManager({
        sessionId: 'test-session-456',
        debug: false
      });

      let initializeCallCount = 0;
      const originalInitialize = session.agent.initialize.bind(session.agent);
      session.agent.initialize = async function() {
        initializeCallCount++;
        return originalInitialize();
      };

      // Call initialize multiple times
      await session.initialize();
      await session.initialize();
      await session.initialize();

      // Should only be called once due to _ready flag
      assert.strictEqual(initializeCallCount, 1, 'agent.initialize() should only be called once');
    });

    it('should set _ready flag after successful initialization', async () => {
      const session = new ChatSessionManager({
        sessionId: 'test-session-789',
        debug: false
      });

      assert.strictEqual(session._ready, false, '_ready should be false before initialize');

      await session.initialize();

      assert.strictEqual(session._ready, true, '_ready should be true after initialize');
    });
  });
});
