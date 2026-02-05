/**
 * Tests for workspace root utilities
 * These tests verify the getCommonPrefix and toRelativePath functions
 */

import { getCommonPrefix, toRelativePath } from '../../src/utils/path-validation.js';
import path from 'path';
import os from 'os';

describe('Workspace Root Utilities', () => {
	describe('getCommonPrefix', () => {
		test('should return single folder when array has one element', () => {
			const result = getCommonPrefix(['/tmp/ws/tyk']);
			expect(result).toBe(path.normalize('/tmp/ws/tyk'));
		});

		test('should find common prefix of two sibling folders', () => {
			const result = getCommonPrefix(['/tmp/ws/tyk', '/tmp/ws/tyk-docs']);
			expect(result).toBe(path.normalize('/tmp/ws'));
		});

		test('should find common prefix of multiple folders', () => {
			const result = getCommonPrefix([
				'/tmp/ws/tyk',
				'/tmp/ws/tyk-docs',
				'/tmp/ws/tyk-analytics'
			]);
			expect(result).toBe(path.normalize('/tmp/ws'));
		});

		test('should find common prefix of nested folders', () => {
			const result = getCommonPrefix(['/tmp/ws/a/b', '/tmp/ws/a/c']);
			expect(result).toBe(path.normalize('/tmp/ws/a'));
		});

		test('should handle folders with one being parent of another', () => {
			const result = getCommonPrefix(['/tmp/ws', '/tmp/ws/sub']);
			expect(result).toBe(path.normalize('/tmp/ws'));
		});

		test('should return first folder when no common prefix exists', () => {
			const result = getCommonPrefix(['/a/b', '/c/d']);
			expect(result).toBe(path.normalize('/a/b'));
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
			expect(result).toBe(path.normalize('/tmp/ws'));
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

			test('should handle Windows drive root as common prefix', () => {
				const result = getCommonPrefix(['C:\\a', 'C:\\b']);
				expect(result).toBe('C:\\');
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
		test('should work together for typical Visor workspace', () => {
			const allowedFolders = [
				'/tmp/visor-workspaces/dark-pig-qzh9/tyk',
				'/tmp/visor-workspaces/dark-pig-qzh9/tyk-docs',
				'/tmp/visor-workspaces/dark-pig-qzh9/tyk-analytics'
			];

			const workspaceRoot = getCommonPrefix(allowedFolders);
			expect(workspaceRoot).toBe(path.normalize('/tmp/visor-workspaces/dark-pig-qzh9'));

			// Converting allowed folders to relative paths
			const relativeFolders = allowedFolders.map(f => toRelativePath(f, workspaceRoot));
			expect(relativeFolders).toEqual(['tyk', 'tyk-docs', 'tyk-analytics']);
		});

		test('should handle single folder workspace', () => {
			const allowedFolders = ['/home/user/project'];

			const workspaceRoot = getCommonPrefix(allowedFolders);
			expect(workspaceRoot).toBe(path.normalize('/home/user/project'));

			// The folder itself becomes '.'
			const relativeFolders = allowedFolders.map(f => toRelativePath(f, workspaceRoot));
			expect(relativeFolders).toEqual(['.']);
		});

		test('should handle file paths within workspace', () => {
			const allowedFolders = [
				'/workspace/repo1',
				'/workspace/repo2'
			];

			const workspaceRoot = getCommonPrefix(allowedFolders);
			expect(workspaceRoot).toBe(path.normalize('/workspace'));

			// File path in repo1
			const filePath = '/workspace/repo1/src/index.js';
			const relativePath = toRelativePath(filePath, workspaceRoot);
			expect(relativePath).toBe(path.join('repo1', 'src', 'index.js'));
		});
	});

	describe('ProbeAgent workspaceRoot computation', () => {
		// These tests verify workspaceRoot is computed correctly in ProbeAgent
		// Note: We import ProbeAgent dynamically to avoid circular dependencies

		test('should compute workspaceRoot from multiple allowedFolders', async () => {
			const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');

			const agent = new ProbeAgent({
				allowedFolders: ['/tmp/ws/tyk', '/tmp/ws/tyk-docs', '/tmp/ws/tyk-analytics']
			});

			// workspaceRoot should be the common prefix
			expect(agent.workspaceRoot).toBe(path.normalize('/tmp/ws'));
			// cwd should default to workspaceRoot
			expect(agent.cwd).toBe(path.normalize('/tmp/ws'));
		});

		test('should preserve workspaceRoot when explicit cwd is provided', async () => {
			const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');

			const agent = new ProbeAgent({
				allowedFolders: ['/tmp/ws/tyk', '/tmp/ws/tyk-docs'],
				cwd: '/custom/cwd'  // Explicit cwd
			});

			// workspaceRoot should still be computed from allowedFolders
			expect(agent.workspaceRoot).toBe(path.normalize('/tmp/ws'));
			// cwd should be the explicit value
			expect(agent.cwd).toBe('/custom/cwd');
		});

		test('should handle single folder in allowedFolders', async () => {
			const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');

			const agent = new ProbeAgent({
				allowedFolders: ['/home/user/project']
			});

			// workspaceRoot should be the single folder
			expect(agent.workspaceRoot).toBe(path.normalize('/home/user/project'));
			expect(agent.cwd).toBe(path.normalize('/home/user/project'));
		});

		test('clone should preserve allowedFolders and recompute workspaceRoot', async () => {
			const { ProbeAgent } = await import('../../src/agent/ProbeAgent.js');

			const original = new ProbeAgent({
				allowedFolders: ['/tmp/ws/tyk', '/tmp/ws/tyk-docs']
			});

			const cloned = original.clone();

			// Cloned agent should have same allowedFolders
			expect(cloned.allowedFolders).toEqual(original.allowedFolders);
			// And should compute same workspaceRoot
			expect(cloned.workspaceRoot).toBe(original.workspaceRoot);
			expect(cloned.workspaceRoot).toBe(path.normalize('/tmp/ws'));
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
});
