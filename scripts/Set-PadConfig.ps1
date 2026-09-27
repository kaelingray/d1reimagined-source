# Switch the private RPCS3 copy's player-1 pad config between the keyboard pad the unattended
# scripts drive (Send-RpcsKey.ps1) and the user's XInput controller. Refuses while RPCS3 runs,
# because RPCS3 may rewrite the config on exit.
#   Set-PadConfig.ps1 -Mode keyboard   (before unattended runs)
#   Set-PadConfig.ps1 -Mode xinput     (afterwards, for the user)
param([Parameter(Mandatory)][ValidateSet('keyboard', 'xinput')][string]$Mode)
$dir = 'D:\Destiny360\RPCS3_TEST048_SETUP\config\input_configs\global'
if (Get-Process rpcs3 -ErrorAction SilentlyContinue) { throw 'Close RPCS3 first.' }
$source = if ($Mode -eq 'keyboard') { "$dir\Default.keyboard.automation.yml" } else { "$dir\Default.xinput.user.yml.bak" }
if (-not (Test-Path $source)) { throw "Missing $source" }
Copy-Item $source "$dir\Default.yml" -Force
"Player 1 pad: " + ((Get-Content "$dir\Default.yml" -TotalCount 2)[1].Trim())
