#!/bin/bash
set -e

# Enable command printing for debugging
# set -x

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Print banner
echo -e "${BLUE}"
echo "╔═══════════════════════════════════════════╗"
echo "║                                           ║"
echo "║   Probe - Code Search Tool Installer      ║"
echo "║                                           ║"
echo "╚═══════════════════════════════════════════╝"
echo -e "${NC}"

# GitHub repository information
REPO_OWNER="probelabs"
REPO_NAME="probe"
BINARY_NAME="probe"
INSTALL_DIR="/usr/local/bin"

# Allow overriding the installation directory with the first argument
if [ -n "$1" ]; then
  INSTALL_DIR="$1"
  echo -e "${YELLOW}Custom installation directory: $INSTALL_DIR${NC}"
fi

# Check if running with sudo
if [ "$EUID" -ne 0 ]; then
  if command -v sudo > /dev/null; then
    SUDO="sudo"
    echo -e "${YELLOW}This script will use sudo to install to $INSTALL_DIR${NC}"
  else
    echo -e "${RED}Error: This script needs to be run with root privileges to install to $INSTALL_DIR${NC}"
    echo "Please run with sudo or as root"
    exit 1
  fi
else
  SUDO=""
fi

# Detect OS and architecture
detect_os_arch() {
  OS="$(uname -s)"
  ARCH="$(uname -m)"
  
  case "$OS" in
    Linux)
      OS_TYPE="linux"
      # Accept both musl and gnu-named assets; prefer match by 'linux'
      OS_KEYWORDS=("linux" "Linux" "musl" "gnu")
      ;;
    Darwin)
      OS_TYPE="darwin"
      OS_KEYWORDS=("darwin" "Darwin" "mac" "Mac" "apple" "Apple" "osx" "OSX")
      ;;
    MINGW*|MSYS*|CYGWIN*)
      OS_TYPE="windows"
      OS_KEYWORDS=("windows" "Windows" "win" "Win")
      ;;
    *)
      echo -e "${RED}Unsupported operating system: $OS${NC}"
      exit 1
      ;;
  esac
  
  case "$ARCH" in
    x86_64|amd64)
      ARCH_TYPE="x86_64"
      ARCH_KEYWORDS=("x86_64" "amd64" "x64" "64bit" "64-bit")
      ;;
    arm64|aarch64)
      ARCH_TYPE="aarch64"
      ARCH_KEYWORDS=("arm64" "aarch64" "arm" "ARM")
      ;;
    *)
      echo -e "${RED}Unsupported architecture: $ARCH${NC}"
      exit 1
      ;;
  esac
  
  echo -e "${GREEN}Detected OS: $OS_TYPE, Architecture: $ARCH_TYPE${NC}"
}

# Get the latest release information
get_latest_release() {
  echo -e "${BLUE}Fetching latest release information...${NC}"
  
  # Get the latest release data with headers
  RELEASE_RESPONSE=$(curl -s -i "https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/latest")
  
  # Check for rate limiting
  if echo "$RELEASE_RESPONSE" | grep -q "API rate limit exceeded"; then
    echo -e "${RED}GitHub API rate limit exceeded${NC}"
    echo -e "${YELLOW}Try again later${NC}"
    exit 1
  fi
  
  # Extract the response body
  RELEASE_DATA=$(echo "$RELEASE_RESPONSE" | sed '1,/^\r$/d')
  
  # Check if we got a valid response
  if echo "$RELEASE_RESPONSE" | grep -q "404 Not Found"; then
    echo -e "${YELLOW}Latest release not found, trying to fetch all releases...${NC}"
    
    # Try to get all releases instead
    ALL_RELEASES=$(curl -s "https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases")
    
    # Extract the first release tag
    LATEST_RELEASE=$(echo "$ALL_RELEASES" | grep -m 1 '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    
    if [ -z "$LATEST_RELEASE" ]; then
      echo -e "${RED}No releases found for $REPO_OWNER/$REPO_NAME${NC}"
      echo -e "${YELLOW}Checking if repository exists...${NC}"
      
      REPO_INFO=$(curl -s "https://api.github.com/repos/$REPO_OWNER/$REPO_NAME")
      if echo "$REPO_INFO" | grep -q '"message": "Not Found"'; then
        echo -e "${RED}Repository $REPO_OWNER/$REPO_NAME not found${NC}"
        exit 1
      else
        echo -e "${YELLOW}Repository exists but has no releases. Using default branch.${NC}"
        
        # Get the default branch
        DEFAULT_BRANCH=$(echo "$REPO_INFO" | grep '"default_branch":' | sed -E 's/.*"default_branch": "([^"]+)".*/\1/')
        LATEST_RELEASE="$DEFAULT_BRANCH"
        
        echo -e "${GREEN}Using default branch: $LATEST_RELEASE${NC}"
        
        # For default branch, we'll need to use a different approach to get files
        echo -e "${YELLOW}No release assets available. Please download the source code manually.${NC}"
        exit 1
      fi
    else
      # Extract assets from the first release
      RELEASE_DATA=$(echo "$ALL_RELEASES" | sed -n "/\"tag_name\": \"$LATEST_RELEASE\"/,/\"url\"/p")
    fi
  else
    # Extract the tag name from the latest release
    LATEST_RELEASE=$(echo "$RELEASE_DATA" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
  fi
  
  if [ -z "$LATEST_RELEASE" ]; then
    echo -e "${RED}Failed to fetch latest release tag${NC}"
    echo -e "${YELLOW}API Response:${NC}"
    echo "$RELEASE_DATA" | head -20
    exit 1
  fi
  
  echo -e "${GREEN}Latest release: $LATEST_RELEASE${NC}"
  
  # Extract the assets list using a more robust method
  ASSETS_LIST=$(echo "$RELEASE_DATA" | grep -o '"browser_download_url": "[^"]*"' | sed -E 's/"browser_download_url": "([^"]+)"/\1/')
    
  if [ -z "$ASSETS_LIST" ]; then
    echo -e "${RED}No assets found for release $LATEST_RELEASE${NC}"
    echo -e "${YELLOW}Trying alternative parsing method...${NC}"
    
    # Try an alternative parsing method
    ASSETS_LIST=$(echo "$RELEASE_DATA" | grep -A 2 '"browser_download_url"' | grep 'https://' | sed -E 's/.*"(https:[^"]+)".*/\1/')
    
    # Debug: Print extracted assets from alternative method
    echo "$ASSETS_LIST"
    
    if [ -z "$ASSETS_LIST" ]; then
      echo -e "${RED}Still no assets found. API response may be different than expected.${NC}"
      echo -e "${YELLOW}API Response (first 20 lines):${NC}"
      echo "$RELEASE_DATA" | head -20
      exit 1
    fi
  fi
  
  echo -e "${GREEN}Found $(echo "$ASSETS_LIST" | wc -l | tr -d ' ') assets for release $LATEST_RELEASE${NC}"
}

# Find the best matching asset for the current OS and architecture
find_best_asset() {
  echo -e "${BLUE}Finding appropriate binary for $OS_TYPE $ARCH_TYPE...${NC}"
  
  # Check if ASSETS_LIST is empty
  if [ -z "$ASSETS_LIST" ]; then
    echo -e "${RED}Error: Assets list is empty${NC}"
    echo -e "${YELLOW}This could be due to an issue with the GitHub API response${NC}"
    exit 1
  fi
  
  # Debug: Check if ASSETS_LIST contains valid URLs
  if ! echo "$ASSETS_LIST" | grep -q "http"; then
    echo -e "${RED}Error: Assets list does not contain valid URLs${NC}"
    echo -e "${YELLOW}Assets list content:${NC}"
    echo "$ASSETS_LIST"
    exit 1
  fi
  
  # Initialize variables
  BEST_ASSET=""
  BEST_SCORE=0
  
  # Process each asset
  while IFS= read -r asset_url; do
    asset_name=$(basename "$asset_url")
    score=0
    
    # Skip checksum files
    if [[ "$asset_name" == *.sha256 || "$asset_name" == *.md5 || "$asset_name" == *.asc ]]; then
      continue
    fi
    
    # Check for OS match
    for keyword in "${OS_KEYWORDS[@]}"; do
      if [[ "$asset_name" == *"$keyword"* ]]; then
        score=$((score + 5))
        break
      fi
    done
    
    # Check for architecture match
    for keyword in "${ARCH_KEYWORDS[@]}"; do
      if [[ "$asset_name" == *"$keyword"* ]]; then
        score=$((score + 5))
        break
      fi
    done
    
    # Prefer exact matches for binary name
    if [[ "$asset_name" == "$BINARY_NAME-"* ]]; then
      score=$((score + 3))
    fi
    
    # If we have a perfect match, use it immediately
    if [ $score -eq 13 ]; then
      BEST_ASSET="$asset_url"
      ASSET_NAME="$asset_name"
      echo -e "${GREEN}Found perfect match: $asset_name${NC}"
      
      # Set ASSET_URL before returning
      ASSET_URL="$BEST_ASSET"
      
      return 0
    fi
    
    # Otherwise, keep track of the best match so far
    if [ $score -gt $BEST_SCORE ]; then
      BEST_SCORE=$score
      BEST_ASSET="$asset_url"
      ASSET_NAME="$asset_name"
    fi
  done <<< "$ASSETS_LIST"
  
  # Debug: Print assets list
  echo "$ASSETS_LIST"
  
  # Check if we found a suitable asset
  if [ -z "$BEST_ASSET" ]; then
    echo -e "${RED}Could not find a suitable binary for $OS_TYPE $ARCH_TYPE${NC}"
    echo -e "${YELLOW}Available assets:${NC}"
    echo "$ASSETS_LIST"
    exit 1
  fi
  
  echo -e "${GREEN}Selected asset: $ASSET_NAME (score: $BEST_SCORE)${NC}"
  
  # Debug: Print best asset before assignment
  
  # Ensure BEST_ASSET is not empty
  if [ -z "$BEST_ASSET" ]; then
    echo -e "${RED}Error: Best asset is empty even though we passed the earlier check${NC}"
    exit 1
  fi
  
  ASSET_URL="$BEST_ASSET"
  
  # Debug: Print selected asset URL
}

# Download the asset and checksum
download_asset() {
  TEMP_DIR=$(mktemp -d)
  
  # Debug: Print asset URL and name
  
  # Check if ASSET_URL or ASSET_NAME is empty
  if [ -z "$ASSET_URL" ]; then
    echo -e "${RED}Error: Asset URL is empty${NC}"
    echo -e "${YELLOW}This could be due to an issue with the GitHub API response or asset selection${NC}"
    exit 1
  fi
  
  if [ -z "$ASSET_NAME" ]; then
    echo -e "${RED}Error: Asset name is empty${NC}"
    echo -e "${YELLOW}This could be due to an issue with the asset selection process${NC}"
    exit 1
  fi
  
  # Check if ASSET_URL is a valid URL
  if ! echo "$ASSET_URL" | grep -q "^https\?://"; then
    echo -e "${RED}Error: Asset URL is not a valid URL: $ASSET_URL${NC}"
    echo -e "${YELLOW}This could be due to an issue with the GitHub API response or asset selection${NC}"
    exit 1
  fi
  
  CHECKSUM_URL="$ASSET_URL.sha256"
  
  echo -e "${BLUE}Downloading $ASSET_NAME...${NC}"
  curl -L -o "$TEMP_DIR/$ASSET_NAME" "$ASSET_URL" || {
    echo -e "${RED}Failed to download $ASSET_NAME${NC}"
    exit 1
  }
  
  echo -e "${BLUE}Downloading checksum...${NC}"
  if curl -L -s -f -o "$TEMP_DIR/$ASSET_NAME.sha256" "$CHECKSUM_URL"; then
    HAS_CHECKSUM=true
  else
    echo -e "${YELLOW}No checksum file found, skipping verification${NC}"
    HAS_CHECKSUM=false
  fi
  
  return 0
}

# Verify the checksum
verify_checksum() {
  if [ "$HAS_CHECKSUM" = false ]; then
    return 0
  fi
  
  echo -e "${BLUE}Verifying checksum...${NC}"
  
  cd "$TEMP_DIR"
  
  if [ "$OS_TYPE" = "darwin" ] || [ "$OS_TYPE" = "linux" ]; then
    # Get the expected checksum from the file
    EXPECTED_CHECKSUM=$(cat "$ASSET_NAME.sha256" | awk '{print $1}')
    
    # Calculate the actual checksum
    if command -v shasum > /dev/null; then
      ACTUAL_CHECKSUM=$(shasum -a 256 "$ASSET_NAME" | awk '{print $1}')
    else
      ACTUAL_CHECKSUM=$(sha256sum "$ASSET_NAME" | awk '{print $1}')
    fi
  else
    # Windows
    EXPECTED_CHECKSUM=$(cat "$ASSET_NAME.sha256")
    ACTUAL_CHECKSUM=$(certutil -hashfile "$ASSET_NAME" SHA256 | grep -v "hash" | tr -d " \t\r\n")
  fi
  
  if [ "$EXPECTED_CHECKSUM" != "$ACTUAL_CHECKSUM" ]; then
    echo -e "${RED}Checksum verification failed!${NC}"
    echo "Expected: $EXPECTED_CHECKSUM"
    echo "Actual: $ACTUAL_CHECKSUM"
    exit 1
  fi
  
  echo -e "${GREEN}Checksum verified successfully${NC}"
}

# Extract and install the binary
install_binary() {
  echo -e "${BLUE}Installing $BINARY_NAME to $INSTALL_DIR...${NC}"
  
  cd "$TEMP_DIR"
  
  # Determine file type and extract accordingly
  if [[ "$ASSET_NAME" == *.tar.gz || "$ASSET_NAME" == *.tgz ]]; then
    tar -xzf "$ASSET_NAME"
  elif [[ "$ASSET_NAME" == *.zip ]]; then
    unzip -q "$ASSET_NAME"
  elif [[ "$ASSET_NAME" == *.tar.bz2 || "$ASSET_NAME" == *.tbz2 ]]; then
    tar -xjf "$ASSET_NAME"
  elif [[ "$ASSET_NAME" == *.tar.xz || "$ASSET_NAME" == *.txz ]]; then
    tar -xJf "$ASSET_NAME"
  else
    # Assume it's a direct binary
    chmod +x "$ASSET_NAME"
    $SUDO mv "$ASSET_NAME" "$INSTALL_DIR/$BINARY_NAME"
    echo -e "${GREEN}Successfully installed $BINARY_NAME to $INSTALL_DIR/$BINARY_NAME${NC}"
    cd - > /dev/null
    rm -rf "$TEMP_DIR"
    return 0
  fi
  
  # Find the binary in the extracted files
  if [ "$OS_TYPE" = "windows" ]; then
    BINARY_FILE=$(find . -type f -name "$BINARY_NAME.exe" | head -1)
    if [ -z "$BINARY_FILE" ]; then
      BINARY_FILE=$(find . -type f -name "*.exe" | head -1)
    fi
  else
    BINARY_FILE=$(find . -type f -name "$BINARY_NAME" | head -1)
    if [ -z "$BINARY_FILE" ]; then
      # Look for any executable file
      BINARY_FILE=$(find . -type f -executable | grep -v "\.sh$" | head -1)
    fi
  fi
  
  if [ -z "$BINARY_FILE" ]; then
    echo -e "${RED}Binary not found in the archive${NC}"
    echo -e "${YELLOW}Archive contents:${NC}"
    find . -type f | sort
    exit 1
  fi
  
  chmod +x "$BINARY_FILE"
  $SUDO mv "$BINARY_FILE" "$INSTALL_DIR/$BINARY_NAME"
  
  echo -e "${GREEN}Successfully installed $BINARY_NAME to $INSTALL_DIR/$BINARY_NAME${NC}"
  
  # Clean up
  cd - > /dev/null
  rm -rf "$TEMP_DIR"
}

# Check if the binary is in PATH
check_path() {
  if command -v "$BINARY_NAME" > /dev/null; then
    echo -e "${GREEN}$BINARY_NAME is now available in your PATH${NC}"
    echo -e "${BLUE}You can run it with: ${GREEN}$BINARY_NAME${NC}"
    
    # Display version if available
    if $BINARY_NAME --version &>/dev/null; then
      echo -e "${BLUE}Installed version: ${GREEN}$($BINARY_NAME --version)${NC}"
    elif $BINARY_NAME version &>/dev/null; then
      echo -e "${BLUE}Installed version: ${GREEN}$($BINARY_NAME version)${NC}"
    fi
  else
    echo -e "${YELLOW}Warning: $INSTALL_DIR is not in your PATH${NC}"
    echo -e "You can run the binary with: ${GREEN}$INSTALL_DIR/$BINARY_NAME${NC}"
    echo -e "To add it to your PATH, add this line to your shell profile:"
    echo -e "${BLUE}export PATH=\$PATH:$INSTALL_DIR${NC}"
  fi
}

# Main installation process
main() {
  detect_os_arch
  get_latest_release
  find_best_asset
  
  # Debug: Check if ASSET_URL is set before proceeding
  if [ -z "$ASSET_URL" ]; then
    echo -e "${RED}Error: Asset URL is not set after find_best_asset${NC}"
    exit 1
  fi
  
  download_asset
  verify_checksum
  install_binary
  check_path
  
  echo -e "${GREEN}Installation completed successfully!${NC}"
}

# Run the installation
main
