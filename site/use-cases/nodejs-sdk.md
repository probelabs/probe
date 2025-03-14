# Building AI Tools with Node.js SDK

This guide explains how to use Probe's Node.js SDK to build custom AI-powered code intelligence tools for your development workflow.

## Overview

Probe's Node.js SDK provides programmatic access to its powerful code search capabilities, allowing you to build custom tools, integrate with AI frameworks, and create specialized workflows for your development team.

Key benefits:

- **Programmatic Access**: Use Probe's capabilities directly from your Node.js code
- **AI Integration**: Ready-to-use tools for Vercel AI SDK, LangChain, and other AI frameworks
- **Custom Workflows**: Build specialized tools for your specific development needs
- **Automation**: Create automated code analysis and documentation pipelines
- **Extensibility**: Extend existing tools with code-aware intelligence

## Common Use Cases

### 1. Building AI-Powered Code Assistants

Create custom AI assistants that understand your codebase:

```javascript
import { search } from '@buger/probe';
import { ChatOpenAI } from '@langchain/openai';
import { PromptTemplate } from '@langchain/core/prompts';
import { StringOutputParser } from '@langchain/core/output_parsers';

async function createCodeAssistant() {
  // Create a chat model
  const model = new ChatOpenAI({
    modelName: "gpt-4o",
    temperature: 0.7
  });
  
  // Create a prompt template
  const promptTemplate = PromptTemplate.fromTemplate(`
    You are a code assistant. I'll provide you with a question and some code search results.
    Please analyze the code and answer the question.
    
    Question: {question}
    
    Code search results:
    {searchResults}
    
    Your analysis:
  `);
  
  // Create a chain
  const chain = promptTemplate
    .pipe(model)
    .pipe(new StringOutputParser());
  
  // Function to answer questions about code
  async function answerCodeQuestion(question, codebasePath) {
    // Search for relevant code
    const searchResults = await search({
      path: codebasePath,
      query: question,
      maxResults: 5,
      maxTokens: 10000
    });
    
    // Get the answer from the AI
    const answer = await chain.invoke({
      question,
      searchResults
    });
    
    return answer;
  }
  
  return { answerCodeQuestion };
}

// Usage
const assistant = await createCodeAssistant();
const answer = await assistant.answerCodeQuestion(
  "How is authentication implemented?",
  "/path/to/your/project"
);
console.log(answer);
```

### 2. Creating Code Search APIs

Build a REST API for code search:

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

### 3. Automated Code Analysis

Create automated code analysis pipelines:

```javascript
import { search, query } from '@buger/probe';
import fs from 'fs/promises';

async function analyzeCodebase(codebasePath) {
  const analysis = {
    timestamp: new Date().toISOString(),
    codebasePath,
    metrics: {},
    patterns: {},
    potentialIssues: []
  };
  
  // Count functions by language
  const languages = ['javascript', 'typescript', 'python', 'rust', 'go'];
  const functionCounts = {};
  
  for (const lang of languages) {
    try {
      const pattern = lang === 'javascript' || lang === 'typescript'
        ? 'function $NAME($$$PARAMS) $$$BODY'
        : lang === 'python'
          ? 'def $NAME($$$PARAMS): $$$BODY'
          : lang === 'rust'
            ? 'fn $NAME($$$PARAMS) $$$BODY'
            : 'func $NAME($$$PARAMS) $$$BODY';
      
      const results = await query({
        path: codebasePath,
        pattern,
        language: lang,
        maxResults: 1000,
        json: true
      });
      
      functionCounts[lang] = results.matches ? results.matches.length : 0;
    } catch (error) {
      console.error(`Error counting functions in ${lang}:`, error);
      functionCounts[lang] = -1; // Error indicator
    }
  }
  
  analysis.metrics.functionCounts = functionCounts;
  
  // Find potential security issues
  const securityPatterns = [
    'password',
    'token',
    'api_key',
    'apikey',
    'secret',
    'credential',
    'eval(',
    'exec(',
    'shell_exec'
  ];
  
  for (const pattern of securityPatterns) {
    try {
      const results = await search({
        path: codebasePath,
        query: pattern,
        maxResults: 50,
        json: true
      });
      
      if (results.matches && results.matches.length > 0) {
        analysis.potentialIssues.push({
          pattern,
          matches: results.matches.map(match => ({
            file: match.file,
            line: match.line,
            content: match.content.substring(0, 100) + '...' // Truncate long content
          }))
        });
      }
    } catch (error) {
      console.error(`Error searching for pattern ${pattern}:`, error);
    }
  }
  
  // Save analysis to file
  await fs.writeFile(
    'codebase-analysis.json',
    JSON.stringify(analysis, null, 2)
  );
  
  return analysis;
}

// Usage
const analysis = await analyzeCodebase('/path/to/your/project');
console.log('Analysis complete. Results saved to codebase-analysis.json');
console.log(`Found ${Object.values(analysis.metrics.functionCounts).reduce((a, b) => a + (b > 0 ? b : 0), 0)} functions across all languages`);
console.log(`Found ${analysis.potentialIssues.length} potential security issues`);
```

### 4. Documentation Generation

Automatically generate documentation for your codebase:

```javascript
import { query, extract } from '@buger/probe';
import fs from 'fs/promises';
import path from 'path';
import { ChatOpenAI } from '@langchain/openai';

async function generateDocumentation(codebasePath, outputDir) {
  // Create output directory if it doesn't exist
  await fs.mkdir(outputDir, { recursive: true });
  
  // Find all functions in the codebase
  const functions = await query({
    path: codebasePath,
    pattern: 'function $NAME($$$PARAMS) $$$BODY',
    language: 'javascript',
    maxResults: 100,
    json: true
  });
  
  // Create AI model for documentation generation
  const model = new ChatOpenAI({
    modelName: "gpt-4o",
    temperature: 0.2
  });
  
  // Generate documentation for each function
  for (const func of functions.matches || []) {
    try {
      // Extract the full function code
      const extracted = await extract({
        files: [`${func.file}:${func.line}`],
        contextLines: 5,
        json: true
      });
      
      // Generate documentation using AI
      const response = await model.invoke([
        {
          role: "system",
          content: "You are a technical documentation expert. Generate clear, concise documentation for the following function. Include: purpose, parameters, return value, and example usage."
        },
        {
          role: "user",
          content: `Generate documentation for this function:\n\n${extracted.content}`
        }
      ]);
      
      // Save documentation to file
      const funcName = func.name || `function_line_${func.line}`;
      const docPath = path.join(outputDir, `${funcName}.md`);
      await fs.writeFile(docPath, response.content);
      
      console.log(`Generated documentation for ${funcName}`);
    } catch (error) {
      console.error(`Error generating documentation for function at ${func.file}:${func.line}:`, error);
    }
  }
  
  console.log(`Documentation generation complete. Files saved to ${outputDir}`);
}

// Usage
await generateDocumentation('/path/to/your/project', './docs');
```

### 5. Code Review Automation

Create automated code review tools:

```javascript
import { search, extract } from '@buger/probe';
import { ChatOpenAI } from '@langchain/openai';
import fs from 'fs/promises';

async function reviewPullRequest(repoPath, changedFiles) {
  const model = new ChatOpenAI({
    modelName: "gpt-4o",
    temperature: 0.3
  });
  
  const reviews = [];
  
  for (const file of changedFiles) {
    try {
      // Extract the file content
      const fileContent = await extract({
        files: [path.join(repoPath, file)],
        json: true
      });
      
      // Search for potential issues
      const securityIssues = await search({
        path: path.join(repoPath, file),
        query: 'password OR token OR secret OR eval OR exec',
        maxResults: 10,
        json: true
      });
      
      // Generate review using AI
      const response = await model.invoke([
        {
          role: "system",
          content: "You are a code review expert. Review the provided code for issues related to security, performance, maintainability, and best practices. Be concise but thorough."
        },
        {
          role: "user",
          content: `Review this file: ${file}\n\nContent:\n${fileContent.content}\n\nPotential security issues found:\n${JSON.stringify(securityIssues.matches || [])}`
        }
      ]);
      
      reviews.push({
        file,
        review: response.content
      });
      
      console.log(`Reviewed ${file}`);
    } catch (error) {
      console.error(`Error reviewing ${file}:`, error);
      reviews.push({
        file,
        error: error.message
      });
    }
  }
  
  // Save reviews to file
  await fs.writeFile(
    'code-review.json',
    JSON.stringify(reviews, null, 2)
  );
  
  return reviews;
}

// Usage
const changedFiles = ['src/auth.js', 'src/api.js', 'src/utils.js'];
const reviews = await reviewPullRequest('/path/to/your/project', changedFiles);
console.log(`Reviewed ${reviews.length} files. Results saved to code-review.json`);
```

## Integration with AI Frameworks

### Vercel AI SDK Integration

```javascript
import { generateText } from 'ai';
import { tools } from '@buger/probe';

// Use the pre-built tools with Vercel AI SDK
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
}
```

### LangChain Integration

```javascript
import { ChatOpenAI } from '@langchain/openai';
import { tools } from '@buger/probe';

// Create the LangChain tools
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
    { role: "system", content: "You are a code intelligence assistant. Use the provided tools to search and analyze code." },
    { role: "user", content: userMessage }
  ]);
  
  return result.content;
}
```

## Advanced Techniques

### Batch Processing Multiple Repositories

```javascript
import { search } from '@buger/probe';
import fs from 'fs/promises';
import path from 'path';

async function batchSearch(repositories, searchQuery) {
  const results = {};
  
  for (const repo of repositories) {
    console.log(`Searching in ${repo}...`);
    try {
      const searchResults = await search({
        path: repo,
        query: searchQuery,
        maxResults: 20,
        json: true // Get structured results
      });
      
      results[repo] = searchResults;
    } catch (error) {
      console.error(`Error searching in ${repo}:`, error);
      results[repo] = { error: error.message };
    }
  }
  
  return results;
}

// Example usage
const repositories = [
  '/path/to/repo1',
  '/path/to/repo2',
  '/path/to/repo3'
];

const results = await batchSearch(repositories, 'security AND (vulnerability OR exploit)');

// Save results to a file
await fs.writeFile(
  path.join(process.cwd(), 'search-results.json'),
  JSON.stringify(results, null, 2)
);

console.log('Search completed and results saved to search-results.json');
```

### Session-Based Caching

```javascript
import { search } from '@buger/probe';

// First search with empty session string (generates a session ID)
const results1 = await search({
  path: '/path/to/your/project',
  query: 'authentication',
  session: ''
});

// Get the session ID from the results
const sessionId = results1.session;
console.log(`Session ID: ${sessionId}`);

// Use the same session ID for related searches
const results2 = await search({
  path: '/path/to/your/project',
  query: 'login',
  session: sessionId
});

// This will skip code blocks already shown in the previous search
console.log(`Found ${results2.matches.length} new matches`);
```

## Best Practices

1. **Use Specific Queries**: More specific queries yield better results and improve performance

2. **Limit Result Size**: Use `maxResults` and `maxTokens` to limit the size of results, especially when using with AI models

3. **Handle Errors**: Always wrap API calls in try/catch blocks to handle potential errors gracefully

4. **Cache Results**: Consider caching results for frequently used queries to improve performance

5. **Use JSON Format**: Use `json: true` for programmatic processing of results

6. **Combine with Other Tools**: Use Probe alongside other tools for a more comprehensive understanding of your codebase

7. **Optimize for Performance**: Use `filesOnly` for initial broad searches, then refine with more specific queries

8. **Use Session IDs**: For related searches, use the same session ID to avoid seeing duplicate code blocks

## Getting Started

### Installation

```bash
# Local installation
npm install @buger/probe

# Global installation
npm install -g @buger/probe
```

### Basic Usage

```javascript
import { search, query, extract } from '@buger/probe';

// Search for code
const searchResults = await search({
  path: '/path/to/your/project',
  query: 'function',
  maxResults: 10
});

// Query for specific code structures
const queryResults = await query({
  path: '/path/to/your/project',
  pattern: 'function $NAME($$$PARAMS) $$$BODY',
  language: 'javascript'
});

// Extract code blocks
const extractResults = await extract({
  files: ['/path/to/your/project/src/main.js:42']
});
```

## Next Steps

- Learn about [Probe's CLI Reference](/cli-mode) for advanced command-line usage
- Explore [using Probe with AI Code Editors](/use-cases/ai-code-editors) for integrated IDE experiences
- Check out [hosting a team chat](/use-cases/team-chat) for collaborative code exploration