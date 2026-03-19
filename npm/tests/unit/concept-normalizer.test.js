/**
 * Tests for the concept normalizer used in fuzzy search dedup.
 * Validates that syntax variations of the same concept collide,
 * while genuinely different concepts do NOT.
 */
import { describe, test, expect } from '@jest/globals';

// Mirror of normalizeQueryConcept from vercel.js (it's closure-scoped, so we replicate it)
function normalizeQueryConcept(query) {
	if (!query) return '';
	return query
		.replace(/^["']|["']$/g, '')      // strip outer quotes
		.replace(/\./g, '')                 // "ctx.GetData" → "ctxGetData"
		.replace(/[_\-\s]+/g, '')           // strip underscores/hyphens/spaces
		.toLowerCase()
		.trim();
}

describe('normalizeQueryConcept', () => {
	// ---- These SHOULD collide (same concept, syntax variations) ----

	describe('correctly collapses syntax variations of the same concept', () => {
		test('quoted vs unquoted', () => {
			expect(normalizeQueryConcept('"ctxGetData"')).toBe(normalizeQueryConcept('ctxGetData'));
			expect(normalizeQueryConcept("'ctxGetData'")).toBe(normalizeQueryConcept('ctxGetData'));
		});

		test('dot notation vs camelCase', () => {
			expect(normalizeQueryConcept('ctx.GetData')).toBe(normalizeQueryConcept('ctxGetData'));
		});

		test('snake_case vs camelCase', () => {
			expect(normalizeQueryConcept('ctx_get_data')).toBe(normalizeQueryConcept('ctxGetData'));
		});

		test('kebab-case vs camelCase', () => {
			expect(normalizeQueryConcept('ctx-get-data')).toBe(normalizeQueryConcept('ctxGetData'));
		});

		test('spaces vs camelCase', () => {
			expect(normalizeQueryConcept('get user')).toBe(normalizeQueryConcept('getUser'));
			expect(normalizeQueryConcept('get user')).toBe(normalizeQueryConcept('get_user'));
		});

		test('case insensitive', () => {
			expect(normalizeQueryConcept('GetData')).toBe(normalizeQueryConcept('getdata'));
			expect(normalizeQueryConcept('GETDATA')).toBe(normalizeQueryConcept('getdata'));
		});

		test('combined: Go method with dots, quotes, and case variations', () => {
			const variants = [
				'"ctx.GetData"',
				'ctx.GetData',
				'ctxGetData',
				'ctx_get_data',
				'CTX_GET_DATA',
				"'ctxGetData'",
			];
			const normalized = variants.map(normalizeQueryConcept);
			expect(new Set(normalized).size).toBe(1);
		});
	});

	// ---- These should NOT collide (genuinely different concepts) ----

	describe('does NOT collapse genuinely different concepts', () => {

		// -- Go --
		test('Go: "func getData" vs "getData" are different (func is a keyword search modifier)', () => {
			expect(normalizeQueryConcept('func getData'))
				.not.toBe(normalizeQueryConcept('getData'));
		});

		test('Go: "type Config" vs "Config" are different', () => {
			expect(normalizeQueryConcept('type Config'))
				.not.toBe(normalizeQueryConcept('Config'));
		});

		test('Go: "interface Handler" vs "Handler" are different', () => {
			expect(normalizeQueryConcept('interface Handler'))
				.not.toBe(normalizeQueryConcept('Handler'));
		});

		// -- Rust --
		test('Rust: "impl Display" vs "Display" are different', () => {
			expect(normalizeQueryConcept('impl Display'))
				.not.toBe(normalizeQueryConcept('Display'));
		});

		test('Rust: "std::collections::HashMap" preserves colons (does not collide with dot form)', () => {
			expect(normalizeQueryConcept('std::collections::HashMap'))
				.not.toBe(normalizeQueryConcept('std.collections.HashMap'));
		});

		test('Rust: "trait Iterator" vs "Iterator" are different', () => {
			expect(normalizeQueryConcept('trait Iterator'))
				.not.toBe(normalizeQueryConcept('Iterator'));
		});

		// -- Python --
		test('Python: "class UserModel" vs "UserModel" are different', () => {
			expect(normalizeQueryConcept('class UserModel'))
				.not.toBe(normalizeQueryConcept('UserModel'));
		});

		test('Python: "def process_data" vs "process_data" are different', () => {
			expect(normalizeQueryConcept('def process_data'))
				.not.toBe(normalizeQueryConcept('process_data'));
		});

		// -- TypeScript --
		test('TS: "export default" vs "default" are different', () => {
			expect(normalizeQueryConcept('export default'))
				.not.toBe(normalizeQueryConcept('default'));
		});

		test('TS: "async function" vs "function" are different', () => {
			expect(normalizeQueryConcept('async function'))
				.not.toBe(normalizeQueryConcept('function'));
		});

		// -- Completely different concepts --
		test('"authentication" vs "authorization" do not collide', () => {
			expect(normalizeQueryConcept('authentication'))
				.not.toBe(normalizeQueryConcept('authorization'));
		});

		test('"getData" vs "setData" do not collide', () => {
			expect(normalizeQueryConcept('getData'))
				.not.toBe(normalizeQueryConcept('setData'));
		});

		test('"user.Create" vs "user.Delete" do not collide', () => {
			expect(normalizeQueryConcept('user.Create'))
				.not.toBe(normalizeQueryConcept('user.Delete'));
		});

		test('"middleware" vs "handler" do not collide', () => {
			expect(normalizeQueryConcept('middleware'))
				.not.toBe(normalizeQueryConcept('handler'));
		});
	});

	// ---- Edge cases ----

	describe('edge cases', () => {
		test('empty/null inputs', () => {
			expect(normalizeQueryConcept('')).toBe('');
			expect(normalizeQueryConcept(null)).toBe('');
			expect(normalizeQueryConcept(undefined)).toBe('');
		});

		test('single character', () => {
			expect(normalizeQueryConcept('a')).toBe('a');
		});

		test('only dots: "..." normalizes to empty string', () => {
			expect(normalizeQueryConcept('...')).toBe('');
		});

		test('only underscores normalizes to empty string', () => {
			expect(normalizeQueryConcept('___')).toBe('');
		});

		test('preserves parentheses (function calls are structurally different)', () => {
			expect(normalizeQueryConcept('getData()')).not.toBe(normalizeQueryConcept('getData'));
		});

		test('preserves colons (Rust paths stay distinct)', () => {
			expect(normalizeQueryConcept('std::io')).not.toBe(normalizeQueryConcept('stdio'));
		});

		test('preserves slashes (file paths stay distinct)', () => {
			expect(normalizeQueryConcept('src/utils')).not.toBe(normalizeQueryConcept('srcutils'));
		});

		test('preserves brackets and angle brackets', () => {
			expect(normalizeQueryConcept('Vec<String>')).not.toBe(normalizeQueryConcept('VecString'));
		});
	});

	// ---- Known acceptable collisions (document them explicitly) ----

	describe('known acceptable collisions (same concept, different notation)', () => {
		test('Go: http.Server vs httpServer — both mean "http server" concept', () => {
			// These collide, but if both fail, the concept doesn't exist
			expect(normalizeQueryConcept('http.Server')).toBe(normalizeQueryConcept('httpServer'));
		});

		test('JS: log.error vs logError — both mean "log error" concept', () => {
			expect(normalizeQueryConcept('log.error')).toBe(normalizeQueryConcept('logError'));
		});

		test('Python: os.path vs ospath — both mean "os path" concept', () => {
			expect(normalizeQueryConcept('os.path')).toBe(normalizeQueryConcept('ospath'));
		});

		test('Go: json.Marshal vs jsonMarshal vs json_marshal', () => {
			const norm = normalizeQueryConcept('json.Marshal');
			expect(norm).toBe(normalizeQueryConcept('jsonMarshal'));
			expect(norm).toBe(normalizeQueryConcept('json_marshal'));
		});
	});
});
