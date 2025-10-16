/**
 * Test for extract() function with content parameter
 * This test verifies the fix for the process.env.DEBUG bug
 */

import { extract } from '../../src/index.js';
import path from 'path';

describe('extract() with content parameter', () => {
	// Sample diff content for testing
	const diffContent = `diff --git a/src/main.rs b/src/main.rs
index 123..456
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,3 +10,4 @@
 fn main() {
-    println!("old");
+    println!("new");
 }`;

	test('should process diff content without crashing', async () => {
		// This test verifies that the extract function doesn't crash
		// when accessing process.env.DEBUG
		const result = await extract({
			content: diffContent,
			format: 'outline-xml',
		});

		// Should return a result (string for outline-xml format)
		expect(result).toBeDefined();
		expect(typeof result).toBe('string');
		expect(result.length).toBeGreaterThan(0);
	});

	test('should handle DEBUG environment variable correctly', async () => {
		// Test with DEBUG enabled
		const originalDebug = process.env.DEBUG;
		process.env.DEBUG = '1';

		try {
			const result = await extract({
				content: diffContent,
				format: 'outline-xml',
			});

			expect(result).toBeDefined();
		} finally {
			// Restore original DEBUG value
			if (originalDebug === undefined) {
				delete process.env.DEBUG;
			} else {
				process.env.DEBUG = originalDebug;
			}
		}
	});

	test('should work with outline-xml format', async () => {
		const result = await extract({
			content: diffContent,
			format: 'outline-xml',
		});

		expect(result).toBeDefined();
		expect(typeof result).toBe('string');
		expect(result.length).toBeGreaterThan(0);
	});

	test('should handle errors gracefully', async () => {
		// Test with invalid content
		try {
			await extract({
				content: 'invalid diff content',
				format: 'outline-xml',
			});
			// If it succeeds, that's also acceptable
		} catch (error) {
			// Should throw a proper Error object, not a TypeError about undefined
			expect(error).toBeInstanceOf(Error);
			expect(error.message).not.toContain('Cannot read properties of undefined');
			expect(error.message).not.toContain('process2');
		}
	});
});
