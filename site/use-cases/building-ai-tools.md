# Building AI Tools on Probe (Node.js SDK & LangChain)

This guide explains how to use Probe's Node.js SDK to build custom AI-powered code intelligence tools for your development workflow.

## Overview

Probe's Node.js SDK provides programmatic access to its powerful code search capabilities, allowing you to build custom tools, integrate with AI frameworks, and create specialized workflows for your development team.

## Installing @buger/probe as a Dependency

Start by adding Probe to your Node.js project:

```bash
# Add to your project
npm install @buger/probe

# Or install globally
npm install -g @buger/probe
```

During installation, the package will automatically download the appropriate Probe binary for your platform (Windows, macOS, or Linux).

## Basic SDK Usage

The SDK provides three main functions:

### Search: Find Code Patterns

```javascript
import { search } from '@buger/probe';

// Search for code patterns
const results = await search({
  path: '/path/to/your/project',
  query: 'authentication',
  maxResults: 10
});

console.log(results);
```

### Query: Find Specific Code Structures

```javascript
import { query } from '@buger/probe';

// Find JavaScript functions
const functions = await query({
  path: '/path/to/your/project',
  pattern: 'function $NAME($$$PARAMS) $$$BODY',
  language: 'javascript'
});

console.log(functions);
```

### Extract: Get Code from Specific Files

```javascript
import { extract } from '@buger/probe';

// Extract code from specific files
const code = await extract({
  files: [
    '/path/to/your/project/src/auth.js',
    '/path/to/your/project/src/api.js:42'  // Extract from line 42
  ]
});

console.log(code);
```

## Examples with LangChain

LangChain is a popular framework for building AI applications. Here's how to integrate Probe with LangChain:

### Creating LangChain Tools

```javascript
import { ChatOpenAI } from '@langchain/openai';
import { tools } from '@buger/probe';

// Create LangChain tools from Probe
const searchTool = tools.createSearchTool();
const queryTool = tools.createQueryTool();
const extractTool = tools.createExtractTool();

// Create a ChatOpenAI instance with tools
const model = new ChatOpenAI({
  modelName: "gpt-4o",
  temperature: 0.7
}).withTools([searchTool, queryTool, extractTool]);

// Use the model with tools
async function chatWithAI(userMessage) {
  const result = await model.invoke([
    { 
      role: "system", 
      content: "You are a code intelligence assistant. Use the provided tools to search and analyze code." 
    },
    { 
      role: "user", 
      content: userMessage 
    }
  ]);
  
  return result.content;
}

// Example usage
const response = await chatWithAI("How is authentication implemented in this project?");
console.log(response);
```

### Building a Code Explanation Chain

```javascript
import { search } from '@buger/probe';
import { ChatOpenAI } from '@langchain/openai';
import { PromptTemplate } from '@langchain/core/prompts';
import { StringOutputParser } from '@langchain/core/output_parsers';

// Create a chain for code explanation
async function createCodeExplainer() {
  // Create a chat model
  const model = new ChatOpenAI({
    modelName: "gpt-4o",
    temperature: 0.5
  });
  
  // Create a prompt template
  const promptTemplate = PromptTemplate.fromTemplate(`
    You are a code explanation expert. I'll provide you with a question and some code search results.
    Please analyze the code and explain it in a clear, concise manner.
    
    Question: {question}
    
    Code search results:
    {searchResults}
    
    Your explanation:
  `);
  
  // Create a chain
  const chain = promptTemplate
    .pipe(model)
    .pipe(new StringOutputParser());
  
  // Function to explain code
  async function explainCode(question, codebasePath) {
    // Search for relevant code
    const searchResults = await search({
      path: codebasePath,
      query: question,
      maxResults: 5,
      maxTokens: 8000
    });
    
    // Get the explanation from the AI
    const explanation = await chain.invoke({
      question,
      searchResults
    });
    
    return explanation;
  }
  
  return { explainCode };
}

// Usage
const explainer = await createCodeExplainer();
const explanation = await explainer.explainCode(
  "How does the authentication system work?",
  "/path/to/your/project"
);
console.log(explanation);
```

## Examples with Vercel AI SDK

The Vercel AI SDK is another popular framework for building AI applications. The latest version of Probe provides tool generators that allow for better configuration and session isolation:

```javascript
import { generateText } from 'ai';
import { searchTool, queryTool, extractTool } from '@buger/probe';
import { randomUUID } from 'crypto';

// Generate a session ID for tool isolation
const sessionId = randomUUID();
console.log(`Generated session ID: ${sessionId}`);

// Configure tools with options
const configOptions = {
  sessionId,
  debug: process.env.DEBUG === 'true',
  maxTokens: 30000 // Optional: override default max tokens
};

// Create configured tool instances
const configuredTools = {
  search: searchTool(configOptions),
  query: queryTool(configOptions),
  extract: extractTool(configOptions)
};

// Use the configured tools with Vercel AI SDK
async function chatWithAI(userMessage) {
  const result = await generateText({
    model: provider(modelName),
    messages: [{ role: 'user', content: userMessage }],
    system: "You are a code intelligence assistant. Use the provided tools to search and analyze code.",
    tools: configuredTools,
    maxSteps: 15,
    temperature: 0.7
  });
  
  return result.text;
}

// Example usage
const response = await chatWithAI("Find all API endpoints in this project");
console.log(response);
```

### Benefits of Tool Generators

The new tool generator approach provides several advantages:

1. **Session Isolation**: Each tool instance can have its own session ID, which is crucial in concurrent environments like web applications
2. **Configurable Options**: You can customize tools with options like debug logging and token limits
3. **Better Debugging**: Enable debug mode to see detailed logs of tool execution
4. **Backward Compatibility**: The package still exports pre-configured tools for backward compatibility

### Backward Compatibility

If you prefer the previous approach, you can still use the pre-configured tools:

```javascript
import { tools } from '@buger/probe';

// Use the pre-configured tools
const result = await generateText({
  model: provider(modelName),
  messages: [{ role: 'user', content: userMessage }],
  system: "You are a code intelligence assistant.",
  tools: {
    search: tools.searchTool,
    query: tools.queryTool,
    extract: tools.extractTool
  }
});
```

## Building a Custom Code Search API

Create a REST API for code search:

```javascript
import express from 'express';
import { search, query, extract } from '@buger/probe';

const app = express();
app.use(express.json());

// Search endpoint
app.post('/api/search', async (req, res) => {
  try {
    const { path, query, options } = req.body;
    const results = await search({
      path,
      query,
      ...options
    });
    res.json({ results });
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Query endpoint
app.post('/api/query', async (req, res) => {
  try {
    const { path, pattern, language, options } = req.body;
    const results = await query({
      path,
      pattern,
      language,
      ...options
    });
    res.json({ results });
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Extract endpoint
app.post('/api/extract', async (req, res) => {
  try {
    const { files, options } = req.body;
    const results = await extract({
      files,
      ...options
    });
    res.json({ results });
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

app.listen(3000, () => {
  console.log('Code search API running on port 3000');
});
```

## Handling Concurrency and Caching

For production applications, you'll want to handle concurrency and implement caching:

### Implementing a Cache Layer

```javascript
import { search } from '@buger/probe';
import NodeCache from 'node-cache';

// Create a cache with 1 hour TTL
const cache = new NodeCache({ stdTTL: 3600 });

// Cached search function
async function cachedSearch(params) {
  // Create a cache key from the parameters
  const cacheKey = JSON.stringify(params);
  
  // Check if we have a cached result
  const cachedResult = cache.get(cacheKey);
  if (cachedResult) {
    console.log('Cache hit!');
    return cachedResult;
  }
  
  // Perform the search
  console.log('Cache miss, performing search...');
  const results = await search(params);
  
  // Cache the results
  cache.set(cacheKey, results);
  
  return results;
}

// Example usage
const results = await cachedSearch({
  path: '/path/to/your/project',
  query: 'authentication',
  maxResults: 10
});
```

### Managing Concurrency

```javascript
import { search } from '@buger/probe';
import pLimit from 'p-limit';

// Limit concurrency to 5 simultaneous searches
const limit = pLimit(5);

// Function to search multiple repositories
async function searchMultipleRepos(query, repositories) {
  // Map each repository to a limited promise
  const promises = repositories.map(repo => 
    limit(() => search({
      path: repo,
      query,
      maxResults: 10
    }))
  );
  
  // Wait for all searches to complete
  const results = await Promise.all(promises);
  
  // Combine and return results
  return repositories.reduce((acc, repo, index) => {
    acc[repo] = results[index];
    return acc;
  }, {});
}

// Example usage
const results = await searchMultipleRepos(
  'authentication',
  [
    '/path/to/repo1',
    '/path/to/repo2',
    '/path/to/repo3',
    '/path/to/repo4',
    '/path/to/repo5'
  ]
);
```

## Best Practices for Production

### Error Handling

```javascript
import { search } from '@buger/probe';

async function robustSearch(params) {
  try {
    return await search(params);
  } catch (error) {
    console.error('Search error:', error);
    
    // Handle specific errors
    if (error.message.includes('Binary not found')) {
      // Handle missing binary
      console.error('Probe binary not found. Trying to reinstall...');
      // Implement reinstall logic
    } else if (error.message.includes('Permission denied')) {
      // Handle permission issues
      console.error('Permission denied. Check file permissions.');
    } else if (error.message.includes('timeout')) {
      // Handle timeouts
      console.error('Search timed out. Try with a more specific query.');
    }
    
    // Return a fallback result
    return { error: error.message, fallback: true };
  }
}
```

### Performance Optimization

```javascript
import { search } from '@buger/probe';

// Optimize search for performance
async function optimizedSearch(query, path) {
  // First, do a quick search for files only
  const fileResults = await search({
    path,
    query,
    filesOnly: true,
    maxResults: 50
  });
  
  // If we have too many files, refine the query
  if (fileResults.files && fileResults.files.length > 20) {
    console.log('Too many files, refining query...');
    // Add more specific terms to narrow down results
    return search({
      path,
      query: `${query} AND (function OR class OR method)`,
      maxResults: 10
    });
  }
  
  // Otherwise, search the content of the files we found
  return search({
    path,
    query,
    maxResults: 10
  });
}
```

### Memory Management

```javascript
import { search } from '@buger/probe';

// Search with memory constraints
async function memoryEfficientSearch(query, path) {
  // Split the search into smaller batches
  const directories = [
    `${path}/src`,
    `${path}/lib`,
    `${path}/packages`
  ];
  
  const results = [];
  
  // Process each directory separately
  for (const dir of directories) {
    const dirResults = await search({
      path: dir,
      query,
      maxResults: 5
    });
    
    results.push(dirResults);
    
    // Optional: Force garbage collection if available
    if (global.gc) {
      global.gc();
    }
  }
  
  return results;
}

// Run with --expose-gc flag to enable manual garbage collection
// node --expose-gc your-script.js
```

## Next Steps

- For individual developer workflows, see [Integrating Probe into AI Code Editors](/use-cases/integrating-probe-into-ai-code-editors)
- For team collaboration, check out [Deploying the Probe Web Interface](/use-cases/deploying-probe-web-interface)
- For advanced CLI usage, explore [Advanced CLI: Speed, Token Limits, and Large Repos](/use-cases/advanced-cli)
- For complete API reference, see the [Node.js SDK API Reference](/nodejs-sdk)