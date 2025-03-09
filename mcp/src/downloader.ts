import axios from 'axios';
import fs from 'fs-extra';
import path from 'path';
import { createHash } from 'crypto';
import { promisify } from 'util';
import { exec as execCallback } from 'child_process';
import tar from 'tar';
import os from 'os';
import { fileURLToPath } from 'url';

const exec = promisify(execCallback);

// GitHub repository information
const REPO_OWNER = "buger";
const REPO_NAME = "probe";
const BINARY_NAME = "probe";

// Get the directory of the current module
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Local storage directory for downloaded binaries - relative to the MCP directory
const LOCAL_DIR = path.resolve(__dirname, '..', '..', 'bin');

// Version info file path
const VERSION_INFO_PATH = path.join(LOCAL_DIR, 'version-info.json');

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
      const assets: ReleaseAsset[] = bestRelease.assets.map((asset: any) => ({
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
      await exec(`unzip -q "${assetPath}" -d "${extractDir}"`);
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
    const findBinary = async (dir: string): Promise<string | null> => {
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
 */
async function getVersionInfo(): Promise<{ version: string; lastUpdated: string } | null> {
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
 */
async function saveVersionInfo(version: string): Promise<void> {
  const versionInfo = {
    version,
    lastUpdated: new Date().toISOString()
  };
  
  await fs.writeFile(VERSION_INFO_PATH, JSON.stringify(versionInfo, null, 2));
  console.log(`Version info saved: ${version}`);
}

/**
 * Downloads the probe binary
 */
export async function downloadProbeBinary(version?: string): Promise<string> {
  try {
    // Create the bin directory if it doesn't exist
    await fs.ensureDir(LOCAL_DIR);
    
    console.log(`Downloading probe binary (version: ${version || 'latest'})...`);
    
    const isWindows = os.platform() === 'win32';
    const binaryName = isWindows ? `${BINARY_NAME}.exe` : BINARY_NAME;
    const binaryPath = path.join(LOCAL_DIR, binaryName);
    
    // Check if the binary already exists and version matches
    if (await fs.pathExists(binaryPath)) {
      const versionInfo = await getVersionInfo();
      
      // If no specific version was requested or versions match, use existing binary
      if ((!version || version === '0.0.0' || (versionInfo && versionInfo.version === version))) {
        console.log(`Using existing binary at ${binaryPath} (version: ${versionInfo?.version || 'unknown'})`);
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