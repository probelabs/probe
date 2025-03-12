# Installation

Probe can be installed in several ways, depending on your preferences and system requirements.

## Quick Installation

You can install Probe with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/buger/probe/main/install.sh | bash
```

**What this script does**:

1. Detects your operating system and architecture
2. Fetches the latest release from GitHub
3. Downloads the appropriate binary for your system
4. Verifies the checksum for security
5. Installs the binary to `/usr/local/bin`

## Requirements

- **Operating Systems**: macOS, Linux, or Windows (with MSYS/Git Bash/WSL)
- **Architectures**: x86_64 (all platforms) or ARM64 (macOS only)
- **Tools**: `curl`, `bash`, and `sudo`/root privileges

## Manual Installation

If you prefer to install manually or the quick installation script doesn't work for your system:

1. Download the appropriate binary for your platform from the [GitHub Releases](https://github.com/buger/probe/releases) page:
   - `probe-x86_64-linux.tar.gz` for Linux (x86_64)
   - `probe-x86_64-darwin.tar.gz` for macOS (Intel)
   - `probe-aarch64-darwin.tar.gz` for macOS (Apple Silicon)
   - `probe-x86_64-windows.zip` for Windows

2. Extract the archive:
   ```bash
   # For Linux/macOS
   tar -xzf probe-*-*.tar.gz
   
   # For Windows
   unzip probe-x86_64-windows.zip
   ```

3. Move the binary to a location in your PATH:
   ```bash
   # For Linux/macOS
   sudo mv probe /usr/local/bin/
   
   # For Windows
   # Move probe.exe to a directory in your PATH
   ```

## Building from Source

For developers who want to build Probe from source:

1. Install Rust and Cargo (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Clone the repository:
   ```bash
   git clone https://github.com/buger/probe.git
   cd probe
   ```

3. Build the project:
   ```bash
   cargo build --release
   ```

4. (Optional) Install globally:
   ```bash
   cargo install --path .
   ```

## Verifying the Installation

To verify that Probe has been installed correctly:

```bash
probe --version
```

This should display the version number of the installed Probe binary.

## Troubleshooting

- **Permissions**: Ensure you can write to `/usr/local/bin`
- **System Requirements**: Double-check your OS/architecture
- **Manual Install**: If the quick install script fails, try the manual installation method
- **GitHub Issues**: Report issues on the [GitHub repository](https://github.com/buger/probe/issues)

## Uninstalling

To uninstall Probe:

```bash
sudo rm /usr/local/bin/probe