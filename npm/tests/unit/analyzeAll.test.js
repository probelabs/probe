/**
 * Unit tests for analyze_all tool schemas and validation
 */

import {
  analyzeAllSchema,
  analyzeAllDescription,
  analyzeAllToolDefinition
} from '../../src/tools/common.js';

describe('analyzeAll schemas and definitions', () => {
  describe('analyzeAllSchema', () => {
    test('should validate required query field', () => {
      const result = analyzeAllSchema.safeParse({
        analysis_prompt: 'test prompt'
      });
      expect(result.success).toBe(false);
    });

    test('should validate required analysis_prompt field', () => {
      const result = analyzeAllSchema.safeParse({
        query: 'test query'
      });
      expect(result.success).toBe(false);
    });

    test('should accept valid minimal input', () => {
      const result = analyzeAllSchema.safeParse({
        query: 'test query',
        analysis_prompt: 'Extract all items'
      });
      expect(result.success).toBe(true);
      expect(result.data.path).toBe('.'); // Default value
      expect(result.data.aggregation).toBe('summarize'); // Default value
    });

    test('should accept all valid aggregation types', () => {
      const validAggregations = ['summarize', 'list_unique', 'count', 'group_by'];

      validAggregations.forEach(aggregation => {
        const result = analyzeAllSchema.safeParse({
          query: 'test',
          analysis_prompt: 'test',
          aggregation
        });
        expect(result.success).toBe(true);
        expect(result.data.aggregation).toBe(aggregation);
      });
    });

    test('should reject invalid aggregation type', () => {
      const result = analyzeAllSchema.safeParse({
        query: 'test',
        analysis_prompt: 'test',
        aggregation: 'invalid_type'
      });
      expect(result.success).toBe(false);
    });

    test('should accept custom path', () => {
      const result = analyzeAllSchema.safeParse({
        query: 'test',
        analysis_prompt: 'test',
        path: './src/components'
      });
      expect(result.success).toBe(true);
      expect(result.data.path).toBe('./src/components');
    });
  });

  describe('analyzeAllDescription', () => {
    test('should describe the tool purpose', () => {
      expect(analyzeAllDescription).toContain('map-reduce');
    });

    test('should mention it processes ALL data', () => {
      expect(analyzeAllDescription.toUpperCase()).toContain('ALL');
    });

    test('should warn about cost/performance', () => {
      expect(analyzeAllDescription.toLowerCase()).toContain('slower');
    });
  });

  describe('analyzeAllToolDefinition', () => {
    test('should be a string', () => {
      expect(typeof analyzeAllToolDefinition).toBe('string');
    });

    test('should start with tool name', () => {
      expect(analyzeAllToolDefinition).toContain('## analyze_all');
    });

    test('should describe parameters', () => {
      expect(analyzeAllToolDefinition).toContain('query');
      expect(analyzeAllToolDefinition).toContain('analysis_prompt');
      expect(analyzeAllToolDefinition).toContain('aggregation');
    });

    test('should list aggregation options', () => {
      expect(analyzeAllToolDefinition).toContain('summarize');
      expect(analyzeAllToolDefinition).toContain('list_unique');
      expect(analyzeAllToolDefinition).toContain('count');
      expect(analyzeAllToolDefinition).toContain('group_by');
    });

    test('should include warning about cost', () => {
      expect(analyzeAllToolDefinition.toUpperCase()).toContain('WARNING');
    });

    test('should include usage examples', () => {
      expect(analyzeAllToolDefinition).toContain('<analyze_all>');
    });
  });
});

describe('analyzeAll integration with tool list', () => {
  test('analyze_all should be in DEFAULT_VALID_TOOLS', async () => {
    const { DEFAULT_VALID_TOOLS } = await import('../../src/tools/common.js');
    expect(DEFAULT_VALID_TOOLS).toContain('analyze_all');
  });
});
