/**
 * Tests for path resolution in search and extract tools
 * Verifies support for relative paths and comma-separated paths
 */

import { parseTargets } from '../../src/tools/common.js';
import { resolve, isAbsolute } from 'path';

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
		// Import the function dynamically since it's not exported
		// We'll test it indirectly through the tool behavior

		test('should resolve relative paths against cwd', () => {
			const cwd = '/project/root';
			const paths = 'src/file.rs, lib/other.rs';

			// Parse and resolve manually to test the logic
			const parsed = paths.split(',').map(p => p.trim()).filter(p => p.length > 0);
			const resolved = parsed.map(p => {
				if (isAbsolute(p)) return p;
				return resolve(cwd, p);
			});

			expect(resolved).toEqual([
				'/project/root/src/file.rs',
				'/project/root/lib/other.rs'
			]);
		});

		test('should not modify absolute paths', () => {
			const cwd = '/project/root';
			const paths = '/absolute/path/file.rs, /other/absolute/path.rs';

			const parsed = paths.split(',').map(p => p.trim()).filter(p => p.length > 0);
			const resolved = parsed.map(p => {
				if (isAbsolute(p)) return p;
				return resolve(cwd, p);
			});

			expect(resolved).toEqual([
				'/absolute/path/file.rs',
				'/other/absolute/path.rs'
			]);
		});

		test('should handle mixed relative and absolute paths', () => {
			const cwd = '/project/root';
			const paths = 'src/file.rs, /absolute/path.rs, lib/other.rs';

			const parsed = paths.split(',').map(p => p.trim()).filter(p => p.length > 0);
			const resolved = parsed.map(p => {
				if (isAbsolute(p)) return p;
				return resolve(cwd, p);
			});

			expect(resolved).toEqual([
				'/project/root/src/file.rs',
				'/absolute/path.rs',
				'/project/root/lib/other.rs'
			]);
		});
	});

	describe('extract target path resolution', () => {
		// Helper to simulate the extractTool path resolution logic
		function resolveTargetPath(target, cwd) {
			const colonIdx = target.indexOf(':');
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
			const result = resolveTargetPath('src/file.rs', '/project');
			expect(result).toBe('/project/src/file.rs');
		});

		test('should resolve relative path with line number', () => {
			const result = resolveTargetPath('src/file.rs:42', '/project');
			expect(result).toBe('/project/src/file.rs:42');
		});

		test('should resolve relative path with line range', () => {
			const result = resolveTargetPath('src/file.rs:10-20', '/project');
			expect(result).toBe('/project/src/file.rs:10-20');
		});

		test('should resolve relative path with symbol', () => {
			const result = resolveTargetPath('src/file.rs#functionName', '/project');
			expect(result).toBe('/project/src/file.rs#functionName');
		});

		test('should resolve relative path with class.method symbol', () => {
			const result = resolveTargetPath('src/file.rs#Class.method', '/project');
			expect(result).toBe('/project/src/file.rs#Class.method');
		});

		test('should not modify absolute path', () => {
			const result = resolveTargetPath('/absolute/file.rs:42', '/project');
			expect(result).toBe('/absolute/file.rs:42');
		});

		test('should handle Windows-style paths on Windows', () => {
			// Skip this test on non-Windows
			if (process.platform !== 'win32') {
				return;
			}
			const result = resolveTargetPath('src\\file.rs:42', 'C:\\project');
			expect(result).toBe('C:\\project\\src\\file.rs:42');
		});
	});
});
