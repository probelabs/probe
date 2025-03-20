/**
 * Common schemas and definitions for AI tools
 * @module tools/common
 */

import { z } from 'zod';

// Common schemas for tool parameters
export const searchSchema = z.object({
	query: z.string().describe('Search query with Elasticsearch syntax. Use + for important terms.'),
	path: z.string().optional().default('.').describe('Path to search in'),
	exact: z.boolean().optional().default(false).describe('Use exact match for specific function or type names'),
	allow_tests: z.boolean().optional().default(false).describe('Allow test files in search results'),
	maxResults: z.number().optional().describe('Maximum number of results to return'),
	maxTokens: z.number().optional().default(10000).describe('Maximum number of tokens to return')
});

export const querySchema = z.object({
	pattern: z.string().describe('AST pattern to search for. Use $NAME for variable names, $$$PARAMS for parameter lists, etc.'),
	path: z.string().optional().default('.').describe('Path to search in'),
	language: z.string().optional().default('rust').describe('Programming language to use for parsing'),
	allow_tests: z.boolean().optional().default(false).describe('Allow test files in search results')
});

export const extractSchema = z.object({
	file_path: z.string().describe('Path to the file to extract from. Can include line numbers or symbol names'),
	line: z.number().optional().describe('Start line number to extract a specific code block'),
	end_line: z.number().optional().describe('End line number for extracting a range of lines'),
	allow_tests: z.boolean().optional().default(false).describe('Allow test files and test code blocks'),
	context_lines: z.number().optional().default(10).describe('Number of context lines to include'),
	format: z.string().optional().default('plain').describe('Output format (plain, markdown, json, color)')
});

// Tool descriptions
export const searchDescription = 'Search code in the repository using Elasticsearch-like query syntax. Use this tool first for any code-related questions.';
export const queryDescription = 'Search code using ast-grep structural pattern matching. Use this tool to find specific code structures like functions, classes, or methods.';
export const extractDescription = 'Extract code blocks from files based on file paths and optional line numbers. Use this tool to see complete context after finding relevant files.';