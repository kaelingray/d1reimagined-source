# Isolated native-recorder shutdown ownership test. No game, keyboard input or NAS I/O.
$ErrorActionPreference='Stop'
. "$PSScriptRoot\Final124RunHelpers.ps1"
$root=Join-Path ([IO.Path]::GetTempPath()) ('d1-content-test-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$savedConfig=$env:D1_CONTENT_CONFIG
$owned=@()
function Check([bool]$Result,[string]$Message) { if (-not $Result) { throw "FAIL $Message" }; "PASS $Message" }
function Start-Fixture([string]$Tag,[string]$Script) {
    $exe=(Get-Command pwsh).Source
    $child=Start-Process -FilePath $exe -ArgumentList @('-NoProfile','-File',(ConvertTo-D1ProcessArgument $Script)) -WindowStyle Hidden -PassThru
    Get-D1ProcessIdentity -Process $child -Tag $Tag -Executable $exe
}
try {
    $config=Join-Path $root 'config.json'
    @{ CONTENT_LOCAL_PATH=$root } | ConvertTo-Json | Set-Content -LiteralPath $config
    $env:D1_CONTENT_CONFIG=$config
    $sleep=Join-Path $root 'sleep.ps1'
    'Start-Sleep 45' | Set-Content -LiteralPath $sleep
    $game=Start-Fixture rpcs3 $sleep
    $backend=Start-Fixture bap $sleep
    $owned+=@($game,$backend)
    $recordScript=Join-Path $root 'record.ps1'
    $body=@'
param([string]$Root,[int]$GameId,[int]$BackendId)
Set-Content -LiteralPath (Join-Path $Root 'recorder-started') -Value ready
while (-not (Test-Path -LiteralPath (Join-Path $Root 'recorder.stop'))) { Start-Sleep -Milliseconds 50 }
@{ game_alive=[bool](Get-Process -Id $GameId -ErrorAction SilentlyContinue); backend_alive=[bool](Get-Process -Id $BackendId -ErrorAction SilentlyContinue) } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Root 'drained.json')
'@
    $body | Set-Content -LiteralPath $recordScript
    $exe=(Get-Command pwsh).Source
    $arguments=@('-NoProfile','-File',$recordScript,'-Root',$root,'-GameId',$game.id,'-BackendId',$backend.id) | ForEach-Object { ConvertTo-D1ProcessArgument $_ }
    $child=Start-Process -FilePath $exe -ArgumentList $arguments -WindowStyle Hidden -PassThru
    $recorder=Get-D1ProcessIdentity -Process $child -Tag native-recorder -Executable $exe
    $owned+=$recorder
    @{ recorder=$recorder; emulator=$game } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'active-recorder-process.json')
    $deadline=(Get-Date).AddSeconds(5)
    while (-not (Test-Path -LiteralPath (Join-Path $root 'recorder-started')) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 50 }
    Check (Test-Path -LiteralPath (Join-Path $root 'recorder-started')) 'recorder fixture is ready'
    Stop-D1OwnedProcess $game
    $receipt=Get-Content -LiteralPath (Join-Path $root 'drained.json') -Raw | ConvertFrom-Json
    Check $receipt.game_alive 'recorder drains while exact client is alive'
    Check $receipt.backend_alive 'backend stays alive through recorder drain'
    Check (-not (Get-D1OwnedProcess $recorder)) 'matching recorder exits cooperatively'
    Check (-not (Get-D1OwnedProcess $game)) 'client then exits'
    Check ([bool](Get-D1OwnedProcess $backend)) 'backend is preserved for later ordered cleanup'
    Remove-Item -LiteralPath (Join-Path $root 'recorder.stop')
    $other=Start-Fixture rpcs3 $sleep
    $owned+=$other
    $stale=[pscustomobject]@{ tag=$other.tag; id=$other.id; executable=$other.executable; startTimeUtcTicks=([long]$other.startTimeUtcTicks+1).ToString() }
    @{ recorder=$backend; emulator=$stale } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'active-recorder-process.json')
    Stop-D1OwnedProcess $other
    Check (-not (Test-Path -LiteralPath (Join-Path $root 'recorder.stop'))) 'stale emulator identity cannot request recorder shutdown'
    Check ([bool](Get-D1OwnedProcess $backend)) 'stale recorder sidecar cannot affect unrelated process'
} finally {
    $env:D1_CONTENT_CONFIG=$savedConfig
    foreach ($identity in $owned) { Stop-D1OwnedProcess $identity }
    # Exact GUID-scoped fixture path, resolved and checked before recursive cleanup.
    $resolved=[IO.Path]::GetFullPath($root)
    if ($resolved.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()),[StringComparison]::OrdinalIgnoreCase) -and (Split-Path $resolved -Leaf) -like 'd1-content-test-*') {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
'All isolated content-capture ownership checks passed.'
