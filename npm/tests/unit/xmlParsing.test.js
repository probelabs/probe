import { parseXmlToolCall, parseXmlToolCallWithRecovery } from '../../src/agent/tools.js';
import { DEFAULT_VALID_TOOLS, buildToolTagPattern, detectUnrecognizedToolCall, unescapeXmlEntities } from '../../src/tools/common.js';

describe('Shared Tool List', () => {
  test('DEFAULT_VALID_TOOLS should include bash', () => {
    expect(DEFAULT_VALID_TOOLS).toContain('bash');
  });

  test('DEFAULT_VALID_TOOLS should include all standard tools', () => {
    const expectedTools = [
      'search',
      'query',
      'extract',
      'delegate',
      'listFiles',
      'searchFiles',
      'bash',
      'attempt_completion'
    ];
    expectedTools.forEach(tool => {
      expect(DEFAULT_VALID_TOOLS).toContain(tool);
    });
  });

  test('buildToolTagPattern should create regex matching all tools', () => {
    const pattern = buildToolTagPattern(DEFAULT_VALID_TOOLS);

    // Test that pattern matches all standard tools
    expect('<search>').toMatch(pattern);
    expect('<bash>').toMatch(pattern);
    expect('<attempt_completion>').toMatch(pattern);
    expect('<attempt_complete>').toMatch(pattern); // alias

    // Test that pattern doesn't match non-tools
    expect('<div>').not.toMatch(pattern);
    expect('<strong>').not.toMatch(pattern);
  });

  test('buildToolTagPattern should work with custom tool list', () => {
    const customTools = ['customTool', 'anotherTool'];
    const pattern = buildToolTagPattern(customTools);

    expect('<customTool>').toMatch(pattern);
    expect('<anotherTool>').toMatch(pattern);
    expect('<search>').not.toMatch(pattern);
  });
});

describe('XML Tool Call Parsing', () => {
  describe('parseXmlToolCall', () => {
    describe('Valid tool parsing', () => {
      test('should parse valid search tool call', () => {
        const xmlString = '<search><query>test query</query></search>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toMatchObject({
          toolName: 'search',
          params: { query: 'test query' }
        });
      });

      test('should parse extract tool call with multiple params', () => {
        const xmlString = '<extract><targets>src/test.js:10-20 other.js#func</targets><input_content>some diff</input_content></extract>';
        const result = parseXmlToolCall(xmlString);

        expect(result).toMatchObject({
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

        expect(result).toMatchObject({
          toolName: 'attempt_completion',
          params: {
            result: 'Task completed successfully'
          }
        });
      });

      test('should parse boolean parameters correctly', () => {
        const xmlString = '<listFiles><recursive>true</recursive><includeHidden>false</includeHidden></listFiles>';
        const result = parseXmlToolCall(xmlString);
        
        expect(result).toMatchObject({
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
          { tool: 'searchFiles', xml: '<searchFiles><pattern>*.js</pattern></searchFiles>', expected: { pattern: '*.js' } }
        ];

        testCases.forEach(({ tool, xml, expected }) => {
          const result = parseXmlToolCall(xml);

          expect(result).toMatchObject({
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

        expect(result).toMatchObject({
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
        expect(result).toMatchObject({
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
        expect(result).toMatchObject({
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

        expect(result).toMatchObject({
          toolName: 'search',
          params: {
            query: 'test query',
            path: './src'
          }
        });
      });
    });
  });

  describe('parseXmlToolCallWithRecovery', () => {
    test('should parse standard tool calls', () => {
      const xmlString = `
        <search>
          <query>testing framework</query>
        </search>
      `;

      const result = parseXmlToolCallWithRecovery(xmlString);

      expect(result).toMatchObject({
        toolName: 'search',
        params: { query: 'testing framework' }
      });
    });

    test('should recover attempt_complete shorthand', () => {
      const xmlString = `<attempt_complete>`;

      const result = parseXmlToolCallWithRecovery(xmlString);

      expect(result).toMatchObject({
        toolName: 'attempt_completion',
        params: { result: '__PREVIOUS_RESPONSE__' }
      });
    });

    test('should pass custom valid tools to underlying parser', () => {
      const customValidTools = ['search'];
      const xmlString = `
        <query>
          <pattern>test pattern</pattern>
        </query>
      `;

      // Should be null because query is not in custom valid tools
      const result = parseXmlToolCallWithRecovery(xmlString, customValidTools);

      expect(result).toBeNull();
    });

    test('should return null when no valid tool call found', () => {
      const xmlString = `Just some text with no tool calls.`;

      const result = parseXmlToolCallWithRecovery(xmlString);

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
      
      const result = parseXmlToolCallWithRecovery(aiResponse);
      
      expect(result).toMatchObject({
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
      
      const result = parseXmlToolCallWithRecovery(aiResponse);
      
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
          <targets>src/components/Header.js:1-50</targets>
        </extract>

        <em>This will show you the first 50 lines of the Header component.</em>
      `;

      const result = parseXmlToolCallWithRecovery(aiResponse);

      expect(result).toMatchObject({
        toolName: 'extract',
        params: {
          targets: 'src/components/Header.js:1-50'
        }
      });
    });

    test('should handle edit tool when allowed', () => {
      const aiResponse = `
        <thinking>
        I need to edit this file using the edit tool.
        </thinking>

        <edit>
          <file_path>src/main.js</file_path>
          <old_string>const x = 1;</old_string>
          <new_string>const x = 2;</new_string>
        </edit>
      `;

      const validToolsWithEdit = ['search', 'query', 'extract', 'edit', 'attempt_completion'];
      const result = parseXmlToolCallWithRecovery(aiResponse, validToolsWithEdit);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: { file_path: 'src/main.js', old_string: 'const x = 1;', new_string: 'const x = 2;' }
      });
    });

    test('should ignore edit tool when not allowed', () => {
      const aiResponse = `
        <edit>
          <file_path>src/main.js</file_path>
          <old_string>const x = 1;</old_string>
          <new_string>const x = 2;</new_string>
        </edit>
      `;

      const validToolsWithoutEdit = ['search', 'query', 'extract', 'attempt_completion'];
      const result = parseXmlToolCallWithRecovery(aiResponse, validToolsWithoutEdit);

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

      const result = parseXmlToolCallWithRecovery(aiResponse);

      expect(result).toMatchObject({
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

      const result = parseXmlToolCallWithRecovery(aiResponse);

      expect(result).toMatchObject({
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

      const result = parseXmlToolCallWithRecovery(aiResponse);

      expect(result).toMatchObject({
        toolName: 'attempt_completion',
        params: {
          result: 'Task completed with all requirements met.'
        }
      });
    });

    test('should handle empty attempt_completion tag without closing', () => {
      const aiResponse = `<attempt_completion>`;

      const result = parseXmlToolCallWithRecovery(aiResponse);

      expect(result).toMatchObject({
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

      const result = parseXmlToolCallWithRecovery(aiResponse);

      expect(result).toMatchObject({
        toolName: 'attempt_completion',
        params: {
          result: `\`\`\`json
{"status": "complete"}
\`\`\``
        }
      });
    });
  });

  describe('Unclosed tool tag handling', () => {
    test('should handle search tool without closing tag', () => {
      const aiResponse = `<search>
<query>function definition`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toMatchObject({
        toolName: 'search',
        params: { query: 'function definition' }
      });
    });

    test('should handle extract tool without closing tag', () => {
      const aiResponse = `<extract>
<targets>src/index.js:42`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toMatchObject({
        toolName: 'extract',
        params: {
          targets: 'src/index.js:42'
        }
      });
    });

    test('should handle query tool without closing tag', () => {
      const aiResponse = `<query>
<pattern>class \\w+`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toMatchObject({
        toolName: 'query',
        params: { pattern: 'class \\w+' }
      });
    });

    test('should handle listFiles tool without closing tag', () => {
      const aiResponse = `<listFiles>
<path>src/</path>
<recursive>true`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toMatchObject({
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
<path>./src</path>
</search>`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toMatchObject({
        toolName: 'search',
        params: {
          query: 'authentication',
          path: './src'
        }
      });
    });

    test('should handle last parameter without closing tag', () => {
      const aiResponse = `<extract>
<targets>src/test.js:10
</extract>`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toMatchObject({
        toolName: 'extract',
        params: {
          targets: 'src/test.js:10'
        }
      });
    });

    test('should handle multiple unclosed parameter tags', () => {
      const aiResponse = `<extract>
<targets>src/app.js:1-100
<input_content>some diff content
</extract>`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toMatchObject({
        toolName: 'extract',
        params: {
          targets: 'src/app.js:1-100',
          input_content: 'some diff content'
        }
      });
    });

    test('should handle unclosed param with multiline content', () => {
      const aiResponse = `<search>
<query>function test() {
  return true;
}
<path>./src</path>
</search>`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toMatchObject({
        toolName: 'search',
        params: {
          query: `function test() {
  return true;
}`,
          path: './src'
        }
      });
    });
  });

  describe('Bash tool parsing', () => {
    test('should parse bash tool call with default valid tools', () => {
      const aiResponse = `<bash>
<command>git log --grep="#7054" --oneline</command>
<workingDirectory>/tmp/workspaces/tyk</workingDirectory>
</bash>`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toMatchObject({
        toolName: 'bash',
        params: {
          command: 'git log --grep="#7054" --oneline',
          workingDirectory: '/tmp/workspaces/tyk'
        }
      });
    });

    test('should handle bash tool with unclosed thinking tag', () => {
      // This is the exact scenario from the user's bug report
      const aiResponse = `<thinking>
The user wants to know in which version PR #7054 was merged.
My plan is as follows:
1. Find the commit hash for PR #7054.
The working directory should be the tyk project path.<bash>
<command>git log --grep="#7054" --oneline</command>
<workingDirectory>/tmp/workspaces/tyk</workingDirectory>
</bash>`;

      const result = parseXmlToolCallWithRecovery(aiResponse);

      expect(result).toMatchObject({
        toolName: 'bash',
        params: {
          command: 'git log --grep="#7054" --oneline',
          workingDirectory: '/tmp/workspaces/tyk'
        }
      });
    });

    test('should handle bash tool directly attached to text (no newline)', () => {
      const aiResponse = `<thinking>
The working directory should be the tyk project path.<bash>
<command>git status</command>
</bash>`;

      const result = parseXmlToolCallWithRecovery(aiResponse);

      expect(result).toMatchObject({
        toolName: 'bash',
        params: {
          command: 'git status'
        }
      });
    });
  });

  describe('Complex unclosed tag scenarios', () => {
    test('should handle tool and param both unclosed', () => {
      const aiResponse = `<search>
<query>test query`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toMatchObject({
        toolName: 'search',
        params: { query: 'test query' }
      });
    });

    test('should handle unclosed thinking + unclosed tool', () => {
      const aiResponse = `<thinking>
I should search for this...

<search>
<query>authentication middleware`;

      const result = parseXmlToolCallWithRecovery(aiResponse);

      expect(result).toMatchObject({
        toolName: 'search',
        params: { query: 'authentication middleware' }
      });
    });

    test('should handle mixed properly closed and unclosed tags', () => {
      const aiResponse = `<extract>
<targets>src/auth.js:10-20
<input_content>some content</input_content>`;

      const result = parseXmlToolCall(aiResponse);

      expect(result).toMatchObject({
        toolName: 'extract',
        params: {
          targets: 'src/auth.js:10-20',
          input_content: 'some content'
        }
      });
    });
  });

  describe('Edit and Create tool parsing (Issue #349)', () => {
    test('should parse edit tool with all parameters', () => {
      const validTools = ['search', 'edit', 'create', 'attempt_completion'];
      const aiResponse = `<edit>
<file_path>src/main.js</file_path>
<old_string>function oldName() {
  return 42;
}</old_string>
<new_string>function newName() {
  return 42;
}</new_string>
</edit>`;

      const result = parseXmlToolCallWithRecovery(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'src/main.js',
          old_string: `function oldName() {
  return 42;
}`,
          new_string: `function newName() {
  return 42;
}`
        }
      });
    });

    test('should parse edit tool with replace_all parameter', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>config.json</file_path>
<old_string>"debug": false</old_string>
<new_string>"debug": true</new_string>
<replace_all>true</replace_all>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'config.json',
          old_string: '"debug": false',
          new_string: '"debug": true',
          replace_all: true
        }
      });
    });

    test('should parse create tool with all parameters', () => {
      const validTools = ['create'];
      const aiResponse = `<create>
<file_path>src/newFile.js</file_path>
<content>export function hello() {
  return "Hello, world!";
}</content>
</create>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'create',
        params: {
          file_path: 'src/newFile.js',
          content: `export function hello() {
  return "Hello, world!";
}`
        }
      });
    });

    test('should parse create tool with overwrite parameter', () => {
      const validTools = ['create'];
      const aiResponse = `<create>
<file_path>README.md</file_path>
<content># My Project

This is a new project.</content>
<overwrite>true</overwrite>
</create>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'create',
        params: {
          file_path: 'README.md',
          content: `# My Project

This is a new project.`,
          overwrite: true
        }
      });
    });

    test('should handle edit tool with unclosed tags', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>test.js
<old_string>foo
<new_string>bar`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'test.js',
          old_string: 'foo',
          new_string: 'bar'
        }
      });
    });

    test('should handle create tool with unclosed tags', () => {
      const validTools = ['create'];
      const aiResponse = `<create>
<file_path>new.txt
<content>Hello World`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'create',
        params: {
          file_path: 'new.txt',
          content: 'Hello World'
        }
      });
    });

    test('should handle edit tool inside thinking tags', () => {
      const validTools = ['edit'];
      const aiResponse = `<thinking>
I need to rename this function to follow the naming convention.
</thinking>

<edit>
<file_path>src/utils.js</file_path>
<old_string>const getUserName = () => {}</old_string>
<new_string>const getUsername = () => {}</new_string>
</edit>`;

      const result = parseXmlToolCallWithRecovery(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'src/utils.js',
          old_string: 'const getUserName = () => {}',
          new_string: 'const getUsername = () => {}'
        }
      });
    });
  });

  describe('Edit tool symbol mode parsing', () => {
    test('should parse edit tool with symbol parameter (symbol replace mode)', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>src/utils.js</file_path>
<symbol>calculateTotal</symbol>
<new_string>function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price, 0);
}</new_string>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'src/utils.js',
          symbol: 'calculateTotal',
          new_string: `function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price, 0);
}`
        }
      });
    });

    test('should parse edit tool with symbol and position (symbol insert mode)', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>src/utils.js</file_path>
<symbol>calculateTotal</symbol>
<new_string>function calculateTax(total) {
  return total * 0.1;
}</new_string>
<position>after</position>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'src/utils.js',
          symbol: 'calculateTotal',
          new_string: `function calculateTax(total) {
  return total * 0.1;
}`,
          position: 'after'
        }
      });
    });

    test('should parse edit tool with symbol and position before', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>src/utils.js</file_path>
<symbol>calculateTotal</symbol>
<position>before</position>
<new_string>// Calculate the total price of items</new_string>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'src/utils.js',
          symbol: 'calculateTotal',
          position: 'before',
          new_string: '// Calculate the total price of items'
        }
      });
    });
  });

  describe('Edit tool line-targeted mode parsing', () => {
    test('should parse edit tool with start_line (line replace mode)', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<new_string>return processItems(order.items);</new_string>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'src/main.js',
          start_line: 42,  // XML parser coerces to number
          new_string: 'return processItems(order.items);'
        }
      });
    });

    test('should parse edit tool with start_line and end_line (range replace)', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<end_line>55</end_line>
<new_string>// simplified
return processItems(order.items);</new_string>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'src/main.js',
          start_line: 42,
          end_line: 55,
          new_string: '// simplified\nreturn processItems(order.items);'
        }
      });
    });

    test('should parse edit tool with start_line hash (preserves as string)', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>src/main.js</file_path>
<start_line>42:ab</start_line>
<new_string>return true;</new_string>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'src/main.js',
          start_line: '42:ab',  // Contains non-numeric chars, stays string
          new_string: 'return true;'
        }
      });
    });

    test('should parse edit tool with start_line and position (insert mode)', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<position>after</position>
<new_string>const validated = validate(input);</new_string>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'src/main.js',
          start_line: 42,
          position: 'after',
          new_string: 'const validated = validate(input);'
        }
      });
    });
  });

  describe('Wrapped tool call detection (Issue: api_call infinite loop)', () => {
    describe('detectUnrecognizedToolCall with wrapped tools', () => {
      test('should detect tool name wrapped in <api_call><tool_name> tags', () => {
        const validTools = ['search', 'attempt_completion'];
        const xmlString = `<api_call>
<tool_name>attempt_completion</tool_name>
<parameters>
<final_answer>{"intent": "code_help"}</final_answer>
</parameters>
</api_call>`;

        const result = detectUnrecognizedToolCall(xmlString, validTools);

        expect(result).toBe('wrapped_tool:attempt_completion');
      });

      test('should detect tool name wrapped in <function> tags', () => {
        const validTools = ['search', 'attempt_completion'];
        const xmlString = `<function>search</function>`;

        const result = detectUnrecognizedToolCall(xmlString, validTools);

        expect(result).toBe('wrapped_tool:search');
      });

      test('should detect tool name in <name> tags inside other structures', () => {
        const validTools = ['extract', 'query'];
        const xmlString = `<call>
<name>extract</name>
<args>file.js</args>
</call>`;

        const result = detectUnrecognizedToolCall(xmlString, validTools);

        expect(result).toBe('wrapped_tool:extract');
      });

      test('should return null for properly formatted tool call', () => {
        const validTools = ['search', 'attempt_completion'];
        const xmlString = `<attempt_completion>
Task completed successfully
</attempt_completion>`;

        const result = detectUnrecognizedToolCall(xmlString, validTools);

        expect(result).toBeNull();
      });

      test('should return null for response with no tool references', () => {
        const validTools = ['search', 'attempt_completion'];
        const xmlString = `Here is my analysis of the code.
The function works correctly.`;

        const result = detectUnrecognizedToolCall(xmlString, validTools);

        expect(result).toBeNull();
      });

      test('should detect wrapped tool even with thinking tags', () => {
        const validTools = ['search', 'attempt_completion'];
        const xmlString = `<thinking>
Let me complete this task
</thinking>

<api_call>
<tool_name>attempt_completion</tool_name>
<parameters>
<result>Done</result>
</parameters>
</api_call>`;

        const result = detectUnrecognizedToolCall(xmlString, validTools);

        expect(result).toBe('wrapped_tool:attempt_completion');
      });

      test('should return unrecognized tool name for unknown tool as tag', () => {
        const validTools = ['search', 'attempt_completion'];
        const xmlString = `<query><pattern>test</pattern></query>`;

        const result = detectUnrecognizedToolCall(xmlString, validTools);

        // query is a known tool name but not in validTools, so it should be detected
        expect(result).toBe('query');
      });

      test('should handle case insensitivity in wrapped tool detection', () => {
        const validTools = ['search', 'attempt_completion'];
        const xmlString = `<api_call>
<tool_name>ATTEMPT_COMPLETION</tool_name>
</api_call>`;

        const result = detectUnrecognizedToolCall(xmlString, validTools);

        expect(result).toBe('wrapped_tool:attempt_completion');
      });

      test('should detect wrapped tool with whitespace around name', () => {
        const validTools = ['search', 'attempt_completion'];
        const xmlString = `<function>  search  </function>`;

        const result = detectUnrecognizedToolCall(xmlString, validTools);

        expect(result).toBe('wrapped_tool:search');
      });

      test('should handle real-world api_call format from trace', () => {
        const validTools = ['search', 'query', 'extract', 'attempt_completion'];
        const xmlString = `<thinking>
The user is asking to investigate the GraphQL layer in tyk-analytics.
</thinking>

<api_call>
<tool_name>attempt_completion</tool_name>
<parameters>
<final_answer>
{
  "intent": "code_help",
  "topic": "Should the GraphQL layer be updated"
}
</final_answer>
</parameters>
</api_call>`;

        const result = detectUnrecognizedToolCall(xmlString, validTools);

        expect(result).toBe('wrapped_tool:attempt_completion');
      });
    });
  });
});


describe('XML entity unescaping', () => {
  describe('unescapeXmlEntities', () => {
    test('should unescape &amp; to &', () => {
      expect(unescapeXmlEntities('foo &amp; bar')).toBe('foo & bar');
    });

    test('should unescape &lt; to <', () => {
      expect(unescapeXmlEntities('a &lt; b')).toBe('a < b');
    });

    test('should unescape &gt; to >', () => {
      expect(unescapeXmlEntities('a &gt; b')).toBe('a > b');
    });

    test('should unescape &quot; to "', () => {
      expect(unescapeXmlEntities('say &quot;hello&quot;')).toBe('say "hello"');
    });

    test('should unescape &apos; to \'', () => {
      expect(unescapeXmlEntities("it&apos;s")).toBe("it's");
    });

    test('should handle multiple entities in one string', () => {
      expect(unescapeXmlEntities('a &amp; b &lt; c &gt; d')).toBe('a & b < c > d');
    });

    test('should not double-decode &amp;lt; (should become &lt;, not <)', () => {
      expect(unescapeXmlEntities('&amp;lt;')).toBe('&lt;');
    });

    test('should not double-decode &amp;amp; (should become &amp;, not &)', () => {
      expect(unescapeXmlEntities('&amp;amp;')).toBe('&amp;');
    });

    test('should return non-string values unchanged', () => {
      expect(unescapeXmlEntities(42)).toBe(42);
      expect(unescapeXmlEntities(true)).toBe(true);
      expect(unescapeXmlEntities(null)).toBe(null);
    });

    test('should handle string with no entities', () => {
      expect(unescapeXmlEntities('plain text')).toBe('plain text');
    });
  });

  describe('parseXmlToolCall with XML entities', () => {
    test('should unescape &amp;&amp; in bash command', () => {
      const xmlString = '<bash><command>cd tyk &amp;&amp; git status</command></bash>';
      const result = parseXmlToolCall(xmlString);

      expect(result).toMatchObject({
        toolName: 'bash',
        params: { command: 'cd tyk && git status' }
      });
    });

    test('should unescape &lt; and &gt; in bash command', () => {
      const xmlString = '<bash><command>echo &quot;hello&quot; &gt; output.txt</command></bash>';
      const result = parseXmlToolCall(xmlString);

      expect(result).toMatchObject({
        toolName: 'bash',
        params: { command: 'echo "hello" > output.txt' }
      });
    });

    test('should unescape entities in search query', () => {
      const xmlString = '<search><query>foo &amp; bar &lt;T&gt;</query></search>';
      const result = parseXmlToolCall(xmlString);

      expect(result).toMatchObject({
        toolName: 'search',
        params: { query: 'foo & bar <T>' }
      });
    });

    test('should unescape entities in attempt_completion content', () => {
      const xmlString = '<attempt_completion>Result: x &lt; y &amp;&amp; a &gt; b</attempt_completion>';
      const result = parseXmlToolCall(xmlString);

      expect(result).toMatchObject({
        toolName: 'attempt_completion',
        params: { result: 'Result: x < y && a > b' }
      });
    });

    test('should unescape entities in edit tool old_string/new_string', () => {
      const validTools = ['edit'];
      const xmlString = `<edit>
<file_path>test.js</file_path>
<old_string>if (a &amp;&amp; b &lt; c)</old_string>
<new_string>if (a &amp;&amp; b &lt;= c)</new_string>
</edit>`;

      const result = parseXmlToolCall(xmlString, validTools);

      expect(result).toMatchObject({
        toolName: 'edit',
        params: {
          file_path: 'test.js',
          old_string: 'if (a && b < c)',
          new_string: 'if (a && b <= c)'
        }
      });
    });

    test('should handle complex bash command with pipes and redirects', () => {
      const xmlString = '<bash><command>cat file.txt | grep &quot;pattern&quot; &amp;&amp; echo &quot;done&quot; &gt; /dev/null</command></bash>';
      const result = parseXmlToolCall(xmlString);

      expect(result).toMatchObject({
        toolName: 'bash',
        params: { command: 'cat file.txt | grep "pattern" && echo "done" > /dev/null' }
      });
    });
  });

  describe('Raw content parameter handling (create/edit tools)', () => {
    test('should preserve content containing </content> tag', () => {
      const validTools = ['create'];
      const aiResponse = `<create>
<file_path>doc.mdx</file_path>
<content>Use the content tag:
\`\`\`jsx
<content>Your text</content>
\`\`\`
More text after.</content>
</create>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result.toolName).toBe('create');
      expect(result.params.file_path).toBe('doc.mdx');
      expect(result.params.content).toContain('<content>Your text</content>');
      expect(result.params.content).toContain('More text after.');
    });

    test('should preserve content containing </create> tag', () => {
      const validTools = ['create'];
      const aiResponse = `<create>
<file_path>example.md</file_path>
<content>Example showing </create> tag in documentation.
This line should also be included.</content>
</create>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result.toolName).toBe('create');
      expect(result.params.file_path).toBe('example.md');
      expect(result.params.content).toContain('</create> tag in documentation');
      expect(result.params.content).toContain('This line should also be included.');
    });

    test('should preserve new_string containing </new_string> tag', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>test.js</file_path>
<old_string>old code</old_string>
<new_string>// See </new_string> in XML docs
more code here</new_string>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result.toolName).toBe('edit');
      expect(result.params.new_string).toContain('</new_string> in XML docs');
      expect(result.params.new_string).toContain('more code here');
    });

    test('should preserve new_string containing </edit> tag', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>test.js</file_path>
<old_string>placeholder</old_string>
<new_string>// Reference: </edit> tag usage
const x = 1;</new_string>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result.toolName).toBe('edit');
      expect(result.params.new_string).toContain('</edit> tag usage');
      expect(result.params.new_string).toContain('const x = 1;');
    });

    test('should keep content "true" as string (no type coercion)', () => {
      const validTools = ['create'];
      const aiResponse = `<create>
<file_path>flag.txt</file_path>
<content>true</content>
</create>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result.params.content).toBe('true');
      expect(typeof result.params.content).toBe('string');
    });

    test('should keep content "42" as string (no type coercion)', () => {
      const validTools = ['create'];
      const aiResponse = `<create>
<file_path>number.txt</file_path>
<content>42</content>
</create>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result.params.content).toBe('42');
      expect(typeof result.params.content).toBe('string');
    });

    test('should keep new_string "false" as string (no type coercion)', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>test.js</file_path>
<old_string>x</old_string>
<new_string>false</new_string>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result.params.new_string).toBe('false');
      expect(typeof result.params.new_string).toBe('string');
    });

    test('should still coerce overwrite to boolean', () => {
      const validTools = ['create'];
      const aiResponse = `<create>
<file_path>test.txt</file_path>
<content>hello</content>
<overwrite>true</overwrite>
</create>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result.params.overwrite).toBe(true);
      expect(typeof result.params.overwrite).toBe('boolean');
    });

    test('should still coerce replace_all to boolean', () => {
      const validTools = ['edit'];
      const aiResponse = `<edit>
<file_path>test.js</file_path>
<old_string>foo</old_string>
<new_string>bar</new_string>
<replace_all>true</replace_all>
</edit>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result.params.replace_all).toBe(true);
      expect(typeof result.params.replace_all).toBe('boolean');
    });

    test('should preserve leading whitespace in content', () => {
      const validTools = ['create'];
      const aiResponse = `<create>
<file_path>indented.py</file_path>
<content>  indented line
  another line</content>
</create>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      expect(result.params.content).toBe('  indented line\n  another line');
    });

    test('should strip only XML formatting newlines from content', () => {
      const validTools = ['create'];
      // Content has newline after <content> and before </content> (XML formatting)
      const aiResponse = `<create>
<file_path>test.txt</file_path>
<content>
first line
last line
</content>
</create>`;

      const result = parseXmlToolCall(aiResponse, validTools);

      // Leading/trailing XML newlines stripped, internal newlines preserved
      expect(result.params.content).toBe('first line\nlast line');
    });
  });
});

describe('Tool parsing with non-tool XML tags in surrounding text', () => {
  const VALID_TOOLS_WITH_EDIT = [...DEFAULT_VALID_TOOLS, 'edit', 'create'];

  test('parses <edit> tool surrounded by non-tool XML', () => {
    const input = `Some text with <thinking> tags before.
<edit>
<file_path>docs/configuration.mdx</file_path>
<old_string>## Sample config file</old_string>
<new_string>## New section\n\n## Sample config file</new_string>
</edit>`;
    const result = parseXmlToolCallWithRecovery(input, VALID_TOOLS_WITH_EDIT);
    expect(result).not.toBeNull();
    expect(result.toolName).toBe('edit');
    expect(result.params.file_path).toBe('docs/configuration.mdx');
    expect(result.params.old_string).toBe('## Sample config file');
  });

  test('parses <create> tool surrounded by non-tool XML', () => {
    const input = `Some text before.<create>
<file_path>docs/configuration.mdx</file_path>
<content>New file content here.</content>
</create>`;
    const result = parseXmlToolCallWithRecovery(input, VALID_TOOLS_WITH_EDIT);
    expect(result).not.toBeNull();
    expect(result.toolName).toBe('create');
    expect(result.params.file_path).toBe('docs/configuration.mdx');
  });
});