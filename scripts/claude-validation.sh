#!/bin/bash
# Claude Code Stop hook validation for probe repository

# Prevent infinite loops
if [ "$CLAUDE_STOP_HOOK_ACTIVE" = "true" ]; then
    echo '{"decision": "allow"}'
    exit 0
fi

# Run validation commands
cd "$(dirname "$0")/.."

# Capture all errors
errors=""

# Auto-format first, then check formatting
echo "🔧 Auto-formatting code..." >&2
make format >/dev/null 2>&1

echo "🔍 Checking code formatting..." >&2
format_output=$(make check-format 2>&1)
if [ $? -ne 0 ]; then
    errors="${errors}📝 CODE FORMATTING ISSUES:\n${format_output}\n\n"
fi

# Check linting
echo "🔍 Running linter (clippy)..." >&2
lint_output=$(make lint 2>&1)
if [ $? -ne 0 ]; then
    errors="${errors}🔧 LINTING ISSUES:\n${lint_output}\n\n"
fi

# Check tests
echo "🔍 Running all tests..." >&2
test_output=$(make test 2>&1)
if [ $? -ne 0 ]; then
    errors="${errors}🧪 TEST FAILURES:\n${test_output}\n\n"
fi

# Generate response
if [ -z "$errors" ]; then
    echo '{"decision": "allow", "reason": "✅ All validations passed: formatting, linting, and tests!"}'
else
    # Escape the errors for JSON (replace quotes and newlines)
    escaped_errors=$(echo -e "$errors" | sed 's/"/\\"/g' | sed 's/$/\\n/' | tr -d '\n' | sed 's/\\n$//')
    echo "{\"decision\": \"block\", \"reason\": \"❌ Validation failed! Please fix these issues:\\n\\n${escaped_errors}\\n💡 Quick fixes: Run 'make fix-all' to auto-fix formatting/linting, then address test failures.\"}"
fi