/**
 * Binary downloader for the probe package
 * @module downloader
 */

import axios from 'axios';
import fs from 'fs-extra';
import path from 'path';
import { createHash } from 'crypto';
import { promisify } from 'util';
import { exec as execCallback } from 'child_process';
import tar from 'tar';
import os from 'os';
import { fileURLToPath } from 'url';
import { ensureBinDirectory } from './utils.js';

const exec = promisify(execCallback);

// GitHub repository information
const REPO_OWNER = "buger";
const REPO_NAME = "probe";
const BINARY_NAME = "probe";

// Get the directory of the current module
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Local storage directory for downloaded binaries
const LOCAL_DIR = path.resolve(__dirname, '..', 'bin');

// Version info file path
const VERSION_INFO_PATH = path.join(LOCAL_DIR, 'version-info.json');

/**
 * Detects the current OS and architecture
 * @returns {Object} Object containing OS and architecture information
 */
function detectOsArch() {
	const osType = os.platform();
	const archType = os.arch();

	let osInfo;
	let archInfo;

	// Detect OS
	switch (osType) {
		case 'linux':
			osInfo = {
				type: 'linux',
				keywords: ['linux', 'Linux', 'gnu']
			};
			break;
		case 'darwin':
			osInfo = {
				type: 'darwin',
				keywords: ['darwin', 'Darwin', 'mac', 'Mac', 'apple', 'Apple', 'osx', 'OSX']
			};
			break;
		case 'win32':
			osInfo = {
				type: 'windows',
				keywords: ['windows', 'Windows', 'win', 'Win']
			};
			break;
		default:
			throw new Error(`Unsupported operating system: ${osType}`);
	}

	// Detect architecture
	switch (archType) {
		case 'x64':
			archInfo = {
				type: 'x86_64',
				keywords: ['x86_64', 'amd64', 'x64', '64bit', '64-bit']
			};
			break;
		case 'arm64':
			archInfo = {
				type: 'aarch64',
				keywords: ['arm64', 'aarch64', 'arm', 'ARM']
			};
			break;
		default:
			throw new Error(`Unsupported architecture: ${archType}`);
	}

	console.log(`Detected OS: ${osInfo.type}, Architecture: ${archInfo.type}`);
	return { os: osInfo, arch: archInfo };
}

/**
 * Gets the latest release information from GitHub
 * @param {string} [version] - Specific version to get
 * @returns {Promise<Object>} Release information
 */
async function getLatestRelease(version) {
	console.log('Fetching release information...');

	try {
		let releaseUrl;
		if (version) {
			// Always use the specified version
			releaseUrl = `https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/tags/v${version}`;
		} else {
			// Use the latest release only if no version is specified
			releaseUrl = `https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/latest`;
		}

		const response = await axios.get(releaseUrl);

		if (response.status !== 200) {
			throw new Error(`Failed to fetch release information: ${response.statusText}`);
		}

		const tag = response.data.tag_name;
		const assets = response.data.assets.map(asset => ({
			name: asset.name,
			url: asset.browser_download_url
		}));

		console.log(`Found release: ${tag} with ${assets.length} assets`);
		return { tag, assets };
	} catch (error) {
		if (axios.isAxiosError(error) && error.response?.status === 404) {
			// If the specific version is not found, try to get all releases
			console.log(`Release v${version} not found, trying to fetch all releases...`);

			const response = await axios.get(`https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases`);

			if (response.data.length === 0) {
				throw new Error('No releases found');
			}

			// Try to find a release that matches the version prefix
			let bestRelease = response.data[0]; // Default to first release

			if (version && version !== '0.0.0') {
				// Try to find a release that starts with the same version prefix
				const versionParts = version.split('.');
				const versionPrefix = versionParts.slice(0, 2).join('.'); // e.g., "0.2" from "0.2.2-rc7"

				console.log(`Looking for releases matching prefix: ${versionPrefix}`);

				for (const release of response.data) {
					const releaseTag = release.tag_name.startsWith('v') ?
						release.tag_name.substring(1) : release.tag_name;

					if (releaseTag.startsWith(versionPrefix)) {
						console.log(`Found matching release: ${release.tag_name}`);
						bestRelease = release;
						break;
					}
				}
			}

			const tag = bestRelease.tag_name;
			const assets = bestRelease.assets.map(asset => ({
				name: asset.name,
				url: asset.browser_download_url
			}));

			console.log(`Using release: ${tag} with ${assets.length} assets`);
			return { tag, assets };
		}

		throw error;
	}
}

/**
 * Finds the best matching asset for the current OS and architecture
 * @param {Array} assets - List of assets
 * @param {Object} osInfo - OS information
 * @param {Object} archInfo - Architecture information
 * @returns {Object} Best matching asset
 */
function findBestAsset(assets, osInfo, archInfo) {
	console.log(`Finding appropriate binary for ${osInfo.type} ${archInfo.type}...`);
	console.log(`Available assets: ${assets.map(a => a.name).join(', ')}`);

	let bestAsset = null;
	let bestScore = 0;
	const assetScores = [];

	for (const asset of assets) {
		// Skip checksum files
		if (asset.name.endsWith('.sha256') || asset.name.endsWith('.md5') || asset.name.endsWith('.asc')) {
			console.log(`Skipping checksum file: ${asset.name}`);
			continue;
		}

		let score = 0;
		let osMatch = false;
		let archMatch = false;
		const matchDetails = [];

		// Check for OS match with priority scoring
		for (const keyword of osInfo.keywords) {
			if (asset.name.toLowerCase().includes(keyword.toLowerCase())) {
				// Give higher scores for more specific matches
				if (keyword === osInfo.type) {
					score += 10; // Exact OS type match
					matchDetails.push(`exact-os:${keyword}`);
				} else {
					score += 7; // Keyword match
					matchDetails.push(`os:${keyword}`);
				}
				osMatch = true;
				break;
			}
		}

		// Check for architecture match with priority scoring
		for (const keyword of archInfo.keywords) {
			if (asset.name.toLowerCase().includes(keyword.toLowerCase())) {
				// Give higher scores for more specific matches
				if (keyword === archInfo.type) {
					score += 10; // Exact arch type match
					matchDetails.push(`exact-arch:${keyword}`);
				} else {
					score += 7; // Keyword match
					matchDetails.push(`arch:${keyword}`);
				}
				archMatch = true;
				break;
			}
		}

		// Prefer exact matches for binary name
		if (asset.name.startsWith(`${BINARY_NAME}-`)) {
			score += 3;
			matchDetails.push('binary-prefix');
		}

		// Bonus for having both OS and arch matches
		if (osMatch && archMatch) {
			score += 5;
			matchDetails.push('complete-match');
		}

		// Special handling for common naming patterns
		const assetLower = asset.name.toLowerCase();
		
		// macOS ARM64 specific patterns
		if (osInfo.type === 'darwin' && archInfo.type === 'aarch64') {
			if (assetLower.includes('darwin') && assetLower.includes('arm64')) {
				score += 15; // High priority for exact darwin+arm64
				matchDetails.push('darwin-arm64-exact');
			} else if (assetLower.includes('apple') && assetLower.includes('silicon')) {
				score += 12; // Apple Silicon naming
				matchDetails.push('apple-silicon');
			}
		}

		// Windows specific patterns
		if (osInfo.type === 'windows') {
			if (assetLower.includes('windows') && (assetLower.includes('x86_64') || assetLower.includes('amd64'))) {
				score += 15; // High priority for exact windows+x64
				matchDetails.push('windows-x64-exact');
			} else if (assetLower.includes('.exe')) {
				score += 8; // Windows executable
				matchDetails.push('exe-file');
			}
		}

		// Linux specific patterns
		if (osInfo.type === 'linux') {
			if (assetLower.includes('linux') && assetLower.includes('x86_64')) {
				score += 15; // High priority for exact linux+x64
				matchDetails.push('linux-x64-exact');
			} else if (assetLower.includes('gnu')) {
				score += 8; // GNU/Linux
				matchDetails.push('gnu-linux');
			}
		}

		assetScores.push({
			name: asset.name,
			score,
			matches: matchDetails,
			osMatch,
			archMatch
		});

		console.log(`Asset: ${asset.name} - Score: ${score} - Matches: [${matchDetails.join(', ')}]`);

		// If we have a very high confidence match, use it immediately
		if (score >= 25 && osMatch && archMatch) {
			console.log(`Found high-confidence match: ${asset.name} (score: ${score})`);
			return asset;
		}

		// Otherwise, keep track of the best match so far
		if (score > bestScore) {
			bestScore = score;
			bestAsset = asset;
		}
	}

	// Log all scored assets for debugging
	console.log('Asset scoring summary:');
	assetScores
		.sort((a, b) => b.score - a.score)
		.forEach(item => {
			console.log(`  ${item.name}: ${item.score} [${item.matches.join(', ')}]`);
		});

	if (!bestAsset) {
		const availableAssets = assets.filter(a => 
			!a.name.endsWith('.sha256') && 
			!a.name.endsWith('.md5') && 
			!a.name.endsWith('.asc')
		).map(a => a.name);
		
		throw new Error(`Could not find a suitable binary for ${osInfo.type} ${archInfo.type}. Available assets: ${availableAssets.join(', ')}`);
	}

	console.log(`Selected asset: ${bestAsset.name} (score: ${bestScore})`);
	return bestAsset;
}

/**
 * Downloads the asset and its checksum
 * @param {Object} asset - Asset to download
 * @param {string} outputDir - Directory to save to
 * @returns {Promise<Object>} Paths to the asset and checksum
 */
async function downloadAsset(asset, outputDir) {
	await fs.ensureDir(outputDir);

	const assetPath = path.join(outputDir, asset.name);
	console.log(`Downloading ${asset.name}...`);

	// Download the asset
	const assetResponse = await axios.get(asset.url, { responseType: 'arraybuffer' });
	await fs.writeFile(assetPath, Buffer.from(assetResponse.data));

	// Try to download the checksum
	const checksumUrl = `${asset.url}.sha256`;
	let checksumPath = null;

	try {
		console.log(`Downloading checksum...`);
		const checksumResponse = await axios.get(checksumUrl);
		checksumPath = path.join(outputDir, `${asset.name}.sha256`);
		await fs.writeFile(checksumPath, checksumResponse.data);
	} catch (error) {
		console.log('No checksum file found, skipping verification');
	}

	return { assetPath, checksumPath };
}

/**
 * Verifies the checksum of the downloaded asset
 * @param {string} assetPath - Path to the asset
 * @param {string|null} checksumPath - Path to the checksum file
 * @returns {Promise<boolean>} Whether verification succeeded
 */
async function verifyChecksum(assetPath, checksumPath) {
	if (!checksumPath) {
		return true;
	}

	console.log(`Verifying checksum...`);

	// Read the expected checksum
	const checksumContent = await fs.readFile(checksumPath, 'utf-8');
	const expectedChecksum = checksumContent.trim().split(' ')[0];

	// Calculate the actual checksum
	const fileBuffer = await fs.readFile(assetPath);
	const actualChecksum = createHash('sha256').update(fileBuffer).digest('hex');

	if (expectedChecksum !== actualChecksum) {
		console.error(`Checksum verification failed!`);
		console.error(`Expected: ${expectedChecksum}`);
		console.error(`Actual: ${actualChecksum}`);
		return false;
	}

	console.log(`Checksum verified successfully`);
	return true;
}

/**
 * Extracts and installs the binary
 * @param {string} assetPath - Path to the asset
 * @param {string} outputDir - Directory to extract to
 * @returns {Promise<string>} Path to the extracted binary
 */
async function extractBinary(assetPath, outputDir) {
	console.log(`Extracting ${path.basename(assetPath)}...`);

	const assetName = path.basename(assetPath);
	const isWindows = os.platform() === 'win32';
	const binaryName = isWindows ? `${BINARY_NAME}.exe` : BINARY_NAME;
	const binaryPath = path.join(outputDir, binaryName);

	try {
		// Create a temporary extraction directory
		const extractDir = path.join(outputDir, 'temp_extract');
		await fs.ensureDir(extractDir);

		// Determine file type and extract accordingly
		if (assetName.endsWith('.tar.gz') || assetName.endsWith('.tgz')) {
			console.log(`Extracting tar.gz to ${extractDir}...`);
			await tar.extract({
				file: assetPath,
				cwd: extractDir
			});
		} else if (assetName.endsWith('.zip')) {
			console.log(`Extracting zip to ${extractDir}...`);
			if (isWindows) {
				// Use PowerShell's Expand-Archive on Windows
				await exec(`powershell -Command "Expand-Archive -Path '${assetPath}' -DestinationPath '${extractDir}' -Force"`);
			} else {
				// Use unzip on Unix-like systems
				await exec(`unzip -q "${assetPath}" -d "${extractDir}"`);
			}
		} else {
			// Assume it's a direct binary
			console.log(`Copying binary directly to ${binaryPath}`);
			await fs.copyFile(assetPath, binaryPath);

			// Make the binary executable
			if (!isWindows) {
				await fs.chmod(binaryPath, 0o755);
			}

			// Clean up the extraction directory
			await fs.remove(extractDir);
			console.log(`Binary installed to ${binaryPath}`);
			return binaryPath;
		}

		// Find the binary in the extracted files
		console.log(`Searching for binary in extracted files...`);
		const { os: osInfo, arch: archInfo } = detectOsArch();
		
		const findBinary = async (dir) => {
			const entries = await fs.readdir(dir, { withFileTypes: true });
			const candidates = [];

			// First pass: collect all potential binary candidates
			for (const entry of entries) {
				const fullPath = path.join(dir, entry.name);

				if (entry.isDirectory()) {
					const result = await findBinary(fullPath);
					if (result) return result;
				} else if (entry.isFile()) {
					// Check if this could be a binary we're looking for
					const entryLower = entry.name.toLowerCase();
					const isExecutable = !isWindows ? await isFileExecutable(fullPath) : true;
					
					if ((entry.name === binaryName ||
						entry.name === BINARY_NAME ||
						entryLower.includes(BINARY_NAME) ||
						(isWindows && entry.name.endsWith('.exe'))) && isExecutable) {
						
						// Score this candidate based on platform match
						let score = 0;
						const matchDetails = [];

						// Base score for being a potential binary
						score += 10;
						matchDetails.push('binary-candidate');

						// Exact name match gets highest priority
						if (entry.name === binaryName || entry.name === BINARY_NAME) {
							score += 50;
							matchDetails.push('exact-name');
						}

						// Platform-specific scoring
						for (const keyword of osInfo.keywords) {
							if (entryLower.includes(keyword.toLowerCase())) {
								score += 20;
								matchDetails.push(`os:${keyword}`);
								break;
							}
						}

						// Architecture-specific scoring
						for (const keyword of archInfo.keywords) {
							if (entryLower.includes(keyword.toLowerCase())) {
								score += 20;
								matchDetails.push(`arch:${keyword}`);
								break;
							}
						}

						// Special handling for macOS ARM64
						if (osInfo.type === 'darwin' && archInfo.type === 'aarch64') {
							if (entryLower.includes('darwin') && entryLower.includes('arm64')) {
								score += 30;
								matchDetails.push('darwin-arm64-match');
							} else if (entryLower.includes('apple') && entryLower.includes('silicon')) {
								score += 25;
								matchDetails.push('apple-silicon-match');
							} else if (entryLower.includes('aarch64')) {
								score += 15;
								matchDetails.push('aarch64-match');
							}
						}

						// Windows-specific patterns
						if (osInfo.type === 'windows') {
							if (entryLower.includes('windows') || entry.name.endsWith('.exe')) {
								score += 15;
								matchDetails.push('windows-match');
							}
						}

						// Linux-specific patterns
						if (osInfo.type === 'linux') {
							if (entryLower.includes('linux')) {
								score += 15;
								matchDetails.push('linux-match');
							}
						}

						candidates.push({
							path: fullPath,
							name: entry.name,
							score,
							matches: matchDetails
						});

						console.log(`Binary candidate: ${entry.name} - Score: ${score} - Matches: [${matchDetails.join(', ')}]`);
					}
				}
			}

			// If we found candidates, return the best one
			if (candidates.length > 0) {
				candidates.sort((a, b) => b.score - a.score);
				const best = candidates[0];
				console.log(`Selected binary: ${best.name} (score: ${best.score})`);
				return best.path;
			}

			return null;
		};

		// Helper function to check if a file is executable (Unix-like systems only)
		const isFileExecutable = async (filePath) => {
			if (isWindows) return true; // Skip check on Windows
			
			try {
				const stats = await fs.stat(filePath);
				// Check if file has execute permission for owner, group, or others
				return (stats.mode & parseInt('111', 8)) !== 0;
			} catch (error) {
				return false;
			}
		};

		const binaryFilePath = await findBinary(extractDir);

		if (!binaryFilePath) {
			// List all extracted files for debugging
			const allFiles = await fs.readdir(extractDir, { recursive: true });
			console.error(`Binary not found in extracted files. Found: ${allFiles.join(', ')}`);
			throw new Error(`Binary not found in the archive.`);
		}

		// Copy the binary directly to the final location
		console.log(`Found binary at ${binaryFilePath}`);
		console.log(`Copying binary to ${binaryPath}`);
		await fs.copyFile(binaryFilePath, binaryPath);

		// Make the binary executable
		if (!isWindows) {
			await fs.chmod(binaryPath, 0o755);
		}

		// Clean up
		await fs.remove(extractDir);

		console.log(`Binary successfully installed to ${binaryPath}`);
		return binaryPath;
	} catch (error) {
		console.error(`Error extracting binary: ${error instanceof Error ? error.message : String(error)}`);
		throw error;
	}
}

/**
 * Gets version info from the version file
 * @returns {Promise<Object|null>} Version information
 */
async function getVersionInfo() {
	try {
		if (await fs.pathExists(VERSION_INFO_PATH)) {
			const content = await fs.readFile(VERSION_INFO_PATH, 'utf-8');
			return JSON.parse(content);
		}
		return null;
	} catch (error) {
		console.warn(`Warning: Could not read version info: ${error}`);
		return null;
	}
}

/**
 * Saves version info to the version file
 * @param {string} version - Version to save
 * @returns {Promise<void>}
 */
async function saveVersionInfo(version) {
	const versionInfo = {
		version,
		lastUpdated: new Date().toISOString()
	};

	await fs.writeFile(VERSION_INFO_PATH, JSON.stringify(versionInfo, null, 2));
	console.log(`Version info saved: ${version}`);
}

/**
 * Gets the package version from package.json
 * @returns {Promise<string>} Package version
 */
async function getPackageVersion() {
	try {
		// Try multiple possible locations for package.json
		const possiblePaths = [
			path.resolve(__dirname, '..', 'package.json'),      // When installed from npm: src/../package.json
			path.resolve(__dirname, '..', '..', 'package.json') // In development: src/../../package.json
		];

		for (const packageJsonPath of possiblePaths) {
			try {
				if (fs.existsSync(packageJsonPath)) {
					console.log(`Found package.json at: ${packageJsonPath}`);
					const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'));
					if (packageJson.version) {
						console.log(`Using version from package.json: ${packageJson.version}`);
						return packageJson.version;
					}
				}
			} catch (err) {
				console.error(`Error reading package.json at ${packageJsonPath}:`, err);
			}
		}

		// If we can't find the version in package.json, return a default version
		return '0.0.0';
	} catch (error) {
		console.error('Error getting package version:', error);
		return '0.0.0';
	}
}

/**
 * Downloads the probe binary
 * @param {string} [version] - Specific version to download
 * @returns {Promise<string>} Path to the downloaded binary
 */
export async function downloadProbeBinary(version) {
	try {
		// Create the bin directory if it doesn't exist
		await ensureBinDirectory();

		// If no version is specified, use the package version
		if (!version || version === '0.0.0') {
			version = await getPackageVersion();
		}

		console.log(`Downloading probe binary (version: ${version || 'latest'})...`);

		const isWindows = os.platform() === 'win32';
		const binaryName = isWindows ? `${BINARY_NAME}.exe` : BINARY_NAME;
		const binaryPath = path.join(LOCAL_DIR, binaryName);

		// Check if the binary already exists and version matches
		if (await fs.pathExists(binaryPath)) {
			const versionInfo = await getVersionInfo();

			// If versions match, use existing binary
			if (versionInfo && versionInfo.version === version) {
				console.log(`Using existing binary at ${binaryPath} (version: ${versionInfo.version})`);
				return binaryPath;
			}

			console.log(`Existing binary version (${versionInfo?.version || 'unknown'}) doesn't match requested version (${version}). Downloading new version...`);
		}

		// Get OS and architecture information
		const { os: osInfo, arch: archInfo } = detectOsArch();

		// Determine which version to download
		let versionToUse = version;
		if (!versionToUse || versionToUse === '0.0.0') {
			console.log('No specific version requested, will use the latest release');
			versionToUse = undefined;
		} else {
			console.log(`Looking for release with version: ${versionToUse}`);
		}

		// Get release information
		const { tag, assets } = await getLatestRelease(versionToUse);
		const tagVersion = tag.startsWith('v') ? tag.substring(1) : tag;
		console.log(`Found release version: ${tagVersion}`);

		// Find and download the appropriate asset
		const bestAsset = findBestAsset(assets, osInfo, archInfo);
		const { assetPath, checksumPath } = await downloadAsset(bestAsset, LOCAL_DIR);

		// Verify checksum if available
		const checksumValid = await verifyChecksum(assetPath, checksumPath);
		if (!checksumValid) {
			throw new Error('Checksum verification failed');
		}

		// Extract the binary
		const extractedBinaryPath = await extractBinary(assetPath, LOCAL_DIR);

		// Save the version information
		await saveVersionInfo(tagVersion);

		// Clean up the downloaded archive
		try {
			await fs.remove(assetPath);
			if (checksumPath) {
				await fs.remove(checksumPath);
			}
		} catch (err) {
			console.log(`Warning: Could not clean up temporary files: ${err}`);
		}

		console.log(`Binary successfully installed at ${extractedBinaryPath} (version: ${tagVersion})`);
		return extractedBinaryPath;
	} catch (error) {
		console.error('Error downloading probe binary:', error);
		throw error;
	}
}
