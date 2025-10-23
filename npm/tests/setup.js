/**
 * Jest setup file
 * This file runs before all tests to set up the testing environment
 */
import { jest, beforeEach, afterEach } from '@jest/globals';
import fs from 'fs';
import path from 'path';

// Set environment to test
process.env.NODE_ENV = 'test';

// Disable debug logging during tests unless explicitly enabled
if (!process.env.TEST_DEBUG) {
  process.env.DEBUG = '';
}

// Prefer local binary in repository to avoid network during tests
try {
  const isWin = process.platform === 'win32';
  const binDir = path.resolve(__dirname, '..', 'bin');
  const candidate = path.join(binDir, isWin ? 'probe.exe' : 'probe-binary');
  if (fs.existsSync(candidate)) {
    process.env.PROBE_PATH = candidate;
  }
} catch {}

// Global test timeout (can be overridden per test)
jest.setTimeout(10000);

// Mock console methods to avoid cluttering test output
const originalConsole = { ...console };
beforeEach(() => {
  if (!process.env.TEST_VERBOSE) {
    console.log = jest.fn();
    console.error = jest.fn();
    console.warn = jest.fn();
  }
});

afterEach(() => {
  if (!process.env.TEST_VERBOSE) {
    console.log = originalConsole.log;
    console.error = originalConsole.error;
    console.warn = originalConsole.warn;
  }
});
