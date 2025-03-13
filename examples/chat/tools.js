import { tool } from 'ai';
import { z } from 'zod';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

/**
 * Simple function to escape shell arguments
 */
function escapeShellArg(arg) {
  return `"${arg.replace(/"/g, '\\"')}"`;
}

/**
 * Tool for searching code using the Probe CLI
 */
export const searchTool = tool({
  name: 'search',
  description: 'Search code in the repository using Elasticsearch-like query syntax. Use this tool to find relevant code snippets based on keywords or patterns. Always use this tool first before attempting to answer questions about the codebase.',
  parameters: z.object({
    query: z.string().describe('Search query with Elasticsearch-like syntax support. Supports logical operators (AND, OR), required (+) and excluded (-) terms, and grouping with parentheses. Examples: "config", "+required -excluded", "(term1 OR term2) AND term3"'),
    path: z.string().optional().describe('Path to search in. Specify one of the allowed folders.'),
    exact: z.boolean().optional().default(false).describe('Use exact match when you explicitly want to match specific search query, without stemming. Used when you exactly know function or Struct name.'),
    allow_tests: z.boolean().optional().default(false).describe('Allow test files in search results.')
  }),
  execute: async ({ query, path, exact, allow_tests }) => {
    try {
      // Build the command with properly escaped arguments
      let command = `probe ${escapeShellArg(query)}`;

      // Add path if provided, otherwise use allowed folders from environment
      // If no path and no allowed folders, use current directory
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

      // Add session ID for caching if available
      if (process.env.PROBE_SESSION_ID) {
        command += ` --session ${process.env.PROBE_SESSION_ID}`;
      }

      // Add some sensible defaults for AI context
      command += ' --max-tokens 40000';

      console.log(`Executing search command: ${command}`);

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
});

/**
 * Tool for querying code using ast-grep patterns
 */
export const queryTool = tool({
  name: 'query',
  description: 'Search code using ast-grep structural pattern matching. Use this tool when you need to find specific code structures like functions, classes, or methods.',
  parameters: z.object({
    pattern: z.string().describe('AST pattern to search for. Use $NAME for variable names, $$$PARAMS for parameter lists, $$$BODY for function bodies, etc.'),
    path: z.string().optional().default('.').describe('Path to search in. Can be a specific file or directory.'),
    language: z.string().optional().default('rust').describe('Programming language to use for parsing. Must match the language of the files being searched.'),
    allow_tests: z.boolean().optional().default(false).describe('Allow test files in search results')
  }),
  execute: async ({ pattern, path, language, allow_tests }) => {
    try {
      // Build the command with properly escaped arguments
      let command = `probe query ${escapeShellArg(pattern)}`;

      // Add path if provided, otherwise use allowed folders from environment
      // If no path and no allowed folders, use current directory
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

      // Add session ID for caching if available
      if (process.env.PROBE_SESSION_ID) {
        command += ` --session ${process.env.PROBE_SESSION_ID}`;
      }

      console.log(`Executing query command: ${command}`);

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
});

/**
 * Tool for extracting code blocks from files
 */
export const extractTool = tool({
  name: 'extract',
  description: 'Extract code blocks from files based on file paths and optional line numbers. Use this tool after finding relevant files with the search tool to see complete context.',
  parameters: z.object({
    file_path: z.string().describe('Path to the file to extract from. Can include a line number (e.g., \'src/main.rs:42\'), a line range (e.g., \'src/main.rs:1-60\'), or a symbol name (e.g., \'src/main.rs#process_file_for_extraction\')'),
    line: z.number().optional().describe('Start line number to extract a specific code block. If provided alone, the tool will find the closest suitable parent node (function, struct, class, etc.) for that line'),
    end_line: z.number().optional().describe('End line number for extracting a range of lines. Used together with \'line\' parameter to specify a range'),
    allow_tests: z.boolean().optional().default(false).describe('Allow test files and test code blocks'),
    context_lines: z.number().optional().default(10).describe('Number of context lines to include before and after the specified line (used when no suitable code block is found)'),
    format: z.string().optional().default('plain').describe('Output format (plain, markdown, json, color)')
  }),
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

      // Add session ID for caching if available
      if (process.env.PROBE_SESSION_ID) {
        command += ` --session ${process.env.PROBE_SESSION_ID}`;
      }

      console.log(`Executing extract command: ${command}`);

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
}); 