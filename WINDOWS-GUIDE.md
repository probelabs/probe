# Probe for Windows - Comprehensive Guide

This guide provides detailed instructions for compiling, running, and troubleshooting the Probe code search tool on Windows systems.

## Prerequisites

Before using Probe on Windows, ensure you have the following:

1. **Microsoft Visual C++ Redistributable 2015-2022 (x64)**
   - Required for running the Probe executable
   - Download from: https://aka.ms/vs/17/release/vc_redist.x64.exe
   - Install before running Probe

2. **Rust and Cargo**
   - Required for compiling Probe from source
   - Install from https://www.rust-lang.org/tools/install
   - Minimum recommended version: 1.70.0

3. **Node.js and npm**
   - Required for running the MCP server
   - Install from https://nodejs.org/
   - Minimum recommended version: Node.js 18.x

4. **Git**
   - For cloning the repository (if needed)
   - Install from https://git-scm.com/download/win

## Compiling Probe on Windows

We've optimized Probe's build configuration specifically for Windows compatibility. The `build-windows.bat` script provided in this repository simplifies the build process.

### Using the build-windows.bat Script

1. Open Command Prompt or PowerShell
2. Navigate to the Probe repository directory
3. Run the build script:
   ```
   .\build-windows.bat
   ```

The script will:
- Check for prerequisites
- Build Probe with Windows-specific settings
- Verify the build was successful
- Place the executable in `.\target\release\probe.exe`

### Using WSL (Windows Subsystem for Linux)

If you prefer to use WSL for development on Windows:

1. Install WSL by running in PowerShell as Administrator:
   ```
   wsl --install
   ```

2. Install Rust in your WSL environment:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. Clone and build Probe in WSL:
   ```bash
   git clone https://github.com/buger/probe.git
   cd probe
   cargo build --release
   ```

4. The binary will be available at `./target/release/probe` within WSL

### Manual Compilation

If you prefer to compile manually:

1. Open Command Prompt or PowerShell
2. Navigate to the Probe repository directory
3. Run the following commands:

```
cargo clean
cargo build --release
```

The Windows-specific settings in `Cargo.toml` will be automatically applied:
- Standard optimization level (opt-level = 2) for better compatibility
- Thin LTO for faster linking and better compatibility
- More codegen units for better compatibility with Windows toolchain
- Static linking of C runtime to reduce dependency issues

## Running Probe on Windows

### Command Line Usage

After compiling, you can run Probe directly:

```
.\target\release\probe.exe search "query" path\to\search
```

Common commands:

```
# Basic search
.\target\release\probe.exe search "function" C:\path\to\codebase

# Search with specific file pattern
.\target\release\probe.exe search "query" --file-pattern "*.js" C:\path\to\codebase

# Extract code using patterns
.\target\release\probe.exe query "fn $NAME($$$PARAMS) $$$BODY" --language rust C:\path\to\codebase
```

### Debug Mode

For more detailed logging, set the DEBUG environment variable:

```
set DEBUG=1
.\target\release\probe.exe search "query" path\to\search
```

## Running the MCP Server

The MCP (Model Context Protocol) server provides a way for AI tools to interact with Probe. 

### MCP Server Setup

To start the MCP server:

1. Navigate to the MCP directory:
   ```
   cd mcp
   ```

2. Install dependencies:
   ```
   npm install
   ```

3. Build the server:
   ```
   npm run build
   ```

4. Start the server:
   ```
   node build\index.js
   ```

### Using Claude Code with WSL

If you have Claude Code installed in WSL, the Probe chat examples will automatically detect and use it:

1. Install Claude Code in WSL:
   ```bash
   # In WSL terminal
   npm install -g @anthropic-ai/claude-code
   ```

2. The probe-chat tool will automatically detect Claude Code in WSL when running from Windows:
   ```
   npx @buger/probe-chat
   ```

3. To verify detection, run with debug mode:
   ```
   set DEBUG_CHAT=1
   npx @buger/probe-chat
   ```

The tool will attempt to use Claude Code in the following order:
1. Direct execution (if in PATH)
2. npm global installation (Windows)
3. WSL installation (if available)
4. Common installation paths

## Troubleshooting Windows Issues

### Missing VCRUNTIME140.dll

If you see an error like `The program can't start because VCRUNTIME140.dll is missing`:

1. Download and install the Microsoft Visual C++ Redistributable from: https://aka.ms/vs/17/release/vc_redist.x64.exe
2. Restart your terminal
3. Try running Probe again

### Access Denied or Permission Issues

If you encounter permission-related errors:

1. Try running the Command Prompt or PowerShell as Administrator
2. Check if Windows Defender or other antivirus software is blocking execution
3. Verify that the executable has not been quarantined

### Missing GCC Compiler

If you encounter the following error during `cargo build --release`:

```
error occurred in cc-rs: failed to find tool "gcc.exe": program not found 
(see https://docs.rs/cc/latest/cc/#compile-time-requirements for help)
```

This indicates that you need to install a C/C++ compiler toolchain. Follow these steps to fix the issue:

1. Install MSYS2 using Windows Package Manager (winget):
   ```
   winget install MSYS2.MSYS2
   ```

2. Open the "MSYS2 MSYS2" shortcut from the Windows Start Menu

3. Install the MinGW-w64 toolchain by running this command in the MSYS2 terminal:
   ```
   pacman -S --needed base-devel mingw-w64-ucrt-x86_64-toolchain
   ```
   (Accept all defaults when prompted)

4. Add the compiler to your PATH:
   - Open Windows Settings
   - Go to System > About > Advanced system settings
   - Click on "Environment Variables"
   - Edit the "Path" variable under either User or System variables
   - Add `C:\msys64\ucrt64\bin` to the list
   - Click OK and close all dialogs

5. Restart any open command prompts, PowerShell windows, or VSCode

6. Try running `cargo build --release` again

For more detailed information, see:
- [cc-rs compile-time requirements](https://docs.rs/cc/latest/cc/#compile-time-requirements)
- [VS Code MinGW setup](https://code.visualstudio.com/docs/cpp/config-mingw#_installing-the-mingww64-toolchain)

### Slow Execution on Windows

If Probe seems slower on Windows:

1. Try using the `--max-threads` option to limit thread usage:
   ```
   .\target\release\probe.exe search "query" --max-threads 4 path\to\search
   ```

2. Consider using smaller search paths to reduce memory usage

### MCP Server Connection Issues

If the MCP server fails to start or connect:

1. Check if the Probe executable is correctly built and accessible
2. Verify that no other service is using port 3000 (the default MCP port)
3. Try running the MCP server with verbose logging:
   ```
   set DEBUG=1
   cd mcp && node build\index.js
   ```

## Windows Compatibility Testing

To verify that Probe is working correctly on your Windows system, you can run our compatibility test suite:

```
.\tests\test-windows-compatibility.bat
```

This will run a series of tests that check:
- MSVC Redistributable detection
- Basic Probe functionality
- MCP server operation

For more detailed information about these tests, see `tests\WINDOWS_TESTING.md`.

## Environment Variables

Several environment variables can modify Probe's behavior on Windows:

| Variable | Description |
|----------|-------------|
| `PROBE_AUTO_INSTALL_MSVC` | Set to `true` to attempt automatic download of the required redistributable during installation |
| `DEBUG` | Set to any value to enable debug logging |
| `PROBE_MAX_THREADS` | Override the default thread count |

## Building for Distribution

When building Probe for distribution to other Windows users:

1. Use the `build-windows.bat` script which applies the optimized Windows settings
2. Test the executable on a clean Windows installation to verify all dependencies are properly handled
3. Include the Microsoft Visual C++ Redistributable requirement in your documentation

## Additional Resources

- [Rust on Windows Documentation](https://doc.rust-lang.org/book/ch01-01-installation.html#windows)
- [Microsoft Visual C++ Redistributable Overview](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)
- [Node.js Windows Installation Guide](https://nodejs.org/en/download/package-manager/#windows)