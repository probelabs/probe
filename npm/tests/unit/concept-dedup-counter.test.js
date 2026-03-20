/**
 * Tests for the concept dedup counter and circuit breaker in the search tool.
 * Verifies the exact counting sequence: 2 real failures → 3rd blocked,
 * and that no double-counting occurs between the block path and track path.
 */
import { describe, test, expect, jest, beforeEach } from '@jest/globals';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Mock the 'ai' package
jest.mock('ai', () => ({
	tool: jest.fn((config) => ({
		name: config.name,
		description: config.description,
		inputSchema: config.inputSchema,
		execute: config.execute
	}))
}));

// Mock search to return controlled results
let mockSearchResult = 'No results found';
const mockSearch = jest.fn(async () => mockSearchResult);

jest.unstable_mockModule(resolve(__dirname, '../../src/search.js'), () => ({
	search: mockSearch
}));
jest.unstable_mockModule(resolve(__dirname, '../../src/extract.js'), () => ({
	extract: jest.fn()
}));
jest.unstable_mockModule(resolve(__dirname, '../../src/delegate.js'), () => ({
	delegate: jest.fn()
}));
jest.unstable_mockModule(resolve(__dirname, '../../src/tools/analyzeAll.js'), () => ({
	analyzeAll: jest.fn()
}));

const { searchTool } = await import('../../src/tools/vercel.js');

describe('Concept dedup counter', () => {
	let tool;

	beforeEach(() => {
		mockSearchResult = 'No results found';
		mockSearch.mockClear();
		// Create a fresh search tool each test (fresh internal counters)
		tool = searchTool({ cwd: '/test', debug: false });
	});

	test('allows first two syntax variations, blocks the third', async () => {
		// Search 1: "ctxGetData" — real search runs, no results
		const r1 = await tool.execute({ query: 'ctxGetData', path: '/test' });
		expect(r1).toContain('No results found');
		expect(r1).not.toContain('CONCEPT ALREADY FAILED');
		expect(mockSearch).toHaveBeenCalledTimes(1);

		// Search 2: "ctx.GetData" — different raw query, same normalized concept — real search runs
		const r2 = await tool.execute({ query: 'ctx.GetData', path: '/test' });
		expect(r2).toContain('No results found');
		expect(r2).not.toContain('CONCEPT ALREADY FAILED');
		expect(mockSearch).toHaveBeenCalledTimes(2);

		// Search 3: "ctx_get_data" — 3rd attempt — BLOCKED, search NOT called
		const r3 = await tool.execute({ query: 'ctx_get_data', path: '/test' });
		expect(r3).toContain('CONCEPT ALREADY FAILED');
		expect(r3).toContain('3 variations tried');
		expect(mockSearch).toHaveBeenCalledTimes(2); // NOT called again
	});

	test('4th and 5th attempts also blocked with correct counts (no double-counting)', async () => {
		await tool.execute({ query: 'ctxGetData', path: '/test' });
		await tool.execute({ query: 'ctx.GetData', path: '/test' });
		expect(mockSearch).toHaveBeenCalledTimes(2);

		// 3rd — blocked, count = 3
		const r3 = await tool.execute({ query: 'ctx_get_data', path: '/test' });
		expect(r3).toContain('3 variations tried');

		// 4th — blocked, count = 4 (kebab-case variant, unique raw query)
		const r4 = await tool.execute({ query: 'ctx-get-data', path: '/test' });
		expect(r4).toContain('4 variations tried');

		// 5th — blocked, count = 5 (UPPER_SNAKE, unique raw query)
		const r5 = await tool.execute({ query: 'CTX_GET_DATA', path: '/test' });
		expect(r5).toContain('5 variations tried');

		// Search was never called after the 2nd — only 2 real searches ran
		expect(mockSearch).toHaveBeenCalledTimes(2);
	});

	test('different concepts are tracked independently', async () => {
		// Fail concept A twice
		await tool.execute({ query: 'ctxGetData', path: '/test' });
		await tool.execute({ query: 'ctx.GetData', path: '/test' });

		// Concept B should still be allowed (different normalized key)
		const r = await tool.execute({ query: 'handleRequest', path: '/test' });
		expect(r).toContain('No results found');
		expect(r).not.toContain('CONCEPT ALREADY FAILED');
		expect(mockSearch).toHaveBeenCalledTimes(3); // all 3 ran real searches
	});

	test('same query with different language param is not deduped', async () => {
		// Search for "class Foo" in python
		await tool.execute({ query: 'class Foo', path: '/test', language: 'python' });

		// Same query in java — different language, should NOT be blocked
		const r = await tool.execute({ query: 'class Foo', path: '/test', language: 'java' });
		expect(r).toContain('No results found');
		expect(r).not.toContain('DUPLICATE SEARCH BLOCKED');
		expect(mockSearch).toHaveBeenCalledTimes(2);
	});

	test('same concept in different paths tracked independently', async () => {
		// Fail in path /test/src twice
		await tool.execute({ query: 'ctxGetData', path: '/test/src' });
		await tool.execute({ query: 'ctx.GetData', path: '/test/src' });

		// Same concept in /test/lib — different path, should be allowed
		const r = await tool.execute({ query: 'ctxGetData', path: '/test/lib' });
		expect(r).toContain('No results found');
		expect(r).not.toContain('CONCEPT ALREADY FAILED');
	});

	test('successful search resets consecutive no-results counter', async () => {
		// 3 consecutive no-results
		await tool.execute({ query: 'missing1', path: '/test' });
		await tool.execute({ query: 'missing2', path: '/test' });
		await tool.execute({ query: 'missing3', path: '/test' });

		// Successful search resets the counter
		mockSearchResult = 'Found: function handleRequest() { ... }';
		await tool.execute({ query: 'handleRequest', path: '/test' });

		// After reset, a new no-result search should NOT trigger circuit breaker
		mockSearchResult = 'No results found';
		const r = await tool.execute({ query: 'anotherMissing', path: '/test' });
		expect(r).toContain('No results found');
		expect(r).not.toContain('CIRCUIT BREAKER');
	});

	test('circuit breaker warns after 4 consecutive no-result searches but still executes', async () => {
		// 4 consecutive no-result searches with different concepts
		await tool.execute({ query: 'missing1', path: '/test' });
		await tool.execute({ query: 'missing2', path: '/test' });
		await tool.execute({ query: 'missing3', path: '/test' });
		await tool.execute({ query: 'missing4', path: '/test' });

		// 5th gets circuit breaker WARNING appended, but search still runs
		const r = await tool.execute({ query: 'missing5', path: '/test' });
		expect(r).toContain('CIRCUIT BREAKER');
		expect(r).toContain('No results found');
		// Search WAS executed — circuit breaker is non-blocking
		expect(mockSearch).toHaveBeenCalledTimes(5);
	});

	test('circuit breaker resets when a search succeeds after threshold', async () => {
		// 4 consecutive no-result searches
		await tool.execute({ query: 'missing1', path: '/test' });
		await tool.execute({ query: 'missing2', path: '/test' });
		await tool.execute({ query: 'missing3', path: '/test' });
		await tool.execute({ query: 'missing4', path: '/test' });
		expect(mockSearch).toHaveBeenCalledTimes(4);

		// 5th search succeeds — should reset the counter
		mockSearchResult = 'Found: function realCode() { ... }';
		const r5 = await tool.execute({ query: 'realCode', path: '/test' });
		expect(r5).toContain('Found: function realCode');
		expect(mockSearch).toHaveBeenCalledTimes(5);

		// After reset, new no-result search should NOT have circuit breaker warning
		mockSearchResult = 'No results found';
		const r6 = await tool.execute({ query: 'anotherMissing', path: '/test' });
		expect(r6).toContain('No results found');
		expect(r6).not.toContain('CIRCUIT BREAKER');
	});

	test('normalizeQueryConcept strips filler prefixes for better dedup', async () => {
		// "wrapToolWithEmitter" should be treated as same concept as "definition of wrapToolWithEmitter"
		await tool.execute({ query: 'wrapToolWithEmitter', path: '/test' });
		await tool.execute({ query: 'definition of wrapToolWithEmitter', path: '/test' });

		// 3rd attempt with "find wrapToolWithEmitter" — same concept, should be blocked
		const r = await tool.execute({ query: 'find wrapToolWithEmitter', path: '/test' });
		expect(r).toContain('CONCEPT ALREADY FAILED');

		// "where is wrapToolWithEmitter" — also same concept
		const r2 = await tool.execute({ query: 'where is wrapToolWithEmitter', path: '/test' });
		expect(r2).toContain('CONCEPT ALREADY FAILED');
	});

	test('concept dedup fires before circuit breaker when applicable', async () => {
		// Same concept twice
		await tool.execute({ query: 'getData', path: '/test' });
		await tool.execute({ query: 'get.Data', path: '/test' });

		// 3rd attempt of same concept — concept dedup fires (not circuit breaker)
		const r = await tool.execute({ query: 'get_data', path: '/test' });
		expect(r).toContain('CONCEPT ALREADY FAILED');
		expect(r).not.toContain('CIRCUIT BREAKER');
	});
});
