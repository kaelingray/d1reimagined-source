# Shared locations for the local test setup, dot-sourced by the other scripts:
#   . "$PSScriptRoot\Paths.ps1"
# Private runtime and data live in <repo>\local (git-ignored; D1_LOCAL overrides it), so the whole
# project can be copied to another PC as one folder.
$D1Repo = Split-Path $PSScriptRoot
$D1Local = if ($env:D1_LOCAL) { $env:D1_LOCAL } else { Join-Path $D1Repo 'local' }
$D1Rpcs3 = Join-Path $D1Local 'RPCS3_TEST048_SETUP'          # diagnostic RPCS3 build + config
$D1Control = Join-Path $D1Rpcs3 'd1'                          # its marker files, watch list, dumps
$D1Signon = Join-Path $D1Local 'SIGNON_LOCAL'                 # stand-in schemas, captures
$D1Logs = Join-Path $D1Signon 'logs'
$D1Shots = Join-Path $D1Rpcs3 'screenshots\BLES01857'
$D1Python = (Get-Command python -ErrorAction Stop).Source
