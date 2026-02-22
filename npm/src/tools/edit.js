/**
 * Edit and Create tools for file modification
 * @module tools/edit
 */

import { tool } from 'ai';
import { promises as fs } from 'fs';
import { dirname, resolve, isAbsolute, sep } from 'path';
import { existsSync } from 'fs';
import { toRelativePath, safeRealpath } from '../utils/path-validation.js';
import { findFuzzyMatch } from './fuzzyMatch.js';
import { findSymbol, detectBaseIndent, reindent } from './symbolEdit.js';
import { parseLineRef, validateLineHash, computeLineHash } from './hashline.js';
import { cleanNewString } from './lineEditHeuristics.js';

/**
 * Validates that a path is within allowed directories
 * @param {string} filePath - Path to validate
 * @param {string[]} allowedFolders - List of allowed folders
 * @returns {boolean} True if path is allowed
 */
function isPathAllowed(filePath, allowedFolders) {
  if (!allowedFolders || allowedFolders.length === 0) {
    // If no restrictions, allow current directory and below
    // Use safeRealpath to resolve symlinks for security
    const resolvedPath = safeRealpath(filePath);
    const cwd = safeRealpath(process.cwd());
    // Ensure proper path separator to prevent path traversal
    return resolvedPath === cwd || resolvedPath.startsWith(cwd + sep);
  }

  // Use safeRealpath to resolve symlinks for security
  // This prevents symlink bypass attacks (e.g., /tmp -> /private/tmp on macOS)
  const resolvedPath = safeRealpath(filePath);
  return allowedFolders.some(folder => {
    const allowedPath = safeRealpath(folder);
    // Ensure proper path separator to prevent path traversal
    return resolvedPath === allowedPath || resolvedPath.startsWith(allowedPath + sep);
  });
}

/**
 * Common configuration for file tools
 * @param {Object} options - Configuration options
 * @returns {Object} Parsed configuration
 */
function parseFileToolOptions(options = {}) {
  const allowedFolders = options.allowedFolders || [];
  return {
    debug: options.debug || false,
    allowedFolders,
    cwd: options.cwd,
    // Consistent fallback chain: workspaceRoot > cwd > allowedFolders[0] > process.cwd()
    workspaceRoot: options.workspaceRoot || options.cwd || (allowedFolders.length > 0 && allowedFolders[0]) || process.cwd()
  };
}

/**
 * Handle AST-aware symbol editing (replace or insert)
 * @param {Object} params - Parameters
 * @returns {Promise<string>} Result message
 */
async function handleSymbolEdit({ resolvedPath, file_path, symbol, new_string, position, debug, cwd, fileTracker }) {
  // Validate symbol
  if (typeof symbol !== 'string' || symbol.trim() === '') {
    return 'Error editing file: Invalid symbol - must be a non-empty string. Provide the name of a function, class, method, or other named code definition (e.g. "myFunction" or "MyClass.myMethod"). To edit by text matching instead, use old_string + new_string.';
  }

  // Validate position if provided
  if (position !== undefined && position !== null && position !== 'before' && position !== 'after') {
    return 'Error editing file: Invalid position - must be "before" or "after". Use position="before" to insert code above the symbol, or position="after" to insert code below it. Omit position entirely to replace the symbol with new_string.';
  }

  // Find the symbol using AST (always re-reads the file — cheap, ~100ms)
  const symbolInfo = await findSymbol(resolvedPath, symbol, cwd || process.cwd());
  if (!symbolInfo) {
    return `Error editing file: Symbol "${symbol}" not found in ${file_path}. Verify the symbol name matches a top-level function, class, method, or other named definition exactly as declared in the source. Use 'search' or 'extract' to inspect the file and find the correct symbol name. Alternatively, use old_string + new_string for text-based editing instead.`;
  }

  // Symbol content verification — check if symbol changed since LLM last read it
  if (fileTracker) {
    const check = fileTracker.checkSymbolContent(resolvedPath, symbol, symbolInfo.code);
    if (!check.ok && check.reason === 'stale') {
      return `Error editing ${file_path}: Symbol "${symbol}" has changed since you last read it. Use extract to re-read the current content, then retry.\n\nExample: <extract><targets>${file_path}#${symbol}</targets></extract>`;
    }
  }

  // Read the file
  const content = await fs.readFile(resolvedPath, 'utf-8');
  const lines = content.split('\n');

  if (position) {
    // Insert mode: add code before/after the symbol
    const refIndent = detectBaseIndent(symbolInfo.code);
    const reindented = reindent(new_string, refIndent);
    const newLines = reindented.split('\n');

    if (position === 'after') {
      lines.splice(symbolInfo.endLine, 0, '', ...newLines);
    } else {
      lines.splice(symbolInfo.startLine - 1, 0, ...newLines, '');
    }

    await fs.writeFile(resolvedPath, lines.join('\n'), 'utf-8');
    if (fileTracker) {
      // Re-read symbol to get updated position and content for chained edits
      const updated = await findSymbol(resolvedPath, symbol, cwd || process.cwd());
      if (updated) {
        fileTracker.trackSymbolAfterWrite(resolvedPath, symbol, updated.code, updated.startLine, updated.endLine);
      }
      fileTracker.markFileSeen(resolvedPath);
    }

    const insertLine = position === 'after' ? symbolInfo.endLine + 1 : symbolInfo.startLine;

    if (debug) {
      console.error(`[Edit] Successfully inserted ${newLines.length} lines ${position} "${symbol}" at line ${insertLine} in ${resolvedPath}`);
    }

    return `Successfully inserted ${newLines.length} lines ${position} symbol "${symbol}" in ${file_path} (at line ${insertLine})`;
  } else {
    // Replace mode: replace entire symbol with new content
    const originalIndent = detectBaseIndent(symbolInfo.code);
    const reindented = reindent(new_string, originalIndent);
    const newLines = reindented.split('\n');

    lines.splice(symbolInfo.startLine - 1, symbolInfo.endLine - symbolInfo.startLine + 1, ...newLines);
    await fs.writeFile(resolvedPath, lines.join('\n'), 'utf-8');
    if (fileTracker) {
      // Re-read symbol to get updated position and content for chained edits
      const updated = await findSymbol(resolvedPath, symbol, cwd || process.cwd());
      if (updated) {
        fileTracker.trackSymbolAfterWrite(resolvedPath, symbol, updated.code, updated.startLine, updated.endLine);
      }
      fileTracker.markFileSeen(resolvedPath);
    }

    if (debug) {
      console.error(`[Edit] Successfully replaced symbol "${symbol}" in ${resolvedPath} (lines ${symbolInfo.startLine}-${symbolInfo.endLine})`);
    }

    return `Successfully replaced symbol "${symbol}" in ${file_path} (was lines ${symbolInfo.startLine}-${symbolInfo.endLine}, now ${newLines.length} lines)`;
  }
}

/**
 * Build a response message for line-targeted edits with context lines and hashes.
 * @param {string} file_path - Display path
 * @param {number} startLine - 1-indexed start line (original)
 * @param {number} endLine - 1-indexed end line (original)
 * @param {number} newLineCount - Number of lines in replacement
 * @param {string[]} updatedLines - All file lines after edit
 * @param {number} insertOffset - Where new content starts (0-indexed in updatedLines)
 * @param {string} action - Description of what happened
 * @param {string[]} heuristicMods - Heuristic modifications applied
 * @returns {string} Formatted response
 */
function buildLineEditResponse(file_path, startLine, endLine, newLineCount, updatedLines, insertOffset, action, heuristicMods) {
  const contextBefore = 1;
  const contextAfter = 1;

  const contextStart = Math.max(0, insertOffset - contextBefore);
  const contextEnd = Math.min(updatedLines.length, insertOffset + newLineCount + contextAfter);

  let context = 'Context:\n';
  for (let i = contextStart; i < contextEnd; i++) {
    const lineNum = i + 1;
    const hash = computeLineHash(updatedLines[i]);
    const isNew = i >= insertOffset && i < insertOffset + newLineCount;
    const marker = isNew ? '>' : ' ';
    context += `${marker} ${lineNum}:${hash} | ${updatedLines[i]}\n`;
  }

  let msg = `Successfully edited ${file_path} (${action})`;
  if (heuristicMods.length > 0) {
    msg += ` [auto-corrected: ${heuristicMods.join(', ')}]`;
  }
  msg += '\n' + context;
  return msg;
}

/**
 * Handle line-targeted editing (replace, insert, delete by line numbers)
 * @param {Object} params - Parameters
 * @returns {Promise<string>} Result message
 */
async function handleLineEdit({ resolvedPath, file_path, start_line, end_line, new_string, position, debug, fileTracker }) {
  // Parse start_line reference
  const startRef = parseLineRef(start_line);
  if (!startRef) {
    return `Error editing file: Invalid start_line '${start_line}'. Use a line number (e.g. "42") or line:hash (e.g. "42:ab"). Line numbers are 1-indexed.`;
  }

  // Parse optional end_line reference
  let endRef = null;
  if (end_line !== undefined && end_line !== null) {
    endRef = parseLineRef(end_line);
    if (!endRef) {
      return `Error editing file: Invalid end_line '${end_line}'. Use a line number (e.g. "55") or line:hash (e.g. "55:cd"). Must be >= start_line.`;
    }
  }

  const startLine = startRef.line;
  const endLine = endRef ? endRef.line : startLine;

  if (endLine < startLine) {
    return `Error editing file: end_line (${endLine}) must be >= start_line (${startLine}).`;
  }

  // Validate position if provided
  if (position !== undefined && position !== null && position !== 'before' && position !== 'after') {
    return 'Error editing file: Invalid position - must be "before" or "after". Use position="before" to insert before the line, or position="after" to insert after it.';
  }

  // Read the file
  const content = await fs.readFile(resolvedPath, 'utf-8');
  const fileLines = content.split('\n');

  // Validate line numbers in range
  if (startLine > fileLines.length) {
    return `Error editing file: Line ${startLine} is beyond file length (${fileLines.length} lines). Use 'extract' to read the current file content.`;
  }
  if (endLine > fileLines.length) {
    return `Error editing file: Line ${endLine} is beyond file length (${fileLines.length} lines). Use 'extract' to read the current file content.`;
  }

  // Validate hashes if present
  if (startRef.hash) {
    const validation = validateLineHash(startLine, startRef.hash, fileLines);
    if (!validation.valid) {
      return `Error editing file: Line ${startLine} has changed since last read. Expected hash '${startRef.hash}' but content is now: ${startLine}:${validation.actualHash} | ${validation.actualContent}. Use '${startLine}:${validation.actualHash}' instead.`;
    }
  }
  if (endRef && endRef.hash) {
    const validation = validateLineHash(endLine, endRef.hash, fileLines);
    if (!validation.valid) {
      return `Error editing file: Line ${endLine} has changed since last read. Expected hash '${endRef.hash}' but content is now: ${endLine}:${validation.actualHash} | ${validation.actualContent}. Use '${endLine}:${validation.actualHash}' instead.`;
    }
  }

  // Run heuristic cleaning
  const { cleaned, modifications } = cleanNewString(new_string, fileLines, startLine, endLine, position);

  if (debug) {
    if (modifications.length > 0) {
      console.error(`[Edit] Heuristic corrections: ${modifications.join(', ')}`);
    }
  }

  // Apply the edit
  const newLines = cleaned === '' ? [] : cleaned.split('\n');

  if (position === 'after') {
    // Insert after the anchor line
    fileLines.splice(startLine, 0, ...newLines);
    await fs.writeFile(resolvedPath, fileLines.join('\n'), 'utf-8');
    if (fileTracker) await fileTracker.trackFileAfterWrite(resolvedPath);
    const action = `${newLines.length} line${newLines.length !== 1 ? 's' : ''} inserted after line ${startLine}`;
    return buildLineEditResponse(file_path, startLine, startLine, newLines.length, fileLines, startLine, action, modifications);
  } else if (position === 'before') {
    // Insert before the anchor line
    fileLines.splice(startLine - 1, 0, ...newLines);
    await fs.writeFile(resolvedPath, fileLines.join('\n'), 'utf-8');
    if (fileTracker) await fileTracker.trackFileAfterWrite(resolvedPath);
    const action = `${newLines.length} line${newLines.length !== 1 ? 's' : ''} inserted before line ${startLine}`;
    return buildLineEditResponse(file_path, startLine, startLine, newLines.length, fileLines, startLine - 1, action, modifications);
  } else {
    // Replace mode: replace lines startLine through endLine (inclusive)
    const replacedCount = endLine - startLine + 1;
    fileLines.splice(startLine - 1, replacedCount, ...newLines);
    await fs.writeFile(resolvedPath, fileLines.join('\n'), 'utf-8');
    if (fileTracker) await fileTracker.trackFileAfterWrite(resolvedPath);

    let action;
    if (newLines.length === 0) {
      action = `${replacedCount} line${replacedCount !== 1 ? 's' : ''} deleted (lines ${startLine}-${endLine})`;
    } else if (startLine === endLine) {
      action = `line ${startLine} replaced with ${newLines.length} line${newLines.length !== 1 ? 's' : ''}`;
    } else {
      action = `lines ${startLine}-${endLine} replaced with ${newLines.length} line${newLines.length !== 1 ? 's' : ''}`;
    }
    return buildLineEditResponse(file_path, startLine, endLine, newLines.length, fileLines, startLine - 1, action, modifications);
  }
}

/**
 * Edit tool generator - supports text replacement and AST-aware symbol editing
 *
 * @param {Object} [options] - Configuration options
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @param {string[]} [options.allowedFolders] - Allowed directories for file operations
 * @param {string} [options.cwd] - Working directory
 * @returns {Object} Configured edit tool
 */
export const editTool = (options = {}) => {
  const { debug, allowedFolders, cwd, workspaceRoot } = parseFileToolOptions(options);

  return tool({
    name: 'edit',
    description: `Edit files using text replacement, AST-aware symbol operations, or line-targeted editing.

Modes:
1. Text edit: Provide old_string + new_string to find and replace text (with fuzzy matching fallback)
2. Symbol replace: Provide symbol + new_string to replace an entire function/class/method by name
3. Symbol insert: Provide symbol + new_string + position to insert code before/after a symbol
4. Line-targeted edit: Provide start_line + new_string to edit by line number (from extract/search output)

Parameters:
- file_path: Path to the file to edit (absolute or relative)
- new_string: Replacement text or new code content
- old_string: (optional) Text to find and replace. If omitted, symbol or start_line must be provided.
- replace_all: (optional) Replace all occurrences (text mode only)
- symbol: (optional) Symbol name for AST-aware editing (e.g. "myFunction", "MyClass.myMethod")
- position: (optional) "before" or "after" — insert code near a symbol or line instead of replacing it
- start_line: (optional) Line reference (e.g. "42" or "42:ab") for line-targeted editing
- end_line: (optional) End of line range, inclusive (e.g. "55" or "55:cd")`,

    inputSchema: {
      type: 'object',
      properties: {
        file_path: {
          type: 'string',
          description: 'Path to the file to edit'
        },
        old_string: {
          type: 'string',
          description: 'Text to find and replace (for text-based editing)'
        },
        new_string: {
          type: 'string',
          description: 'Replacement text or new code content'
        },
        replace_all: {
          type: 'boolean',
          description: 'Replace all occurrences (default: false, text mode only)',
          default: false
        },
        symbol: {
          type: 'string',
          description: 'Symbol name for AST-aware editing (e.g. "myFunction", "MyClass.myMethod")'
        },
        position: {
          type: 'string',
          enum: ['before', 'after'],
          description: 'Insert before/after symbol or line (requires symbol or start_line, omit to replace)'
        },
        start_line: {
          type: 'string',
          description: 'Line reference for line-targeted editing (e.g. "42" or "42:ab" with hash)'
        },
        end_line: {
          type: 'string',
          description: 'End of line range, inclusive (e.g. "55" or "55:cd"). Defaults to start_line.'
        }
      },
      required: ['file_path', 'new_string']
    },

    execute: async ({ file_path, old_string, new_string, replace_all = false, symbol, position, start_line, end_line }) => {
      try {
        // Validate input parameters
        if (!file_path || typeof file_path !== 'string' || file_path.trim() === '') {
          return `Error editing file: Invalid file_path - must be a non-empty string. Provide an absolute path or a path relative to the working directory (e.g. "src/main.js").`;
        }
        if (new_string === undefined || new_string === null || typeof new_string !== 'string') {
          return `Error editing file: Invalid new_string - must be a string. Provide the replacement content as a string value (empty string "" is valid for deletions).`;
        }

        // Resolve the file path
        const resolvedPath = isAbsolute(file_path) ? file_path : resolve(cwd || process.cwd(), file_path);

        if (debug) {
          console.error(`[Edit] Attempting to edit file: ${resolvedPath}`);
        }

        // Check if path is allowed
        if (!isPathAllowed(resolvedPath, allowedFolders)) {
          const relativePath = toRelativePath(resolvedPath, workspaceRoot);
          return `Error editing file: Permission denied - ${relativePath} is outside allowed directories. Use a file path within the project workspace.`;
        }

        // Check if file exists
        if (!existsSync(resolvedPath)) {
          return `Error editing file: File not found - ${file_path}. Verify the path is correct and the file exists. Use 'search' to find files by name, or 'create' to make a new file.`;
        }

        // Check if file has been seen in this session (read-before-write guard)
        if (options.fileTracker && !options.fileTracker.isFileSeen(resolvedPath)) {
          const displayPath = toRelativePath(resolvedPath, workspaceRoot);
          return `Error editing ${displayPath}: This file has not been read yet in this session. Use 'extract' to read the file first, then retry your edit. This ensures you are working with the current file content.\n\nExample: <extract><targets>${displayPath}</targets></extract>`;
        }

        // Route to appropriate mode (priority: symbol > start_line > old_string)
        if (symbol !== undefined && symbol !== null) {
          // AST-aware symbol mode (includes empty string which handleSymbolEdit validates)
          return await handleSymbolEdit({ resolvedPath, file_path, symbol, new_string, position, debug, cwd, fileTracker: options.fileTracker });
        }

        if (start_line !== undefined && start_line !== null) {
          // Line-targeted mode
          return await handleLineEdit({ resolvedPath, file_path, start_line, end_line, new_string, position, debug, fileTracker: options.fileTracker });
        }

        if (old_string === undefined || old_string === null) {
          return 'Error editing file: Must provide either old_string (for text edit), symbol (for AST-aware edit), or start_line (for line-targeted edit). For text editing: set old_string to the exact text to find and new_string to its replacement. For symbol editing: set symbol to a function/class/method name (e.g. "myFunction"). For line-targeted editing: set start_line to a line number from extract/search output (e.g. "42" or "42:ab").';
        }

        // Validate old_string for text mode
        if (typeof old_string !== 'string') {
          return `Error editing file: Invalid old_string - must be a string. Provide the exact text to find in the file, or use the symbol parameter instead for AST-aware editing by name.`;
        }

        // ─── Text-based edit mode ───

        // Read the file
        const content = await fs.readFile(resolvedPath, 'utf-8');

        // Try exact match first, fall back to fuzzy matching
        let matchTarget = old_string;
        let matchStrategy = 'exact';

        if (!content.includes(old_string)) {
          // Exact match failed — try progressive fuzzy matching
          const fuzzy = findFuzzyMatch(content, old_string);
          if (!fuzzy) {
            return `Error editing file: String not found - the specified old_string was not found in ${file_path}. The text may have changed or differ from what you expected. Try: (1) Use 'search' or 'extract' to read the current file content and copy the exact text. (2) Use the symbol parameter to edit by function/class name instead. (3) Verify the file_path is correct.`;
          }
          matchTarget = fuzzy.matchedText;
          matchStrategy = fuzzy.strategy;
          if (debug) {
            console.error(`[Edit] Exact match failed, used ${matchStrategy} matching`);
          }
        }

        // Count occurrences of the matched text
        const occurrences = content.split(matchTarget).length - 1;

        // Check uniqueness if not replacing all
        if (!replace_all && occurrences > 1) {
          return `Error editing file: Multiple occurrences found - the old_string appears ${occurrences} times in ${file_path}. To fix: (1) Set replace_all=true to replace all occurrences, or (2) Include more surrounding lines in old_string to make the match unique (add the full line or adjacent lines for context).`;
        }

        // Perform the replacement
        let newContent;
        if (replace_all) {
          newContent = content.replaceAll(matchTarget, new_string);
        } else {
          newContent = content.replace(matchTarget, new_string);
        }

        // Check if replacement was made
        if (newContent === content) {
          return `Error editing file: No changes made - the replacement result is identical to the original. Verify that old_string and new_string are actually different. If fuzzy matching was used, the matched text may already equal new_string.`;
        }

        // Write the file back
        await fs.writeFile(resolvedPath, newContent, 'utf-8');
        if (options.fileTracker) await options.fileTracker.trackFileAfterWrite(resolvedPath);

        const replacedCount = replace_all ? occurrences : 1;

        if (debug) {
          console.error(`[Edit] Successfully edited ${resolvedPath}, replaced ${replacedCount} occurrence(s)`);
        }

        // Return success message as a string (matching other tools pattern)
        const strategyNote = matchStrategy !== 'exact' ? `, matched via ${matchStrategy}` : '';
        return `Successfully edited ${file_path} (${replacedCount} replacement${replacedCount !== 1 ? 's' : ''}${strategyNote})`;

      } catch (error) {
        console.error('[Edit] Error:', error);
        return `Error editing file: ${error.message}`;
      }
    }
  });
};

/**
 * Create tool generator - Create new files
 *
 * @param {Object} [options] - Configuration options
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @param {string[]} [options.allowedFolders] - Allowed directories for file operations
 * @param {string} [options.cwd] - Working directory
 * @returns {Object} Configured create tool
 */
export const createTool = (options = {}) => {
  const { debug, allowedFolders, cwd, workspaceRoot } = parseFileToolOptions(options);

  return tool({
    name: 'create',
    description: `Create new files with specified content.

This tool creates new files in the filesystem. It will create parent directories if they don't exist.

Parameters:
- file_path: Path where the file should be created (absolute or relative)
- content: Content to write to the file
- overwrite: (optional) Whether to overwrite if file exists (default: false)

Important:
- By default, will fail if the file already exists
- Set overwrite: true to replace existing files
- Parent directories will be created automatically if needed`,

    inputSchema: {
      type: 'object',
      properties: {
        file_path: {
          type: 'string',
          description: 'Path where the file should be created'
        },
        content: {
          type: 'string',
          description: 'Content to write to the file'
        },
        overwrite: {
          type: 'boolean',
          description: 'Overwrite if file exists (default: false)',
          default: false
        }
      },
      required: ['file_path', 'content']
    },

    execute: async ({ file_path, content, overwrite = false }) => {
      try {
        // Validate input parameters
        if (!file_path || typeof file_path !== 'string' || file_path.trim() === '') {
          return `Error creating file: Invalid file_path - must be a non-empty string. Provide an absolute path or a path relative to the working directory (e.g. "src/newFile.js").`;
        }
        if (content === undefined || content === null || typeof content !== 'string') {
          return `Error creating file: Invalid content - must be a string. Provide the file content as a string value (empty string "" is valid for an empty file).`;
        }

        // Resolve the file path
        const resolvedPath = isAbsolute(file_path) ? file_path : resolve(cwd || process.cwd(), file_path);

        if (debug) {
          console.error(`[Create] Attempting to create file: ${resolvedPath}`);
        }

        // Check if path is allowed
        if (!isPathAllowed(resolvedPath, allowedFolders)) {
          const relativePath = toRelativePath(resolvedPath, workspaceRoot);
          return `Error creating file: Permission denied - ${relativePath} is outside allowed directories. Use a file path within the project workspace.`;
        }

        // Check if file exists
        if (existsSync(resolvedPath) && !overwrite) {
          return `Error creating file: File already exists - ${file_path}. Use overwrite: true to replace it.`;
        }

        // Check if file existed before write
        const existed = existsSync(resolvedPath);

        // Ensure parent directory exists
        const dir = dirname(resolvedPath);
        await fs.mkdir(dir, { recursive: true });

        // Write the file
        await fs.writeFile(resolvedPath, content, 'utf-8');
        if (options.fileTracker) await options.fileTracker.trackFileAfterWrite(resolvedPath);

        const action = existed && overwrite ? 'overwrote' : 'created';
        const bytes = Buffer.byteLength(content, 'utf-8');

        if (debug) {
          console.error(`[Create] Successfully ${action} ${resolvedPath}`);
        }

        // Return success message as a string (matching other tools pattern)
        return `Successfully ${action} ${file_path} (${bytes} bytes)`;

      } catch (error) {
        console.error('[Create] Error:', error);
        return `Error creating file: ${error.message}`;
      }
    }
  });
};

// Export schemas for tool definitions
export const editSchema = {
  type: 'object',
  properties: {
    file_path: {
      type: 'string',
      description: 'Path to the file to edit'
    },
    old_string: {
      type: 'string',
      description: 'Text to find and replace (for text-based editing)'
    },
    new_string: {
      type: 'string',
      description: 'Replacement text or new code content'
    },
    replace_all: {
      type: 'boolean',
      description: 'Replace all occurrences (default: false, text mode only)'
    },
    symbol: {
      type: 'string',
      description: 'Symbol name for AST-aware editing (e.g. "myFunction", "MyClass.myMethod")'
    },
    position: {
      type: 'string',
      enum: ['before', 'after'],
      description: 'Insert before/after symbol or line (requires symbol or start_line, omit to replace)'
    },
    start_line: {
      type: 'string',
      description: 'Line reference for line-targeted editing (e.g. "42" or "42:ab" with hash)'
    },
    end_line: {
      type: 'string',
      description: 'End of line range, inclusive (e.g. "55" or "55:cd"). Defaults to start_line.'
    }
  },
  required: ['file_path', 'new_string']
};

export const createSchema = {
  type: 'object',
  properties: {
    file_path: {
      type: 'string',
      description: 'Path where the file should be created'
    },
    content: {
      type: 'string',
      description: 'Content to write to the file'
    },
    overwrite: {
      type: 'boolean',
      description: 'Overwrite if file exists (default: false)'
    }
  },
  required: ['file_path', 'content']
};

// Tool descriptions for XML definitions
export const editDescription = 'Edit files using text replacement, AST-aware symbol operations, or line-targeted editing. Supports fuzzy matching for text edits and optional hash-based integrity verification for line edits.';
export const createDescription = 'Create new files with specified content. Will create parent directories if needed.';

// XML tool definitions
export const editToolDefinition = `
## edit
Description: ${editDescription}

Four editing modes — choose based on the scope of your change:

1. **Text edit** (old_string + new_string): For small, precise changes — fix a condition, rename a variable, update a value. Provide old_string copied verbatim from the file and new_string with the replacement. Fuzzy matching handles minor whitespace/indentation differences automatically, but always try to copy the exact text.

2. **Symbol replace** (symbol + new_string): For replacing an entire function, class, or method by name. No need to quote the old code — just provide the symbol name and the full new implementation. Indentation is automatically adjusted to match the original. Prefer this mode when rewriting whole definitions.

3. **Symbol insert** (symbol + new_string + position): For adding new code before or after an existing symbol. Set position to "before" or "after".

4. **Line-targeted edit** (start_line + new_string): For precise edits using line numbers from extract/search output. Use start_line with a line number (e.g. "42") or line:hash (e.g. "42:ab") for integrity verification. Add end_line for multi-line ranges. Use position="before" or "after" to insert instead of replace.

Parameters:
- file_path: (required) Path to the file to edit
- new_string: (required) Replacement text or new code content
- old_string: (optional) Text to find and replace — copy verbatim from the file, do not paraphrase or reformat
- replace_all: (optional, default: false) Replace all occurrences of old_string (text mode only)
- symbol: (optional) Name of a code symbol (e.g. "myFunction", "MyClass.myMethod") — must match a function, class, or method definition
- position: (optional) "before" or "after" — insert new_string near the symbol or line instead of replacing it
- start_line: (optional) Line reference for line-targeted editing (e.g. "42" or "42:ab")
- end_line: (optional) End of line range, inclusive (e.g. "55" or "55:cd"). Defaults to start_line.

Mode selection rules (priority order):
- If symbol is provided, symbol mode is used (old_string and start_line are ignored)
- If start_line is provided (without symbol), line-targeted mode is used
- If old_string is provided (without symbol or start_line), text mode is used
- If none are provided, the tool returns an error with guidance

When to use each mode:
- Small edits (a line or a few lines): use text mode with old_string
- Replacing entire functions/classes/methods: use symbol mode — no exact text matching needed
- Editing specific lines from extract/search output: use line-targeted mode with start_line
- Editing inside large functions without rewriting them entirely: first use extract with the symbol target (e.g. "file.js#myFunction") to see the function with line numbers, then use start_line/end_line to edit specific lines within it

Error handling:
- If an edit fails, read the error message carefully — it contains specific instructions for how to fix the call and retry
- Common fixes: use 'search'/'extract' to get exact file content, add more context to old_string, switch between text and symbol modes
- Line-targeted hash mismatch: the file changed since last read; the error provides updated line:hash references

Examples:

Text edit (find and replace):
<edit>
<file_path>src/main.js</file_path>
<old_string>return false;</old_string>
<new_string>return true;</new_string>
</edit>

Text edit with replace_all:
<edit>
<file_path>config.json</file_path>
<old_string>"debug": false</old_string>
<new_string>"debug": true</new_string>
<replace_all>true</replace_all>
</edit>

Symbol replace (rewrite entire function by name):
<edit>
<file_path>src/utils.js</file_path>
<symbol>calculateTotal</symbol>
<new_string>function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price * item.quantity, 0);
}</new_string>
</edit>

Symbol insert (add new function after existing one):
<edit>
<file_path>src/utils.js</file_path>
<symbol>calculateTotal</symbol>
<position>after</position>
<new_string>function calculateTax(total, rate) {
  return total * rate;
}</new_string>
</edit>

Line-targeted edit (replace a line):
<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<new_string>  return processItems(order.items);</new_string>
</edit>

Line-targeted edit (replace a range of lines):
<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<end_line>55</end_line>
<new_string>  // simplified implementation
  return processItems(order.items);</new_string>
</edit>

Line-targeted edit with hash verification:
<edit>
<file_path>src/main.js</file_path>
<start_line>42:ab</start_line>
<end_line>55:cd</end_line>
<new_string>  return processItems(order.items);</new_string>
</edit>

Line-targeted insert (add code after a line):
<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<position>after</position>
<new_string>  const validated = validate(input);</new_string>
</edit>

Line-targeted delete (remove lines):
<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<end_line>45</end_line>
<new_string></new_string>
</edit>`;

export const createToolDefinition = `
## create
Description: ${createDescription}

When to use:
- For creating brand new files from scratch
- When you need to add configuration files, documentation, or new modules
- For generating boilerplate code or templates
- When you have the complete content ready to write

When NOT to use:
- For editing existing files (use 'edit' tool instead)
- When a file already exists unless you explicitly want to overwrite it

Parameters:
- file_path: (required) Path where the file should be created
- content: (required) Complete content to write to the file
- overwrite: (optional, default: false) Whether to overwrite if file already exists

Important notes:
- Parent directories will be created automatically if they don't exist
- The tool will fail if the file already exists and overwrite is false
- Be careful with the overwrite option as it completely replaces existing files

Examples:
<create>
<file_path>src/newFile.js</file_path>
<content>export function hello() {
  return "Hello, world!";
}</content>
</create>

<create>
<file_path>README.md</file_path>
<content># My Project

This is a new project.</content>
<overwrite>true</overwrite>
</create>`;
