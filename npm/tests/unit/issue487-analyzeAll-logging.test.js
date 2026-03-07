import { jest } from '@jest/globals';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const analyzeAllPath = resolve(__dirname, '../../src/tools/analyzeAll.js');
const searchPath = resolve(__dirname, '../../src/search.js');
const delegatePath = resolve(__dirname, '../../src/delegate.js');

const mockSearch = jest.fn();
const mockDelegate = jest.fn();

jest.unstable_mockModule(searchPath, () => ({ search: mockSearch }));
jest.unstable_mockModule(delegatePath, () => ({ delegate: mockDelegate }));

const { analyzeAll } = await import(analyzeAllPath);

describe('Issue #487 - analyzeAll chunk-processing visibility', () => {
  beforeEach(() => {
    mockSearch.mockResolvedValue(
      '```txt\nchunk-one-content\n```\n```txt\nchunk-two-content\n```'
    );

    mockDelegate.mockImplementation(async ({ task }) => {
      if (task.includes('Provide your answer in this EXACT format')) {
        return [
          'SEARCH_QUERY: chunk',
          'AGGREGATION: summarize',
          'EXTRACTION_PROMPT: extract chunk facts'
        ].join('\n');
      }

      if (task.includes('You are analyzing search results (chunk')) {
        return '<result>chunk data</result>';
      }

      if (task.includes('Synthesize these analyses into a comprehensive summary')) {
        return '<result>aggregated data</result>';
      }

      if (task.includes('Now provide a COMPREHENSIVE, DETAILED answer')) {
        return '<result>final synthesized answer</result>';
      }

      return '<result>fallback</result>';
    });
  });

  test('logs chunk start/completion even when debug=false', async () => {
    const result = await analyzeAll({
      question: 'What is in the chunks?',
      path: '.',
      chunkSizeTokens: 1,
      maxChunks: 10,
      debug: false
    });

    expect(result).toBe('final synthesized answer');
    expect(console.error).toHaveBeenCalledWith(
      expect.stringContaining('[analyze_all] Started processing chunk')
    );
    expect(console.error).toHaveBeenCalledWith(
      expect.stringContaining('[analyze_all] Completed chunk')
    );
  });
});
