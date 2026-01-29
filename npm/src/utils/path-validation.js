/**
 * Path validation utilities for the probe package
 * @module utils/path-validation
 */

import path from 'path';
import { promises as fs } from 'fs';
import { PathError } from './error-types.js';

/**
 * Validates and normalizes a path to be used as working directory (cwd).
 *
 * Security considerations:
 * - Normalizes path to resolve '..' and '.' components
 * - Returns absolute path to prevent ambiguity
 * - Does NOT restrict access to specific directories (that's the responsibility
 *   of higher-level components like ProbeAgent with allowedFolders)
 *
 * @param {string} inputPath - The path to validate
 * @param {string} [defaultPath] - Default path to use if inputPath is not provided
 * @returns {Promise<string>} Normalized absolute path
 * @throws {PathError} If the path is invalid or doesn't exist
 */
export async function validateCwdPath(inputPath, defaultPath = process.cwd()) {
	// Use default if not provided
	const targetPath = inputPath || defaultPath;

	// Normalize and resolve to absolute path
	// This handles '..' traversal and makes the path unambiguous
	const normalizedPath = path.normalize(path.resolve(targetPath));

	// Verify the path exists and is a directory
	try {
		const stats = await fs.stat(normalizedPath);
		if (!stats.isDirectory()) {
			throw new PathError(`Path is not a directory: ${normalizedPath}`, {
				suggestion: 'The specified path is a file, not a directory. Please provide a directory path for searching.',
				details: { path: normalizedPath, type: 'file' }
			});
		}
	} catch (error) {
		// Re-throw if already a PathError
		if (error instanceof PathError) {
			throw error;
		}
		if (error.code === 'ENOENT') {
			throw new PathError(`Path does not exist: ${normalizedPath}`, {
				suggestion: 'The specified path does not exist. Please verify the path is correct or use a different directory.',
				details: { path: normalizedPath }
			});
		}
		if (error.code === 'EACCES') {
			throw new PathError(`Permission denied: ${normalizedPath}`, {
				recoverable: false,
				suggestion: 'Permission denied accessing this path. This is a system-level restriction.',
				details: { path: normalizedPath }
			});
		}
		throw error;
	}

	return normalizedPath;
}

/**
 * Validates a path option without requiring it to exist.
 * Use this for paths that might be created or are optional.
 *
 * @param {string} inputPath - The path to validate
 * @param {string} [defaultPath] - Default path to use if inputPath is not provided
 * @returns {string} Normalized absolute path
 */
export function normalizePath(inputPath, defaultPath = process.cwd()) {
	const targetPath = inputPath || defaultPath;
	return path.normalize(path.resolve(targetPath));
}
