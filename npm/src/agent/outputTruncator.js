import { writeFile, mkdir } from 'fs/promises';
import { tmpdir } from 'os';
import { join } from 'path';
import { randomUUID } from 'crypto';

const DEFAULT_MAX_OUTPUT_TOKENS = 20000;
const CHARS_PER_TOKEN = 4; // Conservative approximation

/**
 * Validate and normalize a token limit value.
 * Returns the default if the value is invalid (NaN, negative, zero).
 * @param {any} value - The value to validate
 * @returns {number} A valid positive token limit
 */
function validateTokenLimit(value) {
  const num = Number(value);
  if (isNaN(num) || num <= 0) {
    return DEFAULT_MAX_OUTPUT_TOKENS;
  }
  return num;
}

/**
 * Get the maximum output tokens limit based on priority:
 * 1. Constructor value (if provided and valid)
 * 2. Environment variable PROBE_MAX_OUTPUT_TOKENS (if valid)
 * 3. Default (20000)
 * @param {number|undefined} constructorValue - Value passed to ProbeAgent constructor
 * @returns {number} The maximum output tokens limit (always a valid positive number)
 */
export function getMaxOutputTokens(constructorValue) {
  if (constructorValue !== undefined && constructorValue !== null) {
    const validated = validateTokenLimit(constructorValue);
    // Only use constructor value if it was valid; otherwise fall through to env/default
    if (validated !== DEFAULT_MAX_OUTPUT_TOKENS || Number(constructorValue) === DEFAULT_MAX_OUTPUT_TOKENS) {
      return validated;
    }
  }
  if (process.env.PROBE_MAX_OUTPUT_TOKENS) {
    return validateTokenLimit(process.env.PROBE_MAX_OUTPUT_TOKENS);
  }
  return DEFAULT_MAX_OUTPUT_TOKENS;
}

/**
 * Truncate tool output if it exceeds the token limit.
 * When truncated, saves full output to a temp file and returns a message with the file path.
 * If file system operations fail, returns truncated content without file reference.
 *
 * @param {string} content - The tool output content to potentially truncate
 * @param {Object} tokenCounter - TokenCounter instance with countTokens method
 * @param {string} sessionId - Session ID for naming temp files
 * @param {number} maxTokens - Maximum tokens allowed (defaults to 20000)
 * @returns {Promise<{truncated: boolean, content: string, tempFilePath?: string, originalTokens?: number, error?: string}>}
 */
export async function truncateIfNeeded(content, tokenCounter, sessionId, maxTokens) {
  const limit = validateTokenLimit(maxTokens);
  const tokenCount = tokenCounter.countTokens(content);

  if (tokenCount <= limit) {
    return { truncated: false, content };
  }

  // Truncate to approximately maxTokens worth of characters
  const maxChars = limit * CHARS_PER_TOKEN;
  const truncatedContent = content.substring(0, maxChars);

  // Try to write full output to temp file
  let tempFilePath = null;
  let fileError = null;

  try {
    const tempDir = join(tmpdir(), 'probe-output');
    await mkdir(tempDir, { recursive: true });
    tempFilePath = join(tempDir, `tool-output-${sessionId || 'unknown'}-${randomUUID()}.txt`);
    await writeFile(tempFilePath, content, 'utf8');
  } catch (err) {
    fileError = err.message || 'Unknown file system error';
    tempFilePath = null;
  }

  let message;
  if (tempFilePath) {
    message = `Output exceeded maximum size (${tokenCount} tokens, limit: ${limit}).
Full output saved to: ${tempFilePath}

--- Truncated Output (first ${limit} tokens approx) ---
${truncatedContent}
...
--- End of Truncated Output ---`;
  } else {
    message = `Output exceeded maximum size (${tokenCount} tokens, limit: ${limit}).
Warning: Could not save full output to file (${fileError}).

--- Truncated Output (first ${limit} tokens approx) ---
${truncatedContent}
...
--- End of Truncated Output ---`;
  }

  return {
    truncated: true,
    content: message,
    tempFilePath: tempFilePath || undefined,
    originalTokens: tokenCount,
    error: fileError || undefined
  };
}
