import { exec } from 'child_process';
import { promisify } from 'util';
import { randomUUID } from 'crypto';

const execAsync = promisify(exec);

// Generate a session ID
const sessionId = process.env.PROBE_SESSION_ID || randomUUID();
console.log(`Using session ID for search caching: ${sessionId}`);

// Debug mode
const debug = process.env.DEBUG === 'true' || process.env.DEBUG === '1';

/**
 * Simple function to escape shell arguments
 */
function escapeShellArg(arg) {
  return `"${arg.replace(/"/g, '\\"')}"`;
}

/**
 * Create a CLI-based search tool
 */
export const searchTool = {
  type: 'function',
  function: {
    name: 'search',
    description: 'Search code in the repository using Elasticsearch-like query syntax. Use this tool to find relevant code snippets based on keywords or patterns.',
    parameters: {
      type: 'object',
      properties: {
        query: {
          type: 'string',
          description: 'Search query with Elasticsearch-like syntax support. Supports logical operators (AND, OR), required (+) and excluded (-) terms, and grouping with parentheses.'
        },
        path: {
          type: 'string',
          description: 'Path to search in. Specify one of the allowed folders.'
        },
        exact: {
          type: 'boolean',
          description: 'Use exact match when you explicitly want to match specific search query, without stemming.'
        },
        allow_tests: {
          type: 'boolean',
          description: 'Allow test files in search results.'
        }
      },
      required: ['query']
    }
  },
  execute: async ({ query, path, exact, allow_tests }) => {
    try {
      // Build the command with properly escaped arguments
      let command = `probe ${escapeShellArg(query)}`;

      // Add path if provided, otherwise use allowed folders from environment
      if (path && path !== '.') {
        command += ` ${escapeShellArg(path)}`;
      } else if (process.env.ALLOWED_FOLDERS) {
        const folders = process.env.ALLOWED_FOLDERS.split(',').map(f => f.trim());
        if (folders.length > 0) {
          command += ` ${escapeShellArg(folders[0])}`;
        }
      } else {
        // Use current directory if no path and no allowed folders
        command += ' .';
      }

      // Add exact match flag if specified
      if (exact) {
        command += ' --exact';
      }

      // Add allow tests flag if specified
      if (allow_tests) {
        command += ' --allow-tests';
      }

      // Add session ID for caching
      command += ` --session ${sessionId}`;

      // Add some sensible defaults for AI context
      command += ' --max-tokens 40000';

      if (debug) {
        console.log(`[DEBUG] Executing search command: ${command}`);
      } else {
        console.log(`Executing search command: ${command}`);
      }

      // Execute the command with a timeout
      const { stdout, stderr } = await execAsync(command, { timeout: 30000 });

      if (stderr) {
        console.error(`Probe search error: ${stderr}`);
      }

      console.log(`Search results length: ${stdout.length} characters`);

      return stdout;
    } catch (error) {
      console.error('Error executing search command:', error);
      return `Error: ${error.message}`;
    }
  }
};

/**
 * Create a CLI-based query tool
 */
export const queryTool = {
  type: 'function',
  function: {
    name: 'query',
    description: 'Search code using ast-grep structural pattern matching. Use this tool when you need to find specific code structures like functions, classes, or methods.',
    parameters: {
      type: 'object',
      properties: {
        pattern: {
          type: 'string',
          description: 'AST pattern to search for. Use $NAME for variable names, $$$PARAMS for parameter lists, $$$BODY for function bodies, etc.'
        },
        path: {
          type: 'string',
          description: 'Path to search in. Can be a specific file or directory.'
        },
        language: {
          type: 'string',
          description: 'Programming language to use for parsing. Must match the language of the files being searched.'
        },
        allow_tests: {
          type: 'boolean',
          description: 'Allow test files in search results'
        }
      },
      required: ['pattern']
    }
  },
  execute: async ({ pattern, path, language, allow_tests }) => {
    try {
      // Build the command with properly escaped arguments
      let command = `probe query ${escapeShellArg(pattern)}`;

      // Add path if provided, otherwise use allowed folders from environment
      if (path && path !== '.') {
        command += ` ${escapeShellArg(path)}`;
      } else if (process.env.ALLOWED_FOLDERS) {
        const folders = process.env.ALLOWED_FOLDERS.split(',').map(f => f.trim());
        if (folders.length > 0) {
          command += ` ${escapeShellArg(folders[0])}`;
        }
      } else {
        // Use current directory if no path and no allowed folders
        command += ' .';
      }

      // Add language if provided
      if (language) {
        command += ` --language ${escapeShellArg(language)}`;
      }

      // Add allow tests flag if specified
      if (allow_tests) {
        command += ' --allow-tests';
      }

      // Add session ID for caching
      command += ` --session ${sessionId}`;

      if (debug) {
        console.log(`[DEBUG] Executing query command: ${command}`);
      } else {
        console.log(`Executing query command: ${command}`);
      }

      // Execute the command with a timeout
      const { stdout, stderr } = await execAsync(command, { timeout: 30000 });

      if (stderr) {
        console.error(`Probe query error: ${stderr}`);
      }

      console.log(`Query results length: ${stdout.length} characters`);

      return stdout;
    } catch (error) {
      console.error('Error executing query command:', error);
      return `Error: ${error.message}`;
    }
  }
};

/**
 * Create a CLI-based extract tool
 */
export const extractTool = {
  type: 'function',
  function: {
    name: 'extract',
    description: 'Extract code blocks from files based on file paths and optional line numbers. Use this tool after finding relevant files with the search tool to see complete context.',
    parameters: {
      type: 'object',
      properties: {
        file_path: {
          type: 'string',
          description: 'Path to the file to extract from. Can include a line number (e.g., \'src/main.rs:42\'), a line range (e.g., \'src/main.rs:1-60\'), or a symbol name (e.g., \'src/main.rs#process_file_for_extraction\')'
        },
        line: {
          type: 'number',
          description: 'Start line number to extract a specific code block. If provided alone, the tool will find the closest suitable parent node (function, struct, class, etc.) for that line'
        },
        end_line: {
          type: 'number',
          description: 'End line number for extracting a range of lines. Used together with \'line\' parameter to specify a range'
        },
        allow_tests: {
          type: 'boolean',
          description: 'Allow test files and test code blocks'
        },
        context_lines: {
          type: 'number',
          description: 'Number of context lines to include before and after the specified line (used when no suitable code block is found)'
        },
        format: {
          type: 'string',
          description: 'Output format (plain, markdown, json, color)'
        }
      },
      required: ['file_path']
    }
  },
  execute: async ({ file_path, line, end_line, allow_tests, context_lines, format }) => {
    try {
      // Check if file_path is relative and ALLOWED_FOLDERS is set
      let fullFilePath = file_path;
      if (!file_path.startsWith('/') && process.env.ALLOWED_FOLDERS) {
        const folders = process.env.ALLOWED_FOLDERS.split(',').map(f => f.trim());
        if (folders.length > 0) {
          // Try to find the file in the first allowed folder
          fullFilePath = `${folders[0]}/${file_path}`;
        }
      }

      // Build the command with properly escaped arguments
      let command = `probe extract ${escapeShellArg(fullFilePath)}`;

      // Add line if provided
      if (line !== undefined) {
        command += ` --line ${line}`;
      }

      // Add end_line if provided
      if (end_line !== undefined) {
        command += ` --end-line ${end_line}`;
      }

      // Add allow tests flag if specified
      if (allow_tests) {
        command += ' --allow-tests';
      }

      // Add context lines if provided
      if (context_lines !== undefined) {
        command += ` --context-lines ${context_lines}`;
      }

      // Add format if provided
      if (format) {
        command += ` --format ${format}`;
      }

      // Add session ID for caching
      command += ` --session ${sessionId}`;

      if (debug) {
        console.log(`[DEBUG] Executing extract command: ${command}`);
      } else {
        console.log(`Executing extract command: ${command}`);
      }

      // Execute the command with a timeout
      const { stdout, stderr } = await execAsync(command, { timeout: 30000 });

      if (stderr) {
        console.error(`Probe extract error: ${stderr}`);
      }

      console.log(`Extract results length: ${stdout.length} characters`);

      return stdout;
    } catch (error) {
      console.error('Error executing extract command:', error);
      return `Error: ${error.message}`;
    }
  }
};