import axios from 'axios';
import fs from 'fs-extra';
import path from 'path';
import { createHash } from 'crypto';
import { promisify } from 'util';
import { exec as execCallback } from 'child_process';
import tar from 'tar';
import os from 'os';

const exec = promisify(execCallback);

// GitHub repository information
const REPO_OWNER = "buger";
const REPO_NAME = "probe";
const BINARY_NAME = "probe";

// Local storage directory for downloaded binaries
const LOCAL_DIR = path.join(os.homedir(), '.probe-mcp');

interface OsInfo {
  type: string;
  keywords: string[];
}

interface ArchInfo {
  type: string;
  keywords: string[];
}

interface ReleaseAsset {
  name: string;
  url: string;
}

/**
 * Detects the current OS and architecture
 */
function detectOsArch(): { os: OsInfo; arch: ArchInfo } {
  const osType = os.platform();
  const archType = os.arch();
  
  let osInfo: OsInfo;
  let archInfo: ArchInfo;
  
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
 */
async function getLatestRelease(version?: string): Promise<{ tag: string; assets: ReleaseAsset[] }> {
  console.log('Fetching release information...');
  
  try {
    let releaseUrl;
    if (version) {
      // Always use the specified version from package.json
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
    const assets: ReleaseAsset[] = response.data.assets.map((asset: any) => ({
      name: asset.name,
      url: asset.browser_download_url
    }));
    
    console.log(`Found release: ${tag} with ${assets.length} assets`);
    return { tag, assets };
  } catch (error) {
    if (axios.isAxiosError(error) && error.response?.status === 404) {
      // If the specific version is not found, try to get all releases
      console.log('Release not found, trying to fetch all releases...');
      
      const response = await axios.get(`https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases`);
      
      if (response.data.length === 0) {
        throw new Error('No releases found');
      }
      
      // Use the first release
      const tag = response.data[0].tag_name;
      const assets: ReleaseAsset[] = response.data[0].assets.map((asset: any) => ({
        name: asset.name,
        url: asset.browser_download_url
      }));
      
      console.log(`Found release: ${tag} with ${assets.length} assets`);
      return { tag, assets };
    }
    
    throw error;
  }
}

/**
 * Finds the best matching asset for the current OS and architecture
 */
function findBestAsset(assets: ReleaseAsset[], osInfo: OsInfo, archInfo: ArchInfo): ReleaseAsset {
  console.log(`Finding appropriate binary for ${osInfo.type} ${archInfo.type}...`);
  
  let bestAsset: ReleaseAsset | null = null;
  let bestScore = 0;
  
  for (const asset of assets) {
    // Skip checksum files
    if (asset.name.endsWith('.sha256') || asset.name.endsWith('.md5') || asset.name.endsWith('.asc')) {
      continue;
    }
    
    let score = 0;
    
    // Check for OS match
    for (const keyword of osInfo.keywords) {
      if (asset.name.includes(keyword)) {
        score += 5;
        break;
      }
    }
    
    // Check for architecture match
    for (const keyword of archInfo.keywords) {
      if (asset.name.includes(keyword)) {
        score += 5;
        break;
      }
    }
    
    // Prefer exact matches for binary name
    if (asset.name.startsWith(`${BINARY_NAME}-`)) {
      score += 3;
    }
    
    // If we have a perfect match, use it immediately
    if (score === 13) {
      console.log(`Found perfect match: ${asset.name}`);
      return asset;
    }
    
    // Otherwise, keep track of the best match so far
    if (score > bestScore) {
      bestScore = score;
      bestAsset = asset;
    }
  }
  
  if (!bestAsset) {
    throw new Error(`Could not find a suitable binary for ${osInfo.type} ${archInfo.type}`);
  }
  
  console.log(`Selected asset: ${bestAsset.name} (score: ${bestScore})`);
  return bestAsset;
}

/**
 * Downloads the asset and its checksum
 */
async function downloadAsset(asset: ReleaseAsset, outputDir: string): Promise<{ assetPath: string; checksumPath: string | null }> {
  await fs.ensureDir(outputDir);
  
  const assetPath = path.join(outputDir, asset.name);
  console.log(`Downloading ${asset.name}...`);
  
  // Download the asset
  const assetResponse = await axios.get(asset.url, { responseType: 'arraybuffer' });
  await fs.writeFile(assetPath, Buffer.from(assetResponse.data));
  
  // Try to download the checksum
  const checksumUrl = `${asset.url}.sha256`;
  let checksumPath: string | null = null;
  
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
 */
async function verifyChecksum(assetPath: string, checksumPath: string | null): Promise<boolean> {
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
 */
async function extractBinary(assetPath: string, outputDir: string): Promise<string> {
  console.log(`Extracting ${path.basename(assetPath)}...`);
  
  const assetName = path.basename(assetPath);
  const isWindows = os.platform() === 'win32';
  const binaryName = isWindows ? `${BINARY_NAME}.exe` : BINARY_NAME;
  let binaryPath: string;
  
  // Determine file type and extract accordingly
  if (assetName.endsWith('.tar.gz') || assetName.endsWith('.tgz')) {
    // Extract the tar.gz file
    const extractDir = path.join(outputDir, 'extract');
    await fs.ensureDir(extractDir);
    await tar.extract({
      file: assetPath,
      cwd: extractDir
    });
    
    // Find the binary in the extracted files
    const files = await fs.readdir(extractDir, { recursive: true });
    const binaryFile = files.find(file => 
      path.basename(file) === binaryName || 
      (isWindows && path.basename(file).endsWith('.exe'))
    );
    
    if (!binaryFile) {
      throw new Error(`Binary not found in the archive`);
    }
    
    // Move the binary to the output directory
    const sourcePath = path.join(extractDir, binaryFile);
    binaryPath = path.join(outputDir, binaryName);
    await fs.move(sourcePath, binaryPath, { overwrite: true });
    
    // Clean up
    await fs.remove(extractDir);
  } else if (assetName.endsWith('.zip')) {
    // For zip files, we need to use a different approach
    const extractDir = path.join(outputDir, 'extract');
    await fs.ensureDir(extractDir);
    
    // Use unzip command
    await exec(`unzip -q "${assetPath}" -d "${extractDir}"`);
    
    // Find the binary in the extracted files
    const files = await fs.readdir(extractDir, { recursive: true });
    const binaryFile = files.find(file => 
      path.basename(file) === binaryName || 
      (isWindows && path.basename(file).endsWith('.exe'))
    );
    
    if (!binaryFile) {
      throw new Error(`Binary not found in the archive`);
    }
    
    // Move the binary to the output directory
    const sourcePath = path.join(extractDir, binaryFile);
    binaryPath = path.join(outputDir, binaryName);
    await fs.move(sourcePath, binaryPath, { overwrite: true });
    
    // Clean up
    await fs.remove(extractDir);
  } else {
    // Assume it's a direct binary
    binaryPath = path.join(outputDir, binaryName);
    await fs.move(assetPath, binaryPath, { overwrite: true });
  }
  
  // Make the binary executable
  if (!isWindows) {
    await fs.chmod(binaryPath, 0o755);
  }
  
  console.log(`Binary installed to ${binaryPath}`);
  return binaryPath;
}

/**
 * Downloads the probe binary
 */
export async function downloadProbeBinary(version?: string): Promise<string> {
  try {
    // Create the local directory if it doesn't exist
    await fs.ensureDir(LOCAL_DIR);
    
    // Check if we already have the binary for this version
    const versionDir = version ? 
      path.join(LOCAL_DIR, version) : 
      path.join(LOCAL_DIR, 'latest');
    
    const isWindows = os.platform() === 'win32';
    const binaryName = isWindows ? `${BINARY_NAME}.exe` : BINARY_NAME;
    const binaryPath = path.join(versionDir, binaryName);
    
    // If the binary already exists, return its path
    if (await fs.pathExists(binaryPath)) {
      console.log(`Using existing binary at ${binaryPath}`);
      return binaryPath;
    }
    
    // Otherwise, download it
    const { os: osInfo, arch: archInfo } = detectOsArch();
    const { tag, assets } = await getLatestRelease(version);
    
    // Create a directory for this version
    const tagVersion = tag.startsWith('v') ? tag.substring(1) : tag;
    const versionDirWithTag = path.join(LOCAL_DIR, tagVersion);
    await fs.ensureDir(versionDirWithTag);
    
    const bestAsset = findBestAsset(assets, osInfo, archInfo);
    const { assetPath, checksumPath } = await downloadAsset(bestAsset, versionDirWithTag);
    
    const checksumValid = await verifyChecksum(assetPath, checksumPath);
    if (!checksumValid) {
      throw new Error('Checksum verification failed');
    }
    
    const extractedBinaryPath = await extractBinary(assetPath, versionDirWithTag);
    
    // Create a symlink or copy to the version-agnostic directory
    await fs.ensureDir(versionDir);
    const versionAgnosticBinaryPath = path.join(versionDir, binaryName);
    
    if (await fs.pathExists(versionAgnosticBinaryPath)) {
      await fs.remove(versionAgnosticBinaryPath);
    }
    
    if (isWindows) {
      // Windows doesn't support symlinks well, so copy the file
      await fs.copyFile(extractedBinaryPath, versionAgnosticBinaryPath);
    } else {
      // Create a symlink on Unix-like systems
      await fs.symlink(extractedBinaryPath, versionAgnosticBinaryPath);
    }
    
    console.log(`Binary ready at ${versionAgnosticBinaryPath}`);
    return versionAgnosticBinaryPath;
  } catch (error) {
    console.error('Error downloading probe binary:', error);
    throw error;
  }
}