$ErrorActionPreference = 'Stop'

$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$exe = Join-Path $root 'target\debug\rterm.exe'
$token = 'smoke-token'
$port = 17843
$outLog = Join-Path $env:TEMP 'rterm-smoke-web.out.log'
$errLog = Join-Path $env:TEMP 'rterm-smoke-web.err.log'

Remove-Item -LiteralPath $outLog, $errLog -ErrorAction SilentlyContinue

$arguments = @(
  '--bind', "127.0.0.1:$port",
  '--token', $token,
  '--write',
  '--',
  'pwsh', '-NoProfile', '-Command', 'Start-Sleep -Seconds 60'
)

$proc = Start-Process `
  -FilePath $exe `
  -ArgumentList $arguments `
  -RedirectStandardOutput $outLog `
  -RedirectStandardError $errLog `
  -WindowStyle Hidden `
  -PassThru

try {
  $base = "http://127.0.0.1:$port"
  $ready = $false
  foreach ($attempt in 1..40) {
    try {
      $response = Invoke-WebRequest -UseBasicParsing -Uri "$base/t/$token" -TimeoutSec 1
      if ($response.StatusCode -eq 200) {
        $ready = $true
        break
      }
    } catch {
      Start-Sleep -Milliseconds 250
    }
  }

  if (-not $ready) {
    throw 'rterm web smoke server did not become ready'
  }

  $valid = Invoke-WebRequest -UseBasicParsing -Uri "$base/t/$token" -TimeoutSec 3
  if ($valid.StatusCode -ne 200) {
    throw "expected valid token status 200, got $($valid.StatusCode)"
  }

  $asset = Invoke-WebRequest -UseBasicParsing -Uri "$base/assets/main.js" -TimeoutSec 3
  if ($asset.StatusCode -ne 200) {
    throw "expected asset status 200, got $($asset.StatusCode)"
  }

  try {
    Invoke-WebRequest -UseBasicParsing -Uri "$base/t/wrong-token" -TimeoutSec 3 | Out-Null
    throw 'expected wrong token request to fail'
  } catch {
    $status = $_.Exception.Response.StatusCode.value__
    if ($status -ne 404) {
      throw "expected wrong token status 404, got $status"
    }
  }

  Write-Output 'web smoke passed'
} finally {
  if ($proc -and -not $proc.HasExited) {
    Stop-Process -Id $proc.Id -Force
  }
}
