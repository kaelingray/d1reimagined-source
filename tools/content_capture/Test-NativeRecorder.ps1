# Exercise the actual controller functions against a real Windows writer lock.
$ErrorActionPreference='Stop'
$tokens=$null; $parseErrors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'Record-NativeBuffer.ps1'),[ref]$tokens,[ref]$parseErrors)
if ($parseErrors.Count) { throw ($parseErrors | Out-String) }
foreach ($name in 'Save-PendingSegment','Finish-Segment') {
    $definition=$ast.Find({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name },$true)
    Invoke-Expression $definition.Extent.Text
}
function Write-RecorderPhase { }
function Get-D1OwnedProcess { $true }
function Send-RecordKey { $script:toggles++; $true }
function Assert-Recorder([bool]$Condition,[string]$Message) {
    if (-not $Condition) { throw $Message }
    $script:checks++
}
$folder=Join-Path ([IO.Path]::GetTempPath()) ('d1-native-recorder-test-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $folder | Out-Null
$writer=$null; $checks=0; $toggles=0
try {
    $manifest=@{name='test-owned-run'}; $identity=@{id=1;executable='fixture'}
    $script:began=[DateTimeOffset]::UtcNow.ToString('o')
    $script:active=Join-Path $folder 'source.mp4'
    $script:receipt=Join-Path $folder 'receipt'
    $script:recording=$true; $script:stopRequestedAt=$null
    $writer=[IO.FileStream]::new($active,[IO.FileMode]::Create,[IO.FileAccess]::Write,[IO.FileShare]::Read)
    $writer.Write([byte[]](1,2,3)); $writer.Flush()
    Save-PendingSegment
    Assert-Recorder (-not (Finish-Segment -WaitSeconds 0)) 'An open writer must remain pending'
    Assert-Recorder ($recording -and $active -and (Test-Path ($receipt+'.pending'))) 'Timeout lost the identified source'
    Assert-Recorder ($toggles -eq 1) 'First stop must post exactly one toggle'
    Assert-Recorder (-not (Finish-Segment -WaitSeconds 0)) 'Repeated/finally polling must remain pending'
    Assert-Recorder ($toggles -eq 1) 'Pending finalization posted an extra toggle'
    $writer.Dispose(); $writer=$null
    Assert-Recorder (Finish-Segment -WaitSeconds 0) 'Closed source was not finalized'
    Assert-Recorder ($toggles -eq 1 -and -not $recording -and -not $active) 'Closure must clear state without another toggle'
    $info=Get-Content (Join-Path $folder 'receipt.json') -Raw | ConvertFrom-Json
    Assert-Recorder ([bool]$info.stop_requested_at -and [bool]$info.closure_observed_at) 'Receipt conflated stop request and closure'
    Assert-Recorder (-not (Test-Path (Join-Path $folder 'receipt.pending'))) 'Finalized receipt left pending state'
    $script:recording=$true; $script:active=$null
    Assert-Recorder (-not (Finish-Segment -WaitSeconds 0)) 'Unknown recording state must remain unconfirmed'
    Assert-Recorder ($toggles -eq 1) 'Unknown recording state posted a blind cleanup toggle'
    "PASS: $checks native recorder checks (real writer lock, repeated stop, closure and unknown-source cleanup)"
} finally {
    if ($writer) { $writer.Dispose() }
    # This exact, freshly created test directory is the only deletion target.
    $resolved=[IO.Path]::GetFullPath($folder)
    if ($resolved -ne $folder -or (Split-Path $resolved -Leaf) -notlike 'd1-native-recorder-test-*') { throw 'Unexpected test cleanup target' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
