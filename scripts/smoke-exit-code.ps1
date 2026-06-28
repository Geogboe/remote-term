$ErrorActionPreference = 'Continue'

$binaryName = if ($IsWindows) { 'rterm.exe' } else { 'rterm' }
$binary = Join-Path $PSScriptRoot "../target/debug/$binaryName"

& $binary -- pwsh -NoProfile -Command 'Write-Output rterm-smoke; exit 7'
$childExit = $LASTEXITCODE

if ($childExit -ne 7) {
    throw "expected rterm to propagate exit code 7, received $childExit"
}

Write-Output 'exit-code propagation smoke passed'
