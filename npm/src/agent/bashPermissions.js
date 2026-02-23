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
   *
   * Priority order (highest to lowest):
   *   1. Custom deny  — always blocks (user explicitly blocked it)
   *   2. Custom allow — overrides default deny (user explicitly allowed it)
   *   3. Default deny — blocks by default
   *   4. Allow list   — allows recognized safe commands
   *
   * This means `--bash-allow "git:push"` overrides the default deny for git:push
   * without requiring `--no-default-bash-deny`.
   *
   * @param {Object} config - Configuration options
   * @param {string[]} [config.allow] - Additional allow patterns (override default deny)
   * @param {string[]} [config.deny] - Additional deny patterns (always win)
   * @param {boolean} [config.disableDefaultAllow] - Disable default allow list
   * @param {boolean} [config.disableDefaultDeny] - Disable default deny list
   * @param {boolean} [config.debug] - Enable debug logging
   * @param {Object} [config.tracer] - Optional tracer for telemetry
   */
  constructor(config = {}) {
    this.debug = config.debug || false;
    this.tracer = config.tracer || null;

    // Separate default and custom patterns for priority-based resolution
    this.defaultAllowPatterns = config.disableDefaultAllow ? [] : [...DEFAULT_ALLOW_PATTERNS];
    this.customAllowPatterns = (config.allow && Array.isArray(config.allow)) ? [...config.allow] : [];
    this.allowPatterns = [...this.defaultAllowPatterns, ...this.customAllowPatterns];

    this.defaultDenyPatterns = config.disableDefaultDeny ? [] : [...DEFAULT_DENY_PATTERNS];
    this.customDenyPatterns = (config.deny && Array.isArray(config.deny)) ? [...config.deny] : [];
    this.denyPatterns = [...this.defaultDenyPatterns, ...this.customDenyPatterns];

    if (this.debug) {
      console.log(`[BashPermissions] Default allow: ${this.defaultAllowPatterns.length}, Custom allow: ${this.customAllowPatterns.length}`);
      console.log(`[BashPermissions] Default deny: ${this.defaultDenyPatterns.length}, Custom deny: ${this.customDenyPatterns.length}`);
      console.log(`[BashPermissions] Total patterns - Allow: ${this.allowPatterns.length}, Deny: ${this.denyPatterns.length}`);
    }

    // Record initialization event
    this.recordBashEvent('permissions.initialized', {
      allowPatternCount: this.allowPatterns.length,
      denyPatternCount: this.denyPatterns.length,
      hasCustomAllowPatterns: this.customAllowPatterns.length > 0,
      hasCustomDenyPatterns: this.customDenyPatterns.length > 0,
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

    // Priority-based permission check:
    // 1. Custom deny always wins
    // 2. Custom allow overrides default deny
    // 3. Default deny blocks
    // 4. Allow list permits

    // Step 1: Custom deny always wins
    if (matchesAnyPattern(parsed, this.customDenyPatterns)) {
      const matchedPatterns = this.customDenyPatterns.filter(pattern => matchesPattern(parsed, pattern));
      if (this.debug) {
        console.log(`[BashPermissions] DENIED - matches custom deny pattern: ${matchedPatterns[0]}`);
      }
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
        isComplex: false,
        isCustomDeny: true
      });
      return result;
    }

    // Step 2: Custom allow overrides default deny
    const matchesCustomAllow = matchesAnyPattern(parsed, this.customAllowPatterns);

    // Step 3: Default deny (skipped if custom allow matches)
    if (!matchesCustomAllow && matchesAnyPattern(parsed, this.defaultDenyPatterns)) {
      const matchedPatterns = this.defaultDenyPatterns.filter(pattern => matchesPattern(parsed, pattern));
      if (this.debug) {
        console.log(`[BashPermissions] DENIED - matches default deny pattern: ${matchedPatterns[0]}`);
      }
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
        isComplex: false,
        isCustomDeny: false
      });
      return result;
    }

    // Step 4: Check allow patterns
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
      isComplex: false,
      overriddenDeny: matchesCustomAllow && matchesAnyPattern(parsed, this.defaultDenyPatterns)
    };

    if (this.debug) {
      if (result.overriddenDeny) {
        console.log(`[BashPermissions] ALLOWED - custom allow overrides default deny`);
      } else {
        console.log(`[BashPermissions] ALLOWED - command passed all checks`);
      }
    }

    this.recordBashEvent('permission.allowed', {
      command,
      parsedCommand: parsed.command,
      isComplex: false,
      overriddenDeny: result.overriddenDeny || false
    });

    return result;
  }

  /**
   * Split a complex command into component commands by operators
   *
   * ## Escape Handling (Security-Critical)
   *
   * This function intentionally PRESERVES escape sequences (both backslash AND
   * escaped character) in the output. This is step 1 of a 2-step parsing process:
   *
   * 1. _splitComplexCommand: Splits by operators, PRESERVES escapes → `echo "test\" && b"`
   * 2. parseCommand: Interprets escapes in each component → args: ['test" && b']
   *
   * This differs from stripQuotedContent() in bashCommandUtils.js which REMOVES
   * escapes entirely (for operator detection only).
   *
   * The security rationale: if we stripped escapes here, `\"` would become `"`,
   * potentially causing incorrect quote boundary detection and allowing operator
   * injection. By preserving escapes, parseCommand() can correctly interpret them.
   *
   * See bashCommandUtils.js module header for the full escape handling architecture.
   *
   * @private
   * @param {string} command - Complex command to split
   * @returns {string[]} Array of component commands (with escapes preserved)
   */
  _splitComplexCommand(command) {
    // Split by &&, ||, and | operators while respecting quotes and escape sequences
    // IMPORTANT: Preserves backslashes so parseCommand() can interpret them correctly
    const components = [];
    let current = '';
    let inQuotes = false;
    let quoteChar = '';
    let i = 0;

    while (i < command.length) {
      const char = command[i];
      const nextChar = command[i + 1] || '';

      // Handle escape sequences outside quotes
      if (!inQuotes && char === '\\') {
        // Keep the backslash and the next character
        current += char;
        if (nextChar) {
          current += nextChar;
          i += 2;
        } else {
          i++;
        }
        continue;
      }

      // Handle escape sequences inside double quotes (single quotes don't support escaping)
      if (inQuotes && quoteChar === '"' && char === '\\' && nextChar) {
        // Keep both the backslash and the escaped character
        current += char + nextChar;
        i += 2;
        continue;
      }

      // Start of quoted section
      if (!inQuotes && (char === '"' || char === "'")) {
        inQuotes = true;
        quoteChar = char;
        current += char;
        i++;
        continue;
      }

      // End of quoted section
      if (inQuotes && char === quoteChar) {
        inQuotes = false;
        quoteChar = '';
        current += char;
        i++;
        continue;
      }

      // Check for operators only outside quotes
      if (!inQuotes) {
        // Check for && or ||
        if ((char === '&' && nextChar === '&') || (char === '|' && nextChar === '|')) {
          if (current.trim()) {
            components.push(current.trim());
          }
          current = '';
          i += 2; // Skip both characters
          continue;
        }
        // Check for single pipe |
        if (char === '|') {
          if (current.trim()) {
            components.push(current.trim());
          }
          current = '';
          i++;
          continue;
        }
      }

      current += char;
      i++;
    }

    // Add the last component
    if (current.trim()) {
      components.push(current.trim());
    }

    return components;
  }

  /**
   * Check a complex command against complex patterns in allow/deny lists
   * Also supports auto-allowing commands where all components are individually allowed
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

    // No explicit complex pattern matched - try component-based evaluation
    // Split the command by &&, ||, and | operators and check each component
    const components = this._splitComplexCommand(command);

    if (this.debug) {
      console.log(`[BashPermissions] Checking ${components.length} command components: ${JSON.stringify(components)}`);
    }

    if (components.length > 1) {
      // Check each component individually
      const componentResults = [];
      let allAllowed = true;
      let deniedComponent = null;
      let deniedReason = null;

      for (const component of components) {
        // Parse the component as a simple command
        const parsed = parseCommand(component);

        if (parsed.error || parsed.isComplex) {
          // Component itself is complex or has an error - can't auto-allow
          if (this.debug) {
            console.log(`[BashPermissions] Component "${component}" is complex or has error: ${parsed.error}`);
          }
          allAllowed = false;
          deniedComponent = component;
          deniedReason = parsed.error || 'Component contains nested complex constructs';
          break;
        }

        // Check using same priority logic as simple commands:
        // 1. Custom deny always wins
        if (matchesAnyPattern(parsed, this.customDenyPatterns)) {
          if (this.debug) {
            console.log(`[BashPermissions] Component "${component}" matches custom deny pattern`);
          }
          allAllowed = false;
          deniedComponent = component;
          deniedReason = 'Component matches deny pattern';
          break;
        }

        // 2. Custom allow overrides default deny
        const componentMatchesCustomAllow = matchesAnyPattern(parsed, this.customAllowPatterns);

        // 3. Default deny (skipped if custom allow matches)
        if (!componentMatchesCustomAllow && matchesAnyPattern(parsed, this.defaultDenyPatterns)) {
          if (this.debug) {
            console.log(`[BashPermissions] Component "${component}" matches default deny pattern`);
          }
          allAllowed = false;
          deniedComponent = component;
          deniedReason = 'Component matches deny pattern';
          break;
        }

        // 4. Check allow patterns
        if (!matchesAnyPattern(parsed, this.allowPatterns)) {
          if (this.debug) {
            console.log(`[BashPermissions] Component "${component}" not in allow list`);
          }
          allAllowed = false;
          deniedComponent = component;
          deniedReason = 'Component not in allow list';
          break;
        }

        componentResults.push({ component, parsed, allowed: true });
      }

      if (allAllowed) {
        if (this.debug) {
          console.log(`[BashPermissions] ALLOWED - all ${components.length} components passed individual checks`);
        }
        const result = {
          allowed: true,
          command: command,
          isComplex: true,
          allowedByComponents: true,
          components: componentResults
        };
        this.recordBashEvent('permission.allowed', {
          command,
          isComplex: true,
          allowedByComponents: true,
          componentCount: components.length
        });
        return result;
      } else {
        if (this.debug) {
          console.log(`[BashPermissions] DENIED - component "${deniedComponent}" failed: ${deniedReason}`);
        }
        const result = {
          allowed: false,
          reason: `Component "${deniedComponent}" not allowed: ${deniedReason}`,
          command: command,
          isComplex: true,
          failedComponent: deniedComponent
        };
        this.recordBashEvent('permission.denied', {
          command,
          reason: 'component_not_allowed',
          failedComponent: deniedComponent,
          componentReason: deniedReason,
          isComplex: true
        });
        return result;
      }
    }

    // No matching complex pattern found and couldn't split into components - reject
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
      customAllowPatterns: this.customAllowPatterns.length,
      customDenyPatterns: this.customDenyPatterns.length,
      defaultAllowPatterns: this.defaultAllowPatterns.length,
      defaultDenyPatterns: this.defaultDenyPatterns.length,
      totalPatterns: this.allowPatterns.length + this.denyPatterns.length
    };
  }
}

// Export utility functions for testing
export { parseCommand, matchesPattern, matchesAnyPattern };