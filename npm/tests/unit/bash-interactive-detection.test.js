/**
 * Tests for interactive command detection in bashExecutor
 */

import { checkInteractiveCommand } from '../../src/agent/bashExecutor.js';

describe('checkInteractiveCommand', () => {
  // ── Should BLOCK (returns error message) ──

  describe('interactive editors', () => {
    test.each(['vi', 'vim', 'nvim', 'nano', 'emacs', 'pico'])('blocks %s', (editor) => {
      const result = checkInteractiveCommand(`${editor} file.txt`);
      expect(result).not.toBeNull();
      expect(result).toContain('interactive editor');
    });
  });

  describe('interactive pagers', () => {
    test.each(['less', 'more'])('blocks %s', (pager) => {
      const result = checkInteractiveCommand(`${pager} file.txt`);
      expect(result).not.toBeNull();
      expect(result).toContain('interactive pager');
    });
  });

  describe('git commands that open an editor', () => {
    test('blocks git commit without -m', () => {
      const result = checkInteractiveCommand('git commit');
      expect(result).not.toBeNull();
      expect(result).toContain('git commit');
      expect(result).toContain('-m');
    });

    test('blocks git commit --amend without --no-edit', () => {
      const result = checkInteractiveCommand('git commit --amend');
      expect(result).not.toBeNull();
    });

    test('blocks git rebase --continue', () => {
      const result = checkInteractiveCommand('git rebase --continue');
      expect(result).not.toBeNull();
      expect(result).toContain('GIT_EDITOR');
    });

    test('blocks git rebase --skip', () => {
      const result = checkInteractiveCommand('git rebase --skip');
      expect(result).not.toBeNull();
    });

    test('blocks git rebase -i', () => {
      const result = checkInteractiveCommand('git rebase -i HEAD~3');
      expect(result).not.toBeNull();
      expect(result).toContain('interactive');
    });

    test('blocks git rebase --interactive', () => {
      const result = checkInteractiveCommand('git rebase --interactive HEAD~3');
      expect(result).not.toBeNull();
    });

    test('blocks git merge without --no-edit', () => {
      const result = checkInteractiveCommand('git merge feature-branch');
      expect(result).not.toBeNull();
      expect(result).toContain('--no-edit');
    });

    test('blocks git cherry-pick without --no-edit', () => {
      const result = checkInteractiveCommand('git cherry-pick abc123');
      expect(result).not.toBeNull();
      expect(result).toContain('--no-edit');
    });

    test('blocks git revert without --no-edit', () => {
      const result = checkInteractiveCommand('git revert abc123');
      expect(result).not.toBeNull();
      expect(result).toContain('--no-edit');
    });

    test('blocks git tag -a without -m', () => {
      const result = checkInteractiveCommand('git tag -a v1.0');
      expect(result).not.toBeNull();
      expect(result).toContain('-m');
    });

    test('blocks git add -i', () => {
      const result = checkInteractiveCommand('git add -i');
      expect(result).not.toBeNull();
      expect(result).toContain('interactive');
    });

    test('blocks git add --patch', () => {
      const result = checkInteractiveCommand('git add --patch');
      expect(result).not.toBeNull();
    });
  });

  describe('interactive REPLs', () => {
    test.each(['python', 'python3', 'node', 'irb', 'ruby'])('blocks %s without args', (cmd) => {
      const result = checkInteractiveCommand(cmd);
      expect(result).not.toBeNull();
      expect(result).toContain('REPL');
    });
  });

  describe('interactive database clients', () => {
    test('blocks mysql without -e', () => {
      const result = checkInteractiveCommand('mysql -u root mydb');
      expect(result).not.toBeNull();
      expect(result).toContain('-e');
    });

    test('blocks psql without -c', () => {
      const result = checkInteractiveCommand('psql -U postgres mydb');
      expect(result).not.toBeNull();
      expect(result).toContain('-c');
    });
  });

  describe('interactive TUI tools', () => {
    test.each(['top', 'htop', 'btop'])('blocks %s', (cmd) => {
      const result = checkInteractiveCommand(cmd);
      expect(result).not.toBeNull();
      expect(result).toContain('interactive');
    });
  });

  describe('complex commands with interactive components', () => {
    test('blocks cd && git rebase --continue', () => {
      const result = checkInteractiveCommand('cd tyk-docs && git add file.txt && git rebase --continue');
      expect(result).not.toBeNull();
      expect(result).toContain('git rebase');
    });

    test('blocks piped interactive command', () => {
      const result = checkInteractiveCommand('echo hello | vim');
      expect(result).not.toBeNull();
    });

    test('blocks semicolon-separated interactive command', () => {
      const result = checkInteractiveCommand('git add .; git commit');
      expect(result).not.toBeNull();
    });
  });

  // ── Should ALLOW (returns null) ──

  describe('non-interactive commands', () => {
    test('allows git commit -m "message"', () => {
      expect(checkInteractiveCommand('git commit -m "fix bug"')).toBeNull();
    });

    test('allows git commit --message "message"', () => {
      expect(checkInteractiveCommand('git commit --message "fix bug"')).toBeNull();
    });

    test('allows git commit --no-edit', () => {
      expect(checkInteractiveCommand('git commit --no-edit')).toBeNull();
    });

    test('allows git commit --fixup abc123', () => {
      expect(checkInteractiveCommand('git commit --fixup abc123')).toBeNull();
    });

    test('allows git merge --no-edit', () => {
      expect(checkInteractiveCommand('git merge --no-edit feature-branch')).toBeNull();
    });

    test('allows git merge --ff-only', () => {
      expect(checkInteractiveCommand('git merge --ff-only feature-branch')).toBeNull();
    });

    test('allows git cherry-pick --no-edit', () => {
      expect(checkInteractiveCommand('git cherry-pick --no-edit abc123')).toBeNull();
    });

    test('allows git revert --no-edit', () => {
      expect(checkInteractiveCommand('git revert --no-edit abc123')).toBeNull();
    });

    test('allows git tag -a with -m', () => {
      expect(checkInteractiveCommand('git tag -a v1.0 -m "release"')).toBeNull();
    });

    test('allows git add (non-interactive)', () => {
      expect(checkInteractiveCommand('git add file.txt')).toBeNull();
    });

    test('allows git status', () => {
      expect(checkInteractiveCommand('git status')).toBeNull();
    });

    test('allows git log', () => {
      expect(checkInteractiveCommand('git log --oneline -10')).toBeNull();
    });

    test('allows python with script', () => {
      expect(checkInteractiveCommand('python script.py')).toBeNull();
    });

    test('allows node with script', () => {
      expect(checkInteractiveCommand('node app.js')).toBeNull();
    });

    test('allows mysql with -e', () => {
      expect(checkInteractiveCommand('mysql -e "SELECT 1"')).toBeNull();
    });

    test('allows psql with -c', () => {
      expect(checkInteractiveCommand('psql -c "SELECT 1"')).toBeNull();
    });

    test('allows psql with -f', () => {
      expect(checkInteractiveCommand('psql -f script.sql')).toBeNull();
    });

    test('allows ls, cat, grep etc.', () => {
      expect(checkInteractiveCommand('ls -la')).toBeNull();
      expect(checkInteractiveCommand('cat file.txt')).toBeNull();
      expect(checkInteractiveCommand('grep -r pattern .')).toBeNull();
    });

    test('allows complex non-interactive command', () => {
      expect(checkInteractiveCommand('cd dir && git status && ls -la')).toBeNull();
    });

    test('allows env-prefixed git rebase --continue', () => {
      // When user explicitly sets GIT_EDITOR=true, the env prefix strips it
      // but the command itself is still detected as interactive
      // This tests that bare env prefix stripping works
      const result = checkInteractiveCommand('GIT_EDITOR=true git rebase --continue');
      // After stripping GIT_EDITOR=true, the remaining command is "git rebase --continue"
      // which IS interactive — the env prefix does not make it non-interactive
      // The user should pass env vars via the tool's env parameter instead
      expect(result).not.toBeNull();
    });
  });

  describe('edge cases', () => {
    test('returns null for null input', () => {
      expect(checkInteractiveCommand(null)).toBeNull();
    });

    test('returns null for empty string', () => {
      expect(checkInteractiveCommand('')).toBeNull();
    });

    test('returns null for undefined', () => {
      expect(checkInteractiveCommand(undefined)).toBeNull();
    });
  });
});
