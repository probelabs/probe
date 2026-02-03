/**
 * Quick test for analyze_all tool
 */

import dotenv from 'dotenv';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Load .env from the project root (parent of npm directory)
dotenv.config({ path: join(__dirname, '..', '.env') });

import { analyzeAll } from './src/tools/analyzeAll.js';

console.log('=== Quick Test ===\n');

try {
  const result = await analyzeAll({
    question: 'What functions are exported from the search module?',
    path: './src',
    debug: true,
    provider: 'google',
    model: 'gemini-2.0-flash'
  });

  console.log('\n=== RESULT ===');
  console.log(result);
  console.log('\n=== TEST PASSED ===');
} catch (error) {
  console.error('ERROR:', error.message);
  console.error(error.stack);
}
