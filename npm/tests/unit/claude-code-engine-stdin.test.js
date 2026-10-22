/**
 * Claude Code engine: the prompt travels on stdin, not argv.
 *
 * Regression for `spawn E2BIG`: the engine used to run
 *   sh -c "echo '' | claude -p '<escaped prompt>' ..."
 * so a large prompt (e.g. a code review over a 194-file diff) overflowed
 * ARG_MAX / MAX_ARG_STRLEN and every call failed before claude started.
 *
 * These tests put a fake `claude` (a tiny node script) first on PATH and
 * check that a 2 MB prompt arrives intact on stdin with the same CLI flags
 * as before (minus the positional prompt).
 */

import { describe, test, expect, beforeAll, afterAll } from '@jest/globals';
import { spawn } from 'child_process';
import { createHash } from 'crypto';
import { mkdtemp, writeFile, chmod, readFile, unlink, rmdir, mkdir } from 'fs/promises';
import { existsSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import {
  createEnhancedClaudeCLIEngine,
  buildClaudeArgs,
  spawnClaude
} from '../../src/agent/engines/enhanced-claude-code.js';

const FAKE_CLAUDE = `#!/usr/bin/env node
const fs = require('fs');
const argsFile = process.env.FAKE_CLAUDE_ARGS;
fs.writeFileSync(argsFile, JSON.stringify({
  argv: process.argv.slice(2),
  forceColor: process.env.FORCE_COLOR
}));
if (process.env.FAKE_CLAUDE_EXIT_EARLY === '1') {
  // Exit without reading stdin: the parent's write hits EPIPE.
  process.exit(3);
}
const chunks = [];
process.stdin.on('data', (c) => chunks.push(c));
process.stdin.on('end', () => {
  const input = Buffer.concat(chunks);
  fs.writeFileSync(process.env.FAKE_CLAUDE_STDIN, input);
  process.stdout.write(JSON.stringify({
    type: 'result',
    result: 'fake-ok bytes=' + input.length,
    session_id: 'fake-session-1'
  }) + '\\n');
});
`;

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');

// ~2 MB prompt full of shell metacharacters, quotes, newlines and non-ASCII,
// so any shell interpretation or truncation would show up.
function makeLargePrompt(targetBytes = 2 * 1024 * 1024) {
  const unit = "Review this diff: it's \"quoted\" $(rm nothing) `x` ; | & > < * ? \\ \n" +
    'naïve café 日本語 🚀 line\n';
  return unit.repeat(Math.ceil(targetBytes / Buffer.byteLength(unit, 'utf8')));
}

/** The pre-fix command line, rebuilt verbatim (for the before/after check). */
function legacyShellCommand(args, prompt) {
  const legacyArgs = [args[0], prompt, ...args.slice(1)];
  return `echo "" | claude ${legacyArgs.map(arg => `'${arg.replace(/'/g, "'\\''")}'`).join(' ')}`;
}

describe('enhanced Claude Code engine: prompt on stdin', () => {
  let root;
  let binDir;
  let emptyDir;
  let argsFile;
  let stdinFile;
  let originalPath;
  const created = [];

  beforeAll(async () => {
    root = await mkdtemp(join(tmpdir(), 'probe-claude-stdin-'));
    binDir = join(root, 'bin');
    emptyDir = join(root, 'empty');
    await mkdir(binDir);
    await mkdir(emptyDir);
    const fake = join(binDir, 'claude');
    await writeFile(fake, FAKE_CLAUDE);
    await chmod(fake, 0o755);
    argsFile = join(root, 'args.json');
    stdinFile = join(root, 'stdin.bin');
    created.push(fake, argsFile, stdinFile);
    originalPath = process.env.PATH;
  });

  afterAll(async () => {
    process.env.PATH = originalPath;
    delete process.env.FAKE_CLAUDE_ARGS;
    delete process.env.FAKE_CLAUDE_STDIN;
    delete process.env.FAKE_CLAUDE_EXIT_EARLY;
    // Non-recursive cleanup: remove the known files, then the empty dirs.
    for (const f of created) {
      if (existsSync(f)) await unlink(f);
    }
    for (const d of [binDir, emptyDir, root]) {
      if (d && existsSync(d)) await rmdir(d);
    }
  });

  function useFakeClaude({ exitEarly = false } = {}) {
    process.env.PATH = `${binDir}:${originalPath}`;
    process.env.FAKE_CLAUDE_ARGS = argsFile;
    process.env.FAKE_CLAUDE_STDIN = stdinFile;
    if (exitEarly) process.env.FAKE_CLAUDE_EXIT_EARLY = '1';
    else delete process.env.FAKE_CLAUDE_EXIT_EARLY;
  }

  async function collect(engine, prompt) {
    const out = [];
    for await (const chunk of engine.query(prompt)) out.push(chunk);
    return out;
  }

  // Minimal agent: enough for the built-in MCP server to start, so the
  // engine builds exactly the args it builds in production (--mcp-config).
  const fakeAgent = () => ({
    toolImplementations: {},
    allowedTools: { isEnabled: () => true }
  });

  test('a 2 MB prompt arrives intact on stdin with the same flags as before', async () => {
    useFakeClaude();
    const prompt = makeLargePrompt();
    const systemPrompt = "You are a reviewer. Don't 'break' $HOME.";
    const engine = await createEnhancedClaudeCLIEngine({
      agent: fakeAgent(),
      systemPrompt,
      timeout: 60000
    });

    try {
      const chunks = await collect(engine, prompt);

      const received = await readFile(stdinFile);
      expect(received.length).toBe(Buffer.byteLength(prompt, 'utf8'));
      expect(sha256(received)).toBe(sha256(Buffer.from(prompt, 'utf8')));

      const { argv, forceColor } = JSON.parse(await readFile(argsFile, 'utf8'));
      expect(forceColor).toBe('0');
      // Old argv was: -p <prompt> --output-format json --system-prompt ...
      // New argv is identical with the positional prompt removed.
      expect(argv).toEqual([
        '-p',
        '--output-format', 'json',
        '--system-prompt', systemPrompt,
        '--mcp-config', expect.stringMatching(/probe-mcp-.*\.json$/),
        '--allowedTools', 'mcp__probe__*'
      ]);
      expect(argv.some(a => a.includes('Review this diff'))).toBe(false);

      // Output parsing is unchanged.
      const text = chunks.filter(c => c.type === 'text').map(c => c.content).join('');
      expect(text).toBe(`fake-ok bytes=${received.length}`);
      const meta = chunks.find(c => c.type === 'metadata');
      expect(meta.data.conversationId).toBe('fake-session-1');
      expect(chunks.some(c => c.type === 'error')).toBe(false);
    } finally {
      await engine.close();
    }
  }, 60000);

  test('the old sh -c echo-pipe command fails with E2BIG for the same prompt; the new spawn does not', async () => {
    useFakeClaude();
    const prompt = makeLargePrompt();
    const args = buildClaudeArgs({
      systemPrompt: 'sys',
      mcpConfigPath: '/tmp/probe-mcp-x.json',
      session: { getResumeArgs: () => [] },
      debug: false
    });

    const shellCmd = legacyShellCommand(args, prompt);
    const newArgvBytes = args.reduce((n, a) => n + Buffer.byteLength(a, 'utf8') + 1, 0);
    const oldArgvBytes = Buffer.byteLength(shellCmd, 'utf8');
    // Before: the whole prompt (plus escaping) is one argv string.
    expect(oldArgvBytes).toBeGreaterThan(Buffer.byteLength(prompt, 'utf8'));
    // After: argv is a few hundred bytes regardless of prompt size.
    expect(newArgvBytes).toBeLessThan(1024);

    const legacyError = await new Promise((resolve) => {
      let proc;
      try {
        proc = spawn('sh', ['-c', shellCmd], { stdio: ['ignore', 'pipe', 'pipe'] });
      } catch (e) {
        resolve(e);
        return;
      }
      proc.on('error', resolve);
      proc.on('close', () => resolve(null));
    });
    expect(legacyError).not.toBeNull();
    expect(legacyError.code).toBe('E2BIG');

    const { proc } = spawnClaude(args, prompt);
    const result = await new Promise((resolve) => {
      let out = '';
      proc.stdout.on('data', (d) => { out += d; });
      proc.on('error', (e) => resolve({ error: e }));
      proc.on('close', (code) => resolve({ code, out }));
    });
    expect(result.error).toBeUndefined();
    expect(result.code).toBe(0);
    expect(JSON.parse(result.out.trim()).result).toBe(`fake-ok bytes=${Buffer.byteLength(prompt, 'utf8')}`);
  }, 60000);

  test('claude exiting before reading stdin (EPIPE) does not raise an unhandled error', async () => {
    useFakeClaude({ exitEarly: true });
    const engine = await createEnhancedClaudeCLIEngine({
      agent: fakeAgent(),
      systemPrompt: 'sys',
      timeout: 60000
    });
    try {
      const chunks = await collect(engine, makeLargePrompt());
      // Non-zero exit with no output: no text, no thrown error, metadata still yielded.
      expect(chunks.filter(c => c.type === 'text')).toHaveLength(0);
      expect(chunks.some(c => c.type === 'metadata')).toBe(true);
    } finally {
      await engine.close();
    }
  }, 60000);

  test('a missing claude binary reports "Claude Code not found"', async () => {
    useFakeClaude();
    process.env.PATH = emptyDir;
    const engine = await createEnhancedClaudeCLIEngine({
      agent: fakeAgent(),
      systemPrompt: 'sys',
      timeout: 60000
    });
    try {
      const chunks = await collect(engine, 'hello');
      const err = chunks.find(c => c.type === 'error');
      expect(err).toBeDefined();
      expect(err.error.message).toBe('Claude Code not found. Please install it first.');
    } finally {
      process.env.PATH = originalPath;
      await engine.close();
    }
  }, 60000);
});
