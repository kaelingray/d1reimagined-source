# Export and compare the final PS3 builds of Destiny against 01.00, in one run on the PC:
#  1. download the EBOOT (and PARAM.SFO) of each PS3 title update from Sony's update server,
#     reading only those files out of the update PKGs (scripts\ps3_update.py);
#  2. decrypt each EBOOT with RPCS3 (rpcs3.exe --decrypt EBOOT.BIN -> EBOOT.elf);
#  3. export every runtime type of each build (scripts\d1_types.py schema);
#  4. compare 01.00 -> each version and 01.13 -> 01.24 (scripts\d1_builddiff.py): types (matched by
#     structure), the type handles the stand-in uses, web services, executable strings.
# With -Packages it also downloads every game package of every update (about 12 GB) into
# <Out>\final\packages, links the disc's packages beside them (patch files read their unchanged
# blocks from the _0 disc files), and lists the final build's named tags, items and services.
#
#   scripts\Invoke-FinalBuildExport.ps1                     # EBOOTs of 01.13 and 01.24 (~60 MB)
#   scripts\Invoke-FinalBuildExport.ps1 -Versions all       # every update's EBOOT (~310 MB)
#   scripts\Invoke-FinalBuildExport.ps1 -Packages           # plus the full final package set
#
# Results go to local\RESEARCH\ps3_updates (outside Git). The small outputs to share are listed at
# the end (reports, type schemas, updates.json); the ELFs and packages stay private.
param(
    [string]$Title = 'BLES01857',
    [string[]]$Versions = @('01.13', '01.24'),
    [switch]$Packages,
    [string]$Out = '',
    [string]$BaseElf = '',            # the decrypted 01.00 EBOOT
    [string]$DiscPackages = '',       # the disc's packages folder (for -Packages and the 01.00 services)
    [string]$Oodle = '',              # oo2core_3_win64.dll (package blocks are compressed)
    [string]$Rpcs3Exe = ''
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Paths.ps1"
if (-not $Out) { $Out = Join-Path $D1Local 'RESEARCH\ps3_updates' }
if (-not $BaseElf) { $BaseElf = Join-Path $D1Local 'RESEARCH\ps3_decrypt\EBOOT.elf' }
if (-not $DiscPackages) { $DiscPackages = Join-Path $D1Local '01_WORKING\ps3_destiny\PS3_GAME\USRDIR\packages' }
if (-not $Oodle) { $Oodle = Join-Path $D1Local 'oo2core_3_win64.dll' }
if (-not $Rpcs3Exe) { $Rpcs3Exe = Join-Path $D1Rpcs3 'rpcs3.exe' }
New-Item -ItemType Directory -Force $Out | Out-Null
$py = $D1Python

function Step($text) { Write-Host "`n=== $text" -ForegroundColor Cyan }

Step "update list for $Title"
& $py "$PSScriptRoot\ps3_update.py" list $Title | Tee-Object (Join-Path $Out 'update-list.txt')
$all = (Get-Content (Join-Path $Out 'update-list.txt')) | ForEach-Object { ($_ -split '\s+')[0] } | Where-Object { $_ -match '^\d\d\.\d\d$' }
if ($Versions -contains 'all') { $Versions = $all }

Step "downloading EBOOT.BIN / PARAM.SFO of $($Versions -join ', ')$(if ($Packages) { ' and every game package' })"
$fetch = @("$PSScriptRoot\ps3_update.py", 'fetch', $Title, '--versions', ($Versions -join ','),
           '--match', 'EBOOT.BIN,PARAM.SFO', '--out', $Out)
if ($Packages) { $fetch += '--packages' }
& $py @fetch
if ($LASTEXITCODE) { throw 'ps3_update.py fetch failed' }

Step 'decrypting the EBOOTs with RPCS3'
if (-not (Test-Path $Rpcs3Exe)) { throw "rpcs3.exe not found at $Rpcs3Exe (pass -Rpcs3Exe)" }
$elves = @{}
foreach ($v in $Versions) {
    $bin = Join-Path $Out "$v\USRDIR\EBOOT.BIN"
    $elf = Join-Path $Out "$v\USRDIR\EBOOT.elf"
    if (-not (Test-Path $bin)) { Write-Warning "$v has no EBOOT.BIN"; continue }
    if (-not (Test-Path $elf)) {
        Start-Process -FilePath $Rpcs3Exe -ArgumentList @('--decrypt', "`"$bin`"") -Wait -NoNewWindow
    }
    if (Test-Path $elf) { $elves[$v] = $elf; "  $v -> $elf" } else { Write-Warning "RPCS3 did not write $elf" }
}

Step 'exporting runtime types'
$typeArgs = @()
if ((Test-Path $DiscPackages) -and (Test-Path $Oodle)) { $typeArgs = @('--packages', $DiscPackages, '--oodle', $Oodle) }
if (Test-Path $BaseElf) {
    & $py "$PSScriptRoot\d1_types.py" --elf $BaseElf @typeArgs schema (Join-Path $Out 'types_01.00.json')
} else { Write-Warning "no decrypted 01.00 EBOOT at ${BaseElf}: the diffs against 01.00 are skipped" }
foreach ($v in $elves.Keys) {
    & $py "$PSScriptRoot\d1_types.py" --elf $elves[$v] schema (Join-Path $Out "types_$v.json")
}

Step 'comparing builds'
$reports = @()
foreach ($v in ($elves.Keys | Sort-Object)) {
    if (Test-Path $BaseElf) {
        $r = Join-Path $Out "diff_01.00_$v.md"
        $diff = @("$PSScriptRoot\d1_builddiff.py", '--old-elf', $BaseElf, '--new-elf', $elves[$v],
                  '--names', "01.00,$v", '--map-out', (Join-Path $Out "handles_01.00_$v.json"), '--out', $r)
        if ($typeArgs) { $diff += @('--old-packages', $DiscPackages, '--oodle', $Oodle) }
        if ($Packages -and $v -eq ($elves.Keys | Sort-Object)[-1]) { $diff += @('--new-packages', (Join-Path $Out 'final\packages')) }
        & $py @diff
        $reports += $r
    }
}
if ($elves.ContainsKey('01.13') -and $elves.ContainsKey('01.24')) {
    $r = Join-Path $Out 'diff_01.13_01.24.md'
    & $py "$PSScriptRoot\d1_builddiff.py" --old-elf $elves['01.13'] --new-elf $elves['01.24'] --names '01.13,01.24' --out $r
    $reports += $r
}

if ($Packages) {
    Step 'final package set'
    $final = Join-Path $Out 'final\packages'
    if (Test-Path $DiscPackages) {
        # patch _N files read their unchanged blocks from the _0 files beside them
        Get-ChildItem $DiscPackages -Filter '*_0.pkg' | ForEach-Object {
            $target = Join-Path $final $_.Name
            if (-not (Test-Path $target)) {
                try { New-Item -ItemType HardLink -Path $target -Target $_.FullName | Out-Null }
                catch { Copy-Item $_.FullName $target }
            }
        }
    } else { Write-Warning "disc packages not found at ${DiscPackages}: patch files cannot be read alone" }
    $stale = Join-Path $Out 'final\stale.txt'
    if (Test-Path $stale) {
        # files an older update numbered higher than their replacement (01.20 restarted the numbering)
        $staleDir = Join-Path $Out 'final\stale'
        New-Item -ItemType Directory -Force $staleDir | Out-Null
        Get-Content $stale | Where-Object { $_ } | ForEach-Object {
            $p = Join-Path $final $_
            if (Test-Path $p) { Move-Item $p $staleDir -Force }
        }
    }
    if (Test-Path $Oodle) {
        & $py "$PSScriptRoot\d1_pkg.py" named $final | Set-Content (Join-Path $Out 'final_named_tags.txt')
        & $py "$PSScriptRoot\d1_pkg.py" items $final --oodle $Oodle | Set-Content (Join-Path $Out 'final_items.txt')
        & $py "$PSScriptRoot\d1_types.py" --elf $elves[($elves.Keys | Sort-Object)[-1]] --packages $final --oodle $Oodle services |
            Set-Content (Join-Path $Out 'final_services.txt')
    } else { Write-Warning "no Oodle DLL at ${Oodle}: skipped the final package listings" }
}

Step 'done'
"Reports:"; $reports | ForEach-Object { "  $_" }
"Small files to share (types, diffs, lists; no game data):"
Get-ChildItem $Out -File | Where-Object { $_.Extension -in '.md', '.json', '.txt' } | ForEach-Object { "  $($_.FullName)" }
