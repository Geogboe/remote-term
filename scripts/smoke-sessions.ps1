[CmdletBinding()]
param(
    [string]$BindAddress = '127.0.0.2',
    [int]$Port = 0
)

$ErrorActionPreference = 'Stop'

$binaryName = if ($IsWindows) { 'rterm.exe' } else { 'rterm' }
$binary = Join-Path $PSScriptRoot "../target/debug/$binaryName"
$stdout = Join-Path ([System.IO.Path]::GetTempPath()) "rterm-session-smoke.stdout.log"
$stderr = Join-Path ([System.IO.Path]::GetTempPath()) "rterm-session-smoke.stderr.log"
$process = $null

try {
    Remove-Item -LiteralPath $stdout, $stderr -ErrorAction SilentlyContinue
    $arguments = @(
        '--headless',
        '--write',
        '--bind',
        "${BindAddress}:$Port",
        '--',
        'pwsh',
        '-NoProfile',
        '-Command',
        'Start-Sleep -Seconds 3'
    )
    $start = @{
        FilePath = $binary
        ArgumentList = $arguments
        RedirectStandardOutput = $stdout
        RedirectStandardError = $stderr
        PassThru = $true
    }
    if ($IsWindows) {
        $start.WindowStyle = 'Hidden'
    }
    $process = Start-Process @start
    Start-Sleep -Milliseconds 700

    if ($process.HasExited) {
        throw "rterm exited during startup: $(Get-Content -Raw -LiteralPath $stderr)"
    }

    $sessions = & $binary sessions --json | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) {
        throw 'rterm sessions --json failed'
    }
    $matching = @($sessions) | Where-Object { $_.pid -eq $process.Id }
    if ($matching.Count -ne 1) {
        throw "expected one active session for pid $($process.Id), found $($matching.Count)"
    }

    $session = $matching[0]
    $escapedAddress = [regex]::Escape($BindAddress)
    if ($session.local_url -notmatch "^http://${escapedAddress}:[1-9]\d*/t/(?:[a-z]+-){4}[a-z]+$") {
        throw "unexpected generated session URL: $($session.local_url)"
    }

    Wait-Process -Id $process.Id -Timeout 10
    Start-Sleep -Milliseconds 200
    $after = & $binary sessions --json | ConvertFrom-Json
    if (@($after) | Where-Object { $_.pid -eq $process.Id }) {
        throw 'session registry entry remained after child exit'
    }

    Write-Output 'session discovery smoke passed'
}
finally {
    if ($null -ne $process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath $stdout, $stderr -ErrorAction SilentlyContinue
}
