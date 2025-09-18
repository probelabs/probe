/** @type {import('jest').Config} */
export default {
  transform: {},
  
  // Test environment
  testEnvironment: 'node',
  
  // Test file patterns - run stable test files  
  testMatch: [
    '**/tests/**/*.test.js',
    '**/src/agent/acp/tools.test.js',
    '**/src/agent/acp/connection.test.js',
    '**/src/agent/acp/types.test.js'
  ],
  
  // Coverage configuration
  collectCoverageFrom: [
    'src/**/*.js',
    '!src/**/*.test.js',
    '!src/test-*.js',
    '!**/node_modules/**',
    '!**/build/**',
    '!**/dist/**'
  ],
  
  // Coverage thresholds
  coverageThreshold: {
    global: {
      branches: 70,
      functions: 70,
      lines: 70,
      statements: 70
    }
  },
  
  // Coverage reporters
  coverageReporters: [
    'text',
    'lcov',
    'html'
  ],
  
  // Setup files
  setupFilesAfterEnv: ['<rootDir>/tests/setup.js'],
  
  
  // Verbose output
  verbose: true,
  
  // Timeout for tests
  testTimeout: 10000
};