# An optional, independent, user-local worker. No config means no capture side effects.
param([string]$Config, [switch]$Stop, [switch]$InstallStartup)
. "$PSScriptRoot\Paths.ps1"
. "$PSScriptRoot\Final124RunHelpers.ps1"
$ErrorActionPreference='Stop'
if (-not $Config) { $Config=if ($env:D1_CONTENT_CONFIG) { $env:D1_CONTENT_CONFIG } else { Join-Path $D1Local 'content-capture\config.json' } }
if (-not (Test-Path -LiteralPath $Config)) { return }
$settings=Get-Content -LiteralPath $Config -Raw | ConvertFrom-Json
$root=[IO.Path]::GetFullPath($settings.CONTENT_LOCAL_PATH,(Split-Path ([IO.Path]::GetFullPath($Config)) -Parent))
if (-not $settings.CONTENT_CAPTURE_ENABLED -and -not $Stop) { return }
New-Item -ItemType Directory -Path $root -Force | Out-Null
$receipt=Join-Path $root 'monitor-process.json'
$identity=if (Test-Path -LiteralPath $receipt) { Get-Content -LiteralPath $receipt -Raw | ConvertFrom-Json }
if ($Stop) {
    New-Item -ItemType File -Path (Join-Path $root 'monitor.stop') -Force | Out-Null
    if ($identity -and ($worker=Get-D1OwnedProcess $identity)) {
        if (-not $worker.WaitForExit(30000)) { throw 'Content worker is still draining; its local recordings and outbox have been retained.' }
    }
    return
}
if (-not ($identity -and (Get-D1OwnedProcess $identity))) {
    Remove-Item -LiteralPath (Join-Path $root 'monitor.stop') -ErrorAction SilentlyContinue
    $args=@('-u',(Join-Path $D1Repo 'tools\content_capture\monitor.py'),'--config',$Config) | ForEach-Object { ConvertTo-D1ProcessArgument $_ }
    $process=Start-Process -FilePath $D1Python -ArgumentList $args -WindowStyle Hidden -PassThru -WorkingDirectory $D1Repo `
        -RedirectStandardOutput (Join-Path $root 'monitor.log') -RedirectStandardError (Join-Path $root 'monitor.err')
    $identity=Get-D1ProcessIdentity -Process $process -Tag content-monitor -Executable $D1Python
    [IO.File]::WriteAllText($receipt,($identity | ConvertTo-Json))
}
if ($InstallStartup) {
    # Startup shortcut uses the same idempotent ownership guard; credentials are never embedded.
    $shell=New-Object -ComObject WScript.Shell
    $link=$shell.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Startup')) 'Destiny Content Capture.lnk'))
    $link.TargetPath=(Get-Command pwsh -ErrorAction Stop).Source
    $link.Arguments='-NoProfile -WindowStyle Hidden -File '+(ConvertTo-D1ProcessArgument $PSCommandPath)+' -Config '+(ConvertTo-D1ProcessArgument ([IO.Path]::GetFullPath($Config)))
    $link.WorkingDirectory=$D1Repo
    $link.WindowStyle=7
    $link.Save()
}
$identity
