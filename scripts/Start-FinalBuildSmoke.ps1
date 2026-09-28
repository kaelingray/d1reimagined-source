# Check BLES01857 01.24 data and, with -Run, launch a first sign-in/orbit smoke run.
# The check-only path never starts RPCS3. A -Run call requires a prepared RPCS3
# copy with the 01.24 update and both local patches installed.
param(
    [Parameter(Mandatory)][string]$DataRepo,
    [string]$Rpcs3Dir,
    [string]$GameUsrdir,
    [string]$PpuHash,
    [string]$Name = ("FINAL124_" + (Get-Date -Format 'yyyyMMdd_HHmmss')),
    [switch]$Run
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Paths.ps1"

$data = (Resolve-Path -LiteralPath $DataRepo).Path
$types = Join-Path $data 'ps3_updates\types_01.24.json'
$services = Join-Path $data 'ps3_updates\services_01.24.json'
$check = @("$PSScriptRoot\d1_final_preflight.py", '--data-root', $data)
if ($Rpcs3Dir -or $GameUsrdir -or $PpuHash -or $Run) {
    if (-not ($Rpcs3Dir -and $GameUsrdir)) {
        throw 'For a launch check, provide -Rpcs3Dir and -GameUsrdir.'
    }
    $rpcs3 = (Resolve-Path -LiteralPath $Rpcs3Dir).Path
    $usrdir = (Resolve-Path -LiteralPath $GameUsrdir).Path
    $check += @('--rpcs3-dir', $rpcs3, '--game-usrdir', $usrdir, '--local-root', $D1Local)
    if ($PpuHash) { $check += @('--ppu-hash', $PpuHash) }
}

& $D1Python @check
if ($LASTEXITCODE) { throw '01.24 preflight failed; no game was started.' }
if (-not $Run) { return }

$previousRpcs3 = $env:D1_RPCS3
$previousUsrdir = $env:D1_USRDIR
try {
    $env:D1_RPCS3 = $rpcs3
    $env:D1_USRDIR = $usrdir
    & "$PSScriptRoot\Start-LocalStack.ps1" -Name $Name -BapArgs @(
        '--universe-time', 's', '--self-flags', '1;2@B0,C0-1023',
        '--schema', $types, '--services-schema', $services,
        '--activity-schema', $types)
} finally {
    $env:D1_RPCS3 = $previousRpcs3
    $env:D1_USRDIR = $previousUsrdir
}

"01.24 smoke run started as $Name. Watch the screen and $D1Logs\$Name-*.log."
