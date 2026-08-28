/**
 * Internal guarded atomic terminal-receipt writer.
 *
 * Semantics are derived from the accepted ReqProof EXP-0164 finalizer at
 * source commit dc1a80476c89699e4d9a4921b6ef5d7f980a3c60. The caller owns a
 * unique attempt directory and a single writer; this adds no public surface.
 * Publication is process-observed atomicity, not power-loss durability for the
 * directory entry: this narrow primitive intentionally does not fsync the directory.
 */

import { constants } from 'fs';
import { lstat, open, readFile, realpath, rename, unlink } from 'fs/promises';
import { basename, dirname, join, resolve } from 'path';

const DEFAULT_NAME = 'receipt.json';
const DEFAULT_MAX_BYTES = 16_384;
const CLEANUP_ATTEMPTS = 3;

function safeName(name) {
  return typeof name === 'string' && /^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(name) &&
    Buffer.byteLength(name) <= 255 && basename(name) === name;
}

async function absent(path) {
  try {
    return { absent: false, info: await lstat(path) };
  } catch (error) {
    if (error?.code === 'ENOENT') return { absent: true, info: null };
    throw error;
  }
}

async function guardDirectory(directory) {
  if (typeof directory !== 'string' || directory !== resolve(directory)) {
    throw new TypeError('directory must be an absolute canonical path');
  }
  const info = await lstat(directory);
  if (!info.isDirectory() || info.isSymbolicLink() || await realpath(directory) !== directory) {
    throw new Error('directory must be an existing canonical nonsymlink directory');
  }
}

function guardChild(path, directory, expectedName) {
  if (dirname(path) !== directory || basename(path) !== expectedName) {
    throw new Error('receipt path must be an exact immediate child');
  }
}

async function requireAbsent(path, label) {
  const observed = await absent(path);
  if (!observed.absent) {
    const kind = observed.info.isSymbolicLink() ? 'symlink' : 'occupied path';
    throw new Error(`${label} ${kind} is forbidden`);
  }
}

function combinedFailure(primary, cleanupErrors, cleanupComplete) {
  if (cleanupErrors.length === 0) return primary;
  const error = new AggregateError(
    [primary, ...cleanupErrors],
    cleanupComplete
      ? 'receipt operation failed after cleanup recovery'
      : 'receipt operation and owned cleanup failed',
    { cause: primary }
  );
  Object.defineProperties(error, {
    primary: { value: primary, enumerable: true },
    cleanupErrors: { value: Object.freeze([...cleanupErrors]), enumerable: true },
    cleanupComplete: { value: cleanupComplete, enumerable: true }
  });
  return error;
}

function measuredBytes(bytes) {
  if (Buffer.isBuffer(bytes) || bytes instanceof Uint8Array) return bytes.byteLength;
  if (typeof bytes === 'string') return Buffer.byteLength(bytes);
  throw new TypeError('bytes must be a Buffer, string, or Uint8Array');
}

export async function writeAtomicTerminalReceipt({
  directory,
  name = DEFAULT_NAME,
  bytes,
  maxBytes = DEFAULT_MAX_BYTES
} = {}) {
  if (!safeName(name) || !safeName(`${name}.tmp`)) {
    throw new TypeError('name must be a safe basename');
  }
  if (!Number.isSafeInteger(maxBytes) || maxBytes <= 0) {
    throw new TypeError('maxBytes must be a positive safe integer');
  }
  const byteLength = measuredBytes(bytes);
  if (byteLength > maxBytes) throw new Error('receipt exceeds maxBytes');

  await guardDirectory(directory);
  const finalPath = join(directory, name);
  const temporaryPath = join(directory, `${name}.tmp`);
  guardChild(finalPath, directory, name);
  guardChild(temporaryPath, directory, `${name}.tmp`);
  await requireAbsent(finalPath, 'final receipt');
  await requireAbsent(temporaryPath, 'temporary receipt');

  const payload = Buffer.from(bytes);
  let handle;
  let createdTemporary = false;
  let renamed = false;
  let result;
  let primary;
  try {
    handle = await open(
      temporaryPath,
      constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL,
      0o600
    );
    createdTemporary = true;
    let offset = 0;
    while (offset < payload.length) {
      const { bytesWritten } = await handle.write(payload, offset, payload.length - offset, offset);
      if (bytesWritten <= 0) throw new Error('receipt write made no progress');
      offset += bytesWritten;
    }
    await handle.sync();
    await handle.chmod(0o600);
    await handle.close();
    handle = undefined;

    await guardDirectory(directory);
    await requireAbsent(finalPath, 'final receipt');
    const temporary = await lstat(temporaryPath);
    if (!temporary.isFile() || temporary.isSymbolicLink() ||
        (temporary.mode & 0o777) !== 0o600 || temporary.size !== byteLength) {
      throw new Error('temporary receipt failed guarded validation');
    }
    await rename(temporaryPath, finalPath);
    renamed = true;

    const info = await lstat(finalPath);
    const actual = await readFile(finalPath);
    if (!info.isFile() || info.isSymbolicLink() || (info.mode & 0o777) !== 0o600 ||
        info.size !== byteLength || actual.length !== byteLength || !actual.equals(payload)) {
      throw new Error('published receipt failed exact validation');
    }
    result = Object.freeze({ bytes: actual, mode: info.mode & 0o777, size: info.size });
  } catch (error) {
    primary = error;
  }

  if (primary) {
    const cleanupErrors = [];
    let cleanupComplete = false;
    let temporaryAbsent = !createdTemporary || renamed;
    for (let attempt = 1; attempt <= CLEANUP_ATTEMPTS && !cleanupComplete; attempt += 1) {
      if (handle) {
        try {
          await handle.close();
          handle = undefined;
        } catch (error) {
          cleanupErrors.push(error);
        }
      }
      if (!handle && createdTemporary && !renamed) {
        try {
          await guardDirectory(directory);
          guardChild(temporaryPath, directory, `${name}.tmp`);
          const temporary = await absent(temporaryPath);
          if (!temporary.absent) {
            if (!temporary.info.isFile() || temporary.info.isSymbolicLink()) {
              throw new Error('owned temporary receipt changed type during cleanup');
            }
            await unlink(temporaryPath);
          }
          if (!(await absent(temporaryPath)).absent) {
            throw new Error('owned temporary receipt remains after cleanup');
          }
          temporaryAbsent = true;
        } catch (error) {
          cleanupErrors.push(error);
          temporaryAbsent = false;
        }
      }
      cleanupComplete = !handle && temporaryAbsent;
    }
    if (!cleanupComplete) cleanupErrors.push(new Error('owned receipt cleanup remained incomplete'));
    throw combinedFailure(primary, cleanupErrors, cleanupComplete);
  }

  return result;
}
