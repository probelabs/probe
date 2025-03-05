#!/bin/bash
set -e

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
REPO_OWNER="leonidbugaev"
REPO_NAME="code-search"
BINARY_NAME="probe"
INSTALL_DIR="/usr/local/bin"

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
      ;;
    Darwin)
      OS_TYPE="darwin"
      ;;
    MINGW*|MSYS*|CYGWIN*)
      OS_TYPE="windows"
      ;;
    *)
      echo -e "${RED}Unsupported operating system: $OS${NC}"
      exit 1
      ;;
  esac
  
  case "$ARCH" in
    x86_64|amd64)
      ARCH_TYPE="x86_64"
      ;;
    arm64|aarch64)
      ARCH_TYPE="aarch64"
      ;;
    *)
      echo -e "${RED}Unsupported architecture: $ARCH${NC}"
      exit 1
      ;;
  esac
  
  # Special case: aarch64 is only supported on macOS
  if [ "$ARCH_TYPE" = "aarch64" ] && [ "$OS_TYPE" != "darwin" ]; then
    echo -e "${RED}Architecture $ARCH is only supported on macOS${NC}"
    exit 1
  fi
  
  if [ "$OS_TYPE" = "windows" ]; then
    ASSET_NAME="$BINARY_NAME-$ARCH_TYPE-windows.zip"
  else
    ASSET_NAME="$BINARY_NAME-$ARCH_TYPE-$OS_TYPE.tar.gz"
  fi
  
  echo -e "${GREEN}Detected OS: $OS_TYPE, Architecture: $ARCH_TYPE${NC}"
  echo -e "${GREEN}Will download asset: $ASSET_NAME${NC}"
}

# Get the latest release tag
get_latest_release() {
  echo -e "${BLUE}Fetching latest release...${NC}"
  
  LATEST_RELEASE=$(curl -s "https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/latest" | 
                  grep '"tag_name":' | 
                  sed -E 's/.*"([^"]+)".*/\1/')
  
  if [ -z "$LATEST_RELEASE" ]; then
    echo -e "${RED}Failed to fetch latest release tag${NC}"
    exit 1
  fi
  
  echo -e "${GREEN}Latest release: $LATEST_RELEASE${NC}"
}

# Download the asset and checksum
download_asset() {
  TEMP_DIR=$(mktemp -d)
  ASSET_URL="https://github.com/$REPO_OWNER/$REPO_NAME/releases/download/$LATEST_RELEASE/$ASSET_NAME"
  CHECKSUM_URL="$ASSET_URL.sha256"
  
  echo -e "${BLUE}Downloading $ASSET_NAME...${NC}"
  curl -L -o "$TEMP_DIR/$ASSET_NAME" "$ASSET_URL" || {
    echo -e "${RED}Failed to download $ASSET_NAME${NC}"
    exit 1
  }
  
  echo -e "${BLUE}Downloading checksum...${NC}"
  curl -L -o "$TEMP_DIR/$ASSET_NAME.sha256" "$CHECKSUM_URL" || {
    echo -e "${RED}Failed to download checksum${NC}"
    exit 1
  }
  
  return 0
}

# Verify the checksum
verify_checksum() {
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
  echo -e "${BLUE}Installing $BINARY_NAME...${NC}"
  
  cd "$TEMP_DIR"
  
  if [ "$OS_TYPE" = "windows" ]; then
    unzip -q "$ASSET_NAME"
    BINARY_FILE="$BINARY_NAME.exe"
  else
    tar -xzf "$ASSET_NAME"
    BINARY_FILE="$BINARY_NAME"
  fi
  
  if [ ! -f "$BINARY_FILE" ]; then
    echo -e "${RED}Binary not found in the archive${NC}"
    exit 1
  fi
  
  chmod +x "$BINARY_FILE"
  $SUDO mv "$BINARY_FILE" "$INSTALL_DIR/"
  
  echo -e "${GREEN}Successfully installed $BINARY_NAME to $INSTALL_DIR/${BINARY_FILE}${NC}"
  
  # Clean up
  cd - > /dev/null
  rm -rf "$TEMP_DIR"
}

# Check if the binary is in PATH
check_path() {
  if command -v "$BINARY_NAME" > /dev/null; then
    echo -e "${GREEN}$BINARY_NAME is now available in your PATH${NC}"
    echo -e "${BLUE}You can run it with: ${GREEN}$BINARY_NAME${NC}"
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
  download_asset
  verify_checksum
  install_binary
  check_path
  
  echo -e "${GREEN}Installation completed successfully!${NC}"
}

# Run the installation
main