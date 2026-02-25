/**
 * Tests for multi_edit tool
 */

import { describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { multiEditTool } from '../../src/tools/edit.js';
import { promises as fs } from 'fs';
import { join } from 'path';
import { existsSync } from 'fs';
import { tmpdir } from 'os';
import { randomUUID } from 'crypto';

describe('multi_edit tool', () => {
  let testDir;

  beforeEach(async () => {
    testDir = join(tmpdir(), `probe-multi-edit-test-${randomUUID()}`);
    await fs.mkdir(testDir, { recursive: true });
  });

  afterEach(async () => {
    if (existsSync(testDir)) {
      await fs.rm(testDir, { recursive: true, force: true });
    }
  });

  function createTool() {
    return multiEditTool({
      debug: false,
      allowedFolders: [testDir]
    });
  }

  async function createFile(name, content) {
    const path = join(testDir, name);
    await fs.writeFile(path, content);
    return path;
  }

  test('should apply two text edits on different files', async () => {
    const fileA = await createFile('a.txt', 'Hello world');
    const fileB = await createFile('b.txt', 'Goodbye world');

    const tool = createTool();
    const result = await tool.execute({
      edits: JSON.stringify([
        { file_path: fileA, old_string: 'Hello', new_string: 'Hi' },
        { file_path: fileB, old_string: 'Goodbye', new_string: 'Bye' }
      ])
    });

    expect(result).toContain('Multi-edit: 2/2 succeeded');
    expect(result).toContain('[1] OK');
    expect(result).toContain('[2] OK');

    const contentA = await fs.readFile(fileA, 'utf-8');
    expect(contentA).toBe('Hi world');

    const contentB = await fs.readFile(fileB, 'utf-8');
    expect(contentB).toBe('Bye world');
  });

  test('should handle partial failure — failed edit does not stop remaining edits', async () => {
    const fileA = await createFile('a.txt', 'Hello world');
    const fileB = await createFile('b.txt', 'Goodbye world');

    const tool = createTool();
    const result = await tool.execute({
      edits: JSON.stringify([
        { file_path: fileA, old_string: 'Hello', new_string: 'Hi' },
        { file_path: fileA, old_string: 'NONEXISTENT', new_string: 'replaced' },
        { file_path: fileB, old_string: 'Goodbye', new_string: 'Bye' }
      ])
    });

    expect(result).toContain('Multi-edit: 2/3 succeeded, 1 failed');
    expect(result).toContain('[1] OK');
    expect(result).toContain('[2] FAIL');
    expect(result).toContain('[3] OK');

    const contentB = await fs.readFile(fileB, 'utf-8');
    expect(contentB).toBe('Bye world');
  });

  test('should return error for invalid JSON', async () => {
    const tool = createTool();
    const result = await tool.execute({ edits: 'not valid json' });

    expect(result).toContain('Error: Invalid JSON');
  });

  test('should return error for empty array', async () => {
    const tool = createTool();
    const result = await tool.execute({ edits: '[]' });

    expect(result).toContain('Error: edits must be a non-empty JSON array');
  });

  test('should return error when exceeding 50 edits limit', async () => {
    const edits = Array.from({ length: 51 }, (_, i) => ({
      file_path: `file${i}.txt`,
      old_string: 'a',
      new_string: 'b'
    }));

    const tool = createTool();
    const result = await tool.execute({ edits: JSON.stringify(edits) });

    expect(result).toContain('Error: Too many edits (51)');
    expect(result).toContain('Maximum 50');
  });

  test('should handle sequential same-file edits — second sees first changes', async () => {
    const file = await createFile('seq.txt', 'aaa bbb ccc');

    const tool = createTool();
    const result = await tool.execute({
      edits: JSON.stringify([
        { file_path: file, old_string: 'aaa', new_string: 'xxx' },
        { file_path: file, old_string: 'xxx bbb', new_string: 'yyy' }
      ])
    });

    expect(result).toContain('Multi-edit: 2/2 succeeded');

    const content = await fs.readFile(file, 'utf-8');
    expect(content).toBe('yyy ccc');
  });

  test('should handle non-object entries in array gracefully', async () => {
    const file = await createFile('a.txt', 'Hello world');

    const tool = createTool();
    const result = await tool.execute({
      edits: JSON.stringify([
        'not an object',
        { file_path: file, old_string: 'Hello', new_string: 'Hi' }
      ])
    });

    expect(result).toContain('1/2 succeeded, 1 failed');
    expect(result).toContain('[1] FAIL');
    expect(result).toContain('[2] OK');

    const content = await fs.readFile(file, 'utf-8');
    expect(content).toBe('Hi world');
  });

  test('should accept edits as a pre-parsed array (not just JSON string)', async () => {
    const file = await createFile('a.txt', 'Hello world');

    const tool = createTool();
    const result = await tool.execute({
      edits: [
        { file_path: file, old_string: 'Hello', new_string: 'Hi' }
      ]
    });

    expect(result).toContain('Multi-edit: 1/1 succeeded');
  });

  test('should return error for non-array, non-string edits', async () => {
    const tool = createTool();
    const result = await tool.execute({ edits: 42 });

    expect(result).toContain('Error: edits must be a JSON array');
  });
});
