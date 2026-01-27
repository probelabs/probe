import { mkdtemp, writeFile, rm } from 'fs/promises';
import { join } from 'path';
import { tmpdir } from 'os';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

// Tests for architecture context inclusion and fallback behavior

describe('ProbeAgent architecture context', () => {
  let tempDir;

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'probe-arch-test-'));
  });

  afterEach(async () => {
    if (tempDir) {
      await rm(tempDir, { recursive: true, force: true });
    }
  });

  test('includes ARCHITECTURE.md content from repo root (case-insensitive match)', async () => {
    const content = 'Architecture overview for tests.';
    await writeFile(join(tempDir, 'architecture.md'), content, 'utf8');

    const agent = new ProbeAgent({
      path: tempDir,
      debug: false
    });

    const systemMessage = await agent.getSystemMessage();
    expect(systemMessage).toContain('# Architecture');
    expect(systemMessage).toContain(content);
  });

  test('includes AGENTS.md and ARCHITECTURE.md when both are present', async () => {
    const agentsContent = 'Agents guidance for tests.';
    const architectureContent = 'Architecture overview that should be included.';
    await writeFile(join(tempDir, 'AGENTS.md'), agentsContent, 'utf8');
    await writeFile(join(tempDir, 'ARCHITECTURE.md'), architectureContent, 'utf8');

    const agent = new ProbeAgent({
      path: tempDir,
      debug: false
    });

    const systemMessage = await agent.getSystemMessage();
    expect(systemMessage).toContain(agentsContent);
    expect(systemMessage).toContain(architectureContent);
  });

  test('prefers AGENTS.md over CLAUDE.md when both are present', async () => {
    const agentsContent = 'Agents guidance takes precedence.';
    const claudeContent = 'Claude guidance should be skipped.';
    await writeFile(join(tempDir, 'AGENTS.md'), agentsContent, 'utf8');
    await writeFile(join(tempDir, 'CLAUDE.md'), claudeContent, 'utf8');

    const agent = new ProbeAgent({
      path: tempDir,
      debug: false
    });

    const systemMessage = await agent.getSystemMessage();
    expect(systemMessage).toContain(agentsContent);
    expect(systemMessage).not.toContain(claudeContent);
  });

  test('falls back to CLAUDE.md when AGENTS.md is missing', async () => {
    const content = 'Claude guidance for tests.';
    await writeFile(join(tempDir, 'CLAUDE.md'), content, 'utf8');

    const agent = new ProbeAgent({
      path: tempDir,
      debug: false
    });

    const systemMessage = await agent.getSystemMessage();
    expect(systemMessage).toContain(content);
  });

  test('falls back to CLAUDE.md when architectureFileName is agents.md', async () => {
    const content = 'Claude guidance for explicit agents fallback.';
    await writeFile(join(tempDir, 'CLAUDE.md'), content, 'utf8');

    const agent = new ProbeAgent({
      path: tempDir,
      architectureFileName: 'agents.md',
      debug: false
    });

    const systemMessage = await agent.getSystemMessage();
    expect(systemMessage).toContain(content);
  });

  test('uses configured architectureFileName when provided', async () => {
    const content = 'Custom architecture content.';
    await writeFile(join(tempDir, 'DESIGN.md'), content, 'utf8');

    const agent = new ProbeAgent({
      path: tempDir,
      architectureFileName: 'design.md',
      debug: false
    });

    const systemMessage = await agent.getSystemMessage();
    expect(systemMessage).toContain('# Architecture');
    expect(systemMessage).toContain(content);
  });

  test('includes ARCHITECTURE.md alongside custom architectureFileName', async () => {
    const designContent = 'Custom design guidance.';
    const architectureContent = 'Architecture overview still included.';
    await writeFile(join(tempDir, 'design.md'), designContent, 'utf8');
    await writeFile(join(tempDir, 'ARCHITECTURE.md'), architectureContent, 'utf8');

    const agent = new ProbeAgent({
      path: tempDir,
      architectureFileName: 'design.md',
      debug: false
    });

    const systemMessage = await agent.getSystemMessage();
    expect(systemMessage).toContain(designContent);
    expect(systemMessage).toContain(architectureContent);
  });
});
