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
import { EventEmitter } from 'events';
import { PassThrough } from 'stream';
import { createHash } from 'crypto';
import { mkdtemp, writeFile, chmod, readFile, unlink, rmdir, mkdir } from 'fs/promises';
import { existsSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import {
  createEnhancedClaudeCLIEngine,
  buildClaudeArgs,
  spawnClaude,
  resolveClaudeLaunch,
  buildClaudeSpawn,
  quoteCmdArg,
  CLAUDE_MAX_STDIN_CHARS
} from '../../src/agent/engines/enhanced-claude-code.js';

const FAKE_CLAUDE = `#!/usr/bin/env node
const fs = require('fs');
const argsFile = process.env.FAKE_CLAUDE_ARGS;
const argv = process.argv.slice(2);
const spIdx = argv.indexOf('--system-prompt-file');
let systemPromptFile = null;
if (spIdx !== -1) {
  const file = argv[spIdx + 1];
  const content = fs.readFileSync(file);
  systemPromptFile = {
    path: file,
    mode: (fs.statSync(file).mode & 0o777).toString(8),
    sha256: require('crypto').createHash('sha256').update(content).digest('hex')
  };
}
fs.writeFileSync(argsFile, JSON.stringify({ argv, forceColor: process.env.FORCE_COLOR, systemPromptFile }));
if (process.env.FAKE_CLAUDE_EXIT_EARLY === '1') {
  // Exit without reading stdin (the parent's write hits EPIPE), the way
  // Claude Code fails on oversized stdin.
  process.stderr.write('Error: piped stdin input exceeds 10MB. Pass large content as a file path in your prompt instead.\\n');
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

  test('claude exiting non-zero before reading stdin (EPIPE) yields an error chunk with the stderr tail, not an empty answer', async () => {
    useFakeClaude({ exitEarly: true });
    const engine = await createEnhancedClaudeCLIEngine({
      agent: fakeAgent(),
      systemPrompt: 'sys',
      timeout: 60000
    });
    try {
      const chunks = await collect(engine, makeLargePrompt());
      // Non-zero exit with no output: an error chunk (so ProbeAgent.answer
      // throws) instead of an empty "successful" answer; nothing unhandled.
      expect(chunks.filter(c => c.type === 'text')).toHaveLength(0);
      const err = chunks.find(c => c.type === 'error');
      expect(err).toBeDefined();
      expect(err.error.message).toContain('exited with code 3');
      expect(err.error.message).toContain('piped stdin input exceeds 10MB');
    } finally {
      await engine.close();
    }
  }, 60000);

  test('a prompt over the 10MB stdin cap is rejected before spawning', async () => {
    useFakeClaude();
    if (existsSync(argsFile)) await unlink(argsFile);
    const engine = await createEnhancedClaudeCLIEngine({
      agent: fakeAgent(),
      systemPrompt: 'sys',
      timeout: 60000
    });
    try {
      const chunks = await collect(engine, 'y'.repeat(CLAUDE_MAX_STDIN_CHARS + 1));
      expect(chunks).toHaveLength(1);
      expect(chunks[0].type).toBe('error');
      expect(chunks[0].error.message).toContain(String(CLAUDE_MAX_STDIN_CHARS));
      expect(existsSync(argsFile)).toBe(false); // claude was never started
    } finally {
      await engine.close();
    }
  }, 60000);

  test('a large system prompt goes through a 0600 --system-prompt-file that is deleted afterwards', async () => {
    useFakeClaude();
    const systemPrompt = 'Rules: 100% "strict" review.\n'.repeat(8000); // ~230 KB, over Linux MAX_ARG_STRLEN
    const engine = await createEnhancedClaudeCLIEngine({
      agent: fakeAgent(),
      systemPrompt,
      timeout: 60000
    });
    try {
      const chunks = await collect(engine, 'review please');
      expect(chunks.some(c => c.type === 'error')).toBe(false);
      const { argv, systemPromptFile } = JSON.parse(await readFile(argsFile, 'utf8'));
      expect(argv).not.toContain('--system-prompt');
      expect(argv.slice(0, 3)).toEqual(['-p', '--output-format', 'json']);
      expect(argv[3]).toBe('--system-prompt-file');
      expect(argv.join(' ').length).toBeLessThan(1024);
      expect(systemPromptFile.mode).toBe('600');
      expect(systemPromptFile.sha256).toBe(sha256(Buffer.from(systemPrompt, 'utf8')));
      expect(existsSync(systemPromptFile.path)).toBe(false);
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

describe('Windows launch (no shell)', () => {
  const args = ['-p', '--output-format', 'json', '--system-prompt-file', 'C:\\Temp\\probe sys.txt',
    '--mcp-config', 'C:\\Temp\\probe-mcp-1.json', '--allowedTools', 'mcp__probe__*'];

  const fsFrom = (files) => ({
    fileExists: (p) => Object.prototype.hasOwnProperty.call(files, p),
    readFile: (p) => files[p]
  });

  function mockSpawn() {
    const calls = [];
    const impl = (file, argv, options) => {
      calls.push({ file, argv, options });
      const proc = new EventEmitter();
      proc.stdin = new PassThrough();
      proc.stdout = new PassThrough();
      proc.stderr = new PassThrough();
      proc.stdinData = '';
      proc.stdin.on('data', (d) => { proc.stdinData += d; });
      return proc;
    };
    return { calls, impl };
  }

  test('prefers claude.exe on PATH and spawns it directly with the args array', () => {
    const launch = resolveClaudeLaunch({
      platform: 'win32',
      env: { Path: 'C:\\Windows;C:\\Users\\me\\.local\\bin' },
      ...fsFrom({ 'C:\\Users\\me\\.local\\bin\\claude.exe': '' })
    });
    const { calls, impl } = mockSpawn();
    const { proc } = spawnClaude(args, 'prompt on stdin', { launch, spawnImpl: impl });
    expect(calls).toHaveLength(1);
    expect(calls[0].file).toBe('C:\\Users\\me\\.local\\bin\\claude.exe');
    expect(calls[0].argv).toEqual(args);
    expect(calls[0].options.shell).toBe(false);
    expect(calls[0].options.windowsVerbatimArguments).toBeUndefined();
    expect(calls[0].options.stdio).toEqual(['pipe', 'pipe', 'pipe']);
    expect(calls[0].options.env.FORCE_COLOR).toBe('0');
    expect(proc.stdinData).toBe('prompt on stdin');
  });

  test('for the npm claude.cmd shim, runs node on the package bin entry with the same args (no shell)', () => {
    const prefix = 'C:\\Users\\me\\AppData\\Roaming\\npm';
    const pkg = `${prefix}\\node_modules\\@anthropic-ai\\claude-code`;
    const launch = resolveClaudeLaunch({
      platform: 'win32',
      env: { PATH: `C:\\Windows;${prefix}` },
      execPath: 'C:\\Program Files\\nodejs\\node.exe',
      ...fsFrom({
        [`${prefix}\\claude`]: '',
        [`${prefix}\\claude.cmd`]: '',
        [`${pkg}\\package.json`]: JSON.stringify({ bin: { claude: 'cli.js' } }),
        [`${pkg}\\cli.js`]: ''
      })
    });
    const spawned = buildClaudeSpawn(launch, args);
    expect(spawned.file).toBe('C:\\Program Files\\nodejs\\node.exe');
    expect(spawned.argv).toEqual([`${pkg}\\cli.js`, ...args]);
    expect(spawned.options.shell).toBe(false);
    expect(launch.viaCmd).toBe(false);
  });

  test('for a claude.cmd shim whose package bin is the native bin/claude.exe (2.1.x), spawns that exe directly', () => {
    const prefix = 'C:\\Users\\me\\AppData\\Roaming\\npm';
    const pkg = `${prefix}\\node_modules\\@anthropic-ai\\claude-code`;
    const launch = resolveClaudeLaunch({
      platform: 'win32',
      env: { PATH: prefix },
      ...fsFrom({
        [`${prefix}\\claude.cmd`]: '',
        [`${pkg}\\package.json`]: JSON.stringify({ bin: { claude: 'bin/claude.exe' } }),
        [`${pkg}\\bin\\claude.exe`]: ''
      })
    });
    const spawned = buildClaudeSpawn(launch, args);
    expect(spawned.file).toBe(`${pkg}\\bin\\claude.exe`);
    expect(spawned.argv).toEqual(args);
    expect(spawned.options.shell).toBe(false);
  });

  test('when only claude.cmd resolves, uses cmd.exe /d /s /c with verbatim, escaped arguments', () => {
    const prefix = 'C:\\tools\\npm';
    const launch = resolveClaudeLaunch({
      platform: 'win32',
      env: { PATH: prefix, ComSpec: 'C:\\Windows\\system32\\cmd.exe' },
      ...fsFrom({ [`${prefix}\\claude.cmd`]: '' })
    });
    expect(launch.viaCmd).toBe(true);
    const spawned = buildClaudeSpawn(launch, ['-p', '--allowedTools', 'a&b', '100% "x"']);
    expect(spawned.file).toBe('C:\\Windows\\system32\\cmd.exe');
    expect(spawned.argv.slice(0, 3)).toEqual(['/d', '/s', '/c']);
    expect(spawned.options).toEqual({ shell: false, windowsVerbatimArguments: true, windowsHide: true });
    const line = spawned.argv[3];
    expect(line.startsWith('"C:\\tools\\npm\\claude.cmd ')).toBe(true);
    // & and % are ^-escaped (twice, for cmd.exe and the batch re-parse)
    expect(line).toContain('a^^^&b');
    expect(line).toContain('100^^^%');
    expect(line).not.toMatch(/[^^]&/);
    // newlines can never reach the cmd.exe command line
    expect(() => quoteCmdArg('line1\nline2')).toThrow(/newline/);
  });

  test('reports nothing found (null) only after the PATH lookup misses', () => {
    expect(resolveClaudeLaunch({ platform: 'win32', env: { PATH: 'C:\\a;C:\\b' }, ...fsFrom({}) })).toBeNull();
    expect(resolveClaudeLaunch({ platform: 'linux', env: {}, ...fsFrom({}) }))
      .toEqual({ platform: 'linux', command: 'claude', prefixArgs: [], viaCmd: false });
  });
});
