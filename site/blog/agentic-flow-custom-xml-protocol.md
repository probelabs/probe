---
title: "Technical Guide: Agentic Flow via Custom XML Protocol"
description: Learn how to implement agentic AI flows using a custom XML protocol for reliable tool interactions.
layout: doc
date: 2025-07-04
tags:
  - agentic-ai
  - xml-protocol
  - tool-use
  - llm
  - technical-guide
---

This guide details the technical implementation of an agentic AI flow where tool interactions are managed via a custom XML protocol embedded within the LLM's text output.

**1. XML Protocol Definition**

Define a strict XML schema for LLM communication.

*   **Reasoning:** LLM explains its thought process.
    ```xml
    <thinking>Analyzing user request. Need to find files related to 'authentication'. The 'search' tool is appropriate.</thinking>
    ```
*   **Tool Invocation:** LLM requests a tool execution. The root tag is the tool name. Parameters are nested.
    ```xml
    <search>
      <query>authentication implementation</query>
      <path>src/server</path>
      <allow_tests>false</allow_tests>
    </search>
    ```
    ```xml
    <extract>
      <file_path>src/server/auth.js#loginUser</file_path>
    </extract>
    ```
*   **Task Completion:** LLM signals task completion and provides the final result.
    ```xml
    <attempt_completion>
      <result>The authentication flow uses JWT tokens stored in httpOnly cookies. Key functions are `loginUser` and `verifyToken` in `src/server/auth.js`.</result>
    </attempt_completion>
    ```
*   **Result Feedback (Orchestrator -> LLM):** How the orchestrator sends tool results back to the LLM (as a `user` role message).
    ```xml
    <tool_result tool_name="search">
    File: src/server/auth.js
    Lines: 50-75
    ```javascript
    function loginUser(username, password) {
      // ... implementation ...
      const token = generateJwt(user.id);
      return { success: true, token };
    }
    ```
    (Content should be XML-escaped or use CDATA if necessary)
    </tool_result>
    ```
    ```xml
    <tool_result tool_name="extract">
    Error: File not found at path 'src/server/auth.js#loginUser'
    </tool_result>
    ```

**2. System Prompt Engineering (Critical Instructions)**

The system prompt *must* enforce the protocol rigorously.

```bash
# Role Definition
You are a code assistant...

# Core Instructions
- Your primary goal is to fulfill the user's request by reasoning and using the available tools.
- You MUST use the <thinking> tag to explain your reasoning before every action (tool call or completion).
- You MUST use XML tags for ALL tool calls and for signaling task completion. DO NOT respond with plain text unless it is inside the <result> tag of a final <attempt_completion>.
- You MUST choose EXACTLY ONE action per response turn: either ONE tool call (e.g., <search>...</search>) OR the final <attempt_completion> tag.

# XML Protocol Specification
1.  **Thinking:** Use `<thinking>Your reasoning here</thinking>` before any tool or completion tag.
2.  **Tool Calls:** Use `<tool_name>...</tool_name>` where `tool_name` is one of the available tools listed below. Parameters MUST be nested as `<param_name>value</param_name>`. Example:
    <search>
      <query>user session</query>
      <path>./src</path>
    </search>
3.  **Completion:** When the task is fully complete, respond ONLY with `<attempt_completion><result>Your final answer here</result></attempt_completion>`.
4.  **Parameter Values:** Ensure parameter values are valid text. Use XML entities (<, >, &amp;) if needed for special characters within values.

# Available Tools
- `<search>`: Searches code. Parameters: `<query>(string, required)`, `<path>(string, optional)`, `<allow_tests>(boolean, optional)`.
- `<extract>`: Extracts code snippets. Parameters: `<file_path>(string, required)`, `<line>(number, optional)`, `<end_line>(number, optional)`.
- `<query>`: (Add description if used)

# Tool Result Feedback
- After you request a tool, I will provide the result in the next message, enclosed in `<tool_result tool_name="invoked_tool_name">...</tool_result>`.
- If the tool succeeded, the content will be the tool's output.
- If the tool failed, the content will start with "Error:".
- Analyze the content of \`<tool_result>\` to inform your next <thinking> step and subsequent action. If an error occurred, adapt your plan.
```

# Example Interaction Flow
User: How does login work?
Assistant:
<thinking>User wants to understand the login flow. I should search for relevant code.</thinking>
<search>
  <query>login user function</query>
  <path>./src/auth</path>
</search>
User:
<tool_result tool_name="search">
File: src/auth/login.js: ... (function loginUser(...) { ... }) ...
</tool_result>
Assistant:
<thinking>The search found `loginUser` in `src/auth/login.js`. I should extract this function to show the user.</thinking>
<extract>
  <file_path>src/auth/login.js#loginUser</file_path>
</extract>
User:
<tool_result tool_name="extract">
```javascript
function loginUser(email, password) { /* ... */ }
```
</tool_result>
Assistant:
<thinking>I have the function code. The task is complete. I will provide the code and explanation.</thinking>
<attempt_completion>
  <result>Login is handled by the `loginUser` function in `src/auth/login.js`:
```javascript
function loginUser(email, password) { /* ... */ }
```
It likely validates credentials and generates a session token.</result>
</attempt_completion>
```

**3. Orchestrator Implementation**

This component manages the loop, calls the LLM, parses the XML from the stream, executes tools, and feeds results back.

**3.1. Core Loop Structure**

```typescript
import { streamText, CoreMessage } from 'ai';
import { openai } from '@ai-sdk/openai'; // Or other provider

async function processUserRequest(initialMessages: CoreMessage[], systemPrompt: string): Promise<string> {
  let messages: CoreMessage[] = [...initialMessages];
  const maxIterations = 10; // Prevent infinite loops

  for (let i = 0; i < maxIterations; i++) {
    // --- 1. Call LLM for next action ---
    const { textStream } = await streamText({
      model: openai('gpt-4o-mini'), // Choose appropriate model
      system: systemPrompt,
      messages: messages,
      // DO NOT pass 'tools' or 'toolChoice' here
    });

    // --- 2. Process Stream for Custom XML ---
    let assistantResponse = ""; // Full raw text from this turn
    let parsedAction: ParsedAction | null = null; // Result from XML parser

    // Use a stateful streaming XML parser (see section 3.2)
    const xmlParser = new StreamingXmlParser(['thinking', 'search', 'extract', 'query', 'attempt_completion']);

    for await (const textDelta of textStream) {
      assistantResponse += textDelta;
      parsedAction = xmlParser.processChunk(textDelta);
      if (parsedAction && (parsedAction.type === 'tool' || parsedAction.type === 'completion')) {
         // Found a terminal action (tool call or completion), stop processing stream for this turn
         // Note: A real implementation might need to drain the rest of the stream silently
         // or handle the LLM potentially generating text *after* the XML block.
         break;
      }
      // Handle 'thinking' block if needed (e.g., logging)
      if (parsedAction && parsedAction.type === 'thinking') {
          console.log("LLM Thinking:", parsedAction.content);
          parsedAction = null; // Reset after logging thinking
      }
    }

    // Ensure the parser is finalized after the stream ends
    if (!parsedAction) {
        parsedAction = xmlParser.finalize();
    }

    // Add raw assistant response to history
    messages.push({ role: 'assistant', content: assistantResponse });

    // --- 3. Analyze Parsed Action ---
    if (!parsedAction || (parsedAction.type !== 'tool' && parsedAction.type !== 'completion')) {
      // LLM failed to produce a valid tool/completion XML block
      const errorFeedback = "Error: Response did not contain a valid XML action (<tool_name>...</tool_name> or <attempt_completion>...). Adhere strictly to the protocol.";
      messages.push({ role: 'user', content: `<tool_result>Error: ${errorFeedback}</tool_result>` }); // Use tool_result structure for feedback
      if (i === maxIterations - 1) throw new Error("Max iterations reached: LLM failed to provide valid action.");
      continue; // Next iteration, hope LLM corrects
    }

    if (parsedAction.type === 'completion') {
      // Task Complete
      return parsedAction.result;
    }

    if (parsedAction.type === 'tool') {
      // --- 4. Execute Tool ---
      let toolOutput: string;
      try {
        // Add validation logic here based on parsedAction.params and tool schema
        toolOutput = await executeTool(parsedAction.name, parsedAction.params);
      } catch (error: any) {
        toolOutput = `Error: Tool execution failed. ${error.message}`;
      }

      // --- 5. Feed Result Back ---
      // Ensure output is XML-safe
      const escapedOutput = toolOutput.replace(/[<>&'"]/g, (c: string) => {
         return { '<': '<', '>': '>', '&': '&amp;', "'": '&apos;', '"': '"' }[c]!;
      });
      const resultMessageContent = `<tool_result tool_name="${parsedAction.name}">${escapedOutput}</tool_result>`;
      messages.push({ role: 'user', content: resultMessageContent });

      if (i === maxIterations - 1) throw new Error("Max iterations reached after last tool call.");
      // Continue to next iteration
    }
  } // End loop

  throw new Error("Agent loop finished unexpectedly (max iterations likely).");
}

// Placeholder types/functions
type ParsedAction =
  | { type: 'tool'; name: string; params: Record<string, string> }
  | { type: 'completion'; result: string }
  | { type: 'thinking'; content: string };

declare class StreamingXmlParser {
  constructor(knownTags: string[]);
  processChunk(chunk: string): ParsedAction | null;
  finalize(): ParsedAction | null;
}
declare function executeTool(name: string, params: Record<string, string>): Promise<string>;

```

**3.2. Streaming XML Parsing (The Hard Part)**

Parsing XML reliably from a stream where tags can be split across arbitrary text chunks (`textDelta`) is non-trivial. Simple string searching or regex on the accumulating buffer is brittle.

**Required Approach: Stateful Parser**

1.  **Buffer:** Maintain a buffer of incoming text chunks.
2.  **State Machine:** Track the current parsing state (e.g., `OUTSIDE_TAG`, `INSIDE_THINKING`, `INSIDE_SEARCH_TAG`, `INSIDE_SEARCH_QUERY_PARAM`, `INSIDE_COMPLETION_TAG`, `INSIDE_RESULT_TAG`).
3.  **Tag Detection:** Use regex or string searching optimized for finding potential start (`<tag>`) and end (`</tag>`) tags within the buffer.
4.  **Accumulation:** When a known start tag is detected, change state and start accumulating the tag's content and parameters.
5.  **Completion:** When the corresponding end tag for the *current state* is detected:
    *   Extract the fully accumulated content/parameters since the start tag.
    *   Perform final parsing/validation on the complete block.
    *   Create the `ParsedAction` object.
    *   Clear the relevant part of the buffer.
    *   Reset state to `OUTSIDE_TAG`.
6.  **Parameter Parsing:** Once a complete tool tag block is found (e.g., `<search>...</search>`), parse its inner content to extract parameter tags (`<query>...</query>`) and their values.
7.  **Error Handling:** Handle cases like unexpected tags, mismatched tags, or the stream ending while inside a tag.

**Conceptual Pseudocode for `StreamingXmlParser.processChunk`:**

```text
class StreamingXmlParser:
  buffer = ""
  state = "OUTSIDE" // e.g., OUTSIDE, IN_THINKING, IN_SEARCH, IN_SEARCH_QUERY
  currentAction = null // Stores partially built action { type, name, params }
  knownToolTags = ["search", "extract", ...]
  allKnownTags = ["thinking", "attempt_completion", "result"] + knownToolTags + paramTags...

  processChunk(chunk):
    buffer += chunk
    actionCompleted = null

    while true: // Process buffer repeatedly until no more actions found in current buffer
      foundActionThisPass = false
      if state == "OUTSIDE":
        // Check buffer for start tags <thinking>, <attempt_completion>, <search>, etc.
        match = find_earliest_start_tag(buffer, allKnownTags)
        if match:
          // Consume buffer up to tag start
          buffer = buffer.substring(match.index)
          state = "IN_" + match.tagName.toUpperCase()
          if match.tagName is a tool tag:
            currentAction = { type: "tool", name: match.tagName, params: {} }
          else if match.tagName == "thinking":
            currentAction = { type: "thinking", content: "" }
          else if match.tagName == "attempt_completion":
            currentAction = { type: "completion", result: "" } // Need to find <result> later
          // Consume the start tag itself
          buffer = buffer.substring(match.tagLength)
          foundActionThisPass = true
        else:
          break // No more start tags found in buffer for now

      else if state == "IN_THINKING":
        endTagIndex = buffer.indexOf("</thinking>")
        if endTagIndex != -1:
          currentAction.content = buffer.substring(0, endTagIndex) // Extract content
          actionCompleted = currentAction // Finalize this action
          buffer = buffer.substring(endTagIndex + len("</thinking>")) // Consume tag
          state = "OUTSIDE"
          currentAction = null
          foundActionThisPass = true
        else:
          break // End tag not yet in buffer

      else if state == "IN_SEARCH": // Example tool state
        // Look for parameter tags <query>, <path> OR the end tag </search>
        queryStart = buffer.indexOf("<query>")
        pathStart = buffer.indexOf("<path>")
        endSearch = buffer.indexOf("</search>")

        // Find the earliest relevant tag index... (complex logic here)
        earliestIndex = min_positive(queryStart, pathStart, endSearch)

        if earliestIndex == endSearch:
           // Found the end of the search tool call
           actionCompleted = currentAction
           buffer = buffer.substring(endSearch + len("</search>"))
           state = "OUTSIDE"
           currentAction = null
           foundActionThisPass = true
        else if earliestIndex == queryStart:
           // Found a query parameter tag, transition state
           state = "IN_SEARCH_QUERY"
           buffer = buffer.substring(queryStart + len("<query>"))
           foundActionThisPass = true
        else if earliestIndex == pathStart:
           // Found a path parameter tag...
           state = "IN_SEARCH_PATH"
           buffer = buffer.substring(pathStart + len("<path>"))
           foundActionThisPass = true
        else:
           break // No relevant tags found yet

      else if state == "IN_SEARCH_QUERY":
         endQuery = buffer.indexOf("</query>")
         if endQuery != -1:
            currentAction.params["query"] = buffer.substring(0, endQuery) // Extract value
            buffer = buffer.substring(endQuery + len("</query>"))
            state = "IN_SEARCH" // Go back to looking for more params or end tag
            foundActionThisPass = true
         else:
            break

      // ... other states for other tools, params, completion, result ...

      if not foundActionThisPass:
         break // Nothing more to process in the buffer currently

    return actionCompleted // Return completed action, or null if none finished
```
**Note:** This pseudocode is highly simplified. A production implementation requires careful handling of edge cases, buffering, potential nesting (if allowed), and efficient searching. Consider using a dedicated streaming XML parsing library if available for your language, adapted for finding specific top-level tags.

**4. Tool Execution Function (`executeTool`)**

```typescript
async function executeTool(name: string, params: Record<string, string>): Promise<string> {
  console.log(`Executing tool: ${name}`, params);
  try {
    switch (name) {
      case 'search':
        // Validate params.query exists, etc.
        const query = params.query;
        if (!query) throw new Error("Missing required parameter: query");
        const path = params.path || '.'; // Default path
        // Call actual search implementation
        const searchResults = await performCodeSearch(query, path);
        return formatSearchResults(searchResults); // Return results as a string

      case 'extract':
        // Validate params.file_path
        const filePath = params.file_path;
        if (!filePath) throw new Error("Missing required parameter: file_path");
        // Call actual extract implementation
        const extractedCode = await performCodeExtraction(filePath);
        return extractedCode; // Return code as string

      // Add cases for other tools

      default:
        throw new Error(`Unknown tool name: ${name}`);
    }
  } catch (error: any) {
    console.error(`Tool ${name} failed:`, error);
    // Re-throw or format error string for LLM
    throw new Error(`Tool ${name} execution failed: ${error.message}`);
  }
}

// Placeholder implementations
declare function performCodeSearch(query: string, path: string): Promise<any>;
declare function formatSearchResults(results: any): string;
declare function performCodeExtraction(filePath: string): Promise<string>;
```

**5. State and Error Handling Summary**

*   **State:** The `messages` array holds the complete interaction history, including user prompts, raw assistant responses (containing XML), and formatted `<tool_result>` messages (acting as user-role feedback).
*   **LLM Format Errors:** Detected when `parsedAction` is null after stream processing. Feedback sent in `<tool_result>Error: ...</tool_result>`.
*   **XML Parse Errors:** Should be caught by the `StreamingXmlParser` and potentially reported back similarly.
*   **Tool Validation Errors:** Caught before `executeTool` call. Feedback sent in `<tool_result>Error: Invalid parameters...</tool_result>`.
*   **Tool Execution Errors:** Caught by `executeTool`. Error message propagated back in `<tool_result>Error: Tool execution failed...</tool_result>`.

This detailed guide provides the technical foundation for building agentic flows using a custom XML protocol, addressing the complexities of stream parsing and strict LLM instruction.