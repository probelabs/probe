#!/usr/bin/env bash
set -euo pipefail

#––– Claude Hook Wrapper –––––––––––––––––––––––––––––––––––––––––––––––––
# Generic wrapper that runs any command and formats output for Claude Code
# Usage: ./scripts/claude-hook-wrapper.sh <command> [args...]
#
# Returns JSON with:
#   - decision: "approve" if command exits with 0, "block" otherwise
#   - reason: Success message or command output on failure

#––– CONSTANTS ––––––––––––––––––––––––––––––––––––––––––––––––––––––––––
readonly PASS='approve'
readonly FAIL='block'

#––– SHORT-CIRCUIT WHEN NESTED ––––––––––––––––––––––––––––––––––––––––––
if [[ ${CLAUDE_STOP_HOOK_ACTIVE:-false} == "true" ]]; then
  printf '{"decision":"%s"}\n' "$PASS"
  exit 0
fi

#––– SET STOP HOOK VARIABLE FOR CHILD PROCESSES –––––––––––––––––––––––
export CLAUDE_STOP_HOOK_ACTIVE=true

#––– CHANGE TO REPOSITORY ROOT –––––––––––––––––––––––––––––––––––––––––
# This ensures relative paths work correctly regardless of where Claude runs the hook
cd "$(dirname "$0")/.."

#––– VALIDATE ARGUMENTS –––––––––––––––––––––––––––––––––––––––––––––––––
if [[ $# -eq 0 ]]; then
  printf '{"decision":"%s","reason":"Error: No command provided to claude-hook-wrapper.sh"}\n' "$FAIL"
  exit 1
fi

#––– JSON ESCAPE FUNCTION ––––––––––––––––––––––––––––––––––––––––––––––
json_escape() { 
  if command -v jq >/dev/null 2>&1; then
    jq -Rn --arg text "$1" '$text'
  else
    # Fallback if jq is not available - ensure output is properly quoted JSON string
    # Use a more robust approach that handles multiline strings correctly
    escaped=$(printf '%s' "$1" | awk '
    BEGIN { RS = ""; FS = ""; }
    {
      # Process the entire input as one record
      gsub(/\\/, "\\\\");
      gsub(/"/, "\\\"");
      gsub(/\t/, "\\t");
      gsub(/\r/, "\\r");
      gsub(/\n/, "\\n");
      printf "%s", $0;
    }')
    printf '"%s"' "$escaped"
  fi
}

#––– RUN COMMAND ––––––––––––––––––––––––––––––––––––––––––––––––––––––––
# Capture both stdout and stderr
output=$(mktemp)
trap 'rm -f "$output"' EXIT

# Run the command, capturing all output
if "$@" >"$output" 2>&1; then
  # Command succeeded
  success_msg="✅ $1 completed successfully!"
  printf '{"decision":"%s","reason":%s}\n' "$PASS" "$(json_escape "$success_msg")"
else
  # Command failed - include the output in the reason
  exit_code=$?
  reason=$(printf "❌ %s failed with exit code %d!\n\nOutput:\n%s\n\n💡 Please fix the issues above and try again." "$1" "$exit_code" "$(<"$output")")
  printf '{"decision":"%s","reason":%s}\n' "$FAIL" "$(json_escape "$reason")"
fi