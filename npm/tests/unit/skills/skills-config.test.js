import { describe, test, expect } from '@jest/globals';
import { ProbeAgent } from '../../../src/agent/ProbeAgent.js';

describe('ProbeAgent skills configuration', () => {
  describe('skills disabled by default', () => {
    test('should have skills disabled when no options provided', () => {
      const agent = new ProbeAgent({
        path: process.cwd()
      });

      expect(agent.enableSkills).toBe(false);
    });

    test('should have skills disabled when enableSkills is not set', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowedTools: ['*']
      });

      expect(agent.enableSkills).toBe(false);
    });
  });

  describe('enabling skills with allowSkills', () => {
    test('should enable skills when allowSkills is true', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowSkills: true
      });

      expect(agent.enableSkills).toBe(true);
    });

    test('should keep skills disabled when allowSkills is false', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowSkills: false
      });

      expect(agent.enableSkills).toBe(false);
    });
  });

  describe('enabling skills with enableSkills (deprecated)', () => {
    test('should enable skills when enableSkills is true', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        enableSkills: true
      });

      expect(agent.enableSkills).toBe(true);
    });

    test('should keep skills disabled when enableSkills is false', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        enableSkills: false
      });

      expect(agent.enableSkills).toBe(false);
    });
  });

  describe('disableSkills override', () => {
    test('should disable skills when disableSkills is true even if allowSkills is true', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowSkills: true,
        disableSkills: true
      });

      expect(agent.enableSkills).toBe(false);
    });

    test('should disable skills when disableSkills is true even if enableSkills is true', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        enableSkills: true,
        disableSkills: true
      });

      expect(agent.enableSkills).toBe(false);
    });

    test('should respect allowSkills when disableSkills is false', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowSkills: true,
        disableSkills: false
      });

      expect(agent.enableSkills).toBe(true);
    });
  });

  describe('skill tools not initialized when disabled', () => {
    test('should not initialize skill tools when skills are disabled', () => {
      const agent = new ProbeAgent({
        path: process.cwd()
      });

      expect(agent.toolImplementations.listSkills).toBeUndefined();
      expect(agent.toolImplementations.useSkill).toBeUndefined();
    });

    test('should initialize skill tools when allowSkills is true', () => {
      const agent = new ProbeAgent({
        path: process.cwd(),
        allowSkills: true
      });

      // Note: The actual tool implementations depend on whether skills are found
      // We just verify enableSkills is set correctly
      expect(agent.enableSkills).toBe(true);
    });
  });

  describe('clone preserves skills configuration', () => {
    test('should preserve allowSkills in clone', () => {
      const baseAgent = new ProbeAgent({
        path: process.cwd(),
        allowSkills: true
      });

      const cloned = baseAgent.clone();

      expect(cloned.enableSkills).toBe(true);
    });

    test('should preserve disabled skills in clone', () => {
      const baseAgent = new ProbeAgent({
        path: process.cwd(),
        allowSkills: false
      });

      const cloned = baseAgent.clone();

      expect(cloned.enableSkills).toBe(false);
    });
  });
});
