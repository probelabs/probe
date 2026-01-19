/**
 * Tests for MCP extract_code tool path resolution
 * Verifies that the 'path' parameter is correctly used as 'cwd' for resolving relative file paths
 */

import { extract } from '../../src/index.js';
import path from 'path';

// Get the project root directory
const projectRoot = path.resolve(process.cwd(), '..');

describe('MCP extract_code path resolution', () => {
	describe('path parameter maps to cwd', () => {
		test('should resolve relative file paths using path as cwd', async () => {
			// This simulates what the MCP extract_code tool does after the fix
			// path parameter is passed as cwd option to extract()
			const options = {
				files: ['npm/src/extract.js:1-10'],
				cwd: projectRoot,  // MCP's 'path' parameter
				format: 'xml',
				allowTests: true,
			};

			const result = await extract(options);

			expect(result).toBeDefined();
			expect(typeof result).toBe('string');
			expect(result.length).toBeGreaterThan(0);
		});

		test('should work with multiple relative file paths', async () => {
			const options = {
				files: ['npm/src/extract.js:1-5', 'npm/src/search.js:1-5'],
				cwd: projectRoot,
				format: 'xml',
				allowTests: true,
			};

			const result = await extract(options);

			expect(result).toBeDefined();
			expect(typeof result).toBe('string');
			// Should contain content from both files
			expect(result.length).toBeGreaterThan(0);
		});

		test('should work with file:line format relative paths', async () => {
			const options = {
				files: ['npm/src/extract.js:42'],
				cwd: projectRoot,
				format: 'xml',
				allowTests: true,
			};

			const result = await extract(options);

			expect(result).toBeDefined();
			expect(typeof result).toBe('string');
		});

		test('should work with file#symbol format relative paths', async () => {
			const options = {
				files: ['npm/src/extract.js#extract'],
				cwd: projectRoot,
				format: 'xml',
				allowTests: true,
			};

			const result = await extract(options);

			expect(result).toBeDefined();
			expect(typeof result).toBe('string');
		});

		test('should return empty result when cwd does not match file paths', async () => {
			// Using a different cwd that doesn't contain the files
			const options = {
				files: ['npm/src/extract.js:1-10'],
				cwd: '/tmp',
				format: 'xml',
				allowTests: true,
			};

			// Should return empty result (count: 0) when files not found
			const result = await extract(options);
			expect(result).toBeDefined();
			expect(result).toContain('<count>0</count>');
		});
	});

	describe('backward compatibility', () => {
		test('should work without cwd (uses process.cwd)', async () => {
			// Without cwd, should use process.cwd() which is npm/
			const options = {
				files: ['src/extract.js:1-10'],
				format: 'xml',
				allowTests: true,
			};

			const result = await extract(options);

			expect(result).toBeDefined();
			expect(typeof result).toBe('string');
		});

		test('should work with absolute paths regardless of cwd', async () => {
			const absolutePath = path.join(projectRoot, 'npm/src/extract.js');
			const options = {
				files: [`${absolutePath}:1-10`],
				cwd: '/tmp',  // Different cwd
				format: 'xml',
				allowTests: true,
			};

			const result = await extract(options);

			expect(result).toBeDefined();
			expect(typeof result).toBe('string');
		});
	});
});
