/**
 * Path validation utilities for the probe package
 * @module utils/path-validation
 */

import path from 'path';
import fs from 'fs';

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
 * @returns {string} Normalized absolute path
 * @throws {Error} If the path is invalid or doesn't exist
 */
export function validateCwdPath(inputPath, defaultPath = process.cwd()) {
	// Use default if not provided
	const targetPath = inputPath || defaultPath;

	// Normalize and resolve to absolute path
	// This handles '..' traversal and makes the path unambiguous
	const normalizedPath = path.normalize(path.resolve(targetPath));

	// Verify the path exists and is a directory
	try {
		const stats = fs.statSync(normalizedPath);
		if (!stats.isDirectory()) {
			throw new Error(`Path is not a directory: ${normalizedPath}`);
		}
	} catch (error) {
		if (error.code === 'ENOENT') {
			throw new Error(`Path does not exist: ${normalizedPath}`);
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
