/**
 * Tests for the OTEL Log Bridge — console patching for trace context and OTEL log emission.
 */

import { patchConsole, unpatchConsole, isConsolePatched } from '../../src/agent/otelLogBridge.js';

describe('OTEL Log Bridge', () => {
  afterEach(() => {
    // Always restore console after each test
    unpatchConsole();
  });

  test('isConsolePatched returns false before patching', () => {
    expect(isConsolePatched()).toBe(false);
  });

  test('patchConsole patches console methods', () => {
    const origLog = console.log;
    const origError = console.error;

    patchConsole();

    expect(isConsolePatched()).toBe(true);
    // Methods should be replaced (different function references)
    expect(console.log).not.toBe(origLog);
    expect(console.error).not.toBe(origError);
  });

  test('unpatchConsole restores state', () => {
    patchConsole();
    expect(isConsolePatched()).toBe(true);

    unpatchConsole();
    expect(isConsolePatched()).toBe(false);
  });

  test('patchConsole is idempotent', () => {
    patchConsole();
    const patchedLog = console.log;

    patchConsole(); // Call again
    expect(console.log).toBe(patchedLog); // Same patched function
    expect(isConsolePatched()).toBe(true);
  });

  test('patched console.log still calls through to original', () => {
    const calls = [];
    const origLog = console.log;
    // Replace before patching so patch captures our spy
    console.log = (...args) => calls.push(args);

    patchConsole();
    console.log('hello', 'world');

    unpatchConsole();
    console.log = origLog;

    expect(calls.length).toBe(1);
    expect(calls[0][0]).toContain('hello');
    expect(calls[0][1]).toBe('world');
  });

  test('patched console.error still calls through to original', () => {
    const calls = [];
    const origError = console.error;
    console.error = (...args) => calls.push(args);

    patchConsole();
    console.error('[DEBUG] test message');

    unpatchConsole();
    console.error = origError;

    expect(calls.length).toBe(1);
    expect(calls[0][0]).toContain('[DEBUG] test message');
  });

  test('patched console.warn still calls through to original', () => {
    const calls = [];
    const origWarn = console.warn;
    console.warn = (...args) => calls.push(args);

    patchConsole();
    console.warn('warning!');

    unpatchConsole();
    console.warn = origWarn;

    expect(calls.length).toBe(1);
    expect(calls[0][0]).toContain('warning!');
  });

  test('patched methods handle non-string arguments', () => {
    const calls = [];
    const origLog = console.log;
    console.log = (...args) => calls.push(args);

    patchConsole();
    console.log({ key: 'value' }, 42, null);

    unpatchConsole();
    console.log = origLog;

    expect(calls.length).toBe(1);
    expect(calls[0].length).toBeGreaterThanOrEqual(3);
  });

  test('patched methods handle Error objects', () => {
    const calls = [];
    const origError = console.error;
    console.error = (...args) => calls.push(args);

    patchConsole();
    const err = new Error('test error');
    console.error('Failed:', err);

    unpatchConsole();
    console.error = origError;

    expect(calls.length).toBe(1);
    expect(calls[0][0]).toContain('Failed:');
  });

  test('works without @opentelemetry/api installed (graceful degradation)', () => {
    // Since @opentelemetry/api-logs is likely not installed in test environment,
    // patchConsole should still work — just without trace context or OTEL log records
    patchConsole();

    expect(() => {
      console.log('[DEBUG] test without OTEL');
      console.error('[ERROR] test without OTEL');
      console.warn('[WARN] test without OTEL');
      console.info('[INFO] test without OTEL');
    }).not.toThrow();
  });

  test('unpatchConsole is idempotent', () => {
    patchConsole();
    unpatchConsole();
    unpatchConsole(); // Second call should be safe
    expect(isConsolePatched()).toBe(false);
  });
});
