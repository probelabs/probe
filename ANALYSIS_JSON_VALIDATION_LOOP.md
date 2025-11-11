# Analysis: JSON Validation Loop Issue

## Problem Summary

When using ProbeAgent with a JSON schema (via Visor's "refine" check), the AI model (Gemini 2.5 Pro) responds with an invalid XML format (`<task>` tags) instead of using the proper tool format (`<attempt_completion>`). This triggers an infinite correction loop where:

1. AI responds with `<task>` XML (not a valid tool)
2. ProbeAgent detects "no tool call" and prompts for tool use
3. AI responds with the same `<task>` format again
4. Loop repeats until max iterations (34)
5. Eventually AI uses `<attempt_completion>` with plain text instead of JSON
6. JSON validation fails and enters another correction loop (3 attempts)
7. AI keeps responding with plain text despite "CRITICAL JSON ERROR" prompts
8. After 3 failed attempts, the process fails and triggers `on_fail` routing

## Root Causes

### Cause 1: AI Model Using Wrong XML Format

**Location:** npm/src/agent/ProbeAgent.js:2045 (line from logs: 259)

The AI (Gemini 2.5 Pro) responds with:
```xml
<task>
  <refined>false</refined>
  <ask_user>true</ask_user>
  <text>Hello! How can I help you today?</text>
</task>
```

This `<task>` tag is **NOT** a valid ProbeAgent tool. The valid tools are:
- `search`, `query`, `extract`, `listFiles`, `searchFiles`
- `attempt_completion`, `attempt_complete`
- `delegate`, `bash`, `edit`, `create` (if enabled)

**Why this happens:**
- Gemini may have been trained on similar prompt patterns using `<task>` tags
- The model is not strictly following the tool definitions in the system prompt
- The schema structure (`{"refined": boolean, "text": string}`) may be confusing the model to think it should respond in XML matching the schema fields

**Evidence from logs:**
```
[DEBUG] No tool call detected in assistant response. Prompting for tool use.
```

This happens repeatedly for iterations 1-4 (lines 259, 275, 291, 307 in the log).

### Cause 2: Plain Text in attempt_completion Instead of JSON

**Location:** npm/src/agent/ProbeAgent.js:2350-2465 (JSON validation loop)

After eventually using `<attempt_completion>`, the AI provides plain text:
```
Based on the `README.md` file, Visor is an open-source tool for SDLC automation...
```

Instead of JSON:
```json
{"refined": false, "text": "Based on the `README.md` file, Visor is an open-source tool..."}
```

**Why this happens:**
- The tool definition (npm/src/tools/common.js:253-262) says: *"You can provide your response directly inside the XML tags without any parameter wrapper"*
- This guidance conflicts with the schema requirement for JSON format
- The correction prompts ("CRITICAL JSON ERROR") are added **after** the response, not **before**, so the AI doesn't see them until it's already responded incorrectly
- The correction loop uses the same conversation context, but the AI persists in its behavior

**Evidence from logs:**
Lines 403-410 show the first JSON validation failure:
```
[DEBUG] JSON validation: Parse failed with error: Unexpected token 'B', "Based on t"... is not valid JSON
```

Lines 714-816 show the same error persisting through all 3 correction attempts.

### Cause 3: Ineffective Correction Strategy

**Location:** npm/src/agent/schemaUtils.js:640-681 (createJsonCorrectionPrompt)

The correction prompts escalate in urgency:
1. Attempt 1: "CRITICAL JSON ERROR"
2. Attempt 2: "URGENT - JSON PARSING FAILED"
3. Attempt 3: "FINAL ATTEMPT - CRITICAL JSON ERROR"

**Why this fails:**
- The AI has already "decided" on its response format in the conversation context
- Adding corrections as follow-up messages doesn't change the AI's understanding of the task
- The model may be in a "stuck" state where it keeps reusing the same pattern
- No temperature adjustment or alternative strategies are employed

## Detailed Flow of the Bug

### First Run (Visor session 1 - lines 1-96)

1. **Line 1:** User input: "Hi! Just checking"
2. **Lines 4-13:** Initial `attempt_completion` response is **correctly formatted** with full feature explanation
3. **Lines 14-21:** JSON validation **fails** because the response is plain text, not JSON
4. **Lines 22-34:** Mermaid validation passes (no diagrams found)
5. **Lines 36-51:** Response is parsed, no JSON found, treated as plain text
6. **Lines 61-63:** `fail_if` condition evaluates: `output['refined'] !== true` → **true** (fails)
7. **Line 64:** `on_fail.goto('ask')` routes back to ask check
8. **Line 96:** User is prompted again (Wave 2)

### Second Run (Visor session 2 - lines 97-874)

1. **Line 169:** User input: "Hi! Just checking"
2. **Lines 175-253:** AI enters tool loop but responds with `<task>` XML (iterations 1-4)
3. **Line 259:** "No tool call detected" - ProbeAgent prompts for tool use
4. **Lines 254-320:** Loop repeats 4 times with same `<task>` response
5. **Line 321:** AI finally uses `<extract>` tool (iteration 5)
6. **Line 385:** AI uses `<attempt_completion>` with plain text (iteration 6)
7. **Lines 403-410:** JSON validation fails (first time)
8. **Lines 415-447:** Correction attempt 1 with "CRITICAL JSON ERROR" prompt
9. **Lines 720-777:** AI responds with SAME plain text (correction failed)
10. **Lines 780-804:** Same error persists through attempts 2 and 3
11. **Lines 815-821:** Validation fails after 3 attempts
12. **Lines 849-853:** `fail_if` triggers again, routes back to `ask`

## Impact

1. **Performance:** Wasted API calls (up to 37 iterations total: 34 for tools + 3 for corrections)
2. **Cost:** Multiple API requests to expensive models (Gemini 2.5 Pro)
3. **User Experience:** Long delays before failure (161.5s in first run, 116.2s in second)
4. **Reliability:** Task fails even though the AI had the correct answer

## Proposed Solutions

### Solution 1: Improve Tool Reminder Clarity (Quick Fix)

**File:** npm/src/agent/ProbeAgent.js:2006-2038

**Current behavior:** Generic reminder that doesn't explain the schema requirement clearly

**Proposed change:** When schema is provided, make it crystal clear that:
- The response MUST be wrapped in `<attempt_completion>` tags
- The content MUST be valid JSON matching the schema
- NO other XML tags like `<task>` are allowed

```javascript
if (options.schema) {
  reminderContent = `You MUST use the attempt_completion tool to provide your answer as valid JSON.

CRITICAL: Your response must be in this exact format:
<attempt_completion>
${JSON.stringify(schemaExample, null, 2)}
</attempt_completion>

Where the content inside the tags is valid JSON matching this schema:
${options.schema}

DO NOT use <task>, <response>, or any other XML tags.
DO NOT provide plain text - it must be parseable by JSON.parse().
Use attempt_completion NOW if you have enough information.`;
}
```

### Solution 2: Detect Invalid XML Early (Medium Fix)

**File:** npm/src/agent/ProbeAgent.js (in tool parsing logic)

**Current behavior:** Any unrecognized XML is treated as "no tool call"

**Proposed change:** Detect common invalid patterns (like `<task>`, `<response>`) and provide specific correction:

```javascript
// After line 1715 (parsedTool = ...)
if (!parsedTool) {
  // Check for common invalid tags
  const invalidTags = ['<task>', '<response>', '<answer>'];
  const hasInvalidTag = invalidTags.some(tag => assistantMessage.includes(tag));

  if (hasInvalidTag) {
    // Provide specific correction instead of generic reminder
    currentMessages.push({
      role: 'user',
      content: `ERROR: You used an invalid XML tag. You MUST use one of these valid tools:
- <attempt_completion> (to provide final answer)
- <search>, <extract>, <query> (to search/analyze code)

${options.schema ? `Since a schema is provided, use attempt_completion with valid JSON:
<attempt_completion>
{"key": "value matching schema"}
</attempt_completion>` : ''}`
    });
    continue; // Skip generic reminder
  }
}
```

### Solution 3: Early Schema Format Enforcement (Robust Fix)

**File:** npm/src/agent/ProbeAgent.js (system prompt generation)

**Current behavior:** Schema instructions are added AFTER the first response via recursive call

**Proposed change:** Include schema requirements in the INITIAL system prompt:

```javascript
// In buildSystemPrompt() around line 1200
if (options.schema && !options._schemaFormatted) {
  const schemaGuidelines = `
# JSON Schema Response Requirement

CRITICAL: This task requires a JSON response matching this schema:
${options.schema}

When you use attempt_completion, the content MUST be valid JSON:
<attempt_completion>
{"field": "value"}  ← Must be parseable by JSON.parse()
</attempt_completion>

DO NOT provide plain text responses.
DO NOT use <task> or other non-standard XML tags.
`;

  // Add to system message before tool definitions
  systemMessage += schemaGuidelines;
}
```

### Solution 4: Use Structured Output Mode (Best Fix - Requires Provider Support)

**File:** npm/src/providers/google.js or similar

**Current behavior:** Relies on AI to format response correctly in text

**Proposed change:** Use Google's structured output mode (if available) to enforce JSON schema:

```javascript
// In Google provider
if (options.schema && isJsonSchema(options.schema)) {
  // Use responseSchema parameter for enforced structured output
  const requestPayload = {
    contents: messages,
    generationConfig: {
      responseSchema: JSON.parse(options.schema),
      responseMimeType: 'application/json'
    }
  };
  // This forces the model to ONLY return valid JSON matching the schema
}
```

### Solution 5: Add Circuit Breaker for Repetitive Responses (Safety Net)

**File:** npm/src/agent/ProbeAgent.js (in tool loop)

**Current behavior:** Continues trying even when AI is stuck in a pattern

**Proposed change:** Detect when AI repeats the same invalid response:

```javascript
const responseHistory = new Map(); // Track recent responses
let repetitionCount = 0;

// After getting assistant response
const responseHash = crypto.createHash('md5').update(assistantMessage).digest('hex');
if (responseHistory.has(responseHash)) {
  repetitionCount = responseHistory.get(responseHash) + 1;
  responseHistory.set(responseHash, repetitionCount);

  if (repetitionCount >= 2) {
    // AI is stuck - try drastic intervention
    if (options.schema) {
      // Force completion with error message
      finalResult = JSON.stringify({
        error: "AI failed to format response correctly after multiple attempts",
        raw_response: assistantMessage
      });
      completionAttempted = true;
      break;
    }
  }
} else {
  responseHistory.set(responseHash, 1);
}
```

## Recommended Implementation Order

1. **Immediate (Solution 1):** Improve schema reminder clarity - Low effort, immediate impact
2. **Short-term (Solution 2):** Detect invalid XML early - Prevents wasted iterations
3. **Medium-term (Solution 3):** Add schema to system prompt - More robust
4. **Long-term (Solution 4):** Use structured output - Most reliable but requires provider support
5. **Safety net (Solution 5):** Add circuit breaker - Prevents infinite loops

## Testing Strategy

### Test Case 1: Schema with Simple Object
```javascript
const schema = {
  type: "object",
  properties: {
    refined: { type: "boolean" },
    text: { type: "string" }
  }
};
// Should return: {"refined": true, "text": "answer"}
```

### Test Case 2: Schema with Nested Objects
```javascript
const schema = {
  type: "object",
  properties: {
    summary: { type: "string" },
    items: {
      type: "array",
      items: { type: "string" }
    }
  }
};
```

### Test Case 3: Different Providers
- Test with Anthropic (Claude)
- Test with Google (Gemini)
- Test with OpenAI (GPT-4)

Verify each provider correctly formats JSON responses.

## Related Files

- `npm/src/agent/ProbeAgent.js` - Main agent loop and tool handling
- `npm/src/agent/schemaUtils.js` - JSON validation and correction
- `npm/src/tools/common.js` - Tool definitions
- `npm/src/providers/google.js` - Google AI provider implementation

## References

- Original issue log: `/Users/leonidbugaev/Library/Application Support/com.conductor.app/uploads/originals/8f9ce4da-3364-40f3-9620-d23401b69f9d.txt`
- ProbeAgent documentation: `npm/README.md`
- Visor check configuration: Referenced in log as `defaults/task-refinement.yaml`
