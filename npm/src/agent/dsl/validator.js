/**
 * DSL Validator - AST whitelist validation for LLM-generated code.
 *
 * Parses code with Acorn and walks the AST, rejecting any node type
 * not in the whitelist. This is an allow-list approach — unknown syntax
 * is rejected by default.
 */

import * as acorn from 'acorn';
import * as walk from 'acorn-walk';

/**
 * Convert a character offset to line and column numbers.
 * @param {string} code - The source code
 * @param {number} offset - Character offset
 * @returns {{ line: number, column: number }}
 */
function offsetToLineColumn(code, offset) {
  const lines = code.split('\n');
  let pos = 0;
  for (let i = 0; i < lines.length; i++) {
    const lineLength = lines[i].length + 1; // +1 for newline
    if (pos + lineLength > offset) {
      return { line: i + 1, column: offset - pos + 1 };
    }
    pos += lineLength;
  }
  return { line: lines.length, column: 1 };
}

/**
 * Generate a code snippet with an arrow pointing to the error location.
 * @param {string} code - The source code
 * @param {number} line - Line number (1-based)
 * @param {number} column - Column number (1-based)
 * @param {number} contextLines - Number of lines to show before/after (default: 2)
 * @returns {string}
 */
function generateErrorSnippet(code, line, column, contextLines = 2) {
  const lines = code.split('\n');
  const startLine = Math.max(0, line - 1 - contextLines);
  const endLine = Math.min(lines.length, line + contextLines);

  const snippetLines = [];
  const lineNumWidth = String(endLine).length;

  for (let i = startLine; i < endLine; i++) {
    const lineNum = String(i + 1).padStart(lineNumWidth, ' ');
    const marker = (i + 1 === line) ? '>' : ' ';
    snippetLines.push(`${marker} ${lineNum} | ${lines[i]}`);

    // Add arrow line for the error line
    if (i + 1 === line) {
      const padding = ' '.repeat(lineNumWidth + 4); // "  123 | " prefix
      const arrow = ' '.repeat(Math.max(0, column - 1)) + '^';
      snippetLines.push(`${padding}${arrow}`);
    }
  }

  return snippetLines.join('\n');
}

/**
 * Format an error message with code snippet.
 * @param {string} message - The error message
 * @param {string} code - The source code
 * @param {number} offset - Character offset (optional, use -1 if line/column provided)
 * @param {number} line - Line number (optional)
 * @param {number} column - Column number (optional)
 * @returns {string}
 */
function formatErrorWithSnippet(message, code, offset = -1, line = 0, column = 0) {
  if (offset >= 0) {
    const loc = offsetToLineColumn(code, offset);
    line = loc.line;
    column = loc.column;
  }

  if (line <= 0) {
    return message;
  }

  const snippet = generateErrorSnippet(code, line, column);
  return `${message}\n\n${snippet}`;
}

// Node types the LLM is allowed to generate
const ALLOWED_NODE_TYPES = new Set([
  'Program',
  'ExpressionStatement',
  'BlockStatement',
  'VariableDeclaration',
  'VariableDeclarator',
  'FunctionDeclaration',
  'ArrowFunctionExpression',
  'FunctionExpression',
  'CallExpression',
  'NewExpression',
  'MemberExpression',
  'Identifier',
  'Literal',
  'TemplateLiteral',
  'TemplateElement',
  'TaggedTemplateExpression',
  'ArrayExpression',
  'ObjectExpression',
  'SpreadElement',
  'IfStatement',
  'SwitchStatement',
  'SwitchCase',
  'ConditionalExpression',
  'ForOfStatement',
  'ForInStatement',
  'ForStatement',
  'WhileStatement',
  'TryStatement',
  'CatchClause',
  'ThrowStatement',
  'ReturnStatement',
  'BreakStatement',
  'ContinueStatement',
  'AssignmentExpression',
  'UpdateExpression',
  'BinaryExpression',
  'LogicalExpression',
  'UnaryExpression',
  'Property',
  'SequenceExpression',
  'ChainExpression',
]);

// Identifiers that are never allowed
const BLOCKED_IDENTIFIERS = new Set([
  'eval',
  'Function',
  'require',
  'process',
  'globalThis',
  '__proto__',
  'constructor',
  'prototype',
  'import',
  'exports',
  'setTimeout',
  'setInterval',
  'setImmediate',
  'queueMicrotask',
  'Proxy',
  'Reflect',
  'Symbol',
]);

// Property names that are never allowed on member expressions
const BLOCKED_PROPERTIES = new Set([
  '__proto__',
  'constructor',
  'prototype',
  '__defineGetter__',
  '__defineSetter__',
  '__lookupGetter__',
  '__lookupSetter__',
]);

/**
 * Validate DSL code against the whitelist.
 *
 * @param {string} code - The LLM-generated code to validate
 * @returns {{ valid: boolean, errors: string[] }}
 */
export function validateDSL(code) {
  const errors = [];

  // Step 1: Parse with Acorn
  let ast;
  try {
    ast = acorn.parse(code, {
      ecmaVersion: 2022,
      sourceType: 'script',
      allowReturnOutsideFunction: true,
      locations: true, // Enable location tracking for better error messages
    });
  } catch (e) {
    // Acorn errors have loc property with line/column
    const line = e.loc?.line || 0;
    const column = e.loc?.column ? e.loc.column + 1 : 0; // Acorn column is 0-based
    const formattedError = formatErrorWithSnippet(
      `Syntax error: ${e.message}`,
      code,
      -1,
      line,
      column
    );
    return { valid: false, errors: [formattedError] };
  }

  // Helper to add error with code snippet
  const addError = (message, position) => {
    errors.push(formatErrorWithSnippet(message, code, position));
  };

  // Step 2: Walk every node and validate
  walk.full(ast, (node) => {
    // Check node type against whitelist
    if (!ALLOWED_NODE_TYPES.has(node.type)) {
      addError(`Blocked node type: ${node.type}`, node.start);
      return;
    }

    // Block async functions (LLM should not write async/await)
    if (
      (node.type === 'ArrowFunctionExpression' ||
        node.type === 'FunctionExpression') &&
      node.async
    ) {
      addError(`Async functions are not allowed. Write synchronous code — the runtime handles async.`, node.start);
    }

    // Block generator functions
    if (
      (node.type === 'FunctionExpression') &&
      node.generator
    ) {
      addError(`Generator functions are not allowed`, node.start);
    }


    // Check identifiers against blocklist
    if (node.type === 'Identifier' && BLOCKED_IDENTIFIERS.has(node.name)) {
      addError(`Blocked identifier: '${node.name}'`, node.start);
    }

    // Check member expressions for blocked properties
    if (node.type === 'MemberExpression' && !node.computed) {
      if (node.property.type === 'Identifier' && BLOCKED_PROPERTIES.has(node.property.name)) {
        addError(`Blocked property access: '.${node.property.name}'`, node.property.start);
      }
    }

    // Block computed member expressions with blocked string literals
    if (node.type === 'MemberExpression' && node.computed) {
      if (node.property.type === 'Literal' && typeof node.property.value === 'string') {
        if (BLOCKED_PROPERTIES.has(node.property.value) || BLOCKED_IDENTIFIERS.has(node.property.value)) {
          addError(`Blocked computed property access: '["${node.property.value}"]'`, node.property.start);
        }
      }
    }

    // Block variable declarations named with blocked identifiers
    if (node.type === 'VariableDeclarator' && node.id.type === 'Identifier') {
      if (BLOCKED_IDENTIFIERS.has(node.id.name)) {
        addError(`Cannot declare variable with blocked name: '${node.id.name}'`, node.id.start);
      }
    }
  });

  return {
    valid: errors.length === 0,
    errors,
  };
}

/**
 * Parse DSL code into an AST.
 * Exported for use by the transformer.
 *
 * @param {string} code
 * @returns {import('acorn').Node}
 */
export function parseDSL(code) {
  return acorn.parse(code, {
    ecmaVersion: 2022,
    sourceType: 'script',
    allowReturnOutsideFunction: true,
  });
}
