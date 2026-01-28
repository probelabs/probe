/**
 * Tests for searchDelegate behavior in the search tool
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';

// Mock the 'ai' package for tool wrapper
jest.mock('ai', () => ({
  tool: jest.fn((config) => ({
    name: config.name,
    description: config.description,
    inputSchema: config.inputSchema,
    execute: config.execute
  }))
}));

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const mockSearch = jest.fn();
const mockExtract = jest.fn();
const mockDelegate = jest.fn();

const searchModulePath = resolve(__dirname, '../../src/search.js');
const extractModulePath = resolve(__dirname, '../../src/extract.js');
const delegateModulePath = resolve(__dirname, '../../src/delegate.js');

jest.unstable_mockModule(searchModulePath, () => ({
  search: mockSearch
}));

jest.unstable_mockModule(extractModulePath, () => ({
  extract: mockExtract
}));

jest.unstable_mockModule(delegateModulePath, () => ({
  delegate: mockDelegate
}));

const { searchTool } = await import('../../src/tools/vercel.js');

describe('searchDelegate behavior', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  test('delegates search and extracts targets when searchDelegate=true', async () => {
    mockDelegate.mockResolvedValue(JSON.stringify({
      targets: ['src/a.js#foo', 'src/b.js:10-12']
    }));
    mockExtract.mockResolvedValue('EXTRACTED');

    const tracer = {
      withSpan: jest.fn(async (_name, fn) => fn())
    };

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/workspace',
      allowedFolders: ['/workspace'],
      provider: 'google',
      model: 'gemini-2.5-pro',
      tracer
    });

    const result = await tool.execute({
      query: 'searchDelegate',
      path: 'src'
    });

    expect(result).toBe('EXTRACTED');
    expect(mockDelegate).toHaveBeenCalledTimes(1);
    expect(mockDelegate).toHaveBeenCalledWith(expect.objectContaining({
      promptType: 'code-searcher',
      allowedTools: ['search', 'attempt_completion'],
      searchDelegate: false,
      path: '/workspace',
      provider: 'google',
      model: 'gemini-2.5-pro'
    }));
    expect(tracer.withSpan).toHaveBeenCalledWith(
      'search.delegate',
      expect.any(Function),
      expect.objectContaining({
        'search.query': expect.stringContaining('searchDelegate'),
        'search.path': expect.any(String)
      })
    );
    expect(mockExtract).toHaveBeenCalledWith(expect.objectContaining({
      files: expect.arrayContaining([
        expect.stringMatching(/[/\\]workspace[/\\]src[/\\]a\\.js#foo/),
        expect.stringMatching(/[/\\]workspace[/\\]src[/\\]b\\.js:10-12/)
      ])
    }));
    const extractArgs = mockExtract.mock.calls[0][0];
    expect(extractArgs).toEqual(expect.objectContaining({ files: expect.any(Array) }));
    const normalizedFiles = extractArgs.files.map((file) =>
      file.replace(/^[A-Za-z]:/, '').replace(/\\/g, '/')
    );
    expect(normalizedFiles).toEqual(expect.arrayContaining([
      '/workspace/src/a.js#foo',
      '/workspace/src/b.js:10-12'
    ]));
    expect(mockSearch).not.toHaveBeenCalled();
  });

  test('falls back to raw search when delegation fails', async () => {
    mockDelegate.mockRejectedValue(new Error('boom'));
    mockSearch.mockResolvedValue('RAW-SEARCH');

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/workspace',
      allowedFolders: ['/workspace']
    });

    const result = await tool.execute({
      query: 'searchDelegate',
      path: 'src'
    });

    expect(result).toBe('RAW-SEARCH');
    expect(mockSearch).toHaveBeenCalledTimes(1);
  });

  test('falls back to raw search when delegation returns no targets', async () => {
    mockDelegate.mockResolvedValue(JSON.stringify({ targets: [] }));
    mockSearch.mockResolvedValue('RAW-SEARCH');

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/workspace',
      allowedFolders: ['/workspace']
    });

    const result = await tool.execute({
      query: 'searchDelegate',
      path: 'src'
    });

    expect(result).toBe('RAW-SEARCH');
    expect(mockSearch).toHaveBeenCalledTimes(1);
  });

  test('uses raw search when searchDelegate=false', async () => {
    mockSearch.mockResolvedValue('RAW-SEARCH');

    const tool = searchTool({
      searchDelegate: false,
      cwd: '/workspace'
    });

    const result = await tool.execute({
      query: 'searchDelegate',
      path: 'src'
    });

    expect(result).toBe('RAW-SEARCH');
    expect(mockDelegate).not.toHaveBeenCalled();
  });
});
