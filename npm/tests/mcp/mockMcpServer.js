#!/usr/bin/env node

/**
 * Mock MCP Server for Testing ProbeAgent MCP Integration
 *
 * This server implements a complete MCP protocol server with multiple tools
 * for comprehensive testing of the MCP integration functionality.
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';

// Tool schemas for validation
const schemas = {
  foobar: z.object({
    action: z.enum(['get', 'set', 'list']).default('get'),
    key: z.string().optional(),
    value: z.string().optional()
  }),

  calculator: z.object({
    operation: z.enum(['add', 'subtract', 'multiply', 'divide']),
    a: z.number(),
    b: z.number()
  }),

  echo: z.object({
    message: z.string()
  }),

  filesystem: z.object({
    action: z.enum(['read', 'write', 'list']),
    path: z.string(),
    content: z.string().optional()
  }),

  weather: z.object({
    location: z.string(),
    units: z.enum(['celsius', 'fahrenheit']).default('celsius')
  }),

  error_test: z.object({
    error_type: z.enum(['validation', 'runtime', 'timeout']).default('runtime'),
    message: z.string().optional()
  }),

  slow_operation: z.object({
    delay_ms: z.number().min(0).max(10000).default(1000),
    result: z.string().default('completed')
  })
};

// In-memory data store for testing
const dataStore = new Map();
dataStore.set('test_key', 'test_value');
dataStore.set('count', '42');

/**
 * Mock MCP Server implementation
 */
class MockMCPServer {
  constructor() {
    this.server = new Server(
      {
        name: 'mock-mcp-server',
        version: '1.0.0'
      },
      {
        capabilities: {
          tools: {}
        }
      }
    );

    this.setupHandlers();
  }

  setupHandlers() {
    // List tools handler
    this.server.setRequestHandler('tools/list', async () => {
      return {
        tools: [
          {
            name: 'foobar',
            description: 'A simple key-value store tool for testing basic MCP functionality',
            inputSchema: {
              type: 'object',
              properties: {
                action: {
                  type: 'string',
                  enum: ['get', 'set', 'list'],
                  description: 'Action to perform: get a value, set a value, or list all keys',
                  default: 'get'
                },
                key: {
                  type: 'string',
                  description: 'Key for get/set operations'
                },
                value: {
                  type: 'string',
                  description: 'Value for set operation'
                }
              },
              required: []
            }
          },
          {
            name: 'calculator',
            description: 'Performs basic mathematical operations',
            inputSchema: {
              type: 'object',
              properties: {
                operation: {
                  type: 'string',
                  enum: ['add', 'subtract', 'multiply', 'divide'],
                  description: 'Mathematical operation to perform'
                },
                a: {
                  type: 'number',
                  description: 'First number'
                },
                b: {
                  type: 'number',
                  description: 'Second number'
                }
              },
              required: ['operation', 'a', 'b']
            }
          },
          {
            name: 'echo',
            description: 'Echoes back the provided message',
            inputSchema: {
              type: 'object',
              properties: {
                message: {
                  type: 'string',
                  description: 'Message to echo back'
                }
              },
              required: ['message']
            }
          },
          {
            name: 'filesystem',
            description: 'Mock filesystem operations for testing',
            inputSchema: {
              type: 'object',
              properties: {
                action: {
                  type: 'string',
                  enum: ['read', 'write', 'list'],
                  description: 'Filesystem action to perform'
                },
                path: {
                  type: 'string',
                  description: 'File or directory path'
                },
                content: {
                  type: 'string',
                  description: 'Content for write operations'
                }
              },
              required: ['action', 'path']
            }
          },
          {
            name: 'weather',
            description: 'Mock weather service for testing external API simulation',
            inputSchema: {
              type: 'object',
              properties: {
                location: {
                  type: 'string',
                  description: 'Location to get weather for'
                },
                units: {
                  type: 'string',
                  enum: ['celsius', 'fahrenheit'],
                  description: 'Temperature units',
                  default: 'celsius'
                }
              },
              required: ['location']
            }
          },
          {
            name: 'error_test',
            description: 'Tool that generates various types of errors for testing error handling',
            inputSchema: {
              type: 'object',
              properties: {
                error_type: {
                  type: 'string',
                  enum: ['validation', 'runtime', 'timeout'],
                  description: 'Type of error to generate',
                  default: 'runtime'
                },
                message: {
                  type: 'string',
                  description: 'Custom error message'
                }
              },
              required: []
            }
          },
          {
            name: 'slow_operation',
            description: 'Tool that simulates slow operations for testing timeouts',
            inputSchema: {
              type: 'object',
              properties: {
                delay_ms: {
                  type: 'number',
                  minimum: 0,
                  maximum: 10000,
                  description: 'Delay in milliseconds (max 10 seconds)',
                  default: 1000
                },
                result: {
                  type: 'string',
                  description: 'Result message to return after delay',
                  default: 'completed'
                }
              },
              required: []
            }
          }
        ]
      };
    });

    // Tool call handler
    this.server.setRequestHandler('tools/call', async (request) => {
      const { name, arguments: args } = request.params;

      switch (name) {
        case 'foobar':
          return this.handleFoobar(args);
        case 'calculator':
          return this.handleCalculator(args);
        case 'echo':
          return this.handleEcho(args);
        case 'filesystem':
          return this.handleFilesystem(args);
        case 'weather':
          return this.handleWeather(args);
        case 'error_test':
          return this.handleErrorTest(args);
        case 'slow_operation':
          return this.handleSlowOperation(args);
        default:
          throw new Error(`Unknown tool: ${name}`);
      }
    });
  }

  async handleFoobar(args) {
    const validated = schemas.foobar.parse(args);
    const { action, key, value } = validated;

    switch (action) {
      case 'get':
        if (!key) {
          throw new Error('Key is required for get operation');
        }
        const storedValue = dataStore.get(key);
        return {
          content: [
            {
              type: 'text',
              text: storedValue !== undefined
                ? `Value for key "${key}": ${storedValue}`
                : `Key "${key}" not found`
            }
          ]
        };

      case 'set':
        if (!key || value === undefined) {
          throw new Error('Both key and value are required for set operation');
        }
        dataStore.set(key, value);
        return {
          content: [
            {
              type: 'text',
              text: `Successfully set "${key}" = "${value}"`
            }
          ]
        };

      case 'list':
        const keys = Array.from(dataStore.keys());
        return {
          content: [
            {
              type: 'text',
              text: keys.length > 0
                ? `Stored keys: ${keys.join(', ')}`
                : 'No keys stored'
            }
          ]
        };

      default:
        throw new Error(`Unknown action: ${action}`);
    }
  }

  async handleCalculator(args) {
    const validated = schemas.calculator.parse(args);
    const { operation, a, b } = validated;

    let result;
    switch (operation) {
      case 'add':
        result = a + b;
        break;
      case 'subtract':
        result = a - b;
        break;
      case 'multiply':
        result = a * b;
        break;
      case 'divide':
        if (b === 0) {
          throw new Error('Division by zero is not allowed');
        }
        result = a / b;
        break;
      default:
        throw new Error(`Unknown operation: ${operation}`);
    }

    return {
      content: [
        {
          type: 'text',
          text: `${a} ${operation} ${b} = ${result}`
        }
      ]
    };
  }

  async handleEcho(args) {
    const validated = schemas.echo.parse(args);
    const { message } = validated;

    return {
      content: [
        {
          type: 'text',
          text: `Echo: ${message}`
        }
      ]
    };
  }

  async handleFilesystem(args) {
    const validated = schemas.filesystem.parse(args);
    const { action, path, content } = validated;

    // Mock filesystem operations
    const mockFiles = {
      '/test.txt': 'This is test content',
      '/config.json': '{"setting": "value"}',
      '/empty.txt': ''
    };

    switch (action) {
      case 'read':
        if (mockFiles[path] !== undefined) {
          return {
            content: [
              {
                type: 'text',
                text: `File content of ${path}:\n${mockFiles[path]}`
              }
            ]
          };
        } else {
          throw new Error(`File not found: ${path}`);
        }

      case 'write':
        if (content === undefined) {
          throw new Error('Content is required for write operation');
        }
        mockFiles[path] = content;
        return {
          content: [
            {
              type: 'text',
              text: `Successfully wrote ${content.length} characters to ${path}`
            }
          ]
        };

      case 'list':
        const files = Object.keys(mockFiles);
        return {
          content: [
            {
              type: 'text',
              text: `Files in mock filesystem:\n${files.join('\n')}`
            }
          ]
        };

      default:
        throw new Error(`Unknown filesystem action: ${action}`);
    }
  }

  async handleWeather(args) {
    const validated = schemas.weather.parse(args);
    const { location, units } = validated;

    // Mock weather data
    const mockWeatherData = {
      'new york': { celsius: 15, fahrenheit: 59, condition: 'Cloudy' },
      'london': { celsius: 12, fahrenheit: 54, condition: 'Rainy' },
      'tokyo': { celsius: 22, fahrenheit: 72, condition: 'Sunny' },
      'default': { celsius: 20, fahrenheit: 68, condition: 'Clear' }
    };

    const locationKey = location.toLowerCase();
    const weather = mockWeatherData[locationKey] || mockWeatherData.default;
    const temp = weather[units];

    return {
      content: [
        {
          type: 'text',
          text: `Weather in ${location}: ${temp}°${units === 'celsius' ? 'C' : 'F'}, ${weather.condition}`
        }
      ]
    };
  }

  async handleErrorTest(args) {
    const validated = schemas.error_test.parse(args);
    const { error_type, message } = validated;

    switch (error_type) {
      case 'validation':
        throw new Error(message || 'This is a validation error for testing');
      case 'runtime':
        throw new Error(message || 'This is a runtime error for testing');
      case 'timeout':
        // Simulate a very long operation
        await new Promise(resolve => setTimeout(resolve, 30000));
        return {
          content: [
            {
              type: 'text',
              text: 'This should never be reached due to timeout'
            }
          ]
        };
      default:
        throw new Error(`Unknown error type: ${error_type}`);
    }
  }

  async handleSlowOperation(args) {
    const validated = schemas.slow_operation.parse(args);
    const { delay_ms, result } = validated;

    // Wait for the specified delay
    await new Promise(resolve => setTimeout(resolve, delay_ms));

    return {
      content: [
        {
          type: 'text',
          text: `Slow operation ${result} after ${delay_ms}ms delay`
        }
      ]
    };
  }

  async start() {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);
    console.error('Mock MCP Server started and listening on stdio');
  }
}

// Start the server if run directly
if (import.meta.url === `file://${process.argv[1]}`) {
  const server = new MockMCPServer();

  // Handle graceful shutdown
  process.on('SIGINT', async () => {
    console.error('Shutting down mock MCP server...');
    await server.server.close();
    process.exit(0);
  });

  server.start().catch(error => {
    console.error('Failed to start mock MCP server:', error);
    process.exit(1);
  });
}

export { MockMCPServer };