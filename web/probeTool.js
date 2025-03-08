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
 * This tool allows AI to search through code repositories using the Probe tool's capabilities
 */
export const probeTool = tool({
	// Description helps the AI understand when to use this tool
	description: 'Search code in the repository using patterns. Use this tool to find relevant code snippets based on keywords or patterns. Always use this tool first before attempting to answer questions about the codebase.',

	// Define the parameters schema using Zod - simplified to just query and folder
	parameters: z.object({
		keywords: z.string().describe('Search pattern. Try to use simpler queries and focus on keywords that can appear in code. For example, use "config" instead of "configuration settings".'),
		folder: z.string().optional().describe('Path to search in. Specify one of the allowed folders.'),
		exact: z.boolean().optional().default(false).describe('Use exact match when you explicitly want to match specific search pattern, without stemming. Used when you exactly know function or Struct name.'),
		allow_tests: z.boolean().optional().default(false).describe('Allow test files in search results.')
	}),

	// The execute function contains the tool's logic
	execute: async ({ keywords, folder, exact, allow_tests }) => {
		try {
			// Build the command with properly escaped arguments
			let command = `probe ${escapeShellArg(keywords)}`;

			// Add folder if provided, otherwise use allowed folders from environment
			if (folder) {
				command += ` ${escapeShellArg(folder)}`;
			} else if (process.env.ALLOWED_FOLDERS) {
				const folders = process.env.ALLOWED_FOLDERS.split(',').map(f => f.trim());
				if (folders.length > 0) {
					command += ` ${escapeShellArg(folders[0])}`;
				}
			}

			// Add exact match flag if specified
			if (exact) {
				command += ' --exact';
			}

			// Add allow tests flag if specified
			if (allow_tests) {
				command += ' --allow-tests';
			}

			// Add some sensible defaults for AI context
			command += ' --max-tokens 40000';

			console.log(`Executing probe command: ${command}`);

			// Execute the command with a timeout
			const { stdout, stderr } = await execAsync(command, { timeout: 30000 });

			if (stderr) {
				console.error(`Probe error: ${stderr}`);
			}

			console.log(`Probe results length: ${stdout.length} characters`);

			return {
				results: stdout,
				command: command,
				timestamp: new Date().toISOString()
			};
		} catch (error) {
			console.error('Error executing probe command:', error);
			return {
				error: error.message,
				command: `probe ${keywords} ${folder || ''}`,
				timestamp: new Date().toISOString()
			};
		}
	}
});