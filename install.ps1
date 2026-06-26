#Requires -Version 5.1
$ErrorActionPreference = 'Stop'

$Repo = 'Geogboe/rterm'
$Binary = 'rterm'
$Prefix = $Binary.ToUpperInvariant().Replace('-', '_')

function Get-EnvValue {
    param([Parameter(Mandatory)][string]$Name)
    $item = Get-Item -Path "Env:$Name" -ErrorAction SilentlyContinue
    if ($item) { return $item.Value }
    return $null
}

function Write-DebugLog {
    param([string]$Message)
    if ($script:DebugMode) {
        Write-Host "debug: $Message"
    }
}

$script:DebugMode = ((Get-EnvValue "${Prefix}_DEBUG") -eq '1') -or ($env:INSTALLER_DEBUG -eq '1')

if (-not [System.Environment]::Is64BitOperatingSystem) {
    throw '32-bit Windows is not supported'
}

if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq [System.Runtime.InteropServices.Architecture]::X64) {
    $Arch = 'amd64'
} else {
    throw "unsupported Windows architecture: $([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture)"
}

$Tag = Get-EnvValue "${Prefix}_VERSION"
if (-not $Tag) { $Tag = $env:INSTALLER_VERSION }
if (-not $Tag) {
    $releases = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases?per_page=10"
    $release = @($releases | Where-Object { -not $_.draft } | Select-Object -First 1)
    if (-not $release) { throw "could not find a GitHub release; set ${Prefix}_VERSION or INSTALLER_VERSION" }
    $Tag = $release.tag_name
}

$Archive = "${Binary}_${Tag}_windows_${Arch}.zip"
$BaseUrl = "https://github.com/$Repo/releases/download/$Tag"
$Temp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())

Write-DebugLog "release tag: $Tag"
Write-DebugLog "archive: $Archive"

New-Item -ItemType Directory -Path $Temp | Out-Null
try {
    $ArchivePath = Join-Path $Temp $Archive
    $ChecksumsPath = Join-Path $Temp 'checksums.txt'
    Invoke-WebRequest -Uri "$BaseUrl/$Archive" -OutFile $ArchivePath -UseBasicParsing
    Invoke-WebRequest -Uri "$BaseUrl/checksums.txt" -OutFile $ChecksumsPath -UseBasicParsing

    $line = Get-Content $ChecksumsPath | Where-Object { $_ -match "\s$([regex]::Escape($Archive))$" } | Select-Object -First 1
    if (-not $line) { throw "checksum not found for $Archive" }
    $expected = ($line -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Path $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) { throw "checksum mismatch for $Archive" }

    Expand-Archive -Path $ArchivePath -DestinationPath $Temp -Force

    $InstallDir = Get-EnvValue "${Prefix}_INSTALL_DIR"
    if (-not $InstallDir) { $InstallDir = $env:INSTALLER_INSTALL_DIR }
    if (-not $InstallDir) { $InstallDir = Join-Path $env:USERPROFILE '.local\bin' }

    $Force = ((Get-EnvValue "${Prefix}_FORCE") -eq '1') -or ($env:INSTALLER_FORCE -eq '1')
    $Destination = Join-Path $InstallDir "$Binary.exe"
    if ((Test-Path $Destination) -and -not $Force) {
        Write-Host "$Binary is already installed at $Destination"
        Write-Host "Set ${Prefix}_FORCE=1 or INSTALLER_FORCE=1 to reinstall."
    } else {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        Copy-Item -Path (Join-Path $Temp "$Binary.exe") -Destination $Destination -Force
    }

    if (-not (Test-Path $Destination)) { throw "installed binary not found: $Destination" }

    $pathParts = $env:Path -split ';' | Where-Object { $_ }
    if ($pathParts -notcontains $InstallDir) {
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $userParts = @($userPath -split ';' | Where-Object { $_ })
        if ($userParts -notcontains $InstallDir) {
            $newUserPath = (@($userParts) + $InstallDir) -join ';'
            [Environment]::SetEnvironmentVariable('Path', $newUserPath, 'User')
        }
        $env:Path = (@($pathParts) + $InstallDir) -join ';'
        Write-Host "Added $InstallDir to your user PATH. Restart open terminals to pick it up everywhere."
    }

    & $Destination --version
    Write-Host "installed $Binary to $Destination"
} finally {
    Remove-Item -LiteralPath $Temp -Recurse -Force -ErrorAction SilentlyContinue
}
