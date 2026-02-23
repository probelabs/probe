#!/usr/bin/env node

/**
 * Test MCP integration with real AI model
 * This tests the full flow: MCP server -> Tools -> AI -> Response
 */

import 'dotenv/config';
import { createAnthropic } from '@ai-sdk/anthropic';
import { createOpenAI } from '@ai-sdk/openai';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { generateText, tool } from 'ai';
import { z } from 'zod';
import { MCPClientManager } from '@probelabs/probe/agent/mcp';
import { writeFileSync } from 'fs';

// Check for API keys
const hasAnthropic = !!process.env.ANTHROPIC_API_KEY;
const hasOpenAI = !!process.env.OPENAI_API_KEY;
const hasGoogle = !!process.env.GOOGLE_API_KEY || !!process.env.GOOGLE_AI_API_KEY;

if (!hasAnthropic && !hasOpenAI && !hasGoogle) {
  console.error('❌ No API keys found. Please set ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY');
  process.exit(1);
}

async function testMCPWithAI() {
  console.log('=== Testing MCP Integration with AI ===\n');

  // Step 1: Create MCP configuration for Probe
  const mcpConfig = {
    mcpServers: {
      'probe': {
        command: 'npx',
        args: ['-y', '@probelabs/probe@latest', 'mcp'],
        transport: 'stdio',
        enabled: true
      }
    }
  };

  console.log('📡 Connecting to Probe MCP server...');
  const mcpManager = new MCPClientManager({ debug: true });

  try {
    const initResult = await mcpManager.initialize(mcpConfig);
    console.log(`✅ Connected to ${initResult.connected} server(s)`);
    console.log(`📦 Available tools: ${initResult.tools.join(', ')}\n`);
  } catch (error) {
    console.error('❌ Failed to connect to MCP server:', error);
    process.exit(1);
  }

  // Step 2: Get tools from MCP server
  const mcpTools = mcpManager.getVercelTools();
  console.log('🛠️  MCP Tools loaded:', Object.keys(mcpTools).length);

  // Step 3: Create wrapped tools for AI SDK v5
  const aiTools = {};
  for (const [name, mcpTool] of Object.entries(mcpTools)) {
    aiTools[name] = tool({
      description: mcpTool.description,
      inputSchema: z.object(
        Object.entries(mcpTool.inputSchema?.properties || {}).reduce((acc, [key, schema]) => {
          const isRequired = mcpTool.inputSchema?.required?.includes(key);

          // Convert JSON schema to Zod schema (simplified)
          let zodSchema;
          if (schema.type === 'string') {
            zodSchema = z.string();
          } else if (schema.type === 'number') {
            zodSchema = z.number();
          } else if (schema.type === 'boolean') {
            zodSchema = z.boolean();
          } else if (schema.type === 'array') {
            // Properly define array items based on the schema
            if (schema.items?.type === 'string') {
              zodSchema = z.array(z.string());
            } else if (schema.items?.type === 'number') {
              zodSchema = z.array(z.number());
            } else {
              zodSchema = z.array(z.string()); // Default to string array
            }
          } else {
            zodSchema = z.any();
          }

          acc[key] = isRequired ? zodSchema : zodSchema.optional();
          return acc;
        }, {})
      ),
      execute: mcpTool.execute
    });
  }

  console.log('✅ Tools wrapped for AI SDK v5\n');

  // Step 4: Choose AI model
  let model;
  if (hasGoogle) {
    const google = createGoogleGenerativeAI({
      apiKey: process.env.GOOGLE_API_KEY || process.env.GOOGLE_AI_API_KEY
    });
    model = google('gemini-2.5-flash');
    console.log('🤖 Using Google Gemini 2.0 Flash');
  } else if (hasAnthropic) {
    const anthropic = createAnthropic();
    model = anthropic('claude-3-5-haiku-20241022');
    console.log('🤖 Using Claude 3.5 Haiku');
  } else {
    const openai = createOpenAI();
    model = openai('gpt-5.2-mini');
    console.log('🤖 Using GPT-4o Mini');
  }

  // Step 5: Test queries with the AI
  const testQueries = [
    {
      name: 'Search Test',
      query: 'Search for functions that handle MCP or Model Context Protocol in this project',
      expectedTool: 'probe_search_code'
    },
    {
      name: 'Query Test',
      query: 'Find all JavaScript arrow functions that take options as a parameter',
      expectedTool: 'probe_query_code'
    },
    {
      name: 'Extract Test',
      query: 'Extract the MCPClientManager class from mcpClientV2.js',
      expectedTool: 'probe_extract_code'
    }
  ];

  console.log('\n=== Running AI Tests with MCP Tools ===\n');

  for (const test of testQueries) {
    console.log(`📝 Test: ${test.name}`);
    console.log(`   Query: "${test.query}"`);

    try {
      const startTime = Date.now();

      // Call AI with MCP tools
      const result = await generateText({
        model,
        messages: [
          {
            role: 'system',
            content: `You are a code analysis assistant with access to MCP tools for searching and analyzing code.
              Available tools:
              - probe_search_code: Search for code using keywords
              - probe_query_code: Query code using AST patterns
              - probe_extract_code: Extract specific code blocks

              Always use the appropriate tool to answer questions about code.
              Set the path parameter to "${process.cwd()}" for searches.`
          },
          {
            role: 'user',
            content: test.query
          }
        ],
        tools: aiTools,
        maxSteps: 3,
        temperature: 0.3
      });

      const duration = Date.now() - startTime;

      // Check if the expected tool was used
      const toolCalls = result.steps?.filter(step => step.toolCalls?.length > 0) || [];
      const usedTools = toolCalls.flatMap(step =>
        step.toolCalls.map(call => call.toolName)
      );

      console.log(`   ✅ Response received in ${duration}ms`);
      console.log(`   🔧 Tools used: ${usedTools.join(', ') || 'none'}`);

      if (test.expectedTool && !usedTools.includes(test.expectedTool)) {
        console.log(`   ⚠️  Expected tool ${test.expectedTool} was not used`);
      }

      // Show a preview of the response
      const preview = result.text.substring(0, 200);
      console.log(`   📄 Response preview: ${preview}${result.text.length > 200 ? '...' : ''}`);

      // Log token usage
      if (result.usage) {
        console.log(`   💰 Tokens: ${result.usage.promptTokens} prompt, ${result.usage.completionTokens} completion`);
      }

      console.log('');
    } catch (error) {
      console.error(`   ❌ Test failed: ${error.message}\n`);
    }
  }

  // Step 6: Interactive test with complex query
  console.log('=== Complex Query Test ===\n');

  const complexQuery = `Using the MCP tools available, help me understand how the MCP integration works in this codebase.
    Specifically:
    1. Search for MCP-related code
    2. Find the main MCP client implementation
    3. Extract key functions that handle MCP connections`;

  console.log('📝 Complex query:', complexQuery.replace(/\n\s+/g, ' '));

  try {
    const result = await generateText({
      model,
      messages: [
        {
          role: 'system',
          content: `You are a code analysis expert. Use the MCP tools to thoroughly analyze code.
            The codebase is located at: ${process.cwd()}
            Be systematic and use multiple tools to gather comprehensive information.`
        },
        {
          role: 'user',
          content: complexQuery
        }
      ],
      tools: aiTools,
      maxSteps: 10,
      temperature: 0.3
    });

    console.log('\n📊 Analysis Results:');
    console.log('─'.repeat(50));
    console.log(result.text);
    console.log('─'.repeat(50));

    // Save detailed results
    const reportPath = '/tmp/mcp-ai-test-report.md';
    const report = `# MCP Integration Test Report

## Test Date
${new Date().toISOString()}

## Configuration
- Model: ${model.modelId}
- MCP Servers: ${Object.keys(mcpConfig.mcpServers).join(', ')}
- Available Tools: ${Object.keys(aiTools).join(', ')}

## Query
${complexQuery}

## Response
${result.text}

## Tool Usage
${result.steps?.map((step, i) =>
  step.toolCalls?.map(call =>
    `### Step ${i + 1}: ${call.toolName}\n\`\`\`json\n${JSON.stringify(call.args, null, 2)}\n\`\`\``
  ).join('\n')
).join('\n') || 'No tools used'}

## Token Usage
- Prompt Tokens: ${result.usage?.promptTokens || 'N/A'}
- Completion Tokens: ${result.usage?.completionTokens || 'N/A'}
- Total Tokens: ${result.usage?.totalTokens || 'N/A'}
`;

    writeFileSync(reportPath, report);
    console.log(`\n📄 Detailed report saved to: ${reportPath}`);

  } catch (error) {
    console.error('❌ Complex query failed:', error.message);
  }

  // Cleanup
  await mcpManager.disconnect();
  console.log('\n✅ MCP connections closed');
}

// Run the test
testMCPWithAI().catch(console.error);