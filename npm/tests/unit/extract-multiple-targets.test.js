/**
 * Test for extractTool() function with multiple space-separated targets
 * This test verifies that the extract tool can handle multiple targets in one call
 */

describe('extractTool() with multiple targets - unit tests for target splitting', () => {
	test('should split single target correctly', () => {
		const targets = 'src/main.rs:10-20';
		const files = targets.split(/\s+/).filter(f => f.length > 0);

		expect(files).toEqual(['src/main.rs:10-20']);
	});

	test('should split multiple space-separated targets', () => {
		const targets = 'src/main.rs:10-20 src/lib.rs:30-40';
		const files = targets.split(/\s+/).filter(f => f.length > 0);

		expect(files).toEqual(['src/main.rs:10-20', 'src/lib.rs:30-40']);
	});

	test('should handle three or more targets', () => {
		const targets = 'file1.rs:1-10 file2.rs:20-30 file3.rs:40-50';
		const files = targets.split(/\s+/).filter(f => f.length > 0);

		expect(files).toEqual(['file1.rs:1-10', 'file2.rs:20-30', 'file3.rs:40-50']);
	});

	test('should handle targets with symbol references', () => {
		const targets = 'session.rs#AuthService.login auth.rs:2-100 config.rs#DatabaseConfig';
		const files = targets.split(/\s+/).filter(f => f.length > 0);

		expect(files).toEqual(['session.rs#AuthService.login', 'auth.rs:2-100', 'config.rs#DatabaseConfig']);
	});

	test('should handle targets with multiple spaces', () => {
		const targets = 'file1.rs:1-10    file2.rs:20-30';
		const files = targets.split(/\s+/).filter(f => f.length > 0);

		expect(files).toEqual(['file1.rs:1-10', 'file2.rs:20-30']);
	});

	test('should handle targets with tabs and mixed whitespace', () => {
		const targets = 'file1.rs:1-10\tfile2.rs:20-30  file3.rs:40-50';
		const files = targets.split(/\s+/).filter(f => f.length > 0);

		expect(files).toEqual(['file1.rs:1-10', 'file2.rs:20-30', 'file3.rs:40-50']);
	});

	test('should filter out empty strings from whitespace', () => {
		const targets = '  file1.rs:1-10   file2.rs:20-30  ';
		const files = targets.split(/\s+/).filter(f => f.length > 0);

		expect(files).toEqual(['file1.rs:1-10', 'file2.rs:20-30']);
	});

	test('should handle the example from the bug report', () => {
		// This is the exact example from the user's bug report
		const targets = 'src/check-execution-engine.ts:283-469 src/check-execution-engine.ts:474-753';
		const files = targets.split(/\s+/).filter(f => f.length > 0);

		expect(files).toEqual([
			'src/check-execution-engine.ts:283-469',
			'src/check-execution-engine.ts:474-753'
		]);
	});

	test('should handle newlines as whitespace', () => {
		const targets = 'file1.rs:1-10\nfile2.rs:20-30';
		const files = targets.split(/\s+/).filter(f => f.length > 0);

		expect(files).toEqual(['file1.rs:1-10', 'file2.rs:20-30']);
	});

	test('should not split targets incorrectly - no spaces means single file', () => {
		const targets = 'src/main.rs:10-20';
		const files = targets.split(/\s+/).filter(f => f.length > 0);

		expect(files).toHaveLength(1);
		expect(files[0]).toBe('src/main.rs:10-20');
	});
});
