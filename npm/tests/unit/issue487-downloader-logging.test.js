import { jest } from '@jest/globals';
import { fileURLToPath } from 'url';
import { dirname, resolve } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const downloaderPath = resolve(__dirname, '../../src/downloader.js');
const directoryResolverPath = resolve(__dirname, '../../src/directory-resolver.js');
const symlinkUtilsPath = resolve(__dirname, '../../src/utils/symlink-utils.js');

async function loadDownloaderWithMocks({ fsMock, axiosMock, tarMock, getEntryTypeMock }) {
  jest.resetModules();

  jest.unstable_mockModule('fs-extra', () => ({
    default: fsMock
  }));

  jest.unstable_mockModule('axios', () => ({
    default: axiosMock
  }));

  jest.unstable_mockModule('tar', () => ({
    default: tarMock
  }));

  jest.unstable_mockModule(directoryResolverPath, () => ({
    getPackageBinDir: jest.fn().mockResolvedValue('/tmp/probe-issue-487')
  }));

  jest.unstable_mockModule(symlinkUtilsPath, () => ({
    getEntryType: getEntryTypeMock
  }));

  return import(downloaderPath);
}

describe('Issue #487 - downloader lock visibility', () => {
  test('logs cross-process lock wait and completion even without DEBUG/VERBOSE', async () => {
    const fsMock = {
      writeFile: jest.fn(async (_path, _content, opts) => {
        if (opts?.flag === 'wx') {
          const err = new Error('lock exists');
          err.code = 'EEXIST';
          throw err;
        }
      }),
      readFile: jest.fn(async () => JSON.stringify({ pid: 777, timestamp: Date.now() })),
      pathExists: jest
        .fn()
        .mockResolvedValueOnce(false)
        .mockResolvedValueOnce(true)
    };

    const { downloadProbeBinary } = await loadDownloaderWithMocks({
      fsMock,
      axiosMock: { get: jest.fn(), isAxiosError: () => false },
      tarMock: { extract: jest.fn() },
      getEntryTypeMock: jest.fn()
    });

    await downloadProbeBinary('1.2.3');

    expect(console.log).toHaveBeenCalledWith(
      expect.stringContaining('Download in progress by process')
    );
    expect(console.log).toHaveBeenCalledWith(
      expect.stringContaining('Binary now available')
    );
  });

  test('logs in-process wait when a same-version download is already running', async () => {
    let firstAssetRequest = true;

    const fsMock = {
      pathExists: jest.fn().mockResolvedValue(false),
      ensureDir: jest.fn().mockResolvedValue(undefined),
      writeFile: jest.fn(async (_path, _content, opts) => {
        if (opts?.flag === 'wx') {
          const err = new Error('permissions');
          err.code = 'EACCES';
          throw err;
        }
      }),
      readFile: jest.fn(),
      remove: jest.fn().mockResolvedValue(undefined),
      readdir: jest.fn().mockResolvedValue([
        {
          name: 'probe-binary',
          isFile: () => true,
          isDirectory: () => false
        }
      ]),
      copyFile: jest.fn().mockResolvedValue(undefined),
      chmod: jest.fn().mockResolvedValue(undefined)
    };

    const axiosMock = {
      get: jest.fn(async (_url, opts) => {
        if (opts?.responseType === 'arraybuffer') {
          if (firstAssetRequest) {
            firstAssetRequest = false;
            await new Promise(resolve => setTimeout(resolve, 30));
          }
          return { data: Buffer.from('fake-binary') };
        }

        throw new Error('no checksum');
      }),
      isAxiosError: () => false
    };

    const { downloadProbeBinary } = await loadDownloaderWithMocks({
      fsMock,
      axiosMock,
      tarMock: { extract: jest.fn().mockResolvedValue(undefined) },
      getEntryTypeMock: jest.fn().mockResolvedValue({
        isFile: true,
        isDirectory: false,
        size: 11
      })
    });

    const first = downloadProbeBinary('9.9.9');
    await Promise.resolve();
    const second = downloadProbeBinary('9.9.9');

    await Promise.all([first, second]);

    expect(console.log).toHaveBeenCalledWith(
      expect.stringContaining('Download already in progress in this process')
    );
  });
});
