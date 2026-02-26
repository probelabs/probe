/**
 * Unified command parsing utilities for bash tool
 *
 * This module provides a single source of truth for parsing shell commands.
 * It supports only simple commands (no pipes, operators, or substitutions)
 * to align with the executor's capabilities.
 *
 * ## Escape Handling Architecture
 *
 * There are THREE different escape handling behaviors in the bash permission system,
 * each serving a distinct purpose:
 *
 * 1. **stripQuotedContent()** (in parseSimpleCommand): SKIPS both backslash AND next char
 *    - Purpose: Detect operators (|, &&, ||) that exist OUTSIDE quoted strings
 *    - Output is never used for execution, only for operator detection
 *    - Example: `echo "a && b"` → strips quoted content → no `&&` detected outside quotes
 *
 * 2. **parseSimpleCommand()** main loop: STRIPS backslash, KEEPS escaped char
 *    - Purpose: Extract actual argument values that would be passed to the command
 *    - Matches bash behavior where `\"` inside double quotes becomes `"`
 *    - Example: `echo "he said \"hi\""` → args: ['he said "hi"']
 *
 * 3. **_splitComplexCommand()** (in bashPermissions.js): PRESERVES both backslash AND next char
 *    - Purpose: Split complex commands by operators while preserving escape sequences
 *    - Output is passed to parseCommand() which will then interpret the escapes
 *    - Example: `echo "test\" && b" && cmd` → components passed to parseCommand for final parsing
 *
 * This design ensures:
 * - Commands with operators inside quotes (e.g., `echo "a && b"`) are NOT incorrectly
 *   flagged as complex commands
 * - Escaped quotes (e.g., `\"`) don't prematurely end quoted sections
 * - Each component gets proper escape interpretation in the final parsing step
 *
 * @module bashCommandUtils
 */

/**
 * Parse a simple shell command into command and arguments
 * Properly handles quoted arguments and strips quotes
 * Rejects complex shell constructs for security and consistency
 * 
 * @param {string} command - Command string to parse
 * @returns {Object} Parse result with command, args, and validation info
 */
export function parseSimpleCommand(command) {
  if (!command || typeof command !== 'string') {
    return {
      success: false,
      error: 'Command must be a non-empty string',
      command: null,
      args: [],
      isComplex: false
    };
  }

  const trimmed = command.trim();
  if (!trimmed) {
    return {
      success: false,
      error: 'Command cannot be empty',
      command: null,
      args: [],
      isComplex: false
    };
  }

  // Strip quoted content before checking for complex operators
  // This prevents detecting operators inside quotes (e.g., echo "a && b")
  //
  // IMPORTANT: This function is ONLY used to detect operators, NOT for argument parsing.
  // It REMOVES both backslash AND escaped character from the output, unlike parseSimpleCommand
  // which interprets escapes. This is intentional - we only care about finding operators
  // that exist outside of quoted strings. See module header for architecture details.
  const stripQuotedContent = (str) => {
    let result = '';
    let inQuotes = false;
    let quoteChar = '';

    for (let i = 0; i < str.length; i++) {
      const char = str[i];
      const nextChar = str[i + 1];

      // Handle escape sequences outside quotes - skip both chars (not operators)
      if (!inQuotes && char === '\\' && nextChar !== undefined) {
        // Skip the backslash and next char - they can't be operators
        i++;
        continue;
      }

      // Handle escape sequences inside double quotes
      // In bash, only ", $, `, \, and newline are escapable in double quotes
      if (inQuotes && quoteChar === '"' && char === '\\' && nextChar !== undefined) {
        // Skip both the backslash and the escaped character (stays inside quotes)
        i++;
        continue;
      }

      // Start of quoted section
      if (!inQuotes && (char === '"' || char === "'")) {
        inQuotes = true;
        quoteChar = char;
        continue;
      }

      // End of quoted section
      if (inQuotes && char === quoteChar) {
        inQuotes = false;
        quoteChar = '';
        continue;
      }

      // Only add characters that are outside quotes
      if (!inQuotes) {
        result += char;
      }
    }

    return result;
  };

  // Check for complex shell constructs that we don't support
  // Use stripped version (without quoted content) for operator detection
  const strippedForOperators = stripQuotedContent(trimmed);

  const complexPatterns = [
    /\|/,           // Pipes
    /&&/,           // Logical AND
    /\|\|/,         // Logical OR
    /(?<!\\);/,     // Command separator (but not escaped \;)
    /\n/,           // Newline command separator (multi-line commands)
    /\r/,           // Carriage return (CRLF line endings)
    /&$/,           // Background execution
    /\$\(/,         // Command substitution $()
    /`/,            // Command substitution ``
    />/,            // Redirection >
    /</,            // Redirection <
    /\*\*/,         // Glob patterns (potentially dangerous)
    /^\s*\{.*,.*\}|\{.*\.\.\.*\}/,  // Brace expansion like {a,b} or {1..10} (but not find {} placeholders)
  ];

  for (const pattern of complexPatterns) {
    if (pattern.test(strippedForOperators)) {
      return {
        success: false,
        error: 'Complex shell commands with pipes, operators, or redirections are not supported for security reasons',
        command: null,
        args: [],
        isComplex: true,
        detected: pattern.toString()
      };
    }
  }

  // Parse simple command with proper quote handling
  const args = [];
  let current = '';
  let inQuotes = false;
  let quoteChar = '';

  for (let i = 0; i < trimmed.length; i++) {
    const char = trimmed[i];
    const nextChar = i + 1 < trimmed.length ? trimmed[i + 1] : '';

    // Handle escapes outside quotes
    if (!inQuotes && char === '\\' && nextChar) {
      // Add the escaped character (skip the backslash)
      current += nextChar;
      i++; // Skip next character
      continue;
    }

    // Handle escapes inside double quotes (single quotes don't process escapes)
    if (inQuotes && quoteChar === '"' && char === '\\' && nextChar) {
      // In double quotes, backslash escapes certain characters
      // Add the escaped character to current
      current += nextChar;
      i++; // Skip next character
      continue;
    }

    if (!inQuotes && (char === '"' || char === "'")) {
      // Start quoted section - don't include quote char
      inQuotes = true;
      quoteChar = char;
    } else if (inQuotes && char === quoteChar) {
      // End quoted section - don't include quote char
      inQuotes = false;
      quoteChar = '';
    } else if (!inQuotes && char === ' ') {
      // Space outside quotes - end current argument
      if (current.trim()) {
        args.push(current.trim());
        current = '';
      }
    } else {
      // Regular character - add to current argument
      current += char;
    }
  }
  
  // Add final argument if exists
  if (current.trim()) {
    args.push(current.trim());
  }

  // Check for unclosed quotes
  if (inQuotes) {
    return {
      success: false,
      error: `Unclosed quote in command: ${quoteChar}`,
      command: null,
      args: [],
      isComplex: false
    };
  }

  if (args.length === 0) {
    return {
      success: false,
      error: 'No command found after parsing',
      command: null,
      args: [],
      isComplex: false
    };
  }

  const [baseCommand, ...commandArgs] = args;

  return {
    success: true,
    error: null,
    command: baseCommand,
    args: commandArgs,
    fullArgs: args,
    isComplex: false,
    original: command
  };
}

/**
 * Check if a command contains complex shell constructs
 * @param {string} command - Command to check
 * @returns {boolean} True if command is complex
 */
export function isComplexCommand(command) {
  const result = parseSimpleCommand(command);
  return result.isComplex;
}

/**
 * Check if a pattern is a complex pattern (contains shell operators)
 * Complex patterns are used to match full command strings including operators
 * @param {string} pattern - Pattern to check
 * @returns {boolean} True if pattern contains shell operators
 */
export function isComplexPattern(pattern) {
  if (!pattern || typeof pattern !== 'string') return false;

  // Check for operators in the pattern (aligned with complexPatterns in parseSimpleCommand)
  const operatorPatterns = [
    /\|/,           // Pipes
    /&&/,           // Logical AND
    /\|\|/,         // Logical OR
    /;/,            // Command separator
    /\n/,           // Newline command separator
    /&$/,           // Background execution
    /\$\(/,         // Command substitution $()
    /`/,            // Command substitution ``
    />/,            // Redirection >
    /</,            // Redirection <
  ];

  return operatorPatterns.some(p => p.test(pattern));
}

/**
 * Convert a glob-style pattern to regex for matching
 * Supports * as wildcard (matches any characters except operators)
 * @param {string} pattern - Glob pattern
 * @returns {RegExp} Compiled regex
 */
function globToRegex(pattern) {
  // Escape regex special characters except *
  let escaped = pattern.replace(/[.+?^${}()|[\]\\]/g, '\\$&');
  // Convert * to .*? (non-greedy match)
  escaped = escaped.replace(/\*/g, '.*?');
  // Make it match the full string
  return new RegExp('^' + escaped + '$', 'i');
}

/**
 * Match a command string against a complex pattern
 * Complex patterns use glob-style wildcards (*) for matching
 * @param {string} command - Full command string
 * @param {string} pattern - Complex pattern with wildcards
 * @returns {boolean} True if command matches the pattern
 */
export function matchesComplexPattern(command, pattern) {
  if (!command || !pattern) return false;

  // Normalize whitespace
  const normalizedCommand = command.trim().replace(/\s+/g, ' ');
  const normalizedPattern = pattern.trim().replace(/\s+/g, ' ');

  try {
    const regex = globToRegex(normalizedPattern);
    return regex.test(normalizedCommand);
  } catch (e) {
    // If regex fails, fall back to exact match
    return normalizedCommand === normalizedPattern;
  }
}

/**
 * Legacy compatibility function - parses command for permission checking
 * @param {string} command - Command to parse
 * @returns {Object} Parse result compatible with existing permission checker
 */
export function parseCommand(command) {
  const result = parseSimpleCommand(command);
  
  if (!result.success) {
    return {
      command: '',
      args: [],
      error: result.error,
      isComplex: result.isComplex
    };
  }

  return {
    command: result.command,
    args: result.args,
    error: null,
    isComplex: result.isComplex
  };
}

/**
 * Parse command for execution - returns array format expected by spawn()
 * @param {string} command - Command to parse  
 * @returns {string[]|null} Array of [command, ...args] or null if invalid
 */
export function parseCommandForExecution(command) {
  const result = parseSimpleCommand(command);
  
  if (!result.success) {
    return null;
  }

  return result.fullArgs;
}