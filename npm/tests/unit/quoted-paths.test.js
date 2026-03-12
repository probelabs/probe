/**
 * Tests for quoted path handling in parseTargets, splitQuotedString, and parseAndResolvePaths.
 *
 * File paths with spaces must be quoted (single or double) to be treated as
 * a single target. See: https://github.com/probelabs/probe/issues/519
 */

import path from 'path';
import { parseTargets, splitQuotedString, parseAndResolvePaths } from '../../src/tools/common.js';

const resolve = (...args) => path.resolve(...args);

describe('splitQuotedString', () => {
	test('splits simple space-separated tokens', () => {
		expect(splitQuotedString('a b c')).toEqual(['a', 'b', 'c']);
	});

	test('splits comma-separated tokens', () => {
		expect(splitQuotedString('a,b,c')).toEqual(['a', 'b', 'c']);
	});

	test('splits mixed comma and space-separated tokens', () => {
		expect(splitQuotedString('a, b c')).toEqual(['a', 'b', 'c']);
	});

	test('preserves double-quoted strings with spaces', () => {
		expect(splitQuotedString('"path with spaces/file.md" other.rs'))
			.toEqual(['path with spaces/file.md', 'other.rs']);
	});

	test('preserves single-quoted strings with spaces', () => {
		expect(splitQuotedString("'path with spaces/file.md' other.rs"))
			.toEqual(['path with spaces/file.md', 'other.rs']);
	});

	test('handles escaped characters in quotes', () => {
		expect(splitQuotedString('"path\\"with\\"quotes" other'))
			.toEqual(['path"with"quotes', 'other']);
	});

	test('handles multiple quoted strings', () => {
		expect(splitQuotedString('"first path/a.md" "second path/b.md"'))
			.toEqual(['first path/a.md', 'second path/b.md']);
	});

	test('handles mixed quoted and unquoted', () => {
		expect(splitQuotedString('plain.rs "path with spaces/file.md" another.js'))
			.toEqual(['plain.rs', 'path with spaces/file.md', 'another.js']);
	});

	test('handles empty string', () => {
		expect(splitQuotedString('')).toEqual([]);
	});

	test('handles only whitespace', () => {
		expect(splitQuotedString('   ')).toEqual([]);
	});

	test('handles quoted string with commas inside', () => {
		expect(splitQuotedString('"file,with,commas.md" other.rs'))
			.toEqual(['file,with,commas.md', 'other.rs']);
	});
});

describe('parseTargets with quoted paths', () => {
	test('preserves quoted path with spaces (issue #519)', () => {
		const result = parseTargets('"Customers/First American/drive/First American Meeting Notes.md"');
		expect(result).toEqual(['Customers/First American/drive/First American Meeting Notes.md']);
	});

	test('handles quoted path alongside unquoted targets', () => {
		const result = parseTargets('"Customers/First American/Meeting Notes.md" other.rs:10-20');
		expect(result).toEqual(['Customers/First American/Meeting Notes.md', 'other.rs:10-20']);
	});

	test('handles quoted path with symbol suffix outside quotes', () => {
		const result = parseTargets('"path with spaces/file.rs"#MySymbol');
		expect(result).toEqual(['path with spaces/file.rs#MySymbol']);
	});

	test('still works for simple unquoted targets', () => {
		expect(parseTargets('file1.rs:10 file2.rs:20'))
			.toEqual(['file1.rs:10', 'file2.rs:20']);
	});

	test('still works for comma-separated targets', () => {
		expect(parseTargets('file1.rs, file2.rs'))
			.toEqual(['file1.rs', 'file2.rs']);
	});
});

describe('parseAndResolvePaths with quoted paths', () => {
	test('preserves quoted path with spaces', () => {
		const result = parseAndResolvePaths('"Customers/First American/Notes.md"', '/workspace');
		expect(result).toEqual([resolve('/workspace', 'Customers/First American/Notes.md')]);
	});

	test('handles quoted path alongside unquoted paths', () => {
		const result = parseAndResolvePaths('"path with spaces/a.md", src/b.rs', '/workspace');
		expect(result).toEqual([
			resolve('/workspace', 'path with spaces/a.md'),
			resolve('/workspace', 'src/b.rs'),
		]);
	});

	test('handles quoted absolute path with spaces', () => {
		const result = parseAndResolvePaths('"/home/user/My Documents/file.md"', '/workspace');
		expect(result).toEqual(['/home/user/My Documents/file.md']);
	});

	test('still splits unquoted space-separated paths that look like files', () => {
		const result = parseAndResolvePaths('src/ranking.rs src/simd_ranking.rs', '/workspace');
		expect(result).toEqual([
			resolve('/workspace', 'src/ranking.rs'),
			resolve('/workspace', 'src/simd_ranking.rs'),
		]);
	});

	test('still does not split unquoted paths that do not look like files', () => {
		const result = parseAndResolvePaths('some random text', '/workspace');
		expect(result).toEqual([resolve('/workspace', 'some random text')]);
	});
});
