/**
 * Advanced bash command parser that handles Unix pipes, operators, and compound commands
 * @module agent/bashCommandParser
 */

/**
 * Parse a complex bash command string into individual commands
 * Handles pipes, logical operators, command substitution, redirections, etc.
 * @param {string} commandString - The full command string
 * @returns {Object} Parsed command structure
 */
export function parseComplexCommand(commandString) {
  if (!commandString || typeof commandString !== 'string') {
    return { 
      commands: [],
      structure: null,
      isComplex: false,
      full: commandString 
    };
  }

  const trimmed = commandString.trim();
  if (!trimmed) {
    return { 
      commands: [],
      structure: null,
      isComplex: false,
      full: commandString 
    };
  }

  try {
    // Extract all individual commands from the complex command string
    const commands = extractAllCommands(trimmed);
    const isComplex = commands.length > 1;
    
    return {
      commands: commands.map(cmd => parseSimpleCommand(cmd)),
      structure: analyzeCommandStructure(trimmed), // Always analyze structure
      isComplex,
      full: trimmed,
      originalInput: commandString
    };
  } catch (error) {
    // If parsing fails, treat as potentially dangerous single command
    return {
      commands: [parseSimpleCommand(trimmed)],
      structure: { type: 'unknown', error: error.message },
      isComplex: true, // Treat unknown structures as complex for safety
      full: trimmed,
      originalInput: commandString,
      parseError: error.message
    };
  }
}

/**
 * Extract all individual commands from a complex command string
 * @param {string} commandString - Command string to parse
 * @returns {string[]} Array of individual command strings
 */
function extractAllCommands(commandString) {
  const commands = [];
  
  // Use a simple approach - split on major operators while handling quotes
  let current = '';
  let inQuotes = false;
  let quoteChar = '';
  let parenDepth = 0;
  
  for (let i = 0; i < commandString.length; i++) {
    const char = commandString[i];
    const nextChar = commandString[i + 1];
    const prevChar = i > 0 ? commandString[i - 1] : '';
    
    // Handle quotes
    if (!inQuotes && (char === '"' || char === "'")) {
      inQuotes = true;
      quoteChar = char;
      current += char;
      continue;
    } else if (inQuotes && char === quoteChar) {
      inQuotes = false;
      quoteChar = '';
      current += char;
      continue;
    }
    
    // Skip processing inside quotes
    if (inQuotes) {
      current += char;
      continue;
    }
    
    // Track parentheses depth for command substitution
    if (char === '(') {
      parenDepth++;
    } else if (char === ')') {
      parenDepth--;
    }
    
    // Handle command substitution - extract nested command
    if (char === '$' && nextChar === '(' && parenDepth === 0) {
      // Find the matching closing paren
      let subStart = i + 2;
      let subDepth = 1;
      let subEnd = -1;
      
      for (let j = subStart; j < commandString.length; j++) {
        if (commandString[j] === '(') subDepth++;
        else if (commandString[j] === ')') {
          subDepth--;
          if (subDepth === 0) {
            subEnd = j;
            break;
          }
        }
      }
      
      if (subEnd > subStart) {
        const subCommand = commandString.substring(subStart, subEnd);
        commands.push(...extractAllCommands(subCommand));
      }
      
      current += char;
      continue;
    }
    
    // Handle backtick command substitution
    if (char === '`' && parenDepth === 0) {
      let subStart = i + 1;
      let subEnd = commandString.indexOf('`', subStart);
      
      if (subEnd > subStart) {
        const subCommand = commandString.substring(subStart, subEnd);
        commands.push(...extractAllCommands(subCommand));
      }
      
      current += char;
      continue;
    }
    
    // Only split on operators at depth 0 (not inside parentheses)
    if (parenDepth === 0) {
      // Check for multi-character operators
      if ((char === '&' && nextChar === '&') || (char === '|' && nextChar === '|')) {
        commands.push(current.trim()); // Include empty segments for validation
        current = '';
        i++; // Skip next character
        continue;
      }
      
      // Check for single-character operators
      if (char === '|' || char === ';') {
        commands.push(current.trim()); // Include empty segments for validation
        current = '';
        continue;
      }
      
      // Background operator & (but not &&)
      if (char === '&' && nextChar !== '&') {
        commands.push(current.trim()); // Include empty segments for validation
        current = '';
        continue;
      }
    }
    
    current += char;
  }
  
  // Add the last command
  if (current.trim()) {
    commands.push(current.trim());
  }
  
  return commands; // Don't filter empty commands - we need them for validation
}

/**
 * Tokenize command string into meaningful tokens
 * @param {string} commandString - Command to tokenize
 * @returns {string[]} Array of tokens
 */
function tokenizeCommand(commandString) {
  const tokens = [];
  let current = '';
  let inQuotes = false;
  let quoteChar = '';
  
  for (let i = 0; i < commandString.length; i++) {
    const char = commandString[i];
    const nextChar = commandString[i + 1];
    
    // Handle quotes
    if (!inQuotes && (char === '"' || char === "'")) {
      if (current.trim()) tokens.push(current.trim());
      tokens.push(char);
      inQuotes = true;
      quoteChar = char;
      current = '';
      continue;
    } else if (inQuotes && char === quoteChar) {
      if (current.trim()) tokens.push(current.trim());
      tokens.push(char);
      inQuotes = false;
      quoteChar = '';
      current = '';
      continue;
    }
    
    // Skip operator detection inside quotes
    if (inQuotes) {
      current += char;
      continue;
    }
    
    // Handle multi-character operators
    if (char === '&' && nextChar === '&') {
      if (current.trim()) tokens.push(current.trim());
      tokens.push('&&');
      current = '';
      i++; // Skip next char
      continue;
    } else if (char === '|' && nextChar === '|') {
      if (current.trim()) tokens.push(current.trim());
      tokens.push('||');
      current = '';
      i++; // Skip next char
      continue;
    } else if (char === '>' && nextChar === '>') {
      if (current.trim()) tokens.push(current.trim());
      tokens.push('>>');
      current = '';
      i++; // Skip next char
      continue;
    } else if (char === '<' && nextChar === '<') {
      if (current.trim()) tokens.push(current.trim());
      tokens.push('<<');
      current = '';
      i++; // Skip next char
      continue;
    } else if (char === '$' && nextChar === '(') {
      if (current.trim()) tokens.push(current.trim());
      tokens.push('$(');
      current = '';
      i++; // Skip next char
      continue;
    }
    
    // Handle single-character operators and separators
    if ('|&;><(){}[]`'.includes(char)) {
      if (current.trim()) tokens.push(current.trim());
      tokens.push(char);
      current = '';
    } else {
      current += char;
    }
  }
  
  if (current.trim()) tokens.push(current.trim());
  return tokens.filter(t => t.length > 0);
}

/**
 * Check if a token is a command separator
 * @param {string} token - Token to check
 * @returns {boolean} True if token separates commands
 */
function isCommandSeparator(token) {
  return ['|', '||', '&&', ';', '&'].includes(token);
}

/**
 * Extract command from command substitution $(...)
 * @param {string[]} tokens - Tokenized command array
 * @param {number} startIndex - Index of $( token
 * @returns {string|null} Extracted command or null
 */
function extractFromSubstitution(tokens, startIndex) {
  let depth = 1;
  let command = '';
  
  for (let i = startIndex + 1; i < tokens.length && depth > 0; i++) {
    const token = tokens[i];
    if (token === '$(') {
      depth++;
    } else if (token === ')') {
      depth--;
    }
    
    if (depth > 0) {
      command += token;
    }
  }
  
  return command.trim() || null;
}

/**
 * Extract command from backtick substitution `...`
 * @param {string[]} tokens - Tokenized command array
 * @param {number} startIndex - Index of ` token
 * @returns {string|null} Extracted command or null
 */
function extractFromBackticks(tokens, startIndex) {
  let command = '';
  let foundEnd = false;
  
  for (let i = startIndex + 1; i < tokens.length; i++) {
    const token = tokens[i];
    if (token === '`') {
      foundEnd = true;
      break;
    }
    command += token;
  }
  
  return foundEnd && command.trim() ? command.trim() : null;
}

/**
 * Analyze the structure of a complex command
 * @param {string} commandString - Command to analyze
 * @returns {Object} Command structure analysis
 */
function analyzeCommandStructure(commandString) {
  const hasPipes = commandString.includes('|') && !commandString.includes('||');
  const hasLogicalAnd = commandString.includes('&&');
  const hasLogicalOr = commandString.includes('||');
  const hasBackground = commandString.includes('&') && !commandString.includes('&&');
  const hasSemicolon = commandString.includes(';');
  const hasRedirection = /[<>]/.test(commandString);
  const hasCommandSubstitution = /\$\(|\`/.test(commandString);
  
  const structure = {
    type: 'complex',
    features: [],
    hasPipes,
    hasLogicalAnd,
    hasLogicalOr,
    hasBackground,
    hasSemicolon,
    hasRedirection,
    hasCommandSubstitution
  };
  
  if (hasPipes) structure.features.push('pipes');
  if (hasLogicalAnd) structure.features.push('logical_and');
  if (hasLogicalOr) structure.features.push('logical_or');
  if (hasBackground) structure.features.push('background');
  if (hasSemicolon) structure.features.push('sequential');
  if (hasRedirection) structure.features.push('redirection');
  if (hasCommandSubstitution) structure.features.push('command_substitution');
  
  return structure;
}

/**
 * Parse a simple command (no pipes or operators) into command and args
 * @param {string} commandString - Simple command string
 * @returns {Object} Parsed simple command
 */
function parseSimpleCommand(commandString) {
  if (!commandString || typeof commandString !== 'string') {
    return { command: '', args: [], full: commandString };
  }

  const trimmed = commandString.trim();
  
  // Remove redirections for command extraction (but keep them in full)
  const withoutRedirections = trimmed.replace(/\s*[<>]+\s*[^\s]+/g, '').trim();
  
  // Basic command parsing - split on spaces but respect quotes
  const parts = [];
  let current = '';
  let inQuotes = false;
  let quoteChar = '';

  for (let i = 0; i < withoutRedirections.length; i++) {
    const char = withoutRedirections[i];
    
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
    full: trimmed
  };
}

/**
 * Get all unique commands from a parsed complex command
 * @param {Object} parsed - Result from parseComplexCommand
 * @returns {string[]} Array of unique command names
 */
export function getAllCommandNames(parsed) {
  const commands = new Set();
  
  if (parsed.commands) {
    parsed.commands.forEach(cmd => {
      if (cmd.command) {
        commands.add(cmd.command);
      }
    });
  }
  
  return Array.from(commands);
}

/**
 * Check if a parsed command contains any dangerous constructs
 * @param {Object} parsed - Result from parseComplexCommand
 * @returns {Object} Danger analysis
 */
export function analyzeDangerLevel(parsed) {
  const dangers = [];
  
  // Check for parsing errors
  if (parsed.parseError) {
    dangers.push('Command parsing failed - potentially malformed');
  }
  
  // Check for empty or malformed commands
  if (parsed.commands && parsed.commands.length > 0) {
    const hasEmptyCommands = parsed.commands.some(cmd => !cmd.command || cmd.command.trim() === '');
    if (hasEmptyCommands) {
      dangers.push('Empty or malformed commands detected');
    }
  }
  
  // Check structure features - but only flag as dangerous if there are actual issues
  if (parsed.structure) {
    // Command substitution is inherently risky
    if (parsed.structure.hasCommandSubstitution || (parsed.structure.features && parsed.structure.features.includes('command_substitution'))) {
      dangers.push('Command substitution can execute arbitrary commands');
    }
    
    // Background execution is risky
    if (parsed.structure.hasBackground || (parsed.structure.features && parsed.structure.features.includes('background'))) {
      dangers.push('Background execution can hide malicious processes');
    }
    
    // Only flag pipes/redirections as dangerous if we have structural issues
    // The permission system will handle the actual command validation
  }
  
  // Check for unmatched quotes and parentheses in full command
  if (parsed.full && typeof parsed.full === 'string') {
    let singleQuotes = 0;
    let doubleQuotes = 0;
    let openParens = 0;
    let closeParens = 0;
    let inQuotes = false;
    let quoteChar = '';
    
    for (let i = 0; i < parsed.full.length; i++) {
      const char = parsed.full[i];
      const prevChar = i > 0 ? parsed.full[i - 1] : '';
      
      // Skip escaped characters
      if (prevChar === '\\') {
        continue;
      }
      
      if (!inQuotes && (char === "'" || char === '"')) {
        inQuotes = true;
        quoteChar = char;
        // Count all quote characters
        if (char === "'") singleQuotes++;
        else doubleQuotes++;
      } else if (inQuotes && char === quoteChar) {
        inQuotes = false;
        quoteChar = '';
        // Count closing quote too
        if (char === "'") singleQuotes++;
        else doubleQuotes++;
      } else if (!inQuotes) {
        // Count parentheses only outside quotes
        if (char === '(') openParens++;
        if (char === ')') closeParens++;
      }
    }
    
    if (singleQuotes % 2 !== 0 || doubleQuotes % 2 !== 0) {
      dangers.push('Unmatched quotes detected - potentially malformed command');
    }
    
    if (openParens !== closeParens) {
      dangers.push('Unmatched parentheses detected - potentially malformed command substitution');
    }
    
    // Check for malformed command substitution patterns
    if (parsed.full.includes('$(') && openParens > closeParens) {
      dangers.push('Incomplete command substitution detected');
    }
  }
  
  return {
    dangerLevel: dangers.length > 0 ? 'high' : 'low',
    dangers,
    commandCount: parsed.commands ? parsed.commands.length : 0
  };
}