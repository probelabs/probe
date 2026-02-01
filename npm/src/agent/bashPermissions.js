/**
 * Simplified bash command permission checker (aligned with executor capabilities)
 * @module agent/bashPermissions
 */

import { DEFAULT_ALLOW_PATTERNS, DEFAULT_DENY_PATTERNS } from './bashDefaults.js';
import { parseCommand, isComplexCommand, isComplexPattern, matchesComplexPattern } from './bashCommandUtils.js';

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
   * @param {Object} [config.tracer] - Optional tracer for telemetry
   */
  constructor(config = {}) {
    this.debug = config.debug || false;
    this.tracer = config.tracer || null;
    
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

    // Record initialization event
    this.recordBashEvent('permissions.initialized', {
      allowPatternCount: this.allowPatterns.length,
      denyPatternCount: this.denyPatterns.length,
      hasCustomAllowPatterns: !!(config.allow && config.allow.length > 0),
      hasCustomDenyPatterns: !!(config.deny && config.deny.length > 0),
      disableDefaultAllow: !!config.disableDefaultAllow,
      disableDefaultDeny: !!config.disableDefaultDeny
    });
  }

  /**
   * Record a bash telemetry event if tracer is available
   * @param {string} eventType - Event type (e.g., 'permission.checked', 'permission.denied')
   * @param {Object} data - Event data
   */
  recordBashEvent(eventType, data = {}) {
    if (this.tracer && typeof this.tracer.recordBashEvent === 'function') {
      this.tracer.recordBashEvent(eventType, data);
    }
  }

  /**
   * Check if a simple command is allowed (complex commands allowed if they match patterns)
   * @param {string} command - Command to check
   * @returns {Object} Permission result
   */
  check(command) {
    if (!command || typeof command !== 'string') {
      const result = {
        allowed: false,
        reason: 'Invalid or empty command',
        command: command
      };
      this.recordBashEvent('permission.denied', {
        command: String(command),
        reason: result.reason,
        isComplex: false
      });
      return result;
    }

    // Check if this is a complex command
    const commandIsComplex = isComplexCommand(command);

    if (commandIsComplex) {
      // For complex commands, check against complex patterns in allow/deny lists
      return this._checkComplexCommand(command);
    }

    // Parse the simple command
    const parsed = parseCommand(command);

    if (parsed.error) {
      const result = {
        allowed: false,
        reason: parsed.error,
        command: command
      };
      this.recordBashEvent('permission.denied', {
        command,
        reason: result.reason,
        isComplex: false,
        parseError: true
      });
      return result;
    }

    if (!parsed.command) {
      const result = {
        allowed: false,
        reason: 'No valid command found',
        command: command
      };
      this.recordBashEvent('permission.denied', {
        command,
        reason: result.reason,
        isComplex: false
      });
      return result;
    }

    if (this.debug) {
      console.log(`[BashPermissions] Checking simple command: "${command}"`);
      console.log(`[BashPermissions] Parsed: ${parsed.command} with args: [${parsed.args.join(', ')}]`);
    }

    // Check deny patterns first (deny takes precedence)
    if (matchesAnyPattern(parsed, this.denyPatterns)) {
      const matchedPatterns = this.denyPatterns.filter(pattern => matchesPattern(parsed, pattern));
      const result = {
        allowed: false,
        reason: `Command matches deny pattern: ${matchedPatterns[0]}`,
        command: command,
        parsed: parsed,
        matchedPatterns: matchedPatterns
      };
      this.recordBashEvent('permission.denied', {
        command,
        parsedCommand: parsed.command,
        reason: 'matches_deny_pattern',
        matchedPattern: matchedPatterns[0],
        isComplex: false
      });
      return result;
    }

    // Check allow patterns
    if (this.allowPatterns.length > 0) {
      if (!matchesAnyPattern(parsed, this.allowPatterns)) {
        const result = {
          allowed: false,
          reason: 'Command not in allow list',
          command: command,
          parsed: parsed
        };
        this.recordBashEvent('permission.denied', {
          command,
          parsedCommand: parsed.command,
          reason: 'not_in_allow_list',
          isComplex: false
        });
        return result;
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

    this.recordBashEvent('permission.allowed', {
      command,
      parsedCommand: parsed.command,
      isComplex: false
    });

    return result;
  }

  /**
   * Check a complex command against complex patterns in allow/deny lists
   * @private
   * @param {string} command - Complex command to check
   * @returns {Object} Permission result
   */
  _checkComplexCommand(command) {
    if (this.debug) {
      console.log(`[BashPermissions] Checking complex command: "${command}"`);
    }

    // Get complex patterns from allow and deny lists
    const complexAllowPatterns = this.allowPatterns.filter(p => isComplexPattern(p));
    const complexDenyPatterns = this.denyPatterns.filter(p => isComplexPattern(p));

    if (this.debug) {
      console.log(`[BashPermissions] Complex allow patterns: ${complexAllowPatterns.length}`);
      console.log(`[BashPermissions] Complex deny patterns: ${complexDenyPatterns.length}`);
    }

    // Check deny patterns first (deny takes precedence)
    for (const pattern of complexDenyPatterns) {
      if (matchesComplexPattern(command, pattern)) {
        if (this.debug) {
          console.log(`[BashPermissions] DENIED - matches complex deny pattern: ${pattern}`);
        }
        const result = {
          allowed: false,
          reason: `Command matches deny pattern: ${pattern}`,
          command: command,
          isComplex: true,
          matchedPatterns: [pattern]
        };
        this.recordBashEvent('permission.denied', {
          command,
          reason: 'matches_deny_pattern',
          matchedPattern: pattern,
          isComplex: true
        });
        return result;
      }
    }

    // Check allow patterns
    for (const pattern of complexAllowPatterns) {
      if (matchesComplexPattern(command, pattern)) {
        if (this.debug) {
          console.log(`[BashPermissions] ALLOWED - matches complex allow pattern: ${pattern}`);
        }
        const result = {
          allowed: true,
          command: command,
          isComplex: true,
          matchedPattern: pattern
        };
        this.recordBashEvent('permission.allowed', {
          command,
          matchedPattern: pattern,
          isComplex: true
        });
        return result;
      }
    }

    // No matching complex pattern found - reject complex command
    if (this.debug) {
      console.log(`[BashPermissions] DENIED - no matching complex pattern found`);
    }
    this.recordBashEvent('permission.denied', {
      command,
      reason: 'no_matching_complex_pattern',
      isComplex: true
    });
    return {
      allowed: false,
      reason: 'Complex shell commands require explicit allow patterns (e.g., "cd * && git *")',
      command: command,
      isComplex: true
    };
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