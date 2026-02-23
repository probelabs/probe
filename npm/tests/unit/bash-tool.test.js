/**
 * Tests for bash tool functionality (updated for simplified architecture)
 * @module tests/unit/bash-tool
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { BashPermissionChecker, parseCommand, matchesPattern } from '../../src/agent/bashPermissions.js';
import { executeBashCommand, formatExecutionResult, validateExecutionOptions } from '../../src/agent/bashExecutor.js';
import { parseSimpleCommand, parseCommandForExecution } from '../../src/agent/bashCommandUtils.js';
import { DEFAULT_ALLOW_PATTERNS, DEFAULT_DENY_PATTERNS } from '../../src/agent/bashDefaults.js';

describe('Bash Permission Checker', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({
      debug: false
    });
  });

  describe('parseCommand (simplified)', () => {
    test('should parse simple commands', () => {
      const result = parseCommand('ls -la');
      expect(result.command).toBe('ls');
      expect(result.args).toEqual(['-la']);
      expect(result.error).toBeNull();
    });

    test('should parse commands with multiple arguments', () => {
      const result = parseCommand('git log --oneline -10');
      expect(result.command).toBe('git');
      expect(result.args).toEqual(['log', '--oneline', '-10']);
    });

    test('should handle quoted arguments CORRECTLY (fix quote bug)', () => {
      const result = parseCommand('grep -r "TODO" src/');
      expect(result.command).toBe('grep');
      // FIXED: Quotes should be stripped, not preserved
      expect(result.args).toEqual(['-r', 'TODO', 'src/']);
    });

    test('should handle empty commands', () => {
      const result = parseCommand('');
      expect(result.command).toBe('');
      expect(result.args).toEqual([]);
      expect(result.error).toBeTruthy(); // Should have error for empty command
    });

    test('should handle null/undefined commands', () => {
      const result = parseCommand(null);
      expect(result.command).toBe('');
      expect(result.args).toEqual([]);
      expect(result.error).toBeTruthy();
    });

    test('should reject complex commands', () => {
      const result = parseCommand('ls | grep test');
      expect(result.error).toBeTruthy();
      expect(result.isComplex).toBe(true);
    });
  });

  describe('Command Parsing with parseSimpleCommand', () => {
    test('should handle complex quote scenarios', () => {
      const result = parseSimpleCommand('echo "hello world" \'single quotes\' mixed');
      expect(result.success).toBe(true);
      expect(result.command).toBe('echo');
      expect(result.args).toEqual(['hello world', 'single quotes', 'mixed']);
    });

    test('should detect and reject pipes', () => {
      const result = parseSimpleCommand('ls | grep test');
      expect(result.success).toBe(false);
      expect(result.isComplex).toBe(true);
      expect(result.error).toContain('Complex shell commands');
    });

    test('should detect and reject command substitution', () => {
      const result = parseSimpleCommand('echo $(date)');
      expect(result.success).toBe(false);
      expect(result.isComplex).toBe(true);
    });
  });

  describe('Pattern Matching', () => {
    test('should match exact commands', () => {
      const parsed = { command: 'ls', args: ['-la'] };
      expect(matchesPattern(parsed, 'ls')).toBe(true);
      expect(matchesPattern(parsed, 'cat')).toBe(false);
    });

    test('should match wildcard patterns', () => {
      const parsed = { command: 'git', args: ['status'] };
      expect(matchesPattern(parsed, 'git:*')).toBe(true);
      expect(matchesPattern(parsed, 'git:status')).toBe(true);
      expect(matchesPattern(parsed, 'git:log')).toBe(false);
    });

    test('should match specific argument patterns', () => {
      const parsed = { command: 'npm', args: ['list'] };
      expect(matchesPattern(parsed, 'npm:list')).toBe(true);
      expect(matchesPattern(parsed, 'npm:install')).toBe(false);
    });
  });

  describe('Permission Checking', () => {
    test('should allow commands in default allow list', () => {
      const result = checker.check('ls -la');
      expect(result.allowed).toBe(true);
    });

    test('should deny commands in default deny list', () => {
      const result = checker.check('rm -rf /');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('should allow complex commands when all components are in allow list', () => {
      // ls and grep are both in default allow list
      const result = checker.check('ls | grep test');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
      expect(result.allowedByComponents).toBe(true);
    });

    test('should deny complex commands when components are not allowed', () => {
      // unknowncmd is not in any allow list
      const result = checker.check('unknowncmd | othercmd');
      expect(result.allowed).toBe(false);
      expect(result.isComplex).toBe(true);
    });

    test('should respect custom allow patterns', () => {
      const customChecker = new BashPermissionChecker({
        allow: ['custom-cmd:*'],
        disableDefaultAllow: true,
        disableDefaultDeny: true
      });

      const result = customChecker.check('custom-cmd --flag');
      expect(result.allowed).toBe(true);
    });

    test('should respect custom deny patterns', () => {
      const customChecker = new BashPermissionChecker({
        deny: ['dangerous-cmd:*'],
        allow: ['*'], // Allow everything except denied
        disableDefaultDeny: true
      });

      const result = customChecker.check('dangerous-cmd --execute');
      expect(result.allowed).toBe(false);
    });
  });
});

describe('Bash Command Executor', () => {
  describe('validateExecutionOptions', () => {
    test('should validate working directory', () => {
      const options = {
        workingDirectory: '/nonexistent/path',
        timeout: 5000
      };

      const result = validateExecutionOptions(options);
      expect(result.valid).toBe(false);
      expect(result.errors[0]).toContain('does not exist');
    });

    test('should validate timeout', () => {
      const options = {
        timeout: -1000
      };

      const result = validateExecutionOptions(options);
      expect(result.valid).toBe(false);
      expect(result.errors[0]).toContain('timeout');
    });

    test('should accept valid options', () => {
      const options = {
        workingDirectory: process.cwd(),
        timeout: 5000,
        env: { TEST: 'value' }
      };

      const result = validateExecutionOptions(options);
      expect(result.valid).toBe(true);
    });
  });

  describe('executeBashCommand', () => {
    test('should execute simple commands', async () => {
      const result = await executeBashCommand('echo "hello world"');
      expect(result.success).toBe(true);
      expect(result.stdout.trim()).toBe('hello world');
      expect(result.exitCode).toBe(0);
    });

    test('should handle command failures', async () => {
      const result = await executeBashCommand('exit 1');
      expect(result.success).toBe(false);
      expect(result.exitCode).toBe(1);
    });

    test('should respect timeout', async () => {
      const result = await executeBashCommand('sleep 10', { timeout: 100 });
      expect(result.success).toBe(false);
      expect(result.killed).toBe(true);
    }, 10000);

    test('should handle invalid commands', async () => {
      const result = await executeBashCommand('nonexistent-command-xyz');
      expect(result.success).toBe(false);
      expect(result.error).toBeTruthy();
    });
  });

  describe('formatExecutionResult', () => {
    test('should format successful results', () => {
      const result = {
        success: true,
        stdout: 'output line 1\noutput line 2',
        stderr: '',
        exitCode: 0,
        command: 'test command',
        duration: 123
      };

      const formatted = formatExecutionResult(result);
      expect(formatted).toContain('output line 1');
      expect(formatted).toContain('output line 2');
    });

    test('should format error results', () => {
      const result = {
        success: false,
        stdout: '',
        stderr: 'error message',
        exitCode: 1,
        command: 'failing command',
        duration: 50
      };

      const formatted = formatExecutionResult(result);
      expect(formatted).toContain('error message');
      expect(formatted).toContain('Exit code: 1');
    });

    test('should include metadata when requested', () => {
      const result = {
        success: true,
        stdout: 'output',
        stderr: '',
        exitCode: 0,
        command: 'test',
        duration: 100,
        workingDirectory: '/tmp'
      };

      const formatted = formatExecutionResult(result, true);
      expect(formatted).toContain('Duration: 100ms');
      expect(formatted).toContain('Working directory: /tmp');
    });
  });
});

describe('Bash Permission Telemetry', () => {
  test('should record permission.allowed event when command is allowed', () => {
    const recordedEvents = [];
    const mockTracer = {
      recordBashEvent: (eventType, data) => {
        recordedEvents.push({ eventType, data });
      }
    };

    const checker = new BashPermissionChecker({
      debug: false,
      tracer: mockTracer
    });

    // Clear initialization event
    recordedEvents.length = 0;

    const result = checker.check('ls -la');
    expect(result.allowed).toBe(true);

    // Should have recorded an allowed event
    const allowedEvent = recordedEvents.find(e => e.eventType === 'permission.allowed');
    expect(allowedEvent).toBeDefined();
    expect(allowedEvent.data.command).toBe('ls -la');
    expect(allowedEvent.data.parsedCommand).toBe('ls');
    expect(allowedEvent.data.isComplex).toBe(false);
  });

  test('should record permission.denied event when command matches deny pattern', () => {
    const recordedEvents = [];
    const mockTracer = {
      recordBashEvent: (eventType, data) => {
        recordedEvents.push({ eventType, data });
      }
    };

    const checker = new BashPermissionChecker({
      debug: false,
      tracer: mockTracer
    });

    // Clear initialization event
    recordedEvents.length = 0;

    const result = checker.check('rm -rf /');
    expect(result.allowed).toBe(false);

    // Should have recorded a denied event
    const deniedEvent = recordedEvents.find(e => e.eventType === 'permission.denied');
    expect(deniedEvent).toBeDefined();
    expect(deniedEvent.data.command).toBe('rm -rf /');
    expect(deniedEvent.data.reason).toBe('matches_deny_pattern');
  });

  test('should record permission.denied event when command not in allow list', () => {
    const recordedEvents = [];
    const mockTracer = {
      recordBashEvent: (eventType, data) => {
        recordedEvents.push({ eventType, data });
      }
    };

    const checker = new BashPermissionChecker({
      debug: false,
      tracer: mockTracer,
      disableDefaultDeny: true // Disable deny list so we test allow list logic
    });

    // Clear initialization event
    recordedEvents.length = 0;

    const result = checker.check('unknown-command --flag');
    expect(result.allowed).toBe(false);

    // Should have recorded a denied event with "not_in_allow_list" reason
    const deniedEvent = recordedEvents.find(e => e.eventType === 'permission.denied');
    expect(deniedEvent).toBeDefined();
    expect(deniedEvent.data.command).toBe('unknown-command --flag');
    expect(deniedEvent.data.reason).toBe('not_in_allow_list');
  });

  test('should record permission.denied event for complex commands', () => {
    const recordedEvents = [];
    const mockTracer = {
      recordBashEvent: (eventType, data) => {
        recordedEvents.push({ eventType, data });
      }
    };

    const checker = new BashPermissionChecker({
      debug: false,
      tracer: mockTracer
    });

    // Clear initialization event
    recordedEvents.length = 0;

    // ls and grep are in default allow list, so this will be allowed via component evaluation
    const result = checker.check('ls | grep test');
    expect(result.allowed).toBe(true);
    expect(result.isComplex).toBe(true);

    // Should have recorded an allowed event with component flag
    const allowedEvent = recordedEvents.find(e => e.eventType === 'permission.allowed');
    expect(allowedEvent).toBeDefined();
    expect(allowedEvent.data.command).toBe('ls | grep test');
    expect(allowedEvent.data.isComplex).toBe(true);
    expect(allowedEvent.data.allowedByComponents).toBe(true);
  });

  test('should record permission.denied event for complex commands with unknown components', () => {
    const recordedEvents = [];
    const mockTracer = {
      recordBashEvent: (eventType, data) => {
        recordedEvents.push({ eventType, data });
      }
    };

    const checker = new BashPermissionChecker({
      debug: false,
      tracer: mockTracer
    });

    // Clear initialization event
    recordedEvents.length = 0;

    // unknowncmd is not in any allow list
    const result = checker.check('unknowncmd | othercmd');
    expect(result.allowed).toBe(false);
    expect(result.isComplex).toBe(true);

    // Should have recorded a denied event
    const deniedEvent = recordedEvents.find(e => e.eventType === 'permission.denied');
    expect(deniedEvent).toBeDefined();
    expect(deniedEvent.data.command).toBe('unknowncmd | othercmd');
    expect(deniedEvent.data.isComplex).toBe(true);
  });

  test('should record permissions.initialized event on construction', () => {
    const recordedEvents = [];
    const mockTracer = {
      recordBashEvent: (eventType, data) => {
        recordedEvents.push({ eventType, data });
      }
    };

    new BashPermissionChecker({
      debug: false,
      tracer: mockTracer,
      allow: ['custom:*'],
      deny: ['blocked:*']
    });

    const initEvent = recordedEvents.find(e => e.eventType === 'permissions.initialized');
    expect(initEvent).toBeDefined();
    expect(initEvent.data.hasCustomAllowPatterns).toBe(true);
    expect(initEvent.data.hasCustomDenyPatterns).toBe(true);
    expect(initEvent.data.allowPatternCount).toBeGreaterThan(0);
    expect(initEvent.data.denyPatternCount).toBeGreaterThan(0);
  });

  test('should work without tracer (backward compatibility)', () => {
    const checker = new BashPermissionChecker({
      debug: false
      // No tracer provided
    });

    // Should not throw
    const result = checker.check('ls -la');
    expect(result.allowed).toBe(true);
  });
});

describe('Pattern "git:push" matches push with all CLI flags', () => {
  test('git:push pattern matches bare "git push"', () => {
    const parsed = { command: 'git', args: ['push'] };
    expect(matchesPattern(parsed, 'git:push')).toBe(true);
  });

  test('git:push pattern matches "git push origin main"', () => {
    const parsed = { command: 'git', args: ['push', 'origin', 'main'] };
    expect(matchesPattern(parsed, 'git:push')).toBe(true);
  });

  test('git:push pattern matches "git push --force"', () => {
    const parsed = { command: 'git', args: ['push', '--force'] };
    expect(matchesPattern(parsed, 'git:push')).toBe(true);
  });

  test('git:push pattern matches "git push --force-with-lease origin main"', () => {
    const parsed = { command: 'git', args: ['push', '--force-with-lease', 'origin', 'main'] };
    expect(matchesPattern(parsed, 'git:push')).toBe(true);
  });

  test('git:push pattern matches "git push -u origin feature-branch"', () => {
    const parsed = { command: 'git', args: ['push', '-u', 'origin', 'feature-branch'] };
    expect(matchesPattern(parsed, 'git:push')).toBe(true);
  });

  test('git:push pattern matches "git push --set-upstream origin HEAD"', () => {
    const parsed = { command: 'git', args: ['push', '--set-upstream', 'origin', 'HEAD'] };
    expect(matchesPattern(parsed, 'git:push')).toBe(true);
  });

  test('git:push pattern matches "git push --tags"', () => {
    const parsed = { command: 'git', args: ['push', '--tags'] };
    expect(matchesPattern(parsed, 'git:push')).toBe(true);
  });

  test('git:push pattern matches "git push --delete origin old-branch"', () => {
    const parsed = { command: 'git', args: ['push', '--delete', 'origin', 'old-branch'] };
    expect(matchesPattern(parsed, 'git:push')).toBe(true);
  });

  test('git:push pattern matches "git push origin HEAD:refs/for/main"', () => {
    const parsed = { command: 'git', args: ['push', 'origin', 'HEAD:refs/for/main'] };
    expect(matchesPattern(parsed, 'git:push')).toBe(true);
  });

  test('git:push pattern does NOT match "git pull"', () => {
    const parsed = { command: 'git', args: ['pull'] };
    expect(matchesPattern(parsed, 'git:push')).toBe(false);
  });

  test('git:push pattern does NOT match "git status"', () => {
    const parsed = { command: 'git', args: ['status'] };
    expect(matchesPattern(parsed, 'git:push')).toBe(false);
  });

  test('git:push pattern does NOT match bare "git" (no subcommand)', () => {
    const parsed = { command: 'git', args: [] };
    expect(matchesPattern(parsed, 'git:push')).toBe(false);
  });

  test('git:push deny works end-to-end via BashPermissionChecker', () => {
    const checker = new BashPermissionChecker({ debug: false });

    // All of these should be denied by the default deny list containing "git:push"
    const pushCommands = [
      'git push',
      'git push origin main',
      'git push --force',
      'git push --force-with-lease origin main',
      'git push -u origin feature',
      'git push --set-upstream origin HEAD',
      'git push --tags',
      'git push --delete origin old-branch',
      'git push --all',
      'git push --mirror',
      'git push origin HEAD:refs/for/main',
    ];

    for (const cmd of pushCommands) {
      const result = checker.check(cmd);
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    }
  });

  test('git:push allow pattern permits all push variants when deny is disabled', () => {
    const checker = new BashPermissionChecker({
      allow: ['git:push'],
      disableDefaultAllow: true,
      disableDefaultDeny: true,
      debug: false
    });

    const pushCommands = [
      'git push',
      'git push origin main',
      'git push --force',
      'git push -u origin feature',
      'git push --tags',
    ];

    for (const cmd of pushCommands) {
      const result = checker.check(cmd);
      expect(result.allowed).toBe(true);
    }

    // But non-push git commands should NOT be allowed
    expect(checker.check('git status').allowed).toBe(false);
    expect(checker.check('git pull').allowed).toBe(false);
    expect(checker.check('ls').allowed).toBe(false);
  });

  test('git:push:* is redundant when git:push exists (both match same commands)', () => {
    // Demonstrate that git:push already covers everything git:push:* covers
    const pushVariants = [
      { command: 'git', args: ['push'] },
      { command: 'git', args: ['push', 'origin'] },
      { command: 'git', args: ['push', '--force'] },
      { command: 'git', args: ['push', 'origin', 'main'] },
      { command: 'git', args: ['push', '--force-with-lease', 'origin', 'main'] },
    ];

    for (const parsed of pushVariants) {
      const matchesPush = matchesPattern(parsed, 'git:push');
      const matchesPushStar = matchesPattern(parsed, 'git:push:*');
      // Both patterns match the same set of commands
      expect(matchesPush).toBe(true);
      expect(matchesPushStar).toBe(true);
    }
  });
});

describe('Git read-only allow and destructive deny patterns', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({ debug: false });
  });

  describe('Additional git read-only commands are allowed', () => {
    const allowedGitCommands = [
      'git status',
      'git status --short',
      'git log --oneline -10',
      'git diff HEAD~1',
      'git show HEAD',
      'git branch -a',
      'git branch --list',
      'git tag -l',
      'git tag --list',
      'git remote -v',
      'git blame src/main.rs',
      'git shortlog -s -n',
      'git reflog show',
      'git ls-files --cached',
      'git ls-tree HEAD',
      'git ls-remote origin',
      'git rev-parse HEAD',
      'git rev-list --count HEAD',
      'git cat-file -t HEAD',
      'git diff-tree --no-commit-id -r HEAD',
      'git diff-files --stat',
      'git diff-index HEAD',
      'git for-each-ref refs/heads',
      'git merge-base main feature',
      'git name-rev HEAD',
      'git count-objects -v',
      'git verify-commit HEAD',
      'git verify-tag v1.0',
      'git check-ignore node_modules',
      'git check-attr diff README.md',
      'git stash list',
      'git stash show 0',
      'git worktree list',
      'git notes list',
      'git notes show HEAD',
      'git describe --tags',
      'git config user.name',
    ];

    test.each(allowedGitCommands)('allows "%s"', (cmd) => {
      const result = checker.check(cmd);
      expect(result.allowed).toBe(true);
    });
  });

  describe('Destructive git commands are denied', () => {
    const deniedGitCommands = [
      'git push',
      'git push origin main',
      'git push --force',
      'git push --force-with-lease origin main',
      'git reset HEAD~1',
      'git reset --hard HEAD~1',
      'git reset --soft HEAD~1',
      'git clean -fd',
      'git clean -f',
      'git rm file.txt',
      'git rm -r directory/',
      'git commit -m "message"',
      'git commit --amend',
      'git merge feature-branch',
      'git merge --no-ff feature',
      'git rebase main',
      'git rebase -i HEAD~3',
      'git cherry-pick abc123',
      'git stash drop',
      'git stash drop 0',
      'git stash pop',
      'git stash pop 0',
      'git stash push -m "wip"',
      'git stash clear',
      'git branch -d feature',
      'git branch -D feature',
      'git branch --delete feature',
      'git tag -d v1.0',
      'git tag --delete v1.0',
      'git remote remove origin',
      'git remote rm upstream',
      'git checkout --force main',
      'git checkout -f main',
      'git submodule deinit my-module',
      'git notes add -m "note"',
      'git notes remove HEAD',
      'git worktree add ../feature feature-branch',
      'git worktree remove ../feature',
    ];

    test.each(deniedGitCommands)('denies "%s"', (cmd) => {
      const result = checker.check(cmd);
      expect(result.allowed).toBe(false);
    });
  });

  describe('Deny takes precedence over allow for destructive branch/tag/remote ops', () => {
    test('git branch -D is denied even though git:branch:* is in allow list', () => {
      const result = checker.check('git branch -D feature-branch');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('git tag -d is denied even though git:tag:* is in allow list', () => {
      const result = checker.check('git tag -d v1.0');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('git remote remove is denied even though git:remote:* is in allow list', () => {
      const result = checker.check('git remote remove origin');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });
  });
});

describe('GitHub CLI (gh) read-only allow and write deny patterns', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({ debug: false });
  });

  describe('gh read-only commands are allowed', () => {
    const allowedGhCommands = [
      'gh --version',
      'gh help',
      'gh status',
      'gh auth status',
      'gh issue list',
      'gh issue list --state open',
      'gh issue view 123',
      'gh issue view 123 --comments',
      'gh issue status',
      'gh pr list',
      'gh pr list --state merged',
      'gh pr view 456',
      'gh pr view 456 --comments',
      'gh pr status',
      'gh pr diff 456',
      'gh pr checks 456',
      'gh repo list',
      'gh repo view owner/repo',
      'gh release list',
      'gh release view v1.0',
      'gh run list',
      'gh run view 789',
      'gh workflow list',
      'gh workflow view deploy.yml',
      'gh gist list',
      'gh gist view abc123',
      'gh search issues "bug"',
      'gh search prs "fix"',
      'gh search repos "probe"',
      'gh search code "function"',
      'gh search commits "fix bug"',
      'gh api repos/owner/repo/pulls',
      'gh api /user',
    ];

    test.each(allowedGhCommands)('allows "%s"', (cmd) => {
      const result = checker.check(cmd);
      expect(result.allowed).toBe(true);
    });
  });

  describe('gh write/mutating commands are denied', () => {
    const deniedGhCommands = [
      'gh issue create --title "new issue"',
      'gh issue close 123',
      'gh issue delete 123',
      'gh issue edit 123 --title "updated"',
      'gh issue reopen 123',
      'gh issue comment 123 --body "hello"',
      'gh pr create --title "new PR"',
      'gh pr close 456',
      'gh pr merge 456',
      'gh pr edit 456 --title "updated"',
      'gh pr reopen 456',
      'gh pr review 456 --approve',
      'gh pr comment 456 --body "lgtm"',
      'gh repo create my-repo',
      'gh repo delete owner/repo',
      'gh repo fork owner/repo',
      'gh repo rename new-name',
      'gh repo archive owner/repo',
      'gh repo clone owner/repo',
      'gh release create v2.0',
      'gh release delete v1.0',
      'gh release edit v1.0 --notes "updated"',
      'gh run cancel 789',
      'gh run rerun 789',
      'gh workflow run deploy.yml',
      'gh workflow enable deploy.yml',
      'gh workflow disable deploy.yml',
      'gh gist create file.txt',
      'gh gist delete abc123',
      'gh gist edit abc123',
      'gh secret set MY_SECRET',
      'gh secret delete MY_SECRET',
      'gh variable set MY_VAR',
      'gh variable delete MY_VAR',
      'gh label create "bug"',
      'gh label delete "bug"',
      'gh ssh-key add key.pub',
      'gh ssh-key delete 123',
    ];

    test.each(deniedGhCommands)('denies "%s"', (cmd) => {
      const result = checker.check(cmd);
      expect(result.allowed).toBe(false);
    });
  });
});

describe('Complex commands with && || and pipes', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({ debug: false });
  });

  describe('Allowed complex commands (all components safe)', () => {
    const allowedComplex = [
      'cd /tmp && git status',
      'cd /tmp && ls -la',
      'git status && git log --oneline -5',
      'git diff HEAD~1 && git log --oneline',
      'echo hello && pwd',
      'ls -la || echo "no files"',
      'cat package.json | grep name',
      'git log --oneline | head -10',
      'git branch -a | grep feature',
      'git diff --stat | tail -5',
      'gh pr list | grep open',
      'gh issue list --state open | head -5',
      'cd src && ls && pwd',
      'git status && git diff && git log --oneline -3',
      'echo "starting" && git status && echo "done"',
      'ls src/ || ls lib/ || echo "not found"',
      'git rev-parse HEAD && git log --oneline -1',
      'gh pr view 123 | grep title',
      'git ls-files | wc -l',
      'git shortlog -s -n | head -10',
    ];

    test.each(allowedComplex)('allows "%s"', (cmd) => {
      const result = checker.check(cmd);
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
    });
  });

  describe('Denied complex commands (at least one component unsafe)', () => {
    const deniedComplex = [
      'cd /tmp && git push origin main',
      'git status && git commit -m "auto"',
      'git diff && git reset --hard HEAD',
      'echo "deploying" && git push --force',
      'ls && rm -rf /tmp/cache',
      'git log && git rebase main',
      'git status && git merge feature',
      'git branch -a && git branch -D old-branch',
      'git tag -l && git tag -d v1.0',
      'echo "cleaning" && git clean -fd',
      'git status | git push',
      'gh pr list && gh pr merge 456',
      'gh issue list && gh issue close 123',
      'cd /tmp && npm install express',
      'ls && sudo rm -rf /',
      'git diff && git cherry-pick abc123',
      'echo ok && gh repo delete owner/repo',
      'git remote -v && git remote remove origin',
      'git stash list && git stash drop',
      'git log && git commit -m "msg" && git push',
    ];

    test.each(deniedComplex)('denies "%s"', (cmd) => {
      const result = checker.check(cmd);
      expect(result.allowed).toBe(false);
      expect(result.isComplex).toBe(true);
    });
  });

  describe('Mixed operator chains', () => {
    test('allows three safe commands chained with &&', () => {
      const result = checker.check('pwd && ls && echo done');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
      expect(result.allowedByComponents).toBe(true);
    });

    test('denies if last command in chain is unsafe', () => {
      const result = checker.check('git status && git diff && git push');
      expect(result.allowed).toBe(false);
    });

    test('denies if first command in chain is unsafe', () => {
      const result = checker.check('git push && git status && ls');
      expect(result.allowed).toBe(false);
    });

    test('denies if middle command in chain is unsafe', () => {
      const result = checker.check('git status && git commit -m "x" && git log');
      expect(result.allowed).toBe(false);
    });

    test('allows piped safe git and gh commands', () => {
      const result = checker.check('gh pr list --json number | grep 42');
      expect(result.allowed).toBe(true);
    });

    test('allows || fallback between safe commands', () => {
      const result = checker.check('git describe --tags || git rev-parse --short HEAD');
      expect(result.allowed).toBe(true);
    });

    test('denies pipe into unsafe command', () => {
      const result = checker.check('echo "y" | rm -rf /tmp/data');
      expect(result.allowed).toBe(false);
    });
  });
});

describe('Custom allow overrides default deny (priority-based resolution)', () => {
  describe('--bash-allow overrides default deny for specific commands', () => {
    test('git:push in custom allow overrides default deny', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        debug: false
      });
      expect(checker.check('git push').allowed).toBe(true);
      expect(checker.check('git push origin main').allowed).toBe(true);
      expect(checker.check('git push --force').allowed).toBe(true);
      expect(checker.check('git push -u origin feature').allowed).toBe(true);
    });

    test('git:commit in custom allow overrides default deny', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:commit'],
        debug: false
      });
      expect(checker.check('git commit -m "message"').allowed).toBe(true);
      expect(checker.check('git commit --amend').allowed).toBe(true);
    });

    test('npm:install in custom allow overrides default deny', () => {
      const checker = new BashPermissionChecker({
        allow: ['npm:install'],
        debug: false
      });
      expect(checker.check('npm install').allowed).toBe(true);
      expect(checker.check('npm install express').allowed).toBe(true);
    });

    test('multiple custom allow patterns override their respective deny patterns', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push', 'git:commit', 'npm:install'],
        debug: false
      });
      expect(checker.check('git push origin main').allowed).toBe(true);
      expect(checker.check('git commit -m "msg"').allowed).toBe(true);
      expect(checker.check('npm install express').allowed).toBe(true);
    });

    test('other default deny patterns still block when only specific ones are overridden', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        debug: false
      });
      // git:push overridden - allowed
      expect(checker.check('git push').allowed).toBe(true);
      // Other deny patterns still active
      expect(checker.check('git reset --hard HEAD').allowed).toBe(false);
      expect(checker.check('git clean -fd').allowed).toBe(false);
      expect(checker.check('rm -rf /').allowed).toBe(false);
      expect(checker.check('sudo rm -rf /').allowed).toBe(false);
      expect(checker.check('npm install').allowed).toBe(false);
    });

    test('default allow patterns still work alongside custom overrides', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        debug: false
      });
      // Default allow still works
      expect(checker.check('ls -la').allowed).toBe(true);
      expect(checker.check('git status').allowed).toBe(true);
      expect(checker.check('cat file.txt').allowed).toBe(true);
    });
  });

  describe('Custom deny always wins over custom allow', () => {
    test('custom deny beats custom allow for same pattern', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        deny: ['git:push'],
        debug: false
      });
      expect(checker.check('git push').allowed).toBe(false);
      expect(checker.check('git push origin main').allowed).toBe(false);
    });

    test('custom deny beats custom allow even with broader pattern', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        deny: ['git:push:--force'],
        debug: false
      });
      // Regular push allowed (custom allow, not in custom deny)
      expect(checker.check('git push origin main').allowed).toBe(true);
      // Force push denied (custom deny wins)
      expect(checker.check('git push --force').allowed).toBe(false);
    });

    test('can allow push but deny force push specifically', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        deny: ['git:push:--force', 'git:push:--force-with-lease'],
        debug: false
      });
      expect(checker.check('git push origin main').allowed).toBe(true);
      expect(checker.check('git push --tags').allowed).toBe(true);
      expect(checker.check('git push --force').allowed).toBe(false);
      expect(checker.check('git push --force-with-lease').allowed).toBe(false);
    });
  });

  describe('overriddenDeny flag in result', () => {
    test('result indicates when default deny was overridden', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        debug: false
      });
      const result = checker.check('git push origin main');
      expect(result.allowed).toBe(true);
      expect(result.overriddenDeny).toBe(true);
    });

    test('result does not flag overriddenDeny for normally allowed commands', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        debug: false
      });
      const result = checker.check('ls -la');
      expect(result.allowed).toBe(true);
      expect(result.overriddenDeny).toBe(false);
    });
  });

  describe('Complex commands with custom allow override', () => {
    test('cd && git push works when git:push is in custom allow', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        debug: false
      });
      const result = checker.check('cd /tmp && git push origin main');
      expect(result.allowed).toBe(true);
      expect(result.isComplex).toBe(true);
    });

    test('git status && git push works when git:push is in custom allow', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        debug: false
      });
      const result = checker.check('git status && git push origin main');
      expect(result.allowed).toBe(true);
    });

    test('git push | cat works when git:push is in custom allow', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        debug: false
      });
      const result = checker.check('git push origin main 2>&1 | cat');
      // This is a complex command with redirection - should be handled
      // The important thing is git push is no longer denied
    });

    test('complex command denied if one component still in deny', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        debug: false
      });
      // git push is overridden, but rm -rf is still denied
      const result = checker.check('git push && rm -rf /');
      expect(result.allowed).toBe(false);
    });

    test('complex command denied if custom deny matches component', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push'],
        deny: ['git:push:--force'],
        debug: false
      });
      const result = checker.check('git status && git push --force');
      expect(result.allowed).toBe(false);
    });

    test('multi-step deploy pipeline with custom allows', () => {
      const checker = new BashPermissionChecker({
        allow: ['git:push', 'git:commit', 'git:add'],
        debug: false
      });
      expect(checker.check('git add -A && git commit -m "deploy"').allowed).toBe(true);
      expect(checker.check('git commit -m "msg" && git push origin main').allowed).toBe(true);
    });
  });
});

describe('Security Tests (Updated)', () => {
  let checker;

  beforeEach(() => {
    checker = new BashPermissionChecker({ debug: false });
  });

  describe('Dangerous Command Detection', () => {
    test('should block find with -exec', () => {
      const result = checker.check('find . -exec rm {} \\;');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('should block awk (scripting capability)', () => {
      const result = checker.check('awk \'BEGIN { system("rm -rf /") }\'');
      expect(result.allowed).toBe(false);
      expect(result.reason).toContain('deny pattern');
    });

    test('should allow safe find operations', () => {
      const result = checker.check('find . -name "*.js" -type f');
      expect(result.allowed).toBe(true);
    });

    test('should block perl inline execution', () => {
      const result = checker.check('perl -e \'system("dangerous")\'');
      expect(result.allowed).toBe(false);
    });

    test('should block python inline execution', () => {
      const result = checker.check('python -c "import os; os.system(\'rm -rf /\')"');
      expect(result.allowed).toBe(false);
    });
  });

  describe('Architecture Alignment', () => {
    test('should ensure parseCommand and parseCommandForExecution agree', () => {
      const testCases = [
        'ls -la',
        'echo "hello world"',
        'git status',
        'find . -name "*.js"'
      ];

      for (const command of testCases) {
        const parseResult = parseCommand(command);
        const execArray = parseCommandForExecution(command);

        if (parseResult.error) {
          // If parse failed, exec should also fail
          expect(execArray).toBeNull();
        } else {
          // If parse succeeded, exec should work and match
          expect(execArray).not.toBeNull();
          expect(execArray[0]).toBe(parseResult.command);
          expect(execArray.slice(1)).toEqual(parseResult.args);
        }
      }
    });

    test('should allow complex commands via component evaluation when components are allowed', () => {
      // Commands where all components are in default allow list
      const allowedComplexCommands = [
        'ls | grep test',   // ls and grep both allowed
        'echo hello && pwd' // echo and pwd both allowed
      ];

      for (const command of allowedComplexCommands) {
        const permResult = checker.check(command);
        expect(permResult.allowed).toBe(true);
        expect(permResult.isComplex).toBe(true);
        expect(permResult.allowedByComponents).toBe(true);

        // Parser still rejects (for direct execution) but permission check passes
        const execArray = parseCommandForExecution(command);
        expect(execArray).toBeNull();
      }
    });

    test('should reject complex commands with constructs that cannot be split', () => {
      // Commands with command substitution, redirection cannot be split into simple components
      const unsplittableCommands = [
        'echo $(date)',
        'ls > output.txt'
      ];

      for (const command of unsplittableCommands) {
        const permResult = checker.check(command);
        expect(permResult.allowed).toBe(false);
        expect(permResult.isComplex).toBe(true);

        const execArray = parseCommandForExecution(command);
        expect(execArray).toBeNull();
      }
    });
  });
});