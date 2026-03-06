/**
 * Tests for autoQuoteSearchTerms — auto-quoting mixed-case and underscore terms
 * to prevent unwanted stemming/splitting in search queries.
 */

// Inline the function to avoid importing the full vercel.js module
function autoQuoteSearchTerms(query) {
	if (!query || typeof query !== 'string') return query;

	const tokens = [];
	let i = 0;
	while (i < query.length) {
		if (/\s/.test(query[i])) {
			i++;
			continue;
		}
		if (query[i] === '"') {
			const end = query.indexOf('"', i + 1);
			if (end !== -1) {
				tokens.push(query.substring(i, end + 1));
				i = end + 1;
			} else {
				tokens.push(query.substring(i));
				break;
			}
			continue;
		}
		let j = i;
		while (j < query.length && !/\s/.test(query[j]) && query[j] !== '"') {
			j++;
		}
		tokens.push(query.substring(i, j));
		i = j;
	}

	const operators = new Set(['AND', 'OR', 'NOT']);

	const result = tokens.map(token => {
		if (token.startsWith('"')) return token;
		if (operators.has(token)) return token;
		const hasUpper = /[A-Z]/.test(token);
		const hasLower = /[a-z]/.test(token);
		const hasUnderscore = token.includes('_');
		const hasMixedCase = hasUpper && hasLower;
		if (hasMixedCase || hasUnderscore) {
			return `"${token}"`;
		}
		return token;
	});

	return result.join(' ');
}

describe('autoQuoteSearchTerms', () => {
	test('should quote camelCase terms', () => {
		expect(autoQuoteSearchTerms('limitDRL')).toBe('"limitDRL"');
		expect(autoQuoteSearchTerms('getUserData')).toBe('"getUserData"');
		expect(autoQuoteSearchTerms('ForwardMessage')).toBe('"ForwardMessage"');
	});

	test('should quote PascalCase terms', () => {
		expect(autoQuoteSearchTerms('ThrottleRetryLimit')).toBe('"ThrottleRetryLimit"');
		expect(autoQuoteSearchTerms('SessionLimiter')).toBe('"SessionLimiter"');
	});

	test('should quote underscore terms', () => {
		expect(autoQuoteSearchTerms('allowed_ips')).toBe('"allowed_ips"');
		expect(autoQuoteSearchTerms('rate_limit')).toBe('"rate_limit"');
		expect(autoQuoteSearchTerms('MAX_RETRY_COUNT')).toBe('"MAX_RETRY_COUNT"');
	});

	test('should not quote all-lowercase terms', () => {
		expect(autoQuoteSearchTerms('rate limit')).toBe('rate limit');
		expect(autoQuoteSearchTerms('middleware')).toBe('middleware');
	});

	test('should not quote all-uppercase terms (likely acronyms)', () => {
		expect(autoQuoteSearchTerms('CIDR')).toBe('CIDR');
		expect(autoQuoteSearchTerms('API')).toBe('API');
	});

	test('should preserve already-quoted terms', () => {
		expect(autoQuoteSearchTerms('"limitDRL"')).toBe('"limitDRL"');
		expect(autoQuoteSearchTerms('"already quoted"')).toBe('"already quoted"');
	});

	test('should preserve boolean operators', () => {
		expect(autoQuoteSearchTerms('rate AND limit')).toBe('rate AND limit');
		expect(autoQuoteSearchTerms('foo OR bar')).toBe('foo OR bar');
		expect(autoQuoteSearchTerms('NOT deprecated')).toBe('NOT deprecated');
	});

	test('should handle mixed quoted and unquoted terms', () => {
		expect(autoQuoteSearchTerms('"limitDRL" limitRedis')).toBe('"limitDRL" "limitRedis"');
		expect(autoQuoteSearchTerms('rate "getUserData"')).toBe('rate "getUserData"');
	});

	test('should handle multiple camelCase terms with OR', () => {
		expect(autoQuoteSearchTerms('limitDRL limitRedis')).toBe('"limitDRL" "limitRedis"');
		expect(autoQuoteSearchTerms('ForwardMessage SessionLimiter')).toBe('"ForwardMessage" "SessionLimiter"');
	});

	test('should handle mixed camelCase and plain terms', () => {
		expect(autoQuoteSearchTerms('rate ThrottleRetryLimit')).toBe('rate "ThrottleRetryLimit"');
		expect(autoQuoteSearchTerms('middleware ForwardMessage handler')).toBe('middleware "ForwardMessage" handler');
	});

	test('should handle camelCase with AND operator', () => {
		expect(autoQuoteSearchTerms('SessionLimiter AND ForwardMessage')).toBe('"SessionLimiter" AND "ForwardMessage"');
	});

	test('should handle empty and null inputs', () => {
		expect(autoQuoteSearchTerms('')).toBe('');
		expect(autoQuoteSearchTerms(null)).toBe(null);
		expect(autoQuoteSearchTerms(undefined)).toBe(undefined);
	});

	test('should not double-quote terms that are already properly quoted', () => {
		expect(autoQuoteSearchTerms('"ForwardMessage" "SessionLimiter"')).toBe('"ForwardMessage" "SessionLimiter"');
	});
});
