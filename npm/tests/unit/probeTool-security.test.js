/**
 * Tests for probeTool security validations
 * @module tests/unit/probeTool-security
 */

import { jest, describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { listFilesTool, searchFilesTool } from '../../src/agent/probeTool.js';
import path from 'path';
import { promises as fsPromises } from 'fs';
import os from 'os';

describe('ProbeTool Security', () => {
  let testWorkspace;
  let outsideDir;

  beforeEach(async () => {
    // Create a temporary workspace directory
    testWorkspace = path.join(os.tmpdir(), `probe-test-${Date.now()}`);
    await fsPromises.mkdir(testWorkspace, { recursive: true });

    // Create a test file inside the workspace
    await fsPromises.writeFile(
      path.join(testWorkspace, 'test-file.txt'),
      'test content'
    );

    // Create a directory outside the workspace
    outsideDir = path.join(os.tmpdir(), `probe-outside-${Date.now()}`);
    await fsPromises.mkdir(outsideDir, { recursive: true });
    await fsPromises.writeFile(
      path.join(outsideDir, 'outside-file.txt'),
      'outside content'
    );
  });

  afterEach(async () => {
    // Clean up test directories
    try {
      await fsPromises.rm(testWorkspace, { recursive: true, force: true });
      await fsPromises.rm(outsideDir, { recursive: true, force: true });
    } catch (error) {
      // Ignore cleanup errors
    }
  });

  describe('listFilesTool Security', () => {
    test('should allow access to workspace directory', async () => {
      const result = await listFilesTool.execute({
        directory: '.',
        workingDirectory: testWorkspace
      });

      expect(result).toContain('test-file.txt');
    });

    test('should allow access to subdirectories within workspace using relative paths', async () => {
      // Create a subdirectory
      const subdir = path.join(testWorkspace, 'subdir');
      await fsPromises.mkdir(subdir);
      await fsPromises.writeFile(path.join(subdir, 'sub-file.txt'), 'sub content');

      const result = await listFilesTool.execute({
        directory: 'subdir',
        workingDirectory: testWorkspace
      });

      expect(result).toContain('sub-file.txt');
    });

    test('should reject absolute path outside workspace', async () => {
      await expect(async () => {
        await listFilesTool.execute({
          directory: outsideDir,
          workingDirectory: testWorkspace
        });
      }).rejects.toThrow(/Path traversal attempt detected/);
    });

    test('should reject path traversal using ../..', async () => {
      await expect(async () => {
        await listFilesTool.execute({
          directory: '../../../',
          workingDirectory: testWorkspace
        });
      }).rejects.toThrow(/Path traversal attempt detected/);
    });

    test('should reject absolute path to system directories', async () => {
      await expect(async () => {
        await listFilesTool.execute({
          directory: '/etc',
          workingDirectory: testWorkspace
        });
      }).rejects.toThrow(/Path traversal attempt detected/);
    });

    test('should reject absolute path to home directory', async () => {
      const homeDir = os.homedir();
      await expect(async () => {
        await listFilesTool.execute({
          directory: homeDir,
          workingDirectory: testWorkspace
        });
      }).rejects.toThrow(/Path traversal attempt detected/);
    });

    test('should allow access to workspace root using absolute path', async () => {
      const result = await listFilesTool.execute({
        directory: testWorkspace,
        workingDirectory: testWorkspace
      });

      expect(result).toContain('test-file.txt');
    });

    test('should allow access to subdirectory using absolute path within workspace', async () => {
      // Create a subdirectory
      const subdir = path.join(testWorkspace, 'subdir');
      await fsPromises.mkdir(subdir);
      await fsPromises.writeFile(path.join(subdir, 'sub-file.txt'), 'sub content');

      const result = await listFilesTool.execute({
        directory: subdir,
        workingDirectory: testWorkspace
      });

      expect(result).toContain('sub-file.txt');
    });
  });

  describe('searchFilesTool Security', () => {
    test('should allow searching in workspace directory', async () => {
      const result = await searchFilesTool.execute({
        pattern: '*.txt',
        directory: '.',
        workingDirectory: testWorkspace
      });

      expect(result).toContain('test-file.txt');
    });

    test('should allow searching in subdirectories within workspace using relative paths', async () => {
      // Create a subdirectory
      const subdir = path.join(testWorkspace, 'subdir');
      await fsPromises.mkdir(subdir);
      await fsPromises.writeFile(path.join(subdir, 'sub-file.txt'), 'sub content');

      const result = await searchFilesTool.execute({
        pattern: '*.txt',
        directory: 'subdir',
        workingDirectory: testWorkspace
      });

      expect(result).toContain('sub-file.txt');
    });

    test('should reject absolute path outside workspace', async () => {
      await expect(async () => {
        await searchFilesTool.execute({
          pattern: '*.txt',
          directory: outsideDir,
          workingDirectory: testWorkspace
        });
      }).rejects.toThrow(/Path traversal attempt detected/);
    });

    test('should reject path traversal using ../..', async () => {
      await expect(async () => {
        await searchFilesTool.execute({
          pattern: '*.txt',
          directory: '../../../',
          workingDirectory: testWorkspace
        });
      }).rejects.toThrow(/Path traversal attempt detected/);
    });

    test('should reject absolute path to system directories', async () => {
      await expect(async () => {
        await searchFilesTool.execute({
          pattern: '*.txt',
          directory: '/etc',
          workingDirectory: testWorkspace
        });
      }).rejects.toThrow(/Path traversal attempt detected/);
    });

    test('should reject absolute path to home directory', async () => {
      const homeDir = os.homedir();
      await expect(async () => {
        await searchFilesTool.execute({
          pattern: '*.txt',
          directory: homeDir,
          workingDirectory: testWorkspace
        });
      }).rejects.toThrow(/Path traversal attempt detected/);
    });

    test('should allow searching workspace root using absolute path', async () => {
      const result = await searchFilesTool.execute({
        pattern: '*.txt',
        directory: testWorkspace,
        workingDirectory: testWorkspace
      });

      expect(result).toContain('test-file.txt');
    });

    test('should allow searching subdirectory using absolute path within workspace', async () => {
      // Create a subdirectory
      const subdir = path.join(testWorkspace, 'subdir');
      await fsPromises.mkdir(subdir);
      await fsPromises.writeFile(path.join(subdir, 'sub-file.txt'), 'sub content');

      const result = await searchFilesTool.execute({
        pattern: '*.txt',
        directory: subdir,
        workingDirectory: testWorkspace
      });

      expect(result).toContain('sub-file.txt');
    });
  });
});
