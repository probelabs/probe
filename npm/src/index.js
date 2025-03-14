/**
 * @buger/probe - Node.js wrapper for the probe code search tool
 *
 * This module provides JavaScript functions that wrap the probe binary functionality,
 * making it easy to use probe's powerful code search capabilities in Node.js scripts.
 *
 * @module @buger/probe
 */

import { search } from './search.js';
import { query } from './query.js';
import { extract } from './extract.js';
import { getBinaryPath, setBinaryPath } from './utils.js';
import * as tools from './tools/index.js';
import { listFilesByLevel } from './utils/file-lister.js';
import { DEFAULT_SYSTEM_MESSAGE } from './tools/system-message.js';
import { searchTool, queryTool, extractTool } from './tools/vercel.js';

export {
	search,
	query,
	extract,
	getBinaryPath,
	setBinaryPath,
	listFilesByLevel,
	tools,
	DEFAULT_SYSTEM_MESSAGE,
	// Export tool generators directly
	searchTool,
	queryTool,
	extractTool
};