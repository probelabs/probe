/**
 * FileTracker — per-session content-aware file state tracking for safe multi-edit workflows
 *
 * Two-tier tracking:
 * 1. _seenFiles (Set) — which files the LLM has "seen" via search/extract. Guards against blind edits.
 * 2. _contentRecords (Map) — per-symbol content hashes from extract #symbol targets. Detects stale edits.
 *
 * Key benefit: edits proceed when the target symbol hasn't changed, even if other parts of the file changed.
 * Uses SHA-256 content hashing instead of mtime/size for precise change detection.
 *
 * @module tools/fileTracker
 */

import { createHash } from 'crypto';
import { resolve, isAbsolute } from 'path';
import { findSymbol } from './symbolEdit.js';

/**
 * Compute a SHA-256 content hash for a code block.
 * Normalizes trailing whitespace per line for robustness against editor formatting.
 * @param {string} content - The code content to hash
 * @returns {string} First 16 hex chars of SHA-256 hash (64 bits of collision resistance)
 */
export function computeContentHash(content) {
  const normalized = (content || '').split('\n').map(l => l.trimEnd()).join('\n');
  return createHash('sha256').update(normalized).digest('hex').slice(0, 16);
}

/**
 * Extract the file path portion from an extract target string.
 * Strips symbol references (#Symbol) and line references (:line, :start-end).
 * @param {string} target - Extract target (e.g. "file.js#fn", "file.js:10-20")
 * @returns {string} Just the file path
 */
function extractFilePath(target) {
  // Strip #Symbol suffix
  const hashIdx = target.indexOf('#');
  if (hashIdx !== -1) {
    return target.slice(0, hashIdx);
  }
  // Strip :line or :start-end suffix (use lastIndexOf to skip Windows drive letter colons)
  const colonIdx = target.lastIndexOf(':');
  if (colonIdx !== -1) {
    // Only strip if what follows looks like a line reference (digits, dash)
    const after = target.slice(colonIdx + 1);
    if (/^\d+(-\d+)?$/.test(after)) {
      return target.slice(0, colonIdx);
    }
  }
  return target;
}

/**
 * Extract the symbol name from an extract target string.
 * @param {string} target - Extract target (e.g. "file.js#fn")
 * @returns {string|null} Symbol name or null if not a symbol target
 */
function extractSymbolName(target) {
  const hashIdx = target.indexOf('#');
  if (hashIdx !== -1) {
    const symbol = target.slice(hashIdx + 1);
    return symbol || null;
  }
  return null;
}

/**
 * Parse file paths from probe search/extract output.
 * Looks for "File: path" headers and "--- path ---" separators.
 * @param {string} output - Probe output text
 * @returns {string[]} Array of file paths found
 */
function parseFilePathsFromOutput(output) {
  const paths = [];
  const regex = /^(?:File:\s+|---\s+)([^\s].*?)(?:\s+---)?$/gm;
  let match;
  while ((match = regex.exec(output)) !== null) {
    const path = match[1].trim();
    // Skip things that look like metadata, not file paths
    if (path && !path.startsWith('Results') && !path.startsWith('Page') && (path.includes('/') || path.includes('.') || path.includes('\\'))) {
      paths.push(path);
    }
  }
  return paths;
}

export class FileTracker {
  /**
   * @param {Object} [options]
   * @param {boolean} [options.debug=false] - Enable debug logging
   */
  constructor(options = {}) {
    this.debug = options.debug || false;
    /** @type {Set<string>} Files seen via search/extract */
    this._seenFiles = new Set();
    /** @type {Map<string, {contentHash: string, startLine: number, endLine: number, symbolName: string|null, source: string, timestamp: number}>} */
    this._contentRecords = new Map();
  }

  /**
   * Mark a file as "seen" — the LLM has read its content.
   * @param {string} resolvedPath - Absolute path to the file
   */
  markFileSeen(resolvedPath) {
    this._seenFiles.add(resolvedPath);
    if (this.debug) {
      console.error(`[FileTracker] Marked as seen: ${resolvedPath}`);
    }
  }

  /**
   * Check if a file has been seen in this session.
   * @param {string} resolvedPath - Absolute path to the file
   * @returns {boolean}
   */
  isFileSeen(resolvedPath) {
    return this._seenFiles.has(resolvedPath);
  }

  /**
   * Store a content hash for a symbol in a file.
   * @param {string} resolvedPath - Absolute path to the file
   * @param {string} symbolName - Symbol name (e.g. "calculateTotal")
   * @param {string} code - The symbol's source code
   * @param {number} startLine - 1-indexed start line
   * @param {number} endLine - 1-indexed end line
   * @param {string} [source='extract'] - How the content was obtained
   */
  trackSymbolContent(resolvedPath, symbolName, code, startLine, endLine, source = 'extract') {
    const key = `${resolvedPath}#${symbolName}`;
    const contentHash = computeContentHash(code);
    this._contentRecords.set(key, {
      contentHash,
      startLine,
      endLine,
      symbolName,
      source,
      timestamp: Date.now()
    });
    if (this.debug) {
      console.error(`[FileTracker] Tracked symbol ${key} (hash: ${contentHash}, lines ${startLine}-${endLine})`);
    }
  }

  /**
   * Look up a stored content record for a symbol.
   * @param {string} resolvedPath - Absolute path to the file
   * @param {string} symbolName - Symbol name
   * @returns {Object|null} The stored record or null
   */
  getSymbolRecord(resolvedPath, symbolName) {
    return this._contentRecords.get(`${resolvedPath}#${symbolName}`) || null;
  }

  /**
   * Check if a symbol's current content matches what was stored.
   * @param {string} resolvedPath - Absolute path to the file
   * @param {string} symbolName - Symbol name
   * @param {string} currentCode - The symbol's current source code (from findSymbol)
   * @returns {{ok: boolean, reason?: string, message?: string}}
   */
  checkSymbolContent(resolvedPath, symbolName, currentCode) {
    const key = `${resolvedPath}#${symbolName}`;
    const record = this._contentRecords.get(key);

    if (!record) {
      // No record for this specific symbol — allow (file was seen, this is first edit)
      return { ok: true };
    }

    const currentHash = computeContentHash(currentCode);
    if (currentHash === record.contentHash) {
      return { ok: true };
    }

    return {
      ok: false,
      reason: 'stale',
      message: `Symbol "${symbolName}" has changed since you last read it (hash: ${record.contentHash} → ${currentHash}).`
    };
  }

  /**
   * Track files from extract target strings.
   * Marks each file as seen. For #symbol targets, calls findSymbol to get and hash the code.
   * @param {string[]} targets - Array of extract targets (e.g. ["file.js#fn", "file.js:10-20"])
   * @param {string} cwd - Working directory for resolving relative paths
   */
  async trackFilesFromExtract(targets, cwd) {
    const seenPaths = new Set();
    const symbolPromises = [];

    for (const target of targets) {
      const filePath = extractFilePath(target);
      const resolved = isAbsolute(filePath) ? filePath : resolve(cwd, filePath);

      // Mark file as seen (deduplicate)
      if (!seenPaths.has(resolved)) {
        seenPaths.add(resolved);
        this.markFileSeen(resolved);
      }

      // For symbol targets, get the content hash
      const symbolName = extractSymbolName(target);
      if (symbolName) {
        symbolPromises.push(
          findSymbol(resolved, symbolName, cwd)
            .then(symbolInfo => {
              if (symbolInfo) {
                this.trackSymbolContent(
                  resolved, symbolName, symbolInfo.code,
                  symbolInfo.startLine, symbolInfo.endLine, 'extract'
                );
              }
            })
            .catch(err => {
              if (this.debug) {
                console.error(`[FileTracker] Failed to track symbol "${symbolName}" in ${resolved}: ${err.message}`);
              }
            })
        );
      }
    }

    if (symbolPromises.length > 0) {
      await Promise.all(symbolPromises);
    }
  }

  /**
   * Track files discovered in probe search/extract output.
   * Parses "File: path" headers and "--- path ---" separators, marks each as "seen".
   * @param {string} output - Probe output text
   * @param {string} cwd - Working directory for resolving relative paths
   */
  async trackFilesFromOutput(output, cwd) {
    const paths = parseFilePathsFromOutput(output);
    for (const filePath of paths) {
      const resolved = isAbsolute(filePath) ? filePath : resolve(cwd, filePath);
      this.markFileSeen(resolved);
    }
  }

  /**
   * Check if a file is safe to edit (seen-check only).
   * Mode-specific content verification happens in edit handlers.
   * @param {string} resolvedPath - Absolute path to the file
   * @returns {{ok: boolean, reason?: string, message?: string}}
   */
  checkBeforeEdit(resolvedPath) {
    if (!this._seenFiles.has(resolvedPath)) {
      return {
        ok: false,
        reason: 'untracked',
        message: 'This file has not been read yet in this session. Use extract or search to read the file first.'
      };
    }
    return { ok: true };
  }

  /**
   * Mark a file as seen after a successful write (backward compat).
   * Also invalidates content records for the file since its content changed.
   * @param {string} resolvedPath - Absolute path to the file
   */
  async trackFileAfterWrite(resolvedPath) {
    this.markFileSeen(resolvedPath);
    this.invalidateFileRecords(resolvedPath);
  }

  /**
   * Update the stored hash for a symbol after a successful write.
   * Enables chained edits to the same symbol.
   * @param {string} resolvedPath - Absolute path to the file
   * @param {string} symbolName - Symbol name
   * @param {string} code - The symbol's new source code
   * @param {number} startLine - 1-indexed start line (new position)
   * @param {number} endLine - 1-indexed end line (new position)
   */
  trackSymbolAfterWrite(resolvedPath, symbolName, code, startLine, endLine) {
    this.trackSymbolContent(resolvedPath, symbolName, code, startLine, endLine, 'edit');
  }

  /**
   * Remove all content records for a file.
   * Called after non-symbol edits (text/line mode) since those change content
   * without providing a symbol-level update.
   * @param {string} resolvedPath - Absolute path to the file
   */
  invalidateFileRecords(resolvedPath) {
    const prefix = resolvedPath + '#';
    for (const key of this._contentRecords.keys()) {
      if (key.startsWith(prefix)) {
        this._contentRecords.delete(key);
      }
    }
    if (this.debug) {
      console.error(`[FileTracker] Invalidated content records for ${resolvedPath}`);
    }
  }

  /**
   * Quick sync check if a file is being tracked (alias for isFileSeen).
   * @param {string} resolvedPath - Absolute path to the file
   * @returns {boolean}
   */
  isTracked(resolvedPath) {
    return this.isFileSeen(resolvedPath);
  }

  /**
   * Clear all tracking state.
   */
  clear() {
    this._seenFiles.clear();
    this._contentRecords.clear();
  }
}
