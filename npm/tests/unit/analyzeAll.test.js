/**
 * Unit tests for analyze_all tool schemas and validation
 */

import {
  analyzeAllSchema,
  analyzeAllDescription
} from '../../src/tools/common.js';

describe('analyzeAll schemas and definitions', () => {
  describe('analyzeAllSchema', () => {
    test('should validate required question field', () => {
      const result = analyzeAllSchema.safeParse({
        path: './src'
      });
      expect(result.success).toBe(false);
    });

    test('should accept valid minimal input with just question', () => {
      const result = analyzeAllSchema.safeParse({
        question: 'What features are available?'
      });
      expect(result.success).toBe(true);
      expect(result.data.path).toBe('.'); // Default value
    });

    test('should accept question with custom path', () => {
      const result = analyzeAllSchema.safeParse({
        question: 'List all API endpoints',
        path: './src/api'
      });
      expect(result.success).toBe(true);
      expect(result.data.question).toBe('List all API endpoints');
      expect(result.data.path).toBe('./src/api');
    });

    test('should reject empty question', () => {
      const result = analyzeAllSchema.safeParse({
        question: ''
      });
      expect(result.success).toBe(false);
    });

    test('should use default path when not provided', () => {
      const result = analyzeAllSchema.safeParse({
        question: 'What tools exist?'
      });
      expect(result.success).toBe(true);
      expect(result.data.path).toBe('.');
    });
  });

  describe('analyzeAllDescription', () => {
    test('should describe the tool purpose', () => {
      expect(analyzeAllDescription.toLowerCase()).toContain('map-reduce');
    });

    test('should mention it analyzes ALL data', () => {
      expect(analyzeAllDescription.toUpperCase()).toContain('ALL');
    });

    test('should warn about performance', () => {
      expect(analyzeAllDescription.toLowerCase()).toContain('slower');
    });
  });

});

