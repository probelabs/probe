/**
 * Tests for cwd/path options in extract, search, and query functions
 * These tests verify that relative paths are resolved against the specified working directory
 */

import { extract, search, query } from '../../src/index.js';
import { validateCwdPath, normalizePath } from '../../src/utils/path-validation.js';
import path from 'path';
import fs from 'fs';
import os from 'os';

// Get the project root directory (where the probe binary can find files)
const projectRoot = path.resolve(process.cwd(), '..');

describe('cwd/path options for workspace isolation', () => {
	describe('extract() with cwd option', () => {
		test('should resolve relative file paths against cwd option', async () => {
			// Use the project root as the base path
			const result = await extract({
				files: ['npm/src/extract.js:1-10'],
				cwd: projectRoot,
				format: 'json'
			});

			expect(result).toBeDefined();
			// Should successfully extract from the file
			if (typeof result === 'object') {
				expect(result.results).toBeDefined();
			}
		});

		test('should work with absolute paths regardless of cwd option', async () => {
			const absolutePath = path.join(projectRoot, 'npm/src/extract.js');

			const result = await extract({
				files: [`${absolutePath}:1-10`],
				cwd: '/tmp', // Different cwd, but absolute paths should still work
				format: 'json'
			});

			expect(result).toBeDefined();
		});

		test('should use process.cwd() when cwd is not specified', async () => {
			// This test verifies default behavior
			const result = await extract({
				files: ['src/extract.js:1-10'],
				format: 'json'
			});

			expect(result).toBeDefined();
		});

		test('should handle content with cwd option', async () => {
			const diffContent = `diff --git a/npm/src/extract.js b/npm/src/extract.js
index 123..456
--- a/npm/src/extract.js
+++ b/npm/src/extract.js
@@ -1,3 +1,4 @@
 /**
- * old comment
+ * new comment
 */`;

			const result = await extract({
				content: diffContent,
				cwd: projectRoot,
				format: 'outline-xml'
			});

			expect(result).toBeDefined();
			expect(typeof result).toBe('string');
		});
	});

	describe('search() with cwd option', () => {
		test('should resolve relative search path against cwd option', async () => {
			const result = await search({
				query: 'extract',
				path: 'npm/src',
				cwd: projectRoot,
				maxResults: 5,
				format: 'json'
			});

			expect(result).toBeDefined();
			if (typeof result === 'object') {
				expect(result.results).toBeDefined();
			}
		});

		test('should work with absolute paths regardless of cwd option', async () => {
			const absolutePath = path.join(projectRoot, 'npm/src');

			const result = await search({
				query: 'function',
				path: absolutePath,
				cwd: '/tmp', // Different cwd, but absolute paths should still work
				maxResults: 5,
				format: 'json'
			});

			expect(result).toBeDefined();
		});

		test('should use process.cwd() when cwd is not specified', async () => {
			const result = await search({
				query: 'extract',
				path: 'src',
				maxResults: 5,
				format: 'json'
			});

			expect(result).toBeDefined();
		});
	});

	describe('query() with cwd option', () => {
		test('should resolve relative query path against cwd option', async () => {
			const result = await query({
				pattern: 'function $NAME($$$ARGS) { $$$BODY }',
				path: 'npm/src',
				cwd: projectRoot,
				language: 'javascript',
				maxResults: 5,
				format: 'json'
			});

			expect(result).toBeDefined();
		});

		test('should work with absolute paths regardless of cwd option', async () => {
			const absolutePath = path.join(projectRoot, 'npm/src');

			const result = await query({
				pattern: 'function $NAME($$$ARGS) { $$$BODY }',
				path: absolutePath,
				cwd: '/tmp', // Different cwd, but absolute paths should still work
				language: 'javascript',
				maxResults: 5,
				format: 'json'
			});

			expect(result).toBeDefined();
		});

		test('should use process.cwd() when cwd is not specified', async () => {
			const result = await query({
				pattern: 'function $NAME($$$ARGS) { $$$BODY }',
				path: 'src',
				language: 'javascript',
				maxResults: 5,
				format: 'json'
			});

			expect(result).toBeDefined();
		});
	});

	describe('workspace isolation scenario', () => {
		let tempWorkspace;

		beforeAll(() => {
			// Create a temporary workspace directory
			tempWorkspace = fs.mkdtempSync(path.join(os.tmpdir(), 'probe-test-workspace-'));

			// Create a symlink to the npm/src directory in the temp workspace
			const symlinkPath = path.join(tempWorkspace, 'probe-npm');
			try {
				fs.symlinkSync(path.join(projectRoot, 'npm'), symlinkPath);
			} catch (e) {
				// Symlink might fail on some systems, skip those tests
				console.warn('Could not create symlink for test:', e.message);
			}
		});

		afterAll(() => {
			// Clean up temporary workspace
			if (tempWorkspace && fs.existsSync(tempWorkspace)) {
				fs.rmSync(tempWorkspace, { recursive: true, force: true });
			}
		});

		test('should extract files from workspace using cwd option', async () => {
			const symlinkPath = path.join(tempWorkspace, 'probe-npm');
			if (!fs.existsSync(symlinkPath)) {
				console.warn('Skipping test: symlink not available');
				return;
			}

			const result = await extract({
				files: ['probe-npm/src/extract.js:1-10'],
				cwd: tempWorkspace,
				format: 'json'
			});

			expect(result).toBeDefined();
		});

		test('should search files in workspace using cwd option', async () => {
			const symlinkPath = path.join(tempWorkspace, 'probe-npm');
			if (!fs.existsSync(symlinkPath)) {
				console.warn('Skipping test: symlink not available');
				return;
			}

			const result = await search({
				query: 'extract',
				path: 'probe-npm/src',
				cwd: tempWorkspace,
				maxResults: 5,
				format: 'json'
			});

			expect(result).toBeDefined();
		});
	});

	describe('path validation security', () => {
		test('validateCwdPath should normalize paths with .. components', async () => {
			const basePath = process.cwd();
			const parentPath = path.dirname(basePath);

			// Path with .. should be normalized
			const result = await validateCwdPath(path.join(basePath, '..'));
			expect(result).toBe(parentPath);
		});

		test('validateCwdPath should return absolute path', async () => {
			const result = await validateCwdPath('.');
			expect(path.isAbsolute(result)).toBe(true);
		});

		test('validateCwdPath should use default when path is not provided', async () => {
			const result = await validateCwdPath(undefined);
			expect(result).toBe(path.normalize(process.cwd()));
		});

		test('validateCwdPath should throw for non-existent path', async () => {
			await expect(
				validateCwdPath('/this/path/definitely/does/not/exist/12345')
			).rejects.toThrow('Path does not exist');
		});

		test('validateCwdPath should throw for file path (not directory)', async () => {
			const filePath = path.join(projectRoot, 'npm/package.json');
			await expect(
				validateCwdPath(filePath)
			).rejects.toThrow('Path is not a directory');
		});

		test('normalizePath should normalize without requiring existence', () => {
			const result = normalizePath('/some/path/../normalized');
			expect(result).toBe(path.normalize('/some/normalized'));
		});

		test('extract should reject non-existent cwd option', async () => {
			await expect(extract({
				files: ['some/file.js'],
				cwd: '/this/path/does/not/exist/12345',
				format: 'json'
			})).rejects.toThrow('Path does not exist');
		});

		test('search should reject non-existent cwd option', async () => {
			await expect(search({
				query: 'test',
				path: '.',
				cwd: '/this/path/does/not/exist/12345',
				format: 'json'
			})).rejects.toThrow('Path does not exist');
		});

		test('query should reject non-existent cwd option', async () => {
			await expect(query({
				pattern: 'function $NAME() {}',
				path: '.',
				cwd: '/this/path/does/not/exist/12345',
				format: 'json'
			})).rejects.toThrow('Path does not exist');
		});
	});
});
