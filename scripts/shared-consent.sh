#!/usr/bin/env bash
#
# Shared consent mechanism for both git pre-commit hooks and Claude Hooks
# This script implements a universal AI agent consent system using .AGENT_CONSENT file
#
# SECURITY: This script includes protections against symlink attacks:
# - Uses 'printf | tee' instead of '>' redirection for safe file creation
# - Validates consent files are not symlinks before reading them
# - Prevents file clobbering outside the repository through malicious symlinks
#

# Strict mode for better error handling
set -euo pipefail

# Colors for better output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to show consent prompt
show_consent_prompt() {
    local consent_file="$1"
    local context="$2"  # "git-commit" or "agent-task"
    
    echo "${RED}Before you can proceed, you must review the following consent form.${NC}" >&2
    echo "" >&2
    echo "${BLUE}═══════════════════════════════════════════════════════════${NC}" >&2
    echo "${BLUE}                AGENT TASK COMPLETION CONSENT${NC}" >&2
    echo "${BLUE}═══════════════════════════════════════════════════════════${NC}" >&2
    echo "" >&2
    echo "${YELLOW}Before finishing this task, you must honestly review your work.${NC}" >&2
    echo "${YELLOW}Do not lie - if any items below are not true, fix it, acknowledge failure, and ask for human help.${NC}" >&2
    echo "" >&2
    echo "${GREEN}TASK ADHERENCE:${NC}" >&2
    echo "  ✓ I have followed the task provided to me exactly (double-checked)" >&2
    echo "  ✓ I have not cut corners or taken inappropriate shortcuts" >&2
    echo "  ✓ I have not over-engineered the solution beyond what was needed" >&2
    echo "  ✓ If I did something not explicitly asked for, it was necessary for the task and I will mention it to the user" >&2
    echo "" >&2
    echo "${GREEN}CODE QUALITY (if code was modified):${NC}" >&2
    echo "  ✓ The code builds successfully without errors" >&2
    echo "  ✓ I have manually tested the changes and they work as expected" >&2
    echo "  ✓ If the code doesn't work or I don't know how to make it work, I will ask the user" >&2
    echo "  ✓ Tests are properly added and I'm satisfied with the quality" >&2
    echo "  ✓ I have not made tests pass by skipping them or using magic variables" >&2
    echo "" >&2
    
    if [ "$context" = "git-commit" ]; then
        echo "${GREEN}COMMIT SPECIFIC:${NC}" >&2
        echo "  ✓ All changes are intentional and reviewed" >&2
        echo "  ✓ Commit message accurately describes the changes" >&2
        echo "  ✓ No sensitive information is being committed" >&2
        echo "" >&2
    fi
    
    echo "${GREEN}TASK COMPLETION:${NC}" >&2
    echo "  ✓ I have completed the task to the best of my ability" >&2
    echo "  ✓ If I cannot complete the task, I am ready to explain why to the user" >&2
    echo "" >&2
    echo "${BLUE}═══════════════════════════════════════════════════════════${NC}" >&2
    echo "" >&2
    echo "${YELLOW}If you agree with all applicable items above, create the consent file with EXACTLY the content '1':${NC}" >&2
    echo "${YELLOW}  printf '1' | tee $consent_file >/dev/null${NC}" >&2
    echo "${YELLOW}  (This uses safe file creation that prevents symlink attacks)${NC}" >&2
    echo "" >&2
    echo "${RED}IMPORTANT: The file must contain exactly '1' (no extra spaces, newlines, or other content)${NC}" >&2
    echo "${RED}Any other content will be rejected as invalid consent.${NC}" >&2
    echo "" >&2
    echo "${YELLOW}If you cannot complete the task but are ready to inform the user, you may also create the consent file.${NC}" >&2
    echo "" >&2
}

# Function to check consent
check_consent() {
    local consent_file="$1"
    local context="$2"
    local success_message="$3"
    
    if [ ! -f "$consent_file" ]; then
        show_consent_prompt "$consent_file" "$context"
        return 1
    fi
    
    # SECURITY: Validate that the consent file is not a symlink
    # This prevents symlink attacks where malicious symlinks redirect file operations
    if [ -L "$consent_file" ]; then
        echo "${RED}SECURITY ERROR: Consent file is a symlink. This is not allowed for security reasons.${NC}" >&2
        echo "${RED}Please remove the symlink: rm '$consent_file'${NC}" >&2
        echo "${RED}Then create the consent file properly using the safe command shown above.${NC}" >&2
        return 1
    fi
    
    # SECURITY: Validate that the consent file contains exactly "1"
    # This prevents bypassing consent with empty files or other content
    local consent_content
    consent_content="$(cat "$consent_file" 2>/dev/null | tr -d '[:space:]')"
    
    if [ "$consent_content" != "1" ]; then
        echo "${RED}Error: Invalid consent file content. Expected '1', got: '$consent_content'${NC}" >&2
        echo "${RED}Please remove the consent file and follow the instructions to create it correctly.${NC}" >&2
        rm -f "$consent_file"  # Remove invalid file
        show_consent_prompt "$consent_file" "$context"
        return 1
    fi
    
    echo "${GREEN}$success_message${NC}"
    # Remove the consent file after successful check
    rm -f "$consent_file"
    return 0
}

# Main function - determine context and consent file location
main() {
    local context="$1"  # "git-commit", "agent-task", or "claude"
    local project_dir="${2:-$(pwd)}"
    
    # Handle Claude-specific logic
    if [ "$context" = "claude" ]; then
        # Check if we're already in a stop hook continuation
        if [ "${CLAUDE_STOP_HOOK_ACTIVE:-false}" = "true" ]; then
            echo "Stop hook continuation detected. Skipping consent check."
            exit 0
        fi
        
        # Get the project directory (Claude sets CLAUDE_PROJECT_DIR)
        project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
        context="agent-task"  # Treat Claude as agent-task after handling specific logic
    fi
    
    # Always use .AGENT_CONSENT in project root for simplicity
    consent_file="$project_dir/.AGENT_CONSENT"
    
    case "$context" in
        "git-commit")
            success_message="Commit consent confirmed. Proceeding with commit..."
            ;;
        "agent-task")
            success_message="Agent task consent confirmed. Task completion approved..."
            ;;
        *)
            echo "${RED}Error: Invalid context. Use 'git-commit', 'agent-task', or 'claude'${NC}" >&2
            exit 1
            ;;
    esac
    
    check_consent "$consent_file" "$context" "$success_message"
    return $?
}

# If script is run directly (not sourced), execute main with arguments
# Use BASH_SOURCE if available (bash), otherwise fall back to $0 comparison
if [ "${BASH_SOURCE[0]:-$0}" = "${0}" ]; then
    main "$@"
fi