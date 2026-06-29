[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$binaryName = if ($IsWindows) { 'rterm.exe' } else { 'rterm' }
$binary = (Resolve-Path (Join-Path $PSScriptRoot "../target/debug/$binaryName")).Path

$outside = & $binary starship
if ($LASTEXITCODE -ne 0) {
    throw 'rterm starship failed outside a session'
}
if ($outside) {
    throw "rterm starship unexpectedly printed outside a session: $outside"
}

$escapedBinary = $binary.Replace("'", "''")
$command = "& '$escapedBinary' starship; Start-Sleep -Milliseconds 300"
$inside = & $binary --allow-elevated --bind '127.0.0.1:0' -- pwsh -NoProfile -Command $command | Out-String
if ($LASTEXITCODE -ne 0) {
    throw 'wrapped rterm starship command failed'
}
if ($inside -notmatch 'rterm:\d+ local/ro/shared') {
    throw "wrapped Starship metadata was not found in output: $inside"
}

Write-Output 'Starship integration smoke passed'
