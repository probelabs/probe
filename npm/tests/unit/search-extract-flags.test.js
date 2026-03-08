import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const searchModulePath = resolve(__dirname, '../../src/search.js');
const extractModulePath = resolve(__dirname, '../../src/extract.js');
const utilsModulePath = resolve(__dirname, '../../src/utils.js');
const pathValidationModulePath = resolve(__dirname, '../../src/utils/path-validation.js');
const errorTypesModulePath = resolve(__dirname, '../../src/utils/error-types.js');

const mockExecFile = jest.fn();
const mockExec = jest.fn();
const mockSpawn = jest.fn();
const mockGetBinaryPath = jest.fn(async () => '/mock/probe');
const mockValidateCwdPath = jest.fn(async () => '/tmp');

const mockBuildCliArgs = jest.fn((options, flagMap) => {
  const cliArgs = [];
  for (const [key, flag] of Object.entries(flagMap)) {
    if (!(key in options)) continue;
    const value = options[key];
    if (typeof value === 'boolean') {
      if (value) cliArgs.push(flag);
    } else if (Array.isArray(value)) {
      for (const item of value) {
        cliArgs.push(flag, item);
      }
    } else if (value !== undefined && value !== null) {
      cliArgs.push(flag, value.toString());
    }
  }
  return cliArgs;
});

jest.unstable_mockModule('child_process', () => ({
  execFile: mockExecFile,
  exec: mockExec,
  spawn: mockSpawn
}));

jest.unstable_mockModule('util', () => ({
  promisify: jest.fn((fn) => fn)
}));

jest.unstable_mockModule(utilsModulePath, () => ({
  getBinaryPath: mockGetBinaryPath,
  buildCliArgs: mockBuildCliArgs,
  escapeString: (value) => value
}));

jest.unstable_mockModule(pathValidationModulePath, () => ({
  validateCwdPath: mockValidateCwdPath
}));

jest.unstable_mockModule(errorTypesModulePath, () => ({
  TimeoutError: class TimeoutError extends Error {},
  categorizeError: (error) => error
}));

const { search } = await import(searchModulePath);
const { extract } = await import(extractModulePath);

describe('search/extract format and lsp flags', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockGetBinaryPath.mockResolvedValue('/mock/probe');
    mockValidateCwdPath.mockResolvedValue('/tmp');
  });

  test('search passes --format and --lsp together', async () => {
    mockExecFile.mockResolvedValue({
      stdout: 'Total bytes returned: 0\nTotal tokens returned: 0\n',
      stderr: ''
    });

    await search({
      path: '/repo',
      query: 'needle',
      format: 'outline',
      lsp: true,
      timeout: 5,
      maxTokens: 100
    });

    const [, args] = mockExecFile.mock.calls[0];
    expect(args).toEqual(expect.arrayContaining(['--format', 'outline', '--lsp']));
  });

  test('extract passes --format and --lsp together', async () => {
    mockExec.mockResolvedValue({
      stdout: '{"results":[]}',
      stderr: ''
    });

    await extract({
      files: ['src/file.js'],
      format: 'json',
      lsp: true
    });

    const [command] = mockExec.mock.calls[0];
    expect(command).toContain(' --format json');
    expect(command).toContain(' --lsp');
  });
});
