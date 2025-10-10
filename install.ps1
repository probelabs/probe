# Probe - Code Search Tool Installer for Windows
# PowerShell version of the install.sh script

# Set error action preference to stop on any error
$ErrorActionPreference = "Stop"

# Function to write colored output
function Write-ColorOutput {
    param (
        [Parameter(Mandatory=$true)]
        [string]$Message,
        
        [Parameter(Mandatory=$false)]
        [string]$Color = "White"
    )
    
    Write-Host $Message -ForegroundColor $Color
}

# Print banner
Write-ColorOutput -Message "+-----------------------------------------------+" -Color 'Cyan'
Write-ColorOutput -Message "|                                               |" -Color 'Cyan'
Write-ColorOutput -Message "|     Probe - Code Search Tool Installer        |" -Color 'Cyan'
Write-ColorOutput -Message "|                                               |" -Color 'Cyan'
Write-ColorOutput -Message "+-----------------------------------------------+" -Color 'Cyan'

# GitHub repository information
$RepoOwner = "probelabs"
$RepoName = "probe"
$BinaryName = "probe"
$DefaultSystemDir = "$env:ProgramFiles\Probe"  # Default system-wide install directory

$DefaultUserDir = "$env:LOCALAPPDATA\Probe"    # Default user-level install directory
$InstallDir = $DefaultUserDir                  # Default to user-level installation
$InstallMode = "user"                          # Default installation mode

# Parse command-line arguments
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -eq "--system" -or $args[$i] -eq "-s") {
        $InstallDir = $DefaultSystemDir
        $InstallMode = "system"
        Write-ColorOutput -Message "System-wide installation selected (requires admin privileges)" -Color 'Yellow'
    }
    elseif ($args[$i] -eq "--user" -or $args[$i] -eq "-u") {
        $InstallDir = $DefaultUserDir
        $InstallMode = "user"
        Write-ColorOutput -Message "User-level installation selected" -Color 'Green'
    }
    elseif ($args[$i] -eq "--dir" -or $args[$i] -eq "-d") {
        if ($i + 1 -lt $args.Count) {
            $InstallDir = $args[$i + 1]
            $i++
            Write-ColorOutput -Message "Custom installation directory: $InstallDir" -Color 'Yellow'
        }
    }
    elseif ($args[$i] -eq "--help" -or $args[$i] -eq "-h") {
        Write-ColorOutput -Message "Usage: .\install.ps1 [options]" -Color 'Cyan'
        Write-ColorOutput -Message "Options:" -Color 'Cyan'
        Write-ColorOutput -Message "  --system, -s     Install system-wide (requires admin privileges)" -Color 'Cyan'
        Write-ColorOutput -Message "  --user, -u       Install for current user only (default)" -Color 'Cyan'
        Write-ColorOutput -Message "  --dir, -d DIR    Install to a custom directory" -Color 'Cyan'
        Write-ColorOutput -Message "  --help, -h       Show this help message" -Color 'Cyan'
        exit 0
    }

}

# Check for administrator privileges
$IsAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $IsAdmin) {
    if ($InstallMode -eq "system") {
        Write-ColorOutput -Message 'Error: System-wide installation requires administrator privileges' -Color 'Red'
        Write-Host 'Please run PowerShell as Administrator and try again, or use --user for user-level installation.'
        exit 1
    }
    Write-ColorOutput -Message 'Running in user mode - installing to user directory' -Color 'Yellow'

}

# Detect architecture
function Get-SystemArchitecture {
    $Arch = [System.Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE")
    
    if ($Arch -eq "AMD64") {
        $ArchType = "x86_64"
        $ArchKeywords = @("x86_64", "amd64", "x64", "64bit", "64-bit")
    }
    elseif ($Arch -eq "ARM64") {
        $ArchType = "aarch64"
        $ArchKeywords = @("arm64", "aarch64", "arm", "ARM")
    }
    else {
        Write-ColorOutput -Message "Unsupported architecture: $Arch" -Color 'Red'
        exit 1
    }
    
    Write-ColorOutput -Message "Detected OS: windows, Architecture: $ArchType" -Color 'Green'
    
    return @{
        ArchType = $ArchType
        ArchKeywords = $ArchKeywords
        OSType = "windows"
        OSKeywords = @("windows", "Windows", "win", "Win")
    }
}

# Get the latest release information
function Get-LatestRelease {
    Write-ColorOutput -Message 'Fetching latest release information...' -Color 'Cyan'
    
    try {
        $ReleaseResponse = Invoke-RestMethod -Uri "https://api.github.com/repos/$RepoOwner/$RepoName/releases/latest" -ErrorAction Stop

        if ($null -eq $ReleaseResponse) {
            Write-ColorOutput -Message 'Latest release not found, trying to fetch all releases...' -Color 'Yellow'
            
            $AllReleases = Invoke-RestMethod -Uri "https://api.github.com/repos/$RepoOwner/$RepoName/releases" -ErrorAction SilentlyContinue
            
            if ($null -eq $AllReleases -or $AllReleases.Count -eq 0) {
                Write-ColorOutput -Message "No releases found for $RepoOwner/$RepoName" -Color 'Red'
                Write-ColorOutput -Message 'Checking if repository exists...' -Color 'Yellow'

                try {
                    $RepoInfo = Invoke-RestMethod -Uri "https://api.github.com/repos/$RepoOwner/$RepoName" -ErrorAction SilentlyContinue
                    
                    if ($null -ne $RepoInfo) {
                        Write-ColorOutput -Message 'Repository exists but has no releases. Using default branch.' -Color 'Yellow'
                        
                        $DefaultBranch = $RepoInfo.default_branch
                        $LatestRelease = $DefaultBranch

                        Write-ColorOutput -Message "Using default branch: $LatestRelease" -Color 'Green'
                        Write-ColorOutput -Message 'No release assets available. Please download the source code manually.' -Color 'Yellow'
                        exit 1
                    }
                }
                catch {
                    Write-ColorOutput -Message "Repository $RepoOwner/$RepoName not found" -Color 'Red'
                    exit 1
                }
            }
            else {
                $ReleaseResponse = $AllReleases[0]
            }
        }
        
        $LatestRelease = $ReleaseResponse.tag_name
        Write-ColorOutput -Message "Latest release: $LatestRelease" -Color 'Green'
        
        $AssetsList = $ReleaseResponse.assets | ForEach-Object { $_.browser_download_url }
        
        if ($null -eq $AssetsList -or $AssetsList.Count -eq 0) {
            Write-ColorOutput -Message "No assets found for release $LatestRelease" -Color 'Red'
            exit 1
        }
        
        Write-ColorOutput -Message "Found $($AssetsList.Count) assets for release $LatestRelease" -Color 'Green'
        
        return @{
            LatestRelease = $LatestRelease
            AssetsList = $AssetsList
        }
    }
    catch {
        Write-ColorOutput -Message 'Failed to fetch release information' -Color 'Red'
        Write-Host 'Error details: ' -NoNewline
        Write-Host $_
        exit 1
    }
}

# Find the best matching asset for the current OS and architecture
function Find-BestAsset {
    param (
        [string[]]$AssetsList,
        [string[]]$OSKeywords,
        [string[]]$ArchKeywords,
        [string]$ArchType
    )
    
    Write-ColorOutput -Message "Finding appropriate binary for Windows $ArchType..." -Color 'Cyan'
    
    if ($null -eq $AssetsList -or $AssetsList.Count -eq 0) {
        Write-ColorOutput -Message 'Error: Assets list is empty' -Color 'Red'
        Write-ColorOutput -Message 'This could be due to an issue with the GitHub API response' -Color 'Yellow'
        exit 1
    }
    
    $BestAsset = $null
    $BestScore = 0
    $AssetName = $null
    
    foreach ($AssetUrl in $AssetsList) {
        $AssetBaseName = Split-Path -Leaf $AssetUrl
        $Score = 0
        
        # Skip checksum files
        if ($AssetBaseName -match '\.sha256$|\.md5$|\.asc$') {
            continue
        }

        # Skip non-Windows binaries (darwin = macOS, linux = Linux)
        if ($AssetBaseName -match 'darwin|linux') {
            Write-ColorOutput -Message "Skipping non-Windows binary: $AssetBaseName" -Color 'Yellow'
            continue
        }
        
        # Check for OS match
        foreach ($Keyword in $OSKeywords) {
            if ($AssetBaseName -match [regex]::Escape($Keyword)) {
                $Score += 5
                break
            }
        }
        
        # Check for architecture match
        foreach ($Keyword in $ArchKeywords) {
            if ($AssetBaseName -match [regex]::Escape($Keyword)) {
                $Score += 5
                break
            }
        }
        
        # Prefer exact matches for binary name
        if ($AssetBaseName -match "^$BinaryName-") {
            $Score += 3
        }
        
        # If we have a perfect match, use it immediately
        if ($Score -eq 13) {
            $BestAsset = $AssetUrl
            $AssetName = $AssetBaseName
            Write-ColorOutput -Message "Found perfect match: $AssetBaseName" -Color 'Green'
            return @{
                AssetUrl = $BestAsset
                AssetName = $AssetName
            }
        }
        
        # Otherwise, keep track of the best match so far
        if ($Score -gt $BestScore) {
            $BestScore = $Score
            $BestAsset = $AssetUrl
            $AssetName = $AssetBaseName
        }
    }
    
    if ($null -eq $BestAsset) {
        Write-ColorOutput -Message "Could not find a suitable binary for windows $ArchType" -Color 'Red'
        Write-ColorOutput -Message 'Available assets:' -Color 'Yellow'
        $AssetsList | ForEach-Object { $AssetName = Split-Path -Leaf $_; Write-Host "- $AssetName" }
        exit 1
    }
    
    Write-ColorOutput -Message "Selected asset: $AssetName (score: $BestScore)" -Color 'Green'
    
    return @{
        AssetUrl = $BestAsset
        AssetName = $AssetName
    }
}

# Save the asset
function Save-Asset {
    param (
        [string]$AssetUrl,
        [string]$AssetName
    )
    
    $TempDir = Join-Path -Path $env:TEMP -ChildPath ([System.Guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
    
    if ([string]::IsNullOrEmpty($AssetUrl)) {
        Write-ColorOutput -Message 'Error: Asset URL is empty' -Color 'Red'
        Write-ColorOutput -Message 'This could be due to an issue with the GitHub API response or asset selection' -Color 'Yellow'
        exit 1
    }
    
    if ([string]::IsNullOrEmpty($AssetName)) {
        Write-ColorOutput -Message 'Error: Asset name is empty' -Color 'Red'
        Write-ColorOutput -Message 'This could be due to an issue with the asset selection process' -Color 'Yellow'
        exit 1
    }
    
    $ChecksumUrl = "$AssetUrl.sha256"
    $AssetPath = Join-Path -Path $TempDir -ChildPath $AssetName
    $ChecksumPath = "$AssetPath.sha256"
    
    Write-ColorOutput -Message "Downloading $AssetName..." -Color 'Cyan'
    try {
        Invoke-WebRequest -Uri $AssetUrl -OutFile $AssetPath -ErrorAction Stop
    }
    catch {
        Write-ColorOutput -Message "Failed to download $AssetName" -Color 'Red'
        Write-Host 'Error details: ' -NoNewline
        Write-Host $_
        exit 1
    }
    
    Write-ColorOutput -Message 'Downloading checksum...' -Color 'Cyan'
    try {
        Invoke-WebRequest -Uri $ChecksumUrl -OutFile $ChecksumPath -ErrorAction SilentlyContinue
        $HasChecksum = $?
    }
    catch {
        Write-ColorOutput -Message 'No checksum file found, skipping verification' -Color 'Yellow'
        $HasChecksum = $false
    }
    
    return @{
        TempDir = $TempDir
        AssetPath = $AssetPath
        ChecksumPath = $ChecksumPath
        HasChecksum = $HasChecksum
    }
}

# Verify the checksum
function Test-AssetChecksum {
    param (
        [string]$AssetPath,
        [string]$ChecksumPath,
        [bool]$HasChecksum
    )
    
    if (-not $HasChecksum) {
        return $true
    }
    
    Write-ColorOutput -Message 'Verifying checksum...' -Color 'Cyan'
    
    # Get the expected checksum from the file
    $ExpectedChecksum = Get-Content -Path $ChecksumPath -Raw
    $ExpectedChecksum = $ExpectedChecksum.Trim()
    
    # Calculate the actual checksum
    $ActualChecksum = (Get-FileHash -Path $AssetPath -Algorithm SHA256).Hash.ToLower()
    
    if ($ExpectedChecksum -notmatch $ActualChecksum) {
        Write-ColorOutput -Message 'Checksum verification failed!' -Color 'Red'
        Write-Host "Expected: $ExpectedChecksum"
        Write-Host "Actual: $ActualChecksum"
        exit 1
    }
    
    Write-ColorOutput -Message 'Checksum verified successfully' -Color 'Green'
    return $true
}

# Install the binary
function Install-Binary {
    param (
        [string]$TempDir,
        [string]$AssetPath,
        [string]$AssetName,
        [string]$InstallDir
    )
    
    Write-ColorOutput -Message "Installing $BinaryName to $InstallDir..." -Color 'Cyan'
    
    # Create the installation directory if it doesn't exist
    if (-not (Test-Path -Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    
    # Determine file type and extract accordingly
    if ($AssetName -match '\.tar\.gz$|\.tgz$') {
        # Extract using tar (Windows 10 has built-in tar support)
        tar -xzf $AssetPath -C $TempDir -ErrorAction Stop
    }
    elseif ($AssetName -match '\.zip$') {
        Expand-Archive -Path $AssetPath -DestinationPath $TempDir -Force -ErrorAction Stop
    }
    elseif ($AssetName -match '\.tar\.bz2$|\.tbz2$') {
        tar -xjf $AssetPath -C $TempDir -ErrorAction Stop
    }
    elseif ($AssetName -match '\.tar\.xz$|\.txz$') {
        tar -xf $AssetPath -C $TempDir -ErrorAction Stop
    }
    else {
        # Assume it's a direct binary
        $BinaryFile = Join-Path -Path $InstallDir -ChildPath "$BinaryName.exe"
        Copy-Item -Path $AssetPath -Destination $BinaryFile -Force
        Write-ColorOutput -Message "Successfully installed $BinaryName to $BinaryFile" -Color 'Green'
        return $BinaryFile
    }
    
    # Find the binary in the extracted files
    $BinaryFile = Get-ChildItem -Path $TempDir -Recurse -Filter "$BinaryName.exe" -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty FullName
    
    if ([string]::IsNullOrEmpty($BinaryFile)) {
        # Look for any .exe file
        $BinaryFile = Get-ChildItem -Path $TempDir -Recurse -Filter "*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty FullName
    }
    
    if ([string]::IsNullOrEmpty($BinaryFile)) {
        Write-ColorOutput -Message 'Binary not found in the archive' -Color 'Red'
        Write-ColorOutput -Message 'Archive contents:' -Color 'Yellow'
        Get-ChildItem -Path $TempDir -Recurse | ForEach-Object { Write-Host "  $_" }
        exit 1
    }
    
    $DestBinaryFile = Join-Path -Path $InstallDir -ChildPath "$BinaryName.exe"
    Copy-Item -Path $BinaryFile -Destination $DestBinaryFile -Force -ErrorAction Stop
    
    Write-ColorOutput -Message "Successfully installed $BinaryName to $DestBinaryFile" -Color 'Green'
    
    # Clean up
    Remove-Item -Path $TempDir -Recurse -Force
    
    return $DestBinaryFile
}

# Check PATH availability
function Test-PathAvailability {
    param (
        [string]$InstallDir,
        [string]$BinaryFile
    )
    
    $EnvPath = [System.Environment]::GetEnvironmentVariable("PATH", "Machine")
    
    $UserPath = [System.Environment]::GetEnvironmentVariable("PATH", "User")
    $PathScope = "Machine"
    
    # For user-level installation, check and modify user PATH
    if ($InstallMode -eq "user" -or -not $IsAdmin) {
        $EnvPath = $UserPath
        $PathScope = "User"
    }
    
    # Check if already in PATH
    if (($EnvPath -split ';' -contains $InstallDir) -or ($UserPath -split ';' -contains $InstallDir)) {
        Write-ColorOutput -Message "$BinaryName is now available in your PATH" -Color 'Green'
        Write-ColorOutput -Message "You can run it with: $BinaryName" -Color 'Cyan'
        
        # Display version if available
        try {
            $Version = & $BinaryFile --version 2>$null
            if ($? -and $LASTEXITCODE -eq 0) {
                Write-ColorOutput -Message "Installed version: $Version" -Color 'Green'
            }
            else {
                $Version = & $BinaryFile version -ErrorAction SilentlyContinue 2>$null
                if ($LASTEXITCODE -eq 0) {
                    Write-ColorOutput -Message "Installed version: $Version" -Color 'Green'
                }
            }
        }
        catch {
            # Unable to get version
        }
    }
    else {
  
      
        Write-ColorOutput -Message "Warning: $InstallDir is not in your PATH" -Color 'Yellow'
        Write-Host "You can run the binary with: $BinaryFile"
        Write-Host "To add it to your $PathScope PATH, run this command:"
        $pathCmd = @"
[Environment]::SetEnvironmentVariable('PATH', [Environment]::GetEnvironmentVariable('PATH', '$PathScope') + ';$InstallDir', '$PathScope')
"@
        Write-ColorOutput -Message $pathCmd -Color 'Cyan'
  
      
        Write-Host 'Then restart your terminal for the changes to take effect.'
    }
}

# Main installation process
function Start-Installation {
    try {
        $ArchInfo = Get-SystemArchitecture
        $ArchType = $ArchInfo.ArchType
        $ArchKeywords = $ArchInfo.ArchKeywords
        $OSKeywords = $ArchInfo.OSKeywords
        
        $ReleaseInfo = Get-LatestRelease
        $AssetsList = $ReleaseInfo.AssetsList
        
        $AssetInfo = Find-BestAsset -AssetsList $AssetsList -OSKeywords $OSKeywords -ArchKeywords $ArchKeywords -ArchType $ArchType
        $AssetUrl = $AssetInfo.AssetUrl
        $AssetName = $AssetInfo.AssetName
        
        $DownloadInfo = Save-Asset -AssetUrl $AssetUrl -AssetName $AssetName
        $TempDir = $DownloadInfo.TempDir
        $AssetPath = $DownloadInfo.AssetPath
        $ChecksumPath = $DownloadInfo.ChecksumPath
        $HasChecksum = $DownloadInfo.HasChecksum
        
        Test-AssetChecksum -AssetPath $AssetPath -ChecksumPath $ChecksumPath -HasChecksum $HasChecksum
        
        $BinaryFile = Install-Binary -TempDir $TempDir -AssetPath $AssetPath -AssetName $AssetName -InstallDir $InstallDir
        
        Test-PathAvailability -InstallDir $InstallDir -BinaryFile $BinaryFile
        
        Write-ColorOutput -Message 'Installation completed successfully!' -Color 'Green'
    }
    catch {
        Write-ColorOutput -Message 'An error occurred during installation' -Color 'Red'
        Write-Host 'Error details: ' -NoNewline
        Write-Host $_
        exit 1
    }
}

# Run the installation
Start-Installation