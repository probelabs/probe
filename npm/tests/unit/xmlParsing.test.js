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
        const xmlString = '<extract><file_path>src/test.js</file_path><line>10</line><end_line>20</end_line></extract>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toEqual({
          toolName: 'extract',
          params: { 
            file_path: 'src/test.js',
            line: 10,
            end_line: 20
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
        // Test tools that use structured parameters (excluding attempt_completion which uses free-form content)
        const structuredParamTools = ['search', 'query', 'extract', 'listFiles', 'searchFiles', 'implement'];
        
        structuredParamTools.forEach(toolName => {
          const xmlString = `<${toolName}><param>value</param></${toolName}>`;
          const result = parseXmlToolCall(xmlString);
          
          expect(result).toEqual({
            toolName,
            params: { param: 'value' }
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
        
        // The parser will find a valid <search></search> pair but with empty params
        // since the inner content doesn't have proper parameter structure
        expect(result).toEqual({
          toolName: 'search',
          params: {}
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
        
        // This finds the complete <query>test</query> tag and parses it
        // since 'query' is a valid tool name
        expect(result).toEqual({
          toolName: 'query',
          params: {}
        });
      });

      test('should handle whitespace and formatting', () => {
        const xmlString = `
          <search>
            <query>  test query  </query>
            <recursive>   true   </recursive>
          </search>
        `;
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toEqual({
          toolName: 'search',
          params: { 
            query: 'test query',
            recursive: true
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
          <file_path>test.js</file_path>
        </extract>
      `;
      
      const result = parseXmlToolCallWithThinking(xmlString);
      
      expect(result).toEqual({
        toolName: 'extract',
        params: { file_path: 'test.js' }
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
});