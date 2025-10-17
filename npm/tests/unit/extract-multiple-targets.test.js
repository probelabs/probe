/**
 * Test for parseTargets() utility and extract tool integration
 * This test verifies that the extract tool can handle multiple targets in one call
 */

import { parseTargets } from '../../src/tools/common.js';

describe('parseTargets() utility function', () => {
	test('should split single target correctly', () => {
		const files = parseTargets('src/main.rs:10-20');

		expect(files).toEqual(['src/main.rs:10-20']);
	});

	test('should split multiple space-separated targets', () => {
		const files = parseTargets('src/main.rs:10-20 src/lib.rs:30-40');

		expect(files).toEqual(['src/main.rs:10-20', 'src/lib.rs:30-40']);
	});

	test('should handle three or more targets', () => {
		const files = parseTargets('file1.rs:1-10 file2.rs:20-30 file3.rs:40-50');

		expect(files).toEqual(['file1.rs:1-10', 'file2.rs:20-30', 'file3.rs:40-50']);
	});

	test('should handle targets with symbol references', () => {
		const files = parseTargets('session.rs#AuthService.login auth.rs:2-100 config.rs#DatabaseConfig');

		expect(files).toEqual(['session.rs#AuthService.login', 'auth.rs:2-100', 'config.rs#DatabaseConfig']);
	});

	test('should handle targets with multiple spaces', () => {
		const files = parseTargets('file1.rs:1-10    file2.rs:20-30');

		expect(files).toEqual(['file1.rs:1-10', 'file2.rs:20-30']);
	});

	test('should handle targets with tabs and mixed whitespace', () => {
		const files = parseTargets('file1.rs:1-10\tfile2.rs:20-30  file3.rs:40-50');

		expect(files).toEqual(['file1.rs:1-10', 'file2.rs:20-30', 'file3.rs:40-50']);
	});

	test('should filter out empty strings from whitespace', () => {
		const files = parseTargets('  file1.rs:1-10   file2.rs:20-30  ');

		expect(files).toEqual(['file1.rs:1-10', 'file2.rs:20-30']);
	});

	test('should handle the example from the bug report', () => {
		// This is the exact example from the user's bug report
		const files = parseTargets('src/check-execution-engine.ts:283-469 src/check-execution-engine.ts:474-753');

		expect(files).toEqual([
			'src/check-execution-engine.ts:283-469',
			'src/check-execution-engine.ts:474-753'
		]);
	});

	test('should handle newlines as whitespace', () => {
		const files = parseTargets('file1.rs:1-10\nfile2.rs:20-30');

		expect(files).toEqual(['file1.rs:1-10', 'file2.rs:20-30']);
	});

	test('should not split targets incorrectly - no spaces means single file', () => {
		const files = parseTargets('src/main.rs:10-20');

		expect(files).toHaveLength(1);
		expect(files[0]).toBe('src/main.rs:10-20');
	});

	test('should handle null or undefined input', () => {
		expect(parseTargets(null)).toEqual([]);
		expect(parseTargets(undefined)).toEqual([]);
	});

	test('should handle empty string', () => {
		expect(parseTargets('')).toEqual([]);
		expect(parseTargets('   ')).toEqual([]);
	});

	test('should handle non-string input gracefully', () => {
		expect(parseTargets(123)).toEqual([]);
		expect(parseTargets({})).toEqual([]);
		expect(parseTargets([])).toEqual([]);
	});
});
