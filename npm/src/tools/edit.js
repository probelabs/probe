/**
 * Edit and Create tools for file modification
 * @module tools/edit
 */

import { tool } from 'ai';
import { promises as fs } from 'fs';
import { dirname, resolve, isAbsolute, sep } from 'path';
import { existsSync } from 'fs';

/**
 * Validates that a path is within allowed directories
 * @param {string} filePath - Path to validate
 * @param {string[]} allowedFolders - List of allowed folders
 * @returns {boolean} True if path is allowed
 */
function isPathAllowed(filePath, allowedFolders) {
  if (!allowedFolders || allowedFolders.length === 0) {
    // If no restrictions, allow current directory and below
    const resolvedPath = resolve(filePath);
    const cwd = resolve(process.cwd());
    // Ensure proper path separator to prevent path traversal
    return resolvedPath === cwd || resolvedPath.startsWith(cwd + sep);
  }

  const resolvedPath = resolve(filePath);
  return allowedFolders.some(folder => {
    const allowedPath = resolve(folder);
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
  return {
    debug: options.debug || false,
    allowedFolders: options.allowedFolders || [],
    defaultPath: options.defaultPath
  };
}

/**
 * Edit tool generator - Claude Code style string replacement
 *
 * @param {Object} [options] - Configuration options
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @param {string[]} [options.allowedFolders] - Allowed directories for file operations
 * @param {string} [options.defaultPath] - Default working directory
 * @returns {Object} Configured edit tool
 */
export const editTool = (options = {}) => {
  const { debug, allowedFolders, defaultPath } = parseFileToolOptions(options);

  return tool({
    name: 'edit',
    description: `Edit files using exact string replacement (Claude Code style).

This tool performs exact string replacements in files. It requires the old_string to match exactly what's in the file, including all whitespace and indentation.

Parameters:
- file_path: Path to the file to edit (absolute or relative)
- old_string: Exact text to find and replace (must be unique in the file unless replace_all is true)
- new_string: Text to replace with
- replace_all: (optional) Replace all occurrences instead of requiring uniqueness

Important:
- The old_string must match EXACTLY including whitespace
- If old_string appears multiple times and replace_all is false, the edit will fail
- Use larger context around the string to ensure uniqueness when needed`,

    inputSchema: {
      type: 'object',
      properties: {
        file_path: {
          type: 'string',
          description: 'Path to the file to edit'
        },
        old_string: {
          type: 'string',
          description: 'Exact text to find and replace'
        },
        new_string: {
          type: 'string',
          description: 'Text to replace with'
        },
        replace_all: {
          type: 'boolean',
          description: 'Replace all occurrences (default: false)',
          default: false
        }
      },
      required: ['file_path', 'old_string', 'new_string']
    },

    execute: async ({ file_path, old_string, new_string, replace_all = false }) => {
      try {
        // Validate input parameters
        if (!file_path || typeof file_path !== 'string' || file_path.trim() === '') {
          return `Error editing file: Invalid file_path - must be a non-empty string`;
        }
        if (old_string === undefined || old_string === null || typeof old_string !== 'string') {
          return `Error editing file: Invalid old_string - must be a string`;
        }
        if (new_string === undefined || new_string === null || typeof new_string !== 'string') {
          return `Error editing file: Invalid new_string - must be a string`;
        }

        // Resolve the file path
        const resolvedPath = isAbsolute(file_path) ? file_path : resolve(defaultPath || process.cwd(), file_path);

        if (debug) {
          console.error(`[Edit] Attempting to edit file: ${resolvedPath}`);
        }

        // Check if path is allowed
        if (!isPathAllowed(resolvedPath, allowedFolders)) {
          return `Error editing file: Permission denied - ${file_path} is outside allowed directories`;
        }

        // Check if file exists
        if (!existsSync(resolvedPath)) {
          return `Error editing file: File not found - ${file_path}`;
        }

        // Read the file
        const content = await fs.readFile(resolvedPath, 'utf-8');

        // Check if old_string exists in the file
        if (!content.includes(old_string)) {
          return `Error editing file: String not found - the specified old_string was not found in ${file_path}`;
        }

        // Count occurrences
        const occurrences = content.split(old_string).length - 1;

        // Check uniqueness if not replacing all
        if (!replace_all && occurrences > 1) {
          return `Error editing file: Multiple occurrences found - the old_string appears ${occurrences} times. Use replace_all: true to replace all occurrences, or provide more context to make the string unique.`;
        }

        // Perform the replacement
        let newContent;
        if (replace_all) {
          newContent = content.replaceAll(old_string, new_string);
        } else {
          newContent = content.replace(old_string, new_string);
        }

        // Check if replacement was made
        if (newContent === content) {
          return `Error editing file: No changes made - old_string and new_string might be the same`;
        }

        // Write the file back
        await fs.writeFile(resolvedPath, newContent, 'utf-8');

        const replacedCount = replace_all ? occurrences : 1;

        if (debug) {
          console.error(`[Edit] Successfully edited ${resolvedPath}, replaced ${replacedCount} occurrence(s)`);
        }

        // Return success message as a string (matching other tools pattern)
        return `Successfully edited ${file_path} (${replacedCount} replacement${replacedCount !== 1 ? 's' : ''})`;

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
 * @param {string} [options.defaultPath] - Default working directory
 * @returns {Object} Configured create tool
 */
export const createTool = (options = {}) => {
  const { debug, allowedFolders, defaultPath } = parseFileToolOptions(options);

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
          return `Error creating file: Invalid file_path - must be a non-empty string`;
        }
        if (content === undefined || content === null || typeof content !== 'string') {
          return `Error creating file: Invalid content - must be a string`;
        }

        // Resolve the file path
        const resolvedPath = isAbsolute(file_path) ? file_path : resolve(defaultPath || process.cwd(), file_path);

        if (debug) {
          console.error(`[Create] Attempting to create file: ${resolvedPath}`);
        }

        // Check if path is allowed
        if (!isPathAllowed(resolvedPath, allowedFolders)) {
          return `Error creating file: Permission denied - ${file_path} is outside allowed directories`;
        }

        // Check if file exists
        if (existsSync(resolvedPath) && !overwrite) {
          return `Error creating file: File already exists - ${file_path}. Use overwrite: true to replace it.`;
        }

        // Ensure parent directory exists
        const dir = dirname(resolvedPath);
        await fs.mkdir(dir, { recursive: true });

        // Write the file
        await fs.writeFile(resolvedPath, content, 'utf-8');

        const action = existsSync(resolvedPath) && overwrite ? 'overwrote' : 'created';
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
      description: 'Exact text to find and replace'
    },
    new_string: {
      type: 'string',
      description: 'Text to replace with'
    },
    replace_all: {
      type: 'boolean',
      description: 'Replace all occurrences (default: false)'
    }
  },
  required: ['file_path', 'old_string', 'new_string']
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
export const editDescription = 'Edit files using exact string replacement. Requires exact match including whitespace.';
export const createDescription = 'Create new files with specified content. Will create parent directories if needed.';

// XML tool definitions
export const editToolDefinition = `
## edit
Description: ${editDescription}

Parameters:
- file_path: (required) Path to the file to edit
- old_string: (required) Exact text to find and replace (must match including whitespace)
- new_string: (required) Text to replace with
- replace_all: (optional, default: false) Replace all occurrences

Examples:
<edit>
<file_path>src/main.js</file_path>
<old_string>function oldName() {
  return 42;
}</old_string>
<new_string>function newName() {
  return 42;
}</new_string>
</edit>

<edit>
<file_path>config.json</file_path>
<old_string>"debug": false</old_string>
<new_string>"debug": true</new_string>
<replace_all>true</replace_all>
</edit>`;

export const createToolDefinition = `
## create
Description: ${createDescription}

Parameters:
- file_path: (required) Path where the file should be created
- content: (required) Content to write to the file
- overwrite: (optional, default: false) Whether to overwrite if file exists

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