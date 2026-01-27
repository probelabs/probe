import { existsSync } from 'fs';
import { readdir, readFile } from 'fs/promises';
import { resolve, join, isAbsolute, sep } from 'path';
import { parseSkillFile, stripFrontmatter } from './parser.js';

const DEFAULT_SKILL_DIRS = ['.claude/skills', '.codex/skills', 'skills', '.skills'];
const SKILL_FILE_NAME = 'SKILL.md';

export class SkillRegistry {
  constructor({ repoRoot, skillDirs = DEFAULT_SKILL_DIRS, debug = false } = {}) {
    this.repoRoot = repoRoot ? resolve(repoRoot) : process.cwd();
    this.skillDirs = Array.isArray(skillDirs) && skillDirs.length > 0 ? skillDirs : DEFAULT_SKILL_DIRS;
    this.debug = debug;
    this.skills = [];
    this.skillsByName = new Map();
    this.loaded = false;
  }

  async loadSkills() {
    if (this.loaded) return this.skills;

    const discovered = [];
    for (const skillDir of this.skillDirs) {
      const resolvedDir = this._resolveSkillDir(skillDir);
      if (!resolvedDir) continue;
      const skillsInDir = await this._scanSkillDir(resolvedDir);
      discovered.push(...skillsInDir);
    }

    this.skills = discovered;
    this.loaded = true;
    return this.skills;
  }

  getSkills() {
    return this.skills;
  }

  getSkill(name) {
    return this.skillsByName.get(name);
  }

  async loadSkillInstructions(name) {
    const skill = this.skillsByName.get(name);
    if (!skill) return null;

    const content = await readFile(skill.skillFilePath, 'utf8');
    return stripFrontmatter(content);
  }

  _resolveSkillDir(skillDir) {
    const resolved = isAbsolute(skillDir) ? resolve(skillDir) : resolve(this.repoRoot, skillDir);
    const repoRoot = this.repoRoot.endsWith(sep) ? this.repoRoot : `${this.repoRoot}${sep}`;

    if (resolved !== this.repoRoot && !resolved.startsWith(repoRoot)) {
      if (this.debug) {
        console.warn(`[skills] Skipping skill dir outside repo: ${resolved}`);
      }
      return null;
    }

    return resolved;
  }

  async _scanSkillDir(dirPath) {
    if (!existsSync(dirPath)) return [];

    let entries;
    try {
      entries = await readdir(dirPath, { withFileTypes: true });
    } catch (error) {
      if (this.debug) {
        console.warn(`[skills] Failed to read skill dir ${dirPath}: ${error.message}`);
      }
      return [];
    }

    const results = [];
    for (const entry of entries) {
      if (!entry.isDirectory()) continue;

      const skillFolder = join(dirPath, entry.name);
      const skillFilePath = join(skillFolder, SKILL_FILE_NAME);
      if (!existsSync(skillFilePath)) continue;

      const skill = await parseSkillFile(skillFilePath, entry.name, { debug: this.debug });
      if (!skill) continue;

      if (this.skillsByName.has(skill.name)) {
        if (this.debug) {
          console.warn(`[skills] Duplicate skill name '${skill.name}' at ${skillFolder}, skipping`);
        }
        continue;
      }

      this.skillsByName.set(skill.name, skill);
      results.push(skill);
    }

    return results;
  }
}

export { DEFAULT_SKILL_DIRS };
