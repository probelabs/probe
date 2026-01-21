/**
 * Tests for path resolution in search and extract tools
 * Verifies support for relative paths and comma-separated paths
 * Cross-platform compatible (works on both Unix and Windows)
 */

import { parseTargets } from '../../src/tools/common.js';
import { resolve, isAbsolute, join } from 'path';
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

	describe('parseAndResolvePaths helper', () => {
		// Use tmpdir() as a cross-platform absolute path base
		const testCwd = join(tmpdir(), 'test-project');

		test('should resolve relative paths against cwd', () => {
			const paths = 'src/file.rs, lib/other.rs';

			// Parse and resolve manually to test the logic
			const parsed = paths.split(',').map(p => p.trim()).filter(p => p.length > 0);
			const resolved = parsed.map(p => {
				if (isAbsolute(p)) return p;
				return resolve(testCwd, p);
			});

			// Use resolve() to generate expected paths (cross-platform)
			expect(resolved).toEqual([
				resolve(testCwd, 'src/file.rs'),
				resolve(testCwd, 'lib/other.rs')
			]);
		});

		test('should not modify absolute paths', () => {
			// Use platform-specific absolute paths
			const absolutePath1 = resolve(tmpdir(), 'absolute/path/file.rs');
			const absolutePath2 = resolve(tmpdir(), 'other/absolute/path.rs');
			const paths = `${absolutePath1}, ${absolutePath2}`;

			const parsed = paths.split(',').map(p => p.trim()).filter(p => p.length > 0);
			const resolved = parsed.map(p => {
				if (isAbsolute(p)) return p;
				return resolve(testCwd, p);
			});

			expect(resolved).toEqual([
				absolutePath1,
				absolutePath2
			]);
		});

		test('should handle mixed relative and absolute paths', () => {
			const absolutePath = resolve(tmpdir(), 'absolute/path.rs');
			const paths = `src/file.rs, ${absolutePath}, lib/other.rs`;

			const parsed = paths.split(',').map(p => p.trim()).filter(p => p.length > 0);
			const resolved = parsed.map(p => {
				if (isAbsolute(p)) return p;
				return resolve(testCwd, p);
			});

			expect(resolved).toEqual([
				resolve(testCwd, 'src/file.rs'),
				absolutePath,
				resolve(testCwd, 'lib/other.rs')
			]);
		});
	});

	describe('extract target path resolution', () => {
		// Use tmpdir() as a cross-platform absolute path base
		const testCwd = join(tmpdir(), 'project');

		// Helper to simulate the extractTool path resolution logic
		function resolveTargetPath(target, cwd) {
			// On Windows, skip the drive letter colon (e.g., "C:" at index 1)
			// Start searching for line number colon after potential drive letter
			const searchStart = (target.length > 2 && target[1] === ':' && /[a-zA-Z]/.test(target[0])) ? 2 : 0;
			const colonIdx = target.indexOf(':', searchStart);
			const hashIdx = target.indexOf('#');
			let filePart, suffix;

			if (colonIdx !== -1 && (hashIdx === -1 || colonIdx < hashIdx)) {
				filePart = target.substring(0, colonIdx);
				suffix = target.substring(colonIdx);
			} else if (hashIdx !== -1) {
				filePart = target.substring(0, hashIdx);
				suffix = target.substring(hashIdx);
			} else {
				filePart = target;
				suffix = '';
			}

			if (!isAbsolute(filePart) && cwd) {
				filePart = resolve(cwd, filePart);
			}

			return filePart + suffix;
		}

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
	});
});
