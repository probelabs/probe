/**
 * Integration tests for the edit tool's "file has not been read yet" guard.
 * Tests the full extract→edit flow with FileTracker path resolution.
 *
 * Issue #510: edit tool fails with 'file has not been read yet' after
 * successful extract of the same file, due to path resolution mismatches.
 */

import { describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { FileTracker } from '../../src/tools/fileTracker.js';
import { editTool } from '../../src/tools/edit.js';
import { promises as fs } from 'fs';
import { join, resolve, sep } from 'path';
import { tmpdir } from 'os';
import { randomUUID } from 'crypto';
import { existsSync } from 'fs';

describe('Edit tool read-before-write guard — issue #510', () => {
  let workspace;
  let tracker;

  beforeEach(async () => {
    workspace = join(tmpdir(), `probe-edit-guard-test-${randomUUID()}`);
    await fs.mkdir(workspace, { recursive: true });
    tracker = new FileTracker({ debug: false });
  });

  afterEach(async () => {
    if (existsSync(workspace)) {
      await fs.rm(workspace, { recursive: true, force: true });
    }
  });

  /**
   * Helper: create an edit tool with specific options and execute with given params.
   * Simulates _buildNativeTools workingDirectory injection.
   */
  async function executeEdit({ toolCwd, toolAllowedFolders, toolWorkspaceRoot, filePath, oldString, newString, workingDirectory }) {
    const tool = editTool({
      cwd: toolCwd,
      allowedFolders: toolAllowedFolders || [workspace],
      workspaceRoot: toolWorkspaceRoot || workspace,
      fileTracker: tracker,
      debug: false
    });

    return await tool.execute({
      file_path: filePath,
      old_string: oldString,
      new_string: newString,
      // Simulate _buildNativeTools injection
      workingDirectory: workingDirectory || toolCwd
    });
  }

  // ─── Basic extract→edit flow ───

  describe('basic extract→edit flow', () => {
    test('should allow edit after extract marks file as seen (same cwd)', async () => {
      const file = join(workspace, 'main.go');
      await fs.writeFile(file, 'package main\n\nfunc hello() string {\n\treturn "hello"\n}\n');

      // Extract marks file as seen
      await tracker.trackFilesFromExtract([file], workspace);

      // Edit should succeed (not get blocked by read guard)
      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'main.go',
        oldString: 'return "hello"',
        newString: 'return "world"',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
      // File should be modified
      const content = await fs.readFile(file, 'utf8');
      expect(content).toContain('return "world"');
    });

    test('should block edit when file was never extracted', async () => {
      const file = join(workspace, 'unread.go');
      await fs.writeFile(file, 'package main\n');

      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'unread.go',
        oldString: 'package main',
        newString: 'package changed',
        workingDirectory: workspace
      });

      expect(result).toContain('has not been read yet');
    });
  });

  // ─── Path resolution mismatch scenarios ───

  describe('path resolution mismatches (the #510 bug class)', () => {
    test('extract with relative path, edit with same relative path, same cwd', async () => {
      const subdir = join(workspace, 'tyk-pump', 'analytics');
      await fs.mkdir(subdir, { recursive: true });
      const file = join(subdir, 'aggregate.go');
      await fs.writeFile(file, 'package analytics\n\nvar tableName = "old_table"\n');

      // Extract: relative path resolved against workspace
      await tracker.trackFilesFromExtract(['tyk-pump/analytics/aggregate.go'], workspace);

      // Edit: same relative path, same cwd
      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'tyk-pump/analytics/aggregate.go',
        oldString: 'var tableName = "old_table"',
        newString: 'var tableName = "new_table"',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('extract with relative path, edit with absolute path', async () => {
      const file = join(workspace, 'file.go');
      await fs.writeFile(file, 'package main\nvar x = 1\n');

      // Extract: relative path
      await tracker.trackFilesFromExtract(['file.go'], workspace);

      // Edit: absolute path
      const result = await executeEdit({
        toolCwd: workspace,
        filePath: file,  // absolute
        oldString: 'var x = 1',
        newString: 'var x = 2',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('extract with absolute path, edit with relative path', async () => {
      const file = join(workspace, 'file.go');
      await fs.writeFile(file, 'package main\nvar x = 1\n');

      // Extract: absolute path
      await tracker.trackFilesFromExtract([file], workspace);

      // Edit: relative path
      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'file.go',  // relative
        oldString: 'var x = 1',
        newString: 'var x = 2',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('extract with line suffix, edit with bare path', async () => {
      const file = join(workspace, 'file.go');
      await fs.writeFile(file, 'line1\nline2\nline3\n');

      // Extract with line range suffix
      await tracker.trackFilesFromExtract([file + ':1-3'], workspace);

      // Edit with bare path
      const result = await executeEdit({
        toolCwd: workspace,
        filePath: file,
        oldString: 'line2',
        newString: 'modified_line2',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('extract with symbol suffix, edit with bare path', async () => {
      const file = join(workspace, 'file.go');
      await fs.writeFile(file, 'package main\nfunc Process() {\n\treturn\n}\n');

      // Extract with symbol suffix (findSymbol may fail but file should still be marked)
      await tracker.trackFilesFromExtract([file + '#Process'], workspace);

      // Edit with bare path
      const result = await executeEdit({
        toolCwd: workspace,
        filePath: file,
        oldString: 'return',
        newString: 'return nil',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('paths with ".." segments should normalize and match', async () => {
      const subdir = join(workspace, 'src', 'pkg');
      await fs.mkdir(subdir, { recursive: true });
      const file = join(subdir, 'handler.go');
      await fs.writeFile(file, 'package pkg\nvar x = 1\n');

      // Extract with clean path
      await tracker.trackFilesFromExtract([file], workspace);

      // Edit with path containing ".."
      const result = await executeEdit({
        toolCwd: workspace,
        filePath: join('src', 'pkg', '..', 'pkg', 'handler.go'),
        oldString: 'var x = 1',
        newString: 'var x = 2',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('paths with "." segments should normalize and match', async () => {
      const file = join(workspace, 'file.go');
      await fs.writeFile(file, 'package main\nvar x = 1\n');

      // Extract with "." in path
      await tracker.trackFilesFromExtract([join(workspace, '.', 'file.go')], workspace);

      // Edit with clean path
      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'file.go',
        oldString: 'var x = 1',
        newString: 'var x = 2',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });
  });

  // ─── _buildNativeTools workingDirectory injection scenarios ───

  describe('workingDirectory injection scenarios', () => {
    test('extract and edit both receive same workingDirectory (normal case)', async () => {
      const subdir = join(workspace, 'project');
      await fs.mkdir(subdir, { recursive: true });
      const file = join(subdir, 'main.go');
      await fs.writeFile(file, 'package main\nvar x = 1\n');

      // Both tools get workingDirectory = workspace (injected by _buildNativeTools)
      await tracker.trackFilesFromExtract(
        [resolve(workspace, 'project/main.go')],
        workspace
      );

      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'project/main.go',
        oldString: 'var x = 1',
        newString: 'var x = 2',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('workingDirectory differs from tool cwd — normalization saves us', async () => {
      // Setup: workspace has a subdirectory structure
      const projectDir = join(workspace, 'myproject');
      await fs.mkdir(projectDir, { recursive: true });
      const file = join(projectDir, 'app.go');
      await fs.writeFile(file, 'package app\nvar name = "old"\n');

      // Extract marks with absolute path (resolved against workspace)
      const absoluteFile = resolve(workspace, 'myproject/app.go');
      tracker.markFileSeen(absoluteFile);

      // Edit uses same absolute path but arrived at via different cwd
      // This should match because normalization makes both canonical
      const result = await executeEdit({
        toolCwd: projectDir,  // Different cwd
        toolAllowedFolders: [workspace],
        filePath: absoluteFile,  // Using absolute path
        oldString: 'var name = "old"',
        newString: 'var name = "new"',
        workingDirectory: projectDir
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('BUG SCENARIO: workspaceRoot != cwd, both resolve same relative path', async () => {
      // This is the exact #510 scenario:
      // ProbeAgent has workspaceRoot=/workspace, cwd=/workspace
      // _buildNativeTools injects workingDirectory = workspaceRoot
      // But before the fix, extract used options.cwd while edit used workingDirectory
      // After the fix, both use workingDirectory consistently

      const file = join(workspace, 'aggregate.go');
      await fs.writeFile(file, 'package main\nvar col = "old_col"\n');

      // Simulate extract: resolves "aggregate.go" against workspace
      const extractResolved = resolve(workspace, 'aggregate.go');
      tracker.markFileSeen(extractResolved);

      // Simulate edit: also resolves "aggregate.go" against workspace
      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'aggregate.go',
        oldString: 'var col = "old_col"',
        newString: 'var col = "new_col"',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
      const content = await fs.readFile(file, 'utf8');
      expect(content).toContain('var col = "new_col"');
    });
  });

  // ─── Search output → edit flow ───

  describe('search output → edit flow', () => {
    test('search tracks files from output, then edit should work', async () => {
      const file = join(workspace, 'found.go');
      await fs.writeFile(file, 'package found\nvar x = 1\n');

      // Simulate search output tracking
      const searchOutput = `File: ${file}\n  1 | package found\n  2 | var x = 1\n`;
      await tracker.trackFilesFromOutput(searchOutput, workspace);

      const result = await executeEdit({
        toolCwd: workspace,
        filePath: file,
        oldString: 'var x = 1',
        newString: 'var x = 2',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('search tracks relative paths from output, edit uses absolute', async () => {
      const file = join(workspace, 'src', 'handler.go');
      await fs.mkdir(join(workspace, 'src'), { recursive: true });
      await fs.writeFile(file, 'package src\nvar y = "old"\n');

      // Search output contains relative paths
      const searchOutput = `File: src/handler.go\n  1 | package src\n`;
      await tracker.trackFilesFromOutput(searchOutput, workspace);

      // Edit with absolute path
      const result = await executeEdit({
        toolCwd: workspace,
        filePath: file,
        oldString: 'var y = "old"',
        newString: 'var y = "new"',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });
  });

  // ─── Multi-file and sequential edit scenarios ───

  describe('multi-file and sequential scenarios', () => {
    test('extract multiple files, edit each one', async () => {
      const files = ['a.go', 'b.go', 'c.go'];
      for (const name of files) {
        await fs.writeFile(join(workspace, name), `package ${name}\nvar x = 1\n`);
      }

      // Extract all files
      await tracker.trackFilesFromExtract(
        files.map(f => join(workspace, f)),
        workspace
      );

      // Edit each one
      for (const name of files) {
        const result = await executeEdit({
          toolCwd: workspace,
          filePath: name,
          oldString: 'var x = 1',
          newString: 'var x = 2',
          workingDirectory: workspace
        });
        expect(result).not.toContain('has not been read yet');
      }
    });

    test('extract file, edit 3 times (hits staleness limit)', async () => {
      const file = join(workspace, 'counter.go');
      await fs.writeFile(file, 'package main\nvar count = 0\n');

      await tracker.trackFilesFromExtract([file], workspace);

      // Edit 1
      let result = await executeEdit({
        toolCwd: workspace,
        filePath: file,
        oldString: 'var count = 0',
        newString: 'var count = 1',
        workingDirectory: workspace
      });
      expect(result).not.toContain('has not been read yet');
      // Note: edit tool internally calls tracker.recordTextEdit

      // Edit 2
      result = await executeEdit({
        toolCwd: workspace,
        filePath: file,
        oldString: 'var count = 1',
        newString: 'var count = 2',
        workingDirectory: workspace
      });
      expect(result).not.toContain('has not been read yet');

      // Edit 3
      result = await executeEdit({
        toolCwd: workspace,
        filePath: file,
        oldString: 'var count = 2',
        newString: 'var count = 3',
        workingDirectory: workspace
      });
      expect(result).not.toContain('has not been read yet');

      // Staleness check after 3 edits (edit tool records internally)
      const staleCheck = tracker.checkTextEditStaleness(file);
      expect(staleCheck.ok).toBe(false);
      expect(staleCheck.editCount).toBe(3);
    });

    test('extract with different target formats for same file', async () => {
      const file = join(workspace, 'multi.go');
      await fs.writeFile(file, 'package main\nfunc A() {}\nfunc B() {}\n');

      // Extract with various formats — all should mark the same file
      await tracker.trackFilesFromExtract([
        file,              // bare path
        file + ':1-3',     // line range
        file + '#A',       // symbol
      ], workspace);

      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'multi.go',
        oldString: 'func A() {}',
        newString: 'func A() { return }',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });
  });

  // ─── Nested subdirectory scenarios (the visor case) ───

  describe('nested subdirectory scenarios (visor-like)', () => {
    test('repo cloned as subdirectory — extract and edit both use same path', async () => {
      // Simulates: user clones tyk-pump inside /workspace/Oel/
      const oel = join(workspace, 'Oel');
      const tykPump = join(oel, 'tyk-pump', 'analytics');
      await fs.mkdir(tykPump, { recursive: true });
      const file = join(tykPump, 'aggregate.go');
      await fs.writeFile(file, 'package analytics\n\nvar tableName = "agg"\n');

      // Extract: "tyk-pump/analytics/aggregate.go" resolved against /workspace/Oel
      await tracker.trackFilesFromExtract(['tyk-pump/analytics/aggregate.go'], oel);

      // Edit: same relative path, same effective cwd
      const result = await executeEdit({
        toolCwd: oel,
        toolAllowedFolders: [oel],
        toolWorkspaceRoot: oel,
        filePath: 'tyk-pump/analytics/aggregate.go',
        oldString: 'var tableName = "agg"',
        newString: 'var tableName = "new_agg"',
        workingDirectory: oel
      });

      expect(result).not.toContain('has not been read yet');
      const content = await fs.readFile(file, 'utf8');
      expect(content).toContain('var tableName = "new_agg"');
    });

    test('extract with absolute, edit with relative — both resolve same', async () => {
      const projectDir = join(workspace, 'project');
      const srcDir = join(projectDir, 'src');
      await fs.mkdir(srcDir, { recursive: true });
      const file = join(srcDir, 'main.go');
      await fs.writeFile(file, 'package main\nvar v = 1\n');

      // Extract with absolute path
      await tracker.trackFilesFromExtract([file], projectDir);

      // Edit with relative path resolved against same dir
      const result = await executeEdit({
        toolCwd: projectDir,
        toolAllowedFolders: [workspace],
        filePath: 'src/main.go',
        oldString: 'var v = 1',
        newString: 'var v = 2',
        workingDirectory: projectDir
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('deeply nested path — extract and edit both traverse correctly', async () => {
      const deep = join(workspace, 'a', 'b', 'c', 'd');
      await fs.mkdir(deep, { recursive: true });
      const file = join(deep, 'deep.go');
      await fs.writeFile(file, 'package deep\nvar z = "old"\n');

      await tracker.trackFilesFromExtract(['a/b/c/d/deep.go'], workspace);

      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'a/b/c/d/deep.go',
        oldString: 'var z = "old"',
        newString: 'var z = "new"',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });
  });

  // ─── Edge cases ───

  describe('edge cases', () => {
    test('file path with spaces', async () => {
      const dir = join(workspace, 'my project');
      await fs.mkdir(dir, { recursive: true });
      const file = join(dir, 'my file.go');
      await fs.writeFile(file, 'package main\nvar x = 1\n');

      await tracker.trackFilesFromExtract([file], workspace);

      const result = await executeEdit({
        toolCwd: workspace,
        filePath: join('my project', 'my file.go'),
        oldString: 'var x = 1',
        newString: 'var x = 2',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('file path with unicode characters', async () => {
      const dir = join(workspace, 'módulo');
      await fs.mkdir(dir, { recursive: true });
      const file = join(dir, 'código.go');
      await fs.writeFile(file, 'package mod\nvar x = 1\n');

      await tracker.trackFilesFromExtract([file], workspace);

      const result = await executeEdit({
        toolCwd: workspace,
        filePath: join('módulo', 'código.go'),
        oldString: 'var x = 1',
        newString: 'var x = 2',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('extract same file multiple times does not break edit', async () => {
      const file = join(workspace, 'repeat.go');
      await fs.writeFile(file, 'package main\nvar x = 1\n');

      // Extract the same file 5 times (e.g., AI retries)
      for (let i = 0; i < 5; i++) {
        await tracker.trackFilesFromExtract([file], workspace);
      }

      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'repeat.go',
        oldString: 'var x = 1',
        newString: 'var x = 2',
        workingDirectory: workspace
      });

      expect(result).not.toContain('has not been read yet');
    });

    test('edit after clear should fail', async () => {
      const file = join(workspace, 'cleared.go');
      await fs.writeFile(file, 'package main\nvar x = 1\n');

      await tracker.trackFilesFromExtract([file], workspace);
      expect(tracker.isFileSeen(file)).toBe(true);

      tracker.clear();

      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'cleared.go',
        oldString: 'var x = 1',
        newString: 'var x = 2',
        workingDirectory: workspace
      });

      expect(result).toContain('has not been read yet');
    });

    test('non-existent file should get file-not-found error, not read-guard error', async () => {
      // Mark a non-existent file as seen (extract does this)
      tracker.markFileSeen(join(workspace, 'ghost.go'));

      const result = await executeEdit({
        toolCwd: workspace,
        filePath: 'ghost.go',
        oldString: 'anything',
        newString: 'something',
        workingDirectory: workspace
      });

      // Should get "file not found" error, not "not read yet"
      expect(result).toContain('File not found');
      expect(result).not.toContain('has not been read yet');
    });
  });

  // ─── Symlink scenarios (when possible) ───

  describe('symlink scenarios', () => {
    test('extract via symlink, edit via real path', async () => {
      const realDir = join(workspace, 'real');
      const linkDir = join(workspace, 'link');
      await fs.mkdir(realDir, { recursive: true });
      const realFile = join(realDir, 'target.go');
      await fs.writeFile(realFile, 'package real\nvar x = 1\n');

      try {
        await fs.symlink(realDir, linkDir);
      } catch {
        // Skip on systems that don't support symlinks
        return;
      }

      // Extract via symlink path
      const linkFile = join(linkDir, 'target.go');
      await tracker.trackFilesFromExtract([linkFile], workspace);

      // Note: isFileSeen with the real path won't match because
      // we don't resolve symlinks (expensive). But the same
      // symbolic path should match.
      expect(tracker.isFileSeen(linkFile)).toBe(true);
      // Real path may NOT match — this is a known limitation
      // (we'd need realpathSync which is expensive)
    });
  });
});
