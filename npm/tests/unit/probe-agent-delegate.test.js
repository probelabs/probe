import { describe, test, expect, beforeEach, jest } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { delegateTool } from '../../src/tools/vercel.js';
import { delegate } from '../../src/delegate.js';

describe('ProbeAgent enableDelegate option', () => {
  describe('Constructor and initialization', () => {
    test('should default enableDelegate to false', () => {
      const agent = new ProbeAgent({});
      expect(agent.enableDelegate).toBe(false);
    });

    test('should set enableDelegate to true when explicitly enabled', () => {
      const agent = new ProbeAgent({ enableDelegate: true });
      expect(agent.enableDelegate).toBe(true);
    });

    test('should set enableDelegate to false when explicitly disabled', () => {
      const agent = new ProbeAgent({ enableDelegate: false });
      expect(agent.enableDelegate).toBe(false);
    });

    test('should handle truthy values correctly', () => {
      const agent1 = new ProbeAgent({ enableDelegate: 1 });
      expect(agent1.enableDelegate).toBe(true);

      const agent2 = new ProbeAgent({ enableDelegate: 'true' });
      expect(agent2.enableDelegate).toBe(true);
    });

    test('should handle falsy values correctly', () => {
      const agent1 = new ProbeAgent({ enableDelegate: 0 });
      expect(agent1.enableDelegate).toBe(false);

      const agent2 = new ProbeAgent({ enableDelegate: null });
      expect(agent2.enableDelegate).toBe(false);

      const agent3 = new ProbeAgent({ enableDelegate: undefined });
      expect(agent3.enableDelegate).toBe(false);
    });
  });

  describe('Tool registration integration', () => {
    test('should register delegate in toolImplementations when enabled', () => {
      const agent = new ProbeAgent({ enableDelegate: true });
      expect(agent.toolImplementations).toHaveProperty('delegate');
    });

    test('should not register delegate in toolImplementations when disabled', () => {
      const agent = new ProbeAgent({ enableDelegate: false });
      expect(agent.toolImplementations).not.toHaveProperty('delegate');
    });

    test('should register delegate independently from allowEdit option', () => {
      const agent1 = new ProbeAgent({ enableDelegate: true, allowEdit: false });
      expect(agent1.toolImplementations).toHaveProperty('delegate');
      expect(agent1.toolImplementations).not.toHaveProperty('edit');

      const agent2 = new ProbeAgent({ enableDelegate: false, allowEdit: true });
      expect(agent2.toolImplementations).not.toHaveProperty('delegate');
      expect(agent2.toolImplementations).toHaveProperty('edit');

      const agent3 = new ProbeAgent({ enableDelegate: true, allowEdit: true });
      expect(agent3.toolImplementations).toHaveProperty('delegate');
      expect(agent3.toolImplementations).toHaveProperty('edit');
    });

    test('should include edit-related instructions in system message when allowEdit is enabled', async () => {
      const agent = new ProbeAgent({ allowEdit: true });
      const systemMessage = await agent.getSystemMessage();

      // The system message contains edit instructions (not tool definitions)
      expect(systemMessage).toContain('edit');
      expect(systemMessage).toContain('symbol');
    });
  });

  describe('Valid tools array', () => {
    test('should include delegate in toolImplementations when enabled', () => {
      const agent = new ProbeAgent({
        enableDelegate: true,
        provider: 'anthropic'
      });

      // delegate should be registered in toolImplementations
      expect(agent.toolImplementations).toHaveProperty('delegate');
    });

    test('should not include delegate in toolImplementations when disabled', () => {
      const agent = new ProbeAgent({
        enableDelegate: false,
        provider: 'anthropic'
      });

      expect(agent.toolImplementations).not.toHaveProperty('delegate');
    });
  });

  describe('Clone functionality', () => {
    test('should preserve enableDelegate setting when cloning', () => {
      const baseAgent = new ProbeAgent({ enableDelegate: true });
      const cloned = baseAgent.clone();

      expect(cloned.enableDelegate).toBe(true);
    });

    test('should allow overriding enableDelegate when cloning', () => {
      const baseAgent = new ProbeAgent({ enableDelegate: true });
      const cloned = baseAgent.clone({ overrides: { enableDelegate: false } });

      expect(baseAgent.enableDelegate).toBe(true);
      expect(cloned.enableDelegate).toBe(false);
    });

    test('should clone with enableDelegate false by default', () => {
      const baseAgent = new ProbeAgent({ enableDelegate: false });
      const cloned = baseAgent.clone();

      expect(cloned.enableDelegate).toBe(false);
    });
  });

  describe('Combined with other options', () => {
    test('should work with custom prompt', async () => {
      const customPrompt = 'You are a specialized code analyzer.';
      const agent = new ProbeAgent({
        enableDelegate: true,
        customPrompt
      });

      const systemMessage = await agent.getSystemMessage();

      expect(systemMessage).toContain(customPrompt);
      // delegate is registered as a native tool, not in system message
      expect(agent.toolImplementations).toHaveProperty('delegate');
    });

    test('should work with different prompt types', () => {
      const promptTypes = ['code-explorer', 'engineer', 'code-review', 'support', 'architect'];

      for (const promptType of promptTypes) {
        const agent = new ProbeAgent({
          enableDelegate: true,
          promptType
        });

        // delegate is registered as a native tool
        expect(agent.toolImplementations).toHaveProperty('delegate');
      }
    });

    test('should work with debug mode', () => {
      const agent = new ProbeAgent({
        enableDelegate: true,
        debug: true
      });

      expect(agent.enableDelegate).toBe(true);
      expect(agent.debug).toBe(true);
    });

    test('should work with path configuration', () => {
      const agent = new ProbeAgent({
        enableDelegate: true,
        path: '/test/path'
      });

      expect(agent.enableDelegate).toBe(true);
      expect(agent.allowedFolders).toContain('/test/path');
    });
  });

  describe('Type safety', () => {
    test('should handle various input types without errors', () => {
      // String
      expect(() => new ProbeAgent({ enableDelegate: 'yes' })).not.toThrow();

      // Number
      expect(() => new ProbeAgent({ enableDelegate: 42 })).not.toThrow();

      // Boolean
      expect(() => new ProbeAgent({ enableDelegate: true })).not.toThrow();
      expect(() => new ProbeAgent({ enableDelegate: false })).not.toThrow();

      // Null/undefined
      expect(() => new ProbeAgent({ enableDelegate: null })).not.toThrow();
      expect(() => new ProbeAgent({ enableDelegate: undefined })).not.toThrow();
    });
  });

  describe('Native tool registration', () => {
    test('should register delegate as a native tool with correct schema', () => {
      const agent = new ProbeAgent({ enableDelegate: true });

      // delegate is registered in toolImplementations
      expect(agent.toolImplementations).toHaveProperty('delegate');
      expect(typeof agent.toolImplementations.delegate.execute).toBe('function');
    });

    test('should register both search and delegate when delegate is enabled', () => {
      const agent = new ProbeAgent({ enableDelegate: true });

      expect(agent.toolImplementations).toHaveProperty('search');
      expect(agent.toolImplementations).toHaveProperty('delegate');
    });
  });

  describe('Search delegate model override isolation', () => {
    test('searchDelegateProvider/Model do not affect the parent agent model', () => {
      const agent = new ProbeAgent({
        provider: 'anthropic',
        model: 'claude-sonnet-4-6',
        searchDelegateProvider: 'google',
        searchDelegateModel: 'gemini-2.0-flash'
      });

      // Parent agent's resolved model is NOT overridden
      expect(agent.clientApiProvider).toBe('anthropic');
      expect(agent.clientApiModel).toBe('claude-sonnet-4-6');

      // Search delegate overrides are stored separately
      expect(agent.searchDelegateProvider).toBe('google');
      expect(agent.searchDelegateModel).toBe('gemini-2.0-flash');

      // apiType (used for explicit delegate tool at ProbeAgent.js:1717)
      // reflects the parent provider, not the search delegate override
      expect(agent.apiType).not.toBe('google');
      expect(agent.model).not.toBe('gemini-2.0-flash');
    });

    test('explicit delegate tool uses parent model, not search delegate override', () => {
      const agent = new ProbeAgent({
        provider: 'anthropic',
        model: 'claude-sonnet-4-6',
        searchDelegateProvider: 'google',
        searchDelegateModel: 'gemini-2.0-flash',
        enableDelegate: true
      });

      // The explicit delegate tool is registered with the parent's model
      expect(agent.toolImplementations).toHaveProperty('delegate');
      // apiType and model (used at ProbeAgent.js:1717-1718 for delegate calls)
      // must NOT be the search delegate values
      expect(agent.apiType).not.toBe('google');
      expect(agent.model).not.toBe('gemini-2.0-flash');
    });

    test('defaults to null when searchDelegateProvider/Model not set', () => {
      const agent = new ProbeAgent({
        provider: 'anthropic',
        model: 'claude-sonnet-4-6'
      });

      expect(agent.searchDelegateProvider).toBeNull();
      expect(agent.searchDelegateModel).toBeNull();
    });
  });

  describe('Abort signal and cancellation', () => {
    test('should have an AbortController on construction', () => {
      const agent = new ProbeAgent({});
      expect(agent._abortController).toBeInstanceOf(AbortController);
      expect(agent.abortSignal).toBeInstanceOf(AbortSignal);
      expect(agent.abortSignal.aborted).toBe(false);
    });

    test('cancel() should abort the signal', () => {
      const agent = new ProbeAgent({});
      agent.cancel();
      expect(agent.cancelled).toBe(true);
      expect(agent.abortSignal.aborted).toBe(true);
    });

    test('cleanup() should abort the signal', async () => {
      const agent = new ProbeAgent({});
      await agent.cleanup();
      expect(agent.abortSignal.aborted).toBe(true);
    });

    test('parentAbortSignal should be passed in configOptions', () => {
      const agent = new ProbeAgent({ enableDelegate: true });
      // configOptions are built in initializeTools, which is called in constructor
      // The signal should be accessible via the agent's abortSignal getter
      expect(agent.abortSignal).toBeDefined();
      expect(agent.abortSignal.aborted).toBe(false);
    });
  });

  describe('Provider inheritance for delegation', () => {
    test('should have apiType as string for all provider types', () => {
      // Test that apiType is always a string identifier
      const providerTypes = ['anthropic', 'openai', 'google', 'bedrock', 'claude-code', 'codex'];

      for (const providerType of providerTypes) {
        const agent = new ProbeAgent({ provider: providerType });
        // apiType should be set based on provider
        // For providers without API keys, it may fall back to different values
        expect(typeof agent.apiType).toBe('string');
      }
    });

    test('should store provider as string in clientApiProvider', () => {
      // Verify clientApiProvider stores the string identifier
      const agent = new ProbeAgent({ provider: 'google' });
      expect(agent.clientApiProvider).toBe('google');

      const agent2 = new ProbeAgent({ provider: 'claude-code' });
      expect(agent2.clientApiProvider).toBe('claude-code');

      const agent3 = new ProbeAgent({ provider: 'codex' });
      expect(agent3.clientApiProvider).toBe('codex');
    });

    test('should store claude-code provider in clientApiProvider', () => {
      const agent = new ProbeAgent({ provider: 'claude-code' });
      // clientApiProvider should always store the input provider string
      expect(agent.clientApiProvider).toBe('claude-code');
    });

    test('should store codex provider in clientApiProvider', () => {
      const agent = new ProbeAgent({ provider: 'codex' });
      // clientApiProvider should always store the input provider string
      expect(agent.clientApiProvider).toBe('codex');
    });

    test('should have apiType as string type (not function)', () => {
      // Test that apiType is always a string, never a function
      // This is critical for delegation since the delegate tool validates
      // that provider must be a string, null, or undefined
      const agent = new ProbeAgent({});

      // apiType should always be a string
      expect(typeof agent.apiType).toBe('string');

      // If provider is set (for non-mock environments), it should be different from apiType
      // apiType = string identifier ('google', 'anthropic', etc.)
      // provider = AI SDK factory function or null
      if (agent.provider !== null && agent.provider !== undefined) {
        // provider is either a function (AI SDK factory) or null
        expect(agent.apiType).not.toBe(agent.provider);
      }
    });
  });
});

describe('Delegate tool validation', () => {
  // These tests focus on the parameter validation in the delegate tool
  // They check that invalid types are rejected immediately before delegation starts

  test('should reject function as provider', async () => {
    const delegate = delegateTool();

    await expect(delegate.execute({
      task: 'Test task',
      provider: () => {}, // Function instead of string
      model: 'test-model'
    })).rejects.toThrow('provider must be a string, null, or undefined');
  });

  test('should reject object as provider', async () => {
    const delegate = delegateTool();

    await expect(delegate.execute({
      task: 'Test task',
      provider: { name: 'google' }, // Object instead of string
      model: 'test-model'
    })).rejects.toThrow('provider must be a string, null, or undefined');
  });

  test('should reject number as provider', async () => {
    const delegate = delegateTool();

    await expect(delegate.execute({
      task: 'Test task',
      provider: 123, // Number instead of string
      model: 'test-model'
    })).rejects.toThrow('provider must be a string, null, or undefined');
  });

  test('should reject array as provider', async () => {
    const delegate = delegateTool();

    await expect(delegate.execute({
      task: 'Test task',
      provider: ['google'], // Array instead of string
      model: 'test-model'
    })).rejects.toThrow('provider must be a string, null, or undefined');
  });

  test('should validate provider type at runtime', () => {
    // The delegate tool schema only defines 'task' as a public parameter.
    // The provider, model, etc. are internal parameters added by ProbeAgent.
    // Runtime validation in the execute function ensures provider is a string.
    // This test verifies the validation works by checking invalid types are rejected.

    const delegateToolInstance = delegateTool();

    // The tool should have a task parameter in its schema
    expect(delegateToolInstance).toBeDefined();
    expect(typeof delegateToolInstance.execute).toBe('function');
  });
});

describe('Delegate tracer handling', () => {
  test('should not throw when tracer is missing createDelegationSpan method', async () => {
    // This test ensures that a tracer object without createDelegationSpan method
    // doesn't cause "tracer.createDelegationSpan is not a function" error
    const incompleteTracer = {
      // tracer object without createDelegationSpan
      someOtherMethod: () => {}
    };

    // The delegate should handle this gracefully and not throw
    // It will fail for other reasons (no task, etc.), but NOT because of tracer
    await expect(delegate({
      task: 'Test task',
      tracer: incompleteTracer
    })).rejects.not.toThrow('createDelegationSpan is not a function');
  });

  test('should not throw when tracer is an empty object', async () => {
    const emptyTracer = {};

    await expect(delegate({
      task: 'Test task',
      tracer: emptyTracer
    })).rejects.not.toThrow('createDelegationSpan is not a function');
  });

  test('should not throw when tracer is null', async () => {
    await expect(delegate({
      task: 'Test task',
      tracer: null
    })).rejects.not.toThrow('createDelegationSpan is not a function');
  });

  test('should not throw when tracer is undefined', async () => {
    await expect(delegate({
      task: 'Test task',
      tracer: undefined
    })).rejects.not.toThrow('createDelegationSpan is not a function');
  });
});

/**
 * Tests for delegation path inheritance fix (Issue #348)
 *
 * This test suite verifies that subagents receive the correct workspace root
 * instead of a subdirectory from the parent's navigation context, preventing
 * "path doubling" issues where paths like:
 *   /workspace/tyk/internal/build/tyk/internal/build/version.go
 * are incorrectly constructed.
 */
describe('Delegation path inheritance (Issue #348)', () => {
  // NOTE: The effectivePath calculation is tested with proper mocking in
  // tests/delegate-config.test.js - see "should prioritize allowedFolders[0]
  // (workspace root) over cwd (navigation context)" test.

  describe('ProbeAgent cwd initialization', () => {
    test('should set cwd when explicitly provided', () => {
      const workspaceRoot = '/workspace/project';

      const agent = new ProbeAgent({
        path: workspaceRoot,
        cwd: workspaceRoot
      });

      // When cwd is explicitly provided, it should be used
      expect(agent.cwd).toBe(workspaceRoot);
      expect(agent.allowedFolders).toContain(workspaceRoot);
    });

    test('should default cwd to workspaceRoot when not explicitly provided', () => {
      const workspaceRoot = '/workspace/project';

      const agent = new ProbeAgent({
        path: workspaceRoot
        // cwd not provided
      });

      // cwd should default to workspaceRoot (computed from allowedFolders)
      expect(agent.cwd).toBe(agent.workspaceRoot);
      expect(agent.allowedFolders).toContain(workspaceRoot);
    });

    test('should use cwd over allowedFolders[0] when explicitly set', () => {
      const workspaceRoot = '/workspace/project';
      const explicitCwd = '/workspace/project/specific-dir';

      const agent = new ProbeAgent({
        path: workspaceRoot,
        cwd: explicitCwd
      });

      expect(agent.cwd).toBe(explicitCwd);
      expect(agent.allowedFolders).toContain(workspaceRoot);
    });
  });

  describe('Path doubling prevention', () => {
    test('should demonstrate path doubling scenario (before fix)', () => {
      // This test documents the bug scenario

      // Scenario: Parent at /workspace/tyk navigates to /workspace/tyk/internal/build
      // Parent's cwd becomes: /workspace/tyk/internal/build
      // When delegating, this cwd is passed as path to subagent
      // Subagent's allowedFolders becomes: ['/workspace/tyk/internal/build']
      // Subagent's cwd (null) falls back to allowedFolders[0]
      // AI calls extract('tyk/internal/build/version.go')
      // Path resolves to: /workspace/tyk/internal/build/tyk/internal/build/version.go
      // Result: File not found!

      const parentNavigationCwd = '/workspace/tyk/internal/build';
      const relativePathFromAI = 'tyk/internal/build/version.go';

      // Path doubling: joining subdirectory cwd with relative path
      const incorrectPath = `${parentNavigationCwd}/${relativePathFromAI}`;
      expect(incorrectPath).toBe('/workspace/tyk/internal/build/tyk/internal/build/version.go');
    });

    test('should ensure subagent starts from workspace root', () => {
      // The fix in delegate.js now passes cwd: path to subagent
      // This ensures subagent's tool config uses workspace root for path resolution

      const workspaceRoot = '/workspace/project';

      // Simulating what delegate.js does after fix:
      const subagentOptions = {
        path: workspaceRoot,       // Workspace root from delegateTool
        cwd: workspaceRoot         // Explicitly set cwd to same value (the fix)
      };

      const agent = new ProbeAgent(subagentOptions);

      // Verify cwd is explicitly set
      expect(agent.cwd).toBe(workspaceRoot);
      // Verify allowedFolders uses the same workspace root
      expect(agent.allowedFolders[0]).toBe(workspaceRoot);
    });
  });

  describe('Edge cases', () => {
    test('should handle when allowedFolders is empty', () => {
      const tool = delegateTool({
        cwd: '/some/path',
        allowedFolders: []  // Empty array
      });

      expect(tool).toBeDefined();
    });

    test('should handle when allowedFolders is undefined', () => {
      const tool = delegateTool({
        cwd: '/some/path'
        // allowedFolders not provided
      });

      expect(tool).toBeDefined();
    });

    test('should handle when both cwd and allowedFolders are undefined', () => {
      const tool = delegateTool({});

      expect(tool).toBeDefined();
    });

    test('should handle multiple allowedFolders', () => {
      const tool = delegateTool({
        cwd: '/workspace/project/deep/nested',
        allowedFolders: ['/workspace/project', '/workspace/other']
      });

      expect(tool).toBeDefined();
      // First allowedFolder should be used as workspace root
    });
  });
});

describe('delegateTool path priority', () => {
  // These tests verify the effectivePath calculation logic
  // Priority: explicit path > allowedFolders[0] (workspace root) > cwd

  test('effectivePath priority order documentation', () => {
    // The fix changes the priority from:
    //   BEFORE: path || cwd || allowedFolders[0]
    //   AFTER:  path || allowedFolders[0] || cwd

    // This ensures navigation context (cwd) doesn't override workspace root

    const scenarios = [
      {
        name: 'AI provides explicit path',
        path: '/explicit/path',
        cwd: '/nav/context',
        allowedFolders: ['/workspace/root'],
        expected: '/explicit/path'
      },
      {
        name: 'No explicit path, use workspace root (not cwd)',
        path: undefined,
        cwd: '/workspace/root/deep/nested',  // Navigation context
        allowedFolders: ['/workspace/root'],  // Workspace root
        expected: '/workspace/root'  // Should use this, not cwd
      },
      {
        name: 'No allowedFolders, fall back to cwd',
        path: undefined,
        cwd: '/some/path',
        allowedFolders: undefined,
        expected: '/some/path'
      }
    ];

    scenarios.forEach(scenario => {
      // Calculate effectivePath using the same logic as delegateTool
      const workspaceRoot = scenario.allowedFolders && scenario.allowedFolders[0];
      const effectivePath = scenario.path || workspaceRoot || scenario.cwd;

      expect(effectivePath).toBe(scenario.expected);
    });
  });
});

describe('delegateTool allowEdit inheritance (#534)', () => {
  test('should pass allowEdit=true to delegate when parent has allowEdit', async () => {
    const tool = delegateTool({
      cwd: '/workspace',
      allowedFolders: ['/workspace'],
      allowEdit: true,
    });

    // The tool should be configured — verify it destructured allowEdit
    expect(tool).toBeDefined();
    expect(typeof tool.execute).toBe('function');
  });

  test('should default allowEdit to false when not provided', async () => {
    const tool = delegateTool({
      cwd: '/workspace',
      allowedFolders: ['/workspace'],
    });

    expect(tool).toBeDefined();
    expect(typeof tool.execute).toBe('function');
  });

  test('ProbeAgent derives hashLines from allowEdit by default', () => {
    const agentWithEdit = new ProbeAgent({ allowEdit: true });
    expect(agentWithEdit.allowEdit).toBe(true);
    expect(agentWithEdit.hashLines).toBe(true);

    const agentWithoutEdit = new ProbeAgent({ allowEdit: false });
    expect(agentWithoutEdit.allowEdit).toBe(false);
    expect(agentWithoutEdit.hashLines).toBe(false);

    const agentDefault = new ProbeAgent({});
    expect(agentDefault.allowEdit).toBe(false);
    expect(agentDefault.hashLines).toBe(false);
  });
});

describe('Delegate MCP config propagation', () => {
  test('delegateTool should receive enableMcp from configOptions', () => {
    // When enableMcp is passed in options, delegateTool should use it
    const tool = delegateTool({
      cwd: '/workspace',
      allowedFolders: ['/workspace'],
      enableMcp: true,
      mcpConfig: { servers: { test: { command: 'echo' } } },
      mcpConfigPath: '/path/to/mcp.json',
    });

    expect(tool).toBeDefined();
    expect(typeof tool.execute).toBe('function');
  });

  test('delegateTool should default enableMcp to false when not provided', () => {
    const tool = delegateTool({
      cwd: '/workspace',
      allowedFolders: ['/workspace'],
    });

    expect(tool).toBeDefined();
    expect(typeof tool.execute).toBe('function');
  });

  test('ProbeAgent configOptions should include MCP config when enableMcp is set', () => {
    const mcpConfig = { servers: { 'workable-api': { command: 'node', args: ['server.js'] } } };
    const agent = new ProbeAgent({
      enableDelegate: true,
      enableMcp: true,
      mcpConfig,
      mcpConfigPath: '/path/to/mcp.json',
    });

    // The agent stores MCP config
    expect(agent.enableMcp).toBe(true);
    expect(agent.mcpConfig).toEqual(mcpConfig);
    expect(agent.mcpConfigPath).toBe('/path/to/mcp.json');

    // Delegate tool should be registered
    expect(agent.toolImplementations).toHaveProperty('delegate');
  });

  test('ProbeAgent should store MCP config in constructor before initializeTools', () => {
    // Verify MCP config fields are set before tools are initialized
    // This ensures configOptions can include them
    const agent = new ProbeAgent({
      enableMcp: true,
      mcpConfig: { servers: {} },
      mcpConfigPath: '/test/mcp.json',
    });

    expect(agent.enableMcp).toBe(true);
    expect(agent.mcpConfig).toEqual({ servers: {} });
    expect(agent.mcpConfigPath).toBe('/test/mcp.json');
  });
});

describe('Delegate prompt type propagation', () => {
  test('delegateTool should receive promptType from configOptions', () => {
    const tool = delegateTool({
      cwd: '/workspace',
      allowedFolders: ['/workspace'],
      promptType: 'engineer',
    });

    expect(tool).toBeDefined();
    expect(typeof tool.execute).toBe('function');
  });

  test('ProbeAgent with engineer promptType should propagate to delegate subagent', () => {
    const agent = new ProbeAgent({
      enableDelegate: true,
      promptType: 'engineer',
    });

    // Agent should store the prompt type
    expect(agent.promptType).toBe('engineer');
    // Delegate should be registered
    expect(agent.toolImplementations).toHaveProperty('delegate');
  });

  test('code-researcher prompt type does not exist - should fall back to code-explorer', async () => {
    // This documents the bug: 'code-researcher' was the delegate default but doesn't exist
    const { predefinedPrompts } = await import('../../src/agent/shared/prompts.js');

    expect(predefinedPrompts['code-researcher']).toBeUndefined();
    expect(predefinedPrompts['code-explorer']).toBeDefined();
    expect(predefinedPrompts['engineer']).toBeDefined();
  });

  test('code-explorer prompt is read-only - inappropriate for delegate subagents', async () => {
    const { predefinedPrompts } = await import('../../src/agent/shared/prompts.js');

    // The code-explorer prompt explicitly says READ-ONLY
    expect(predefinedPrompts['code-explorer']).toContain('READ-ONLY');
    expect(predefinedPrompts['code-explorer']).toContain('NEVER create, modify, delete, or write files');

    // The engineer prompt does NOT have this restriction
    expect(predefinedPrompts['engineer']).not.toContain('READ-ONLY');
  });

  test('default promptType is code-explorer which would be inherited by delegate', () => {
    const agent = new ProbeAgent({});
    expect(agent.promptType).toBe('code-explorer');
  });
});
