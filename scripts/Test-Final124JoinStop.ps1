# Isolated shutdown regression: sleeping children, no emulator or network service.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Final124RunHelpers.ps1"
function Assert-StopCheck($Actual, $Expected, [string]$Label) {
    if ($Actual -ne $Expected) { throw "$Label expected '$Expected', got '$Actual'." }
    Write-Host "PASS $Label"
}

$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ('d1-join-stop-' + [guid]::NewGuid())
$fixtureScripts = Join-Path $fixtureRoot 'scripts'
$fixtureLocal = Join-Path $fixtureRoot 'runtime'
$fixtureData = Join-Path $fixtureRoot 'data'
$fixtureLogs = Join-Path $fixtureLocal 'SIGNON_LOCAL\logs'
$identities = @()
$previousLocal = $env:D1_LOCAL
$previousRpcs3 = $env:D1_RPCS3
try {
    New-Item -ItemType Directory -Path $fixtureScripts, $fixtureData, $fixtureLogs,
        (Join-Path $fixtureLocal 'RPCS3_FINAL124_SMOKE001\log') -Force | Out-Null
    foreach ($file in @('Paths.ps1', 'Final124RunHelpers.ps1', 'Invoke-Final124Join.ps1')) {
        $tokens = $null; $parseErrors = $null
        [void][System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $file), [ref]$tokens, [ref]$parseErrors)
        Assert-StopCheck @($parseErrors).Count 0 "$file syntax"
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination $fixtureScripts
    }
    Set-Content -LiteralPath (Join-Path $fixtureLocal 'RPCS3_FINAL124_SMOKE001\log\RPCS3.log') 'fixture client evidence'
    $pwsh = (Get-Command pwsh).Source
    foreach ($tag in @('bap', 'rpcs3', 'gdbd', 'proxy', 'unrelated')) {
        $child = Start-Process -FilePath $pwsh -ArgumentList '-NoProfile -Command "Start-Sleep -Seconds 90"' -WindowStyle Hidden -PassThru
        $identities += Get-D1ProcessIdentity $child $tag $pwsh
    }
    $client = $identities | Where-Object tag -eq rpcs3
    $backend = $identities | Where-Object tag -eq bap
    $debugger = $identities | Where-Object tag -eq gdbd
    $unrelated = $identities | Where-Object tag -eq unrelated
    $stale = $unrelated | Select-Object *
    $stale.startTimeUtcTicks = ([long]$stale.startTimeUtcTicks + 1).ToString()
    $stackManifest = Join-Path $fixtureLogs 'FINAL124_TEST-processes.json'
    @{name='FINAL124_TEST';processes=@($backend, $stale, $client, $debugger)} |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $stackManifest
    $identities | Where-Object tag -eq proxy | ConvertTo-Json |
        Set-Content -LiteralPath (Join-Path $fixtureLogs 'FINAL124_TEST-proxy-process.json')
    @{client=$client;backend=$backend;debugger=$debugger} | ConvertTo-Json -Depth 5 |
        Set-Content -LiteralPath (Join-Path $fixtureScripts 'observed-identities.json')
    # Observe actual child liveness at each real stop, rather than just sorting tags.
    @'
$script:realFixtureStop = (Get-Command Stop-D1OwnedProcess).ScriptBlock
$script:stopFixtureScripts = $PSScriptRoot
function Stop-D1OwnedProcess {
    param([Parameter(Mandatory)]$Identity)
    $observed = Get-Content -LiteralPath (Join-Path $script:stopFixtureScripts 'observed-identities.json') -Raw | ConvertFrom-Json
    @{tag=$Identity.tag;client=[bool](Get-D1OwnedProcess $observed.client);
      backend=[bool](Get-D1OwnedProcess $observed.backend);debugger=[bool](Get-D1OwnedProcess $observed.debugger)} |
        ConvertTo-Json -Compress | Add-Content -LiteralPath (Join-Path $script:stopFixtureScripts 'stops.jsonl')
    & $script:realFixtureStop -Identity $Identity
}
'@ | Add-Content -LiteralPath (Join-Path $fixtureScripts 'Final124RunHelpers.ps1')
    $stopScript = Join-Path $fixtureScripts 'Invoke-Final124Join.ps1'
    $failure = $null
    try { & $stopScript -Name TEST -Local $fixtureLocal -DataRepo $fixtureData -Stop }
    catch { $failure = $_.Exception.Message }
    Assert-StopCheck ([bool]($failure -like '*exact original -Name*')) $true 'unknown shortened name fails clearly'
    Assert-StopCheck ([bool](Get-D1OwnedProcess $client)) $true 'unknown name keeps client running'
    Assert-StopCheck ([bool](Get-D1OwnedProcess $backend)) $true 'unknown name keeps backend running'
    Assert-StopCheck (Test-Path -LiteralPath (Join-Path $fixtureScripts 'stops.jsonl')) $false 'unknown name attempts no stops'

    & $stopScript -Name FINAL124_TEST -Local $fixtureLocal -DataRepo $fixtureData -Stop
    $observations = @(Get-Content -LiteralPath (Join-Path $fixtureScripts 'stops.jsonl') | ConvertFrom-Json)
    $clientStop = $observations | Where-Object tag -eq rpcs3
    Assert-StopCheck $clientStop.backend $true 'backend stays alive until client shutdown'
    Assert-StopCheck $clientStop.debugger $false 'debugger detaches before client shutdown'
    foreach ($tag in @('bap', 'proxy')) {
        Assert-StopCheck ($observations | Where-Object tag -eq $tag).client $false "$tag stops after client exit"
    }
    Assert-StopCheck ([bool](Get-D1OwnedProcess $unrelated)) $true 'stale identity cannot stop unrelated child'
    Assert-StopCheck (Test-Path -LiteralPath "$stackManifest.stop") $true 'cooperative shutdown marker is written'
    Assert-StopCheck (Get-Content -LiteralPath (Join-Path $fixtureLogs 'FINAL124_TEST-rpcs3-final.log')) 'fixture client evidence' 'final client evidence is retained'
    foreach ($identity in $identities | Where-Object tag -ne unrelated) {
        Assert-StopCheck ([bool](Get-D1OwnedProcess $identity)) $false "$($identity.tag) has exited"
    }
    & $stopScript -Name FINAL124_TEST -Local $fixtureLocal -DataRepo $fixtureData -Stop
    Assert-StopCheck ([bool](Get-D1OwnedProcess $unrelated)) $true 'repeated stop preserves unrelated child'
    Assert-StopCheck (Get-Content -LiteralPath (Join-Path $fixtureLogs 'FINAL124_TEST-rpcs3-final.log')) 'fixture client evidence' 'repeated stop preserves prior evidence'
    Write-Host 'All isolated join shutdown checks passed.' -ForegroundColor Green
} finally {
    foreach ($identity in $identities) { Stop-D1OwnedProcess $identity }
    $env:D1_LOCAL = $previousLocal
    $env:D1_RPCS3 = $previousRpcs3
    $resolvedRoot = [IO.Path]::GetFullPath($fixtureRoot)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $resolvedRoot.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path $resolvedRoot -Leaf) -notmatch '^d1-join-stop-[0-9a-f-]{36}$') {
        throw "Refusing to remove unexpected fixture directory: $resolvedRoot"
    }
    if (Test-Path -LiteralPath $resolvedRoot) { Remove-Item -LiteralPath $resolvedRoot -Recurse -Force }
}
