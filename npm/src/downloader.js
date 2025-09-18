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
import { getPackageBinDir } from './directory-resolver.js';

const exec = promisify(execCallback);

// GitHub repository information
const REPO_OWNER = "probelabs";
const REPO_NAME = "probe";
const BINARY_NAME = "probe";

// Get the directory of the current module
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Note: LOCAL_DIR and VERSION_INFO_PATH are now resolved dynamically
// using getPackageBinDir() to handle different installation environments

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
			keywords: ['windows', 'Windows', 'msvc', 'pc-windows']
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

	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Detected OS: ${osInfo.type}, Architecture: ${archInfo.type}`);
	}
	return { os: osInfo, arch: archInfo };
}

/**
 * Constructs the asset name and download URL directly based on version and platform
 * @param {string} version - The version to download (e.g., "0.6.0-rc60")
 * @param {Object} osInfo - OS information from detectOsArch()
 * @param {Object} archInfo - Architecture information from detectOsArch()
 * @returns {Object} Asset information with name and url
 */
function constructAssetInfo(version, osInfo, archInfo) {
	let platform;
	let extension;
	
	// Map OS and arch to the expected format in release names
	switch (osInfo.type) {
		case 'linux':
			platform = `${archInfo.type}-unknown-linux-gnu`;
			extension = 'tar.gz';
			break;
		case 'darwin':
			platform = `${archInfo.type}-apple-darwin`;
			extension = 'tar.gz';
			break;
		case 'windows':
			platform = `${archInfo.type}-pc-windows-msvc`;
			extension = 'zip';
			break;
		default:
			throw new Error(`Unsupported OS type: ${osInfo.type}`);
	}
	
	const assetName = `probe-v${version}-${platform}.${extension}`;
	const checksumName = `${assetName}.sha256`;
	
	const baseUrl = `https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/v${version}`;
	const assetUrl = `${baseUrl}/${assetName}`;
	const checksumUrl = `${baseUrl}/${checksumName}`;
	
	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Constructed asset URL: ${assetUrl}`);
	}
	
	return {
		name: assetName,
		url: assetUrl,
		checksumName: checksumName,
		checksumUrl: checksumUrl
	};
}

/**
 * Gets the latest release information from GitHub
 * @param {string} [version] - Specific version to get
 * @returns {Promise<Object>} Release information
 */
async function getLatestRelease(version) {
	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log('Fetching release information...');
	}

	try {
		let releaseUrl;
		if (version) {
			// Always use the specified version
			releaseUrl = `https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/tags/v${version}`;
		} else {
			// Get all releases to find the most recent one (including prereleases)
			releaseUrl = `https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases`;
		}

		const response = await axios.get(releaseUrl);

		if (response.status !== 200) {
			throw new Error(`Failed to fetch release information: ${response.statusText}`);
		}

		let releaseData;
		if (version) {
			// Single release for specific version
			releaseData = response.data;
		} else {
			// Array of releases, pick the most recent one (first in the array)
			if (!Array.isArray(response.data) || response.data.length === 0) {
				throw new Error('No releases found');
			}
			releaseData = response.data[0];
		}

		const tag = releaseData.tag_name;
		const assets = releaseData.assets.map(asset => ({
			name: asset.name,
			url: asset.browser_download_url
		}));

		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Found release: ${tag} with ${assets.length} assets`);
		}
		return { tag, assets };
	} catch (error) {
		if (axios.isAxiosError(error) && error.response?.status === 404) {
			// If the specific version is not found, try to get all releases
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Release v${version} not found, trying to fetch all releases...`);
			}

			const response = await axios.get(`https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases`);

			if (response.data.length === 0) {
				throw new Error('No releases found');
			}

			// Try to find a release that matches the version
			let bestRelease = response.data[0]; // Default to latest release

			if (version && version !== '0.0.0') {
				if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
					console.log(`Looking for releases matching version: ${version}`);
					console.log(`Available releases: ${response.data.slice(0, 5).map(r => r.tag_name).join(', ')}...`);
				}

				// Try to find exact match first
				for (const release of response.data) {
					const releaseTag = release.tag_name.startsWith('v') ?
						release.tag_name.substring(1) : release.tag_name;

					if (releaseTag === version) {
						if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
							console.log(`Found exact matching release: ${release.tag_name}`);
						}
						bestRelease = release;
						break;
					}
				}

				// If no exact match, try to find a release with matching major.minor version
				if (bestRelease === response.data[0]) {
					const versionParts = version.split(/[\.-]/);
					const majorMinor = versionParts.slice(0, 2).join('.');

					if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
						console.log(`Looking for releases matching major.minor: ${majorMinor}`);
					}

					for (const release of response.data) {
						const releaseTag = release.tag_name.startsWith('v') ?
							release.tag_name.substring(1) : release.tag_name;
						const releaseVersionParts = releaseTag.split(/[\.-]/);
						const releaseMajorMinor = releaseVersionParts.slice(0, 2).join('.');

						if (releaseMajorMinor === majorMinor) {
							if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
								console.log(`Found matching major.minor release: ${release.tag_name}`);
							}
							bestRelease = release;
							break;
						}
					}
				}
			}

			const tag = bestRelease.tag_name;
			const assets = bestRelease.assets.map(asset => ({
				name: asset.name,
				url: asset.browser_download_url
			}));

			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Using release: ${tag} with ${assets.length} assets`);
			}
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
	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Finding appropriate binary for ${osInfo.type} ${archInfo.type}...`);
	}

	let bestAsset = null;
	let bestScore = 0;

	for (const asset of assets) {
		// Skip checksum files
		if (asset.name.endsWith('.sha256') || asset.name.endsWith('.md5') || asset.name.endsWith('.asc')) {
			continue;
		}

		if (osInfo.type === 'windows' && asset.name.match(/darwin|linux/)) {
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Skipping non-Windows binary: ${asset.name}`);
			}
			continue;
		} else if (osInfo.type === 'darwin' && asset.name.match(/windows|msvc|linux/)) {
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Skipping non-macOS binary: ${asset.name}`);
			}
			continue;
		} else if (osInfo.type === 'linux' && asset.name.match(/darwin|windows|msvc/)) {
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Skipping non-Linux binary: ${asset.name}`);
			}
			continue;
		}

		let score = 0;
		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Evaluating asset: ${asset.name}`);
		}

		// Check for OS match - give higher priority to exact OS matches
		let osMatched = false;
		for (const keyword of osInfo.keywords) {
			if (asset.name.includes(keyword)) {
				score += 10;
				osMatched = true;
				if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
					console.log(`  OS match found (${keyword}): +10, score = ${score}`);
				}
				break;
			}
		}

		// Check for architecture match
		for (const keyword of archInfo.keywords) {
			if (asset.name.includes(keyword)) {
				score += 5;
				if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
					console.log(`  Arch match found (${keyword}): +5, score = ${score}`);
				}
				break;
			}
		}

		// Prefer exact matches for binary name
		if (asset.name.startsWith(`${BINARY_NAME}-`)) {
			score += 3;
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`  Binary name match: +3, score = ${score}`);
			}
		}

		if (osMatched && score >= 15) {
			score += 5;
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`  OS+Arch bonus: +5, score = ${score}`);
			}
		}

		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`  Final score for ${asset.name}: ${score}`);
		}

		// If we have a perfect match, use it immediately
		if (score === 23) {
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Found perfect match: ${asset.name}`);
			}
			return asset;
		}

		// Otherwise, keep track of the best match so far
		if (score > bestScore) {
			bestScore = score;
			bestAsset = asset;
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`  New best asset: ${asset.name} (score: ${score})`);
			}
		}
	}

	if (!bestAsset) {
		throw new Error(`Could not find a suitable binary for ${osInfo.type} ${archInfo.type}`);
	}

	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Selected asset: ${bestAsset.name} (score: ${bestScore})`);
	}
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
	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Downloading ${asset.name}...`);
	}

	// Download the asset
	const assetResponse = await axios.get(asset.url, { responseType: 'arraybuffer' });
	await fs.writeFile(assetPath, Buffer.from(assetResponse.data));

	// Try to download the checksum
	const checksumUrl = asset.checksumUrl || `${asset.url}.sha256`;
	const checksumFileName = asset.checksumName || `${asset.name}.sha256`;
	let checksumPath = null;

	try {
		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Downloading checksum...`);
		}
		const checksumResponse = await axios.get(checksumUrl);
		checksumPath = path.join(outputDir, checksumFileName);
		await fs.writeFile(checksumPath, checksumResponse.data);
	} catch (error) {
		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log('No checksum file found, skipping verification');
		}
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

	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Verifying checksum...`);
	}

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

	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Checksum verified successfully`);
	}
	return true;
}

/**
 * Extracts and installs the binary
 * @param {string} assetPath - Path to the asset
 * @param {string} outputDir - Directory to extract to
 * @returns {Promise<string>} Path to the extracted binary
 */
async function extractBinary(assetPath, outputDir) {
	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Extracting ${path.basename(assetPath)}...`);
	}

	const assetName = path.basename(assetPath);
	const isWindows = os.platform() === 'win32';
	// Use the correct binary name: probe.exe for Windows, probe-binary for Unix
	const binaryName = isWindows ? `${BINARY_NAME}.exe` : `${BINARY_NAME}-binary`;
	const binaryPath = path.join(outputDir, binaryName);

	try {
		// Create a temporary extraction directory
		const extractDir = path.join(outputDir, 'temp_extract');
		await fs.ensureDir(extractDir);

		// Determine file type and extract accordingly
		if (assetName.endsWith('.tar.gz') || assetName.endsWith('.tgz')) {
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Extracting tar.gz to ${extractDir}...`);
			}
			await tar.extract({
				file: assetPath,
				cwd: extractDir
			});
		} else if (assetName.endsWith('.zip')) {
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Extracting zip to ${extractDir}...`);
			}
			await exec(`unzip -q "${assetPath}" -d "${extractDir}"`);
		} else {
			// Assume it's a direct binary
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Copying binary directly to ${binaryPath}`);
			}
			await fs.copyFile(assetPath, binaryPath);

			// Make the binary executable
			if (!isWindows) {
				await fs.chmod(binaryPath, 0o755);
			}

			// Clean up the extraction directory
			await fs.remove(extractDir);
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Binary installed to ${binaryPath}`);
			}
			return binaryPath;
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
			// List all extracted files for debugging
			const allFiles = await fs.readdir(extractDir, { recursive: true });
			console.error(`Binary not found in extracted files. Found: ${allFiles.join(', ')}`);
			throw new Error(`Binary not found in the archive.`);
		}

		// Copy the binary directly to the final location
		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Found binary at ${binaryFilePath}`);
			console.log(`Copying binary to ${binaryPath}`);
		}
		await fs.copyFile(binaryFilePath, binaryPath);

		// Make the binary executable
		if (!isWindows) {
			await fs.chmod(binaryPath, 0o755);
		}

		// Clean up
		await fs.remove(extractDir);

		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Binary successfully installed to ${binaryPath}`);
		}
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
async function getVersionInfo(binDir) {
	try {
		const versionInfoPath = path.join(binDir, 'version-info.json');
		if (await fs.pathExists(versionInfoPath)) {
			const content = await fs.readFile(versionInfoPath, 'utf-8');
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
 * @param {string} binDir - Directory where version info should be saved
 * @returns {Promise<void>}
 */
async function saveVersionInfo(version, binDir) {
	const versionInfo = {
		version,
		lastUpdated: new Date().toISOString()
	};

	const versionInfoPath = path.join(binDir, 'version-info.json');
	await fs.writeFile(versionInfoPath, JSON.stringify(versionInfo, null, 2));
	if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
		console.log(`Version info saved: ${version} at ${versionInfoPath}`);
	}
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
					if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
						console.log(`Found package.json at: ${packageJsonPath}`);
					}
					const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'));
					if (packageJson.version) {
						if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
							console.log(`Using version from package.json: ${packageJson.version}`);
						}
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
		// Get writable directory for binary storage (handles CI, npx, Docker scenarios)
		const localDir = await getPackageBinDir();

		// If no version is specified, use the package version
		if (!version || version === '0.0.0') {
			version = await getPackageVersion();
		}

		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Downloading probe binary (version: ${version || 'latest'})...`);
			console.log(`Using binary directory: ${localDir}`);
		}

		const isWindows = os.platform() === 'win32';
		// Use the correct binary name: probe.exe for Windows, probe-binary for Unix
		const binaryName = isWindows ? `${BINARY_NAME}.exe` : `${BINARY_NAME}-binary`;
		const binaryPath = path.join(localDir, binaryName);

		// Check if the binary already exists and version matches
		if (await fs.pathExists(binaryPath)) {
			const versionInfo = await getVersionInfo(localDir);

			// If versions match, use existing binary
			if (versionInfo && versionInfo.version === version) {
				if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
					console.log(`Using existing binary at ${binaryPath} (version: ${versionInfo.version})`);
				}
				return binaryPath;
			}

			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Existing binary version (${versionInfo?.version || 'unknown'}) doesn't match requested version (${version}). Downloading new version...`);
			}
		}

		// Get OS and architecture information
		const { os: osInfo, arch: archInfo } = detectOsArch();

		// Determine which version to download
		let versionToUse = version;
		let bestAsset;
		let tagVersion;

		if (!versionToUse || versionToUse === '0.0.0') {
			// No specific version - use GitHub API to get the latest release
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log('No specific version requested, will use the latest release');
			}
			const { tag, assets } = await getLatestRelease(undefined);
			tagVersion = tag.startsWith('v') ? tag.substring(1) : tag;
			bestAsset = findBestAsset(assets, osInfo, archInfo);
			
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Found release version: ${tagVersion}`);
			}
		} else {
			// Specific version requested - construct download URL directly
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Direct download for version: ${versionToUse}`);
			}
			tagVersion = versionToUse;
			bestAsset = constructAssetInfo(versionToUse, osInfo, archInfo);
		}
		const { assetPath, checksumPath } = await downloadAsset(bestAsset, localDir);

		// Verify checksum if available
		const checksumValid = await verifyChecksum(assetPath, checksumPath);
		if (!checksumValid) {
			throw new Error('Checksum verification failed');
		}

		// Extract the binary
		const extractedBinaryPath = await extractBinary(assetPath, localDir);

		// Save the version information
		await saveVersionInfo(tagVersion, localDir);

		// Clean up the downloaded archive
		try {
			await fs.remove(assetPath);
			if (checksumPath) {
				await fs.remove(checksumPath);
			}
		} catch (err) {
			if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
				console.log(`Warning: Could not clean up temporary files: ${err}`);
			}
		}

		if (process.env.DEBUG === '1' || process.env.VERBOSE === '1') {
			console.log(`Binary successfully installed at ${extractedBinaryPath} (version: ${tagVersion})`);
		}
		return extractedBinaryPath;
	} catch (error) {
		console.error('Error downloading probe binary:', error);
		throw error;
	}
}
