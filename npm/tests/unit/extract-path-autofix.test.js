/**
 * Tests for extract tool path auto-fix when model uses wrong workspace subdirectory.
 *
 * Scenario: search returns relative paths like "gateway/file.go", model constructs
 * wrong absolute path /workspace/gateway/file.go instead of /workspace/tyk/gateway/file.go.
 * The autofix should find the file in the correct allowedFolder subdirectory.
 */

import { mkdtempSync, mkdirSync, writeFileSync, rmSync, existsSync } from 'fs';
import { join, sep } from 'path';
import { tmpdir } from 'os';

// Inline splitTargetSuffix to avoid circular import from vercel.js
function splitTargetSuffix(target) {
	const searchStart = (target.length > 2 && target[1] === ':' && /[a-zA-Z]/.test(target[0])) ? 2 : 0;
	const colonIdx = target.indexOf(':', searchStart);
	const hashIdx = target.indexOf('#');
	if (colonIdx !== -1 && (hashIdx === -1 || colonIdx < hashIdx)) {
		return { filePart: target.substring(0, colonIdx), suffix: target.substring(colonIdx) };
	} else if (hashIdx !== -1) {
		return { filePart: target.substring(0, hashIdx), suffix: target.substring(hashIdx) };
	}
	return { filePart: target, suffix: '' };
}

// Reproduce the autofix logic from vercel.js extractTool for unit testing
function autoFixExtractPaths(extractFiles, effectiveCwd, allowedFolders, debug = false) {
	if (!allowedFolders || allowedFolders.length === 0) return extractFiles;

	return extractFiles.map(target => {
		const { filePart, suffix } = splitTargetSuffix(target);
		if (existsSync(filePart)) return target;

		const cwdPrefix = effectiveCwd.endsWith(sep) ? effectiveCwd : effectiveCwd + sep;
		const relativePart = filePart.startsWith(cwdPrefix)
			? filePart.slice(cwdPrefix.length)
			: null;

		if (relativePart) {
			for (const folder of allowedFolders) {
				const candidate = join(folder, relativePart);
				if (existsSync(candidate)) {
					return candidate + suffix;
				}
			}
		}

		for (const folder of allowedFolders) {
			const folderPrefix = folder.endsWith(sep) ? folder : folder + sep;
			const sepEscaped = sep === '\\' ? '\\\\' : sep;
			const wsParent = folderPrefix.replace(new RegExp('[^' + sepEscaped + ']+' + sepEscaped + '$'), '');
			if (filePart.startsWith(wsParent)) {
				const tail = filePart.slice(wsParent.length);
				const candidate = join(folderPrefix, tail);
				if (candidate !== filePart && existsSync(candidate)) {
					return candidate + suffix;
				}
			}
		}

		return target;
	});
}

describe('Extract path auto-fix', () => {
	let workspace;
	let projectDir;

	beforeAll(() => {
		// Create temp workspace: /tmp/xxx/ (workspace root)
		//                        /tmp/xxx/tyk/gateway/mw_rate_limiting.go
		//                        /tmp/xxx/tyk/internal/rate/limiter.go
		workspace = mkdtempSync(join(tmpdir(), 'probe-test-ws-'));
		projectDir = join(workspace, 'tyk');
		mkdirSync(join(projectDir, 'gateway'), { recursive: true });
		mkdirSync(join(projectDir, 'internal', 'rate'), { recursive: true });
		writeFileSync(join(projectDir, 'gateway', 'mw_rate_limiting.go'), 'package gateway');
		writeFileSync(join(projectDir, 'gateway', 'session_manager.go'), 'package gateway');
		writeFileSync(join(projectDir, 'internal', 'rate', 'limiter.go'), 'package rate');
	});

	afterAll(() => {
		rmSync(workspace, { recursive: true, force: true });
	});

	test('should fix path missing project subdirectory', () => {
		// Model constructs: /workspace/gateway/session_manager.go (wrong)
		// Correct:          /workspace/tyk/gateway/session_manager.go
		const wrongPath = join(workspace, 'gateway', 'session_manager.go');
		const result = autoFixExtractPaths(
			[wrongPath],
			workspace,
			[projectDir]
		);
		expect(result[0]).toBe(join(projectDir, 'gateway', 'session_manager.go'));
	});

	test('should fix path with line number suffix', () => {
		const wrongPath = join(workspace, 'gateway', 'session_manager.go') + ':50-500';
		const result = autoFixExtractPaths(
			[wrongPath],
			workspace,
			[projectDir]
		);
		expect(result[0]).toBe(join(projectDir, 'gateway', 'session_manager.go') + ':50-500');
	});

	test('should fix path with symbol suffix', () => {
		const wrongPath = join(workspace, 'gateway', 'session_manager.go') + '#ForwardMessage';
		const result = autoFixExtractPaths(
			[wrongPath],
			workspace,
			[projectDir]
		);
		expect(result[0]).toBe(join(projectDir, 'gateway', 'session_manager.go') + '#ForwardMessage');
	});

	test('should fix nested paths', () => {
		const wrongPath = join(workspace, 'internal', 'rate', 'limiter.go');
		const result = autoFixExtractPaths(
			[wrongPath],
			workspace,
			[projectDir]
		);
		expect(result[0]).toBe(join(projectDir, 'internal', 'rate', 'limiter.go'));
	});

	test('should not modify correct paths', () => {
		const correctPath = join(projectDir, 'gateway', 'mw_rate_limiting.go');
		const result = autoFixExtractPaths(
			[correctPath],
			workspace,
			[projectDir]
		);
		expect(result[0]).toBe(correctPath);
	});

	test('should handle multiple targets with mixed correct/wrong paths', () => {
		const correct = join(projectDir, 'gateway', 'mw_rate_limiting.go');
		const wrong = join(workspace, 'gateway', 'session_manager.go');
		const result = autoFixExtractPaths(
			[correct, wrong],
			workspace,
			[projectDir]
		);
		expect(result[0]).toBe(correct);
		expect(result[1]).toBe(join(projectDir, 'gateway', 'session_manager.go'));
	});

	test('should keep path unchanged when file not found anywhere', () => {
		const nonexistent = join(workspace, 'nonexistent', 'file.go');
		const result = autoFixExtractPaths(
			[nonexistent],
			workspace,
			[projectDir]
		);
		expect(result[0]).toBe(nonexistent);
	});

	test('should try multiple allowedFolders', () => {
		// Create a second project
		const project2 = join(workspace, 'analytics');
		mkdirSync(join(project2, 'api'), { recursive: true });
		writeFileSync(join(project2, 'api', 'handler.go'), 'package api');

		const wrongPath = join(workspace, 'api', 'handler.go');
		const result = autoFixExtractPaths(
			[wrongPath],
			workspace,
			[projectDir, project2]
		);
		expect(result[0]).toBe(join(project2, 'api', 'handler.go'));

		rmSync(project2, { recursive: true, force: true });
	});
});
