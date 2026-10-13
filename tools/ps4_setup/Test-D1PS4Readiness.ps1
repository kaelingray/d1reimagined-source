[CmdletBinding()]
param(
    [string]$Repo = 'F:\d1\d1preservation-main',
    [string]$EmulatorDirectory = 'F:\d1\tools\shadPS4\0.19.0',
    [string]$HostsList = 'F:\d1\tools\shadPS4\d1-hosts.txt',
    [string]$Build = '',
    [string]$Report = 'F:\d1\d1preservation-main\cfw-staging\2026-10-11\readiness.json'
)
# Read-only preflight, apart from its report. This does not launch a game or alter containment.
$ErrorActionPreference = 'Stop'
$checks = [System.Collections.Generic.List[object]]::new()
function Check([string]$name, [bool]$pass, [string]$detail) {
    $checks.Add([ordered]@{name=$name;pass=$pass;detail=$detail})
}
function CheckHash([string]$name, [string]$path, [string]$hash, [long]$size = -1) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { Check $name $false "Missing: $path"; return }
    $file = Get-Item -LiteralPath $path
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    Check $name ($actual -eq $hash -and ($size -lt 0 -or $file.Length -eq $size)) "bytes=$($file.Length); sha256=$actual"
}
# The executable build comes from the registry (capture-rig/builds.json): -Build, $env:D1_BUILD, default.
$registry = Get-Content -LiteralPath (Join-Path $Repo 'capture-rig\builds.json') -Raw | ConvertFrom-Json
if (-not $Build) { $Build = if ($env:D1_BUILD) { $env:D1_BUILD } else { $registry.default } }
$entry = $registry.builds.$Build
if ($null -eq $entry) { throw "Unknown build: $Build" }
Check "Selected build $Build" $true "$($entry.build_label); addresses_valid=$($entry.addresses_valid); reference=$($registry.reference)"
CheckHash "Static-analysis ELF for $Build" (Join-Path $Repo $entry.eboot) $entry.eboot_sha256 $entry.eboot_bytes
CheckHash "Launch executable for $Build" (Join-Path $Repo $entry.launch) $entry.launch_sha256 $entry.launch_bytes
$packages = @(Get-ChildItem -LiteralPath (Join-Path $Repo $entry.packages) -File -Filter '*.pkg' -ErrorAction SilentlyContinue)
$sums = Join-Path (Split-Path (Join-Path $Repo $entry.game_dir)) 'SHA256SUMS'
$expectedPackages = if ($Build -eq $registry.reference) { 1647 }
    elseif (Test-Path -LiteralPath $sums) { @(Get-Content -LiteralPath $sums | Where-Object { $_ -match '  packages/[^/]+\.pkg$' }).Count }
    else { -1 }
Check 'Installed Bungie content packages' ($packages.Count -gt 0 -and $packages.Count -eq $expectedPackages) "files=$($packages.Count); expected $expectedPackages (reference: pinned estate; others: completed extraction manifest)"
if ($entry.manifest_sha256) { CheckHash "Full English content manifest for $Build" (Join-Path $Repo $entry.manifest) $entry.manifest_sha256 $entry.manifest_bytes }
else { Check "Full English content manifest for $Build" $false 'No manifest pin recorded for this build yet' }
if ($entry.diag_patch) {
    $patch = Join-Path $Repo $entry.diag_patch
    try { $xml = [xml](Get-Content -LiteralPath $patch -Raw); Check 'Lab patch XML' $true $patch }
    catch { Check 'Lab patch XML' $false $_.Exception.Message }
} else {
    $autoPatches = @(Get-ChildItem -LiteralPath (Join-Path $EmulatorDirectory 'user\patches') -Recurse -File -ErrorAction SilentlyContinue)
    Check 'No client patch for this build (addresses not re-derived)' ($autoPatches.Count -eq 0) "emulator patch folder files=$($autoPatches.Count); contained launches also pass -i"
}
$exe = Join-Path $EmulatorDirectory 'shadPS4.exe'
Check 'Portable emulator' (Test-Path -LiteralPath $exe -PathType Leaf) $exe
try {
    $general = (Get-Content -LiteralPath (Join-Path $EmulatorDirectory 'user\config.json') -Raw | ConvertFrom-Json).General
    $local = $general.shad_net_enabled -eq $true -and $general.connected_to_network -eq $true -and
        $general.enable_upnp -eq $false -and $general.shadnet_server -eq '127.0.0.1:31313' -and
        $general.shadnet_webapi_server -eq 'http://127.0.0.1:31315'
    Check 'Local shadNet configuration' $local 'shadNet and WebAPI loopback; UPnP disabled'
} catch { Check 'Local shadNet configuration' $false $_.Exception.Message }
try {
    $profiles = @(Get-NetFirewallProfile -PolicyStore ActiveStore)
    Check 'Windows firewall profiles active' ($profiles.Count -eq 3 -and @($profiles | Where-Object { -not $_.Enabled }).Count -eq 0) 'All three profiles must be enabled'
    $rules = @(Get-NetFirewallRule -PolicyStore ActiveStore -DisplayName 'D1 shadPS4 - block outbound' -ErrorAction Stop)
    $valid = @($rules | Where-Object { $_.Enabled -eq 'True' -and $_.Direction -eq 'Outbound' -and $_.Action -eq 'Block' -and $_.Profile -eq 'Any' } |
        Get-NetFirewallApplicationFilter | Where-Object { $_.Program -eq $exe })
    Check 'Effective emulator outbound block' ($valid.Count -gt 0) 'ActiveStore; exact executable; all profiles'
} catch { Check 'Effective emulator outbound block' $false $_.Exception.Message }
try {
    $hostsText = Get-Content -LiteralPath "$env:SystemRoot\System32\drivers\etc\hosts"
    $expectedHosts = @(Get-Content -LiteralPath $HostsList | Where-Object { $_ -match '^127\.0\.0\.1\s+\S+' } | ForEach-Object { ($_ -split '\s+')[1] } | Sort-Object -Unique)
    $badHosts = @()
    foreach ($hostName in $expectedHosts) {
        $mapping = @($hostsText | Where-Object { ($_ -split '#',2)[0] -match ('^\s*127\.0\.0\.1\s+' + [regex]::Escape($hostName) + '\s*$') })
        # Resolve only mapped names: resolving an unmapped one would send its query upstream.
        if ($mapping.Count -eq 0) { $badHosts += $hostName; continue }
        $addresses = @(try { [System.Net.Dns]::GetHostAddresses($hostName) } catch { })
        if ($mapping.Count -eq 0 -or $addresses.Count -eq 0 -or @($addresses | Where-Object { -not [System.Net.IPAddress]::IsLoopback($_) }).Count -gt 0) { $badHosts += $hostName }
    }
    Check 'Windows service redirects' ($expectedHosts.Count -ge 50 -and $badHosts.Count -eq 0) "unique hosts=$($expectedHosts.Count); failed=$($badHosts -join ','); 01.29 list: 50 unique names in 51 rows; build 56578 adds 4"
} catch { Check 'Windows service redirects' $false $_.Exception.Message }
$moduleDirectory = Join-Path $EmulatorDirectory 'user\sys_modules'
$moduleFailures = @()
$moduleCount = 0
try {
    foreach ($line in Get-Content -LiteralPath (Join-Path $moduleDirectory 'MANIFEST-11.00.txt')) {
        if ($line -match '^([a-fA-F0-9]{64})\s+(.+)$') {
            $moduleCount++
            $want = $Matches[1]; $module = $Matches[2]
            $path = Join-Path $moduleDirectory $module
            if (-not (Test-Path -LiteralPath $path -PathType Leaf) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $want) { $moduleFailures += $module }
        }
    }
    Check 'Firmware 11.00 module receipt' ($moduleCount -eq 37 -and $moduleFailures.Count -eq 0) "modules=$moduleCount; failed=$($moduleFailures -join ','); upstream reference used firmware 10.01"
} catch { Check 'Firmware 11.00 module receipt' $false $_.Exception.Message }
Check 'No existing emulator process' (@(Get-Process shadPS4,rpcs3 -ErrorAction SilentlyContinue).Count -eq 0) 'One emulator at a time'
CheckHash 'Canonical preparation launcher installed' (Join-Path $Repo 'capture-rig\run-boot.sh') '989e79b0c6c0d3fdab817c0374a9807c95da846dca63e25ff8d0a3f08973d6af'
CheckHash 'Canonical server-check delegate installed' (Join-Path $Repo 'capture-rig\run-signon-experiment.sh') '6a3b2c89b098d9bd00f7dae2e0a0e7f5557a9bd483b7fc92af0e733c2c190d2d'
# Static file/config checks cannot establish a successful Windows/WSL runtime port.
Check 'Native Windows/WSL runtime validated' $false 'Canonical preparation and contained Linux servers validated separately. Windows guest transport, probe hooks and dump-dependent fixtures remain unaccepted.'
$result = [ordered]@{atUtc=[DateTime]::UtcNow.ToString('o');ready=(@($checks | Where-Object {-not $_.pass}).Count -eq 0);checks=$checks}
New-Item -ItemType Directory -Path (Split-Path -Parent $Report) -Force | Out-Null
$result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Report -Encoding utf8
$result | ConvertTo-Json -Depth 6
if (-not $result.ready) { exit 2 }
