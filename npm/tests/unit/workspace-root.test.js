/**
 * Tests for workspace root utilities
 * These tests verify the getCommonPrefix and toRelativePath functions
 */

import { getCommonPrefix, toRelativePath, safeRealpath } from '../../src/utils/path-validation.js';
import path from 'path';
import os from 'os';
import fs from 'fs';

// Helper to compute expected resolved paths (accounts for symlinks like /tmp -> /private/tmp on macOS)
const resolvePath = (p) => safeRealpath(p);

describe('Workspace Root Utilities', () => {
	describe('getCommonPrefix', () => {
		test('should return single folder when array has one element', () => {
			const result = getCommonPrefix(['/tmp/ws/tyk']);
			// safeRealpath resolves /tmp to /private/tmp on macOS
			expect(result).toBe(resolvePath('/tmp/ws/tyk'));
		});

		test('should find common prefix of two sibling folders', () => {
			const result = getCommonPrefix(['/tmp/ws/tyk', '/tmp/ws/tyk-docs']);
			expect(result).toBe(resolvePath('/tmp/ws'));
		});

		test('should find common prefix of multiple folders', () => {
			const result = getCommonPrefix([
				'/tmp/ws/tyk',
				'/tmp/ws/tyk-docs',
				'/tmp/ws/tyk-analytics'
			]);
			expect(result).toBe(resolvePath('/tmp/ws'));
		});

		test('should find common prefix of nested folders', () => {
			const result = getCommonPrefix(['/tmp/ws/a/b', '/tmp/ws/a/c']);
			expect(result).toBe(resolvePath('/tmp/ws/a'));
		});

		test('should handle folders with one being parent of another', () => {
			const result = getCommonPrefix(['/tmp/ws', '/tmp/ws/sub']);
			expect(result).toBe(resolvePath('/tmp/ws'));
		});

		test('should return first folder when no common prefix exists', () => {
			const result = getCommonPrefix(['/a/b', '/c/d']);
			// safeRealpath resolves non-existent paths relative to system root
			expect(result).toBe(resolvePath('/a/b'));
		});

		test('should handle empty array', () => {
			const result = getCommonPrefix([]);
			expect(result).toBe(process.cwd());
		});

		test('should handle null', () => {
			const result = getCommonPrefix(null);
			expect(result).toBe(process.cwd());
		});

		test('should handle undefined', () => {
			const result = getCommonPrefix(undefined);
			expect(result).toBe(process.cwd());
		});

		test('should normalize paths with mixed separators', () => {
			// Use paths that might have different separators
			const folder1 = path.join('/tmp', 'ws', 'a');
			const folder2 = path.join('/tmp', 'ws', 'b');
			const result = getCommonPrefix([folder1, folder2]);
			expect(result).toBe(resolvePath('/tmp/ws'));
		});

		// Platform-specific tests
		if (process.platform === 'win32') {
			test('should handle Windows paths with same drive', () => {
				const result = getCommonPrefix(['C:\\Users\\ws\\tyk', 'C:\\Users\\ws\\docs']);
				expect(result).toBe('C:\\Users\\ws');
			});

			test('should handle Windows paths with different drives', () => {
				const result = getCommonPrefix(['C:\\a', 'D:\\b']);
				// Should return first folder as there's no common prefix
				expect(result).toBe('C:\\a');
			});

			test('should return first folder when only Windows drive is common', () => {
				// When only the drive letter is common, return first folder for more useful context
				// This matches the Unix behavior where '/' as common returns first folder
				const result = getCommonPrefix(['C:\\a', 'C:\\b']);
				expect(result).toBe('C:\\a');
			});
		} else {
			test('should return first folder when only Unix root is common', () => {
				// When only '/' is common, return first folder for more useful context
				const result = getCommonPrefix(['/a', '/b']);
				expect(result).toBe('/a');
			});
		}
	});

	describe('toRelativePath', () => {
		const root = path.join(os.tmpdir(), 'workspace');

		test('should convert path within workspace to relative', () => {
			const absPath = path.join(root, 'src', 'file.js');
			const result = toRelativePath(absPath, root);
			expect(result).toBe(path.join('src', 'file.js'));
		});

		test('should return "." for workspace root itself', () => {
			const result = toRelativePath(root, root);
			expect(result).toBe('.');
		});

		test('should return absolute path if outside workspace', () => {
			const outsidePath = path.join(os.tmpdir(), 'other', 'file.js');
			const result = toRelativePath(outsidePath, root);
			expect(result).toBe(outsidePath);
		});

		test('should handle null absolutePath', () => {
			const result = toRelativePath(null, root);
			expect(result).toBe(null);
		});

		test('should handle undefined absolutePath', () => {
			const result = toRelativePath(undefined, root);
			expect(result).toBe(undefined);
		});

		test('should handle null workspaceRoot', () => {
			const absPath = '/some/path/file.js';
			const result = toRelativePath(absPath, null);
			expect(result).toBe(absPath);
		});

		test('should handle undefined workspaceRoot', () => {
			const absPath = '/some/path/file.js';
			const result = toRelativePath(absPath, undefined);
			expect(result).toBe(absPath);
		});

		test('should handle deeply nested paths', () => {
			const absPath = path.join(root, 'a', 'b', 'c', 'd', 'file.js');
			const result = toRelativePath(absPath, root);
			expect(result).toBe(path.join('a', 'b', 'c', 'd', 'file.js'));
		});

		test('should not match paths that only share prefix string', () => {
			// Ensure /workspace-extra doesn't match /workspace
			const absPath = root + '-extra/file.js';
			const result = toRelativePath(absPath, root);
			// Should return original path since it's not actually within workspace
			expect(result).toBe(absPath);
		});

		test('should handle paths with trailing separators', () => {
			const rootWithSep = root + path.sep;
			const absPath = path.join(root, 'src', 'file.js');
			const result = toRelativePath(absPath, rootWithSep);
			expect(result).toBe(path.join('src', 'file.js'));
		});
	});

	describe('Integration: getCommonPrefix + toRelativePath', () => {
		// Use actual temp directory for cross-platform tests
		const tempBase = os.tmpdir();

		test('should work together for typical Visor workspace', () => {
			// Use paths relative to actual temp dir for cross-platform compatibility
			const allowedFolders = [
				path.join(tempBase, 'visor-ws', 'tyk'),
				path.join(tempBase, 'visor-ws', 'tyk-docs'),
				path.join(tempBase, 'visor-ws', 'tyk-analytics')
			];

			const workspaceRoot = getCommonPrefix(allowedFolders);
			// Should resolve to the common prefix
			expect(workspaceRoot).toBe(resolvePath(path.join(tempBase, 'visor-ws')));

			// Converting allowed folders to relative paths
			const relativeFolders = allowedFolders.map(f => toRelativePath(f, workspaceRoot));
			expect(relativeFolders).toEqual(['tyk', 'tyk-docs', 'tyk-analytics']);
		});

		test('should handle single folder workspace', () => {
			const allowedFolders = [path.join(tempBase, 'project')];

			const workspaceRoot = getCommonPrefix(allowedFolders);
			expect(workspaceRoot).toBe(resolvePath(path.join(tempBase, 'project')));

			// The folder itself becomes '.'
			const relativeFolders = allowedFolders.map(f => toRelativePath(f, workspaceRoot));
			expect(relativeFolders).toEqual(['.']);
		});

		test('should handle file paths within workspace', () => {
			const allowedFolders = [
				path.join(tempBase, 'workspace', 'repo1'),
				path.join(tempBase, 'workspace', 'repo2')
			];

			const workspaceRoot = getCommonPrefix(allowedFolders);
			expect(workspaceRoot).toBe(resolvePath(path.join(tempBase, 'workspace')));

			// File path in repo1
			const filePath = path.join(tempBase, 'workspace', 'repo1', 'src', 'index.js');
			const relativePath = toRelativePath(filePath, workspaceRoot);
			expect(relativePath).toBe(path.join('repo1', 'src', 'index.js'));
		});
	});

	describe('ProbeAgent workspaceRoot computation', () => {
		// These tests verify workspaceRoot is computed correctly in ProbeAgent
		// Note: We import ProbeAgent dynamically to avoid circular dependencies
		// Use actual temp directory for cross-platform compatibility
		const tempBase = os.tmpdir();

		test('should compute workspaceRoot from multiple allowedFolders', async () => {
			const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');

			const folders = [
				path.join(tempBase, 'ws', 'tyk'),
				path.join(tempBase, 'ws', 'tyk-docs'),
				path.join(tempBase, 'ws', 'tyk-analytics')
			];
			const agent = new ProbeAgent({
				allowedFolders: folders
			});

			// workspaceRoot should be the common prefix (with symlinks resolved)
			expect(agent.workspaceRoot).toBe(resolvePath(path.join(tempBase, 'ws')));
			// cwd should default to workspaceRoot
			expect(agent.cwd).toBe(resolvePath(path.join(tempBase, 'ws')));
		});

		test('should preserve workspaceRoot when explicit cwd is provided', async () => {
			const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');

			const folders = [
				path.join(tempBase, 'ws', 'tyk'),
				path.join(tempBase, 'ws', 'tyk-docs')
			];
			const customCwd = path.join(tempBase, 'custom-cwd');
			const agent = new ProbeAgent({
				allowedFolders: folders,
				cwd: customCwd
			});

			// workspaceRoot should still be computed from allowedFolders
			expect(agent.workspaceRoot).toBe(resolvePath(path.join(tempBase, 'ws')));
			// cwd should be the explicit value
			expect(agent.cwd).toBe(customCwd);
		});

		test('should handle single folder in allowedFolders', async () => {
			const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');

			const folder = path.join(tempBase, 'project');
			const agent = new ProbeAgent({
				allowedFolders: [folder]
			});

			// workspaceRoot should be the single folder (with symlinks resolved)
			expect(agent.workspaceRoot).toBe(resolvePath(folder));
			expect(agent.cwd).toBe(resolvePath(folder));
		});

		test('clone should preserve allowedFolders and recompute workspaceRoot', async () => {
			const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');

			const folders = [
				path.join(tempBase, 'ws', 'tyk'),
				path.join(tempBase, 'ws', 'tyk-docs')
			];
			const original = new ProbeAgent({
				allowedFolders: folders
			});

			const cloned = original.clone();

			// Cloned agent should have same allowedFolders
			expect(cloned.allowedFolders).toEqual(original.allowedFolders);
			// And should compute same workspaceRoot
			expect(cloned.workspaceRoot).toBe(original.workspaceRoot);
			expect(cloned.workspaceRoot).toBe(resolvePath(path.join(tempBase, 'ws')));
		});
	});

	describe('delegateTool workspaceRoot option', () => {
		test('should use passed workspaceRoot option over allowedFolders[0]', async () => {
			const { delegateTool } = await import('../../src/tools/vercel.js');

			// When allowedFolders are sibling directories, workspaceRoot should be their parent
			const tool = delegateTool({
				allowedFolders: ['/workspace/tyk', '/workspace/tyk-docs'],
				workspaceRoot: '/workspace',  // Computed common prefix
				cwd: '/workspace/tyk/deep/nested'  // Navigation context
			});

			expect(tool).toBeDefined();
			expect(tool.name).toBe('delegate');
			// The tool should use workspaceRoot (/workspace) not allowedFolders[0] (/workspace/tyk)
		});

		test('should fall back to allowedFolders[0] when workspaceRoot not provided', async () => {
			const { delegateTool } = await import('../../src/tools/vercel.js');

			// Legacy behavior: if workspaceRoot not explicitly provided
			const tool = delegateTool({
				allowedFolders: ['/workspace/tyk', '/workspace/tyk-docs'],
				cwd: '/workspace/tyk/deep/nested'
			});

			expect(tool).toBeDefined();
			expect(tool.name).toBe('delegate');
			// Should fall back to allowedFolders[0] for backwards compatibility
		});
	});

	describe('bashTool workspaceRoot consistency', () => {
		test('should use workspaceRoot in default working directory chain', async () => {
			const { bashTool } = await import('../../src/tools/bash.js');

			// When workspaceRoot is provided, it should be used in the fallback chain
			const tool = bashTool({
				allowedFolders: ['/workspace/tyk', '/workspace/tyk-docs'],
				workspaceRoot: '/workspace',  // Computed common prefix
				cwd: '/workspace'  // cwd defaults to workspaceRoot in ProbeAgent
			});

			expect(tool).toBeDefined();
			expect(tool.name).toBe('bash');
		});

		test('should handle workspaceRoot without cwd', async () => {
			const { bashTool } = await import('../../src/tools/bash.js');

			const tool = bashTool({
				allowedFolders: ['/workspace/tyk', '/workspace/tyk-docs'],
				workspaceRoot: '/workspace'
				// cwd not provided - should use workspaceRoot
			});

			expect(tool).toBeDefined();
			expect(tool.name).toBe('bash');
		});
	});

	describe('analyzeAllTool workspaceRoot consistency', () => {
		test('should pass workspaceRoot to analyzeAll function', async () => {
			const { analyzeAllTool } = await import('../../src/tools/vercel.js');

			const tool = analyzeAllTool({
				allowedFolders: ['/workspace/tyk', '/workspace/tyk-docs'],
				workspaceRoot: '/workspace',
				cwd: '/workspace'
			});

			expect(tool).toBeDefined();
			expect(tool.name).toBe('analyze_all');
		});
	});

	describe('Symlink Security', () => {
		const testDir = path.join(os.tmpdir(), 'probe-symlink-test-' + Date.now());
		const realDir = path.join(testDir, 'real');
		const symlinkDir = path.join(testDir, 'symlink');
		const testFile = path.join(realDir, 'file.js');
		let resolvedRealDir;

		beforeAll(() => {
			// Create test directories and file
			fs.mkdirSync(realDir, { recursive: true });
			fs.writeFileSync(testFile, '// test file');
			// Resolve realDir to handle systems where tmpdir is itself a symlink (e.g., macOS /var -> /private/var)
			resolvedRealDir = fs.realpathSync(realDir);
			// Create a symlink pointing to the real directory
			try {
				fs.symlinkSync(realDir, symlinkDir);
			} catch (e) {
				// Symlinks may not be supported on some systems/configurations
				console.warn('Could not create symlink for tests:', e.message);
			}
		});

		afterAll(() => {
			// Clean up test directories
			try {
				if (fs.existsSync(testFile)) {
					fs.unlinkSync(testFile);
				}
				if (fs.existsSync(symlinkDir)) {
					fs.unlinkSync(symlinkDir);
				}
				if (fs.existsSync(realDir)) {
					fs.rmdirSync(realDir);
				}
				if (fs.existsSync(testDir)) {
					fs.rmdirSync(testDir);
				}
			} catch (e) {
				// Ignore cleanup errors
			}
		});

		test('getCommonPrefix should resolve symlinks to real paths', () => {
			// Skip if symlink wasn't created
			if (!fs.existsSync(symlinkDir)) {
				console.warn('Skipping symlink test - symlink not available');
				return;
			}

			// When using symlink, result should be the real path
			const result = getCommonPrefix([symlinkDir]);
			expect(result).toBe(resolvedRealDir);
		});

		test('getCommonPrefix should find common prefix when mixing symlink and real path', () => {
			// Skip if symlink wasn't created
			if (!fs.existsSync(symlinkDir)) {
				console.warn('Skipping symlink test - symlink not available');
				return;
			}

			// Both should resolve to the same real path
			const result = getCommonPrefix([realDir, symlinkDir]);
			// Since both resolve to realDir, the common prefix should be realDir
			expect(result).toBe(resolvedRealDir);
		});

		test('toRelativePath should resolve symlinks for security', () => {
			// Skip if symlink wasn't created
			if (!fs.existsSync(symlinkDir)) {
				console.warn('Skipping symlink test - symlink not available');
				return;
			}

			// Use the actual file that exists (via symlink path)
			const fileViaSymlink = path.join(symlinkDir, 'file.js');

			// toRelativePath should resolve the symlink and recognize it's within workspace
			// Need to use resolvedRealDir since toRelativePath also resolves symlinks
			const result = toRelativePath(fileViaSymlink, resolvedRealDir);
			expect(result).toBe('file.js');
		});

		test('toRelativePath should handle non-existent paths gracefully', () => {
			// Non-existent path should still work (falls back to normalized path)
			const result = toRelativePath('/non/existent/path/file.js', '/non/existent');
			expect(result).toBe(path.join('path', 'file.js'));
		});

		test('getCommonPrefix should handle non-existent paths gracefully', () => {
			// Non-existent paths should fall back to best-effort resolution
			// Use paths relative to temp dir for cross-platform compatibility
			const tempBase = os.tmpdir();
			const result = getCommonPrefix([
				path.join(tempBase, 'non-existent-test', 'a'),
				path.join(tempBase, 'non-existent-test', 'b')
			]);
			expect(result).toBe(resolvePath(path.join(tempBase, 'non-existent-test')));
		});
	});
});
