/**
 * Tests for bash tool integration with ProbeAgent
 * @module tests/unit/bash-probe-agent-integration
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';

// Mock all the heavy dependencies that ProbeAgent uses
jest.mock('@ai-sdk/anthropic', () => ({}));
jest.mock('@ai-sdk/openai', () => ({}));
jest.mock('@ai-sdk/google', () => ({}));
jest.mock('ai', () => ({
  generateText: jest.fn(),
  tool: jest.fn((config) => ({
    name: config.name,
    description: config.description,
    inputSchema: config.inputSchema,
    execute: config.execute
  }))
}));

// Mock the tools that depend on external binaries
jest.mock('../../src/search.js', () => ({
  search: jest.fn().mockResolvedValue('mock search result')
}));
jest.mock('../../src/query.js', () => ({
  query: jest.fn().mockResolvedValue('mock query result')
}));
jest.mock('../../src/extract.js', () => ({
  extract: jest.fn().mockResolvedValue('mock extract result')
}));
jest.mock('../../src/delegate.js', () => ({
  delegate: jest.fn().mockResolvedValue('mock delegate result')
}));

// Import after mocking
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

describe('Bash Tool ProbeAgent Integration', () => {
  describe('ProbeAgent Configuration', () => {
    test('should create ProbeAgent with bash disabled by default', () => {
      const agent = new ProbeAgent({
        debug: false
      });

      expect(agent.enableBash).toBe(false);
      expect(agent.bashConfig).toEqual({});
      expect(agent.toolImplementations.bash).toBeUndefined();
    });

    test('should create ProbeAgent with bash enabled', () => {
      const agent = new ProbeAgent({
        enableBash: true,
        debug: false
      });

      expect(agent.enableBash).toBe(true);
      expect(agent.bashConfig).toEqual({});
      
      // Should have bash tool in implementations
      expect(agent.toolImplementations.bash).toBeDefined();
    });

    test('should create ProbeAgent with custom bash config', () => {
      const bashConfig = {
        allow: ['docker:ps', 'make:help'],
        deny: ['git:push'],
        timeout: 60000,
        workingDirectory: '/test'
      };

      const agent = new ProbeAgent({
        enableBash: true,
        bashConfig: bashConfig,
        debug: false
      });

      expect(agent.enableBash).toBe(true);
      expect(agent.bashConfig).toEqual(bashConfig);
      expect(agent.toolImplementations.bash).toBeDefined();
    });

    test('should pass configuration to tools properly', () => {
      const bashConfig = {
        allow: ['custom:command'],
        timeout: 30000
      };

      const agent = new ProbeAgent({
        enableBash: true,
        bashConfig: bashConfig,
        allowedFolders: ['/home/test'],
        debug: false
      });

      expect(agent.enableBash).toBe(true);
      expect(agent.bashConfig).toEqual(bashConfig);
      expect(agent.allowedFolders).toEqual(['/home/test']);
    });
  });

  describe('Tool Implementation Availability', () => {
    test('should not have bash tool when disabled', () => {
      const agent = new ProbeAgent({
        enableBash: false,
        debug: false
      });

      expect(agent.toolImplementations.bash).toBeUndefined();
      
      // Should have other tools
      expect(agent.toolImplementations.search).toBeDefined();
      expect(agent.toolImplementations.query).toBeDefined();
      expect(agent.toolImplementations.extract).toBeDefined();
    });

    test('should have bash tool when enabled', () => {
      const agent = new ProbeAgent({
        enableBash: true,
        debug: false
      });

      expect(agent.toolImplementations.bash).toBeDefined();
      
      // Should also have other tools
      expect(agent.toolImplementations.search).toBeDefined();
      expect(agent.toolImplementations.query).toBeDefined();
      expect(agent.toolImplementations.extract).toBeDefined();
    });

    test('should have executable bash tool', () => {
      const agent = new ProbeAgent({
        enableBash: true,
        debug: false
      });

      const bashTool = agent.toolImplementations.bash;
      expect(bashTool).toBeDefined();
      expect(typeof bashTool.execute).toBe('function');
    });
  });

  describe('Configuration Edge Cases', () => {
    test('should handle empty bash config', () => {
      const agent = new ProbeAgent({
        enableBash: true,
        bashConfig: {},
        debug: false
      });

      expect(agent.enableBash).toBe(true);
      expect(agent.bashConfig).toEqual({});
      expect(agent.toolImplementations.bash).toBeDefined();
    });

    test('should handle null bash config', () => {
      const agent = new ProbeAgent({
        enableBash: true,
        bashConfig: null,
        debug: false
      });

      expect(agent.enableBash).toBe(true);
      expect(agent.bashConfig).toEqual({});
      expect(agent.toolImplementations.bash).toBeDefined();
    });

    test('should handle undefined bash config', () => {
      const agent = new ProbeAgent({
        enableBash: true,
        debug: false
      });

      expect(agent.enableBash).toBe(true);
      expect(agent.bashConfig).toEqual({});
      expect(agent.toolImplementations.bash).toBeDefined();
    });

    test('should handle complex bash configuration', () => {
      const complexConfig = {
        allow: ['docker:*', 'npm:run:*', 'make:*'],
        deny: ['docker:rm:*', 'npm:install:*'],
        disableDefaultAllow: false,
        disableDefaultDeny: false,
        timeout: 120000,
        workingDirectory: '/app',
        env: {
          NODE_ENV: 'test',
          DEBUG: '1'
        },
        maxBuffer: 10 * 1024 * 1024
      };

      const agent = new ProbeAgent({
        enableBash: true,
        bashConfig: complexConfig,
        debug: false
      });

      expect(agent.bashConfig).toEqual(complexConfig);
      expect(agent.toolImplementations.bash).toBeDefined();
    });
  });

  describe('Working Directory Integration', () => {
    test('should use allowedFolders as default working directory', () => {
      const agent = new ProbeAgent({
        enableBash: true,
        allowedFolders: ['/project', '/tmp'],
        debug: false
      });

      expect(agent.allowedFolders).toEqual(['/project', '/tmp']);
      expect(agent.toolImplementations.bash).toBeDefined();
    });

    test('should handle empty allowedFolders', () => {
      const agent = new ProbeAgent({
        enableBash: true,
        allowedFolders: [],
        debug: false
      });

      expect(agent.allowedFolders).toEqual([]);
      expect(agent.toolImplementations.bash).toBeDefined();
    });

    test('should use path parameter as allowedFolders', () => {
      const agent = new ProbeAgent({
        enableBash: true,
        path: '/single/path',
        debug: false
      });

      expect(agent.allowedFolders).toEqual(['/single/path']);
      expect(agent.toolImplementations.bash).toBeDefined();
    });
  });

  describe('Debug Mode Integration', () => {
    test('should pass debug flag to bash configuration', () => {
      const agent = new ProbeAgent({
        enableBash: true,
        debug: true
      });

      expect(agent.debug).toBe(true);
      expect(agent.enableBash).toBe(true);
      expect(agent.toolImplementations.bash).toBeDefined();
    });

    test('should respect process.env.DEBUG', () => {
      const originalDebug = process.env.DEBUG;
      process.env.DEBUG = '1';

      const agent = new ProbeAgent({
        enableBash: true
      });

      expect(agent.debug).toBe(true);
      expect(agent.enableBash).toBe(true);
      
      // Restore original value
      if (originalDebug !== undefined) {
        process.env.DEBUG = originalDebug;
      } else {
        delete process.env.DEBUG;
      }
    });
  });
});

describe('Bash Tool Configuration Parsing', () => {
  // These tests simulate what the CLI would do when parsing arguments
  describe('CLI-style Configuration Processing', () => {
    test('should parse comma-separated allow patterns', () => {
      const allowPatterns = 'docker:ps,docker:images,npm:list';
      const parsedPatterns = allowPatterns.split(',').map(p => p.trim()).filter(p => p.length > 0);

      expect(parsedPatterns).toEqual(['docker:ps', 'docker:images', 'npm:list']);

      const agent = new ProbeAgent({
        enableBash: true,
        bashConfig: {
          allow: parsedPatterns
        },
        debug: false
      });

      expect(agent.bashConfig.allow).toEqual(parsedPatterns);
    });

    test('should parse comma-separated deny patterns', () => {
      const denyPatterns = 'rm:*,sudo:*,chmod:777';
      const parsedPatterns = denyPatterns.split(',').map(p => p.trim()).filter(p => p.length > 0);

      expect(parsedPatterns).toEqual(['rm:*', 'sudo:*', 'chmod:777']);

      const agent = new ProbeAgent({
        enableBash: true,
        bashConfig: {
          deny: parsedPatterns
        },
        debug: false
      });

      expect(agent.bashConfig.deny).toEqual(parsedPatterns);
    });

    test('should handle empty patterns gracefully', () => {
      const allowPatterns = '';
      const parsedPatterns = allowPatterns.split(',').map(p => p.trim()).filter(p => p.length > 0);

      expect(parsedPatterns).toEqual([]);

      const agent = new ProbeAgent({
        enableBash: true,
        bashConfig: {
          allow: parsedPatterns.length > 0 ? parsedPatterns : undefined
        },
        debug: false
      });

      expect(agent.bashConfig.allow).toBeUndefined();
    });

    test('should handle patterns with extra whitespace', () => {
      const allowPatterns = ' docker:ps , npm:list , git:status ';
      const parsedPatterns = allowPatterns.split(',').map(p => p.trim()).filter(p => p.length > 0);

      expect(parsedPatterns).toEqual(['docker:ps', 'npm:list', 'git:status']);
    });

    test('should parse timeout values', () => {
      const timeoutString = '60000';
      const timeout = parseInt(timeoutString, 10);

      expect(timeout).toBe(60000);
      expect(isNaN(timeout)).toBe(false);

      const agent = new ProbeAgent({
        enableBash: true,
        bashConfig: {
          timeout: timeout
        },
        debug: false
      });

      expect(agent.bashConfig.timeout).toBe(60000);
    });

    test('should handle invalid timeout values', () => {
      const timeoutString = 'invalid';
      const timeout = parseInt(timeoutString, 10);

      expect(isNaN(timeout)).toBe(true);
      
      // In real CLI, this would cause an error before ProbeAgent creation
      // Here we just test that ProbeAgent doesn't break with invalid values
      const agent = new ProbeAgent({
        enableBash: true,
        bashConfig: {
          timeout: isNaN(timeout) ? undefined : timeout
        },
        debug: false
      });

      expect(agent.bashConfig.timeout).toBeUndefined();
    });

    test('should handle disable flags', () => {
      // Simulating --no-default-bash-allow and --no-default-bash-deny
      const defaultBashAllow = false;
      const defaultBashDeny = false;

      const agent = new ProbeAgent({
        enableBash: true,
        bashConfig: {
          disableDefaultAllow: defaultBashAllow === false,
          disableDefaultDeny: defaultBashDeny === false
        },
        debug: false
      });

      expect(agent.bashConfig.disableDefaultAllow).toBe(true);
      expect(agent.bashConfig.disableDefaultDeny).toBe(true);
    });
  });

  describe('Environment Variable Integration', () => {
    test('should not interfere with existing environment handling', () => {
      const agent = new ProbeAgent({
        enableBash: true,
        debug: false
      });

      // Should still have basic functionality
      expect(agent.sessionId).toBeTruthy();
      expect(typeof agent.sessionId).toBe('string');
      expect(agent.toolImplementations).toBeDefined();
    });
  });
});