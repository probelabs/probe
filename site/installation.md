# Installation

Probe can be installed in several ways, depending on your preferences and system requirements.

## Quick Installation

You can install Probe in several ways - choose the method that best fits your environment:

### Using Docker (Recommended for containerized environments)

```bash
# Pull the CLI image
docker pull buger/probe:latest

# Create an alias for easy usage
alias probe='docker run --rm -v $(pwd):/workspace buger/probe'

# For the AI chat interface
docker pull buger/probe-chat:latest
```

**Benefits of Docker installation**:
- No local dependencies required
- Consistent environment across systems
- Multi-platform support (AMD64, ARM64)
- Easy CI/CD integration
- Isolated execution environment

[→ Full Docker documentation](./integrations/docker.md)

### Using npm (Recommended for Node.js users)

```bash
npm install -g @probelabs/probe@latest
```

### Using curl (For macOS and Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/buger/probe/main/install.sh | bash
```

**What the curl script does**:

1. Detects your operating system and architecture
2. Fetches the latest release from GitHub
3. Downloads the appropriate binary for your system
4. Verifies the checksum for security
5. Installs the binary to `/usr/local/bin`

### Using PowerShell (For Windows)

```powershell
iwr -useb https://raw.githubusercontent.com/buger/probe/main/install.ps1 | iex
```

**What the PowerShell script does**:

1. Detects your system architecture (x86_64 or ARM64)
2. Fetches the latest release from GitHub
3. Downloads the appropriate Windows binary
4. Verifies the checksum for security
5. Installs the binary to your user directory (`%LOCALAPPDATA%\Probe`) by default
6. Provides instructions to add the binary to your PATH if needed

**Installation options**:

The PowerShell script supports several options:

```powershell
# Install for current user (default)
iwr -useb https://raw.githubusercontent.com/buger/probe/main/install.ps1 | iex

# Install system-wide (requires admin privileges)
iwr -useb https://raw.githubusercontent.com/buger/probe/main/install.ps1 | iex -args "--system"

# Install to a custom directory
iwr -useb https://raw.githubusercontent.com/buger/probe/main/install.ps1 | iex -args "--dir", "C:\Tools\Probe"

# Show help
iwr -useb https://raw.githubusercontent.com/buger/probe/main/install.ps1 | iex -args "--help"
```

## Requirements

- **Operating Systems**: macOS, Linux, or Windows
- **Architectures**: x86_64 (all platforms) or ARM64 (macOS and Windows)
- **Tools**:
  - For macOS/Linux: `curl`, `bash`, and `sudo`/root privileges
  - For Windows: PowerShell 5.1 or later

## Manual Installation

If you prefer to install manually or the quick installation script doesn't work for your system:

1. Download the appropriate binary for your platform from the [GitHub Releases](https://github.com/probelabs/probe/releases) page:
   - `probe-x86_64-linux.tar.gz` for Linux (x86_64)
   - `probe-x86_64-darwin.tar.gz` for macOS (Intel)
   - `probe-aarch64-darwin.tar.gz` for macOS (Apple Silicon)
   - `probe-x86_64-windows.zip` for Windows (x86_64)
   - `probe-aarch64-windows.zip` for Windows (ARM64)

2. Extract the archive:
   ```bash
   # For Linux/macOS
   tar -xzf probe-*-*.tar.gz
   
   # For Windows (using PowerShell)
   Expand-Archive -Path probe-*-windows.zip -DestinationPath .\probe
   ```

3. Move the binary to a location in your PATH:
   ```bash
   # For Linux/macOS
   sudo mv probe /usr/local/bin/
   
   # For Windows (using PowerShell)
   # Create a directory for the binary (if it doesn't exist)
   $installDir = "$env:LOCALAPPDATA\Probe"
   New-Item -ItemType Directory -Path $installDir -Force
   
   # Move the binary
   Move-Item -Path .\probe\probe.exe -Destination $installDir
   
   # Add to PATH (optional)
   [Environment]::SetEnvironmentVariable('PATH', [Environment]::GetEnvironmentVariable('PATH', 'User') + ";$installDir", 'User')
   ```

## Building from Source

For developers who want to build Probe from source:

1. Install Rust and Cargo (if not already installed):
   
   For macOS/Linux:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
   
   For Windows:
   ```powershell
   # Download and run the Rust installer
   Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
   .\rustup-init.exe
   # Follow the on-screen instructions
   ```

2. Clone the repository:
   ```bash
   git clone https://github.com/probelabs/probe.git
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
   
   This will install the binary to your Cargo bin directory, which is typically:
   - `$HOME/.cargo/bin` on macOS/Linux
   - `%USERPROFILE%\.cargo\bin` on Windows

## Verifying the Installation

To verify that Probe has been installed correctly:

For macOS/Linux:
```bash
probe --version
```

For Windows:
```powershell
probe --version
```

This should display the version number of the installed Probe binary.

If you get a "command not found" error on Windows, make sure the installation directory is in your PATH or use the full path to the executable:
```powershell
# If installed to the default user location
& "$env:LOCALAPPDATA\Probe\probe.exe" --version

# If installed to the default system location
& "$env:ProgramFiles\Probe\probe.exe" --version
```

## Troubleshooting

- **Permissions**:
  - For macOS/Linux: Ensure you can write to `/usr/local/bin`
  - For Windows: Ensure you have write permissions to the installation directory
- **System Requirements**: Double-check your OS/architecture compatibility
- **PATH Issues**:
  - For Windows: Restart your terminal after adding the installation directory to PATH
  - For macOS/Linux: Verify that `/usr/local/bin` is in your PATH
- **Manual Install**: If the quick install script fails, try the manual installation method
- **GitHub Issues**: Report issues on the [GitHub repository](https://github.com/probelabs/probe/issues)

## Uninstalling

To uninstall Probe:

```bash
# If installed via npm
npm uninstall -g @probelabs/probe@latest

# If installed via curl script or manually on macOS/Linux
sudo rm /usr/local/bin/probe
```

For Windows:

```powershell
# If installed via PowerShell script to default user location
Remove-Item -Path "$env:LOCALAPPDATA\Probe\probe.exe" -Force

# If installed via PowerShell script to system location
Remove-Item -Path "$env:ProgramFiles\Probe\probe.exe" -Force

# If you added the installation directory to PATH, you may want to remove it
# For user PATH:
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
$userPath = ($userPath -split ';' | Where-Object { $_ -ne "$env:LOCALAPPDATA\Probe" }) -join ';'
[Environment]::SetEnvironmentVariable('PATH', $userPath, 'User')

# For system PATH (requires admin privileges):
$systemPath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
$systemPath = ($systemPath -split ';' | Where-Object { $_ -ne "$env:ProgramFiles\Probe" }) -join ';'
[Environment]::SetEnvironmentVariable('PATH', $systemPath, 'Machine')