/**
 * Path validation utilities for the probe package
 * @module utils/path-validation
 */

import path from 'path';
import { promises as fs, realpathSync } from 'fs';
import { PathError } from './error-types.js';

/**
 * Safely resolve symlinks for a path.
 * Returns the real path if it exists, otherwise finds the nearest existing
 * ancestor directory and resolves it, then appends the remaining path components.
 * This is important for security to prevent symlink bypass attacks.
 *
 * @param {string} inputPath - Path to resolve
 * @returns {string} Resolved real path or best-effort resolved path
 */
export function safeRealpath(inputPath) {
	try {
		return realpathSync(inputPath);
	} catch (error) {
		// If path doesn't exist, find the nearest existing ancestor
		// and resolve it, then append the remaining components.
		// This handles cases like non-existent nested paths where an ancestor
		// may be a symlink (e.g., /var -> /private/var on macOS)
		const normalized = path.normalize(inputPath);
		const parts = normalized.split(path.sep);

		// Try progressively shorter paths until we find one that exists
		for (let i = parts.length - 1; i >= 0; i--) {
			const ancestorPath = parts.slice(0, i).join(path.sep) || path.sep;
			try {
				const resolvedAncestor = realpathSync(ancestorPath);
				// Found an existing ancestor - append remaining components
				const remainingParts = parts.slice(i);
				return path.join(resolvedAncestor, ...remainingParts);
			} catch (ancestorError) {
				// This ancestor doesn't exist either, try the next one up
				continue;
			}
		}

		// No existing ancestor found, return normalized path
		return normalized;
	}
}

/**
 * Validates and normalizes a path to be used as working directory (cwd).
 *
 * Security considerations:
 * - Normalizes path to resolve '..' and '.' components
 * - Returns absolute path to prevent ambiguity
 * - Does NOT restrict access to specific directories (that's the responsibility
 *   of higher-level components like ProbeAgent with allowedFolders)
 *
 * @param {string} inputPath - The path to validate (can be a file or directory; file paths are resolved to their parent directory)
 * @param {string} [defaultPath] - Default path to use if inputPath is not provided
 * @returns {Promise<string>} Normalized absolute directory path. If inputPath is a file, returns its parent directory.
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
			// If the path is a file, resolve to its parent directory
			// This handles cases where a file path is passed as cwd
			// Use safeRealpath to resolve symlinks before extracting parent directory
			const resolvedPath = safeRealpath(normalizedPath);
			const dirPath = path.dirname(resolvedPath);
			try {
				const dirStats = await fs.stat(dirPath);
				if (dirStats.isDirectory()) {
					return safeRealpath(dirPath);
				}
			} catch (dirError) {
				// Parent directory doesn't exist or isn't accessible - fall through to throw error for original path
			}
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
 * IMPORTANT: This function returns a value for DISPLAY and CWD purposes only.
 * It is NOT a security boundary. All security checks should be performed against
 * the original allowedFolders array, not against workspaceRoot.
 *
 * When no common prefix exists (e.g., unrelated paths), returns the first folder.
 * This is intentional - the caller should use allowedFolders for security validation.
 *
 * Examples:
 * - ['/tmp/ws/tyk', '/tmp/ws/tyk-docs'] -> '/tmp/ws'
 * - ['/tmp/ws/tyk'] -> '/tmp/ws/tyk'
 * - ['/a/b', '/c/d'] -> '/a/b' (no common prefix, returns first folder for cwd)
 * - ['C:\\Users\\ws\\tyk', 'C:\\Users\\ws\\docs'] -> 'C:\\Users\\ws' (Windows)
 *
 * @param {string[]} folders - Array of absolute folder paths
 * @returns {string} Common prefix path (for display/cwd, NOT security boundary)
 */
export function getCommonPrefix(folders) {
	if (!folders || folders.length === 0) {
		return process.cwd();
	}

	if (folders.length === 1) {
		// Resolve symlinks for security
		return safeRealpath(folders[0]);
	}

	// Resolve symlinks and normalize all paths to handle mixed separators
	// This prevents symlink bypass attacks where a symlink could point outside the workspace
	const normalized = folders.map(f => safeRealpath(f));

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

	// Resolve symlinks for security to prevent bypass attacks
	// Use safeRealpath which falls back to normalized path if resolution fails
	let normalized = safeRealpath(absolutePath);
	let normalizedRoot = safeRealpath(workspaceRoot);

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
