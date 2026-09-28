# Check BLES01857 01.24 data and, with -Run, launch a first sign-in/orbit smoke run.
# The check-only path never starts RPCS3. A -Run call requires a prepared RPCS3
# copy with the 01.24 update and both local patches installed.
param(
    [Parameter(Mandatory)][string]$DataRepo,
    [string]$Rpcs3Dir,
    [string]$GameUsrdir,
    [string]$PpuHash,
    [string]$ManifestFile,
    [string]$Name = ("FINAL124_" + (Get-Date -Format 'yyyyMMdd_HHmmss')),
    [string[]]$SignonArgs = @('--title-config-format', '01.24', '--public-ip', '127.0.0.1'),
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

$manifestArgument = @()
if ($ManifestFile) {
    if ($SignonArgs -contains '--manifest-file') { throw 'Specify a manifest file only once.' }
    $manifestArgument = @('--manifest-file', (Resolve-Path -LiteralPath $ManifestFile).Path)
} elseif ($SignonArgs -notcontains '--manifest-file') {
    $updatePackages = Join-Path $rpcs3 'dev_hdd0\game\BLES01857\USRDIR\a0000001\packages'
    $manifest = Join-Path $D1Local 'RESEARCH\ps3_updates\manifest-final-smoke.bin'
    & $D1Python "$PSScriptRoot\build_manifest_probe.py" `
        --disc-packages (Join-Path $usrdir 'packages') `
        --update-packages-dir $updatePackages --out $manifest
    if ($LASTEXITCODE) { throw 'Final-build diagnostic manifest generation failed.' }
    $manifestArgument = @('--manifest-file', $manifest)
}

$previousRpcs3 = $env:D1_RPCS3
$previousUsrdir = $env:D1_USRDIR
try {
    $env:D1_RPCS3 = $rpcs3
    $env:D1_USRDIR = $usrdir
    & "$PSScriptRoot\Start-LocalStack.ps1" -Name $Name -SignonArgs ($SignonArgs + $manifestArgument) -BapArgs @(
        '--universe-time', 's', '--self-flags', '1;2@B0,C0-1023',
        '--schema', $types, '--services-schema', $services,
        '--activity-schema', $types)
} finally {
    $env:D1_RPCS3 = $previousRpcs3
    $env:D1_USRDIR = $previousUsrdir
}

"01.24 smoke run started as $Name. Watch the screen and $D1Logs\$Name-*.log."
