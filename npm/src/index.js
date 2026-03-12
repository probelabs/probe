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
	executePlanSchema,
	cleanupExecutePlanSchema,
	bashSchema,
	listFilesSchema,
	searchFilesSchema,
	readImageSchema,
	listSkillsSchema,
	useSkillSchema
} from './tools/common.js';
import {
	editSchema,
	createSchema,
	multiEditSchema
} from './tools/edit.js';
import { searchTool, queryTool, extractTool, delegateTool, analyzeAllTool } from './tools/vercel.js';
import { createExecutePlanTool, createCleanupExecutePlanTool } from './tools/executePlan.js';
import { bashTool } from './tools/bash.js';
import { editTool, createTool, multiEditTool } from './tools/edit.js';
import { FileTracker } from './tools/fileTracker.js';
import { ProbeAgent, ENGINE_ACTIVITY_TIMEOUT_DEFAULT, ENGINE_ACTIVITY_TIMEOUT_MIN, ENGINE_ACTIVITY_TIMEOUT_MAX } from './agent/ProbeAgent.js';
import { SimpleTelemetry, SimpleAppTracer, initializeSimpleTelemetryFromOptions } from './agent/simpleTelemetry.js';
import { listFilesToolInstance, searchFilesToolInstance } from './agent/probeTool.js';
import { StorageAdapter, InMemoryStorageAdapter } from './agent/storage/index.js';
import { HookManager, HOOK_TYPES } from './agent/hooks/index.js';
import {
	TaskManager,
	taskSchema,
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
	// Export AI Agent
	ProbeAgent,
	// Export timeout constants
	ENGINE_ACTIVITY_TIMEOUT_DEFAULT,
	ENGINE_ACTIVITY_TIMEOUT_MIN,
	ENGINE_ACTIVITY_TIMEOUT_MAX,
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
	createExecutePlanTool,
	createCleanupExecutePlanTool,
	bashTool,
	editTool,
	createTool,
	multiEditTool,
	FileTracker,
	// Export tool instances
	listFilesToolInstance,
	searchFilesToolInstance,
	// Export schemas
	searchSchema,
	querySchema,
	extractSchema,
	delegateSchema,
	analyzeAllSchema,
	executePlanSchema,
	cleanupExecutePlanSchema,
	bashSchema,
	editSchema,
	createSchema,
	multiEditSchema,
	listFilesSchema,
	searchFilesSchema,
	readImageSchema,
	listSkillsSchema,
	useSkillSchema,
	// Export task management
	TaskManager,
	taskSchema,
	taskSystemPrompt,
	createTaskTool
};
