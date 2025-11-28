/**
 * Binary extractor for bundled probe binaries
 * @module extractor
 */

import fs from 'fs-extra';
import path from 'path';
import tar from 'tar';
import AdmZip from 'adm-zip';
import os from 'os';
import { fileURLToPath } from 'url';
import { getEntryType } from './utils/symlink-utils.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const BINARY_NAME = "probe";

/**
 * Detects the current OS and architecture
 * @returns {Object} Object containing OS and architecture information
 */
function detectPlatform() {
	const osType = os.platform();
	const archType = os.arch();

	let platform;
	let extension;

	// Map to the same format used in release artifacts
    if (osType === 'linux') {
        if (archType === 'x64') {
            platform = 'x86_64-unknown-linux-musl';
            extension = 'tar.gz';
        } else if (archType === 'arm64') {
            platform = 'aarch64-unknown-linux-musl';
            extension = 'tar.gz';
        } else {
            throw new Error(`Unsupported Linux architecture: ${archType}`);
        }
	} else if (osType === 'darwin') {
		if (archType === 'x64') {
			platform = 'x86_64-apple-darwin';
			extension = 'tar.gz';
		} else if (archType === 'arm64') {
			platform = 'aarch64-apple-darwin';
			extension = 'tar.gz';
		} else {
			throw new Error(`Unsupported macOS architecture: ${archType}`);
		}
	} else if (osType === 'win32') {
		if (archType === 'x64') {
			platform = 'x86_64-pc-windows-msvc';
			extension = 'zip';
		} else {
			throw new Error(`Unsupported Windows architecture: ${archType}`);
		}
	} else {
		throw new Error(`Unsupported operating system: ${osType}`);
	}

	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Detected platform: ${platform}`);
	}

	return { platform, extension, osType, archType };
}

/**
 * Validates that a path is within a base directory (prevents path traversal)
 * @param {string} filePath - Path to validate
 * @param {string} baseDir - Base directory that filePath must be within
 * @returns {boolean} True if path is safe
 */
function isPathSafe(filePath, baseDir) {
	const normalizedBase = path.normalize(baseDir);
	const normalizedPath = path.normalize(filePath);
	const relativePath = path.relative(normalizedBase, normalizedPath);

	// Path is safe if it doesn't start with '..' and isn't absolute
	return !relativePath.startsWith('..') && !path.isAbsolute(relativePath);
}

/**
 * Extracts a tar.gz archive using the tar library
 * @param {string} archivePath - Path to the .tar.gz file
 * @param {string} extractDir - Directory to extract to
 */
async function extractTarGz(archivePath, extractDir) {
	await tar.extract({
		file: archivePath,
		cwd: extractDir,
		// Security: Prevent path traversal attacks
		onentry: (entry) => {
			const fullPath = path.join(extractDir, entry.path);
			if (!isPathSafe(fullPath, extractDir)) {
				throw new Error(`Path traversal attempt detected: ${entry.path}`);
			}
		}
	});
}

/**
 * Extracts a zip archive using adm-zip library
 * @param {string} archivePath - Path to the .zip file
 * @param {string} extractDir - Directory to extract to
 */
async function extractZip(archivePath, extractDir) {
	const zip = new AdmZip(archivePath);
	const zipEntries = zip.getEntries();

	// Extract each entry with path validation
	for (const entry of zipEntries) {
		const outputPath = path.join(extractDir, entry.entryName);

		// Security: Validate path to prevent traversal attacks
		if (!isPathSafe(outputPath, extractDir)) {
			throw new Error(`Path traversal attempt detected: ${entry.entryName}`);
		}

		if (entry.isDirectory) {
			await fs.ensureDir(outputPath);
		} else {
			await fs.ensureDir(path.dirname(outputPath));
			await fs.writeFile(outputPath, entry.getData());
		}
	}
}

/**
 * Finds the binary file in the extracted directory
 * @param {string} dir - Directory to search
 * @param {string} baseDir - Base directory for path validation
 * @param {string} binaryName - Name of binary to find
 * @param {boolean} isWindows - Whether running on Windows
 * @returns {Promise<string|null>} Path to binary or null
 */
async function findBinary(dir, baseDir, binaryName, isWindows) {
	const entries = await fs.readdir(dir, { withFileTypes: true });

	for (const entry of entries) {
		const fullPath = path.join(dir, entry.name);

		// Security: Validate path to prevent traversal
		if (!isPathSafe(fullPath, baseDir)) {
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Skipping unsafe path: ${fullPath}`);
			}
			continue;
		}

		// Use shared utility to follow symlinks and get actual target type
		const entryType = await getEntryType(entry, fullPath);

		if (entryType.isDirectory) {
			const result = await findBinary(fullPath, baseDir, binaryName, isWindows);
			if (result) return result;
		} else if (entryType.isFile) {
			// Check if this is the binary we're looking for
			if (entry.name === binaryName ||
				entry.name === BINARY_NAME ||
				(isWindows && entry.name.endsWith('.exe'))) {
				return fullPath;
			}
		}
	}

	return null;
}

/**
 * Extracts the bundled binary for the current platform
 * @param {string} version - Version string (used for archive naming)
 * @returns {Promise<string>} Path to the extracted binary
 */
export async function extractBundledBinary(version) {
	const { platform, extension, osType } = detectPlatform();

	// Construct the archive filename
	const archiveName = `probe-v${version}-${platform}.${extension}`;

	// Path to the bundled archive
	const binariesDir = path.resolve(__dirname, '..', 'bin', 'binaries');
	const archivePath = path.join(binariesDir, archiveName);

	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Looking for bundled binary at: ${archivePath}`);
	}

	// Check if the archive exists
	if (!(await fs.pathExists(archivePath))) {
		throw new Error(
			`Bundled binary not found for platform ${platform}.\n` +
			`Expected archive: ${archiveName}\n` +
			`Searched in: ${binariesDir}\n` +
			`\n` +
			`Supported platforms:\n` +
            `  - x86_64-unknown-linux-musl (Linux x64, static)\n` +
            `  - aarch64-unknown-linux-musl (Linux ARM64, static)\n` +
			`  - x86_64-apple-darwin (macOS Intel)\n` +
			`  - aarch64-apple-darwin (macOS Apple Silicon)\n` +
			`  - x86_64-pc-windows-msvc (Windows x64)\n` +
			`\n` +
			`Your platform: ${platform}`
		);
	}

	// Determine output binary name and path
	const binDir = path.resolve(__dirname, '..', 'bin');
	const isWindows = osType === 'win32';
	const binaryName = isWindows ? `${BINARY_NAME}.exe` : `${BINARY_NAME}-binary`;
	const binaryPath = path.join(binDir, binaryName);

	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Extracting ${archiveName} to ${binDir}...`);
	}

	// Create a temporary extraction directory
	const extractDir = path.join(binDir, 'temp_extract');
	await fs.ensureDir(extractDir);

	try {
		// Extract based on file type using proper libraries (no shell commands!)
		if (extension === 'tar.gz') {
			await extractTarGz(archivePath, extractDir);
		} else if (extension === 'zip') {
			await extractZip(archivePath, extractDir);
		}

		// Find the binary in the extracted files
		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Searching for binary in extracted files...`);
		}

		const binaryFilePath = await findBinary(extractDir, extractDir, binaryName, isWindows);

		if (!binaryFilePath) {
			const allFiles = await fs.readdir(extractDir, { recursive: true });
			throw new Error(
				`Binary not found in the archive.\n` +
				`Expected binary name: ${binaryName}\n` +
				`Files in archive: ${allFiles.join(', ')}`
			);
		}

		// Copy the binary to the final location
		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Found binary at ${binaryFilePath}`);
			console.log(`Installing to ${binaryPath}`);
		}

		await fs.copyFile(binaryFilePath, binaryPath);

		// Make the binary executable on Unix-like systems
		if (!isWindows) {
			await fs.chmod(binaryPath, 0o755);
		}

		// Clean up the temporary extraction directory
		await fs.remove(extractDir);

		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Binary successfully extracted to ${binaryPath}`);
		}

		return binaryPath;
	} catch (error) {
		// Clean up on error
		try {
			await fs.remove(extractDir);
		} catch {}

		throw new Error(`Failed to extract bundled binary: ${error.message}`);
	}
}
