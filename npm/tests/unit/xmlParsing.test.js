import { parseXmlToolCall, parseXmlToolCallWithThinking } from '../../src/agent/tools.js';
import { DEFAULT_VALID_TOOLS, buildToolTagPattern, detectUnrecognizedToolCall, unescapeXmlEntities } from '../../src/tools/common.js';
import { removeThinkingTags, extractThinkingContent } from '../../src/agent/xmlParsingUtils.js';

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
      
      expect(result).toMatchObject({
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

      expect(result).toMatchObject({
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

    test('should return thinkingContent when present', () => {
      const xmlString = `
        <thinking>
        I need to analyze this code carefully.
        Let me think about the best approach.
        </thinking>
        <search>
          <query>authentication</query>
        </search>
      `;

      const result = parseXmlToolCallWithThinking(xmlString);

      expect(result).toMatchObject({
        toolName: 'search',
        params: { query: 'authentication' }
      });
      expect(result.thinkingContent).toBeDefined();
      expect(result.thinkingContent).toContain('I need to analyze this code carefully');
      expect(result.thinkingContent).toContain('best approach');
    });

    test('should return null thinkingContent when no thinking tags', () => {
      const xmlString = `
        <search>
          <query>test query</query>
        </search>
      `;

      const result = parseXmlToolCallWithThinking(xmlString);

      expect(result).toMatchObject({
        toolName: 'search',
        params: { query: 'test query' }
      });
      expect(result.thinkingContent).toBeNull();
    });

    test('should extract thinkingContent from multiple thinking blocks (first only)', () => {
      const xmlString = `
        <thinking>First thought block</thinking>
        <thinking>Second thought block</thinking>
        <extract>
          <targets>file.js</targets>
        </extract>
      `;

      const result = parseXmlToolCallWithThinking(xmlString);

      expect(result).toMatchObject({
        toolName: 'extract',
        params: { targets: 'file.js' }
      });
      // extractThinkingContent only captures first thinking block
      expect(result.thinkingContent).toBe('First thought block');
    });

    test('should include thinkingContent with attempt_completion', () => {
      const xmlString = `
        <thinking>
        The task is complete. I've analyzed all the files.
        </thinking>
        <attempt_completion>Task completed successfully</attempt_completion>
      `;

      const result = parseXmlToolCallWithThinking(xmlString);

      expect(result).toMatchObject({
        toolName: 'attempt_completion',
        params: { result: 'Task completed successfully' }
      });
      expect(result.thinkingContent).toContain('The task is complete');
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
          <targets>src/components/Header.js:1-50</targets>
        </extract>

        <em>This will show you the first 50 lines of the Header component.</em>
      `;

      const result = parseXmlToolCallWithThinking(aiResponse);

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
      const result = parseXmlToolCallWithThinking(aiResponse, validToolsWithEdit);

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
      const result = parseXmlToolCallWithThinking(aiResponse, validToolsWithoutEdit);

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

      const result = parseXmlToolCallWithThinking(aiResponse);

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

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toMatchObject({
        toolName: 'attempt_completion',
        params: {
          result: 'Task completed with all requirements met.'
        }
      });
    });

    test('should handle empty attempt_completion tag without closing', () => {
      const aiResponse = `<attempt_completion>`;

      const result = parseXmlToolCallWithThinking(aiResponse);

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

      const result = parseXmlToolCallWithThinking(aiResponse);

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

  describe('Unclosed thinking tag handling', () => {
    test('should remove unclosed thinking tag and its content', () => {
      const aiResponse = `<thinking>
I need to search for the authentication code.
This is my reasoning...

<search>
<query>authentication</query>
</search>`;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toMatchObject({
        toolName: 'search',
        params: { query: 'authentication' }
      });
    });

    test('should handle properly closed thinking tag', () => {
      const aiResponse = `<thinking>
Let me analyze this.
</thinking>

<extract>
<targets>src/auth.js</targets>
</extract>`;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toMatchObject({
        toolName: 'extract',
        params: { targets: 'src/auth.js' }
      });
    });

    test('should handle multiple thinking tags with one unclosed', () => {
      const aiResponse = `<thinking>First thought</thinking>
<thinking>Second thought that never ends...

<query>
<pattern>test</pattern>
</query>`;

      const result = parseXmlToolCallWithThinking(aiResponse);

      expect(result).toMatchObject({
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

      const result = parseXmlToolCallWithThinking(aiResponse);

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

      const result = parseXmlToolCallWithThinking(aiResponse);

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

      const result = parseXmlToolCallWithThinking(aiResponse);

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

      const result = parseXmlToolCallWithThinking(aiResponse, validTools);

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

      const result = parseXmlToolCallWithThinking(aiResponse, validTools);

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

describe('removeThinkingTags and extractThinkingContent', () => {
  test('removeThinkingTags strips closed thinking tags', () => {
    const input = '<thinking>some reasoning</thinking>The actual answer here.';
    expect(removeThinkingTags(input)).toBe('The actual answer here.');
  });

  test('removeThinkingTags strips unclosed thinking tags', () => {
    const input = '<thinking>some reasoning that never closes';
    expect(removeThinkingTags(input)).toBe('');
  });

  test('removeThinkingTags returns empty when content is entirely in thinking tags', () => {
    const input = '<thinking>This is a long detailed analysis that the model put entirely in thinking tags. It contains all the useful content but will be stripped away.</thinking>';
    expect(removeThinkingTags(input)).toBe('');
  });

  test('removeThinkingTags preserves content outside thinking tags', () => {
    const input = '<thinking>reasoning</thinking>\n\nHere is the detailed answer with more than 50 characters of substantive content.';
    const result = removeThinkingTags(input);
    expect(result).toContain('Here is the detailed answer');
    expect(result.length).toBeGreaterThan(50);
  });

  test('extractThinkingContent extracts content from thinking tags', () => {
    const input = '<thinking>This is the actual analysis with useful content</thinking>';
    expect(extractThinkingContent(input)).toBe('This is the actual analysis with useful content');
  });

  test('extractThinkingContent returns null when no thinking tags', () => {
    expect(extractThinkingContent('No thinking tags here')).toBeNull();
  });

  describe('__PREVIOUS_RESPONSE__ thinking tag regression', () => {
    test('should detect when previous response content is mostly inside thinking tags', () => {
      // This simulates the bug: model puts answer in thinking tags,
      // attempt_complete reuses it, then removeThinkingTags destroys the answer
      const prevResponse = '<thinking>\nI have analyzed the two traces. The failures are caused by excessive data size from raw CI logs being fed into Gemini 2.5 Pro.\n\n1. Limit output size in execute_plan DSL\n2. Summarize before returning\n3. Warn/abort on large tool results\n</thinking>';

      const stripped = removeThinkingTags(prevResponse);
      const thinkingContent = extractThinkingContent(prevResponse);

      // The stripped version would be nearly empty (the bug)
      expect(stripped.length).toBeLessThan(50);

      // But extractThinkingContent recovers the actual answer
      expect(thinkingContent).toContain('excessive data size');
      expect(thinkingContent.length).toBeGreaterThan(50);
    });

    test('should preserve content when it exists outside thinking tags', () => {
      const prevResponse = '<thinking>Let me think about this</thinking>\n\nThe analysis shows that both failures are caused by excessive data size from raw CI logs. Here are the recommendations:\n1. Limit output size\n2. Summarize before returning';

      const stripped = removeThinkingTags(prevResponse);

      // Enough content outside thinking tags — no need for fallback
      expect(stripped.length).toBeGreaterThan(50);
      expect(stripped).toContain('excessive data size');
    });
  });
});

describe('Issue #439: removeThinkingTags destroys JSON output', () => {
  describe('extractThinkingContent should handle nested thinking tags', () => {
    test('should not return content starting with <thinking>', () => {
      const input = '<thinking><thinking>content</thinking></thinking>';
      const result = extractThinkingContent(input);
      expect(result).not.toMatch(/^<thinking>/);
      expect(result).toBe('content');
    });

    test('should strip inner thinking tags from extracted content', () => {
      const input = '<thinking>\n<thinking>\nReal content here\n</thinking>\n</thinking>';
      const result = extractThinkingContent(input);
      expect(result).not.toContain('<thinking>');
      expect(result).not.toContain('</thinking>');
      expect(result).toContain('Real content here');
    });

    test('should handle deeply nested thinking tags', () => {
      const input = '<thinking><thinking><thinking>deep content</thinking></thinking></thinking>';
      const result = extractThinkingContent(input);
      expect(result).not.toContain('<thinking>');
      expect(result).toBe('deep content');
    });

    test('should handle nested tags with actual content mixed in', () => {
      const input = '<thinking>outer start\n<thinking>inner content</thinking>\nouter end</thinking>';
      const result = extractThinkingContent(input);
      // Non-greedy match captures from outer <thinking> to first </thinking>
      // which is "outer start\n<thinking>inner content"
      // Then we recursively extract from the inner <thinking>, getting "inner content"
      expect(result).not.toContain('<thinking>');
    });

    test('should return null for empty nested thinking tags', () => {
      const input = '<thinking><thinking></thinking></thinking>';
      const result = extractThinkingContent(input);
      // After extracting and cleaning, content is empty
      expect(result).toBeNull();
    });

    test('production scenario: Gemini 2.5 Pro nested thinking (issue #439 trace)', () => {
      const input = `<thinking>
<thinking>
I have already explained to the user why the full debugging process
was not followed. I have analyzed the trace and confirmed that the
assistant only performed the initial steps...
</thinking>
</thinking>`;
      const result = extractThinkingContent(input);
      expect(result).not.toMatch(/^<thinking>/);
      expect(result).not.toContain('<thinking>');
      expect(result).not.toContain('</thinking>');
      expect(result).toContain('I have already explained');
    });
  });

  describe('removeThinkingTags behavior with JSON (documenting the bug)', () => {
    test('BUG: removeThinkingTags DESTROYS JSON with closed <thinking> in string values', () => {
      // This documents the BUG behavior - removeThinkingTags strips the tags
      // including from inside JSON strings, corrupting the structure
      const json = '{"text":"<thinking>some analysis</thinking>rest of content"}';
      const result = removeThinkingTags(json);
      // The <thinking>...</thinking> is removed even from inside the JSON string
      expect(result).toBe('{"text":"rest of content"}');
      // This is technically valid JSON but loses content
    });

    test('BUG: removeThinkingTags TRUNCATES JSON with unclosed <thinking> in string values', () => {
      // This documents the CRITICAL BUG - unclosed <thinking> truncates everything
      const json = '{"text":"<thinking>content without closing tag"}';
      const result = removeThinkingTags(json);
      // TRUNCATED! Everything from <thinking> onwards is gone
      expect(result).toBe('{"text":"');
      // This is INVALID JSON - the exact bug from issue #439
      expect(() => JSON.parse(result)).toThrow();
    });

    test('removeThinkingTags works correctly when thinking tags are OUTSIDE JSON', () => {
      const input = '<thinking>reasoning</thinking>{"text":"actual content"}';
      const result = removeThinkingTags(input);
      expect(result).toBe('{"text":"actual content"}');
      expect(() => JSON.parse(result)).not.toThrow();
    });
  });

  describe('FIX: skip removeThinkingTags for valid JSON', () => {
    test('valid JSON should be detected and preserved (not passed to removeThinkingTags)', () => {
      // This is the fix logic from ProbeAgent.js:4802-4818
      const jsonWithEmbeddedThinking = '{"text":"<thinking>content"}';

      // Step 1: Check if it's valid JSON
      let isValidJson = false;
      try {
        JSON.parse(jsonWithEmbeddedThinking);
        isValidJson = true;
      } catch {
        // Not valid JSON
      }

      // Step 2: If valid JSON, we skip removeThinkingTags
      expect(isValidJson).toBe(true);

      // Step 3: Since we skip removeThinkingTags, content is preserved
      const finalResult = jsonWithEmbeddedThinking; // NOT calling removeThinkingTags
      expect(() => JSON.parse(finalResult)).not.toThrow();
      const parsed = JSON.parse(finalResult);
      expect(parsed.text).toBe('<thinking>content');
    });

    test('non-JSON content should still have removeThinkingTags applied', () => {
      const plainTextWithThinking = '<thinking>reasoning</thinking>The actual answer';

      // Step 1: Check if it's valid JSON
      let isValidJson = false;
      try {
        JSON.parse(plainTextWithThinking);
        isValidJson = true;
      } catch {
        // Not valid JSON
      }

      expect(isValidJson).toBe(false);

      // Step 2: Since not JSON, apply removeThinkingTags
      const finalResult = removeThinkingTags(plainTextWithThinking);
      expect(finalResult).toBe('The actual answer');
    });

    test('production scenario: JSON with residual thinking from nested extraction', () => {
      // Simulate: nested thinking → extractThinkingContent fails to clean →
      // tryAutoWrapForSimpleSchema embeds in JSON → final cleanup
      const residualThinking = '<thinking>some residual content from nested extraction';
      const wrappedJson = JSON.stringify({ text: residualThinking });

      // Without the fix: removeThinkingTags would truncate
      const withoutFix = removeThinkingTags(wrappedJson);
      expect(withoutFix).toBe('{"text":"');
      expect(() => JSON.parse(withoutFix)).toThrow();

      // With the fix: we detect valid JSON and skip removeThinkingTags
      let isValidJson = false;
      try {
        JSON.parse(wrappedJson);
        isValidJson = true;
      } catch {
        // Not valid JSON
      }
      expect(isValidJson).toBe(true);

      // Content is preserved
      const withFix = wrappedJson; // Skip removeThinkingTags for valid JSON
      expect(() => JSON.parse(withFix)).not.toThrow();
      const parsed = JSON.parse(withFix);
      expect(parsed.text).toContain('some residual content');
    });
  });

  describe('Real scenario: auto-wrapped content with nested thinking extraction', () => {
    test('complete flow: nested thinking → extract → auto-wrap → should preserve content', () => {
      // Simulate Gemini 2.5 Pro producing nested thinking
      const aiResponse = '<thinking>\n<thinking>\nActual analysis content here.\n</thinking>\n</thinking>';

      // Step 1: extractThinkingContent (now fixed for nested tags)
      const extracted = extractThinkingContent(aiResponse);

      // The extracted content should NOT start with <thinking>
      expect(extracted).not.toMatch(/^<thinking>/);
      expect(extracted).toContain('Actual analysis content here');

      // Step 2: Simulate auto-wrap (JSON.stringify)
      const wrapped = JSON.stringify({ text: extracted });

      // Step 3: The wrapped JSON should be valid
      expect(() => JSON.parse(wrapped)).not.toThrow();

      // Step 4: Parse and verify content is preserved
      const parsed = JSON.parse(wrapped);
      expect(parsed.text).toContain('Actual analysis content here');
      expect(parsed.text.length).toBeGreaterThan(10);
    });

    test('complete flow: when skip removeThinkingTags for valid JSON is applied', () => {
      // Simulate the scenario where content with residual <thinking> gets auto-wrapped
      const contentWithResidualTag = '<thinking>some residual thinking text';
      const wrapped = JSON.stringify({ text: contentWithResidualTag });

      // If we detect valid JSON, we should NOT call removeThinkingTags
      let finalResult = wrapped;
      let isValidJson = false;
      try {
        JSON.parse(finalResult);
        isValidJson = true;
      } catch {
        // Not valid JSON
      }

      // The JSON is valid, so we should skip removeThinkingTags
      expect(isValidJson).toBe(true);

      // The final result should still be valid JSON
      expect(() => JSON.parse(finalResult)).not.toThrow();

      // Content should be preserved (not truncated)
      const parsed = JSON.parse(finalResult);
      expect(parsed.text).toContain('some residual thinking text');
    });
  });

  describe('__PREVIOUS_RESPONSE__ length-based heuristic edge cases', () => {
    test('length > 50 check behavior with extracted thinking content', () => {
      // A valid answer inside thinking tags
      const prevResponse = '<thinking>The build failed because of a missing dependency in package.json.</thinking>';

      // Extract thinking content (simulating __PREVIOUS_RESPONSE__ handler)
      const thinkingContent = extractThinkingContent(prevResponse);

      // The content should be extracted and cleaned
      expect(thinkingContent).toBe('The build failed because of a missing dependency in package.json.');

      // This content is > 50 chars so it would pass the length check
      expect(thinkingContent.length).toBeGreaterThan(50);
    });

    test('short valid answer inside thinking tags', () => {
      // A perfectly valid short answer that would fail the > 50 check
      const prevResponse = '<thinking>Build failed: missing dep.</thinking>';

      const thinkingContent = extractThinkingContent(prevResponse);

      expect(thinkingContent).toBe('Build failed: missing dep.');
      // This is < 50 chars and would fail the length check
      expect(thinkingContent.length).toBeLessThan(50);
    });

    test('nested thinking with garbage would now be cleaned', () => {
      // Previously, this would return "<thinking>" repeated content
      // which would pass the length > 50 check despite being garbage
      const garbage = '<thinking><thinking><thinking>actual content</thinking></thinking></thinking>';

      const result = extractThinkingContent(garbage);

      // With the fix, we get clean content without any thinking tags
      expect(result).not.toContain('<thinking>');
      expect(result).toBe('actual content');
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