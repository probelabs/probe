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

export {
	search,
	query,
	extract,
	getBinaryPath,
	setBinaryPath,
	tools
};