/**
 * Tests for path resolution in search and extract tools
 * Verifies support for relative paths and comma-separated paths
 * Cross-platform compatible (works on both Unix and Windows)
 */

import { parseTargets, parseAndResolvePaths, resolveTargetPath } from '../../src/tools/common.js';
import { resolve, join } from 'path';
import { tmpdir } from 'os';

describe('Path Resolution', () => {
	describe('parseTargets', () => {
		test('should parse space-separated targets', () => {
			const result = parseTargets('file1.rs file2.rs file3.rs');
			expect(result).toEqual(['file1.rs', 'file2.rs', 'file3.rs']);
		});

		test('should parse comma-separated targets', () => {
			const result = parseTargets('file1.rs,file2.rs,file3.rs');
			expect(result).toEqual(['file1.rs', 'file2.rs', 'file3.rs']);
		});

		test('should parse comma-separated targets with spaces', () => {
			const result = parseTargets('file1.rs, file2.rs, file3.rs');
			expect(result).toEqual(['file1.rs', 'file2.rs', 'file3.rs']);
		});

		test('should parse mixed space and comma-separated targets', () => {
			const result = parseTargets('file1.rs,file2.rs file3.rs, file4.rs');
			expect(result).toEqual(['file1.rs', 'file2.rs', 'file3.rs', 'file4.rs']);
		});

		test('should handle targets with line numbers', () => {
			const result = parseTargets('file1.rs:10, file2.rs:20-30');
			expect(result).toEqual(['file1.rs:10', 'file2.rs:20-30']);
		});

		test('should handle targets with symbols', () => {
			const result = parseTargets('file1.rs#func1, file2.rs#Class.method');
			expect(result).toEqual(['file1.rs#func1', 'file2.rs#Class.method']);
		});

		test('should handle empty string', () => {
			const result = parseTargets('');
			expect(result).toEqual([]);
		});

		test('should handle null/undefined', () => {
			expect(parseTargets(null)).toEqual([]);
			expect(parseTargets(undefined)).toEqual([]);
		});

		test('should filter out empty entries', () => {
			const result = parseTargets('file1.rs,,file2.rs,  ,file3.rs');
			expect(result).toEqual(['file1.rs', 'file2.rs', 'file3.rs']);
		});
	});

	describe('parseAndResolvePaths', () => {
		// Use tmpdir() as a cross-platform absolute path base
		const testCwd = join(tmpdir(), 'test-project');

		test('should resolve relative paths against cwd', () => {
			const result = parseAndResolvePaths('src/file.rs, lib/other.rs', testCwd);

			expect(result).toEqual([
				resolve(testCwd, 'src/file.rs'),
				resolve(testCwd, 'lib/other.rs')
			]);
		});

		test('should not modify absolute paths', () => {
			const absolutePath1 = resolve(tmpdir(), 'absolute/path/file.rs');
			const absolutePath2 = resolve(tmpdir(), 'other/absolute/path.rs');

			const result = parseAndResolvePaths(`${absolutePath1}, ${absolutePath2}`, testCwd);

			expect(result).toEqual([absolutePath1, absolutePath2]);
		});

		test('should handle mixed relative and absolute paths', () => {
			const absolutePath = resolve(tmpdir(), 'absolute/path.rs');

			const result = parseAndResolvePaths(`src/file.rs, ${absolutePath}, lib/other.rs`, testCwd);

			expect(result).toEqual([
				resolve(testCwd, 'src/file.rs'),
				absolutePath,
				resolve(testCwd, 'lib/other.rs')
			]);
		});

		test('should return empty array for empty input', () => {
			expect(parseAndResolvePaths('', testCwd)).toEqual([]);
			expect(parseAndResolvePaths(null, testCwd)).toEqual([]);
			expect(parseAndResolvePaths(undefined, testCwd)).toEqual([]);
		});

		test('should handle paths without cwd', () => {
			const result = parseAndResolvePaths('src/file.rs', null);
			expect(result).toEqual(['src/file.rs']);
		});

		test('should not double-resolve when path equals cwd', () => {
			const relativeCwd = join('project', 'src');
			const expected = resolve(process.cwd(), relativeCwd);
			const result = parseAndResolvePaths(relativeCwd, relativeCwd);
			expect(result).toEqual([expected]);
		});
	});

	describe('resolveTargetPath', () => {
		// Use tmpdir() as a cross-platform absolute path base
		const testCwd = join(tmpdir(), 'project');

		test('should resolve relative path without suffix', () => {
			const result = resolveTargetPath('src/file.rs', testCwd);
			expect(result).toBe(resolve(testCwd, 'src/file.rs'));
		});

		test('should resolve relative path with line number', () => {
			const result = resolveTargetPath('src/file.rs:42', testCwd);
			expect(result).toBe(resolve(testCwd, 'src/file.rs') + ':42');
		});

		test('should resolve relative path with line range', () => {
			const result = resolveTargetPath('src/file.rs:10-20', testCwd);
			expect(result).toBe(resolve(testCwd, 'src/file.rs') + ':10-20');
		});

		test('should resolve relative path with symbol', () => {
			const result = resolveTargetPath('src/file.rs#functionName', testCwd);
			expect(result).toBe(resolve(testCwd, 'src/file.rs') + '#functionName');
		});

		test('should resolve relative path with class.method symbol', () => {
			const result = resolveTargetPath('src/file.rs#Class.method', testCwd);
			expect(result).toBe(resolve(testCwd, 'src/file.rs') + '#Class.method');
		});

		test('should not modify absolute path', () => {
			const absolutePath = resolve(tmpdir(), 'absolute/file.rs');
			const result = resolveTargetPath(absolutePath + ':42', testCwd);
			expect(result).toBe(absolutePath + ':42');
		});

		test('should handle nested relative paths', () => {
			const result = resolveTargetPath('src/deep/nested/file.rs:42', testCwd);
			expect(result).toBe(resolve(testCwd, 'src/deep/nested/file.rs') + ':42');
		});

		test('should handle path without cwd', () => {
			const result = resolveTargetPath('src/file.rs:42', null);
			expect(result).toBe('src/file.rs:42');
		});

		test('should handle absolute path with symbol', () => {
			const absolutePath = resolve(tmpdir(), 'absolute/file.rs');
			const result = resolveTargetPath(absolutePath + '#MyClass.method', testCwd);
			expect(result).toBe(absolutePath + '#MyClass.method');
		});
	});
});
