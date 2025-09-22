/**
 * Unit tests for MCPXmlBridge and XML parsing functionality
 */

import { jest } from '@jest/globals';
import {
  MCPXmlBridge,
  mcpToolToXmlDefinition,
  parseXmlMcpToolCall,
  parseHybridXmlToolCall,
  createHybridSystemMessage
} from '../../src/agent/mcp/xmlBridge.js';

describe('MCPXmlBridge', () => {
  let bridge;

  beforeEach(() => {
    bridge = new MCPXmlBridge({ debug: false });
  });

  afterEach(async () => {
    if (bridge) {
      await bridge.cleanup();
    }
  });

  describe('Tool Definition Conversion', () => {
    test('should convert simple MCP tool to XML definition', () => {
      const tool = {
        description: 'A test tool',
        inputSchema: {
          type: 'object',
          properties: {
            message: {
              type: 'string',
              description: 'Message to process'
            }
          },
          required: ['message']
        }
      };

      const xmlDef = mcpToolToXmlDefinition('test_tool', tool);

      expect(xmlDef).toContain('## test_tool');
      expect(xmlDef).toContain('Description: A test tool');
      expect(xmlDef).toContain('- message: string (required) - Message to process');
      expect(xmlDef).toContain('<test_tool>');
      expect(xmlDef).toContain('<params>');
      expect(xmlDef).toContain('</test_tool>');
    });

    test('should handle tool with enum parameters', () => {
      const tool = {
        description: 'Tool with enum',
        inputSchema: {
          type: 'object',
          properties: {
            action: {
              type: 'string',
              enum: ['get', 'set', 'delete'],
              description: 'Action to perform'
            },
            value: {
              type: 'string',
              description: 'Optional value'
            }
          },
          required: ['action']
        }
      };

      const xmlDef = mcpToolToXmlDefinition('enum_tool', tool);

      expect(xmlDef).toContain('- action: string (required) - Action to perform [choices: get, set, delete]');
      expect(xmlDef).toContain('- value: string (optional) - Optional value');
    });

    test('should handle tool with no parameters', () => {
      const tool = {
        description: 'Simple tool',
        inputSchema: {}
      };

      const xmlDef = mcpToolToXmlDefinition('simple_tool', tool);

      expect(xmlDef).toContain('## simple_tool');
      expect(xmlDef).toContain('Description: Simple tool');
      expect(xmlDef).toContain('<simple_tool>');
    });

    test('should handle tool with missing schema', () => {
      const tool = {
        description: 'Tool without schema'
      };

      const xmlDef = mcpToolToXmlDefinition('no_schema_tool', tool);

      expect(xmlDef).toContain('## no_schema_tool');
      expect(xmlDef).toContain('Description: Tool without schema');
    });
  });

  describe('XML Parsing', () => {
    test('should parse simple MCP tool call with JSON params', () => {
      const xmlString = `
        <foobar>
        <params>
        {
          "action": "get",
          "key": "test_key"
        }
        </params>
        </foobar>
      `;

      const result = parseXmlMcpToolCall(xmlString, ['foobar']);

      expect(result).toBeDefined();
      expect(result.toolName).toBe('foobar');
      expect(result.params).toEqual({
        action: 'get',
        key: 'test_key'
      });
    });

    test('should parse simple string parameter', () => {
      const xmlString = `
        <echo>
        <params>Hello World</params>
        </echo>
      `;

      const result = parseXmlMcpToolCall(xmlString, ['echo']);

      expect(result).toBeDefined();
      expect(result.toolName).toBe('echo');
      expect(result.params).toEqual({
        value: 'Hello World'
      });
    });

    test('should handle legacy XML parameter format', () => {
      const xmlString = `
        <legacy_tool>
        <param1>value1</param1>
        <param2>value2</param2>
        </legacy_tool>
      `;

      const result = parseXmlMcpToolCall(xmlString, ['legacy_tool']);

      expect(result).toBeDefined();
      expect(result.toolName).toBe('legacy_tool');
      expect(result.params).toEqual({
        param1: 'value1',
        param2: 'value2'
      });
    });

    test('should handle invalid JSON gracefully', () => {
      const xmlString = `
        <bad_json>
        <params>{ invalid json }</params>
        </bad_json>
      `;

      const result = parseXmlMcpToolCall(xmlString, ['bad_json']);

      expect(result).toBeDefined();
      expect(result.toolName).toBe('bad_json');
      expect(result.params).toEqual({
        value: '{ invalid json }'
      });
    });

    test('should return null for unknown tool', () => {
      const xmlString = `
        <unknown_tool>
        <params>{"test": "value"}</params>
        </unknown_tool>
      `;

      const result = parseXmlMcpToolCall(xmlString, ['known_tool']);

      expect(result).toBeNull();
    });

    test('should ignore thinking tags', () => {
      const xmlString = `
        <thinking>
        This is my analysis of the problem...
        </thinking>
        <foobar>
        <params>{"action": "list"}</params>
        </foobar>
      `;

      const result = parseXmlMcpToolCall(xmlString, ['foobar']);

      expect(result).toBeDefined();
      expect(result.toolName).toBe('foobar');
      expect(result.params).toEqual({
        action: 'list'
      });
    });
  });

  describe('Hybrid XML Parsing', () => {
    test('should parse native tools with standard XML format', () => {
      const xmlString = `
        <search>
        <query>error handling</query>
        <path>./src</path>
        </search>
      `;

      const result = parseHybridXmlToolCall(xmlString, ['search']);

      expect(result).toBeDefined();
      expect(result.type).toBe('native');
      expect(result.toolName).toBe('search');
      expect(result.params).toEqual({
        query: 'error handling',
        path: './src'
      });
    });

    test('should parse MCP tools with JSON params', () => {
      const mockBridge = {
        getToolNames: () => ['foobar']
      };

      const xmlString = `
        <foobar>
        <params>{"action": "get", "key": "test"}</params>
        </foobar>
      `;

      const result = parseHybridXmlToolCall(xmlString, ['search'], mockBridge);

      expect(result).toBeDefined();
      expect(result.type).toBe('mcp');
      expect(result.toolName).toBe('foobar');
      expect(result.params).toEqual({
        action: 'get',
        key: 'test'
      });
    });

    test('should prioritize native tools over MCP tools', () => {
      const mockBridge = {
        getToolNames: () => ['search'] // Same name as native tool
      };

      const xmlString = `
        <search>
        <query>test query</query>
        </search>
      `;

      const result = parseHybridXmlToolCall(xmlString, ['search'], mockBridge);

      expect(result).toBeDefined();
      expect(result.type).toBe('native');
      expect(result.toolName).toBe('search');
    });

    test('should return null when no tools match', () => {
      const mockBridge = {
        getToolNames: () => ['foobar']
      };

      const xmlString = `
        <unknown_tool>
        <param>value</param>
        </unknown_tool>
      `;

      const result = parseHybridXmlToolCall(xmlString, ['search'], mockBridge);

      expect(result).toBeNull();
    });
  });

  describe('System Message Creation', () => {
    test('should create hybrid system message with both native and MCP tools', () => {
      const baseMessage = 'You are a helpful assistant.';
      const nativeToolDefs = `
        ## search
        Search for code patterns
      `;

      const mockBridge = {
        getToolNames: () => ['foobar', 'calculator'],
        getXmlToolDefinitions: () => `
        ## foobar
        Key-value store tool

        ## calculator
        Mathematical operations
        `
      };

      const hybridMessage = createHybridSystemMessage(baseMessage, nativeToolDefs, mockBridge);

      expect(hybridMessage).toContain('You are a helpful assistant.');
      expect(hybridMessage).toContain('=== NATIVE TOOLS ===');
      expect(hybridMessage).toContain('## search');
      expect(hybridMessage).toContain('=== MCP TOOLS ===');
      expect(hybridMessage).toContain('## foobar');
      expect(hybridMessage).toContain('## calculator');
      expect(hybridMessage).toContain('=== TOOL USAGE INSTRUCTIONS ===');
      expect(hybridMessage).toContain('For NATIVE tools, use standard XML format:');
      expect(hybridMessage).toContain('For MCP tools, use JSON within params tag:');
    });

    test('should handle message with only native tools', () => {
      const baseMessage = 'You are a helpful assistant.';
      const nativeToolDefs = '## search\nSearch for code patterns';

      const hybridMessage = createHybridSystemMessage(baseMessage, nativeToolDefs, null);

      expect(hybridMessage).toContain('=== NATIVE TOOLS ===');
      expect(hybridMessage).not.toContain('=== MCP TOOLS ===');
      expect(hybridMessage).toContain('=== TOOL USAGE INSTRUCTIONS ===');
    });

    test('should handle message with only MCP tools', () => {
      const baseMessage = 'You are a helpful assistant.';

      const mockBridge = {
        getToolNames: () => ['foobar'],
        getXmlToolDefinitions: () => '## foobar\nKey-value store tool'
      };

      const hybridMessage = createHybridSystemMessage(baseMessage, null, mockBridge);

      expect(hybridMessage).not.toContain('=== NATIVE TOOLS ===');
      expect(hybridMessage).toContain('=== MCP TOOLS ===');
      expect(hybridMessage).toContain('=== TOOL USAGE INSTRUCTIONS ===');
    });

    test('should handle empty MCP bridge', () => {
      const baseMessage = 'You are a helpful assistant.';
      const nativeToolDefs = '## search\nSearch for code patterns';

      const mockBridge = {
        getToolNames: () => [],
        getXmlToolDefinitions: () => ''
      };

      const hybridMessage = createHybridSystemMessage(baseMessage, nativeToolDefs, mockBridge);

      expect(hybridMessage).toContain('=== NATIVE TOOLS ===');
      expect(hybridMessage).not.toContain('=== MCP TOOLS ===');
    });
  });

  describe('Bridge Initialization', () => {
    test('should initialize with empty configuration', async () => {
      await bridge.initialize({
        mcpServers: {}
      });

      expect(bridge.getToolNames()).toEqual([]);
      expect(bridge.getXmlToolDefinitions()).toBe('');
    });

    test('should handle null configuration', async () => {
      await bridge.initialize(null);

      expect(bridge.getToolNames()).toEqual([]);
      expect(bridge.getXmlToolDefinitions()).toBe('');
    });

    test('should indicate if tool is MCP tool', () => {
      expect(bridge.isMcpTool('nonexistent')).toBe(false);
    });
  });

  describe('Tool Execution', () => {
    test('should throw error when executing non-existent tool', async () => {
      const xmlString = `
        <nonexistent>
        <params>{"test": "value"}</params>
        </nonexistent>
      `;

      await expect(bridge.executeFromXml(xmlString))
        .rejects.toThrow('No valid MCP tool call found in XML');
    });

    test('should handle execution errors gracefully', async () => {
      // Mock a tool that exists but will fail
      bridge.mcpTools = {
        'error_tool': {
          execute: async () => {
            throw new Error('Tool execution failed');
          }
        }
      };

      const xmlString = `
        <error_tool>
        <params>{"test": "value"}</params>
        </error_tool>
      `;

      const result = await bridge.executeFromXml(xmlString);

      expect(result.success).toBe(false);
      expect(result.toolName).toBe('error_tool');
      expect(result.error).toBe('Tool execution failed');
    });
  });

  describe('Cleanup', () => {
    test('should cleanup without errors when no manager exists', async () => {
      await expect(bridge.cleanup()).resolves.not.toThrow();
    });

    test('should cleanup manager if it exists', async () => {
      // Mock manager
      const mockManager = {
        disconnect: jest.fn().mockResolvedValue(undefined)
      };

      bridge.mcpManager = mockManager;

      await bridge.cleanup();

      expect(mockManager.disconnect).toHaveBeenCalled();
    });
  });
});