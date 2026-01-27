import { readFile } from 'fs/promises';
import { dirname } from 'path';
import YAML from 'yaml';

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

function normalizeFrontmatter(data) {
  if (!data || typeof data !== 'object' || Array.isArray(data)) return {};
  return data;
}

function deriveSkillName(rawName, directoryName, { debug, skillFilePath }) {
  const candidate = rawName || directoryName;
  if (isValidSkillName(candidate)) return candidate;

  if (rawName && debug) {
    console.warn(`[skills] Invalid skill name '${rawName}' in ${skillFilePath}; falling back to directory name`);
  }

  if (isValidSkillName(directoryName)) {
    return directoryName;
  }

  if (debug) {
    console.warn(`[skills] Invalid directory name '${directoryName}' for skill at ${skillFilePath}; skipping`);
  }

  return null;
}

function deriveDescription(rawDescription, body) {
  let description = rawDescription || '';
  if (!description) {
    description = getFirstParagraph(body);
  }
  return truncateDescription(description);
}

export function stripFrontmatter(content) {
  const { body } = extractFrontmatter(content);
  return body.trim();
}

export async function parseSkillFile(skillFilePath, directoryName, { debug = false } = {}) {
  let content;
  try {
    content = await readFile(skillFilePath, 'utf8');
  } catch (error) {
    if (debug) {
      console.warn(`[skills] Failed to read ${skillFilePath}: ${error.message}`);
    }
    return null;
  }

  const { hasFrontmatter, frontmatterText, body, invalid } = extractFrontmatter(content);
  if (invalid) {
    if (debug) {
      console.warn(`[skills] Invalid frontmatter in ${skillFilePath}; skipping`);
    }
    return null;
  }

  let data = {};
  if (hasFrontmatter) {
    try {
      data = YAML.parse(frontmatterText) || {};
    } catch (error) {
      if (debug) {
        console.warn(`[skills] Invalid YAML in ${skillFilePath}; skipping`);
      }
      return null;
    }
  }

  data = normalizeFrontmatter(data);

  const rawName = typeof data.name === 'string' ? data.name.trim() : '';
  const name = deriveSkillName(rawName, directoryName, { debug, skillFilePath });
  if (!name) return null;

  const rawDescription = typeof data.description === 'string' ? data.description.trim() : '';
  const description = deriveDescription(rawDescription, body);

  return {
    name,
    description,
    skillFilePath,
    directoryName,
    sourceDir: dirname(skillFilePath)
  };
}
