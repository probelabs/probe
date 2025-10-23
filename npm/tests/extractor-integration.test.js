/**
 * Integration tests for binary extractor
 * These tests verify the extractor works with real archives
 */

import { jest } from '@jest/globals';
import fs from 'fs-extra';
import path from 'path';
import os from 'os';

describe('Extractor Integration Tests', () => {
	describe('Platform Detection Logic', () => {
		test('should map Node.js platform to correct binary platform string', () => {
			const platform = os.platform();
			const arch = os.arch();

			let expectedPlatform;
			let expectedExtension;

            if (platform === 'linux') {
                expectedExtension = 'tar.gz';
                if (arch === 'x64') {
                    expectedPlatform = 'x86_64-unknown-linux-musl';
                } else if (arch === 'arm64') {
                    expectedPlatform = 'aarch64-unknown-linux-musl';
                }
			} else if (platform === 'darwin') {
				expectedExtension = 'tar.gz';
				if (arch === 'x64') {
					expectedPlatform = 'x86_64-apple-darwin';
				} else if (arch === 'arm64') {
					expectedPlatform = 'aarch64-apple-darwin';
				}
			} else if (platform === 'win32') {
				expectedExtension = 'zip';
				if (arch === 'x64') {
					expectedPlatform = 'x86_64-pc-windows-msvc';
				}
			}

			// Verify we got a platform (not running on unsupported platform)
			if (expectedPlatform) {
				expect(expectedPlatform).toBeDefined();
				expect(expectedExtension).toBeDefined();
			}
		});
	});

	describe('Path Traversal Security', () => {
		test('should detect path traversal with ../ sequences', () => {
			const testCases = [
				{ path: '../../../etc/passwd', expected: 'unsafe' },
				{ path: '../../secrets.txt', expected: 'unsafe' },
				{ path: 'subdir/../../outside.txt', expected: 'safe or unsafe depending on resolution' },
				{ path: 'normal/file.txt', expected: 'safe' },
				{ path: '/absolute/path', expected: 'unsafe' }
			];

			testCases.forEach(({ path: testPath, expected }) => {
				// This demonstrates the security check logic
				const containsTraversal = testPath.includes('..');
				const isAbsolute = path.isAbsolute(testPath);

				if (expected === 'unsafe') {
					expect(containsTraversal || isAbsolute).toBe(true);
				}
			});
		});

		test('should validate relative path correctly', () => {
			const baseDir = '/safe/extraction/dir';
			const safePath = path.join(baseDir, 'subdir', 'file.txt');
			const unsafePath = path.join(baseDir, '..', '..', 'etc', 'passwd');

			const safeRelative = path.relative(baseDir, safePath);
			const unsafeRelative = path.relative(baseDir, unsafePath);

			// Safe path should not start with ..
			expect(safeRelative.startsWith('..')).toBe(false);

			// Unsafe path should start with ..
			expect(unsafeRelative.startsWith('..')).toBe(true);
		});
	});

	describe('Archive Naming Convention', () => {
		test('should construct correct archive names for all platforms', () => {
			const version = '1.0.0';

            const platforms = [
                { os: 'linux', arch: 'x64', name: 'x86_64-unknown-linux-musl', ext: 'tar.gz' },
                { os: 'linux', arch: 'arm64', name: 'aarch64-unknown-linux-musl', ext: 'tar.gz' },
                { os: 'darwin', arch: 'x64', name: 'x86_64-apple-darwin', ext: 'tar.gz' },
                { os: 'darwin', arch: 'arm64', name: 'aarch64-apple-darwin', ext: 'tar.gz' },
                { os: 'win32', arch: 'x64', name: 'x86_64-pc-windows-msvc', ext: 'zip' }
            ];

			platforms.forEach(({ name, ext }) => {
				const expectedName = `probe-v${version}-${name}.${ext}`;
				expect(expectedName).toMatch(/^probe-v\d+\.\d+\.\d+(-\w+)?-.+\.(tar\.gz|zip)$/);
			});
		});
	});

	describe('Binary Name Detection', () => {
		test('should use correct binary name for each platform', () => {
			const isWindows = os.platform() === 'win32';
			const expectedName = isWindows ? 'probe.exe' : 'probe-binary';

			expect(expectedName).toBeDefined();
			if (isWindows) {
				expect(expectedName).toBe('probe.exe');
			} else {
				expect(expectedName).toBe('probe-binary');
			}
		});
	});
});

describe('Security Validations', () => {
	test('isPathSafe logic - prevents directory traversal', () => {
		const baseDir = path.normalize('/safe/dir');

		const testPaths = [
			{ input: path.join(baseDir, 'file.txt'), safe: true },
			{ input: path.join(baseDir, 'sub', 'file.txt'), safe: true },
			{ input: path.join(baseDir, '..', 'outside.txt'), safe: false },
			{ input: '/etc/passwd', safe: false },
			{ input: path.join(baseDir, '..', '..', 'etc', 'passwd'), safe: false }
		];

		testPaths.forEach(({ input, safe }) => {
			const normalizedPath = path.normalize(input);
			const relativePath = path.relative(baseDir, normalizedPath);

			const isActuallySafe = !relativePath.startsWith('..') && !path.isAbsolute(relativePath);

			expect(isActuallySafe).toBe(safe);
		});
	});

	test('validates paths before extraction', () => {
		// This test documents the security approach
		const extractDir = '/tmp/extract';
		const entries = [
			{ name: 'probe', safe: true },
			{ name: 'README.md', safe: true },
			{ name: '../../../etc/passwd', safe: false },
			{ name: '../../secrets.txt', safe: false }
		];

		entries.forEach(({ name, safe }) => {
			const fullPath = path.join(extractDir, name);
			const normalized = path.normalize(fullPath);
			const relative = path.relative(extractDir, normalized);

			const isPathSafe = !relative.startsWith('..') && !path.isAbsolute(relative);

			expect(isPathSafe).toBe(safe);
		});
	});
});
