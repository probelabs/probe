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

  test('delegates search and returns structured JSON when searchDelegate=true', async () => {
    // Delegate returns paths relative to the search directory (searchPaths[0]),
    // not relative to cwd
    mockDelegate.mockResolvedValue(JSON.stringify({
      targets: ['a.js#foo', 'b.js:10-12']
    }));

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

    // Now returns structured JSON instead of auto-extracting
    const parsed = JSON.parse(result);
    expect(parsed.confidence).toBe('medium');
    expect(parsed.groups).toHaveLength(1);
    expect(parsed.groups[0].files).toEqual(expect.arrayContaining([
      expect.stringContaining('a.js#foo'),
      expect.stringContaining('b.js:10-12')
    ]));
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
    // Extract should NOT be called — structured JSON is returned directly
    expect(mockExtract).not.toHaveBeenCalled();
    expect(mockSearch).not.toHaveBeenCalled();
  });

  test('resolves delegate paths against workspace root when subagent returns workspace-relative paths', async () => {
    // Real scenario: subagent runs from /tmp/workspace (workspace root)
    // and returns paths relative to that root, including the project dir name
    mockDelegate.mockResolvedValue(JSON.stringify({
      targets: ['tyk-analytics/dashboard/api.go#Handler', 'tyk-analytics/dashboard/model.go:10-20']
    }));

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/tmp/workspace',
      allowedFolders: ['/tmp/workspace']
    });

    const result = await tool.execute({
      query: 'APIDefinition',
      path: '/tmp/workspace/tyk-analytics'
    });

    // Returns structured JSON with resolved paths (workspace prefix stripped)
    const parsed = JSON.parse(result);
    expect(parsed.confidence).toBe('medium');
    expect(parsed.groups).toHaveLength(1);
    const files = parsed.groups[0].files;
    // Should NOT have doubled paths
    expect(files.some(f => f.includes('tyk-analytics/tyk-analytics'))).toBe(false);
    // Extract should NOT be called — structured JSON is returned directly
    expect(mockExtract).not.toHaveBeenCalled();
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

  test('strips workspace root prefix from file paths in structured output', async () => {
    // Delegate returns absolute paths (which is common from the subagent)
    mockDelegate.mockResolvedValue(JSON.stringify({
      targets: ['/tmp/workspace/tyk/apidef/migration.go#migrateGlobalRateLimit']
    }));

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/tmp/workspace/tyk',
      allowedFolders: ['/tmp/workspace/tyk']
    });

    const result = await tool.execute({
      query: 'migrateGlobalRateLimit',
      path: '/tmp/workspace/tyk'
    });

    // Returns structured JSON with workspace prefix stripped from file paths
    const parsed = JSON.parse(result);
    expect(parsed.confidence).toBe('medium');
    expect(parsed.groups).toHaveLength(1);
    expect(parsed.groups[0].files[0]).toBe('apidef/migration.go#migrateGlobalRateLimit');
    // Should not contain the workspace root prefix
    expect(result).not.toContain('/tmp/workspace/tyk/');
  });

  test('uses searchDelegateProvider/searchDelegateModel config options to override delegate model', async () => {
    mockDelegate.mockResolvedValue(JSON.stringify({
      targets: ['a.js#foo']
    }));

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

  test('search delegation passes allowEdit to delegate (#534)', async () => {
    mockDelegate.mockResolvedValue(JSON.stringify({
      targets: ['a.js#foo']
    }));

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/workspace',
      allowedFolders: ['/workspace'],
      allowEdit: true,
      tracer: { withSpan: jest.fn(async (_name, fn) => fn()) }
    });

    await tool.execute({ query: 'test', path: 'src' });

    expect(mockDelegate).toHaveBeenCalledWith(expect.objectContaining({
      allowEdit: true
    }));
  });

  test('search delegation defaults allowEdit to false when not set (#534)', async () => {
    mockDelegate.mockResolvedValue(JSON.stringify({
      targets: ['a.js#foo']
    }));

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/workspace',
      allowedFolders: ['/workspace'],
      tracer: { withSpan: jest.fn(async (_name, fn) => fn()) }
    });

    await tool.execute({ query: 'test', path: 'src' });

    expect(mockDelegate).toHaveBeenCalledWith(expect.objectContaining({
      allowEdit: false
    }));
  });

  describe('structured response with searches field', () => {
    test('passes through searches array from delegate response', async () => {
      mockDelegate.mockResolvedValue(JSON.stringify({
        confidence: 'high',
        groups: [
          { reason: 'Core auth logic', files: ['src/auth.js#login'] }
        ],
        searches: [
          { query: 'authentication', path: '/workspace', had_results: true },
          { query: 'login handler', path: '/workspace', had_results: true },
          { query: 'oauth provider', path: '/workspace', had_results: false }
        ]
      }));

      const tool = searchTool({
        searchDelegate: true,
        cwd: '/workspace',
        allowedFolders: ['/workspace'],
        tracer: { withSpan: jest.fn(async (_name, fn) => fn()) }
      });

      const result = await tool.execute({ query: 'How does authentication work?', path: '/workspace' });
      const parsed = JSON.parse(result);

      expect(parsed.confidence).toBe('high');
      expect(parsed.groups).toHaveLength(1);
      expect(parsed.groups[0].reason).toBe('Core auth logic');
      expect(parsed.searches).toHaveLength(3);
      expect(parsed.searches[0]).toEqual({ query: 'authentication', path: '/workspace', had_results: true });
      expect(parsed.searches[2].had_results).toBe(false);
    });

    test('defaults searches to empty array when missing from response', async () => {
      // Legacy format without searches field
      mockDelegate.mockResolvedValue(JSON.stringify({
        targets: ['src/main.js#init']
      }));

      const tool = searchTool({
        searchDelegate: true,
        cwd: '/workspace',
        allowedFolders: ['/workspace'],
        tracer: { withSpan: jest.fn(async (_name, fn) => fn()) }
      });

      const result = await tool.execute({ query: 'Where is init?', path: '/workspace' });
      const parsed = JSON.parse(result);

      expect(parsed.searches).toEqual([]);
      expect(parsed.groups).toHaveLength(1);
    });

    test('handles partial results with low confidence from iteration-limited delegate', async () => {
      // Simulate what a delegate outputs when it runs out of iterations:
      // partial results with low confidence and search history
      mockDelegate.mockResolvedValue(JSON.stringify({
        confidence: 'low',
        groups: [
          { reason: 'Possibly related to rate limiting', files: ['src/middleware/rateLimit.js#RateLimiter'] }
        ],
        searches: [
          { query: 'rate limiting', path: '/workspace', had_results: true },
          { query: 'throttle middleware', path: '/workspace', had_results: false },
          { query: 'request quota', path: '/workspace', had_results: false }
        ]
      }));

      const tool = searchTool({
        searchDelegate: true,
        cwd: '/workspace',
        allowedFolders: ['/workspace'],
        tracer: { withSpan: jest.fn(async (_name, fn) => fn()) }
      });

      const result = await tool.execute({ query: 'How does rate limiting work?', path: '/workspace' });
      const parsed = JSON.parse(result);

      // Partial results should still be returned as structured JSON
      expect(parsed.confidence).toBe('low');
      expect(parsed.groups).toHaveLength(1);
      expect(parsed.searches).toHaveLength(3);
      // Parent can see which searches failed and decide to retry with different terms
      const failedSearches = parsed.searches.filter(s => !s.had_results);
      expect(failedSearches).toHaveLength(2);
    });

    test('handles empty groups with searches from exhausted delegate', async () => {
      // Delegate ran out of iterations without finding anything relevant
      mockDelegate.mockResolvedValue(JSON.stringify({
        confidence: 'low',
        groups: [],
        searches: [
          { query: 'nonexistent function', path: '/workspace', had_results: false },
          { query: 'missing_module', path: '/workspace', had_results: false }
        ]
      }));

      const tool = searchTool({
        searchDelegate: true,
        cwd: '/workspace',
        allowedFolders: ['/workspace'],
        tracer: { withSpan: jest.fn(async (_name, fn) => fn()) }
      });

      // When delegate returns empty groups, should fall back to raw search
      mockSearch.mockResolvedValue('Fallback search results');
      const result = await tool.execute({ query: 'Find nonexistent thing', path: '/workspace' });

      // Falls back to raw search since no groups
      expect(result).toBe('Fallback search results');
      expect(mockSearch).toHaveBeenCalledTimes(1);
    });
  });

  test('delegate prompt includes searches field in output format', async () => {
    mockDelegate.mockResolvedValue(JSON.stringify({
      confidence: 'medium',
      groups: [{ reason: 'test', files: ['a.js#foo'] }],
      searches: [{ query: 'test', path: '.', had_results: true }]
    }));

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/workspace',
      allowedFolders: ['/workspace'],
      tracer: { withSpan: jest.fn(async (_name, fn) => fn()) }
    });

    await tool.execute({ query: 'test', path: '/workspace' });

    // The delegate task prompt should mention searches field
    const delegateCall = mockDelegate.mock.calls[0][0];
    expect(delegateCall.task).toContain('"searches"');
    expect(delegateCall.task).toContain('had_results');
    // Should also mention relevance filtering
    expect(delegateCall.task).toContain('VERIFIED');
    expect(delegateCall.task).toContain('RELEVANCE FILTERING');
  });

  test('uses searchDelegateSchema (only query+path) when searchDelegate=true', async () => {
    mockDelegate.mockResolvedValue(JSON.stringify({
      confidence: 'medium',
      groups: [{ reason: 'test', files: ['a.js#foo'] }],
      searches: []
    }));

    const tool = searchTool({
      searchDelegate: true,
      cwd: '/workspace',
      allowedFolders: ['/workspace'],
      tracer: { withSpan: jest.fn(async (_name, fn) => fn()) }
    });

    // The tool schema should only have query and path for delegate mode
    const schema = tool.inputSchema;
    expect(schema.shape.query).toBeDefined();
    expect(schema.shape.path).toBeDefined();
    // Should NOT have exact, language, nextPage etc in delegate schema
    expect(schema.shape.exact).toBeUndefined();
    expect(schema.shape.language).toBeUndefined();
    expect(schema.shape.nextPage).toBeUndefined();
  });
});
