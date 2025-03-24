// Example of using the probe npm package in Node.js

import { search, query, extract, tools } from '@buger/probe';

// Basic search example
async function basicSearchExample() {
	console.log('=== Basic Search Example ===');

	try {
		const results = await search({
			path: '/path/to/your/project',
			query: 'function',
			maxResults: 5
		});

		console.log('Search results:');
		console.log(results);
	} catch (error) {
		console.error('Search error:', error);
	}
}

// Advanced search with multiple options
async function advancedSearchExample() {
	console.log('\n=== Advanced Search Example ===');

	try {
		const results = await search({
			path: '/path/to/your/project',
			query: 'config AND (parse OR tokenize)',
			ignore: ['node_modules', 'dist'],
			reranker: 'hybrid',
			frequencySearch: true,
			maxResults: 10,
			maxTokens: 20000,
			allowTests: false
		});

		console.log('Advanced search results:');
		console.log(results);
	} catch (error) {
		console.error('Advanced search error:', error);
	}
}

// Query for specific code structures
async function queryExample() {
	console.log('\n=== Query Example ===');

	try {
		// Find all JavaScript functions
		const jsResults = await query({
			path: '/path/to/your/project',
			pattern: 'function $NAME($$$PARAMS) $$$BODY',
			language: 'javascript',
			maxResults: 5
		});

		console.log('JavaScript functions:');
		console.log(jsResults);

		// Find all Rust structs
		const rustResults = await query({
			path: '/path/to/your/project',
			pattern: 'struct $NAME $$$BODY',
			language: 'rust',
			maxResults: 5
		});

		console.log('Rust structs:');
		console.log(rustResults);
	} catch (error) {
		console.error('Query error:', error);
	}
}

// Extract code blocks from specific files
async function extractExample() {
	console.log('\n=== Extract Example ===');

	try {
		const results = await extract({
			files: [
				'/path/to/your/project/src/main.js',
				'/path/to/your/project/src/utils.js:42'  // Extract from line 42
			],
			contextLines: 2,
			format: 'markdown'
		});

		console.log('Extracted code:');
		console.log(results);
	} catch (error) {
		console.error('Extract error:', error);
	}
}

// Practical example: Find and analyze all API endpoints
async function findApiEndpoints() {
	console.log('\n=== Find API Endpoints Example ===');

	try {
		// Search for route definitions in Express.js
		const expressRoutes = await search({
			path: '/path/to/your/project',
			query: 'app.get OR app.post OR app.put OR app.delete',
			maxResults: 20
		});

		console.log('Express.js routes:');
		console.log(expressRoutes);

		// Search for controller methods in a Spring Boot application
		const springControllers = await query({
			path: '/path/to/your/project',
			pattern: '@RequestMapping($$$PARAMS) $$$BODY',
			language: 'java',
			maxResults: 20
		});

		console.log('Spring Boot controllers:');
		console.log(springControllers);
	} catch (error) {
		console.error('API endpoint search error:', error);
	}
}

// Practical example: Generate documentation from code comments
async function generateDocumentation() {
	console.log('\n=== Generate Documentation Example ===');

	try {
		// Search for JSDoc comments
		const jsdocComments = await search({
			path: '/path/to/your/project',
			query: '/**',
			maxResults: 50
		});

		console.log('JSDoc comments:');
		console.log(jsdocComments);

		// Process the comments to generate documentation
		const docs = processJsDocComments(jsdocComments);
		console.log('Generated documentation:', docs);
	} catch (error) {
		console.error('Documentation generation error:', error);
	}
}

// Helper function for the documentation example
function processJsDocComments(comments) {
	// This would parse the JSDoc comments and convert them to documentation
	return "Documentation would be generated here";
}

// Practical example: Find unused code
async function findUnusedCode() {
	console.log('\n=== Find Unused Code Example ===');

	try {
		// First, find all function definitions
		const functionDefs = await query({
			path: '/path/to/your/project',
			pattern: 'function $NAME($$$PARAMS) $$$BODY',
			language: 'javascript',
			maxResults: 100
		});

		// Extract function names
		const functionNames = extractFunctionNames(functionDefs);

		// For each function, search for its usage
		const unusedFunctions = [];

		for (const name of functionNames) {
			const usages = await search({
				path: '/path/to/your/project',
				query: name,
				maxResults: 2  // We only need to know if it's used at least once
			});

			// If only one result (the definition itself), it's unused
			if (countOccurrences(usages) <= 1) {
				unusedFunctions.push(name);
			}
		}

		console.log('Potentially unused functions:');
		console.log(unusedFunctions);
	} catch (error) {
		console.error('Unused code search error:', error);
	}
}

// Helper functions for the unused code example
function extractFunctionNames(functionDefs) {
	// This would extract function names from the query results
	return ["someFunction", "anotherFunction"];
}

function countOccurrences(searchResults) {
	// This would count occurrences in the search results
	return 1;
}

// Example of using AI tools with Vercel AI SDK
async function vercelAiToolsExample() {
	console.log('\n=== Vercel AI SDK Tools Example ===');

	try {
		// Import necessary modules (in a real application)
		// import { generateText } from 'ai';
		// import { createOpenAI } from '@ai-sdk/openai';

		console.log('Using Vercel AI SDK tools:');
		console.log('- searchTool:', typeof tools.searchTool);
		console.log('- queryTool:', typeof tools.queryTool);
		console.log('- extractTool:', typeof tools.extractTool);

		// Example of how to use the tools with Vercel AI SDK
		console.log('\nExample usage with Vercel AI SDK:');
		console.log(`
async function chatWithAI(userMessage) {
  const result = await generateText({
    model: provider(modelName),
    messages: [{ role: 'user', content: userMessage }],
    system: "You are a code intelligence assistant. Use the provided tools to search and analyze code.",
    tools: {
      search: tools.searchTool,
      query: tools.queryTool,
      extract: tools.extractTool
    },
    maxSteps: 15,
    temperature: 0.7
  });
  
  return result.text;
}`);
	} catch (error) {
		console.error('Vercel AI SDK tools error:', error);
	}
}

// Example of using AI tools with LangChain
async function langchainToolsExample() {
	console.log('\n=== LangChain Tools Example ===');

	try {
		// Create the LangChain tools
		const searchTool = tools.createSearchTool();
		const queryTool = tools.createQueryTool();
		const extractTool = tools.createExtractTool();

		console.log('Created LangChain tools:');
		console.log('- searchTool:', typeof searchTool);
		console.log('- queryTool:', typeof queryTool);
		console.log('- extractTool:', typeof extractTool);

		// Example of how to use the tools with LangChain
		console.log('\nExample usage with LangChain:');
		console.log(`
async function chatWithAI(userMessage) {
  const model = new ChatOpenAI({
    modelName: "gpt-4o",
    temperature: 0.7
  }).withTools([searchTool, queryTool, extractTool]);
  
  const result = await model.invoke([
    { role: "system", content: "You are a code intelligence assistant. Use the provided tools to search and analyze code." },
    { role: "user", content: userMessage }
  ]);
  
  return result.content;
}`);
	} catch (error) {
		console.error('LangChain tools error:', error);
	}
}

// Example of using the DEFAULT_SYSTEM_MESSAGE
async function systemMessageExample() {
	console.log('\n=== System Message Example ===');

	try {
		console.log('DEFAULT_SYSTEM_MESSAGE:', typeof tools.DEFAULT_SYSTEM_MESSAGE);

		// Example of how to use the DEFAULT_SYSTEM_MESSAGE
		console.log('\nExample usage of DEFAULT_SYSTEM_MESSAGE:');
		console.log(`
// Use the default system message in your AI application
const systemMessage = tools.DEFAULT_SYSTEM_MESSAGE;

// Example with Vercel AI SDK
const result = await generateText({
  model: provider(modelName),
  messages: [{ role: 'user', content: userMessage }],
  system: tools.DEFAULT_SYSTEM_MESSAGE,
  tools: {
    search: tools.searchTool,
    query: tools.queryTool,
    extract: tools.extractTool
  }
});

// Example with LangChain
const model = new ChatOpenAI({
  modelName: "gpt-4o",
  temperature: 0.7
}).withTools([searchTool, queryTool, extractTool]);

const result = await model.invoke([
  { role: "system", content: tools.DEFAULT_SYSTEM_MESSAGE },
  { role: "user", content: userMessage }
]);`);
	} catch (error) {
		console.error('System message error:', error);
	}
}

// Run all examples
async function runAllExamples() {
	await basicSearchExample();
	await advancedSearchExample();
	await queryExample();
	await extractExample();
	await findApiEndpoints();
	await generateDocumentation();
	await findUnusedCode();
	await vercelAiToolsExample();
	await langchainToolsExample();
	await systemMessageExample();
}

runAllExamples().catch(console.error);