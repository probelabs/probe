import { parseXmlToolCall, parseXmlToolCallWithThinking } from '../../src/agent/tools.js';

describe('XML Tool Call Parsing', () => {
  describe('parseXmlToolCall', () => {
    describe('Valid tool parsing', () => {
      test('should parse valid search tool call', () => {
        const xmlString = '<search><query>test query</query></search>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toEqual({
          toolName: 'search',
          params: { query: 'test query' }
        });
      });

      test('should parse extract tool call with multiple params', () => {
        const xmlString = '<extract><targets>src/test.js:10-20 other.js#func</targets><input_content>some diff</input_content></extract>';
        const result = parseXmlToolCall(xmlString);

        expect(result).toEqual({
          toolName: 'extract',
          params: {
            targets: 'src/test.js:10-20 other.js#func',
            input_content: 'some diff'
          }
        });
      });

      test('should parse attempt_completion with direct content', () => {
        const xmlString = '<attempt_completion>Task completed successfully</attempt_completion>';
        const result = parseXmlToolCall(xmlString);

        expect(result).toEqual({
          toolName: 'attempt_completion',
          params: {
            result: 'Task completed successfully'
          }
        });
      });

      test('should parse boolean parameters correctly', () => {
        const xmlString = '<listFiles><recursive>true</recursive><includeHidden>false</includeHidden></listFiles>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toEqual({
          toolName: 'listFiles',
          params: {
            recursive: true,
            includeHidden: false
          }
        });
      });

      test('should handle all valid tools with structured parameters', () => {
        // Test tools with valid parameters from their schemas
        const testCases = [
          { tool: 'search', xml: '<search><query>test</query></search>', expected: { query: 'test' } },
          { tool: 'query', xml: '<query><pattern>$NAME</pattern></query>', expected: { pattern: '$NAME' } },
          { tool: 'extract', xml: '<extract><targets>file.js</targets></extract>', expected: { targets: 'file.js' } },
          { tool: 'listFiles', xml: '<listFiles><directory>src</directory></listFiles>', expected: { directory: 'src' } },
          { tool: 'searchFiles', xml: '<searchFiles><pattern>*.js</pattern></searchFiles>', expected: { pattern: '*.js' } },
          { tool: 'implement', xml: '<implement><task>test</task></implement>', expected: { task: 'test' } }
        ];

        testCases.forEach(({ tool, xml, expected }) => {
          const result = parseXmlToolCall(xml);

          expect(result).toEqual({
            toolName: tool,
            params: expected
          });
        });
      });
    });

    describe('Non-tool XML tag filtering', () => {
      test('should ignore HTML formatting tags', () => {
        const xmlString = '<ins>This is inserted text</ins>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toBeNull();
      });

      test('should ignore HTML emphasis tags', () => {
        const xmlString = '<em>emphasized text</em>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toBeNull();
      });

      test('should ignore HTML deletion tags', () => {
        const xmlString = '<del>deleted text</del>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toBeNull();
      });

      test('should ignore HTML strong tags', () => {
        const xmlString = '<strong>bold text</strong>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toBeNull();
      });

      test('should ignore custom XML tags', () => {
        const xmlString = '<customTag>custom content</customTag>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toBeNull();
      });

      test('should ignore markdown-style tags', () => {
        const xmlString = '<code>sample code</code>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toBeNull();
      });

      test('should ignore multiple non-tool tags in sequence', () => {
        const xmlString = '<ins>inserted</ins><del>deleted</del><em>emphasized</em>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toBeNull();
      });
    });

    describe('Custom valid tools list', () => {
      test('should respect custom valid tools list', () => {
        const customValidTools = ['search', 'extract'];
        
        // Should parse tools in the custom list
        const validXml = '<search><query>test</query></search>';
        const validResult = parseXmlToolCall(validXml, customValidTools);
        expect(validResult).toEqual({
          toolName: 'search',
          params: { query: 'test' }
        });
        
        // Should ignore tools not in the custom list
        const invalidXml = '<query><pattern>test</pattern></query>';
        const invalidResult = parseXmlToolCall(invalidXml, customValidTools);
        expect(invalidResult).toBeNull();
      });

      test('should handle empty valid tools list', () => {
        const emptyValidTools = [];
        const xmlString = '<search><query>test</query></search>';
        const result = parseXmlToolCall(xmlString, emptyValidTools);
        
        expect(result).toBeNull();
      });

      test('should work with single tool in valid list', () => {
        const singleTool = ['attempt_completion'];
        const xmlString = '<attempt_completion>done</attempt_completion>';
        const result = parseXmlToolCall(xmlString, singleTool);

        expect(result).toEqual({
          toolName: 'attempt_completion',
          params: { result: 'done' }
        });
      });
    });

    describe('Edge cases', () => {
      test('should handle malformed XML gracefully', () => {
        const xmlString = '<search><query>unclosed tag</search>';
        const result = parseXmlToolCall(xmlString);

        // With improved parser, it now handles unclosed parameter tags
        // The parser finds <search></search> and extracts the unclosed <query> param
        expect(result).toEqual({
          toolName: 'search',
          params: { query: 'unclosed tag' }
        });
      });

      test('should handle empty XML string', () => {
        const xmlString = '';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toBeNull();
      });

      test('should handle XML with no closing tag', () => {
        const xmlString = '<search><query>test</query>';
        const result = parseXmlToolCall(xmlString);

        // With improved parser, it handles unclosed tool tags
        // Finds <search> (unclosed) with properly closed <query> parameter
        expect(result).toEqual({
          toolName: 'search',
          params: { query: 'test' }
        });
      });

      test('should handle whitespace and formatting', () => {
        const xmlString = `
          <search>
            <query>  test query  </query>
            <path>  ./src  </path>
          </search>
        `;
        const result = parseXmlToolCall(xmlString);

        expect(result).toEqual({
          toolName: 'search',
          params: {
            query: 'test query',
            path: './src'
          }
        });
      });
    });
  });

  describe('parseXmlToolCallWithThinking', () => {
    test('should parse tool call and ignore thinking tags', () => {
      const xmlString = `
        <thinking>
        I need to search for the user's query about testing.
        </thinking>
        <search>
          <query>testing framework</query>
        </search>
      `;
      
      const result = parseXmlToolCallWithThinking(xmlString);
      
      expect(result).toEqual({
        toolName: 'search',
        params: { query: 'testing framework' }
      });
    });

    test('should handle multiple thinking blocks', () => {
      const xmlString = `
        <thinking>First thought</thinking>
        <thinking>Second thought</thinking>
        <extract>
          <targets>test.js</targets>
        </extract>
      `;

      const result = parseXmlToolCallWithThinking(xmlString);

      expect(result).toEqual({
        toolName: 'extract',
        params: { targets: 'test.js' }
      });
    });

    test('should ignore non-tool tags even with thinking', () => {
      const xmlString = `
        <thinking>I should format this text</thinking>
        <ins>This is inserted text</ins>
        <thinking>But I need to use a tool instead</thinking>
      `;
      
      const result = parseXmlToolCallWithThinking(xmlString);
      
      expect(result).toBeNull();
    });

    test('should pass custom valid tools to underlying parser', () => {
      const customValidTools = ['search'];
      const xmlString = `
        <thinking>I'll use query tool</thinking>
        <query>
          <pattern>test pattern</pattern>
        </query>
      `;
      
      // Should be null because query is not in custom valid tools
      const result = parseXmlToolCallWithThinking(xmlString, customValidTools);
      
      expect(result).toBeNull();
    });

    test('should handle thinking without tool calls', () => {
      const xmlString = `
        <thinking>
        Just thinking about the problem, no tool call needed yet.
        </thinking>
      `;
      
      const result = parseXmlToolCallWithThinking(xmlString);
      
      expect(result).toBeNull();
    });
  });

  describe('Real-world scenarios', () => {
    test('should handle AI response with formatting and tool call', () => {
      const aiResponse = `
        I need to search for information about this topic.
        
        <ins>Let me search for the relevant code:</ins>
        
        <search>
          <query>authentication middleware</query>
        </search>
        
        <em>This search should help us find the authentication logic.</em>
      `;
      
      const result = parseXmlToolCallWithThinking(aiResponse);
      
      expect(result).toEqual({
        toolName: 'search',
        params: { query: 'authentication middleware' }
      });
    });

    test('should handle response with only HTML formatting (no tools)', () => {
      const aiResponse = `
        Here's the analysis:
        
        <strong>Key Points:</strong>
        - <ins>The code needs refactoring</ins>
        - <del>Old approach is deprecated</del>
        - <em>New pattern is more efficient</em>
        
        <code>function example() { return true; }</code>
      `;
      
      const result = parseXmlToolCallWithThinking(aiResponse);
      
      expect(result).toBeNull();
    });

    test('should handle mixed content with thinking and HTML tags', () => {
      const aiResponse = `
        <thinking>
        The user wants me to extract code from a specific file.
        I should use the extract tool for this.
        </thinking>
        
        I understand you need the code. <ins>Let me extract it for you:</ins>
        
        <extract>
          <file_path>src/components/Header.js</file_path>
          <line>1</line>
          <end_line>50</end_line>
        </extract>
        
        <em>This will show you the first 50 lines of the Header component.</em>
      `;
      
      const result = parseXmlToolCallWithThinking(aiResponse);
      
      expect(result).toEqual({
        toolName: 'extract',
        params: { 
          file_path: 'src/components/Header.js',
          line: 1,
          end_line: 50
        }
      });
    });

    test('should handle implement tool when allowed', () => {
      const aiResponse = `
        <thinking>
        I need to implement this feature using the implement tool.
        </thinking>
        
        <implement>
          <description>Add user authentication</description>
        </implement>
      `;
      
      const validToolsWithImplement = ['search', 'query', 'extract', 'implement', 'attempt_completion'];
      const result = parseXmlToolCallWithThinking(aiResponse, validToolsWithImplement);
      
      expect(result).toEqual({
        toolName: 'implement',
        params: { description: 'Add user authentication' }
      });
    });

    test('should ignore implement tool when not allowed', () => {
      const aiResponse = `
        <implement>
          <description>Add user authentication</description>
        </implement>
      `;

      const validToolsWithoutImplement = ['search', 'query', 'extract', 'attempt_completion'];
      const result = parseXmlToolCallWithThinking(aiResponse, validToolsWithoutImplement);

      expect(result).toBeNull();
    });
  });

  describe('Unclosed attempt_completion tag handling', () => {
    test('should handle attempt_completion with content but no closing tag', () => {
      const aiResponse = `<attempt_completion>
\`\`\`json
{
  "issues": [
    {
      "file": "test.ts",
      "line": 442,
      "message": "Security issue"
    }
  ]
}
\`\`\``;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toEqual({
        toolName: 'attempt_completion',
        params: {
          result: `\`\`\`json
{
  "issues": [
    {
      "file": "test.ts",
      "line": 442,
      "message": "Security issue"
    }
  ]
}
\`\`\``
        }
      });
    });

    test('should handle attempt_completion with text content and no closing tag', () => {
      const aiResponse = `Some explanation text before the tag.

<attempt_completion>
The task has been completed successfully.
All tests are passing.`;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toEqual({
        toolName: 'attempt_completion',
        params: {
          result: `The task has been completed successfully.
All tests are passing.`
        }
      });
    });

    test('should handle attempt_completion with closing tag (normal case)', () => {
      const aiResponse = `<attempt_completion>
Task completed with all requirements met.
</attempt_completion>`;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toEqual({
        toolName: 'attempt_completion',
        params: {
          result: 'Task completed with all requirements met.'
        }
      });
    });

    test('should handle empty attempt_completion tag without closing', () => {
      const aiResponse = `<attempt_completion>`;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toEqual({
        toolName: 'attempt_completion',
        params: {
          result: '__PREVIOUS_RESPONSE__'
        }
      });
    });

    test('should prioritize attempt_completion over other content', () => {
      const aiResponse = `Here's some explanation text.

<ins>Important:</ins> The analysis is complete.

<attempt_completion>
\`\`\`json
{"status": "complete"}
\`\`\``;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toEqual({
        toolName: 'attempt_completion',
        params: {
          result: `\`\`\`json
{"status": "complete"}
\`\`\``
        }
      });
    });
  });

  describe('Unclosed thinking tag handling', () => {
    test('should remove unclosed thinking tag and its content', () => {
      const aiResponse = `<thinking>
I need to search for the authentication code.
This is my reasoning...

<search>
<query>authentication</query>
</search>`;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toEqual({
        toolName: 'search',
        params: { query: 'authentication' }
      });
    });

    test('should handle properly closed thinking tag', () => {
      const aiResponse = `<thinking>
Let me analyze this.
</thinking>

<extract>
<file_path>src/auth.js</file_path>
</extract>`;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toEqual({
        toolName: 'extract',
        params: { file_path: 'src/auth.js' }
      });
    });

    test('should handle multiple thinking tags with one unclosed', () => {
      const aiResponse = `<thinking>First thought</thinking>
<thinking>Second thought that never ends...

<query>
<pattern>test</pattern>
</query>`;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toEqual({
        toolName: 'query',
        params: { pattern: 'test' }
      });
    });
  });

  describe('Unclosed tool tag handling', () => {
    test('should handle search tool without closing tag', () => {
      const aiResponse = `<search>
<query>function definition`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toEqual({
        toolName: 'search',
        params: { query: 'function definition' }
      });
    });

    test('should handle extract tool without closing tag', () => {
      const aiResponse = `<extract>
<file_path>src/index.js</file_path>
<line>42`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toEqual({
        toolName: 'extract',
        params: {
          file_path: 'src/index.js',
          line: 42
        }
      });
    });

    test('should handle query tool without closing tag', () => {
      const aiResponse = `<query>
<pattern>class \\w+`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toEqual({
        toolName: 'query',
        params: { pattern: 'class \\w+' }
      });
    });

    test('should handle listFiles tool without closing tag', () => {
      const aiResponse = `<listFiles>
<path>src/</path>
<recursive>true`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toEqual({
        toolName: 'listFiles',
        params: {
          path: 'src/',
          recursive: true
        }
      });
    });
  });

  describe('Unclosed parameter tag handling', () => {
    test('should handle parameter without closing tag followed by another param', () => {
      const aiResponse = `<search>
<query>authentication
<max_results>10</max_results>
</search>`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toEqual({
        toolName: 'search',
        params: {
          query: 'authentication',
          max_results: 10
        }
      });
    });

    test('should handle last parameter without closing tag', () => {
      const aiResponse = `<extract>
<file_path>src/test.js</file_path>
<line>10
</extract>`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toEqual({
        toolName: 'extract',
        params: {
          file_path: 'src/test.js',
          line: 10
        }
      });
    });

    test('should handle multiple unclosed parameter tags', () => {
      const aiResponse = `<extract>
<file_path>src/app.js
<line>1
<end_line>100
</extract>`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toEqual({
        toolName: 'extract',
        params: {
          file_path: 'src/app.js',
          line: 1,
          end_line: 100
        }
      });
    });

    test('should handle unclosed param with multiline content', () => {
      const aiResponse = `<search>
<query>function test() {
  return true;
}
<max_results>5</max_results>
</search>`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toEqual({
        toolName: 'search',
        params: {
          query: `function test() {
  return true;
}`,
          max_results: 5
        }
      });
    });
  });

  describe('Complex unclosed tag scenarios', () => {
    test('should handle tool and param both unclosed', () => {
      const aiResponse = `<search>
<query>test query`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toEqual({
        toolName: 'search',
        params: { query: 'test query' }
      });
    });

    test('should handle unclosed thinking + unclosed tool', () => {
      const aiResponse = `<thinking>
I should search for this...

<search>
<query>authentication middleware`;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toEqual({
        toolName: 'search',
        params: { query: 'authentication middleware' }
      });
    });

    test('should handle mixed properly closed and unclosed tags', () => {
      const aiResponse = `<extract>
<file_path>src/auth.js</file_path>
<line>10
<end_line>20</end_line>`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toEqual({
        toolName: 'extract',
        params: {
          file_path: 'src/auth.js',
          line: 10,
          end_line: 20
        }
      });
    });
  });
});