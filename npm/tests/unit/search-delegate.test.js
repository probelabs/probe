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
    // Delegate returns paths relative to the search directory (searchPaths[0]),
    // not relative to cwd
    mockDelegate.mockResolvedValue(JSON.stringify({
      targets: ['a.js#foo', 'b.js:10-12']
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
      allowedTools: ['search', 'extract', 'listFiles'],
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
      }),
      expect.any(Function)
    );
    const extractArgs = mockExtract.mock.calls[0][0];
    expect(extractArgs).toEqual(expect.objectContaining({ files: expect.any(Array) }));
    // Paths should be resolved against delegateBase (allowedFolders[0] = /workspace),
    // not searchPaths[0] (/workspace/src)
    const normalizedFiles = extractArgs.files.map((file) =>
      file.replace(/^[A-Za-z]:/, '').replace(/\\/g, '/')
    );
    expect(normalizedFiles).toEqual(expect.arrayContaining([
      '/workspace/a.js#foo',
      '/workspace/b.js:10-12'
    ]));
    expect(mockSearch).not.toHaveBeenCalled();
  });

  test('resolves delegate paths against workspace root when subagent returns workspace-relative paths', async () => {
    // Real scenario: subagent runs from /tmp/workspace (workspace root)
    // and returns paths relative to that root, including the project dir name
    mockDelegate.mockResolvedValue(JSON.stringify({
      targets: ['tyk-analytics/dashboard/api.go#Handler', 'tyk-analytics/dashboard/model.go:10-20']
    }));
    mockExtract.mockResolvedValue('EXTRACTED');

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/tmp/workspace',
      allowedFolders: ['/tmp/workspace']
    });

    const result = await tool.execute({
      query: 'APIDefinition',
      path: '/tmp/workspace/tyk-analytics'
    });

    expect(result).toBe('EXTRACTED');
    const extractArgs = mockExtract.mock.calls[0][0];
    const normalizedFiles = extractArgs.files.map((file) =>
      file.replace(/^[A-Za-z]:/, '').replace(/\\/g, '/')
    );
    // Paths should resolve against delegateBase (/tmp/workspace), NOT searchPaths[0] (/tmp/workspace/tyk-analytics)
    // This prevents doubled path: /tmp/workspace/tyk-analytics/tyk-analytics/dashboard/api.go
    expect(normalizedFiles).toEqual(expect.arrayContaining([
      '/tmp/workspace/tyk-analytics/dashboard/api.go#Handler',
      '/tmp/workspace/tyk-analytics/dashboard/model.go:10-20'
    ]));
    // Should NOT have doubled paths
    expect(normalizedFiles.some(f => f.includes('tyk-analytics/tyk-analytics'))).toBe(false);
    // Extract cwd should be the search path (resolutionBase)
    expect(extractArgs.cwd).toBe('/tmp/workspace/tyk-analytics');
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

  test('strips workspace root prefix from extract output paths', async () => {
    // Delegate returns absolute paths (which is common from the subagent)
    mockDelegate.mockResolvedValue(JSON.stringify({
      targets: ['/tmp/workspace/tyk/apidef/migration.go#migrateGlobalRateLimit']
    }));
    // Extract output contains the absolute path in its text
    mockExtract.mockResolvedValue(
      '=== /tmp/workspace/tyk/apidef/migration.go ===\n' +
      'func migrateGlobalRateLimit() {\n' +
      '  // ...\n' +
      '}\n'
    );

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/tmp/workspace/tyk',
      allowedFolders: ['/tmp/workspace/tyk']
    });

    const result = await tool.execute({
      query: 'migrateGlobalRateLimit',
      path: '/tmp/workspace/tyk'
    });

    // The workspace root prefix should be stripped from the output
    expect(result).toBe(
      '=== apidef/migration.go ===\n' +
      'func migrateGlobalRateLimit() {\n' +
      '  // ...\n' +
      '}\n'
    );
    expect(result).not.toContain('/tmp/workspace/tyk/');
  });

  test('uses searchDelegateProvider/searchDelegateModel config options to override delegate model', async () => {
    mockDelegate.mockResolvedValue(JSON.stringify({
      targets: ['a.js#foo']
    }));
    mockExtract.mockResolvedValue('EXTRACTED');

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/workspace',
      allowedFolders: ['/workspace'],
      provider: 'anthropic',
      model: 'claude-sonnet-4-6',
      searchDelegateProvider: 'google',
      searchDelegateModel: 'gemini-2.0-flash'
    });

    await tool.execute({ query: 'test', path: 'src' });

    expect(mockDelegate).toHaveBeenCalledWith(expect.objectContaining({
      provider: 'google',
      model: 'gemini-2.0-flash'
    }));
  });

  test('uses PROBE_SEARCH_DELEGATE_PROVIDER/MODEL env vars to override delegate model', async () => {
    const originalProvider = process.env.PROBE_SEARCH_DELEGATE_PROVIDER;
    const originalModel = process.env.PROBE_SEARCH_DELEGATE_MODEL;

    try {
      process.env.PROBE_SEARCH_DELEGATE_PROVIDER = 'google';
      process.env.PROBE_SEARCH_DELEGATE_MODEL = 'gemini-2.0-flash';

      mockDelegate.mockResolvedValue(JSON.stringify({
        targets: ['a.js#foo']
      }));
      mockExtract.mockResolvedValue('EXTRACTED');

      const tool = searchTool({
        searchDelegate: true,
        cwd: '/workspace',
        allowedFolders: ['/workspace'],
        provider: 'anthropic',
        model: 'claude-sonnet-4-6'
      });

      await tool.execute({ query: 'test', path: 'src' });

      expect(mockDelegate).toHaveBeenCalledWith(expect.objectContaining({
        provider: 'google',
        model: 'gemini-2.0-flash'
      }));
    } finally {
      if (originalProvider === undefined) delete process.env.PROBE_SEARCH_DELEGATE_PROVIDER;
      else process.env.PROBE_SEARCH_DELEGATE_PROVIDER = originalProvider;
      if (originalModel === undefined) delete process.env.PROBE_SEARCH_DELEGATE_MODEL;
      else process.env.PROBE_SEARCH_DELEGATE_MODEL = originalModel;
    }
  });

  test('config options take priority over env vars for search delegate', async () => {
    const originalProvider = process.env.PROBE_SEARCH_DELEGATE_PROVIDER;
    const originalModel = process.env.PROBE_SEARCH_DELEGATE_MODEL;

    try {
      process.env.PROBE_SEARCH_DELEGATE_PROVIDER = 'openai';
      process.env.PROBE_SEARCH_DELEGATE_MODEL = 'gpt-4o-mini';

      mockDelegate.mockResolvedValue(JSON.stringify({
        targets: ['a.js#foo']
      }));
      mockExtract.mockResolvedValue('EXTRACTED');

      const tool = searchTool({
        searchDelegate: true,
        cwd: '/workspace',
        allowedFolders: ['/workspace'],
        provider: 'anthropic',
        model: 'claude-sonnet-4-6',
        searchDelegateProvider: 'google',
        searchDelegateModel: 'gemini-2.0-flash'
      });

      await tool.execute({ query: 'test', path: 'src' });

      // Config options should win over env vars
      expect(mockDelegate).toHaveBeenCalledWith(expect.objectContaining({
        provider: 'google',
        model: 'gemini-2.0-flash'
      }));
    } finally {
      if (originalProvider === undefined) delete process.env.PROBE_SEARCH_DELEGATE_PROVIDER;
      else process.env.PROBE_SEARCH_DELEGATE_PROVIDER = originalProvider;
      if (originalModel === undefined) delete process.env.PROBE_SEARCH_DELEGATE_MODEL;
      else process.env.PROBE_SEARCH_DELEGATE_MODEL = originalModel;
    }
  });

  test('falls back to parent provider/model when no search delegate override is set', async () => {
    const originalProvider = process.env.PROBE_SEARCH_DELEGATE_PROVIDER;
    const originalModel = process.env.PROBE_SEARCH_DELEGATE_MODEL;

    try {
      delete process.env.PROBE_SEARCH_DELEGATE_PROVIDER;
      delete process.env.PROBE_SEARCH_DELEGATE_MODEL;

      mockDelegate.mockResolvedValue(JSON.stringify({
        targets: ['a.js#foo']
      }));
      mockExtract.mockResolvedValue('EXTRACTED');

      const tool = searchTool({
        searchDelegate: true,
        cwd: '/workspace',
        allowedFolders: ['/workspace'],
        provider: 'anthropic',
        model: 'claude-sonnet-4-6'
      });

      await tool.execute({ query: 'test', path: 'src' });

      expect(mockDelegate).toHaveBeenCalledWith(expect.objectContaining({
        provider: 'anthropic',
        model: 'claude-sonnet-4-6'
      }));
    } finally {
      if (originalProvider === undefined) delete process.env.PROBE_SEARCH_DELEGATE_PROVIDER;
      else process.env.PROBE_SEARCH_DELEGATE_PROVIDER = originalProvider;
      if (originalModel === undefined) delete process.env.PROBE_SEARCH_DELEGATE_MODEL;
      else process.env.PROBE_SEARCH_DELEGATE_MODEL = originalModel;
    }
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

  test('dedup messages escalate with repeat count and force stop after 3', async () => {
    mockSearch.mockResolvedValue('search result');

    const tool = searchTool({
      cwd: '/workspace',
      searchDelegate: false
    });

    // First call succeeds
    const r1 = await tool.execute({ query: 'test query', path: '.' });
    expect(r1).toBe('search result');

    // Second call is first dedup block (1x)
    const r2 = await tool.execute({ query: 'test query', path: '.' });
    expect(r2).toContain('DUPLICATE SEARCH BLOCKED (1x)');

    // Third call is second dedup block (2x)
    const r3 = await tool.execute({ query: 'test query', path: '.' });
    expect(r3).toContain('DUPLICATE SEARCH BLOCKED (2x)');

    // Fourth call triggers force-stop message (3x)
    const r4 = await tool.execute({ query: 'test query', path: '.' });
    expect(r4).toContain('STOP');
    expect(r4).toContain('3 times');
  });

  test('dedup counter is per search key, not global', async () => {
    mockSearch.mockResolvedValue('search result');

    const tool = searchTool({
      cwd: '/workspace',
      searchDelegate: false
    });

    // First query
    await tool.execute({ query: 'query1', path: '.' });
    // Duplicate of query1 → 1x
    const r1 = await tool.execute({ query: 'query1', path: '.' });
    expect(r1).toContain('1x');

    // Different query succeeds
    const r2 = await tool.execute({ query: 'query2', path: '.' });
    expect(r2).toBe('search result');

    // Duplicate of query2 should be 1x (independent counter)
    const r3 = await tool.execute({ query: 'query2', path: '.' });
    expect(r3).toContain('1x');

    // Duplicate of query1 again should be 2x (its own counter continues)
    const r4 = await tool.execute({ query: 'query1', path: '.' });
    expect(r4).toContain('2x');
  });

  test('same query on different paths is not blocked', async () => {
    mockSearch.mockResolvedValue('search result');

    const tool = searchTool({
      cwd: '/workspace',
      searchDelegate: false
    });

    // Search in path /repo1
    const r1 = await tool.execute({ query: 'findme', path: '/repo1' });
    expect(r1).toBe('search result');

    // Same query in path /repo2 should NOT be blocked
    const r2 = await tool.execute({ query: 'findme', path: '/repo2' });
    expect(r2).toBe('search result');
  });

  test('dedup message differentiates no-results vs had-results', async () => {
    const tool = searchTool({
      cwd: '/workspace',
      searchDelegate: false
    });

    // Search that returns results
    mockSearch.mockResolvedValueOnce('Found: file1.rs line 10');
    await tool.execute({ query: 'with_results', path: '.' });
    const r1 = await tool.execute({ query: 'with_results', path: '.' });
    expect(r1).toContain('found results');
    expect(r1).toContain('Use extract');

    // Search that returns no results
    mockSearch.mockResolvedValueOnce('No results found.');
    await tool.execute({ query: 'no_results', path: '.' });
    const r2 = await tool.execute({ query: 'no_results', path: '.' });
    expect(r2).toContain('NO results');
    expect(r2).not.toContain('Use extract');
  });

  test('no-results hint for ticket/issue ID queries', async () => {
    mockSearch.mockResolvedValue('No results found. Search completed in 5ms');

    const tool = searchTool({
      cwd: '/workspace',
      searchDelegate: false
    });

    // Ticket ID pattern like TT-16546
    const r1 = await tool.execute({ query: 'TT-16546', path: '.' });
    expect(r1).toContain('ticket/issue ID');
    expect(r1).toContain('technical concepts');

    // JIRA-style pattern
    mockSearch.mockResolvedValue('No results found.');
    const r2 = await tool.execute({ query: 'PROJ-123', path: '/other' });
    expect(r2).toContain('ticket/issue ID');

    // Normal query should NOT get ticket hint
    mockSearch.mockResolvedValue('No results found.');
    const r3 = await tool.execute({ query: 'handleAuth', path: '/repo' });
    expect(r3).not.toContain('ticket/issue ID');
  });
});
