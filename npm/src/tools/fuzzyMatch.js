/**
 * Progressive fuzzy string matching for the edit tool.
 * Strategies are tried in order:
 *   exact (handled by caller) → line-trimmed → whitespace-normalized → indent-flexible
 *
 * All functions are PURE — no file I/O, no side effects.
 * Each match function returns the ACTUAL text from the file content (not the search string),
 * so the caller can do content.replace(matchedText, newString).
 *
 * @module tools/fuzzyMatch
 */

/**
 * Try all fuzzy strategies in order. Returns first match or null.
 * @param {string} content - Full file content
 * @param {string} searchString - String to find
 * @returns {{ matchedText: string, strategy: string, count: number } | null}
 */
export function findFuzzyMatch(content, searchString) {
  // Guard: empty or whitespace-only search string
  if (!searchString || searchString.trim().length === 0) {
    return null;
  }

  // Normalize \r\n to \n for consistent handling
  const normalizedContent = content.replace(/\r\n/g, '\n');
  const normalizedSearch = searchString.replace(/\r\n/g, '\n');

  const contentLines = normalizedContent.split('\n');
  const searchLines = normalizedSearch.split('\n');

  // Strategy 1: Line-trimmed
  const trimmed = lineTrimmedMatch(contentLines, searchLines);
  if (trimmed) return { ...trimmed, strategy: 'line-trimmed' };

  // Strategy 2: Whitespace-normalized
  const normalized = whitespaceNormalizedMatch(normalizedContent, normalizedSearch);
  if (normalized) return { ...normalized, strategy: 'whitespace-normalized' };

  // Strategy 3: Indentation-flexible
  const indentFlex = indentFlexibleMatch(contentLines, searchLines);
  if (indentFlex) return { ...indentFlex, strategy: 'indent-flexible' };

  return null;
}

/**
 * Line-trimmed matching: trims each line before comparing.
 * Slides a window of searchLines.length across contentLines.
 * If trimmed lines match, returns the actual text from content at those line positions.
 *
 * @param {string[]} contentLines - Lines of the full file content
 * @param {string[]} searchLines - Lines of the search string
 * @returns {{ matchedText: string, count: number } | null}
 */
export function lineTrimmedMatch(contentLines, searchLines) {
  if (searchLines.length === 0) return null;

  const trimmedSearchLines = searchLines.map(line => line.trim());

  // If all search lines are empty after trimming, no meaningful match
  if (trimmedSearchLines.every(line => line === '')) return null;

  const windowSize = searchLines.length;
  const matches = [];

  for (let i = 0; i <= contentLines.length - windowSize; i++) {
    let allMatch = true;
    for (let j = 0; j < windowSize; j++) {
      if (contentLines[i + j].trim() !== trimmedSearchLines[j]) {
        allMatch = false;
        break;
      }
    }
    if (allMatch) {
      const matchedText = contentLines.slice(i, i + windowSize).join('\n');
      matches.push(matchedText);
    }
  }

  if (matches.length === 0) return null;

  return {
    matchedText: matches[0],
    count: matches.length,
  };
}

/**
 * Whitespace-normalized matching: collapses all whitespace runs (spaces, tabs)
 * to a single space before comparing. Returns actual text from content.
 *
 * Builds a character index map from normalized positions back to original positions
 * so we can extract the actual content substring.
 *
 * @param {string} content - Full file content
 * @param {string} search - Search string
 * @returns {{ matchedText: string, count: number } | null}
 */
export function whitespaceNormalizedMatch(content, search) {
  if (!search || search.trim().length === 0) return null;

  // Build normalized content with position mapping.
  // We normalize horizontal whitespace (spaces, tabs) to single space,
  // but preserve newlines as meaningful structure.
  const { normalized: normContent, indexMap: contentMap } = buildNormalizedMap(content);
  const { normalized: normSearch } = buildNormalizedMap(search);

  if (normSearch.length === 0) return null;

  // Find all occurrences of normalized search in normalized content
  const matches = [];
  let searchStart = 0;

  while (searchStart <= normContent.length - normSearch.length) {
    const idx = normContent.indexOf(normSearch, searchStart);
    if (idx === -1) break;

    // Map normalized positions back to original content positions
    const originalStart = contentMap[idx];
    const originalEnd = contentMap[idx + normSearch.length - 1];

    // Extract actual text from original content — include the full last character
    // We need to find the end of the character at originalEnd
    let actualEnd = originalEnd + 1;
    // If the original character at originalEnd started a whitespace run that was collapsed,
    // extend to include the full whitespace run
    while (actualEnd < content.length && /[ \t]/.test(content[actualEnd]) && (actualEnd === originalEnd + 1 || /[ \t]/.test(content[actualEnd - 1]))) {
      // Only extend if the next normalized position would be beyond our match
      if (contentMap.indexOf(actualEnd) > idx + normSearch.length - 1 || contentMap.indexOf(actualEnd) === -1) {
        break;
      }
      actualEnd++;
    }

    const matchedText = content.substring(originalStart, actualEnd);
    matches.push(matchedText);

    searchStart = idx + 1;
  }

  if (matches.length === 0) return null;

  return {
    matchedText: matches[0],
    count: matches.length,
  };
}

/**
 * Build a normalized string and a map from normalized character index to original index.
 * Collapses runs of horizontal whitespace (spaces, tabs) to a single space.
 * Preserves newlines.
 *
 * @param {string} str - Original string
 * @returns {{ normalized: string, indexMap: number[] }}
 */
function buildNormalizedMap(str) {
  const normalized = [];
  const indexMap = [];
  let i = 0;

  while (i < str.length) {
    const ch = str[i];

    if (ch === ' ' || ch === '\t') {
      // Start of a whitespace run — collapse to single space
      normalized.push(' ');
      indexMap.push(i);
      // Skip the rest of the whitespace run
      while (i < str.length && (str[i] === ' ' || str[i] === '\t')) {
        i++;
      }
    } else {
      normalized.push(ch);
      indexMap.push(i);
      i++;
    }
  }

  return {
    normalized: normalized.join(''),
    indexMap,
  };
}

/**
 * Indentation-flexible matching: strips minimum common indentation from both
 * content window and search lines, then compares.
 *
 * @param {string[]} contentLines - Lines of the full file content
 * @param {string[]} searchLines - Lines of the search string
 * @returns {{ matchedText: string, count: number } | null}
 */
export function indentFlexibleMatch(contentLines, searchLines) {
  if (searchLines.length === 0) return null;

  // If all search lines are empty, no meaningful match
  if (searchLines.every(line => line.trim() === '')) return null;

  // Strip minimum indent from search lines
  const searchMinIndent = getMinIndent(searchLines);
  const strippedSearch = searchLines.map(line => stripIndent(line, searchMinIndent));

  const windowSize = searchLines.length;
  const matches = [];

  for (let i = 0; i <= contentLines.length - windowSize; i++) {
    const windowLines = contentLines.slice(i, i + windowSize);
    const windowMinIndent = getMinIndent(windowLines);
    const strippedWindow = windowLines.map(line => stripIndent(line, windowMinIndent));

    let allMatch = true;
    for (let j = 0; j < windowSize; j++) {
      if (strippedWindow[j] !== strippedSearch[j]) {
        allMatch = false;
        break;
      }
    }

    if (allMatch) {
      const matchedText = windowLines.join('\n');
      matches.push(matchedText);
    }
  }

  if (matches.length === 0) return null;

  return {
    matchedText: matches[0],
    count: matches.length,
  };
}

/**
 * Get the minimum indentation level (number of leading whitespace characters)
 * across all non-empty lines.
 *
 * @param {string[]} lines
 * @returns {number}
 */
function getMinIndent(lines) {
  let min = Infinity;

  for (const line of lines) {
    // Skip empty or whitespace-only lines for indent calculation
    if (line.trim() === '') continue;

    const match = line.match(/^([ \t]*)/);
    if (match) {
      min = Math.min(min, match[1].length);
    }
  }

  return min === Infinity ? 0 : min;
}

/**
 * Strip a fixed number of leading characters from a line.
 * For empty/whitespace-only lines, return them as-is (trimmed to empty)
 * to handle blank lines in code blocks gracefully.
 *
 * @param {string} line
 * @param {number} amount - Number of leading characters to strip
 * @returns {string}
 */
function stripIndent(line, amount) {
  if (line.trim() === '') return '';
  if (amount <= 0) return line;
  return line.substring(Math.min(amount, line.length));
}
