/**
 * Simplified bash command permission checker (aligned with executor capabilities)
 * @module agent/bashPermissions
 */

import { DEFAULT_ALLOW_PATTERNS, DEFAULT_DENY_PATTERNS } from './bashDefaults.js';
import { parseCommand, isComplexCommand } from './bashCommandUtils.js';

/**
 * Check if a pattern matches a parsed command
 * @param {Object} parsedCommand - Parsed command with command and args
 * @param {string} pattern - Pattern to match against (e.g., "git:status", "npm:*")
 * @returns {boolean} True if pattern matches
 */
function matchesPattern(parsedCommand, pattern) {
  if (!parsedCommand || !pattern) return false;
  
  const { command, args } = parsedCommand;
  if (!command) return false;
  
  // Split pattern into parts separated by ':'
  const patternParts = pattern.split(':');
  const commandName = patternParts[0];
  
  // Check if command name matches (with wildcard support)
  if (commandName === '*') {
    // Wildcard matches any command
    return true;
  } else if (commandName !== command) {
    // Command name doesn't match
    return false;
  }
  
  // If only command name specified, it matches
  if (patternParts.length === 1) {
    return true;
  }
  
  // Check arguments
  for (let i = 1; i < patternParts.length; i++) {
    const patternArg = patternParts[i];
    const argIndex = i - 1;
    
    if (patternArg === '*') {
      // Wildcard matches any argument (or no argument)
      continue;
    }
    
    if (argIndex >= args.length) {
      // Not enough arguments to match pattern
      return false;
    }
    
    const actualArg = args[argIndex];
    if (patternArg !== actualArg) {
      // Argument doesn't match
      return false;
    }
  }
  
  return true;
}

/**
 * Check if any pattern in a list matches the command
 * @param {Object} parsedCommand - Parsed command
 * @param {string[]} patterns - Array of patterns to check
 * @returns {boolean} True if any pattern matches
 */
function matchesAnyPattern(parsedCommand, patterns) {
  if (!patterns || patterns.length === 0) return false;
  return patterns.some(pattern => matchesPattern(parsedCommand, pattern));
}

/**
 * Bash permission checker for simple commands only
 * Rejects complex shell constructs for security and alignment with executor
 */
export class BashPermissionChecker {
  /**
   * Create a permission checker
   * @param {Object} config - Configuration options
   * @param {string[]} [config.allow] - Additional allow patterns
   * @param {string[]} [config.deny] - Additional deny patterns
   * @param {boolean} [config.disableDefaultAllow] - Disable default allow list
   * @param {boolean} [config.disableDefaultDeny] - Disable default deny list
   * @param {boolean} [config.debug] - Enable debug logging
   */
  constructor(config = {}) {
    this.debug = config.debug || false;
    
    // Build allow patterns
    this.allowPatterns = [];
    if (!config.disableDefaultAllow) {
      this.allowPatterns.push(...DEFAULT_ALLOW_PATTERNS);
      if (this.debug) {
        console.log(`[BashPermissions] Added ${DEFAULT_ALLOW_PATTERNS.length} default allow patterns`);
      }
    }
    if (config.allow && Array.isArray(config.allow)) {
      this.allowPatterns.push(...config.allow);
      if (this.debug) {
        console.log(`[BashPermissions] Added ${config.allow.length} custom allow patterns:`, config.allow);
      }
    }

    // Build deny patterns
    this.denyPatterns = [];
    if (!config.disableDefaultDeny) {
      this.denyPatterns.push(...DEFAULT_DENY_PATTERNS);
      if (this.debug) {
        console.log(`[BashPermissions] Added ${DEFAULT_DENY_PATTERNS.length} default deny patterns`);
      }
    }
    if (config.deny && Array.isArray(config.deny)) {
      this.denyPatterns.push(...config.deny);
      if (this.debug) {
        console.log(`[BashPermissions] Added ${config.deny.length} custom deny patterns:`, config.deny);
      }
    }

    if (this.debug) {
      console.log(`[BashPermissions] Total patterns - Allow: ${this.allowPatterns.length}, Deny: ${this.denyPatterns.length}`);
    }
  }

  /**
   * Check if a simple command is allowed (rejects complex commands for security)
   * @param {string} command - Command to check
   * @returns {Object} Permission result
   */
  check(command) {
    if (!command || typeof command !== 'string') {
      return {
        allowed: false,
        reason: 'Invalid or empty command',
        command: command
      };
    }

    // First check if this is a complex command - reject immediately for security
    if (isComplexCommand(command)) {
      return {
        allowed: false,
        reason: 'Complex shell commands with pipes, operators, or redirections are not supported for security reasons',
        command: command,
        isComplex: true
      };
    }

    // Parse the simple command
    const parsed = parseCommand(command);
    
    if (parsed.error) {
      return {
        allowed: false,
        reason: parsed.error,
        command: command
      };
    }

    if (!parsed.command) {
      return {
        allowed: false,
        reason: 'No valid command found',
        command: command
      };
    }

    if (this.debug) {
      console.log(`[BashPermissions] Checking simple command: "${command}"`);
      console.log(`[BashPermissions] Parsed: ${parsed.command} with args: [${parsed.args.join(', ')}]`);
    }

    // Check deny patterns first (deny takes precedence)
    if (matchesAnyPattern(parsed, this.denyPatterns)) {
      const matchedPatterns = this.denyPatterns.filter(pattern => matchesPattern(parsed, pattern));
      return {
        allowed: false,
        reason: `Command matches deny pattern: ${matchedPatterns[0]}`,
        command: command,
        parsed: parsed,
        matchedPatterns: matchedPatterns
      };
    }

    // Check allow patterns
    if (this.allowPatterns.length > 0) {
      if (!matchesAnyPattern(parsed, this.allowPatterns)) {
        return {
          allowed: false,
          reason: 'Command not in allow list',
          command: command,
          parsed: parsed
        };
      }
    }

    // Command passed all checks
    const result = {
      allowed: true,
      command: command,
      parsed: parsed,
      isComplex: false
    };
    
    if (this.debug) {
      console.log(`[BashPermissions] ALLOWED - command passed all checks`);
    }
    
    return result;
  }

  /**
   * Get configuration summary
   * @returns {Object} Configuration info
   */
  getConfig() {
    return {
      allowPatterns: this.allowPatterns.length,
      denyPatterns: this.denyPatterns.length,
      totalPatterns: this.allowPatterns.length + this.denyPatterns.length
    };
  }
}

// Export utility functions for testing
export { parseCommand, matchesPattern, matchesAnyPattern };