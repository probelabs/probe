import { writeFile, mkdir } from 'fs/promises';
import { tmpdir } from 'os';
import { join } from 'path';
import { randomUUID } from 'crypto';

const DEFAULT_MAX_OUTPUT_TOKENS = 20000;
const CHARS_PER_TOKEN = 4; // Conservative approximation

/**
 * Get the maximum output tokens limit based on priority:
 * 1. Constructor value (if provided)
 * 2. Environment variable PROBE_MAX_OUTPUT_TOKENS
 * 3. Default (20000)
 * @param {number|undefined} constructorValue - Value passed to ProbeAgent constructor
 * @returns {number} The maximum output tokens limit
 */
export function getMaxOutputTokens(constructorValue) {
  if (constructorValue !== undefined && constructorValue !== null) {
    return Number(constructorValue);
  }
  if (process.env.PROBE_MAX_OUTPUT_TOKENS) {
    return Number(process.env.PROBE_MAX_OUTPUT_TOKENS);
  }
  return DEFAULT_MAX_OUTPUT_TOKENS;
}

/**
 * Truncate tool output if it exceeds the token limit.
 * When truncated, saves full output to a temp file and returns a message with the file path.
 *
 * @param {string} content - The tool output content to potentially truncate
 * @param {Object} tokenCounter - TokenCounter instance with countTokens method
 * @param {string} sessionId - Session ID for naming temp files
 * @param {number} maxTokens - Maximum tokens allowed (defaults to 20000)
 * @returns {Promise<{truncated: boolean, content: string, tempFilePath?: string, originalTokens?: number}>}
 */
export async function truncateIfNeeded(content, tokenCounter, sessionId, maxTokens) {
  const limit = maxTokens || DEFAULT_MAX_OUTPUT_TOKENS;
  const tokenCount = tokenCounter.countTokens(content);

  if (tokenCount <= limit) {
    return { truncated: false, content };
  }

  // Write full output to temp file
  const tempDir = join(tmpdir(), 'probe-output');
  await mkdir(tempDir, { recursive: true });
  const tempFilePath = join(tempDir, `tool-output-${sessionId || 'unknown'}-${randomUUID()}.txt`);
  await writeFile(tempFilePath, content, 'utf8');

  // Truncate to approximately maxTokens worth of characters
  const maxChars = limit * CHARS_PER_TOKEN;
  const truncatedContent = content.substring(0, maxChars);

  const message = `Output exceeded maximum size (${tokenCount} tokens, limit: ${limit}).
Full output saved to: ${tempFilePath}

--- Truncated Output (first ${limit} tokens approx) ---
${truncatedContent}
...
--- End of Truncated Output ---`;

  return { truncated: true, content: message, tempFilePath, originalTokens: tokenCount };
}
