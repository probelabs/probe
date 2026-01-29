/**
 * Tests for error-types module
 */

import { describe, test, expect } from '@jest/globals';
import {
  ErrorCategory,
  ProbeError,
  PathError,
  ParameterError,
  TimeoutError,
  ApiError,
  DelegationError,
  categorizeError,
  formatErrorForAI
} from '../../src/utils/error-types.js';

describe('ErrorCategory', () => {
  test('should define all expected categories', () => {
    expect(ErrorCategory.PATH_ERROR).toBe('path_error');
    expect(ErrorCategory.PARAMETER_ERROR).toBe('parameter_error');
    expect(ErrorCategory.TIMEOUT_ERROR).toBe('timeout_error');
    expect(ErrorCategory.API_ERROR).toBe('api_error');
    expect(ErrorCategory.DELEGATION_ERROR).toBe('delegation_error');
    expect(ErrorCategory.INTERNAL_ERROR).toBe('internal_error');
  });
});

describe('ProbeError', () => {
  test('should create error with default values', () => {
    const error = new ProbeError('Test error');
    expect(error.message).toBe('Test error');
    expect(error.category).toBe(ErrorCategory.INTERNAL_ERROR);
    expect(error.recoverable).toBe(false);
    expect(error.suggestion).toBeNull();
    expect(error.details).toEqual({});
    expect(error.name).toBe('ProbeError');
  });

  test('should create error with custom options', () => {
    const error = new ProbeError('Test error', {
      category: ErrorCategory.PATH_ERROR,
      recoverable: true,
      suggestion: 'Try a different path',
      details: { path: '/test' }
    });

    expect(error.category).toBe(ErrorCategory.PATH_ERROR);
    expect(error.recoverable).toBe(true);
    expect(error.suggestion).toBe('Try a different path');
    expect(error.details).toEqual({ path: '/test' });
  });

  test('should format as XML correctly', () => {
    const error = new ProbeError('Path not found', {
      category: ErrorCategory.PATH_ERROR,
      recoverable: true,
      suggestion: 'Verify the path exists'
    });

    const xml = error.toXml();
    expect(xml).toContain('type="path_error"');
    expect(xml).toContain('recoverable="true"');
    expect(xml).toContain('<message>Path not found</message>');
    expect(xml).toContain('<suggestion>Verify the path exists</suggestion>');
  });

  test('should escape XML special characters', () => {
    const error = new ProbeError('Error with <special> & "characters"');
    const xml = error.toXml();
    expect(xml).toContain('&lt;special&gt;');
    expect(xml).toContain('&amp;');
    expect(xml).toContain('&quot;characters&quot;');
  });

  test('should include details in XML when present', () => {
    const error = new ProbeError('Error', {
      details: { key: 'value' }
    });
    const xml = error.toXml();
    expect(xml).toContain('<details>');
    // Details are XML-escaped, so quotes become &quot;
    expect(xml).toContain('&quot;key&quot;');
    expect(xml).toContain('&quot;value&quot;');
  });

  test('toString should return backward-compatible format', () => {
    const error = new ProbeError('Something went wrong');
    expect(error.toString()).toBe('Error: Something went wrong');
  });
});

describe('PathError', () => {
  test('should have correct category and default suggestion', () => {
    const error = new PathError('Path does not exist');
    expect(error.category).toBe(ErrorCategory.PATH_ERROR);
    expect(error.recoverable).toBe(true);
    expect(error.suggestion).toContain('verify the path');
    expect(error.name).toBe('PathError');
  });

  test('should allow custom suggestion', () => {
    const error = new PathError('Custom path error', {
      suggestion: 'Custom suggestion'
    });
    expect(error.suggestion).toBe('Custom suggestion');
  });

  test('should allow setting recoverable to false', () => {
    const error = new PathError('Permission denied', {
      recoverable: false
    });
    expect(error.recoverable).toBe(false);
  });
});

describe('ParameterError', () => {
  test('should have correct category and default suggestion', () => {
    const error = new ParameterError('Invalid parameter');
    expect(error.category).toBe(ErrorCategory.PARAMETER_ERROR);
    expect(error.recoverable).toBe(true);
    expect(error.suggestion).toContain('parameter');
    expect(error.name).toBe('ParameterError');
  });
});

describe('TimeoutError', () => {
  test('should have correct category and default suggestion', () => {
    const error = new TimeoutError('Operation timed out');
    expect(error.category).toBe(ErrorCategory.TIMEOUT_ERROR);
    expect(error.recoverable).toBe(true);
    expect(error.suggestion).toContain('timed out');
    expect(error.name).toBe('TimeoutError');
  });
});

describe('ApiError', () => {
  test('should have correct category and default to non-recoverable', () => {
    const error = new ApiError('API failure');
    expect(error.category).toBe(ErrorCategory.API_ERROR);
    expect(error.recoverable).toBe(false);
    expect(error.name).toBe('ApiError');
  });

  test('should allow setting recoverable', () => {
    const error = new ApiError('Rate limit', { recoverable: true });
    expect(error.recoverable).toBe(true);
  });
});

describe('DelegationError', () => {
  test('should have correct category and default suggestion', () => {
    const error = new DelegationError('Delegation failed');
    expect(error.category).toBe(ErrorCategory.DELEGATION_ERROR);
    expect(error.recoverable).toBe(true);
    expect(error.suggestion).toContain('delegated task');
    expect(error.name).toBe('DelegationError');
  });
});

describe('categorizeError', () => {
  test('should return ProbeError as-is', () => {
    const original = new PathError('Already categorized');
    const result = categorizeError(original);
    expect(result).toBe(original);
  });

  test('should categorize ENOENT as PathError', () => {
    const error = new Error('No such file or directory');
    error.code = 'ENOENT';
    const result = categorizeError(error);

    expect(result).toBeInstanceOf(PathError);
    expect(result.category).toBe(ErrorCategory.PATH_ERROR);
    expect(result.recoverable).toBe(true);
    expect(result.originalError).toBe(error);
  });

  test('should categorize "Path does not exist" as PathError', () => {
    const error = new Error('Path does not exist: /some/path');
    const result = categorizeError(error);

    expect(result).toBeInstanceOf(PathError);
    expect(result.suggestion).toContain('verify the path');
  });

  test('should categorize "not a directory" as PathError', () => {
    const error = new Error('Path is not a directory');
    const result = categorizeError(error);

    expect(result).toBeInstanceOf(PathError);
    expect(result.suggestion).toContain('directory');
  });

  test('should categorize permission denied as non-recoverable PathError', () => {
    const error = new Error('permission denied');
    error.code = 'EACCES';
    const result = categorizeError(error);

    expect(result).toBeInstanceOf(PathError);
    expect(result.recoverable).toBe(false);
  });

  test('should categorize ETIMEDOUT as TimeoutError', () => {
    const error = new Error('Connection timed out');
    error.code = 'ETIMEDOUT';
    const result = categorizeError(error);

    expect(result).toBeInstanceOf(TimeoutError);
    expect(result.recoverable).toBe(true);
  });

  test('should categorize "timed out" message as TimeoutError', () => {
    const error = new Error('Search operation timed out');
    const result = categorizeError(error);

    expect(result).toBeInstanceOf(TimeoutError);
  });

  test('should categorize rate limit errors as ApiError', () => {
    const testCases = [
      'rate_limit exceeded',
      'rate limit hit',
      'Error 429: Too many requests',
      'Too Many Requests'
    ];

    for (const message of testCases) {
      const error = new Error(message);
      const result = categorizeError(error);
      expect(result).toBeInstanceOf(ApiError);
      expect(result.recoverable).toBe(true);
    }
  });

  test('should categorize server errors as ApiError', () => {
    const testCases = ['500 Internal Server Error', 'Bad Gateway 502', '503 Service Unavailable'];

    for (const message of testCases) {
      const error = new Error(message);
      const result = categorizeError(error);
      expect(result).toBeInstanceOf(ApiError);
      expect(result.recoverable).toBe(true);
    }
  });

  test('should categorize context limit errors as ApiError', () => {
    const testCases = [
      'context limit exceeded',
      'token limit reached',
      'Maximum tokens exceeded'
    ];

    for (const message of testCases) {
      const error = new Error(message);
      const result = categorizeError(error);
      expect(result).toBeInstanceOf(ApiError);
    }
  });

  test('should categorize auth errors as non-recoverable ApiError', () => {
    const error = new Error('Invalid API key');
    const result = categorizeError(error);

    expect(result).toBeInstanceOf(ApiError);
    expect(result.recoverable).toBe(false);
  });

  test('should categorize parameter validation errors as ParameterError', () => {
    const testCases = [
      'parameter is required',
      'value must be a number',
      'invalid parameter: foo'
    ];

    for (const message of testCases) {
      const error = new Error(message);
      const result = categorizeError(error);
      expect(result).toBeInstanceOf(ParameterError);
      expect(result.recoverable).toBe(true);
    }
  });

  test('should categorize delegation errors as DelegationError', () => {
    const error = new Error('Delegation failed: subagent error');
    const result = categorizeError(error);

    expect(result).toBeInstanceOf(DelegationError);
    expect(result.recoverable).toBe(true);
  });

  test('should categorize network errors as ApiError', () => {
    const testCases = [
      { code: 'ECONNRESET', message: 'Connection reset' },
      { code: 'ECONNREFUSED', message: 'Connection refused' },
      { code: 'ENOTFOUND', message: 'DNS lookup failed' }
    ];

    for (const { code, message } of testCases) {
      const error = new Error(message);
      error.code = code;
      const result = categorizeError(error);
      expect(result).toBeInstanceOf(ApiError);
      expect(result.recoverable).toBe(true);
    }
  });

  test('should default to internal error for unknown errors', () => {
    const error = new Error('Something completely unknown happened');
    const result = categorizeError(error);

    expect(result).toBeInstanceOf(ProbeError);
    expect(result.category).toBe(ErrorCategory.INTERNAL_ERROR);
    expect(result.recoverable).toBe(false);
  });

  test('should handle string errors', () => {
    const result = categorizeError('Simple string error');
    expect(result).toBeInstanceOf(ProbeError);
    expect(result.message).toBe('Simple string error');
  });

  test('should handle null/undefined errors', () => {
    const result = categorizeError(null);
    expect(result).toBeInstanceOf(ProbeError);
    expect(result.message).toBe('null');
  });
});

describe('formatErrorForAI', () => {
  test('should format regular Error as XML', () => {
    const error = new Error('Path does not exist: /test');
    const xml = formatErrorForAI(error);

    expect(xml).toContain('<error');
    expect(xml).toContain('type="path_error"');
    expect(xml).toContain('<message>');
    expect(xml).toContain('</error>');
  });

  test('should format ProbeError as XML', () => {
    const error = new PathError('Not found', { suggestion: 'Check the path' });
    const xml = formatErrorForAI(error);

    expect(xml).toContain('type="path_error"');
    expect(xml).toContain('<suggestion>Check the path</suggestion>');
  });

  test('should produce valid XML structure', () => {
    const error = new Error('Test error');
    const xml = formatErrorForAI(error);

    // Basic XML structure validation
    expect(xml.startsWith('<error')).toBe(true);
    expect(xml.endsWith('</error>')).toBe(true);
    expect(xml.includes('<message>')).toBe(true);
    expect(xml.includes('</message>')).toBe(true);
  });
});

describe('detectUnrecognizedToolCall', () => {
  // Import the function for testing
  let detectUnrecognizedToolCall;

  beforeAll(async () => {
    const module = await import('../../src/tools/common.js');
    detectUnrecognizedToolCall = module.detectUnrecognizedToolCall;
  });

  test('should return null when no tool tags found', () => {
    const response = 'Just some text without any tool calls';
    const result = detectUnrecognizedToolCall(response, ['search', 'attempt_completion']);
    expect(result).toBeNull();
  });

  test('should return null when only valid tools are used', () => {
    const response = '<search><query>test</query></search>';
    const result = detectUnrecognizedToolCall(response, ['search', 'attempt_completion']);
    expect(result).toBeNull();
  });

  test('should detect extract when not in valid tools', () => {
    const response = '<extract><targets>file.js:10-20</targets></extract>';
    const result = detectUnrecognizedToolCall(response, ['search', 'attempt_completion']);
    expect(result).toBe('extract');
  });

  test('should detect query when not in valid tools', () => {
    const response = '<query><pattern>fn test</pattern></query>';
    const result = detectUnrecognizedToolCall(response, ['search', 'attempt_completion']);
    expect(result).toBe('query');
  });

  test('should detect bash when not in valid tools', () => {
    const response = '<bash><command>ls -la</command></bash>';
    const result = detectUnrecognizedToolCall(response, ['search', 'attempt_completion']);
    expect(result).toBe('bash');
  });

  test('should ignore non-tool tags like thinking', () => {
    const response = '<thinking>Let me analyze this...</thinking>';
    const result = detectUnrecognizedToolCall(response, ['search', 'attempt_completion']);
    expect(result).toBeNull();
  });

  test('should handle null/undefined input', () => {
    expect(detectUnrecognizedToolCall(null, ['search'])).toBeNull();
    expect(detectUnrecognizedToolCall(undefined, ['search'])).toBeNull();
    expect(detectUnrecognizedToolCall('', ['search'])).toBeNull();
  });
});

describe('Integration scenarios', () => {
  test('should handle typical path not found scenario', () => {
    const error = new Error('Path does not exist: /tmp/nonexistent');
    const xml = formatErrorForAI(error);

    expect(xml).toContain('type="path_error"');
    expect(xml).toContain('recoverable="true"');
    expect(xml).toContain('verify the path');
  });

  test('should handle search timeout scenario', () => {
    const error = new Error('Search operation timed out after 30 seconds');
    error.killed = true;
    const xml = formatErrorForAI(error);

    expect(xml).toContain('type="timeout_error"');
    expect(xml).toContain('recoverable="true"');
  });

  test('should handle API rate limit scenario', () => {
    const error = new Error('Error 429: rate_limit exceeded');
    const xml = formatErrorForAI(error);

    expect(xml).toContain('type="api_error"');
    expect(xml).toContain('recoverable="true"');
  });
});
