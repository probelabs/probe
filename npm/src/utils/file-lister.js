/**
 * File listing utility for the probe package
 * @module utils/file-lister
 */

import fs from 'fs';
import path from 'path';
import { promisify } from 'util';
import { exec } from 'child_process';

const execAsync = promisify(exec);

/**
 * List files in a directory by nesting level, respecting .gitignore
 * 
 * @param {Object} options - Options for listing files
 * @param {string} options.directory - Directory to list files from
 * @param {number} [options.maxFiles=100] - Maximum number of files to return
 * @param {boolean} [options.respectGitignore=true] - Whether to respect .gitignore
 * @returns {Promise<string[]>} - Array of file paths
 */
export async function listFilesByLevel(options) {
	const {
		directory,
		maxFiles = 100,
		respectGitignore = true
	} = options;

	// Check if directory exists
	if (!fs.existsSync(directory)) {
		throw new Error(`Directory does not exist: ${directory}`);
	}

	// Use git ls-files if .git directory exists and respectGitignore is true
	const gitDirExists = fs.existsSync(path.join(directory, '.git'));
	if (gitDirExists && respectGitignore) {
		try {
			return await listFilesUsingGit(directory, maxFiles);
		} catch (error) {
			console.error(`Warning: Failed to use git ls-files: ${error.message}`);
			console.error('Falling back to manual file listing');
		}
	}

	// Fall back to manual file listing
	return await listFilesByLevelManually(directory, maxFiles, respectGitignore);
}

/**
 * List files using git ls-files (respects .gitignore by default)
 * 
 * @param {string} directory - Directory to list files from
 * @param {number} maxFiles - Maximum number of files to return
 * @returns {Promise<string[]>} - Array of file paths
 */
async function listFilesUsingGit(directory, maxFiles) {
	// Use git ls-files to get tracked files (respects .gitignore)
	const { stdout } = await execAsync('git ls-files', { cwd: directory });

	// Split output into lines and filter out empty lines
	const files = stdout.split('\n').filter(Boolean);

	// Sort files by directory depth (breadth-first)
	const sortedFiles = files.sort((a, b) => {
		const depthA = a.split(path.sep).length;
		const depthB = b.split(path.sep).length;
		return depthA - depthB;
	});

	// Limit to maxFiles
	return sortedFiles.slice(0, maxFiles);
}

/**
 * List files manually by nesting level
 * 
 * @param {string} directory - Directory to list files from
 * @param {number} maxFiles - Maximum number of files to return
 * @param {boolean} respectGitignore - Whether to respect .gitignore
 * @returns {Promise<string[]>} - Array of file paths
 */
async function listFilesByLevelManually(directory, maxFiles, respectGitignore) {
	// Load .gitignore patterns if needed
	let ignorePatterns = [];
	if (respectGitignore) {
		ignorePatterns = loadGitignorePatterns(directory);
	}

	// Initialize result array
	const result = [];

	// Initialize queue with root directory
	const queue = [{ dir: directory, level: 0 }];

	// Process queue (breadth-first)
	while (queue.length > 0 && result.length < maxFiles) {
		const { dir, level } = queue.shift();

		try {
			// Read directory contents
			const entries = fs.readdirSync(dir, { withFileTypes: true });

			// Process files first (at current level)
			const files = entries.filter(entry => entry.isFile());
			for (const file of files) {
				if (result.length >= maxFiles) break;

				const filePath = path.join(dir, file.name);
				const relativePath = path.relative(directory, filePath);

				// Skip if file matches any ignore pattern
				if (shouldIgnore(relativePath, ignorePatterns)) continue;

				result.push(relativePath);
			}

			// Then add directories to queue for next level
			const dirs = entries.filter(entry => entry.isDirectory());
			for (const subdir of dirs) {
				const subdirPath = path.join(dir, subdir.name);
				const relativeSubdirPath = path.relative(directory, subdirPath);

				// Skip if directory matches any ignore pattern
				if (shouldIgnore(relativeSubdirPath, ignorePatterns)) continue;

				// Skip node_modules and .git directories
				if (subdir.name === 'node_modules' || subdir.name === '.git') continue;

				queue.push({ dir: subdirPath, level: level + 1 });
			}
		} catch (error) {
			console.error(`Warning: Could not read directory ${dir}: ${error.message}`);
		}
	}

	return result;
}

/**
 * Load .gitignore patterns from a directory
 * 
 * @param {string} directory - Directory to load .gitignore from
 * @returns {string[]} - Array of ignore patterns
 */
function loadGitignorePatterns(directory) {
	const gitignorePath = path.join(directory, '.gitignore');

	if (!fs.existsSync(gitignorePath)) {
		return [];
	}

	try {
		const content = fs.readFileSync(gitignorePath, 'utf8');
		return content
			.split('\n')
			.map(line => line.trim())
			.filter(line => line && !line.startsWith('#'));
	} catch (error) {
		console.error(`Warning: Could not read .gitignore: ${error.message}`);
		return [];
	}
}

/**
 * Check if a file path should be ignored based on ignore patterns
 * 
 * @param {string} filePath - File path to check
 * @param {string[]} ignorePatterns - Array of ignore patterns
 * @returns {boolean} - Whether the file should be ignored
 */
function shouldIgnore(filePath, ignorePatterns) {
	if (!ignorePatterns.length) return false;

	// Simple pattern matching (could be improved with minimatch or similar)
	for (const pattern of ignorePatterns) {
		// Exact match
		if (pattern === filePath) return true;

		// Directory match (pattern ends with /)
		if (pattern.endsWith('/') && filePath.startsWith(pattern)) return true;

		// File extension match (pattern starts with *.)
		if (pattern.startsWith('*.') && filePath.endsWith(pattern.substring(1))) return true;

		// Wildcard at start (pattern starts with *)
		if (pattern.startsWith('*') && filePath.endsWith(pattern.substring(1))) return true;

		// Wildcard at end (pattern ends with *)
		if (pattern.endsWith('*') && filePath.startsWith(pattern.substring(0, pattern.length - 1))) return true;
	}

	return false;
}