/**
 * Unified command parsing utilities for bash tool
 * 
 * This module provides a single source of truth for parsing shell commands.
 * It supports only simple commands (no pipes, operators, or substitutions)
 * to align with the executor's capabilities.
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

  // Check for complex shell constructs that we don't support
  const complexPatterns = [
    /\|/,           // Pipes
    /&&/,           // Logical AND
    /\|\|/,         // Logical OR
    /;/,            // Command separator
    /&$/,           // Background execution
    /\$\(/,         // Command substitution $()
    /`/,            // Command substitution ``
    />/,            // Redirection >
    /</,            // Redirection <
    /\*\*/,         // Glob patterns (potentially dangerous)
    /^\s*\{|\}\s*$/,  // Brace expansion (but not braces inside quoted args)
  ];

  for (const pattern of complexPatterns) {
    if (pattern.test(trimmed)) {
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
  let escaped = false;

  for (let i = 0; i < trimmed.length; i++) {
    const char = trimmed[i];
    const nextChar = i + 1 < trimmed.length ? trimmed[i + 1] : '';
    
    if (escaped) {
      // Handle escaped characters
      current += char;
      escaped = false;
      continue;
    }

    if (char === '\\' && !inQuotes) {
      // Escape next character
      escaped = true;
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