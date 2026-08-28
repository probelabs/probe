import { describe, expect, jest, test } from '@jest/globals';
import { lstat, mkdir, mkdtemp, readFile, realpath, rmdir, symlink, unlink, writeFile } from 'fs/promises';
import { tmpdir } from 'os';
import { basename, dirname, join } from 'path';
import { writeAtomicTerminalReceipt } from '../../src/agent/governance/atomicTerminalReceipt.js';

async function absent(path) {
  try { return { absent: false, info: await lstat(path) }; }
  catch (error) { if (error?.code === 'ENOENT') return { absent: true, info: null }; throw error; }
}

async function guardTestRoot(directory) {
  const info = await lstat(directory);
  if (!info.isDirectory() || info.isSymbolicLink() || await realpath(directory) !== directory) {
    throw new Error('test receipt root lost its canonical nonsymlink boundary');
  }
}

async function cleanupExactDirectory(directory) {
  const names = ['receipt.json.tmp', 'receipt.json'];
  await guardTestRoot(directory);
  for (const name of names) {
    await guardTestRoot(directory);
    const path = join(directory, name);
    if (dirname(path) !== directory || basename(path) !== name) throw new Error('invalid test cleanup child');
    const observed = await absent(path);
    if (!observed.absent) {
      if (!observed.info.isFile() || observed.info.isSymbolicLink()) {
        throw new Error(`refusing nonregular test cleanup child: ${name}`);
      }
      await unlink(path);
    }
    if (!(await absent(path)).absent) throw new Error(`test cleanup left residue: ${name}`);
  }
  await guardTestRoot(directory);
  await rmdir(directory);
  if (!(await absent(directory)).absent) throw new Error('test cleanup left root residue');
}

async function withCanonicalDirectory(run) {
  const created = await mkdtemp(join(tmpdir(), 'probe-atomic-receipt-'));
  const directory = await realpath(created);
  try { await run(directory); }
  finally { await cleanupExactDirectory(directory); }
}

async function isAbsent(path) {
  return (await absent(path)).absent;
}

describe('writeAtomicTerminalReceipt', () => {
  test('publishes exact bytes with mode 0600 and removes only the temporary file', async () => {
    await withCanonicalDirectory(async directory => {
      const bytes = Buffer.from('{"classification":"PASS","schema":"test/v1"}\n');
      const result = await writeAtomicTerminalReceipt({ directory, bytes });
      const finalPath = join(directory, 'receipt.json');

      expect(result).toMatchObject({ size: bytes.length, mode: 0o600 });
      expect(result.bytes.equals(bytes)).toBe(true);
      expect((await readFile(finalPath)).equals(bytes)).toBe(true);
      expect((await lstat(finalPath)).mode & 0o777).toBe(0o600);
      expect(await isAbsent(join(directory, 'receipt.json.tmp'))).toBe(true);
    });
  });

  test('enforces the cap before Buffer.from allocation', async () => {
    await withCanonicalDirectory(async directory => {
      const fromSpy = jest.spyOn(Buffer, 'from');
      try {
        await expect(writeAtomicTerminalReceipt({
          directory,
          bytes: 'x'.repeat(1024 * 1024),
          maxBytes: 16
        })).rejects.toThrow('maxBytes');
        expect(fromSpy).not.toHaveBeenCalled();
      } finally {
        fromSpy.mockRestore();
      }
      expect(await isAbsent(join(directory, 'receipt.json'))).toBe(true);
      expect(await isAbsent(join(directory, 'receipt.json.tmp'))).toBe(true);
    });
  });

  test.each(['../receipt.json', '/receipt.json', '.', '..'])('rejects escaping name %s', async name => {
    await withCanonicalDirectory(async directory => {
      await expect(writeAtomicTerminalReceipt({ directory, name, bytes: '{}' })).rejects.toThrow('safe basename');
      expect(await isAbsent(join(directory, 'receipt.json.tmp'))).toBe(true);
    });
  });

  test('rejects a symlink directory and symlink target boundaries', async () => {
    await withCanonicalDirectory(async directory => {
      const realChild = join(directory, 'real-child');
      const linkedChild = join(directory, 'linked-child');
      await mkdir(realChild);
      await symlink(realChild, linkedChild);
      try {
        await expect(writeAtomicTerminalReceipt({ directory: linkedChild, bytes: '{}' }))
          .rejects.toThrow('canonical nonsymlink');

        const target = join(directory, 'receipt.json');
        await symlink(join(directory, 'missing'), target);
        await expect(writeAtomicTerminalReceipt({ directory, bytes: '{}' })).rejects.toThrow('symlink');
        await unlink(target);

        const temporary = join(directory, 'receipt.json.tmp');
        await symlink(join(directory, 'missing'), temporary);
        await expect(writeAtomicTerminalReceipt({ directory, bytes: '{}' })).rejects.toThrow('symlink');
        await unlink(temporary);
      } finally {
        await guardTestRoot(directory);
        const linked = await absent(linkedChild);
        if (!linked.absent) {
          if (!linked.info.isSymbolicLink()) throw new Error('linked-child changed type');
          await unlink(linkedChild);
        }
        const real = await absent(realChild);
        if (!real.absent) {
          if (!real.info.isDirectory() || real.info.isSymbolicLink()) throw new Error('real-child changed type');
          await rmdir(realChild);
        }
      }
    });
  });

  test('fails closed on occupied final or temporary paths without deleting foreign bytes', async () => {
    await withCanonicalDirectory(async directory => {
      const finalPath = join(directory, 'receipt.json');
      const temporaryPath = join(directory, 'receipt.json.tmp');
      await writeFile(finalPath, 'occupied-final');
      await expect(writeAtomicTerminalReceipt({ directory, bytes: '{}' })).rejects.toThrow('occupied path');
      expect(await readFile(finalPath, 'utf8')).toBe('occupied-final');
      expect(await isAbsent(temporaryPath)).toBe(true);

      await unlink(finalPath);
      await writeFile(temporaryPath, 'occupied-temporary');
      await expect(writeAtomicTerminalReceipt({ directory, bytes: '{}' })).rejects.toThrow('occupied path');
      expect(await readFile(temporaryPath, 'utf8')).toBe('occupied-temporary');
      await unlink(temporaryPath);

      await writeAtomicTerminalReceipt({ directory, bytes: '{}' });
      expect(await isAbsent(temporaryPath)).toBe(true);
    });
  });

  test.each(['open', 'write', 'fsync', 'chmod', 'close', 'rename', 'lstat'])(
    'cleans owned descriptor and temporary residue after injected %s failure',
    async stage => {
      const actualFs = jest.requireActual('fs/promises');
      let handleClosed = false;
      let closeCalls = 0;
      let renamed = false;
      jest.resetModules();
      jest.unstable_mockModule('fs/promises', () => ({
        ...actualFs,
        open: async (...args) => {
          if (stage === 'open') throw new Error('injected open failure');
          const handle = await actualFs.open(...args);
          let closeInjected = false;
          return {
            write: async (...writeArgs) => {
              if (stage === 'write') throw new Error('injected write failure');
              return handle.write(...writeArgs);
            },
            sync: async () => {
              if (stage === 'fsync') throw new Error('injected fsync failure');
              return handle.sync();
            },
            chmod: async mode => {
              if (stage === 'chmod') throw new Error('injected chmod failure');
              return handle.chmod(mode);
            },
            close: async () => {
              closeCalls += 1;
              if (stage === 'close' && !closeInjected) {
                closeInjected = true;
                throw new Error('injected close failure');
              }
              await handle.close();
              handleClosed = true;
            }
          };
        },
        rename: async (...args) => {
          if (stage === 'rename') throw new Error('injected rename failure');
          await actualFs.rename(...args);
          renamed = true;
        },
        lstat: async path => {
          if (stage === 'lstat' && renamed && path.endsWith('/receipt.json')) {
            throw new Error('injected lstat failure');
          }
          return actualFs.lstat(path);
        }
      }));

      try {
        const { writeAtomicTerminalReceipt: injectedWriter } =
          await import('../../src/agent/governance/atomicTerminalReceipt.js');
        await withCanonicalDirectory(async directory => {
          await expect(injectedWriter({ directory, bytes: '{"id":1}\n' }))
            .rejects.toThrow(`injected ${stage} failure`);
          expect(await isAbsent(join(directory, 'receipt.json.tmp'))).toBe(true);
          expect(await isAbsent(join(directory, 'receipt.json'))).toBe(stage !== 'lstat');
          expect(handleClosed).toBe(stage !== 'open');
          expect(closeCalls).toBe(stage === 'open' ? 0 : stage === 'close' ? 2 : 1);
        });
      } finally {
        jest.unmock('fs/promises');
        jest.resetModules();
      }
    }
  );

  test.each(['close', 'unlink'])(
    'retains a write primary while surfacing and recovering a first cleanup %s failure',
    async cleanupStage => {
      const actualFs = jest.requireActual('fs/promises');
      let closeCalls = 0;
      let unlinkCalls = 0;
      let handleClosed = false;
      jest.resetModules();
      jest.unstable_mockModule('fs/promises', () => ({
        ...actualFs,
        open: async (...args) => {
          const handle = await actualFs.open(...args);
          return {
            write: async () => { throw new Error('injected primary write failure'); },
            sync: handle.sync.bind(handle),
            chmod: handle.chmod.bind(handle),
            close: async () => {
              closeCalls += 1;
              if (cleanupStage === 'close' && closeCalls === 1) {
                throw new Error('injected cleanup close failure');
              }
              await handle.close();
              handleClosed = true;
            }
          };
        },
        unlink: async path => {
          unlinkCalls += 1;
          if (cleanupStage === 'unlink' && unlinkCalls === 1) {
            throw new Error('injected cleanup unlink failure');
          }
          return actualFs.unlink(path);
        }
      }));

      try {
        const { writeAtomicTerminalReceipt: injectedWriter } =
          await import('../../src/agent/governance/atomicTerminalReceipt.js');
        await withCanonicalDirectory(async directory => {
          let failure;
          try { await injectedWriter({ directory, bytes: '{"id":1}\n' }); }
          catch (error) { failure = error; }
          expect(failure).toBeInstanceOf(AggregateError);
          expect(failure.primary.message).toBe('injected primary write failure');
          expect(failure.cause).toBe(failure.primary);
          expect(failure.errors[0]).toBe(failure.primary);
          expect(failure.cleanupErrors).toHaveLength(1);
          expect(failure.cleanupErrors[0].message).toBe(`injected cleanup ${cleanupStage} failure`);
          expect(failure.cleanupComplete).toBe(true);
          expect(handleClosed).toBe(true);
          expect(closeCalls).toBe(cleanupStage === 'close' ? 2 : 1);
          expect(unlinkCalls).toBe(cleanupStage === 'unlink' ? 2 : 1);
          expect(await isAbsent(join(directory, 'receipt.json.tmp'))).toBe(true);
          expect(await isAbsent(join(directory, 'receipt.json'))).toBe(true);
        });
      } finally {
        jest.unmock('fs/promises');
        jest.resetModules();
      }
    }
  );

  test('reports every bounded cleanup failure and incomplete residue explicitly', async () => {
    const actualFs = jest.requireActual('fs/promises');
    let handleClosed = false;
    let unlinkCalls = 0;
    jest.resetModules();
    jest.unstable_mockModule('fs/promises', () => ({
      ...actualFs,
      open: async (...args) => {
        const handle = await actualFs.open(...args);
        return {
          write: async () => { throw new Error('persistent-case primary write failure'); },
          sync: handle.sync.bind(handle),
          chmod: handle.chmod.bind(handle),
          close: async () => {
            await handle.close();
            handleClosed = true;
          }
        };
      },
      unlink: async () => {
        unlinkCalls += 1;
        throw new Error(`persistent cleanup unlink failure ${unlinkCalls}`);
      }
    }));

    try {
      const { writeAtomicTerminalReceipt: injectedWriter } =
        await import('../../src/agent/governance/atomicTerminalReceipt.js');
      await withCanonicalDirectory(async directory => {
        let failure;
        try { await injectedWriter({ directory, bytes: '{"id":1}\n' }); }
        catch (error) { failure = error; }
        expect(failure).toBeInstanceOf(AggregateError);
        expect(failure.primary.message).toBe('persistent-case primary write failure');
        expect(failure.cleanupComplete).toBe(false);
        expect(failure.cleanupErrors.map(error => error.message)).toEqual([
          'persistent cleanup unlink failure 1',
          'persistent cleanup unlink failure 2',
          'persistent cleanup unlink failure 3',
          'owned receipt cleanup remained incomplete'
        ]);
        expect(handleClosed).toBe(true);
        expect(unlinkCalls).toBe(3);
        const temporaryPath = join(directory, 'receipt.json.tmp');
        expect(await isAbsent(temporaryPath)).toBe(false);
        await guardTestRoot(directory);
        const temporary = await lstat(temporaryPath);
        expect(temporary.isFile() && !temporary.isSymbolicLink()).toBe(true);
        await actualFs.unlink(temporaryPath);
        expect(await isAbsent(temporaryPath)).toBe(true);
      });
    } finally {
      jest.unmock('fs/promises');
      jest.resetModules();
    }
  });
});
