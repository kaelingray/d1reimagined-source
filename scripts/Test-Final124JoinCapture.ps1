# Real native PowerShell boundaries; no emulator, keyboard or network service.
$ErrorActionPreference='Stop'
. "$PSScriptRoot\Final124RunHelpers.ps1"
$root=Join-Path ([IO.Path]::GetTempPath()) ('d1-join-capture-'+[guid]::NewGuid())
$savedNativePreference=$PSNativeCommandUseErrorActionPreference
$savedLocal=$env:D1_LOCAL
$savedRpcs3=$env:D1_RPCS3
$savedPython=$env:D1_PYTHON
$owned=@()
function Assert-CaptureCheck($Actual,$Expected,[string]$Label) {
    if ($Actual -ne $Expected) { throw "$Label expected '$Expected', got '$Actual'." }
    Write-Host "PASS $Label"
}
try {
    $PSNativeCommandUseErrorActionPreference=$false # reproduces the ignored-exit bug
    New-Item -ItemType Directory -Path $root | Out-Null
    $success=Join-Path $root 'successful screenshot.ps1'
    @'
param([string]$Value)
Write-Output $Value
exit 0
'@ | Set-Content -LiteralPath $success
    $failure=Join-Path $root 'failed screenshot.ps1'
    @'
exit 7
'@ | Set-Content -LiteralPath $failure
    $literal='path with spaces; $unchanged and `literal'
    $output=@(Invoke-D1CheckedPowerShellStep -Path $success -Arguments @('-Value',$literal))
    Assert-CaptureCheck $output.Count 1 'successful child preserves one result'
    Assert-CaptureCheck $output[0] $literal 'literal argument survives native boundary'
    $failureMessage=$null
    $continued=$false
    try {
        Invoke-D1CheckedPowerShellStep -Path $failure | Out-Null
        $continued=$true
    } catch { $failureMessage=$_.Exception.Message }
    Assert-CaptureCheck $continued $false 'failed screenshot prevents subsequent completion'
    Assert-CaptureCheck ([bool]($failureMessage -like '*failed screenshot.ps1*exit code 7*')) $true 'child exit identifies failed step'
    $output=@(Invoke-D1CheckedPowerShellStep -Path $success -Arguments @('-Value','next result'))
    Assert-CaptureCheck $output[0] 'next result' 'later success replaces stale native failure status'
    $tokens=$null;$parseErrors=$null
    [Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'Invoke-Final124Join.ps1'),[ref]$tokens,[ref]$parseErrors) | Out-Null
    Assert-CaptureCheck $parseErrors.Count 0 'join runner syntax'
    # Execute the actual join runner around harmless sleeping children. Only the
    # unavailable game/input/backend steps are replaced; ownership cleanup is real.
    $scripts=Join-Path $root 'scripts'
    $local=Join-Path $root 'runtime'
    $data=Join-Path $root 'data'
    $logs=Join-Path $local 'SIGNON_LOCAL\logs'
    New-Item -ItemType Directory -Path $scripts,$data,$logs,(Join-Path $root 'server'),
        (Join-Path $local 'RPCS3_FINAL124_SMOKE001\log'),(Join-Path $local 'RESEARCH\ps3_updates') -Force | Out-Null
    foreach ($file in 'Paths.ps1','Final124RunHelpers.ps1','Invoke-Final124Join.ps1') {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination $scripts
    }
    @'
function Start-Sleep { param($Seconds=0,$Milliseconds=0) [Threading.Thread]::Sleep(1) }
'@ | Add-Content -LiteralPath (Join-Path $scripts 'Final124RunHelpers.ps1')
    'param($Mode,$Rpcs3Dir)' | Set-Content -LiteralPath (Join-Path $scripts 'Set-PadConfig.ps1')
    'import time; time.sleep(90)' | Set-Content -LiteralPath (Join-Path $root 'server\bap_server.py')
    'print("isolated mock state; no native acceptance")' | Set-Content -LiteralPath (Join-Path $scripts 'rpcs3_final124_bootflow.py')
    @'
param($Name,$Local,$DataRepo,$BootSeconds,$PressSeconds,[switch]$ContinueAfterBap,[switch]$KeepRpcs3,$BapExtra)
. "$PSScriptRoot\Final124RunHelpers.ps1"
$children=@()
$pwsh=(Get-Command pwsh).Source
foreach ($tag in 'rpcs3','bap') {
    $child=Start-Process -FilePath $pwsh -ArgumentList '-NoProfile -Command "Start-Sleep -Seconds 90"' -WindowStyle Hidden -PassThru
    $children+=Get-D1ProcessIdentity -Process $child -Tag $tag -Executable $pwsh
}
$logs=Join-Path $Local 'SIGNON_LOCAL\logs'
@{name=$Name;processes=$children} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $logs "$Name-processes.json")
'fixture evidence' | Set-Content -LiteralPath (Join-Path $Local 'RPCS3_FINAL124_SMOKE001\log\RPCS3.log')
"upo rsp`nmm rsp" | Set-Content -LiteralPath (Join-Path $logs "$Name-bap.log")
'mock launcher finished; sleeping identities retained'
'@ | Set-Content -LiteralPath (Join-Path $scripts 'Invoke-Final124Run.ps1')
    @'
param($ProcessId,[string[]]$Keys=@(),$HoldMs,$WaitSeconds)
$phase=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'failure-phase.txt')
if (($phase -eq 'first' -and $Keys -contains 'D') -or ($phase -eq 'final' -and -not $Keys.Count)) { exit 7 }
'isolated mock screenshot result'
'@ | Set-Content -LiteralPath (Join-Path $scripts 'Invoke-RpcsStep.ps1')
    $env:D1_PYTHON=if ($savedPython) {$savedPython}else{(Get-Command python -ErrorAction Stop).Source}
    foreach ($phase in 'first','final') {
        $run='FINAL124_CAPTURE_'+$phase
        Set-Content -LiteralPath (Join-Path $scripts 'failure-phase.txt') $phase
        $outputs=@();$failureMessage=$null
        try { $outputs=@(& (Join-Path $scripts 'Invoke-Final124Join.ps1') -Name $run -Local $local -DataRepo $data -AfterMatchSeconds 5) }
        catch { $failureMessage=$_.Exception.Message }
        $manifest=Read-D1ProcessManifest (Join-Path $logs "$run-processes.json")
        $identities=@($manifest.processes)+(Get-Content -LiteralPath (Join-Path $logs "$run-proxy-process.json") -Raw | ConvertFrom-Json)
        $owned+=$identities
        Assert-CaptureCheck ([bool]($failureMessage -like '*Invoke-RpcsStep.ps1*exit code 7*')) $true "$phase failure propagates through actual join runner"
        Assert-CaptureCheck ([bool](($outputs -join '\n') -match 'done; close with')) $false "$phase failure cannot report done"
        Assert-CaptureCheck @($identities | Where-Object { Get-D1OwnedProcess $_ }).Count 0 "$phase failure closes all exact owned children and proxy"
        Assert-CaptureCheck (Test-Path -LiteralPath (Join-Path $logs "$run-processes.json.stop")) $true "$phase failure records cooperative shutdown"
    }
    'All fourteen isolated native capture checks passed.'
} finally {
    foreach ($identity in $owned) { Stop-D1OwnedProcess $identity }
    $PSNativeCommandUseErrorActionPreference=$savedNativePreference
    $env:D1_LOCAL=$savedLocal
    $env:D1_RPCS3=$savedRpcs3
    $env:D1_PYTHON=$savedPython
    $absolute=[IO.Path]::GetFullPath($root)
    $temp=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\'
    if (-not $absolute.StartsWith($temp,[StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path $absolute -Leaf) -notmatch '^d1-join-capture-[0-9a-f-]{36}$') { throw 'Unexpected native capture fixture path' }
    if (Test-Path -LiteralPath $absolute) { Remove-Item -LiteralPath $absolute -Recurse -Force }
}
