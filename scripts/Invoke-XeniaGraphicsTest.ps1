<#
Runs one unattended Xenia title-screen graphics test.

Creates D:\Destiny360\XENIA_GFX_<TestId>, copies the patched Xenia executable and base config
into it, applies the given config overrides, boots the working-copy game with the imported
content root, presses F12 (Xenia's guest-output screenshot) at each capture time, optionally F4 (one-frame GPU trace,
written under scratch\gpu) at -TraceAtSeconds, then closes
Xenia. Screenshots land in <test folder>\screenshots\415608F8 and are copied to D:\Destiny360\LOGS.

Example:
  .\scripts\Invoke-XeniaGraphicsTest.ps1 -TestId T014_rov -Set @{ render_target_path_d3d12 = '"rov"' }
Values are written verbatim into the TOML, so quote strings ('"rov"').
#>
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^[A-Za-z0-9_-]+$')][string]$TestId,
    [hashtable]$Set = @{},
    [int[]]$CaptureAtSeconds = @(60, 90),
    [int]$TraceAtSeconds = 0,
    [string]$ExeSource = 'D:\Destiny360\XENIA_TEST009_FULL_CONTENT\xenia_canary.exe',
    [string]$ConfigSource = 'D:\Destiny360\XENIA_TEST009_FULL_CONTENT\xenia-canary.config.toml',
    [string]$ContentRoot = 'D:\Destiny360\XENIA_TEST007_STFS_ACTIVE\content',
    [string]$Game = 'D:\Destiny360\01_WORKING\test001\Content\0000000000000000\415608F8\00004000\ED4E3CCA83E6F64008DDF957088F3E06'
)

$ErrorActionPreference = 'Stop'
if (Get-Process xenia_canary -ErrorAction SilentlyContinue) { throw 'Another Xenia instance is running.' }
if ($Game -like 'D:\Destiny360\00_ORIGINAL_DUMP\*') { throw 'Use a working copy, not the original archive.' }

$dir = "D:\Destiny360\XENIA_GFX_$TestId"
$logs = 'D:\Destiny360\LOGS'
if (Test-Path -LiteralPath $dir) { throw "Test folder already exists: $dir" }
New-Item -ItemType Directory -Path $dir | Out-Null
Copy-Item -LiteralPath $ExeSource -Destination $dir
$config = Join-Path $dir 'xenia-canary.config.toml'
$text = Get-Content -LiteralPath $ConfigSource -Raw
foreach ($key in $Set.Keys) {
    $pattern = "(?m)^$([regex]::Escape($key))\s*=\s*[^#\r\n]*"
    if (-not [regex]::IsMatch($text, $pattern)) { throw "Config key not found: $key" }
    $text = [regex]::Replace($text, $pattern, "$key = $($Set[$key])".PadRight(50))
}
Set-Content -LiteralPath $config -Value $text -NoNewline -Encoding utf8
Copy-Item -LiteralPath $config -Destination (Join-Path $logs "Gfx-$TestId.config.toml")

$log = Join-Path $logs "Gfx-$TestId.xenia.log"
$argsList = @(('"' + $Game + '"'), "--log_file=$log", "--content_root=$ContentRoot")
$proc = Start-Process -FilePath (Join-Path $dir 'xenia_canary.exe') -ArgumentList $argsList -WorkingDirectory $dir -PassThru
$shell = New-Object -ComObject WScript.Shell
$start = Get-Date
try {
    $actions = @($CaptureAtSeconds | ForEach-Object { [pscustomobject]@{ At = $_; Key = '{F12}' } })
    if ($TraceAtSeconds -gt 0) { $actions += [pscustomobject]@{ At = $TraceAtSeconds; Key = '{F4}' } }
    foreach ($a in ($actions | Sort-Object At)) {
        $wait = $a.At - ((Get-Date) - $start).TotalSeconds
        if ($wait -gt 0) { Start-Sleep -Seconds ([math]::Ceiling($wait)) }
        if ($proc.HasExited) { throw "Xenia exited early with code $($proc.ExitCode)" }
        $null = $shell.AppActivate($proc.Id)
        Start-Sleep -Milliseconds 500
        $shell.SendKeys($a.Key)
        Start-Sleep -Seconds $(if ($a.Key -eq '{F4}') { 8 } else { 2 })
    }
} finally {
    if (-not $proc.HasExited) { $null = $proc.CloseMainWindow(); if (-not $proc.WaitForExit(15000)) { Stop-Process -Id $proc.Id -Force } }
}

$shots = Get-ChildItem -Path (Join-Path $dir 'screenshots') -Recurse -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime
$i = 0
foreach ($s in $shots) {
    $i++
    Copy-Item -LiteralPath $s.FullName -Destination (Join-Path $logs "Gfx-$TestId-$i$($s.Extension)")
}
$traces = Get-ChildItem -Path (Join-Path $dir 'scratch\gpu') -Recurse -File -ErrorAction SilentlyContinue
foreach ($t in $traces) { Copy-Item -LiteralPath $t.FullName -Destination (Join-Path $logs "Gfx-$TestId-$($t.Name)") }
[pscustomobject]@{ Traces = ($traces | ForEach-Object { "$($_.Name) ($($_.Length) bytes)" }) -join '; '; TestId = $TestId; Overrides = ($Set.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join '; '; Screenshots = $i; Log = $log }
