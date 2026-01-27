import { describe, test, expect } from '@jest/globals';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { SkillRegistry } from '../../../src/agent/skills/registry.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '../../../..');
const skillsRoot = resolve(repoRoot, 'test_data/skills');

describe('SkillRegistry', () => {
  test('loads valid skills and applies fallbacks', async () => {
    const registry = new SkillRegistry({
      repoRoot: skillsRoot,
      skillDirs: ['.claude/skills', '.codex/skills']
    });

    const skills = await registry.loadSkills();
    const names = skills.map(skill => skill.name);

    expect(names).toContain('valid-skill');
    expect(names).toContain('no-name');
    expect(names).toContain('no-desc');
    expect(names).toContain('invalid-name');
    expect(names).not.toContain('invalid-yaml');

    const noNameSkill = registry.getSkill('no-name');
    expect(noNameSkill).toBeTruthy();
    expect(noNameSkill.name).toBe('no-name');

    const noDescSkill = registry.getSkill('no-desc');
    expect(noDescSkill).toBeTruthy();
    expect(noDescSkill.description).toBe('First paragraph description.');

    const invalidNameSkill = registry.getSkill('invalid-name');
    expect(invalidNameSkill).toBeTruthy();
    expect(invalidNameSkill.name).toBe('invalid-name');
  });

  test('loads full instructions for a skill', async () => {
    const registry = new SkillRegistry({
      repoRoot: skillsRoot,
      skillDirs: ['.claude/skills']
    });

    await registry.loadSkills();
    const instructions = await registry.loadSkillInstructions('valid-skill');

    expect(instructions).toContain('Use this skill to test valid parsing.');
    expect(instructions).not.toContain('name: valid-skill');
  });
});
