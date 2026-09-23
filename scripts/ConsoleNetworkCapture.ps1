param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Start', 'Stop')]
    [string] $Mode
)

$ErrorActionPreference = 'Stop'
$upstreamName = 'Ethernet 2'
$consoleName = 'Ethernet'
$captureDir = 'D:\Destiny360\LOGS\console-network-capture'
$statePath = Join-Path $captureDir 'state.json'
$statusPath = Join-Path $captureDir 'status.txt'
$etlPath = Join-Path $captureDir 'console-network.etl'
$pcapPath = Join-Path $captureDir 'console-network.pcapng'
$captureStarted = $false
$sharingChanged = $false

New-Item -ItemType Directory -Path $captureDir -Force | Out-Null

function Write-Status([string] $message) {
    $line = "$(Get-Date -Format o) $message"
    Add-Content -LiteralPath $statusPath -Value $line
}

function Get-SharingConnection([object] $manager, [string] $name) {
    foreach ($connection in $manager.EnumEveryConnection()) {
        $properties = $manager.NetConnectionProps($connection)
        if ($properties.Name -eq $name) {
            return $connection
        }
    }
    throw "Network connection '$name' was not found."
}

try {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Administrator rights are required.'
    }

    $upstream = Get-NetAdapter -Name $upstreamName
    $console = Get-NetAdapter -Name $consoleName
    if ($upstream.Status -ne 'Up' -or $console.Status -ne 'Up') {
        throw "Both network ports must be connected. $upstreamName=$($upstream.Status), $consoleName=$($console.Status)."
    }
    $sharing = New-Object -ComObject HNetCfg.HNetShare
    $upstreamConnection = Get-SharingConnection $sharing $upstreamName
    $consoleConnection = Get-SharingConnection $sharing $consoleName
    $upstreamConfig = $sharing.INetSharingConfigurationForINetConnection($upstreamConnection)
    $consoleConfig = $sharing.INetSharingConfigurationForINetConnection($consoleConnection)

    if ($Mode -eq 'Start') {
        if (Test-Path -LiteralPath $statePath) {
            throw "A capture state file already exists at $statePath. Stop that capture before starting another."
        }
        foreach ($connection in $sharing.EnumEveryConnection()) {
            $properties = $sharing.NetConnectionProps($connection)
            $configuration = $sharing.INetSharingConfigurationForINetConnection($connection)
            if ($configuration.SharingEnabled) {
                throw "Internet Connection Sharing is already enabled on '$($properties.Name)'; leaving the existing setup untouched."
            }
        }

        # Preserve the relevant original settings before changing the network.
        [pscustomobject]@{
            Upstream = $upstreamName
            Console = $consoleName
            UpstreamSharingWasEnabled = [bool] $upstreamConfig.SharingEnabled
            ConsoleSharingWasEnabled = [bool] $consoleConfig.SharingEnabled
            StartedAt = (Get-Date -Format o)
        } | ConvertTo-Json | Set-Content -LiteralPath $statePath -Encoding UTF8

        & pktmon filter remove | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Could not clear old packet filters.' }
        & pktmon filter add ConsolePort -m $console.MacAddress | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Could not filter the packet capture to the console port.' }
        & pktmon start --capture --comp nics --pkt-size 128 --file-name $etlPath --file-size 64 --log-mode circular | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Could not start the packet capture.' }
        $captureStarted = $true
        Write-Status 'Packet monitor started for the console port.'

        # Public = connected router port; private = direct cable to Xbox.
        $sharingChanged = $true
        $upstreamConfig.EnableSharing(0)
        $consoleConfig.EnableSharing(1)
        Start-Sleep -Seconds 3
        $consoleAddress = @(Get-NetIPAddress -InterfaceAlias $consoleName -AddressFamily IPv4 -ErrorAction SilentlyContinue |
            Where-Object { $_.IPAddress -eq '192.168.137.1' })
        if ($consoleAddress.Count -eq 0) {
            throw 'Windows sharing did not assign 192.168.137.1 to the console port.'
        }
        Write-Status 'READY: Internet sharing is active and packet capture is running.'
    } else {
        if (-not (Test-Path -LiteralPath $statePath)) {
            throw "No capture state file exists at $statePath; sharing settings were not changed."
        }
        $state = Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json
        if ($state.Upstream -ne $upstreamName -or $state.Console -ne $consoleName) {
            throw 'Capture state names do not match the current network connections.'
        }
        & pktmon stop | Out-Null
        Write-Status 'Packet monitor stopped.'
        if (-not $state.ConsoleSharingWasEnabled -and $consoleConfig.SharingEnabled) {
            $consoleConfig.DisableSharing()
        }
        if (-not $state.UpstreamSharingWasEnabled -and $upstreamConfig.SharingEnabled) {
            $upstreamConfig.DisableSharing()
        }
        Write-Status 'Original Internet sharing state restored.'
        if (Test-Path -LiteralPath $etlPath) {
            & pktmon etl2pcap $etlPath --out $pcapPath | Out-Null
            if ($LASTEXITCODE -ne 0) { throw 'Capture stopped, but pcapng conversion failed.' }
            Write-Status "DONE: Trace saved to $pcapPath"
        }
        Rename-Item -LiteralPath $statePath -NewName "state-completed-$(Get-Date -Format yyyyMMdd-HHmmss).json"
    }
} catch {
    Write-Status "ERROR: $($_.Exception.Message)"
    if ($Mode -eq 'Start') {
        if ($captureStarted) {
            & pktmon stop 2>$null | Out-Null
        }
        if ($sharingChanged) {
            try {
                if ($consoleConfig.SharingEnabled) { $consoleConfig.DisableSharing() }
                if ($upstreamConfig.SharingEnabled) { $upstreamConfig.DisableSharing() }
            } catch {
                Write-Status "CLEANUP ERROR: $($_.Exception.Message)"
            }
        }
        if (Test-Path -LiteralPath $statePath) {
            Rename-Item -LiteralPath $statePath -NewName "state-failed-$(Get-Date -Format yyyyMMdd-HHmmss).json"
        }
        Write-Status 'Start failure cleanup attempted where changes were made.'
    }
    exit 1
}
