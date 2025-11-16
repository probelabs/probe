import { describe, test, expect, beforeEach } from '@jest/globals';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('ProbeAgent allowedTools option', () => {
  describe('_parseAllowedTools helper', () => {
    test('should default to all tools when allowedTools is undefined', () => {
      const agent = new ProbeAgent({
        path: process.cwd()
      });

      expect(agent.allowedTools.mode).toBe('all');
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(true);
      expect(agent.allowedTools.isEnabled('bash')).toBe(true);
    });

    test('should allow all tools with ["*"]', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['*']
      });

      expect(agent.allowedTools.mode).toBe('all');
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(true);
      expect(agent.allowedTools.isEnabled('bash')).toBe(true);
      expect(agent.allowedTools.isEnabled('implement')).toBe(true);
    });

    test('should support exclusions with "*" and "!" prefix', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['*', '!bash', '!implement']
      });

      expect(agent.allowedTools.mode).toBe('all');
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(true);
      expect(agent.allowedTools.isEnabled('bash')).toBe(false);
      expect(agent.allowedTools.isEnabled('implement')).toBe(false);
    });

    test('should disable all tools with empty array', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: []
      });

      expect(agent.allowedTools.mode).toBe('none');
      expect(agent.allowedTools.isEnabled('search')).toBe(false);
      expect(agent.allowedTools.isEnabled('query')).toBe(false);
      expect(agent.allowedTools.isEnabled('bash')).toBe(false);
      expect(agent.allowedTools.isEnabled('attempt_completion')).toBe(false);
    });

    test('should allow specific tools only (whitelist mode)', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['search', 'query', 'extract']
      });

      expect(agent.allowedTools.mode).toBe('whitelist');
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(true);
      expect(agent.allowedTools.isEnabled('extract')).toBe(true);
      expect(agent.allowedTools.isEnabled('bash')).toBe(false);
      expect(agent.allowedTools.isEnabled('implement')).toBe(false);
      expect(agent.allowedTools.isEnabled('listFiles')).toBe(false);
    });

    test('should ignore exclusions in whitelist mode', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['search', 'query', '!bash']
      });

      expect(agent.allowedTools.mode).toBe('whitelist');
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(true);
      expect(agent.allowedTools.isEnabled('bash')).toBe(false);
    });
  });

  describe('tool filtering behavior', () => {
    test('should filter tools in initialization', async () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['search', 'extract']
      });

      // Tool implementations should respect the filter
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('extract')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(false);
      expect(agent.allowedTools.isEnabled('bash')).toBe(false);
    });

    test('should respect both allowEdit flag and allowedTools', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowEdit: true,
        allowedTools: ['*', '!implement']
      });

      // Edit and create should be enabled (allowEdit=true, not in exclusion list)
      expect(agent.allowedTools.isEnabled('edit')).toBe(true);
      expect(agent.allowedTools.isEnabled('create')).toBe(true);
      // Implement should be excluded (in exclusion list)
      expect(agent.allowedTools.isEnabled('implement')).toBe(false);
    });

    test('should respect both enableBash flag and allowedTools', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        enableBash: true,
        allowedTools: ['search', 'extract']
      });

      // Bash should not be allowed even though enableBash=true
      // because it's not in the allowedTools whitelist
      expect(agent.allowedTools.isEnabled('bash')).toBe(false);
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('extract')).toBe(true);
    });

    test('should allow bash when both enableBash and allowedTools permit it', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        enableBash: true,
        allowedTools: ['*']
      });

      expect(agent.allowedTools.isEnabled('bash')).toBe(true);
    });
  });

  describe('tool execution with allowedTools', () => {
    test('should block tool execution when not in allowedTools', async () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['search', 'attempt_completion']
      });
      await agent.initialize();

      // Mock the AI to try using the 'query' tool which is not allowed
      const mockAIResponse = `<thinking>I need to use the query tool</thinking>
<query>
<pattern>function</pattern>
<path>.</path>
</query>`;

      // We can't easily test the full flow without mocking, but we can verify
      // that the tool check works in the _parseAllowedTools logic
      expect(agent.allowedTools.isEnabled('query')).toBe(false);
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
    });
  });

  describe('raw AI mode (no tools)', () => {
    test('should set raw AI mode with empty allowedTools array', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: []
      });

      expect(agent.allowedTools.mode).toBe('none');
      expect(agent.allowedTools.isEnabled('search')).toBe(false);
      expect(agent.allowedTools.isEnabled('attempt_completion')).toBe(false);
    });
  });

  describe('clone with allowedTools', () => {
    test('should preserve allowedTools configuration in clone', () => {
      const baseAgent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['search', 'query']
      });

      const cloned = baseAgent.clone();

      expect(cloned.allowedTools.mode).toBe('whitelist');
      expect(cloned.allowedTools.isEnabled('search')).toBe(true);
      expect(cloned.allowedTools.isEnabled('query')).toBe(true);
      expect(cloned.allowedTools.isEnabled('bash')).toBe(false);
    });

    test('should preserve exclusions in clone', () => {
      const baseAgent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['*', '!bash', '!implement']
      });

      const cloned = baseAgent.clone();

      expect(cloned.allowedTools.mode).toBe('all');
      expect(cloned.allowedTools.isEnabled('search')).toBe(true);
      expect(cloned.allowedTools.isEnabled('query')).toBe(true);
      expect(cloned.allowedTools.isEnabled('bash')).toBe(false);
      expect(cloned.allowedTools.isEnabled('implement')).toBe(false);
    });

    test('should allow override of allowedTools in clone', () => {
      const baseAgent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['search']
      });

      const cloned = baseAgent.clone({
        overrides: {
          allowedTools: ['query', 'extract']
        }
      });

      expect(baseAgent.allowedTools.isEnabled('search')).toBe(true);
      expect(baseAgent.allowedTools.isEnabled('query')).toBe(false);

      expect(cloned.allowedTools.isEnabled('search')).toBe(false);
      expect(cloned.allowedTools.isEnabled('query')).toBe(true);
      expect(cloned.allowedTools.isEnabled('extract')).toBe(true);
    });
  });

  describe('invalid tool names', () => {
    test('should accept invalid tool names in whitelist mode without error', () => {
      // This allows for forward compatibility with new tools
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['search', 'nonexistent', 'another_fake']
      });

      expect(agent.allowedTools.mode).toBe('whitelist');
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('nonexistent')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(false);
    });

    test('should handle all invalid tool names gracefully', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['nope', 'invalid', 'fake']
      });

      expect(agent.allowedTools.mode).toBe('whitelist');
      // Invalid tools are "allowed" but won't match any real tools
      expect(agent.allowedTools.isEnabled('nope')).toBe(true);
      expect(agent.allowedTools.isEnabled('search')).toBe(false);
      expect(agent.allowedTools.isEnabled('query')).toBe(false);
    });
  });

  describe('validTools array in agentic loop', () => {
    test('should respect allowedTools when building validTools array', async () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['search', 'extract', 'attempt_completion']
      });
      await agent.initialize();

      // Verify allowedTools configuration
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('extract')).toBe(true);
      expect(agent.allowedTools.isEnabled('attempt_completion')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(false);
      expect(agent.allowedTools.isEnabled('listFiles')).toBe(false);
    });

    test('should build empty validTools array when disableTools is true', async () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        disableTools: true
      });
      await agent.initialize();

      // Verify that no tools are enabled
      expect(agent.allowedTools.mode).toBe('none');
      expect(agent.allowedTools.isEnabled('search')).toBe(false);
      expect(agent.allowedTools.isEnabled('query')).toBe(false);
      expect(agent.allowedTools.isEnabled('extract')).toBe(false);
      expect(agent.allowedTools.isEnabled('attempt_completion')).toBe(false);
    });

    test('should respect both enableBash flag and allowedTools in validTools', async () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        enableBash: true,
        allowedTools: ['search', 'bash', 'attempt_completion']
      });
      await agent.initialize();

      // Bash should be enabled because both enableBash=true AND it's in allowedTools
      expect(agent.allowedTools.isEnabled('bash')).toBe(true);
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('attempt_completion')).toBe(true);
      // Query is not in allowedTools
      expect(agent.allowedTools.isEnabled('query')).toBe(false);
    });

    test('should not include bash in validTools when not in allowedTools', async () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        enableBash: true,
        allowedTools: ['search', 'query', 'attempt_completion']
      });
      await agent.initialize();

      // Bash should NOT be enabled even though enableBash=true
      // because it's not in the allowedTools list
      expect(agent.allowedTools.isEnabled('bash')).toBe(false);
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(true);
    });

    test('should not include edit tools in validTools when not in allowedTools', async () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowEdit: true,
        allowedTools: ['search', 'query', 'attempt_completion']
      });
      await agent.initialize();

      // Edit tools should NOT be enabled even though allowEdit=true
      // because they're not in the allowedTools list
      expect(agent.allowedTools.isEnabled('implement')).toBe(false);
      expect(agent.allowedTools.isEnabled('edit')).toBe(false);
      expect(agent.allowedTools.isEnabled('create')).toBe(false);
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
    });

    test('should include edit tools in validTools when in allowedTools', async () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowEdit: true,
        allowedTools: ['search', 'implement', 'attempt_completion']
      });
      await agent.initialize();

      // Implement should be enabled because both allowEdit=true AND it's in allowedTools
      expect(agent.allowedTools.isEnabled('implement')).toBe(true);
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('attempt_completion')).toBe(true);
    });
  });

  describe('disableTools convenience flag', () => {
    test('should disable all tools when disableTools is true', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        disableTools: true
      });

      expect(agent.allowedTools.mode).toBe('none');
      expect(agent.allowedTools.isEnabled('search')).toBe(false);
      expect(agent.allowedTools.isEnabled('query')).toBe(false);
      expect(agent.allowedTools.isEnabled('attempt_completion')).toBe(false);
    });

    test('should take precedence over allowedTools when both are set', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['search', 'query', 'extract'],
        disableTools: true  // This should win
      });

      expect(agent.allowedTools.mode).toBe('none');
      expect(agent.allowedTools.isEnabled('search')).toBe(false);
      expect(agent.allowedTools.isEnabled('query')).toBe(false);
    });

    test('should not affect allowedTools when disableTools is false', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['search', 'query'],
        disableTools: false
      });

      expect(agent.allowedTools.mode).toBe('whitelist');
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(true);
    });
  });

  describe('CLI parsing compatibility', () => {
    test('should support CLI exclusion syntax: "*,!bash,!implement"', () => {
      // Simulate CLI parsing: --allowed-tools "*,!bash,!implement"
      const toolsArg = '*,!bash,!implement';
      const allowedTools = toolsArg.split(',').map(t => t.trim()).filter(t => t.length > 0);

      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: allowedTools
      });

      expect(allowedTools).toEqual(['*', '!bash', '!implement']);
      expect(agent.allowedTools.mode).toBe('all');
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(true);
      expect(agent.allowedTools.isEnabled('bash')).toBe(false);
      expect(agent.allowedTools.isEnabled('implement')).toBe(false);
    });

    test('should support CLI whitelist syntax: "search,query,extract"', () => {
      // Simulate CLI parsing: --allowed-tools "search,query,extract"
      const toolsArg = 'search,query,extract';
      const allowedTools = toolsArg.split(',').map(t => t.trim()).filter(t => t.length > 0);

      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: allowedTools
      });

      expect(allowedTools).toEqual(['search', 'query', 'extract']);
      expect(agent.allowedTools.mode).toBe('whitelist');
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(true);
      expect(agent.allowedTools.isEnabled('extract')).toBe(true);
      expect(agent.allowedTools.isEnabled('bash')).toBe(false);
    });

    test('should support CLI MCP wildcard syntax: "mcp__filesystem__*,search"', () => {
      // Simulate CLI parsing: --allowed-tools "mcp__filesystem__*,search"
      const toolsArg = 'mcp__filesystem__*,search';
      const allowedTools = toolsArg.split(',').map(t => t.trim()).filter(t => t.length > 0);

      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: allowedTools
      });

      expect(allowedTools).toEqual(['mcp__filesystem__*', 'search']);
      expect(agent.allowedTools.mode).toBe('whitelist');
      expect(agent.allowedTools.isEnabled('mcp__filesystem__read_file')).toBe(true);
      expect(agent.allowedTools.isEnabled('mcp__filesystem__write_file')).toBe(true);
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('mcp__github__list_issues')).toBe(false);
    });
  });

  describe('MCP tool filtering with mcp__ prefix', () => {
    test('should allow MCP tools with mcp__ prefix', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['mcp__filesystem__read_file', 'search']
      });

      expect(agent.allowedTools.isEnabled('mcp__filesystem__read_file')).toBe(true);
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(false);
    });

    test('should support wildcard patterns for MCP tools', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['mcp__filesystem__*', 'search']
      });

      expect(agent.allowedTools.isEnabled('mcp__filesystem__read_file')).toBe(true);
      expect(agent.allowedTools.isEnabled('mcp__filesystem__write_file')).toBe(true);
      expect(agent.allowedTools.isEnabled('mcp__github__list_issues')).toBe(false);
      expect(agent.allowedTools.isEnabled('search')).toBe(true);
    });

    test('should block MCP tools with exclusion patterns', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['*', '!mcp__*']
      });

      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('query')).toBe(true);
      expect(agent.allowedTools.isEnabled('mcp__filesystem__read_file')).toBe(false);
      expect(agent.allowedTools.isEnabled('mcp__github__list_issues')).toBe(false);
    });

    test('should support specific MCP server exclusions', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['*', '!mcp__filesystem__*']
      });

      expect(agent.allowedTools.isEnabled('search')).toBe(true);
      expect(agent.allowedTools.isEnabled('mcp__github__list_issues')).toBe(true);
      expect(agent.allowedTools.isEnabled('mcp__filesystem__read_file')).toBe(false);
      expect(agent.allowedTools.isEnabled('mcp__filesystem__write_file')).toBe(false);
    });
  });
});
