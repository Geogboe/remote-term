[CmdletBinding()]
param(
    [int]$Runs = 10
)

$ErrorActionPreference = 'Stop'

$binaryName = if ($IsWindows) { 'rterm.exe' } else { 'rterm' }
$binary = (Resolve-Path (Join-Path $PSScriptRoot "../target/debug/$binaryName")).Path
$marker = 'final-output-marker'

for ($run = 1; $run -le $Runs; $run++) {
    $output = & $binary --bind '127.0.0.1:0' -- pwsh -NoProfile -Command "Write-Output '$marker'" | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "short-lived child failed on run $run"
    }
    if ($output -notmatch [regex]::Escape($marker)) {
        throw "final child output was lost on run ${run}: $output"
    }
}

Write-Output "final output smoke passed ($Runs runs)"
