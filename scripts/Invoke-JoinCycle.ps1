# One unattended activity-join experiment (about 8 minutes): start the loopback stack with the
# given BAP/lobby arguments, start a second BAP stand-in as the activity host proxy
# (127.1.1.128:12851) and a catch-all listener (127.1.1.129:12852), log in with the stored
# character (Invoke-TitanLogin.ps1), trace ADDRs through the GDB daemon for SECONDS, then print
# the lobby, BAP, proxy and listener logs and an F12 screenshot path.
# Example:
#   Invoke-JoinCycle.ps1 -Name TEST094 -LobbyArgs @('--reply-type','0x01','--session-result') `
#     -AhpReply 00000000000000007f01018000003233 -AhArgs @('--ah-mgr-reply', <hex>) -Trace 0x3204a8
param([string]$Name, [string[]]$LobbyArgs, [string]$Trace = '0x3204a8', [string]$Dumps = 'r5:64', [int]$Seconds = 420, [int]$Hits = 60, [string]$AhpReply = '000102030405060708090a0b0c0d0e0f', [string[]]$ExtraBap = @(), [string[]]$AhArgs = @())
Set-Location C:\Users\lukem\source\repos\WIP\d1xenia
.\scripts\Start-LocalStack.ps1 -Name $Name -BapArgs (@('--universe-time','s','--self-flags','1;2@B0,C0-1023','--ahp-reply',$AhpReply) + $ExtraBap) -LobbyArgs $LobbyArgs | Out-Null
Get-CimInstance Win32_Process -Filter "Name like 'python%'" | Where-Object { $_.CommandLine -like '*session_listener*' -or $_.CommandLine -like '*127.1.1.128*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Confirm:$false }
Start-Process -FilePath C:\Python314\python.exe -ArgumentList (@('-u','server/bap_server.py','--bind','127.1.1.128','--port','12851') + $AhArgs) -WorkingDirectory (Get-Location) -WindowStyle Hidden -RedirectStandardOutput "D:\Destiny360\SIGNON_LOCAL\logs\$Name-ahl.log" -RedirectStandardError "D:\Destiny360\SIGNON_LOCAL\logs\$Name-ahl.err"
Start-Process -FilePath C:\Python314\python.exe -ArgumentList @('-u','server/session_listener.py','--bind','127.1.1.129','--port','12852') -WorkingDirectory (Get-Location) -WindowStyle Hidden -RedirectStandardOutput "D:\Destiny360\SIGNON_LOCAL\logs\$Name-ahost.log" -RedirectStandardError "D:\Destiny360\SIGNON_LOCAL\logs\$Name-ahost.err"
.\scripts\Invoke-TitanLogin.ps1 -Name $Name
"=== trace"
C:\Python314\python.exe scripts/rpcs3_gdb_daemon.py send hits $Hits $Seconds $Trace $Dumps | ForEach-Object { $_.Substring(0,[Math]::Min(300,$_.Length)) }
"=== lobby"; Get-Content "D:\Destiny360\SIGNON_LOCAL\logs\$Name-lobby.log" | Where-Object { $_ -notmatch 'keepalive' } | Select-Object -Last 6 | ForEach-Object { $_.Substring(0,[Math]::Min(200,$_.Length)) }
"=== bap"; Select-String -Path "D:\Destiny360\SIGNON_LOCAL\logs\$Name-bap.log" -Pattern 'echo|payload=1080|payload=2119|service 70[12]' -NotMatch | Select-Object -Last 14 | ForEach-Object { $_.Line.Substring(0,[Math]::Min(170,$_.Line.Length)) }
"=== screen"; .\scripts\Invoke-RpcsStep.ps1 -WaitSeconds 0

"=== ah listener"; Get-Content "D:\Destiny360\SIGNON_LOCAL\logs\$Name-ahl.log","D:\Destiny360\SIGNON_LOCAL\logs\$Name-ahl.err" | Select-Object -Last 15

"=== activity host listener"; Get-Content "D:\Destiny360\SIGNON_LOCAL\logs\$Name-ahost.log","D:\Destiny360\SIGNON_LOCAL\logs\$Name-ahost.err" | Select-Object -Last 15
