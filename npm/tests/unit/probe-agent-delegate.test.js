import { describe, test, expect, beforeEach, jest } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

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
      expect(message1).not.toContain('implement');

      const agent2 = new ProbeAgent({ enableDelegate: false, allowEdit: true });
      const message2 = await agent2.getSystemMessage();
      expect(message2).not.toContain('## delegate');
      expect(message2).toContain('implement');

      const agent3 = new ProbeAgent({ enableDelegate: true, allowEdit: true });
      const message3 = await agent3.getSystemMessage();
      expect(message3).toContain('delegate');
      expect(message3).toContain('implement');
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
});
