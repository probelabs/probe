/**
 * Heuristic corrections for common LLM mistakes in line-targeted edits.
 * Handles echo stripping, indent restoration, and prefix stripping.
 * @module tools/lineEditHeuristics
 */

import { detectBaseIndent, reindent } from './symbolEdit.js';
import { stripHashlinePrefixes } from './hashline.js';

/**
 * Strip boundary lines that LLMs accidentally echo from the file context.
 *
 * Rules:
 * - Replace: if first line of new_string matches the line before start_line → strip it.
 *            if last line matches the line after end_line → strip it.
 * - Insert-after: if first line matches the anchor line → strip it.
 * - Insert-before: if last line matches the anchor line → strip it.
 * - Blank lines are never considered matches (two blanks matching is coincidence).
 *
 * @param {string} newStr - The new_string content
 * @param {string[]} fileLines - Array of file lines (0-indexed)
 * @param {number} startLine - 1-indexed start line
 * @param {number} endLine - 1-indexed end line (same as startLine for single-line or insert)
 * @param {string|undefined} position - "before", "after", or undefined (replace mode)
 * @returns {{result: string, modifications: string[]}}
 */
export function stripEchoedBoundaries(newStr, fileLines, startLine, endLine, position) {
  const modifications = [];
  let lines = newStr.split('\n');

  if (lines.length === 0) return { result: newStr, modifications };

  if (position === 'after') {
    // Insert-after: anchor line is at startLine (1-indexed)
    const anchorIdx = startLine - 1;
    if (anchorIdx >= 0 && anchorIdx < fileLines.length) {
      const anchorTrimmed = fileLines[anchorIdx].trim();
      if (anchorTrimmed.length > 0 && lines.length > 0 && lines[0].trim() === anchorTrimmed) {
        lines = lines.slice(1);
        modifications.push('stripped echoed anchor line (insert-after)');
      }
    }
  } else if (position === 'before') {
    // Insert-before: anchor line is at startLine (1-indexed)
    const anchorIdx = startLine - 1;
    if (anchorIdx >= 0 && anchorIdx < fileLines.length) {
      const anchorTrimmed = fileLines[anchorIdx].trim();
      if (anchorTrimmed.length > 0 && lines.length > 0 && lines[lines.length - 1].trim() === anchorTrimmed) {
        lines = lines.slice(0, -1);
        modifications.push('stripped echoed anchor line (insert-before)');
      }
    }
  } else {
    // Replace mode: check line before start and line after end
    const beforeIdx = startLine - 2; // line before start (0-indexed)
    if (beforeIdx >= 0 && beforeIdx < fileLines.length) {
      const beforeTrimmed = fileLines[beforeIdx].trim();
      if (beforeTrimmed.length > 0 && lines.length > 0 && lines[0].trim() === beforeTrimmed) {
        lines = lines.slice(1);
        modifications.push('stripped echoed line before range');
      }
    }

    const afterIdx = endLine; // line after end (0-indexed, since endLine is 1-indexed)
    if (afterIdx >= 0 && afterIdx < fileLines.length) {
      const afterTrimmed = fileLines[afterIdx].trim();
      if (afterTrimmed.length > 0 && lines.length > 0 && lines[lines.length - 1].trim() === afterTrimmed) {
        lines = lines.slice(0, -1);
        modifications.push('stripped echoed line after range');
      }
    }
  }

  return { result: lines.join('\n'), modifications };
}

/**
 * Restore indentation if the replacement has a different base indent than the original lines.
 * @param {string} newStr - The new_string content
 * @param {string[]} originalLines - The original lines being replaced (from the file)
 * @returns {{result: string, modifications: string[]}}
 */
export function restoreIndentation(newStr, originalLines) {
  const modifications = [];

  if (!newStr || !originalLines || originalLines.length === 0) {
    return { result: newStr || '', modifications };
  }

  const originalCode = originalLines.join('\n');
  const targetIndent = detectBaseIndent(originalCode);
  const newIndent = detectBaseIndent(newStr);

  if (targetIndent !== newIndent) {
    const reindented = reindent(newStr, targetIndent);
    if (reindented !== newStr) {
      modifications.push(`reindented from "${newIndent}" to "${targetIndent}"`);
      return { result: reindented, modifications };
    }
  }

  return { result: newStr, modifications };
}

/**
 * Pipeline: stripHashlinePrefixes → stripEchoedBoundaries → restoreIndentation.
 * @param {string} newStr - The new_string content
 * @param {string[]} fileLines - Array of all file lines (0-indexed)
 * @param {number} startLine - 1-indexed start line
 * @param {number} endLine - 1-indexed end line
 * @param {string|undefined} position - "before", "after", or undefined
 * @returns {{cleaned: string, modifications: string[]}}
 */
export function cleanNewString(newStr, fileLines, startLine, endLine, position) {
  const modifications = [];

  if (!newStr && newStr !== '') return { cleaned: '', modifications };

  // Step 1: Strip hashline prefixes
  const { cleaned: afterPrefixes, stripped } = stripHashlinePrefixes(newStr);
  if (stripped) modifications.push('stripped line-number prefixes');

  // Step 2: Strip echoed boundaries
  const { result: afterEchoes, modifications: echoMods } = stripEchoedBoundaries(
    afterPrefixes, fileLines, startLine, endLine, position
  );
  modifications.push(...echoMods);

  // Step 3: Restore indentation (only for replace mode, not insert)
  if (!position) {
    const originalLines = fileLines.slice(startLine - 1, endLine);
    const { result: afterIndent, modifications: indentMods } = restoreIndentation(afterEchoes, originalLines);
    modifications.push(...indentMods);
    return { cleaned: afterIndent, modifications };
  }

  return { cleaned: afterEchoes, modifications };
}
