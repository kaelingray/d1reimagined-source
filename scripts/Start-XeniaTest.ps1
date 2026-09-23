param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-Za-z0-9_-]+$')]
    [string]$TestId,

    [Parameter(Mandatory = $true)]
    [string]$GamePath
)

$ErrorActionPreference = 'Stop'
$base = 'D:\Destiny360'
$xeniaDirectory = Join-Path $base 'XENIA'
$exe = Join-Path $xeniaDirectory 'xenia_canary.exe'
$config = Join-Path $xeniaDirectory 'xenia-canary.config.toml'
$logDirectory = Join-Path $base 'LOGS'
$game = (Resolve-Path -LiteralPath $GamePath).Path
$originalDirectory = Join-Path $base '00_ORIGINAL_DUMP'
$log = Join-Path $logDirectory "$TestId.xenia.log"
$configCopy = Join-Path $logDirectory "$TestId.config.toml"

if (-not (Test-Path -LiteralPath $game -PathType Leaf)) { throw "GamePath must be a file: $game" }
if ($game.StartsWith($originalDirectory + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Run Xenia against a working copy, not the original archive.'
}
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Xenia executable missing: $exe" }
if (-not (Test-Path -LiteralPath $config -PathType Leaf)) { throw "Xenia config missing: $config" }
if (Test-Path -LiteralPath $log) { throw "Test log already exists: $log" }
if (Test-Path -LiteralPath $configCopy) { throw "Test config snapshot already exists: $configCopy" }

New-Item -ItemType Directory -Force -Path $logDirectory | Out-Null
Copy-Item -LiteralPath $config -Destination $configCopy
Write-Output "Starting Xenia. Close Xenia after recording the result."
Write-Output "Game: $game"
Write-Output "Log: $log"
Write-Output "Config snapshot: $configCopy"

Push-Location $xeniaDirectory
try {
    & $exe $game "--log_file=$log"
} finally {
    Pop-Location
}
