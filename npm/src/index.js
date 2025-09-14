/**
 * @probelabs/probe - Node.js wrapper for the probe code search tool
 *
 * This module provides JavaScript functions that wrap the probe binary functionality,
 * making it easy to use probe's powerful code search capabilities in Node.js scripts.
 *
 * @module @probelabs/probe
 */

import { search } from './search.js';
import { query } from './query.js';
import { extract } from './extract.js';
import { delegate } from './delegate.js';
import { getBinaryPath, setBinaryPath } from './utils.js';
import * as tools from './tools/index.js';
import { listFilesByLevel } from './utils/file-lister.js';
import { DEFAULT_SYSTEM_MESSAGE } from './tools/system-message.js';
import {
	searchSchema,
	querySchema,
	extractSchema,
	delegateSchema,
	attemptCompletionSchema,
	searchToolDefinition,
	queryToolDefinition,
	extractToolDefinition,
	delegateToolDefinition,
	attemptCompletionToolDefinition,
	parseXmlToolCall
} from './tools/common.js';
import { searchTool, queryTool, extractTool, delegateTool } from './tools/vercel.js';
import { ProbeAgent } from './agent/ProbeAgent.js';

export {
	search,
	query,
	extract,
	delegate,
	getBinaryPath,
	setBinaryPath,
	listFilesByLevel,
	tools,
	DEFAULT_SYSTEM_MESSAGE,
	// Export AI Agent (NEW!)
	ProbeAgent,
	// Export tool generators directly
	searchTool,
	queryTool,
	extractTool,
	delegateTool,
	// Export schemas
	searchSchema,
	querySchema,
	extractSchema,
	delegateSchema,
	attemptCompletionSchema,
	// Export tool definitions
	searchToolDefinition,
	queryToolDefinition,
	extractToolDefinition,
	delegateToolDefinition,
	attemptCompletionToolDefinition,
	// Export parser function
	parseXmlToolCall
};