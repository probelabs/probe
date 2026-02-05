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

/**
 * Compute the common prefix (workspace root) from an array of folder paths.
 * This is useful for finding a single workspace root from multiple allowed folders.
 *
 * Examples:
 * - ['/tmp/ws/tyk', '/tmp/ws/tyk-docs'] -> '/tmp/ws'
 * - ['/tmp/ws/tyk'] -> '/tmp/ws/tyk'
 * - ['/a/b', '/c/d'] -> '/a/b' (no common prefix, returns first folder)
 * - ['C:\\Users\\ws\\tyk', 'C:\\Users\\ws\\docs'] -> 'C:\\Users\\ws' (Windows)
 *
 * @param {string[]} folders - Array of absolute folder paths
 * @returns {string} Common prefix path (workspace root)
 */
export function getCommonPrefix(folders) {
	if (!folders || folders.length === 0) {
		return process.cwd();
	}

	if (folders.length === 1) {
		return path.normalize(folders[0]);
	}

	// Normalize all paths to handle mixed separators
	const normalized = folders.map(f => path.normalize(f));

	// Split into segments
	const segments = normalized.map(f => f.split(path.sep));

	// Find minimum length
	const minLen = Math.min(...segments.map(s => s.length));

	// Find common prefix segments
	const commonSegments = [];
	for (let i = 0; i < minLen; i++) {
		const segment = segments[0][i];
		if (segments.every(s => s[i] === segment)) {
			commonSegments.push(segment);
		} else {
			break;
		}
	}

	// Handle edge cases
	if (commonSegments.length === 0) {
		// No common prefix at all, return first folder
		return normalized[0];
	}

	// Handle Windows drive letters (e.g., 'C:')
	// If only the drive letter is common, return first folder for more useful context
	if (commonSegments.length === 1 && /^[a-zA-Z]:$/.test(commonSegments[0])) {
		return normalized[0];
	}

	// Handle Unix root (empty string from split)
	// If only the root '/' is common, return first folder for more useful context
	if (commonSegments.length === 1 && commonSegments[0] === '') {
		return normalized[0];
	}

	return commonSegments.join(path.sep);
}

/**
 * Convert an absolute path to a relative path from the workspace root.
 * Returns the original path if it cannot be made relative (outside workspace).
 *
 * @param {string} absolutePath - Absolute path to convert
 * @param {string} workspaceRoot - Workspace root to compute relative path from
 * @returns {string} Relative path or original if outside workspace
 */
export function toRelativePath(absolutePath, workspaceRoot) {
	if (!absolutePath || !workspaceRoot) {
		return absolutePath;
	}

	// Normalize and remove any trailing separators for consistent comparison
	let normalized = path.normalize(absolutePath);
	let normalizedRoot = path.normalize(workspaceRoot);

	// Remove trailing separators (path.normalize doesn't always do this)
	while (normalizedRoot.length > 1 && normalizedRoot.endsWith(path.sep)) {
		normalizedRoot = normalizedRoot.slice(0, -1);
	}

	// Check if path is within workspace (exact match or starts with root + separator)
	if (normalized === normalizedRoot) {
		return '.';
	}

	if (normalized.startsWith(normalizedRoot + path.sep)) {
		return path.relative(normalizedRoot, normalized);
	}

	// Path is outside workspace, return as-is
	return absolutePath;
}
