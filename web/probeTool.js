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
	description: 'Search code repositories. Try have more general queries and simpler queries. Example: instead of `rpc layer implementation` just use `rpc` etc. Avoid unnecessary verbs or nouns, focus on main keywrods.',

	// Define the parameters schema using Zod - simplified to just query and folder
	parameters: z.object({
		keywords: z.string().describe('The search query to find in the codebase. Do not add name of the folder to query. Try have more general queries and simpler queries - It is not a search engine. Example: instead of `rpc layer implementation` just use `rpc` etc. '),
		folder: z.string().optional().describe('Specific directory to search (defaults to all allowed folders from environment). Only 1 directory per call.')
	}),

	// The execute function contains the tool's logic
	execute: async ({ keywords, folder }) => {
		try {
			// Build the command with properly escaped arguments
			let command = `probe ${escapeShellArg(keywords)}`;

			// Add folder if provided, otherwise use allowed folders from environment
			if (folder) {
				command += ` ${escapeShellArg(folder)}`;
			} else if (process.env.ALLOWED_FOLDERS) {
				command += ` ${escapeShellArg(process.env.ALLOWED_FOLDERS[0])}`;
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
				command: `probe ${escapeShellArg(keywords)} ${folder ? escapeShellArg(folder) : ''}`,
				timestamp: new Date().toISOString()
			};
		}
	}
});