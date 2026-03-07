/**
 * Tests for auto-fixing space-separated file paths.
 *
 * Models sometimes pass multiple space-separated file paths as a single string
 * (e.g. "src/ranking.rs src/simd_ranking.rs"). Both parseAndResolvePaths and
 * normalizeTargets should split these into separate entries.
 */

import { parseAndResolvePaths } from '../../src/tools/common.js';

describe('Space-separated paths auto-fix', () => {
	describe('parseAndResolvePaths', () => {
		test('should split space-separated file paths', () => {
			const result = parseAndResolvePaths('src/ranking.rs src/simd_ranking.rs', '/workspace');
			expect(result).toEqual([
				'/workspace/src/ranking.rs',
				'/workspace/src/simd_ranking.rs',
			]);
		});

		test('should split space-separated files without path separators but with extensions', () => {
			const result = parseAndResolvePaths('CLAUDE.md ARCHITECTURE.md', '/workspace');
			expect(result).toEqual([
				'/workspace/CLAUDE.md',
				'/workspace/ARCHITECTURE.md',
			]);
		});

		test('should not split a single path', () => {
			const result = parseAndResolvePaths('src/ranking.rs', '/workspace');
			expect(result).toEqual(['/workspace/src/ranking.rs']);
		});

		test('should not split paths with spaces that do not look like file paths', () => {
			const result = parseAndResolvePaths('some random text', '/workspace');
			expect(result).toEqual(['/workspace/some random text']);
		});

		test('should still support comma-separated paths', () => {
			const result = parseAndResolvePaths('src/a.rs, src/b.rs', '/workspace');
			expect(result).toEqual([
				'/workspace/src/a.rs',
				'/workspace/src/b.rs',
			]);
		});

		test('should handle absolute paths with spaces', () => {
			const result = parseAndResolvePaths('/home/user/src/a.rs /home/user/src/b.rs', '/workspace');
			expect(result).toEqual([
				'/home/user/src/a.rs',
				'/home/user/src/b.rs',
			]);
		});
	});
});
