/**
 * @probelabs/probe - Node.js wrapper for the probe code search tool
 *
 * This module provides JavaScript functions that wrap the probe binary functionality,
 * making it easy to use probe's powerful code search capabilities in Node.js scripts.
 *
 * @module @probelabs/probe
 */

// Load .env file if present (silent fail if not found)
import dotenv from 'dotenv';
dotenv.config();

import { search } from './search.js';
import { query } from './query.js';
import { extract } from './extract.js';
import { grep } from './grep.js';
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
	analyzeAllSchema,
	attemptCompletionSchema,
	bashSchema,
	searchToolDefinition,
	queryToolDefinition,
	extractToolDefinition,
	delegateToolDefinition,
	analyzeAllToolDefinition,
	attemptCompletionToolDefinition,
	bashToolDefinition,
	googleSearchToolDefinition,
	urlContextToolDefinition,
	parseXmlToolCall
} from './tools/common.js';
import {
	editSchema,
	createSchema,
	editToolDefinition,
	createToolDefinition
} from './tools/edit.js';
import { searchTool, queryTool, extractTool, delegateTool, analyzeAllTool } from './tools/vercel.js';
import { bashTool } from './tools/bash.js';
import { editTool, createTool } from './tools/edit.js';
import { ProbeAgent } from './agent/ProbeAgent.js';
import { SimpleTelemetry, SimpleAppTracer, initializeSimpleTelemetryFromOptions } from './agent/simpleTelemetry.js';
import { listFilesToolInstance, searchFilesToolInstance } from './agent/probeTool.js';
import { StorageAdapter, InMemoryStorageAdapter } from './agent/storage/index.js';
import { HookManager, HOOK_TYPES } from './agent/hooks/index.js';
import {
	TaskManager,
	taskSchema,
	taskToolDefinition,
	taskSystemPrompt,
	createTaskTool
} from './agent/tasks/index.js';

export {
	search,
	query,
	extract,
	grep,
	delegate,
	getBinaryPath,
	setBinaryPath,
	listFilesByLevel,
	tools,
	DEFAULT_SYSTEM_MESSAGE,
	// Export AI Agent (NEW!)
	ProbeAgent,
	// Export storage adapters
	StorageAdapter,
	InMemoryStorageAdapter,
	// Export hooks
	HookManager,
	HOOK_TYPES,
	// Export simple telemetry classes (lightweight, no heavy dependencies)
	SimpleTelemetry,
	SimpleAppTracer,
	initializeSimpleTelemetryFromOptions,
	// Export tool generators directly
	searchTool,
	queryTool,
	extractTool,
	delegateTool,
	analyzeAllTool,
	bashTool,
	editTool,
	createTool,
	// Export tool instances
	listFilesToolInstance,
	searchFilesToolInstance,
	// Export schemas
	searchSchema,
	querySchema,
	extractSchema,
	delegateSchema,
	analyzeAllSchema,
	attemptCompletionSchema,
	bashSchema,
	editSchema,
	createSchema,
	// Export tool definitions
	searchToolDefinition,
	queryToolDefinition,
	extractToolDefinition,
	delegateToolDefinition,
	analyzeAllToolDefinition,
	attemptCompletionToolDefinition,
	bashToolDefinition,
	editToolDefinition,
	createToolDefinition,
	googleSearchToolDefinition,
	urlContextToolDefinition,
	// Export parser function
	parseXmlToolCall,
	// Export task management
	TaskManager,
	taskSchema,
	taskToolDefinition,
	taskSystemPrompt,
	createTaskTool
};