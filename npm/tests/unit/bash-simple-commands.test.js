/**
 * Tests for simplified bash tool (rejects complex commands for security)
 * @module tests/unit/bash-simple-commands
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { parseSimpleCommand, parseCommand, parseCommandForExecution, isComplexCommand, isComplexPattern, matchesComplexPattern } from '../../src/agent/bashCommandUtils.js';
import { BashPermissionChecker } from '../../src/agent/bashPermissions.js';

// Mock the 'ai' package since it may not be available in test environment
jest.mock('ai', () => ({
  tool: jest.fn((config) => ({
    name: config.name,
    description: config.description,
    inputSchema: config.inputSchema,
    execute: config.execute
  }))
}));

import { bashTool } from '../../src/tools/bash.js';

describe('Simple Command Parser', () => {
  describe('parseSimpleCommand', () => {
    test('should parse simple commands correctly', () => {
      const result = parseSimpleCommand('ls -la');
      expect(result.success).toBe(true);
      expect(result.command).toBe('ls');
      expect(result.args).toEqual(['-la']);
      expect(result.isComplex).toBe(false);
    });

    test('should handle quoted arguments correctly (fix quote bug)', () => {
      const result = parseSimpleCommand('grep "test file" *.txt');
      expect(result.success).toBe(true);
      expect(result.command).toBe('grep');
      expect(result.args).toEqual(['test file', '*.txt']); // Quotes stripped!
      expect(result.fullArgs).toEqual(['grep', 'test file', '*.txt']);
    });

    test('should handle single quotes correctly', () => {
      const result = parseSimpleCommand("echo 'hello world'");
      expect(result.success).toBe(true);
      expect(result.command).toBe('echo');
      expect(result.args).toEqual(['hello world']); // Single quotes stripped!
    });

    test('should handle mixed quotes', () => {
      const result = parseSimpleCommand('git commit -m "Fixed bug" --author="John Doe"');
      expect(result.success).toBe(true);
      expect(result.command).toBe('git');
      expect(result.args).toEqual(['commit', '-m', 'Fixed bug', '--author=John Doe']);
    });

    test('should reject commands with unclosed quotes', () => {
      const result = parseSimpleCommand('echo "unclosed quote');
      expect(result.success).toBe(false);
      expect(result.error).toContain('Unclosed quote');
    });

    test('should reject empty commands', () => {
      const result = parseSimpleCommand('');
      expect(result.success).toBe(false);
      expect(result.error).toContain('empty');
    });

    test('should reject null/undefined commands', () => {
      expect(parseSimpleCommand(null).success).toBe(false);
      expect(parseSimpleCommand(undefined).success).toBe(false);
    });
  });

  describe('Complex Command Detection', () => {
    test('should detect pipes as complex', () => {
      expect(isComplexCommand('ls | grep test')).toBe(true);
      const result = parseSimpleCommand('ls | grep test');
      expect(result.success).toBe(false);
      expect(result.isComplex).toBe(true);
      expect(result.error).toContain('Complex shell commands');
    });

    test('should detect logical operators as complex', () => {
      expect(isComplexCommand('make && make test')).toBe(true);
      expect(isComplexCommand('make || echo failed')).toBe(true);
    });

    test('should detect command substitution as complex', () => {
      expect(isComplexCommand('echo $(date)')).toBe(true);
      expect(isComplexCommand('ls `pwd`')).toBe(true);
    });

    test('should detect redirections as complex', () => {
      expect(isComplexCommand('ls > output.txt')).toBe(true);
      expect(isComplexCommand('cat < input.txt')).toBe(true);
    });

    test('should detect background execution as complex', () => {
      expect(isComplexCommand('long-task &')).toBe(true);
    });

    test('should detect command separators as complex', () => {
      expect(isComplexCommand('cd /tmp; ls')).toBe(true);
    });

    test('should allow simple commands', () => {
      expect(isComplexCommand('ls -la')).toBe(false);
      expect(isComplexCommand('git status')).toBe(false);
      expect(isComplexCommand('npm test')).toBe(false);
    });
  });

  describe('parseCommandForExecution', () => {
    test('should return array for simple commands', () => {
      const result = parseCommandForExecution('ls -la');
      expect(result).toEqual(['ls', '-la']);
    });

    test('should return null for complex commands', () => {
      const result = parseCommandForExecution('ls | grep test');
      expect(result).toBeNull();
    });

    test('should handle quotes correctly for execution', () => {
      const result = parseCommandForExecution('grep "test file" data.txt');
      expect(result).toEqual(['grep', 'test file', 'data.txt']);
    });
  });
});

describe('Complex Pattern Matching', () => {
  describe('isComplexPattern', () => {
    test('should detect patterns with pipes', () => {
      expect(isComplexPattern('ls | grep *')).toBe(true);
      expect(isComplexPattern('git branch -a | grep *')).toBe(true);
    });

    test('should detect patterns with logical AND', () => {
      expect(isComplexPattern('cd * && git *')).toBe(true);
      expect(isComplexPattern('make && make test')).toBe(true);
    });

    test('should detect patterns with logical OR', () => {
      expect(isComplexPattern('* || echo failed')).toBe(true);
    });

    test('should detect patterns with redirections', () => {
      expect(isComplexPattern('* > output.txt')).toBe(true);
      expect(isComplexPattern('cat < *')).toBe(true);
    });

    test('should detect patterns with semicolon', () => {
      expect(isComplexPattern('cd *; ls')).toBe(true);
    });

    test('should detect patterns with command substitution', () => {
      expect(isComplexPattern('echo $(date)')).toBe(true);
      expect(isComplexPattern('ls `pwd`')).toBe(true);
    });

    test('should detect patterns with background execution', () => {
      expect(isComplexPattern('task &')).toBe(true);
    });

    test('should not detect simple patterns as complex', () => {
      expect(isComplexPattern('git:status')).toBe(false);
      expect(isComplexPattern('ls:*')).toBe(false);
      expect(isComplexPattern('npm:test')).toBe(false);
    });

    test('should handle null/undefined', () => {
      expect(isComplexPattern(null)).toBe(false);
      expect(isComplexPattern(undefined)).toBe(false);
      expect(isComplexPattern('')).toBe(false);
    });
  });

  describe('matchesComplexPattern', () => {
    test('should match exact complex commands', () => {
      expect(matchesComplexPattern('cd /tmp && ls', 'cd /tmp && ls')).toBe(true);
    });

    test('should match with wildcard at end', () => {
      expect(matchesComplexPattern('cd /tmp && git status', 'cd * && git *')).toBe(true);
      expect(matchesComplexPattern('cd /project && git log', 'cd * && git *')).toBe(true);
    });

    test('should match pipe patterns', () => {
      expect(matchesComplexPattern('git branch -a | grep release', 'git branch -a | grep *')).toBe(true);
      expect(matchesComplexPattern('git branch -a | grep feature', 'git branch -a | grep *')).toBe(true);
    });

    test('should match git fetch && git tag patterns', () => {
      expect(matchesComplexPattern('git fetch --tags && git tag -l "v*"', 'git fetch * && git tag *')).toBe(true);
    });

    test('should not match non-matching commands', () => {
      expect(matchesComplexPattern('rm -rf /', 'cd * && git *')).toBe(false);
      expect(matchesComplexPattern('ls | grep test', 'git branch -a | grep *')).toBe(false);
    });

    test('should handle whitespace normalization', () => {
      expect(matchesComplexPattern('cd  /tmp  &&  ls', 'cd * && ls')).toBe(true);
    });

    test('should handle null/undefined', () => {
      expect(matchesComplexPattern(null, 'pattern')).toBe(false);
      expect(matchesComplexPattern('command', null)).toBe(false);
    });
  });
});

describe('Simplified Permission Checker', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({
      allow: ['test:*', 'echo:hello'],
      deny: ['rm:*', 'dangerous:cmd'],
      debug: false
    });
  });

  describe('Complex Command Handling', () => {
    test('should allow complex commands when all components are in allow list', () => {
      // With default patterns, ls and grep are both allowed
      const result = checker.check('ls | grep test');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
      expect(result.allowedByComponents).toBe(true);
    });

    test('should reject complex commands when components are not in allow list', () => {
      // customcmd and othercmd are not in any allow list
      const result = checker.check('customcmd | othercmd');
      expect(result.allowed).toBe(false);
      expect(result.isComplex).toBe(true);
    });

    test('should allow complex commands with matching allow patterns', () => {
      const checkerWithComplex = new BashPermissionChecker({
        allow: ['cd * && git *', 'git branch -a | grep *'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result1 = checkerWithComplex.check('cd /project && git status');
      expect(result1.allowed).toBe(true);
      expect(result1.isComplex).toBe(true);

      const result2 = checkerWithComplex.check('git branch -a | grep release');
      expect(result2.allowed).toBe(true);
      expect(result2.isComplex).toBe(true);
    });

    test('should reject complex commands matching deny patterns', () => {
      const checkerWithComplex = new BashPermissionChecker({
        allow: ['cd * && *'],
        deny: ['cd * && rm *'],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checkerWithComplex.check('cd /tmp && rm -rf *');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('should reject command substitution without matching patterns', () => {
      const result = checker.check('echo $(whoami)');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('Complex shell commands require explicit allow patterns');
    });

    test('should reject redirections without matching patterns', () => {
      const result = checker.check('ls > output.txt');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('Complex shell commands require explicit allow patterns');
    });
  });

  describe('Simple Command Processing', () => {
    test('should allow commands matching allow patterns', () => {
      const result = checker.check('test --version');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(false);
    });

    test('should reject commands matching deny patterns', () => {
      const result = checker.check('rm -rf /');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('should handle quoted arguments in pattern matching', () => {
      // This tests the quote handling fix
      const result = checker.check('echo "hello"');
      expect(result.allowed).toBe(true); // Should match echo:hello pattern
    });

    test('should reject commands with no matching allow pattern', () => {
      const result = checker.check('unknown-command');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('Command not in allow list');
    });
  });

  describe('Configuration', () => {
    test('should work with empty allow list (allows everything not denied)', () => {
      const permissiveChecker = new BashPermissionChecker({
        allow: [], // Empty allow list
        deny: ['rm:*'],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const allowed = permissiveChecker.check('any-command');
      expect(allowed.allowed).toBe(true);

      const denied = permissiveChecker.check('rm -rf');
      expect(denied.allowed).toBe(false);
    });

    test('should provide configuration summary', () => {
      const config = checker.getConfig();
      expect(config).toHaveProperty('allowPatterns');
      expect(config).toHaveProperty('denyPatterns');
      expect(config).toHaveProperty('totalPatterns');
      expect(typeof config.allowPatterns).toBe('number');
      expect(typeof config.denyPatterns).toBe('number');
    });
  });
});

describe('Bash Tool Integration with Simplified Architecture', () => {
  test('should create bash tool with working configuration', () => {
    const tool = bashTool({
      enableBash: true,
      bashConfig: {
        allow: ['ls:*', 'echo:*'],
        deny: ['rm:*'],
        timeout: 30000
      }
    });

    expect(tool.name).toBe('bash');
    expect(tool.description).toContain('Execute bash commands');
    expect(typeof tool.execute).toBe('function');
  });

  test('should allow complex commands when all components are in allow list in tool execution', async () => {
    const tool = bashTool({
      enableBash: true,
      bashConfig: {
        allow: ['ls', 'grep'],  // Both components allowed
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      }
    });

    // The permission check should pass and command should execute
    const result = await tool.execute({ command: 'ls | grep test' });
    // Should not contain permission denied since both components are allowed
    expect(result).not.toContain('Permission denied');
  });

  test('should reject complex commands when components are not allowed in tool execution', async () => {
    const tool = bashTool({
      enableBash: true,
      bashConfig: {
        allow: ['ls'],  // Only ls allowed, not grep
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      }
    });

    const result = await tool.execute({ command: 'ls | unknowncmd test' });
    expect(result).toContain('Permission denied');
  });

  test('should allow complex commands with matching patterns in tool execution', async () => {
    const tool = bashTool({
      enableBash: true,
      bashConfig: {
        allow: ['ls | grep *'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      }
    });

    // The permission check should pass - actual execution may fail in test env
    // but we're testing that permissions work correctly
    let permissionError = null;
    try {
      await tool.execute({ command: 'ls | grep test' });
    } catch (error) {
      if (error.message && error.message.includes('Permission denied')) {
        permissionError = error;
      }
    }

    expect(permissionError).toBeNull();
  });

  test('should handle simple commands in tool', async () => {
    const tool = bashTool({
      enableBash: true,
      bashConfig: { 
        allow: ['echo:*'], 
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true 
      }
    });

    // This should not throw during permission check
    // (actual execution might fail without proper environment, but permissions should pass)
    let permissionError = null;
    try {
      await tool.execute({ command: 'echo hello' });
    } catch (error) {
      if (error.message.includes('Permission denied')) {
        permissionError = error;
      }
      // Other execution errors (like missing environment) are expected in tests
    }

    expect(permissionError).toBeNull();
  });
});

describe('Component-based Complex Command Evaluation', () => {
  describe('Auto-allow when all components are allowed', () => {
    test('should allow piped commands when both components are in allow list', () => {
      const checker = new BashPermissionChecker({
        allow: ['ls', 'grep'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('ls | grep test');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
      expect(result.allowedByComponents).toBe(true);
    });

    test('should allow && chained commands when all components are allowed', () => {
      const checker = new BashPermissionChecker({
        allow: ['cd', 'git'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('cd /project && git status');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
      expect(result.allowedByComponents).toBe(true);
    });

    test('should allow || chained commands when all components are allowed', () => {
      const checker = new BashPermissionChecker({
        allow: ['make', 'echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('make || echo "build failed"');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
      expect(result.allowedByComponents).toBe(true);
    });

    test('should allow multi-operator chains when all components are allowed', () => {
      const checker = new BashPermissionChecker({
        allow: ['git', 'npm', 'echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('git pull && npm install && npm test || echo "failed"');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
      expect(result.allowedByComponents).toBe(true);
    });

    test('should allow gh piped to tail (original bug case)', () => {
      const checker = new BashPermissionChecker({
        allow: ['gh', 'tail'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('gh run view 21026947516 --repo TykTechnologies/tyk-analytics --job 60454942726 --log | tail -n 100');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
      expect(result.allowedByComponents).toBe(true);
    });
  });

  describe('Deny when any component is not allowed', () => {
    test('should deny piped command when first component is not allowed', () => {
      const checker = new BashPermissionChecker({
        allow: ['grep'],  // ls not allowed
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('ls | grep test');
      expect(result.allowed).toBe(false);
      expect(result.isComplex).toBe(true);
      expect(result.failedComponent).toBe('ls');
    });

    test('should deny piped command when second component is not allowed', () => {
      const checker = new BashPermissionChecker({
        allow: ['ls'],  // grep not allowed
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('ls | grep test');
      expect(result.allowed).toBe(false);
      expect(result.isComplex).toBe(true);
      expect(result.failedComponent).toBe('grep test');
    });

    test('should deny when any component matches deny list', () => {
      const checker = new BashPermissionChecker({
        allow: ['ls', 'rm'],
        deny: ['rm'],  // rm explicitly denied
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('ls -la && rm -rf /tmp/test');
      expect(result.allowed).toBe(false);
    });
  });

  describe('Explicit complex patterns take precedence', () => {
    test('should use explicit complex pattern over component evaluation', () => {
      const checker = new BashPermissionChecker({
        allow: ['cd * && git *'],  // Explicit complex pattern
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('cd /project && git status');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
      expect(result.matchedPattern).toBe('cd * && git *');  // Matched by explicit pattern
      expect(result.allowedByComponents).toBeUndefined();  // Not via component evaluation
    });

    test('should deny via complex deny pattern even if components would allow', () => {
      const checker = new BashPermissionChecker({
        allow: ['ls', 'grep'],  // Both components allowed
        deny: ['ls | grep secret*'],  // But this specific combination denied
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('ls | grep secret');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });
  });

  describe('Quote handling in component splitting', () => {
    test('should not treat operators inside quotes as complex', () => {
      const checker = new BashPermissionChecker({
        allow: ['echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      // The && is inside quotes, so this is a simple command
      const result = checker.check('echo "a && b"');
      // Correctly detected as simple (not complex) because && is inside quotes
      expect(result.isComplex).toBe(false);
      expect(result.allowed).toBe(true);
    });

    test('should handle complex commands with quoted arguments', () => {
      const checker = new BashPermissionChecker({
        allow: ['git', 'echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('git commit -m "feat: new feature" && echo "done"');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
      expect(result.allowedByComponents).toBe(true);
    });
  });

  describe('Security: Edge cases and attack vectors', () => {
    test('should reject if any component contains nested complex constructs', () => {
      const checker = new BashPermissionChecker({
        allow: ['echo', 'ls'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      // Nested command substitution inside a component
      const result = checker.check('echo hello && ls $(pwd)');
      expect(result.allowed).toBe(false);
      expect(result.isComplex).toBe(true);
    });

    test('should reject commands with redirection in any component', () => {
      const checker = new BashPermissionChecker({
        allow: ['ls', 'cat'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      // Redirection in second component
      const result = checker.check('ls && cat > /tmp/out');
      expect(result.allowed).toBe(false);
      expect(result.isComplex).toBe(true);
    });

    test('should reject commands with input redirection in any component', () => {
      const checker = new BashPermissionChecker({
        allow: ['ls', 'cat'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('ls && cat < /etc/passwd');
      expect(result.allowed).toBe(false);
      expect(result.isComplex).toBe(true);
    });

    test('should reject if component contains backtick command substitution', () => {
      const checker = new BashPermissionChecker({
        allow: ['echo', 'ls'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('echo hello && ls `pwd`');
      expect(result.allowed).toBe(false);
      expect(result.isComplex).toBe(true);
    });

    test('should reject if component has background execution', () => {
      const checker = new BashPermissionChecker({
        allow: ['sleep', 'echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('sleep 10 & && echo done');
      expect(result.allowed).toBe(false);
      expect(result.isComplex).toBe(true);
    });

    test('should deny when one component is in deny list even if other is allowed', () => {
      const checker = new BashPermissionChecker({
        allow: ['ls', 'echo', 'rm'],
        deny: ['rm:-rf:*', 'rm:-f:*'],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('ls && rm -rf /tmp/test');
      expect(result.allowed).toBe(false);
    });

    test('should handle empty components gracefully', () => {
      const checker = new BashPermissionChecker({
        allow: ['ls', 'echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      // Double pipe would create empty component
      const result = checker.check('ls || || echo');
      // Should still be complex and handle gracefully
      expect(result.isComplex).toBe(true);
    });

    test('should handle whitespace-heavy commands correctly', () => {
      const checker = new BashPermissionChecker({
        allow: ['ls', 'grep'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('   ls   -la   |    grep   test   ');
      expect(result.allowed).toBe(true);
      expect(result.allowedByComponents).toBe(true);
    });

    test('should reject unknown commands even in long chains', () => {
      const checker = new BashPermissionChecker({
        allow: ['ls', 'grep', 'cat', 'head'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      // One unknown command in the middle
      const result = checker.check('ls | grep test | malicious | head -10');
      expect(result.allowed).toBe(false);
      expect(result.failedComponent).toBe('malicious');
    });

    test('should handle very long command chains', () => {
      const checker = new BashPermissionChecker({
        allow: ['echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const longChain = Array(20).fill('echo test').join(' && ');
      const result = checker.check(longChain);
      expect(result.allowed).toBe(true);
      expect(result.allowedByComponents).toBe(true);
    });

    test('should handle mixed operators correctly', () => {
      const checker = new BashPermissionChecker({
        allow: ['cmd1', 'cmd2', 'cmd3', 'cmd4'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('cmd1 && cmd2 || cmd3 | cmd4');
      expect(result.allowed).toBe(true);
      expect(result.allowedByComponents).toBe(true);
      expect(result.components.length).toBe(4);
    });

    test('should not be fooled by operator-like strings in arguments', () => {
      const checker = new BashPermissionChecker({
        allow: ['echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      // These should be split properly even though args contain operator chars
      const result = checker.check('echo "test && fail" && echo "pass || next"');
      expect(result.allowed).toBe(true);
      expect(result.components.length).toBe(2);
    });

    test('should reject semicolon-separated commands (not supported for component eval)', () => {
      const checker = new BashPermissionChecker({
        allow: ['ls', 'pwd'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      // Semicolon is complex but not split for component eval
      const result = checker.check('ls ; pwd');
      expect(result.allowed).toBe(false);
      expect(result.isComplex).toBe(true);
    });

    test('should handle single quotes correctly in splitting', () => {
      const checker = new BashPermissionChecker({
        allow: ['echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check("echo 'test && test' && echo 'done'");
      expect(result.allowed).toBe(true);
      expect(result.components.length).toBe(2);
    });

    test('should reject if allow list is empty (strict mode)', () => {
      const checker = new BashPermissionChecker({
        allow: [],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('ls | grep test');
      // With empty allow list but no patterns, all components fail
      expect(result.allowed).toBe(false);
    });

    test('should properly escape special regex chars in command matching', () => {
      const checker = new BashPermissionChecker({
        allow: ['echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      // Commands with special regex characters
      const result = checker.check('echo "test[0]" && echo "test.*"');
      expect(result.allowed).toBe(true);
    });

    test('should deny dangerous commands even through component evaluation', () => {
      // Using default deny patterns
      const checker = new BashPermissionChecker({
        allow: ['ls', 'rm'],  // rm allowed in allow list
        // default deny includes rm:-rf:*
      });

      // Should be denied because rm -rf matches deny pattern
      const result = checker.check('ls && rm -rf /');
      expect(result.allowed).toBe(false);
    });

    test('should handle unbalanced quotes in one component', () => {
      const checker = new BashPermissionChecker({
        allow: ['echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      // Unbalanced quote should cause component parsing to fail
      const result = checker.check('echo "test && echo done');
      expect(result.allowed).toBe(false);
    });
  });

  describe('Security: Denial of service prevention', () => {
    test('should handle pathologically nested quotes', () => {
      const checker = new BashPermissionChecker({
        allow: ['echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = checker.check('echo "\'\\"test\\"\'\"');
      // Should handle without hanging, result doesn't matter as much as not crashing
      expect(result).toBeDefined();
    });

    test('should handle very long single component', () => {
      const checker = new BashPermissionChecker({
        allow: ['echo'],
        deny: [],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const longArg = 'a'.repeat(10000);
      const result = checker.check(`echo ${longArg} && echo done`);
      expect(result.allowed).toBe(true);
    });
  });
});

describe('Architecture Alignment Tests', () => {
  test('should ensure parser and executor handle same command format', () => {
    const testCommands = [
      'ls -la',
      'git status',
      'echo "hello world"',
      'npm test',
      'docker ps',
      'make clean'
    ];

    for (const command of testCommands) {
      const parserResult = parseCommandForExecution(command);
      const permissionResult = parseCommand(command);

      // Both should succeed for simple commands
      expect(parserResult).not.toBeNull();
      expect(permissionResult.error).toBeNull();

      // Both should parse to same basic structure
      expect(parserResult[0]).toBe(permissionResult.command);
      expect(parserResult.slice(1)).toEqual(permissionResult.args);
    }
  });

  test('should ensure complex commands are detected consistently', () => {
    const complexCommands = [
      'ls | grep test',
      'make && make test',
      'echo $(date)',
      'ls > output.txt',
      'cmd1 ; cmd2',
      'background-task &'
    ];

    for (const command of complexCommands) {
      const parserResult = parseCommandForExecution(command);
      const isComplex = isComplexCommand(command);

      // Parser should reject complex commands (returns null)
      expect(parserResult).toBeNull();
      expect(isComplex).toBe(true);
    }
  });

  test('should allow complex commands via component evaluation when all components match wildcard', () => {
    // With wildcard allow pattern, component evaluation will allow piped/chained commands
    const checkerWithWildcard = new BashPermissionChecker({
      allow: ['*'], // Wildcard allows any simple command
      deny: [],
      disableDefaultAllow: true,
      disableDefaultDeny: true
    });

    // Commands where all components can be parsed as simple commands
    const result1 = checkerWithWildcard.check('ls | grep test');
    expect(result1.allowed).toBe(true);
    expect(result1.allowedByComponents).toBe(true);

    const result2 = checkerWithWildcard.check('make && make test');
    expect(result2.allowed).toBe(true);
    expect(result2.allowedByComponents).toBe(true);
  });

  test('should reject complex commands with constructs that cannot be split into simple components', () => {
    const checkerWithWildcard = new BashPermissionChecker({
      allow: ['*'],
      deny: [],
      disableDefaultAllow: true,
      disableDefaultDeny: true
    });

    // Command substitution cannot be split into simple components
    const result1 = checkerWithWildcard.check('echo $(date)');
    expect(result1.allowed).toBe(false);

    // Redirection cannot be split into simple components
    const result2 = checkerWithWildcard.check('ls > output.txt');
    expect(result2.allowed).toBe(false);
  });

  test('should allow complex commands when matching patterns are configured', () => {
    const checkerWithPatterns = new BashPermissionChecker({
      allow: [
        'ls | grep *',
        'make && make test',
        'echo $(date)',
        'ls > *',
        'cmd1 ; cmd2',
        'background-task &'
      ],
      deny: [],
      disableDefaultAllow: true,
      disableDefaultDeny: true
    });

    const complexCommands = [
      'ls | grep test',
      'make && make test',
      'echo $(date)',
      'ls > output.txt',
      'cmd1 ; cmd2',
      'background-task &'
    ];

    for (const command of complexCommands) {
      const result = checkerWithPatterns.check(command);
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
    }
  });
});