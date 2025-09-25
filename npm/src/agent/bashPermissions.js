/**
 * Bash command permission checker
 * @module agent/bashPermissions
 */

import { DEFAULT_ALLOW_PATTERNS, DEFAULT_DENY_PATTERNS } from './bashDefaults.js';

/**
 * Parse a bash command into command and arguments
 * @param {string} command - The full command string
 * @returns {Object} Parsed command object
 */
function parseCommand(command) {
  if (!command || typeof command !== 'string') {
    return { command: '', args: [], full: command };
  }

  // Basic command parsing - split on spaces but respect quotes
  const parts = [];
  let current = '';
  let inQuotes = false;
  let quoteChar = '';

  for (let i = 0; i < command.length; i++) {
    const char = command[i];
    
    if (!inQuotes && (char === '"' || char === "'")) {
      inQuotes = true;
      quoteChar = char;
      current += char;
    } else if (inQuotes && char === quoteChar) {
      inQuotes = false;
      quoteChar = '';
      current += char;
    } else if (!inQuotes && char === ' ') {
      if (current.trim()) {
        parts.push(current.trim());
        current = '';
      }
    } else {
      current += char;
    }
  }
  
  if (current.trim()) {
    parts.push(current.trim());
  }

  const baseCommand = parts[0] || '';
  const args = parts.slice(1);

  return {
    command: baseCommand,
    args,
    full: command.trim()
  };
}

/**
 * Check if a parsed command matches a pattern
 * @param {Object} parsed - Parsed command object
 * @param {string} pattern - Pattern to match against
 * @returns {boolean} True if matches
 */
function matchesPattern(parsed, pattern) {
  const { command, args } = parsed;
  
  if (!pattern || !command) {
    return false;
  }

  // Pattern can be:
  // 1. "command" - exact command match with no args
  // 2. "command:*" - command with any args
  // 3. "command:arg1:arg2" - command with specific args
  // 4. "command:arg1:*" - command with specific first arg and any additional args

  if (!pattern.includes(':')) {
    // Exact command match with no arguments
    return command === pattern && args.length === 0;
  }

  const patternParts = pattern.split(':');
  const patternCommand = patternParts[0];
  const patternArgs = patternParts.slice(1);

  // Command must match
  if (command !== patternCommand) {
    return false;
  }

  // If pattern is "command:*", allow any args
  if (patternArgs.length === 1 && patternArgs[0] === '*') {
    return true;
  }

  // Check specific args
  for (let i = 0; i < patternArgs.length; i++) {
    const patternArg = patternArgs[i];
    
    if (patternArg === '*') {
      // Wildcard - any remaining args are allowed
      return true;
    }
    
    if (i >= args.length) {
      // Pattern expects more args than provided
      return false;
    }
    
    const actualArg = args[i];
    
    if (patternArg !== actualArg) {
      return false;
    }
  }

  // All pattern args matched, check if we have extra args
  return args.length === patternArgs.length;
}

/**
 * Check if a parsed command matches any pattern in a list
 * @param {Object} parsed - Parsed command object
 * @param {string[]} patterns - Array of patterns to check
 * @returns {boolean} True if any pattern matches
 */
function matchesAnyPattern(parsed, patterns) {
  if (!patterns || patterns.length === 0) {
    return false;
  }

  return patterns.some(pattern => matchesPattern(parsed, pattern));
}

/**
 * Bash command permission checker class
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
   * Check if a command is allowed
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

    const parsed = parseCommand(command);
    
    if (!parsed.command) {
      return {
        allowed: false,
        reason: 'No command found',
        command: command
      };
    }

    if (this.debug) {
      console.log(`[BashPermissions] Checking command: "${command}"`);
      console.log(`[BashPermissions] Parsed - Command: "${parsed.command}", Args: [${parsed.args.join(', ')}]`);
    }

    // Check deny patterns first (deny takes precedence)
    if (matchesAnyPattern(parsed, this.denyPatterns)) {
      const result = {
        allowed: false,
        reason: `Command '${command}' matches deny pattern`,
        command: command,
        parsed: parsed
      };
      
      if (this.debug) {
        console.log(`[BashPermissions] DENIED - matches deny pattern`);
      }
      
      return result;
    }

    // Check allow patterns
    if (this.allowPatterns.length > 0) {
      if (matchesAnyPattern(parsed, this.allowPatterns)) {
        const result = {
          allowed: true,
          command: command,
          parsed: parsed
        };
        
        if (this.debug) {
          console.log(`[BashPermissions] ALLOWED - matches allow pattern`);
        }
        
        return result;
      }
      
      // If allow list exists but command doesn't match, deny
      const result = {
        allowed: false,
        reason: `Command '${command}' not in allow list`,
        command: command,
        parsed: parsed
      };
      
      if (this.debug) {
        console.log(`[BashPermissions] DENIED - not in allow list`);
      }
      
      return result;
    }

    // If no allow list (shouldn't happen with defaults), allow
    const result = {
      allowed: true,
      command: command,
      parsed: parsed
    };
    
    if (this.debug) {
      console.log(`[BashPermissions] ALLOWED - no restrictions`);
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