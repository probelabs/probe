/**
 * DSL Runtime - SandboxJS execution engine.
 *
 * Orchestrates the full pipeline:
 * 1. Validate (AST whitelist)
 * 2. Transform (inject await, wrap in async IIFE)
 * 3. Execute in SandboxJS with tool globals + timeout
 *
 * Returns the result or a structured error.
 */

import SandboxModule from '@nyariv/sandboxjs';
import { validateDSL } from './validator.js';
import { transformDSL } from './transformer.js';
import { generateSandboxGlobals, getAsyncFunctionNames } from './environment.js';

const Sandbox = SandboxModule.default || SandboxModule;

/**
 * Create a DSL runtime instance.
 *
 * @param {Object} options
 * @param {Object} options.toolImplementations - Native tool execute functions
 * @param {Object} [options.mcpBridge] - MCP bridge for calling MCP tools
 * @param {Object} [options.mcpTools={}] - MCP tool metadata
 * @param {Function} options.llmCall - Function for LLM() calls: (instruction, data, options?) => Promise<any>
 * @param {number} [options.mapConcurrency=3] - Concurrency limit for map()
 * @param {number} [options.timeoutMs=120000] - Execution timeout in milliseconds (default 2 min)
 * @param {number} [options.maxLoopIterations=5000] - Max iterations for while/for loops
 * @param {Object} [options.tracer=null] - SimpleAppTracer instance for OTEL telemetry
 * @returns {Object} Runtime with execute() method
 */
export function createDSLRuntime(options) {
  const {
    toolImplementations = {},
    mcpBridge = null,
    mcpTools = {},
    llmCall,
    mapConcurrency = 3,
    timeoutMs = 120000,
    maxLoopIterations = 5000,
    tracer = null,
    sessionStore = {},
    outputBuffer = null,
  } = options;

  // Generate the globals and async function names, passing tracer for per-call tracing
  const toolGlobals = generateSandboxGlobals({
    toolImplementations,
    mcpBridge,
    mcpTools,
    llmCall,
    mapConcurrency,
    tracer,
    sessionStore,
    outputBuffer,
  });

  const asyncFunctionNames = getAsyncFunctionNames(mcpTools);

  /**
   * Execute DSL code.
   *
   * @param {string} code - The LLM-generated DSL code (sync-looking)
   * @param {string} [description] - Human-readable description for logging
   * @returns {Promise<{ status: 'success'|'error', result?: any, error?: string, logs: string[] }>}
   */
  async function execute(code, description) {
    const logs = [];
    const startTime = Date.now();

    // Step 1: Validate
    tracer?.addEvent?.('dsl.phase.validate_start', {
      'dsl.code_length': code.length,
    });

    const validation = validateDSL(code);
    if (!validation.valid) {
      tracer?.addEvent?.('dsl.phase.validate_failed', {
        'dsl.error_count': validation.errors.length,
        'dsl.errors': validation.errors.join('; ').substring(0, 500),
      });
      return {
        status: 'error',
        error: `Validation failed:\n${validation.errors.join('\n')}`,
        logs,
      };
    }

    tracer?.addEvent?.('dsl.phase.validate_complete');

    // Step 2: Transform (inject await, wrap in async IIFE)
    let transformedCode;
    try {
      tracer?.addEvent?.('dsl.phase.transform_start');
      transformedCode = transformDSL(code, asyncFunctionNames);
      tracer?.addEvent?.('dsl.phase.transform_complete', {
        'dsl.transformed_length': transformedCode.length,
      });
    } catch (e) {
      tracer?.addEvent?.('dsl.phase.transform_failed', {
        'dsl.error': e.message,
      });
      return {
        status: 'error',
        error: `Transform failed: ${e.message}`,
        logs,
      };
    }

    // Step 3: Execute in SandboxJS with timeout
    tracer?.addEvent?.('dsl.phase.execute_start', {
      'dsl.timeout_ms': timeoutMs,
      'dsl.max_loop_iterations': maxLoopIterations,
    });

    try {
      // Set up log collector
      toolGlobals._logs = logs;

      // Loop iteration counter for infinite loop protection
      let loopIterations = 0;
      toolGlobals.__checkLoop = () => {
        loopIterations++;
        if (loopIterations > maxLoopIterations) {
          throw new Error(`Loop exceeded maximum of ${maxLoopIterations} iterations. Use break to exit loops earlier or process fewer items.`);
        }
      };

      const sandbox = new Sandbox({
        globals: {
          ...Sandbox.SAFE_GLOBALS,
          ...toolGlobals,
          // Override: remove dangerous globals that SAFE_GLOBALS might include
          Function: undefined,
          eval: undefined,
        },
        prototypeWhitelist: Sandbox.SAFE_PROTOTYPES,
      });

      const exec = sandbox.compileAsync(transformedCode);

      // Catch unhandled rejections from SandboxJS async error propagation
      let escapedError = null;
      const rejectionHandler = (reason) => {
        escapedError = reason;
      };
      process.on('unhandledRejection', rejectionHandler);

      // Race execution against timeout
      let timeoutHandle;
      const executionPromise = exec().run();
      const timeoutPromise = new Promise((_, reject) => {
        timeoutHandle = setTimeout(() => {
          reject(new Error(`Execution timed out after ${Math.round(timeoutMs / 1000)}s. Script took too long — reduce the amount of work (fewer items, smaller data) or increase timeout.`));
        }, timeoutMs);
      });

      let result;
      try {
        result = await Promise.race([executionPromise, timeoutPromise]);
      } finally {
        clearTimeout(timeoutHandle);
        // Delay handler removal — SandboxJS can throw async errors after execution completes
        setTimeout(() => {
          process.removeListener('unhandledRejection', rejectionHandler);
        }, 500);
      }

      // Check for escaped async errors
      if (escapedError) {
        throw escapedError;
      }

      const elapsed = Date.now() - startTime;
      logs.push(`[runtime] Completed in ${elapsed}ms`);

      tracer?.addEvent?.('dsl.phase.execute_complete', {
        'dsl.duration_ms': elapsed,
        'dsl.loop_iterations': loopIterations,
      });

      return {
        status: 'success',
        result,
        logs,
      };
    } catch (e) {
      const elapsed = Date.now() - startTime;
      logs.push(`[runtime] Failed after ${elapsed}ms`);

      tracer?.addEvent?.('dsl.phase.execute_failed', {
        'dsl.duration_ms': elapsed,
        'dsl.error': e.message?.substring(0, 500),
      });

      return {
        status: 'error',
        error: `Execution failed: ${e.message}`,
        logs,
      };
    }
  }

  return { execute };
}
