#!/usr/bin/env bash
#
# Shared consent mechanism for both git pre-commit hooks and Claude Hooks
# This script implements a universal AI agent consent system with per-task consent files
# 
# Per-task consent prevents race conditions by using task-specific consent files:
# - Environment variable AGENT_CONSENT_KEY specifies unique task identifiers
# - For git commits, uses commit hash or staged content hash as identifier
# - Maintains backward compatibility with simple .AGENT_CONSENT file
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

# Function to generate a hash for the current git staged content
generate_git_staged_hash() {
    if ! command -v git >/dev/null 2>&1; then
        return 1
    fi
    
    # Generate hash based on staged content to ensure uniqueness per commit
    if git diff --cached --quiet; then
        # No staged changes - use current HEAD commit hash
        git rev-parse HEAD 2>/dev/null | cut -c1-8
    else
        # Hash the staged diff content for uniqueness
        git diff --cached | sha256sum 2>/dev/null | cut -c1-8 || \
        git diff --cached | shasum -a 256 2>/dev/null | cut -c1-8 || \
        git diff --cached | md5sum 2>/dev/null | cut -c1-8 || \
        git diff --cached | md5 2>/dev/null | cut -c1-8 || \
        echo "$(date +%s)" # Fallback to timestamp
    fi
}

# Function to determine the consent file name based on task context
get_consent_filename() {
    local context="$1"
    local project_dir="$2"
    
    # Check for explicit task identifier from environment
    if [ -n "${AGENT_CONSENT_KEY:-}" ]; then
        echo "$project_dir/.AGENT_CONSENT_${AGENT_CONSENT_KEY}"
        return 0
    fi
    
    # For git commits, generate hash-based consent file
    if [ "$context" = "git-commit" ]; then
        local git_hash
        if git_hash="$(generate_git_staged_hash)"; then
            echo "$project_dir/.AGENT_CONSENT_${git_hash}"
            return 0
        fi
    fi
    
    # Fallback to simple consent file for backward compatibility
    echo "$project_dir/.AGENT_CONSENT"
}

# Function to read and parse consent content from markdown file
read_consent_markdown() {
    local project_dir="$1"
    local markdown_file="$project_dir/AGENT_CONSENT.md"
    
    # Check if markdown file exists and is readable
    if [ ! -f "$markdown_file" ] || [ ! -r "$markdown_file" ]; then
        return 1  # Signal to use fallback
    fi
    
    # SECURITY: Validate that the markdown file is not a symlink
    if [ -L "$markdown_file" ]; then
        echo "${YELLOW}Warning: AGENT_CONSENT.md is a symlink. Using fallback consent text for security.${NC}" >&2
        return 1
    fi
    
    # Read and return the markdown content
    cat "$markdown_file" 2>/dev/null
}

# Function to convert markdown content to colored terminal output
format_consent_content() {
    local content="$1"
    local consent_filename="$2"
    local consent_file="$3"
    local context="$4"
    
    # Process the markdown content with context-aware filtering
    local skip_section=""
    local temp_file
    temp_file=$(mktemp)
    echo "$content" > "$temp_file"
    
    while IFS= read -r line; do
        case "$line" in
            "# "*)
                # Main heading - use blue with borders
                title="${line#\# }"
                echo "${BLUE}═══════════════════════════════════════════════════════════${NC}" >&2
                printf "${BLUE}%*s${NC}\\n" $(((${#title} + 63) / 2)) "$title" >&2
                echo "${BLUE}═══════════════════════════════════════════════════════════${NC}" >&2
                echo "" >&2
                ;;
            "## "*)
                # Section headings - use green
                section="${line#\#\# }"
                # Skip commit-specific section if not in git-commit context
                if [[ "$section" == *"COMMIT SPECIFIC"* ]] && [ "$context" != "git-commit" ]; then
                    skip_section="commit"
                    continue
                fi
                # Reset skip flag for other sections
                if [[ "$section" != *"COMMIT SPECIFIC"* ]]; then
                    skip_section=""
                fi
                echo "${GREEN}$section:${NC}" >&2
                ;;
            "- ✓ "* | "  ✓ "*)
                # Checklist items - clean up and display, unless in skipped section
                if [ "$skip_section" = "commit" ]; then
                    continue
                fi
                item="${line#*✓ }"
                echo "  ✓ $item" >&2
                ;;
            "**Do not lie**"*)
                # Bold warning text - use yellow
                text="${line#\*\*Do not lie\*\* - }"
                echo "${YELLOW}Do not lie - $text${NC}" >&2
                ;;
            "**If you agree with all applicable items above**"*)
                # Instructions start - use yellow
                echo "${YELLOW}If you agree with all applicable items above, create the consent file with EXACTLY the content '1':${NC}" >&2
                ;;
            "printf '1' | tee CONSENT_FILENAME >/dev/null")
                # Command template - replace placeholder and use yellow
                echo "${YELLOW}  printf '1' | tee $consent_filename >/dev/null${NC}" >&2
                echo "${YELLOW}  Full path: $consent_file${NC}" >&2
                echo "${YELLOW}  (This uses safe file creation that prevents symlink attacks)${NC}" >&2
                ;;
            "**IMPORTANT**"*)
                # Important note - use red
                text="${line#\*\*IMPORTANT\*\*: }"
                echo "${RED}IMPORTANT: $text${NC}" >&2
                ;;
            "Any other content will be rejected as invalid consent.")
                # Validation warning - use red
                echo "${RED}$line${NC}" >&2
                ;;
            "If you cannot complete the task"*)
                # Final note - use yellow
                echo "${YELLOW}$line${NC}" >&2
                ;;
            "---" | '```'* | "")
                # Skip separators, code blocks, and empty lines
                if [ "$line" = "" ]; then
                    echo "" >&2
                fi
                ;;
            *)
                # Regular text - use yellow for descriptions
                if [[ "$line" == *"Before finishing this task"* ]]; then
                    echo "${YELLOW}$line${NC}" >&2
                elif [ -n "$line" ]; then
                    echo "$line" >&2
                fi
                ;;
        esac
    done < "$temp_file"
    
    # Clean up temporary file
    rm -f "$temp_file"
}

# Function to show hardcoded fallback consent prompt
show_fallback_consent_prompt() {
    local consent_filename="$1"
    local consent_file="$2"
    local context="$3"
    
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
    echo "${YELLOW}  printf '1' | tee $consent_filename >/dev/null${NC}" >&2
    echo "${YELLOW}  Full path: $consent_file${NC}" >&2
    echo "${YELLOW}  (This uses safe file creation that prevents symlink attacks)${NC}" >&2
    echo "" >&2
    echo "${RED}IMPORTANT: The file must contain exactly '1' (no extra spaces, newlines, or other content)${NC}" >&2
    echo "${RED}Any other content will be rejected as invalid consent.${NC}" >&2
    echo "" >&2
    echo "${YELLOW}If you cannot complete the task but are ready to inform the user, you may also create the consent file.${NC}" >&2
    echo "" >&2
}

# Function to show consent prompt
show_consent_prompt() {
    local consent_file="$1"
    local context="$2"  # "git-commit" or "agent-task"
    local consent_filename
    consent_filename="$(basename "$consent_file")"
    
    # Get project directory from consent file path
    local project_dir
    project_dir="$(dirname "$consent_file")"
    
    echo "${RED}Before you can proceed, you must review the following consent form.${NC}" >&2
    echo "" >&2
    
    # Try to read consent content from markdown file
    local markdown_content
    if markdown_content="$(read_consent_markdown "$project_dir")"; then
        # Successfully read markdown file, format and display it
        format_consent_content "$markdown_content" "$consent_filename" "$consent_file" "$context"
    else
        # Fallback to hardcoded content
        echo "${YELLOW}Note: Using fallback consent text (AGENT_CONSENT.md not found or not readable).${NC}" >&2
        echo "" >&2
        show_fallback_consent_prompt "$consent_filename" "$consent_file" "$context"
    fi
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

# Function to determine project root directory
get_project_root() {
    local fallback_dir="$1"
    
    # First, try to find git repository root
    if command -v git >/dev/null 2>&1; then
        local git_root
        if git_root="$(git rev-parse --show-toplevel 2>/dev/null)" && [ -n "$git_root" ] && [ -d "$git_root" ]; then
            echo "$git_root"
            return 0
        fi
    fi
    
    # Fallback to provided directory or current directory
    local target_dir="${fallback_dir:-$(pwd)}"
    
    # Validate the fallback directory exists
    if [ ! -d "$target_dir" ]; then
        echo "${RED}Error: Directory '$target_dir' does not exist${NC}" >&2
        return 1
    fi
    
    # Convert to absolute path for consistency
    if command -v realpath >/dev/null 2>&1; then
        realpath "$target_dir"
    elif command -v readlink >/dev/null 2>&1; then
        readlink -f "$target_dir" 2>/dev/null || echo "$target_dir"
    else
        # Fallback: convert to absolute path manually
        cd "$target_dir" && pwd
    fi
}

# Function to validate we're in the expected repository context
validate_repository_context() {
    local project_dir="$1"
    local context="$2"
    
    # For git-commit context, we must be in a git repository
    if [ "$context" = "git-commit" ]; then
        if ! git rev-parse --git-dir >/dev/null 2>&1; then
            echo "${RED}Error: git-commit context requires being in a git repository${NC}" >&2
            return 1
        fi
        
        # Ensure the project directory is within the git repository
        local git_root
        if git_root="$(git rev-parse --show-toplevel 2>/dev/null)"; then
            # Check if project_dir is under git_root (resolve any symlinks first)
            local abs_project_dir
            if command -v realpath >/dev/null 2>&1; then
                abs_project_dir="$(realpath "$project_dir")"
                git_root="$(realpath "$git_root")"
            else
                abs_project_dir="$project_dir"
            fi
            
            case "$abs_project_dir" in
                "$git_root"*)
                    # Project directory is under git root - this is expected
                    ;;
                *)
                    echo "${RED}Warning: Project directory '$project_dir' is outside git repository root '$git_root'${NC}" >&2
                    echo "${YELLOW}This may indicate an incorrect directory detection${NC}" >&2
                    ;;
            esac
        fi
    fi
    
    return 0
}

# Main function - determine context and consent file location
main() {
    local context="$1"  # "git-commit", "agent-task", or "claude"
    local fallback_dir="${2:-}"
    
    # Handle Claude-specific logic
    if [ "$context" = "claude" ]; then
        # Check if we're already in a stop hook continuation
        if [ "${CLAUDE_STOP_HOOK_ACTIVE:-false}" = "true" ]; then
            echo "Stop hook continuation detected. Skipping consent check."
            exit 0
        fi
        
        # Use CLAUDE_PROJECT_DIR as fallback if available
        fallback_dir="${CLAUDE_PROJECT_DIR:-$fallback_dir}"
        context="agent-task"  # Treat Claude as agent-task after handling specific logic
    fi
    
    # Determine the project root directory reliably
    local project_dir
    if ! project_dir="$(get_project_root "$fallback_dir")"; then
        echo "${RED}Error: Failed to determine project root directory${NC}" >&2
        exit 1
    fi
    
    # Validate repository context
    if ! validate_repository_context "$project_dir" "$context"; then
        exit 1
    fi
    
    # Determine consent file based on task context and identifiers
    local consent_file
    consent_file="$(get_consent_filename "$context" "$project_dir")"
    
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