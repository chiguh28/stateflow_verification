[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module Microsoft.PowerShell.Utility -ErrorAction Stop

$requiredRustToolchain = 'stable-x86_64-pc-windows-msvc'

function Write-Step {
    param([Parameter(Mandatory = $true)][string]$Message)

    Write-Host "`n==> $Message" -ForegroundColor Cyan
}

function Invoke-WingetInstall {
    param(
        [Parameter(Mandatory = $true)][string]$PackageId,
        [string]$InstallerOverride
    )

    if (-not (Get-Command 'winget.exe' -ErrorAction SilentlyContinue)) {
        throw 'winget is required to install missing dependencies. Install or update App Installer from the Microsoft Store, then run setup-and-run.bat again.'
    }

    $wingetArguments = @(
        'install',
        '--id', $PackageId,
        '--exact',
        '--source', 'winget',
        '--accept-source-agreements',
        '--accept-package-agreements',
        '--silent'
    )

    if ($InstallerOverride) {
        $wingetArguments += @('--override', $InstallerOverride)
    }

    & winget @wingetArguments | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "winget could not install $PackageId (exit code $LASTEXITCODE). Check network access and approve any Windows permission prompt, then run this file again."
    }
}

function Update-CurrentPath {
    $machinePath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'

    $pathParts = @($env:Path, $machinePath, $userPath)
    if (Test-Path $cargoBin) {
        $pathParts += $cargoBin
    }

    $env:Path = ($pathParts | Where-Object { $_ } | Select-Object -Unique) -join ';'
}

function Get-VisualStudioVcVarsPath {
    $vswherePath = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswherePath)) {
        return $null
    }

    $vswhereArguments = @(
        '-latest',
        '-products', '*',
        '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
        '-property', 'installationPath'
    )

    $installationPaths = & $vswherePath @vswhereArguments 2>$null
    if ($LASTEXITCODE -ne 0 -or -not $installationPaths) {
        return $null
    }

    $installationPath = ($installationPaths | Select-Object -First 1).Trim()
    $vcVarsPath = Join-Path $installationPath 'VC\Auxiliary\Build\vcvars64.bat'
    if (Test-Path $vcVarsPath) {
        return $vcVarsPath
    }

    return $null
}

function Test-WebView2RuntimeInstalled {
    $webViewDirectories = @()

    if (${env:ProgramFiles(x86)}) {
        $webViewDirectories += Join-Path ${env:ProgramFiles(x86)} 'Microsoft\EdgeWebView\Application'
    }
    if ($env:ProgramFiles) {
        $webViewDirectories += Join-Path $env:ProgramFiles 'Microsoft\EdgeWebView\Application'
    }

    foreach ($webViewDirectory in $webViewDirectories) {
        if (-not (Test-Path $webViewDirectory)) {
            continue
        }

        $versionDirectories = Get-ChildItem -Path $webViewDirectory -Directory -ErrorAction SilentlyContinue
        foreach ($versionDirectory in $versionDirectories) {
            $webViewExecutable = Join-Path $versionDirectory.FullName 'msedgewebview2.exe'
            if (Test-Path $webViewExecutable) {
                return $true
            }
        }
    }

    return $false
}

function Test-NodeVersionSupported {
    $nodeCommand = Get-Command 'node.exe' -ErrorAction SilentlyContinue
    if (-not $nodeCommand) {
        return $false
    }

    $nodeVersionText = (& $nodeCommand.Source --version).Trim()
    if ($LASTEXITCODE -ne 0 -or $nodeVersionText -notmatch '^v(?<version>\d+\.\d+\.\d+)') {
        return $false
    }

    $nodeVersion = [version]$Matches.version
    $isSupportedNode20 = $nodeVersion.Major -eq 20 -and $nodeVersion.Minor -ge 19
    $isSupportedNode22OrNewer = $nodeVersion.Major -gt 22 -or (
        $nodeVersion.Major -eq 22 -and $nodeVersion.Minor -ge 12
    )

    return $isSupportedNode20 -or $isSupportedNode22OrNewer
}

function Install-MissingPrerequisites {
    Write-Step 'Checking Microsoft C++ Build Tools'
    $vcVarsPath = Get-VisualStudioVcVarsPath
    if (-not $vcVarsPath) {
        $visualStudioOverride = '--wait --quiet --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
        Invoke-WingetInstall -PackageId 'Microsoft.VisualStudio.2022.BuildTools' -InstallerOverride $visualStudioOverride
        Update-CurrentPath
        $vcVarsPath = Get-VisualStudioVcVarsPath
    }
    if (-not $vcVarsPath) {
        throw 'The MSVC x64 C++ tools were not found after installation. Reopen Windows or Visual Studio Installer, complete the C++ Build Tools installation, and run this file again.'
    }

    Write-Step 'Checking Rustup and the Rust MSVC toolchain'
    if (-not (Get-Command 'rustup.exe' -ErrorAction SilentlyContinue)) {
        Invoke-WingetInstall -PackageId 'Rustlang.Rustup'
        Update-CurrentPath
    }

    $rustupCommand = Get-Command 'rustup.exe' -ErrorAction SilentlyContinue
    if (-not $rustupCommand) {
        throw 'Rustup was not found after installation. Reopen Windows, then run this file again.'
    }

    $installedToolchains = & $rustupCommand.Source 'toolchain' 'list'
    if ($LASTEXITCODE -ne 0) {
        throw "Could not list Rust toolchains (exit code $LASTEXITCODE)."
    }

    $hasRequiredToolchain = $installedToolchains | Where-Object { $_ -match '^stable-x86_64-pc-windows-msvc(?:\s|$)' }
    if (-not $hasRequiredToolchain) {
        & $rustupCommand.Source 'toolchain' 'install' $requiredRustToolchain '--profile' 'minimal' | Out-Host
        if ($LASTEXITCODE -ne 0) {
            throw "Rust toolchain installation failed (exit code $LASTEXITCODE). Check network access and run this file again."
        }
    }

    $env:RUSTUP_TOOLCHAIN = $requiredRustToolchain

    Write-Step 'Checking Node.js'
    if (-not (Test-NodeVersionSupported)) {
        Invoke-WingetInstall -PackageId 'OpenJS.NodeJS.LTS'
        Update-CurrentPath
    }
    if (-not (Test-NodeVersionSupported)) {
        throw 'Node.js is missing or too old. Install Node.js 20.19+ or 22.12+ from https://nodejs.org/, reopen Windows, and run this file again.'
    }

    Write-Step 'Checking WebView2 Runtime'
    if (-not (Test-WebView2RuntimeInstalled)) {
        Invoke-WingetInstall -PackageId 'Microsoft.EdgeWebView2Runtime'
    }
    if (-not (Test-WebView2RuntimeInstalled)) {
        throw 'WebView2 Runtime was not found after installation. Install the Evergreen Runtime from https://developer.microsoft.com/microsoft-edge/webview2/, then run this file again.'
    }

    Update-CurrentPath
    return $vcVarsPath
}

function Install-NpmDependenciesIfNeeded {
    param([Parameter(Mandatory = $true)][string]$RepositoryRoot)

    $lockFilePath = Join-Path $RepositoryRoot 'package-lock.json'
    $dependencyDirectory = Join-Path $RepositoryRoot 'node_modules'
    $dependencyMarkerPath = Join-Path $dependencyDirectory '.stateflow-package-lock.sha256'
    $lockFileHash = (Get-FileHash -LiteralPath $lockFilePath -Algorithm SHA256).Hash
    $installedLockFileHash = $null

    if (Test-Path $dependencyMarkerPath) {
        $installedLockFileHash = (Get-Content -LiteralPath $dependencyMarkerPath -Raw).Trim()
    }

    if ($installedLockFileHash -eq $lockFileHash -and (Test-Path (Join-Path $dependencyDirectory '.bin\tauri.cmd'))) {
        Write-Host 'npm dependencies already match package-lock.json.' -ForegroundColor Green
        return
    }

    Write-Step 'Installing npm dependencies with npm ci'
    $npmCommand = Get-Command 'npm.cmd' -ErrorAction SilentlyContinue
    if (-not $npmCommand) {
        throw 'npm.cmd was not found. Install Node.js LTS, reopen Windows, and run this file again.'
    }

    Push-Location $RepositoryRoot
    try {
        & $npmCommand.Source 'ci'
        if ($LASTEXITCODE -ne 0) {
            throw "npm ci failed (exit code $LASTEXITCODE). Check network access and run this file again."
        }
    }
    finally {
        Pop-Location
    }

    Set-Content -LiteralPath $dependencyMarkerPath -Value $lockFileHash -Encoding ASCII
}

function Import-MsvcEnvironmentAndStartApp {
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$VcVarsPath
    )

    $launchCommand = 'call "{0}" >nul && npm.cmd run tauri -- dev' -f $VcVarsPath
    Write-Step 'Starting STATEFLOW in Tauri development mode'
    Write-Host 'The app window and Vite development process will stay open until the app is closed.'

    Push-Location $RepositoryRoot
    try {
        & $env:ComSpec /d /c $launchCommand
        if ($LASTEXITCODE -ne 0) {
            throw "STATEFLOW did not exit normally (exit code $LASTEXITCODE)."
        }
    }
    finally {
        Pop-Location
    }
}

try {
    $repositoryRoot = Split-Path -Parent $PSScriptRoot
    $vcVarsPath = Install-MissingPrerequisites
    Install-NpmDependenciesIfNeeded -RepositoryRoot $repositoryRoot
    Import-MsvcEnvironmentAndStartApp -RepositoryRoot $repositoryRoot -VcVarsPath $vcVarsPath
}
catch {
    Write-Host "`nSTATEFLOW setup failed: $($_.Exception.Message)" -ForegroundColor Red
    if ($_.InvocationInfo.PositionMessage) {
        Write-Host $_.InvocationInfo.PositionMessage -ForegroundColor DarkGray
    }
    exit 1
}
