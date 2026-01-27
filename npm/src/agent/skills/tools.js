import { wrapToolWithEmitter } from '../probeTool.js';

function normalizeSkillName(name) {
  return typeof name === 'string' ? name.trim() : '';
}

export function createSkillToolInstances({ registry, activeSkills }) {
  const listSkillsTool = {
    execute: async (params = {}) => {
      const filter = typeof params.filter === 'string' ? params.filter.trim().toLowerCase() : '';
      const skills = await registry.loadSkills();
      const filtered = filter
        ? skills.filter(skill =>
            skill.name.toLowerCase().includes(filter) ||
            (skill.description || '').toLowerCase().includes(filter)
          )
        : skills;

      return {
        skills: filtered.map(skill => ({
          name: skill.name,
          description: skill.description
        }))
      };
    }
  };

  const useSkillTool = {
    execute: async (params = {}) => {
      const rawName = normalizeSkillName(params.name);
      if (!rawName) {
        throw new Error('Skill name is required');
      }

      await registry.loadSkills();
      let skill = registry.getSkill(rawName);
      if (!skill) {
        skill = registry.getSkill(rawName.toLowerCase());
      }

      if (!skill) {
        const available = registry.getSkills().map(s => s.name).join(', ') || 'None';
        throw new Error(`Skill '${rawName}' not found. Available skills: ${available}`);
      }

      const instructions = await registry.loadSkillInstructions(skill.name);
      if (!instructions) {
        throw new Error(`Skill '${skill.name}' has no instructions`);
      }

      activeSkills.set(skill.name, { ...skill, instructions });

      return {
        name: skill.name,
        description: skill.description,
        instructions
      };
    }
  };

  return {
    listSkillsToolInstance: wrapToolWithEmitter(listSkillsTool, 'listSkills', listSkillsTool.execute),
    useSkillToolInstance: wrapToolWithEmitter(useSkillTool, 'useSkill', useSkillTool.execute)
  };
}
