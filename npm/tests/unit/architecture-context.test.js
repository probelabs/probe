import { mkdtemp, writeFile, rm } from 'fs/promises';
import { join } from 'path';
import { tmpdir } from 'os';
import { ProbeAgent } from '../../src/agent/ProbeAgent.js';

// Tests for ARCHITECTURE.md context inclusion

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
});
