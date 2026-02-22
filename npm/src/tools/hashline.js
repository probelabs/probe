/**
 * Hash-based line integrity utilities for line-targeted editing.
 * Uses DJB2 hash of whitespace-stripped content, mod 256, as 2-char hex.
 * Pure functions, zero external dependencies.
 * @module tools/hashline
 */

/**
 * Compute a 2-char hex hash for a line of code.
 * DJB2 hash of whitespace-stripped content mod 256.
 * @param {string} line - The line content
 * @returns {string} 2-char hex hash (e.g. "ab")
 */
export function computeLineHash(line) {
  const stripped = (line || '').replace(/\s+/g, '');
  let h = 5381;
  for (let i = 0; i < stripped.length; i++) {
    h = ((h << 5) + h + stripped.charCodeAt(i)) & 0xFFFFFFFF;
  }
  return ((h >>> 0) % 256).toString(16).padStart(2, '0');
}

/**
 * Parse a line reference string into line number and optional hash.
 * Handles XML coercion: number 42 → {line:42, hash:null}
 * String formats: "42" → {line:42, hash:null}, "42:ab" → {line:42, hash:"ab"}
 * @param {string|number} ref - Line reference
 * @returns {{line: number, hash: string|null}|null} Parsed ref or null if invalid
 */
export function parseLineRef(ref) {
  if (ref === undefined || ref === null) return null;

  const str = String(ref).trim();
  if (!str) return null;

  // Format: "42:ab" (line with hash)
  const hashMatch = str.match(/^(\d+):([0-9a-fA-F]{2})$/);
  if (hashMatch) {
    const line = parseInt(hashMatch[1], 10);
    if (line < 1 || !isFinite(line)) return null;
    return { line, hash: hashMatch[2].toLowerCase() };
  }

  // Format: "42" (plain line number)
  const lineMatch = str.match(/^(\d+)$/);
  if (lineMatch) {
    const line = parseInt(lineMatch[1], 10);
    if (line < 1 || !isFinite(line)) return null;
    return { line, hash: null };
  }

  return null;
}

/**
 * Validate a hash against the actual file content at a line number.
 * @param {number} lineNum - 1-indexed line number
 * @param {string} hash - Expected 2-char hex hash
 * @param {string[]} fileLines - Array of file lines
 * @returns {{valid: boolean, actualHash: string, actualContent: string}}
 */
export function validateLineHash(lineNum, hash, fileLines) {
  const idx = lineNum - 1;
  if (idx < 0 || idx >= fileLines.length) {
    return { valid: false, actualHash: '', actualContent: '' };
  }
  const actualContent = fileLines[idx];
  const actualHash = computeLineHash(actualContent);
  return {
    valid: actualHash === hash.toLowerCase(),
    actualHash,
    actualContent
  };
}

/**
 * Annotate probe output with line hashes.
 * Transforms "  42 |" to "  42:ab |" in each line.
 * Handles the probe output format: optional whitespace + line number + space(s) + pipe.
 * @param {string} output - Raw probe output
 * @returns {string} Annotated output with hashes
 */
export function annotateOutputWithHashes(output) {
  if (!output || typeof output !== 'string') return output;

  return output.split('\n').map(line => {
    // Strip trailing \r from CRLF line endings before matching
    const cleanLine = line.endsWith('\r') ? line.slice(0, -1) : line;
    // Match probe output format: leading whitespace + digits + whitespace + pipe
    const match = cleanLine.match(/^(\s*)(\d+)(\s*\|)(.*)$/);
    if (!match) return line;

    const [, prefix, lineNum, pipeSection, content] = match;
    const hash = computeLineHash(content);
    const cr = line.endsWith('\r') ? '\r' : '';
    return `${prefix}${lineNum}:${hash}${pipeSection}${content}${cr}`;
  }).join('\n');
}

/**
 * Strip accidental line-number or line:hash prefixes from LLM new_string content.
 * LLMs sometimes echo the "42:ab | " or "42 | " prefix format in their replacement text.
 * @param {string} text - The new_string content
 * @returns {{cleaned: string, stripped: boolean}} Cleaned text and whether stripping occurred
 */
export function stripHashlinePrefixes(text) {
  if (!text || typeof text !== 'string') return { cleaned: text || '', stripped: false };

  const lines = text.split('\n');
  if (lines.length === 0) return { cleaned: '', stripped: false };

  // Check if majority of non-empty lines have the prefix pattern
  const nonEmptyLines = lines.filter(l => l.trim().length > 0);
  if (nonEmptyLines.length === 0) return { cleaned: text, stripped: false };

  // Pattern: optional whitespace + digits + optional ":xx" + space(s) + pipe + space
  const prefixPattern = /^\s*\d+(?::[0-9a-fA-F]{2})?\s*\|\s?/;
  const matchCount = nonEmptyLines.filter(l => prefixPattern.test(l)).length;

  // Only strip if majority (>50%) of non-empty lines have prefixes
  if (matchCount / nonEmptyLines.length <= 0.5) {
    return { cleaned: text, stripped: false };
  }

  const cleaned = lines.map(line => {
    if (line.trim().length === 0) return line;
    return line.replace(prefixPattern, '');
  }).join('\n');

  return { cleaned, stripped: true };
}
