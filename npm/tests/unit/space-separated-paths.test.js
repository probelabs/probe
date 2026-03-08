/**
 * Tests for auto-fixing space-separated file paths.
 *
 * Models sometimes pass multiple space-separated file paths as a single string
 * (e.g. "src/ranking.rs src/simd_ranking.rs"). Both parseAndResolvePaths and
 * normalizeTargets should split these into separate entries.
 */

import path from 'path';
import { parseAndResolvePaths } from '../../src/tools/common.js';

// Helper: build expected platform-specific resolved path
const resolve = (...args) => path.resolve(...args);

describe('Space-separated paths auto-fix', () => {
	describe('parseAndResolvePaths', () => {
		test('should split space-separated file paths', () => {
			const result = parseAndResolvePaths('src/ranking.rs src/simd_ranking.rs', '/workspace');
			expect(result).toEqual([
				resolve('/workspace', 'src/ranking.rs'),
				resolve('/workspace', 'src/simd_ranking.rs'),
			]);
		});

		test('should split space-separated files without path separators but with extensions', () => {
			const result = parseAndResolvePaths('CLAUDE.md ARCHITECTURE.md', '/workspace');
			expect(result).toEqual([
				resolve('/workspace', 'CLAUDE.md'),
				resolve('/workspace', 'ARCHITECTURE.md'),
			]);
		});

		test('should not split a single path', () => {
			const result = parseAndResolvePaths('src/ranking.rs', '/workspace');
			expect(result).toEqual([resolve('/workspace', 'src/ranking.rs')]);
		});

		test('should not split paths with spaces that do not look like file paths', () => {
			const result = parseAndResolvePaths('some random text', '/workspace');
			expect(result).toEqual([resolve('/workspace', 'some random text')]);
		});

		test('should still support comma-separated paths', () => {
			const result = parseAndResolvePaths('src/a.rs, src/b.rs', '/workspace');
			expect(result).toEqual([
				resolve('/workspace', 'src/a.rs'),
				resolve('/workspace', 'src/b.rs'),
			]);
		});

		test('should handle absolute paths with spaces', () => {
			const result = parseAndResolvePaths('/home/user/src/a.rs /home/user/src/b.rs', '/workspace');
			expect(result).toEqual([
				resolve('/home/user/src/a.rs'),
				resolve('/home/user/src/b.rs'),
			]);
		});
	});
});
