/**
 * DSL Validator - AST whitelist validation for LLM-generated code.
 *
 * Parses code with Acorn and walks the AST, rejecting any node type
 * not in the whitelist. This is an allow-list approach — unknown syntax
 * is rejected by default.
 */

import * as acorn from 'acorn';
import * as walk from 'acorn-walk';

// Node types the LLM is allowed to generate
const ALLOWED_NODE_TYPES = new Set([
  'Program',
  'ExpressionStatement',
  'BlockStatement',
  'VariableDeclaration',
  'VariableDeclarator',
  'ArrowFunctionExpression',
  'FunctionExpression',
  'CallExpression',
  'MemberExpression',
  'Identifier',
  'Literal',
  'TemplateLiteral',
  'TemplateElement',
  'ArrayExpression',
  'ObjectExpression',
  'SpreadElement',
  'IfStatement',
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
    });
  } catch (e) {
    return { valid: false, errors: [`Syntax error: ${e.message}`] };
  }

  // Step 2: Walk every node and validate
  walk.full(ast, (node) => {
    // Check node type against whitelist
    if (!ALLOWED_NODE_TYPES.has(node.type)) {
      errors.push(`Blocked node type: ${node.type} at position ${node.start}`);
      return;
    }

    // Block async functions (LLM should not write async/await)
    if (
      (node.type === 'ArrowFunctionExpression' ||
        node.type === 'FunctionExpression') &&
      node.async
    ) {
      errors.push(`Async functions are not allowed at position ${node.start}. Write synchronous code — the runtime handles async.`);
    }

    // Block generator functions
    if (
      (node.type === 'FunctionExpression') &&
      node.generator
    ) {
      errors.push(`Generator functions are not allowed at position ${node.start}`);
    }

    // Block regex literals — SandboxJS doesn't support them
    if (node.type === 'Literal' && node.regex) {
      errors.push(`Regex literals are not supported at position ${node.start}. Use String methods like indexOf(), includes(), startsWith() instead.`);
    }

    // Check identifiers against blocklist
    if (node.type === 'Identifier' && BLOCKED_IDENTIFIERS.has(node.name)) {
      errors.push(`Blocked identifier: '${node.name}' at position ${node.start}`);
    }

    // Check member expressions for blocked properties
    if (node.type === 'MemberExpression' && !node.computed) {
      if (node.property.type === 'Identifier' && BLOCKED_PROPERTIES.has(node.property.name)) {
        errors.push(`Blocked property access: '.${node.property.name}' at position ${node.property.start}`);
      }
    }

    // Block computed member expressions with blocked string literals
    if (node.type === 'MemberExpression' && node.computed) {
      if (node.property.type === 'Literal' && typeof node.property.value === 'string') {
        if (BLOCKED_PROPERTIES.has(node.property.value) || BLOCKED_IDENTIFIERS.has(node.property.value)) {
          errors.push(`Blocked computed property access: '["${node.property.value}"]' at position ${node.property.start}`);
        }
      }
    }

    // Block variable declarations named with blocked identifiers
    if (node.type === 'VariableDeclarator' && node.id.type === 'Identifier') {
      if (BLOCKED_IDENTIFIERS.has(node.id.name)) {
        errors.push(`Cannot declare variable with blocked name: '${node.id.name}' at position ${node.id.start}`);
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
