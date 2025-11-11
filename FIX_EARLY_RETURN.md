# Fix: Prevent Early Returns with Schema Instructions

## Problem

PR #291 introduced schema instructions in the initial message to prevent JSON validation loops. While this fixed the validation issue, it created a new problem where the AI would return results too early without performing adequate analysis.

## Root Cause

The schema instructions included an example like:
```json
{
  "issues": []
}
```

For code review tasks, this looked like a "completed review with no issues found" rather than a placeholder. The AI interpreted this as the expected immediate answer and returned empty results without performing the analysis.

Evidence from logs:
- `[DEBUG] --- Tool Loop Iteration 1/34 ---`
- AI returns `{"issues": []}` on first iteration
- Almost no analysis performed

## The Fix

Modified `generateSchemaInstructions()` in `npm/src/agent/schemaUtils.js`:

### Before
```javascript
// Generated example with placeholder values like {"issues": []}
const exampleObj = generateExampleFromSchema(parsedSchema, { debug });
if (exampleObj) {
  instructions += `Example:\n<attempt_completion>\n${JSON.stringify(exampleObj, null, 2)}\n</attempt_completion>\n\n`;
}
```

### After
```javascript
// Removed example entirely - schema definition is sufficient
// Just show the schema and clear instructions
instructions += 'Your response inside attempt_completion must be ONLY valid JSON - no plain text, no explanations, no markdown.\n\nIMPORTANT: First complete the requested analysis/task thoroughly, then provide your final answer in the JSON format above.';
```

## Key Changes

1. **Removed example JSON entirely**: The schema definition itself is sufficient to show the structure
2. **Added work-first instruction**: "First complete the requested analysis/task thoroughly, then provide your final answer"
3. **Removed schema from "no tool call" reminder**: Schema instructions only shown once at the beginning, not repeated in reminders
4. **Simplified approach**: Less confusion, fewer tokens, clearer intent

## Schema Instruction Strategy

### Before This Fix
Schema instructions were added in **TWO places**:
1. Initial user message (line 1462) ✅
2. "No tool call" reminder (line 2023) ❌ Unnecessary repetition

### After This Fix
Schema instructions added in **ONE place only**:
1. Initial user message (line 1462) ✅ - Sets expectations upfront
2. Removed from reminder (line 2013) ✅ - No repetition

### Rationale
- Schema shown **once at the beginning** is sufficient
- Repeating in reminders is redundant and adds noise
- When `attempt_completion` is called, validation happens then
- If validation fails, correction prompt can remind about schema format
- Cleaner conversation flow, fewer tokens

## Expected Behavior After Fix

- AI sees schema format **once** at the beginning (prevents validation loops from #291)
- AI understands it needs to perform the task first (prevents early returns)
- AI works without repeated schema reminders (cleaner flow)
- Validation happens when `attempt_completion` is called
- AI will populate the schema with actual analysis results

## Testing

The fix preserves the intent of PR #291 (prevent validation loops) while addressing the early return issue.

Example schema:
```json
{
  "type": "object",
  "properties": {
    "issues": { "type": "array", "items": {...} }
  }
}
```

Before fix: AI returns `{"issues": []}` immediately
After fix: AI performs analysis, then returns `{"issues": [...actual issues...]}`

## Impact

- Fixes the early return / empty results issue
- Maintains the JSON validation loop fix from #291
- No breaking changes to API
- Backward compatible
