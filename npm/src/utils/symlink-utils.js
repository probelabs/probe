/**
 * Symlink resolution utilities for the probe package
 * @module utils/symlink-utils
 */

import fs from 'fs';
import { promises as fsPromises } from 'fs';

/**
 * Get entry type following symlinks (async version)
 *
 * Uses fs.stat() which follows symlinks to get the actual target type.
 * Falls back to dirent type if stat fails (e.g., broken symlink).
 *
 * @param {fs.Dirent} entry - Directory entry from readdir
 * @param {string} fullPath - Full path to the entry
 * @returns {Promise<{isFile: boolean, isDirectory: boolean, size: number}>}
 */
export async function getEntryType(entry, fullPath) {
	try {
		const stats = await fsPromises.stat(fullPath);
		return {
			isFile: stats.isFile(),
			isDirectory: stats.isDirectory(),
			size: stats.size
		};
	} catch {
		// Fall back to dirent type if stat fails (e.g., broken symlink)
		return {
			isFile: entry.isFile(),
			isDirectory: entry.isDirectory(),
			size: 0
		};
	}
}

/**
 * Get entry type following symlinks (sync version)
 *
 * Uses fs.statSync() which follows symlinks to get the actual target type.
 * Falls back to dirent type if stat fails (e.g., broken symlink).
 *
 * @param {fs.Dirent} entry - Directory entry from readdir
 * @param {string} fullPath - Full path to the entry
 * @returns {{isFile: boolean, isDirectory: boolean, size: number}}
 */
export function getEntryTypeSync(entry, fullPath) {
	try {
		const stats = fs.statSync(fullPath);
		return {
			isFile: stats.isFile(),
			isDirectory: stats.isDirectory(),
			size: stats.size
		};
	} catch {
		// Fall back to dirent type if stat fails (e.g., broken symlink)
		return {
			isFile: entry.isFile(),
			isDirectory: entry.isDirectory(),
			size: 0
		};
	}
}
