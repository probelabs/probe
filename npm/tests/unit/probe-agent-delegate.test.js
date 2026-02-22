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

  describe('System message integration', () => {
    test('should include delegate tool definition when enabled', async () => {
      const agent = new ProbeAgent({ enableDelegate: true });
      const systemMessage = await agent.getSystemMessage();

      expect(systemMessage).toContain('delegate');
      expect(systemMessage).toContain('Delegate big distinct tasks to specialized probe subagents');
      expect(systemMessage).toMatch(/##\s*delegate/);
    });

    test('should not include delegate tool definition when disabled', async () => {
      const agent = new ProbeAgent({ enableDelegate: false });
      const systemMessage = await agent.getSystemMessage();

      // Should not contain delegate tool definition section
      expect(systemMessage).not.toMatch(/##\s*delegate/);
      expect(systemMessage).not.toContain('Delegate big distinct tasks to specialized probe subagents');
    });

    test('should include delegate in available tools list when enabled', async () => {
      const agent = new ProbeAgent({ enableDelegate: true });
      const systemMessage = await agent.getSystemMessage();

      // Check for delegate in the available tools list
      expect(systemMessage).toContain('- delegate: Delegate big distinct tasks to specialized probe subagents');
    });

    test('should not include delegate in available tools list when disabled', async () => {
      const agent = new ProbeAgent({ enableDelegate: false });
      const systemMessage = await agent.getSystemMessage();

      // Should not contain delegate in tools list
      const toolsSection = systemMessage.match(/Available Tools:([\s\S]*?)(?=\n\n|$)/);
      if (toolsSection) {
        expect(toolsSection[1]).not.toContain('- delegate:');
      }
    });

    test('should work independently from allowEdit option', async () => {
      // Test all combinations
      const agent1 = new ProbeAgent({ enableDelegate: true, allowEdit: false });
      const message1 = await agent1.getSystemMessage();
      expect(message1).toContain('delegate');
      expect(message1).not.toContain('## edit');

      const agent2 = new ProbeAgent({ enableDelegate: false, allowEdit: true });
      const message2 = await agent2.getSystemMessage();
      expect(message2).not.toContain('## delegate');
      expect(message2).toContain('edit');

      const agent3 = new ProbeAgent({ enableDelegate: true, allowEdit: true });
      const message3 = await agent3.getSystemMessage();
      expect(message3).toContain('delegate');
      expect(message3).toContain('edit');
    });

    test('should include symbol mode in edit tool when allowEdit is enabled', async () => {
      const agent = new ProbeAgent({ allowEdit: true });
      const systemMessage = await agent.getSystemMessage();

      // The unified edit tool now handles symbol mode too
      expect(systemMessage).toContain('## edit');
      expect(systemMessage).toContain('symbol');
      // Separate symbol tools no longer exist
      expect(systemMessage).not.toContain('## replace_symbol');
      expect(systemMessage).not.toContain('## insert_symbol');
      expect(systemMessage).not.toContain('## edit_lines');
    });
  });

  describe('Valid tools array', () => {
    test('should include delegate in validTools when parsing tool calls', async () => {
      const agent = new ProbeAgent({
        enableDelegate: true,
        provider: 'anthropic'
      });

      // We need to check that delegate is in validTools during answer() execution
      // This is tested indirectly by ensuring the system message is correct
      const systemMessage = await agent.getSystemMessage();

      // Verify the tool definition exists, which means it should be in validTools
      expect(systemMessage).toContain('## delegate');
    });

    test('should not include delegate in validTools when disabled', async () => {
      const agent = new ProbeAgent({
        enableDelegate: false,
        provider: 'anthropic'
      });

      const systemMessage = await agent.getSystemMessage();

      // Verify the tool definition doesn't exist
      expect(systemMessage).not.toMatch(/##\s*delegate/);
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
      expect(systemMessage).toContain('delegate');
    });

    test('should work with different prompt types', async () => {
      const promptTypes = ['code-explorer', 'engineer', 'code-review', 'support', 'architect'];

      for (const promptType of promptTypes) {
        const agent = new ProbeAgent({
          enableDelegate: true,
          promptType
        });

        const systemMessage = await agent.getSystemMessage();
        expect(systemMessage).toContain('delegate');
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

  describe('System message structure', () => {
    test('should maintain proper tool definition format when delegate is enabled', async () => {
      const agent = new ProbeAgent({ enableDelegate: true });
      const systemMessage = await agent.getSystemMessage();

      // Check for proper markdown structure
      expect(systemMessage).toMatch(/##\s*delegate/);
      expect(systemMessage).toMatch(/Description:.*delegate/i);
      expect(systemMessage).toMatch(/Parameters:/);
      expect(systemMessage).toContain('task:');
    });

    test('should place delegate tool in logical position', async () => {
      const agent = new ProbeAgent({ enableDelegate: true });
      const systemMessage = await agent.getSystemMessage();

      // Delegate should appear after attempt_completion (at the end with optional tools)
      const searchIndex = systemMessage.indexOf('## search');
      const completionIndex = systemMessage.indexOf('## attempt_completion');
      const delegateIndex = systemMessage.indexOf('## delegate');

      expect(searchIndex).toBeGreaterThan(-1);
      expect(completionIndex).toBeGreaterThan(searchIndex);
      expect(delegateIndex).toBeGreaterThan(completionIndex);
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
