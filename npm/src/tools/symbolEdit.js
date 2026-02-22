/**
 * AST-aware symbol editing helpers
 * Uses probe's tree-sitter AST parsing to find and manipulate code symbols.
 * @module tools/symbolEdit
 */

import { extract } from '../extract.js';

/**
 * Look up a symbol in a file using probe's AST-based extract
 * @param {string} filePath - Absolute path to the file
 * @param {string} symbolName - Name of the symbol to find
 * @param {string} cwd - Working directory for extract
 * @returns {Promise<Object|null>} Symbol info with startLine, endLine, code, nodeType, file; or null
 */
export async function findSymbol(filePath, symbolName, cwd) {
  try {
    const result = await extract({
      files: [`${filePath}#${symbolName}`],
      format: 'json',
      json: true,
      cwd
    });

    if (!result || !result.results || result.results.length === 0) {
      return null;
    }

    const match = result.results[0];
    return {
      startLine: match.lines[0],  // 1-indexed
      endLine: match.lines[1],    // 1-indexed
      code: match.code,
      nodeType: match.node_type,
      file: match.file
    };
  } catch (error) {
    if (process.env.DEBUG === '1') {
      console.error(`[SymbolEdit] findSymbol error for "${symbolName}" in ${filePath}: ${error.message}`);
    }
    return null;
  }
}

/**
 * Look up ALL matching symbols in a file using probe's AST-based extract.
 * When a bare name like "process" matches multiple definitions (e.g. a top-level
 * function AND class methods), this returns all of them with qualified names.
 * @param {string} filePath - Absolute path to the file
 * @param {string} symbolName - Name of the symbol to find
 * @param {string} cwd - Working directory for extract
 * @returns {Promise<Array<Object>>} Array of symbol info objects (may be empty)
 */
export async function findAllSymbols(filePath, symbolName, cwd) {
  try {
    const result = await extract({
      files: [`${filePath}#${symbolName}`],
      format: 'json',
      json: true,
      cwd
    });

    if (!result || !result.results || result.results.length === 0) {
      return [];
    }

    return result.results.map(match => ({
      startLine: match.lines[0],
      endLine: match.lines[1],
      code: match.code,
      nodeType: match.node_type,
      file: match.file,
      qualifiedName: match.symbol_signature || symbolName,
    }));
  } catch (error) {
    if (process.env.DEBUG === '1') {
      console.error(`[SymbolEdit] findAllSymbols error for "${symbolName}" in ${filePath}: ${error.message}`);
    }
    return [];
  }
}

/**
 * Detect the base indentation of a code block (leading whitespace of first non-empty line)
 * @param {string} code - The code block
 * @returns {string} The leading whitespace string
 */
export function detectBaseIndent(code) {
  const lines = code.split('\n');
  for (const line of lines) {
    if (line.trim().length > 0) {
      const match = line.match(/^(\s*)/);
      return match ? match[1] : '';
    }
  }
  return '';
}

/**
 * Reindent new content to match a target indentation level.
 * Strips the existing base indent from the new content and replaces it with the target indent.
 * @param {string} newContent - The new code content to reindent
 * @param {string} targetIndent - The target indentation string
 * @returns {string} Reindented content
 */
export function reindent(newContent, targetIndent) {
  const lines = newContent.split('\n');
  const sourceIndent = detectBaseIndent(newContent);

  return lines.map(line => {
    if (line.trim().length === 0) {
      return '';
    }
    if (line.startsWith(sourceIndent)) {
      return targetIndent + line.slice(sourceIndent.length);
    }
    return line;
  }).join('\n');
}
