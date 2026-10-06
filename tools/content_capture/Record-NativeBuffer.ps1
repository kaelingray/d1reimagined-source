# Targeted native game video/audio; runs alongside testing, with exact run ownership.
param(
    [Parameter(Mandatory)][string]$Config,
    [Parameter(Mandatory)][string]$ProcessManifest,
    [Parameter(Mandatory)][string]$Segments,
    [Parameter(Mandatory)][string]$StopFile,
    [ValidateRange(10,120)][int]$SegmentSeconds=30
)
$ErrorActionPreference='Stop'
$repo=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
. (Join-Path $repo 'scripts\Final124RunHelpers.ps1')
$manifest=Read-D1ProcessManifest $ProcessManifest
$identity=@($manifest.processes | Where-Object tag -eq rpcs3)
if ($identity.Count -ne 1 -or -not (Get-D1OwnedProcess $identity[0])) { throw 'Exact emulator ownership is unavailable' }
$identity=$identity[0]
$root=Split-Path $Segments -Parent
$recorderIdentity=Get-D1ProcessIdentity -Process (Get-Process -Id $PID) -Tag native-recorder
$record=@{ recorder=$recorderIdentity; emulator=$identity }
[IO.File]::WriteAllText((Join-Path $root 'active-recorder-process.json'),($record | ConvertTo-Json -Depth 5))
$rpcs=Split-Path $identity.executable -Parent
$recordings=Join-Path $rpcs 'recordings\BLES01857'
New-Item -ItemType Directory -Path $Segments -Force | Out-Null
$lock=[IO.FileStream]::new((Join-Path $Segments 'recorder.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
$recording=$false
$active=$null
$stopRequestedAt=$null
$receipt=$null
function Write-RecorderPhase([string]$Phase, [hashtable]$Details=@{}) {
    # stderr preserves boolean input results and survives append-only log handles.
    try {
        $value=[ordered]@{ component='native-recorder'; at=[DateTimeOffset]::UtcNow.ToString('o'); run=$manifest.name; phase=$Phase; details=$Details }
        [Console]::Error.WriteLine(($value | ConvertTo-Json -Depth 5 -Compress))
    } catch { }
}
function Stop-Requested { (Test-Path -LiteralPath $StopFile) -or (Test-Path -LiteralPath "$ProcessManifest.stop") -or -not (Get-D1OwnedProcess $identity) }
function Send-RecordKey {
    & (Join-Path $repo 'scripts\Send-RpcsKey.ps1') -ProcessId $identity.id -ProcessIdentity $identity -Keys F11 -HoldMs 30 -GapMs 0 -IfWindowReady -InputDiagnostics
}
function Save-PendingSegment {
    $value=@{ file=$script:active; recorded_at=$script:began; run=$manifest.name; source='RPCS3 native game/audio'; ownership=$identity; stop_requested_at=$script:stopRequestedAt }
    $temp=$script:receipt+'.tmp'
    [IO.File]::WriteAllText($temp,($value | ConvertTo-Json -Depth 5))
    Move-Item -LiteralPath $temp -Destination ($script:receipt+'.pending') -Force
}
function Finish-Segment([int]$WaitSeconds=15) {
    if (-not $script:recording) { return $true }
    if (-not $script:active) { return $false }
    if (-not $script:stopRequestedAt) {
        # Posting a shortcut does not prove the GUI consumed it. Never toggle
        # again until the identified source is demonstrably closed.
        $script:stopRequestedAt=[DateTimeOffset]::UtcNow.ToString('o')
        Save-PendingSegment
        Write-RecorderPhase 'stop-requested' @{ source=$script:active }
        if (Get-D1OwnedProcess $identity) { Send-RecordKey | Out-Null }
    }
    $size=0
    $deadline=(Get-Date).AddSeconds($WaitSeconds)
    do {
        try {
            $stream=[IO.FileStream]::new($script:active,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
            $size=$stream.Length
            $stream.Dispose()
            if ($size -gt 0) { break }
        } catch { }
        if ((Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
    } until ((Get-Date) -ge $deadline)
    if ($size -gt 0) {
        $end=[DateTimeOffset]::UtcNow.ToString('o')
        $value=@{ file=$script:active; recorded_at=$script:began; ended_at=$end; closure_observed_at=$end; stop_requested_at=$script:stopRequestedAt; run=$manifest.name; source='RPCS3 native game/audio'; ownership=$identity }
        $temp=$script:receipt+'.tmp'
        [IO.File]::WriteAllText($temp,($value | ConvertTo-Json -Depth 5))
        Move-Item -LiteralPath $temp -Destination ($script:receipt+'.json') -Force
        Remove-Item -LiteralPath ($script:receipt+'.pending')
        Write-RecorderPhase 'source-finalized' @{ source=$script:active; size=$size; receipt=(Split-Path $script:receipt -Leaf) }
        $script:recording=$false
        $script:active=$null
        $script:stopRequestedAt=$null
        $script:receipt=$null
        return $true
    } else {
        Write-RecorderPhase 'source-finalization-timeout' @{ source=$script:active }
    }
    return $false
}
try {
    # Refuse to toggle an existing active recording owned by another controller.
    $latest=Get-ChildItem -LiteralPath $recordings -Filter '*.mp4' -ErrorAction SilentlyContinue | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
    if ($latest) {
        try { $s=[IO.FileStream]::new($latest.FullName,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read); $s.Dispose() }
        catch { throw 'A native recording is already active; refusing to toggle it' }
    }
    :segments while (-not (Stop-Requested)) {
        $before=@(Get-ChildItem -LiteralPath $recordings -Filter '*.mp4' -ErrorAction SilentlyContinue | ForEach-Object FullName)
        $script:began=[DateTimeOffset]::UtcNow.ToString('o')
        Write-RecorderPhase 'start-requested' @{ existing_sources=$before.Count }
        $sent=Send-RecordKey
        Write-RecorderPhase 'start-key-returned' @{ posted=[bool]$sent }
        if (-not $sent) { Start-Sleep 1; continue }
        $script:recording=$true
        $deadline=(Get-Date).AddSeconds(8)
        do {
            $new=@(Get-ChildItem -LiteralPath $recordings -Filter '*.mp4' -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.FullName })
            if ($new.Count -eq 1) { $script:active=$new[0].FullName; break }
            Start-Sleep -Milliseconds 100
        } until ((Get-Date) -ge $deadline -or (Stop-Requested))
        Write-RecorderPhase $(if ($script:active) { 'source-identified' } else { 'source-missing' }) @{
            candidates=@($new | ForEach-Object { @{ name=$_.Name; created_at=$_.CreationTimeUtc.ToString('o'); size=$_.Length } })
            elapsed_seconds=([DateTimeOffset]::UtcNow-[DateTimeOffset]::Parse($script:began)).TotalSeconds
        }
        if (-not $script:active) { throw 'Native recorder did not create one identifiable new source file' }
        $script:receipt=Join-Path $Segments ([Guid]::NewGuid().ToString('N'))
        Save-PendingSegment
        $until=(Get-Date).AddSeconds($SegmentSeconds)
        while ((Get-Date) -lt $until -and -not (Stop-Requested)) { Start-Sleep -Milliseconds 250 }
        while (-not (Finish-Segment)) {
            if (Stop-Requested) { break segments }
            Start-Sleep -Milliseconds 250
        }
    }
} finally {
    try {
        if ($active) { Finish-Segment -WaitSeconds 0 | Out-Null }
        elseif ($recording) {
            Write-RecorderPhase 'unidentified-source-preserved' @{ reason='Recording state is unknown; no cleanup toggle is safe' }
        }
    }
    finally { $lock.Dispose() }
}
