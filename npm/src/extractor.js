/**
 * Binary extractor for bundled probe binaries
 * @module extractor
 */

import fs from 'fs-extra';
import path from 'path';
import tar from 'tar';
import os from 'os';
import { promisify } from 'util';
import { exec as execCallback } from 'child_process';
import { fileURLToPath } from 'url';

const exec = promisify(execCallback);

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
			platform = 'x86_64-unknown-linux-gnu';
			extension = 'tar.gz';
		} else if (archType === 'arm64') {
			platform = 'aarch64-unknown-linux-gnu';
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
			`  - x86_64-unknown-linux-gnu (Linux x64)\n` +
			`  - aarch64-unknown-linux-gnu (Linux ARM64)\n` +
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
		// Extract based on file type
		if (extension === 'tar.gz') {
			await tar.extract({
				file: archivePath,
				cwd: extractDir
			});
		} else if (extension === 'zip') {
			await exec(`unzip -q "${archivePath}" -d "${extractDir}"`);
		}

		// Find the binary in the extracted files
		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Searching for binary in extracted files...`);
		}

		const findBinary = async (dir) => {
			const entries = await fs.readdir(dir, { withFileTypes: true });

			for (const entry of entries) {
				const fullPath = path.join(dir, entry.name);

				if (entry.isDirectory()) {
					const result = await findBinary(fullPath);
					if (result) return result;
				} else if (entry.isFile()) {
					// Check if this is the binary we're looking for
					if (entry.name === binaryName ||
						entry.name === BINARY_NAME ||
						(isWindows && entry.name.endsWith('.exe'))) {
						return fullPath;
					}
				}
			}

			return null;
		};

		const binaryFilePath = await findBinary(extractDir);

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
