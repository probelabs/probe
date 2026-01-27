import { existsSync } from 'fs';
import { readdir, readFile, realpath, lstat } from 'fs/promises';
import { resolve, join, isAbsolute, sep, relative } from 'path';
import { parseSkillFile, stripFrontmatter } from './parser.js';

const DEFAULT_SKILL_DIRS = ['.claude/skills', '.codex/skills', 'skills', '.skills'];
const SKILL_FILE_NAME = 'SKILL.md';

function isPathInside(basePath, targetPath) {
  const base = resolve(basePath);
  const target = resolve(targetPath);
  const rel = relative(base, target);
  if (rel === '') return true;
  if (rel === '..' || rel.startsWith(`..${sep}`)) return false;
  if (isAbsolute(rel)) return false;
  return true;
}

function isSafeEntryName(name) {
  if (!name || name === '.' || name === '..') return false;
  if (name.includes('\0')) return false;
  return !name.includes('/') && !name.includes('\\');
}

export class SkillRegistry {
  constructor({ repoRoot, skillDirs = DEFAULT_SKILL_DIRS, debug = false } = {}) {
    this.repoRoot = repoRoot ? resolve(repoRoot) : process.cwd();
    this.repoRootReal = null;
    this.skillDirs = Array.isArray(skillDirs) && skillDirs.length > 0 ? skillDirs : DEFAULT_SKILL_DIRS;
    this.debug = debug;
    this.skills = [];
    this.skillsByName = new Map();
    this.loadErrors = [];
    this.loaded = false;
  }

  async loadSkills() {
    if (this.loaded) return this.skills;

    this.loadErrors = [];
    this.repoRootReal = await this._resolveRealPath(this.repoRoot);
    if (!this.repoRootReal) {
      if (this.debug) {
        console.warn(`[skills] Failed to resolve repo root: ${this.repoRoot}`);
      }
      this.loaded = true;
      return this.skills;
    }

    const discovered = [];
    for (const skillDir of this.skillDirs) {
      const resolvedDir = await this._resolveSkillDir(skillDir);
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

  getLoadErrors() {
    return this.loadErrors;
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

  async _resolveRealPath(target) {
    try {
      return await realpath(target);
    } catch (_error) {
      return null;
    }
  }

  async _resolveSkillDir(skillDir) {
    const resolved = isAbsolute(skillDir) ? resolve(skillDir) : resolve(this.repoRoot, skillDir);
    const repoRoot = this.repoRootReal || resolve(this.repoRoot);
    const resolvedReal = await this._resolveRealPath(resolved);
    if (!resolvedReal) return null;

    if (!isPathInside(repoRoot, resolvedReal)) {
      if (this.debug) {
        console.warn(`[skills] Skipping skill dir outside repo: ${resolvedReal}`);
      }
      return null;
    }

    return resolvedReal;
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
      if (!isSafeEntryName(entry.name)) {
        if (this.debug) {
          console.warn(`[skills] Skipping unsafe skill directory name: ${entry.name}`);
        }
        continue;
      }

      const skillFolder = join(dirPath, entry.name);
      const skillFilePath = join(skillFolder, SKILL_FILE_NAME);
      let skillStat;
      try {
        skillStat = await lstat(skillFilePath);
      } catch (_error) {
        continue;
      }

      if (skillStat.isSymbolicLink()) {
        if (this.debug) {
          console.warn(`[skills] Skipping symlinked SKILL.md: ${skillFilePath}`);
        }
        continue;
      }

      const resolvedSkillPath = await this._resolveRealPath(skillFilePath);
      if (!resolvedSkillPath || !isPathInside(dirPath, resolvedSkillPath)) {
        if (this.debug) {
          console.warn(`[skills] Skipping skill path outside directory: ${resolvedSkillPath || skillFilePath}`);
        }
        continue;
      }
      if (!existsSync(skillFilePath)) continue;

      const { skill, error } = await parseSkillFile(skillFilePath, entry.name);
      if (!skill) {
        if (error) {
          this.loadErrors.push({
            path: skillFilePath,
            code: error.code,
            message: error.message
          });
        }
        if (this.debug && error) {
          console.warn(`[skills] Skipping ${skillFilePath}: ${error.code} (${error.message})`);
        }
        continue;
      }

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
