/**
 * Tests for multi-line bash command support
 *
 * Multi-line commands (newline-separated) are a common pattern in AI agent usage,
 * e.g. LLMs generating scripts with `set -e` followed by sequential commands.
 * Newlines outside quotes act as command separators (like `;` in shell),
 * but newlines inside quoted strings are part of the string value.
 *
 * @module tests/unit/bash-multiline-commands
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { parseSimpleCommand, isComplexCommand, parseCommandForExecution } from '../../src/agent/bashCommandUtils.js';
import { BashPermissionChecker } from '../../src/agent/bashPermissions.js';
import { parseXmlToolCall } from '../../src/tools/common.js';

// Mock the 'ai' package since it may not be available in test environment
jest.mock('ai', () => ({
  tool: jest.fn((config) => ({
    name: config.name,
    description: config.description,
    inputSchema: config.inputSchema,
    execute: config.execute
  }))
}));

describe('Multi-line command detection (isComplexCommand)', () => {
  test('should detect simple two-line command as complex', () => {
    const cmd = 'cd /project\ngit status';
    expect(isComplexCommand(cmd)).toBe(true);
  });

  test('should detect set -e followed by commands as complex', () => {
    const cmd = 'set -e\ncd /project\ngit status';
    expect(isComplexCommand(cmd)).toBe(true);
  });

  test('should detect multi-line git workflow as complex', () => {
    const cmd = [
      'set -e',
      'cd tyk-docs',
      'git checkout -b feat/portal-webhooks-docs',
      'git add portal/customization/webhooks.mdx',
      'git commit -m "feat(docs): Add documentation for Developer Portal Webhooks"',
      'git push origin feat/portal-webhooks-docs',
    ].join('\n');
    expect(isComplexCommand(cmd)).toBe(true);
  });

  test('should NOT detect single-line command as complex due to newlines', () => {
    expect(isComplexCommand('git status')).toBe(false);
    expect(isComplexCommand('echo hello')).toBe(false);
  });

  test('should NOT detect newlines inside double-quoted strings as command separators', () => {
    // The newline here is inside quotes - it's part of the string value
    const cmd = 'echo "line1\nline2"';
    expect(isComplexCommand(cmd)).toBe(false);
  });

  test('should NOT detect newlines inside single-quoted strings as command separators', () => {
    const cmd = "echo 'line1\nline2'";
    expect(isComplexCommand(cmd)).toBe(false);
  });

  test('should detect newlines outside quotes even when some are inside quotes', () => {
    // Two commands: echo with multi-line string, then pwd
    // The newline between echo and pwd is outside quotes = separator
    const cmd = 'echo "hello\nworld"\npwd';
    expect(isComplexCommand(cmd)).toBe(true);
  });
});

describe('Multi-line command parsing (parseSimpleCommand)', () => {
  test('should mark two-line command as complex', () => {
    const result = parseSimpleCommand('ls -la\npwd');
    expect(result.isComplex).toBe(true);
    expect(result.success).toBe(false);
  });

  test('should mark set -e script as complex', () => {
    const result = parseSimpleCommand('set -e\ncd /tmp\nls');
    expect(result.isComplex).toBe(true);
    expect(result.success).toBe(false);
  });

  test('should NOT mark single command with newline in quotes as complex', () => {
    const result = parseSimpleCommand('git commit -m "first line\nsecond line"');
    expect(result.isComplex).toBe(false);
    expect(result.success).toBe(true);
    expect(result.command).toBe('git');
    expect(result.args).toContain('commit');
  });

  test('should handle newline in double-quoted arg followed by more args', () => {
    const result = parseSimpleCommand('echo "multi\nline" --flag');
    expect(result.isComplex).toBe(false);
    expect(result.success).toBe(true);
    expect(result.command).toBe('echo');
    expect(result.args).toEqual(['multi\nline', '--flag']);
  });

  test('should handle newline in single-quoted arg', () => {
    const result = parseSimpleCommand("echo 'multi\nline'");
    expect(result.isComplex).toBe(false);
    expect(result.success).toBe(true);
    expect(result.args).toEqual(['multi\nline']);
  });
});

describe('Multi-line commands with parseCommandForExecution', () => {
  test('should return null for multi-line commands (they are complex)', () => {
    const result = parseCommandForExecution('cd /project\ngit status');
    expect(result).toBeNull();
  });

  test('should return args for single command with newline in quoted string', () => {
    const result = parseCommandForExecution('echo "hello\nworld"');
    expect(result).not.toBeNull();
    expect(result[0]).toBe('echo');
    expect(result[1]).toBe('hello\nworld');
  });
});

describe('Multi-line permission checking', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({
      allow: ['set', 'cd', 'git', 'echo', 'ls', 'gh', 'pwd'],
      deny: [],
      disableDefaultAllow: true,
      disableDefaultDeny: true,
      debug: false
    });
  });

  test('should allow multi-line command when all components are allowed', () => {
    const cmd = 'set -e\ncd /project\ngit status';
    const result = checker.check(cmd);
    expect(result.allowed).toBe(true);
    expect(result.isComplex).toBe(true);
  });

  test('should deny multi-line command when one component is not allowed', () => {
    const cmd = 'set -e\ncd /project\nunknown-cmd';
    const result = checker.check(cmd);
    expect(result.allowed).toBe(false);
    expect(result.isComplex).toBe(true);
  });

  test('should allow full git workflow multi-line command', () => {
    const cmd = [
      'set -e',
      'cd tyk-docs',
      'git checkout -b feat/portal-webhooks-docs',
      'git add portal/customization/webhooks.mdx',
      'git commit -m "feat(docs): Add documentation"',
      'git push origin feat/portal-webhooks-docs',
    ].join('\n');
    const result = checker.check(cmd);
    expect(result.allowed).toBe(true);
    expect(result.isComplex).toBe(true);
  });

  test('should allow gh pr create with multi-line --body', () => {
    const cmd = [
      'gh pr create --title "Add webhooks docs" --body "This PR adds documentation.',
      '',
      'Key topics:',
      '- Webhooks intro',
      '- Configuration',
      '- Security"',
    ].join('\n');
    // This is a single command with newlines inside quotes
    const result = checker.check(cmd);
    expect(result.allowed).toBe(true);
    // Since the newlines are inside quotes, this should be simple, not complex
    expect(result.isComplex).toBe(false);
  });

  test('should allow real-world multi-line script with gh pr create', () => {
    const cmd = [
      'set -e',
      'cd tyk-docs',
      'git checkout -b feat/portal-webhooks-docs',
      'git add portal/customization/webhooks.mdx',
      'git commit -m "feat(docs): Add documentation for Developer Portal Webhooks"',
      'git push origin feat/portal-webhooks-docs',
      'gh pr create --title "feat(docs): Add documentation for Developer Portal Webhooks" --body "This PR adds a new documentation page for using Webhooks in the Tyk Developer Portal.',
      '',
      'Key topics covered:',
      '- Introduction to Webhooks',
      '- Configuration using the Tyk Dashboard API',
      '- Authentication and Security',
      '- Payload Structure',
      '- Supported Webhook Events',
      '- End-to-End Flow',
      '- Examples and Use Cases',
      '- Error Handling and Retries',
      '- Best Practices',
      '- Testing and Debugging"',
    ].join('\n');
    const result = checker.check(cmd);
    expect(result.allowed).toBe(true);
    expect(result.isComplex).toBe(true);
  });

  test('should deny multi-line script if one line has denied command', () => {
    const checkerWithDeny = new BashPermissionChecker({
      allow: ['set', 'cd', 'git', 'echo', 'rm'],
      deny: ['rm:-rf'],
      disableDefaultAllow: true,
      disableDefaultDeny: true,
      debug: false
    });

    const cmd = 'set -e\ncd /project\nrm -rf /important';
    const result = checkerWithDeny.check(cmd);
    expect(result.allowed).toBe(false);
  });
});

describe('Multi-line commands with mixed operators', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({
      allow: ['set', 'cd', 'git', 'echo', 'ls', 'pwd'],
      deny: [],
      disableDefaultAllow: true,
      disableDefaultDeny: true,
      debug: false
    });
  });

  test('should handle newlines mixed with && operators', () => {
    // Some lines use && and some use newlines
    const cmd = 'set -e\ncd /project && git status\necho done';
    const result = checker.check(cmd);
    expect(result.allowed).toBe(true);
    expect(result.isComplex).toBe(true);
  });

  test('should handle newlines mixed with || operators', () => {
    const cmd = 'cd /project || echo "fallback"\nls';
    const result = checker.check(cmd);
    expect(result.allowed).toBe(true);
    expect(result.isComplex).toBe(true);
  });

  test('should handle newlines mixed with pipes', () => {
    const cmd = 'ls -la\ngit log --oneline | head -10';
    const result = checker.check(cmd);
    // head is not in our allow list, so let's just check it's detected as complex
    expect(result.isComplex).toBe(true);
  });
});

describe('XML parsing of multi-line bash commands', () => {
  test('should preserve multi-line content in bash command parameter', () => {
    const aiResponse = `<bash>
<command>
set -e
cd tyk-docs
git status
</command>
</bash>`;

    const result = parseXmlToolCall(aiResponse);
    expect(result.toolName).toBe('bash');
    // The command should preserve internal newlines
    expect(result.params.command).toContain('\n');
    expect(result.params.command).toContain('set -e');
    expect(result.params.command).toContain('cd tyk-docs');
    expect(result.params.command).toContain('git status');
  });

  test('should preserve newlines inside quoted strings in bash command', () => {
    const aiResponse = `<bash>
<command>gh pr create --title "Add docs" --body "Line 1
Line 2
Line 3"</command>
</bash>`;

    const result = parseXmlToolCall(aiResponse);
    expect(result.toolName).toBe('bash');
    expect(result.params.command).toContain('Line 1\nLine 2\nLine 3');
  });

  test('should parse full multi-line script from XML', () => {
    const aiResponse = `<bash>
<command>
set -e
cd tyk-docs
git checkout -b feat/webhooks
git add docs/webhooks.mdx
git commit -m "feat: add webhooks docs"
git push origin feat/webhooks
gh pr create --title "Add webhooks docs" --body "Summary of changes.

Details:
- Added webhooks page
- Added examples"
</command>
</bash>`;

    const result = parseXmlToolCall(aiResponse);
    expect(result.toolName).toBe('bash');
    const cmd = result.params.command;
    expect(cmd).toContain('set -e');
    expect(cmd).toContain('git checkout -b feat/webhooks');
    expect(cmd).toContain('gh pr create');
    // The body should have preserved newlines
    expect(cmd).toContain('Summary of changes.');
    expect(cmd).toContain('- Added webhooks page');
  });
});

describe('Edge cases for multi-line commands', () => {
  test('should handle Windows-style line endings (CRLF)', () => {
    const cmd = 'cd /project\r\ngit status';
    expect(isComplexCommand(cmd)).toBe(true);
  });

  test('should handle multiple consecutive newlines', () => {
    const cmd = 'cd /project\n\n\ngit status';
    expect(isComplexCommand(cmd)).toBe(true);
  });

  test('should handle trailing newline on single command', () => {
    // A single command with trailing newline should NOT be complex
    const cmd = 'git status\n';
    // After trim, this is just "git status"
    expect(isComplexCommand(cmd)).toBe(false);
  });

  test('should handle leading newline on single command', () => {
    const cmd = '\ngit status';
    // After trim, this is just "git status"
    expect(isComplexCommand(cmd)).toBe(false);
  });

  test('should handle heredoc-style pattern in quoted string', () => {
    // git commit with heredoc-like multi-line message
    const cmd = 'git commit -m "feat: add feature\n\nThis is the body of the commit.\nIt spans multiple lines."';
    expect(isComplexCommand(cmd)).toBe(false);

    const result = parseSimpleCommand(cmd);
    expect(result.success).toBe(true);
    expect(result.command).toBe('git');
  });

  test('should handle command with only whitespace between newlines', () => {
    const cmd = 'echo hello\n   \necho world';
    expect(isComplexCommand(cmd)).toBe(true);
  });

  test('should handle tab characters in multi-line commands', () => {
    const cmd = 'set -e\n\tcd /project\n\tgit status';
    expect(isComplexCommand(cmd)).toBe(true);
  });

  test('newline as argument separator within single-line context should not happen', () => {
    // A single command where newline appears between args (not in quotes)
    // should be treated as two commands, not one command with weird args
    const cmd = 'echo\nhello';
    expect(isComplexCommand(cmd)).toBe(true);
  });
});

describe('Execution routing for multi-line commands', () => {
  test('parseCommandForExecution returns null for multi-line (routes to sh -c)', () => {
    const cmd = 'set -e\ncd /project\ngit status';
    // Multi-line is complex, so parseCommandForExecution should return null
    // The executor should then use sh -c for these
    expect(parseCommandForExecution(cmd)).toBeNull();
  });

  test('parseCommandForExecution works for single command with newline in quotes', () => {
    const cmd = 'echo "first\nsecond"';
    const result = parseCommandForExecution(cmd);
    expect(result).not.toBeNull();
    expect(result).toEqual(['echo', 'first\nsecond']);
  });
});

describe('Real-world multi-line command scenarios', () => {
  test('npm build and test script', () => {
    const cmd = [
      'set -e',
      'cd /project',
      'npm install',
      'npm run build',
      'npm test',
    ].join('\n');
    expect(isComplexCommand(cmd)).toBe(true);

    const result = parseSimpleCommand(cmd);
    expect(result.isComplex).toBe(true);
  });

  test('docker build and push', () => {
    const cmd = [
      'set -e',
      'docker build -t myapp:latest .',
      'docker push myapp:latest',
    ].join('\n');
    expect(isComplexCommand(cmd)).toBe(true);
  });

  test('mkdir and file creation', () => {
    const cmd = [
      'mkdir -p /tmp/test',
      'cd /tmp/test',
      'ls -la',
    ].join('\n');
    expect(isComplexCommand(cmd)).toBe(true);
  });

  test('environment variable export and use', () => {
    const cmd = [
      'export FOO=bar',
      'echo $FOO',
    ].join('\n');
    expect(isComplexCommand(cmd)).toBe(true);
  });

  test('curl with multi-line JSON body in quotes', () => {
    const cmd = `curl -X POST http://localhost:3000/api -d '{"name": "test",
"value": "hello",
"nested": {
  "key": "val"
}}'`;
    // The newlines are all inside single quotes - this is a single command
    expect(isComplexCommand(cmd)).toBe(false);
  });

  test('complex gh pr create from real usage', () => {
    const cmd = `gh pr create --title "feat(docs): Add Portal Webhooks" --body "This PR adds a new documentation page.

## Key topics covered:
- Introduction to Webhooks
- Configuration using the Tyk Dashboard API
- Authentication and Security
- Payload Structure

## Testing
- Verified locally with mdx preview"`;
    // All newlines are inside the --body quotes
    expect(isComplexCommand(cmd)).toBe(false);
  });
});
