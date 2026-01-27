import { existsSync } from 'fs';
import { readdir, readFile } from 'fs/promises';
import { resolve, join, isAbsolute, sep, dirname } from 'path';
import YAML from 'yaml';

const DEFAULT_SKILL_DIRS = ['.claude/skills', '.codex/skills', 'skills', '.skills'];
const SKILL_FILE_NAME = 'SKILL.md';
const SKILL_NAME_REGEX = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const MAX_SKILL_NAME_LENGTH = 64;
const MAX_DESCRIPTION_CHARS = 400;

function isValidSkillName(name) {
  if (!name || typeof name !== 'string') return false;
  if (name.length > MAX_SKILL_NAME_LENGTH) return false;
  return SKILL_NAME_REGEX.test(name);
}

function getFirstParagraph(text) {
  const lines = text.split(/\r?\n/);
  const paragraphLines = [];

  for (const line of lines) {
    if (line.trim() === '') {
      if (paragraphLines.length > 0) {
        break;
      }
      continue;
    }

    paragraphLines.push(line.trim());
  }

  return paragraphLines.join(' ').trim();
}

function extractFrontmatter(content) {
  const trimmed = content.replace(/^\uFEFF/, '');
  const lines = trimmed.split(/\r?\n/);

  if (lines.length === 0 || lines[0].trim() !== '---') {
    return { hasFrontmatter: false, frontmatterText: '', body: trimmed };
  }

  let endIndex = -1;
  for (let i = 1; i < lines.length; i++) {
    if (lines[i].trim() === '---') {
      endIndex = i;
      break;
    }
  }

  if (endIndex === -1) {
    return { hasFrontmatter: true, invalid: true, frontmatterText: '', body: '' };
  }

  const frontmatterText = lines.slice(1, endIndex).join('\n');
  const body = lines.slice(endIndex + 1).join('\n');

  return { hasFrontmatter: true, frontmatterText, body };
}

function truncateDescription(text) {
  if (!text) return '';
  const trimmed = text.trim();
  if (trimmed.length <= MAX_DESCRIPTION_CHARS) return trimmed;
  return `${trimmed.slice(0, MAX_DESCRIPTION_CHARS - 3)}...`;
}

function escapeXml(value) {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

export function formatAvailableSkillsXml(skills) {
  if (!skills || skills.length === 0) return '';

  const lines = ['<available_skills>'];
  for (const skill of skills) {
    lines.push('  <skill>');
    lines.push(`    <name>${escapeXml(skill.name)}</name>`);
    lines.push(`    <description>${escapeXml(skill.description || '')}</description>`);
    lines.push('  </skill>');
  }
  lines.push('</available_skills>');

  return lines.join('\n');
}

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
    const { body } = extractFrontmatter(content);
    return body.trim();
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

      const skill = await this._parseSkillFile(skillFilePath, entry.name);
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

  async _parseSkillFile(skillFilePath, directoryName) {
    let content;
    try {
      content = await readFile(skillFilePath, 'utf8');
    } catch (error) {
      if (this.debug) {
        console.warn(`[skills] Failed to read ${skillFilePath}: ${error.message}`);
      }
      return null;
    }

    const { hasFrontmatter, frontmatterText, body, invalid } = extractFrontmatter(content);
    if (invalid) {
      if (this.debug) {
        console.warn(`[skills] Invalid frontmatter in ${skillFilePath}; skipping`);
      }
      return null;
    }

    let data = {};
    if (hasFrontmatter) {
      try {
        data = YAML.parse(frontmatterText) || {};
      } catch (error) {
        if (this.debug) {
          console.warn(`[skills] Invalid YAML in ${skillFilePath}; skipping`);
        }
        return null;
      }
    }

    if (!data || typeof data !== 'object' || Array.isArray(data)) {
      data = {};
    }

    const rawName = typeof data.name === 'string' ? data.name.trim() : '';
    let name = rawName || directoryName;

    if (!isValidSkillName(name)) {
      if (rawName && this.debug) {
        console.warn(`[skills] Invalid skill name '${rawName}' in ${skillFilePath}; falling back to directory name`);
      }
      if (isValidSkillName(directoryName)) {
        name = directoryName;
      } else {
        if (this.debug) {
          console.warn(`[skills] Invalid directory name '${directoryName}' for skill at ${skillFilePath}; skipping`);
        }
        return null;
      }
    }

    let description = typeof data.description === 'string' ? data.description.trim() : '';
    if (!description) {
      description = getFirstParagraph(body);
    }

    description = truncateDescription(description);

    return {
      name,
      description,
      skillFilePath,
      directoryName,
      sourceDir: dirname(skillFilePath)
    };
  }
}

export { DEFAULT_SKILL_DIRS };
