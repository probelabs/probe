/**
 * Unit tests for allow_tests default behavior in Vercel AI SDK tools
 *
 * Issue #323: allow_tests parameter has a documented default of true,
 * but this default was not applied when AI tools omit the parameter.
 *
 * @module tests/allow-tests-default
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';

describe('allow_tests Default Behavior', () => {
	describe('Zod Schema Defaults', () => {
		test('querySchema should have allow_tests default to true', async () => {
			const { querySchema } = await import('../src/tools/common.js');

			// Parse an empty object - should get default value
			const result = querySchema.parse({ pattern: 'test' });

			expect(result.allow_tests).toBe(true);
		});

		test('extractSchema should have allow_tests default to true', async () => {
			const { extractSchema } = await import('../src/tools/common.js');

			// Parse with only required field
			const result = extractSchema.parse({ targets: 'file.js' });

			expect(result.allow_tests).toBe(true);
		});

		test('querySchema should respect explicit false value', async () => {
			const { querySchema } = await import('../src/tools/common.js');

			const result = querySchema.parse({ pattern: 'test', allow_tests: false });

			expect(result.allow_tests).toBe(false);
		});

		test('extractSchema should respect explicit false value', async () => {
			const { extractSchema } = await import('../src/tools/common.js');

			const result = extractSchema.parse({ targets: 'file.js', allow_tests: false });

			expect(result.allow_tests).toBe(false);
		});
	});

	describe('Default Application Logic', () => {
		/**
		 * Tests the default application logic that should be used in vercel.js tools.
		 * The fix uses nullish coalescing: allowTests: allow_tests ?? true
		 * This handles both undefined and null as "not specified"
		 */
		function applyAllowTestsDefault(allow_tests) {
			return allow_tests ?? true;
		}

		test('should return true when allow_tests is undefined', () => {
			expect(applyAllowTestsDefault(undefined)).toBe(true);
		});

		test('should return true when allow_tests is explicitly true', () => {
			expect(applyAllowTestsDefault(true)).toBe(true);
		});

		test('should return false when allow_tests is explicitly false', () => {
			expect(applyAllowTestsDefault(false)).toBe(false);
		});

		test('should return true when allow_tests is null (treated as not specified)', () => {
			// null should be treated the same as undefined - use default value
			expect(applyAllowTestsDefault(null)).toBe(true);
		});
	});

	describe('Vercel Tool Implementation Verification', () => {
		/**
		 * Verify that the vercel.js file contains the correct default application pattern.
		 * This is a code inspection test to catch regressions.
		 */
		test('searchTool should apply default for allowTests', async () => {
			const fs = await import('fs');
			const path = await import('path');
			const { fileURLToPath } = await import('url');
			const __dirname = path.default.dirname(fileURLToPath(import.meta.url));
			const vercelPath = path.default.resolve(__dirname, '../src/tools/vercel.js');
			const vercelSource = fs.default.readFileSync(vercelPath, 'utf-8');

			// Check that the searchTool uses the nullish coalescing pattern
			expect(vercelSource).toContain('allowTests: allow_tests ?? true');
		});

		test('queryTool should apply default for allowTests', async () => {
			const fs = await import('fs');
			const path = await import('path');
			const { fileURLToPath } = await import('url');
			const __dirname = path.default.dirname(fileURLToPath(import.meta.url));
			const vercelPath = path.default.resolve(__dirname, '../src/tools/vercel.js');
			const vercelSource = fs.default.readFileSync(vercelPath, 'utf-8');

			// The queryTool should have the pattern somewhere in its implementation
			// Count occurrences to ensure all tools have the fix
			const matches = vercelSource.match(/allowTests: allow_tests \?\? true/g);
			// Should have at least 4 occurrences (search, query, extract with targets, extract with input_content)
			expect(matches).not.toBeNull();
			expect(matches.length).toBeGreaterThanOrEqual(4);
		});

		test('extractTool should apply default for allowTests in both branches', async () => {
			const fs = await import('fs');
			const path = await import('path');
			const { fileURLToPath } = await import('url');
			const __dirname = path.default.dirname(fileURLToPath(import.meta.url));
			const vercelPath = path.default.resolve(__dirname, '../src/tools/vercel.js');
			const vercelSource = fs.default.readFileSync(vercelPath, 'utf-8');

			// Extract tool has two places where allowTests is set (targets and input_content branches)
			// Both should use the nullish coalescing pattern
			const extractSection = vercelSource.slice(
				vercelSource.indexOf('export const extractTool'),
				vercelSource.indexOf('export const delegateTool')
			);

			const matches = extractSection.match(/allowTests: allow_tests \?\? true/g);
			expect(matches).not.toBeNull();
			expect(matches.length).toBe(2); // One for targets branch, one for input_content branch
		});
	});

	describe('LangChain Tool Implementation Verification', () => {
		/**
		 * Verify that the langchain.js file contains the correct default application pattern.
		 * This is a code inspection test to catch regressions.
		 */
		test('LangChain tools should apply default for allowTests', async () => {
			const fs = await import('fs');
			const path = await import('path');
			const { fileURLToPath } = await import('url');
			const __dirname = path.default.dirname(fileURLToPath(import.meta.url));
			const langchainPath = path.default.resolve(__dirname, '../src/tools/langchain.js');
			const langchainSource = fs.default.readFileSync(langchainPath, 'utf-8');

			// Count occurrences to ensure all tools have the fix
			const matches = langchainSource.match(/allowTests: allow_tests \?\? true/g);
			// Should have 3 occurrences (search, query, extract)
			expect(matches).not.toBeNull();
			expect(matches.length).toBe(3);
		});
	});
});
