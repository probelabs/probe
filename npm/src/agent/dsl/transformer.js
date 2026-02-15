/**
 * AST Transformer - Auto-injects await before async tool calls.
 *
 * The LLM writes synchronous-looking code. This transformer:
 * 1. Parses the code into an AST
 * 2. Finds all CallExpressions where the callee is a known async tool function
 * 3. Inserts `await` before those calls in the source
 * 4. Marks arrow functions containing async calls as `async`
 * 5. Wraps the whole program in an async IIFE
 *
 * Uses offset-based string insertion (not AST regeneration) to preserve
 * the original code structure as much as possible.
 */

import * as acorn from 'acorn';
import * as walk from 'acorn-walk';

/**
 * Transform DSL code by injecting await and async wrappers.
 *
 * @param {string} code - The sync-looking DSL code
 * @param {Set<string>} asyncFunctionNames - Names of functions that are async (tool functions)
 * @returns {string} Transformed code with await injected, wrapped in async IIFE
 */
export function transformDSL(code, asyncFunctionNames) {
  let ast;
  try {
    ast = acorn.parse(code, {
      ecmaVersion: 2022,
      sourceType: 'script',
      allowReturnOutsideFunction: true,
    });
  } catch (e) {
    throw new Error(`Transform parse error: ${e.message}`);
  }

  // Collect insertions: { offset, text } sorted by offset descending
  // We insert from end to start so offsets don't shift
  const insertions = [];

  // Track which arrow/function expressions need to be marked async
  const functionsNeedingAsync = new Set();

  // Find the enclosing function for a given node position
  function findEnclosingFunction(node) {
    // Walk the AST to find parent functions
    // We'll use a different approach: collect all functions and their ranges
    return null; // Handled by the parent tracking below
  }

  // First pass: collect all function scopes with their ranges
  const functionScopes = [];
  walk.full(ast, (node) => {
    if (node.type === 'ArrowFunctionExpression' || node.type === 'FunctionExpression') {
      functionScopes.push(node);
    }
  });

  // Second pass: find async calls and determine what needs transformation
  walk.full(ast, (node) => {
    if (node.type !== 'CallExpression') return;

    const calleeName = getCalleeName(node);
    if (!calleeName || !asyncFunctionNames.has(calleeName)) return;

    // This call needs await. Check if it's already awaited.
    // (It shouldn't be since we block AwaitExpression in the validator,
    // but be defensive.)

    // Insert 'await ' before the call expression
    insertions.push({ offset: node.start, text: 'await ' });

    // Find the enclosing function (if any) and mark it as needing async
    for (const fn of functionScopes) {
      if (fn.body.start <= node.start && fn.body.end >= node.end) {
        functionsNeedingAsync.add(fn);
      }
    }
  });

  // Also check: if 'map' is called with a callback that contains async calls,
  // mark that callback as async. The callback is typically the second argument.
  walk.full(ast, (node) => {
    if (node.type !== 'CallExpression') return;
    const calleeName = getCalleeName(node);
    if (calleeName !== 'map' || node.arguments.length < 2) return;

    const callback = node.arguments[1];
    if (callback.type === 'ArrowFunctionExpression' || callback.type === 'FunctionExpression') {
      // Check if this callback contains any async tool calls
      let hasAsyncCall = false;
      walk.full(callback, (inner) => {
        if (inner.type === 'CallExpression') {
          const innerName = getCalleeName(inner);
          if (innerName && asyncFunctionNames.has(innerName)) {
            hasAsyncCall = true;
          }
        }
      });
      if (hasAsyncCall) {
        functionsNeedingAsync.add(callback);
      }
    }
  });

  // Third pass: inject loop guards (__checkLoop()) into while/for loops
  walk.full(ast, (node) => {
    if (node.type === 'WhileStatement' || node.type === 'ForStatement' || node.type === 'ForOfStatement' || node.type === 'ForInStatement') {
      // Insert __checkLoop(); at the start of the loop body
      const body = node.body;
      if (body.type === 'BlockStatement' && body.body.length > 0) {
        // Insert after the opening brace
        insertions.push({ offset: body.start + 1, text: ' __checkLoop();' });
      }
    }
  });

  // Fourth pass: fix catch clause parameters (SandboxJS doesn't bind them)
  // Rename the catch parameter to avoid var conflicts on Node 20, then inject
  // var with the original name. This ensures the original variable name works
  // in the catch body on all Node.js versions.
  // Transform: catch (e) { BODY } → catch (__catchParam) { var e = __getLastError(); BODY }
  walk.full(ast, (node) => {
    if (node.type === 'CatchClause' && node.param) {
      const paramName = node.param.name;
      // Replace the catch parameter identifier with a dummy name
      insertions.push({
        offset: node.param.start,
        deleteCount: node.param.end - node.param.start,
        text: '__catchParam',
      });
      // Inject var declaration with the original name
      insertions.push({
        offset: node.body.start + 1,
        text: ` var ${paramName} = __getLastError();`,
      });
    }
  });

  // Fifth pass: transform throw statements to capture error
  // Transform: throw EXPR; → throw __setLastError(EXPR);
  walk.full(ast, (node) => {
    if (node.type === 'ThrowStatement' && node.argument) {
      insertions.push({ offset: node.argument.start, text: '__setLastError(' });
      insertions.push({ offset: node.argument.end, text: ')' });
    }
  });

  // Build insertions for async markers on functions
  for (const fn of functionsNeedingAsync) {
    // Insert 'async ' before the function
    // For arrow functions: `(x) => ...` → `async (x) => ...`
    // For function expressions: `function(x) { ... }` → `async function(x) { ... }`
    insertions.push({ offset: fn.start, text: 'async ' });
  }

  // Sort insertions by offset descending (apply from end to preserve offsets)
  insertions.sort((a, b) => b.offset - a.offset);

  // Apply insertions/replacements to the source code
  let transformed = code;
  for (const ins of insertions) {
    const deleteCount = ins.deleteCount || 0;
    transformed = transformed.slice(0, ins.offset) + ins.text + transformed.slice(ins.offset + deleteCount);
  }

  // Wrap in async IIFE with return so SandboxJS awaits the result
  transformed = `return (async () => {\n${transformed}\n})()`;

  return transformed;
}

/**
 * Extract the function name from a CallExpression callee.
 * Handles: `foo()` → 'foo', `obj.foo()` → 'foo' (for member access)
 *
 * @param {import('acorn').Node} callExpr
 * @returns {string|null}
 */
function getCalleeName(callExpr) {
  const callee = callExpr.callee;
  if (callee.type === 'Identifier') {
    return callee.name;
  }
  // For member expressions like mcp_server.tool(), get the full dotted name
  // But our tools use flat names like mcp_github_create_issue, so Identifier is sufficient
  return null;
}
