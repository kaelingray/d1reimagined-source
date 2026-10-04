# Shared process ownership and evidence readers for the 01.24 run wrapper.
# Dot-sourcing this file starts no processes and changes no environment variables.

function ConvertTo-D1ProcessArgument {
    param([AllowEmptyString()][string]$Value)
    # Start-Process joins ArgumentList with spaces on Windows. Apply the Windows
    # argv quoting rules so paths with spaces and semicolon option lists survive.
    if ($Value -ne '' -and $Value -notmatch '[\s"]') { return $Value }
    '"' + ([regex]::Replace(([regex]::Replace($Value, '(\\*)"', '$1$1\"')), '(\\+)$', '$1$1')) + '"'
}

function Get-D1ProcessIdentity {
    param(
        [Parameter(Mandatory)][System.Diagnostics.Process]$Process,
        [string]$Tag,
        [string]$Executable
    )
    # Process.Path may be null immediately after Start-Process returns. The
    # caller already knows which executable it launched; persist that explicit
    # path instead of accepting a null identity that can never match cleanup.
    if ($Executable) { $Executable = (Resolve-Path -LiteralPath $Executable -ErrorAction Stop).Path }
    else {
        for ($attempt = 0; $attempt -lt 20 -and -not $Executable; $attempt++) {
            $Process.Refresh()
            $Executable = $Process.Path
            if (-not $Executable) { Start-Sleep -Milliseconds 25 }
        }
    }
    if (-not $Executable) { throw "Could not identify the executable of owned $Tag process $($Process.Id)." }
    $Process.Refresh()
    [pscustomobject]@{
        tag = $Tag
        id = $Process.Id
        startTimeUtcTicks = $Process.StartTime.ToUniversalTime().Ticks.ToString()
        executable = $Executable
    }
}

function Get-D1OwnedProcess {
    param([Parameter(Mandatory)]$Identity)
    if (-not $Identity.executable -or -not $Identity.startTimeUtcTicks) { return }
    $process = Get-Process -Id $Identity.id -ErrorAction SilentlyContinue
    if (-not $process) { return }
    try {
        # PIDs can be reused after the launcher exits. Both identity checks must
        # still match before monitoring, stopping or sending input to a process.
        if ($process.StartTime.ToUniversalTime().Ticks.ToString() -eq $Identity.startTimeUtcTicks -and
            $process.Path -eq $Identity.executable) { return $process }
    } catch { return }
}

function Stop-D1OwnedProcess {
    param([Parameter(Mandatory)]$Identity)
    $process = Get-D1OwnedProcess -Identity $Identity
    if ($process) {
        Stop-Process -InputObject $process -Force -Confirm:$false -ErrorAction Stop
        # Stop-Process can return before the OS releases the log's write handle.
        # Final snapshots must wait for termination, rather than treating a
        # still-closing file as a permanently failed capture.
        if (-not $process.WaitForExit(5000)) {
            throw "Owned $($Identity.tag) process $($Identity.id) did not exit within 5 seconds."
        }
    }
}

function Assert-D1StopNotRequested {
    param([string]$ProcessManifest)
    if ($ProcessManifest -and (Test-Path -LiteralPath "$ProcessManifest.stop")) {
        throw 'Run interrupted: cooperative stop was requested.'
    }
}

function Get-D1LauncherChildren {
    param([Parameter(Mandatory)]$LauncherIdentity)
    # Query only while this exact launcher still owns its PID. Parent PID alone
    # cannot identify children left over from an earlier process with a reused
    # PID; each child's actual start time must follow this launcher as well.
    if (-not (Get-D1OwnedProcess $LauncherIdentity)) { return }
    $children = Get-CimInstance Win32_Process -Filter "ParentProcessId = $([int]$LauncherIdentity.id)" -ErrorAction Stop
    if (-not (Get-D1OwnedProcess $LauncherIdentity)) { return }
    foreach ($child in $children) {
        if ($child.Name -ieq 'rpcn.exe') { continue } # deliberately shared
        $process = Get-Process -Id $child.ProcessId -ErrorAction SilentlyContinue
        if (-not $process) { continue }
        try {
            $identity = Get-D1ProcessIdentity -Process $process -Tag 'launcher-child' -Executable $child.ExecutablePath
            # CIM rounds CreationDate to milliseconds. Compare at that precision
            # to reject a child PID reused between enumeration and inspection.
            $actualMilliseconds = [long]$identity.startTimeUtcTicks - [long]$identity.startTimeUtcTicks % 10000
            $listedTicks = $child.CreationDate.ToUniversalTime().Ticks
            $listedMilliseconds = $listedTicks - $listedTicks % 10000
            if ($actualMilliseconds -eq $listedMilliseconds -and
                [long]$identity.startTimeUtcTicks -ge [long]$LauncherIdentity.startTimeUtcTicks) { $identity }
        } catch { Write-Warning "Could not identify launcher child $($child.ProcessId): $_" }
    }
}

function Read-D1ProcessManifest {
    param([Parameter(Mandatory)][string]$Path)
    if (Test-Path -LiteralPath $Path) {
        Get-Content -LiteralPath $Path -Raw -ErrorAction Stop | ConvertFrom-Json -ErrorAction Stop
    }
}

function Read-D1LaunchOutputTail {
    param([Parameter(Mandatory)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return }
    $stream = $null
    try {
        $sharing = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
        $stream = [IO.FileStream]::new($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, $sharing)
        # A shader/compiler failure can write a very long line. Bound bytes as
        # well as lines so startup diagnostics cannot copy an entire runtime log.
        $length = [int][Math]::Min(8192, $stream.Length)
        [void]$stream.Seek(-$length, [IO.SeekOrigin]::End)
        $buffer = [byte[]]::new($length)
        $read = $stream.Read($buffer, 0, $length)
        $tail = [Text.Encoding]::UTF8.GetString($buffer, 0, $read)
        ($tail -split '\r?\n' | Select-Object -Last 8) -join [Environment]::NewLine
    } catch {
        "Could not read output tail: $($_.Exception.Message)"
    } finally {
        if ($stream) { $stream.Dispose() }
    }
}

function Get-D1LaunchedProcessDiagnostic {
    param(
        [Parameter(Mandatory)][System.Diagnostics.Process]$Process,
        [Parameter(Mandatory)][string]$Tag,
        [string[]]$OutputPaths = @()
    )
    # Only accept the retained Process from Start-Process (or an already verified
    # identity). Do not look up a PID here: it may now belong to another process.
    $hasExited = $null
    $exitCode = $null
    $status = 'process status unavailable'
    try {
        $Process.Refresh()
        $hasExited = $Process.HasExited
        $status = "HasExited=$hasExited"
        if ($hasExited) {
            $exitCode = $Process.ExitCode
            $hex = ([long]$exitCode -band 0xffffffffL).ToString('X8')
            $status += "; ExitCode=$exitCode (0x$hex)"
        }
    } catch { $status += ": $($_.Exception.Message)" }
    $details = @("${Tag}: $status")
    foreach ($path in $OutputPaths) {
        $tail = Read-D1LaunchOutputTail $path
        if ($tail) { $details += "$([IO.Path]::GetFileName($path)):$([Environment]::NewLine)$tail" }
    }
    [pscustomobject]@{
        hasExited = $hasExited
        exitCode = $exitCode
        message = $details -join [Environment]::NewLine
    }
}

function Get-D1CurrentRunLog {
    param([Parameter(Mandatory)]$Identity, [Parameter(Mandatory)][string[]]$Sources)
    if (-not $Identity.startTimeUtcTicks) { throw 'RPCS3 creation time is unavailable; shared log ownership cannot be established.' }
    $startedUtc = [datetime]::new([long]$Identity.startTimeUtcTicks, [DateTimeKind]::Utc)
    foreach ($source in $Sources) {
        if (Test-Path -LiteralPath $source -PathType Leaf) {
            $file = Get-Item -LiteralPath $source -ErrorAction Stop
            if ($file.LastWriteTimeUtc -ge $startedUtc) { return $file.FullName }
        }
    }
    throw "No shared RPCS3 log was written since this emulator started at $($startedUtc.ToString('o')); refusing to capture a previous run's log. Checked $($Sources -join ' or ')."
}

function Copy-D1SharedFileSnapshot {
    param(
        [Parameter(Mandatory)][string]$Source,
        [Parameter(Mandatory)][string]$Destination
    )
    $sourceStream = $null
    $destinationStream = $null
    $temporary = $null
    try {
        $Source = [IO.Path]::GetFullPath($Source)
        $Destination = [IO.Path]::GetFullPath($Destination)
        if ($Source -eq $Destination) { throw 'Source and destination must differ.' }
        if (Test-Path -LiteralPath $Destination -PathType Container) { throw 'Destination is a directory.' }
        # RPCS3 keeps its log open for writing. A reader must also permit that
        # existing write handle and any later log rotation/delete operation.
        $sharing = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
        $sourceStream = [IO.FileStream]::new($Source, [IO.FileMode]::Open, [IO.FileAccess]::Read, $sharing)
        $length = $sourceStream.Length
        $remaining = $length
        $temporary = "$Destination.tmp.$([guid]::NewGuid().ToString('N'))"
        $destinationStream = [IO.FileStream]::new($temporary, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $buffer = [byte[]]::new(65536)
        # Bound the snapshot to the initial length so a busy writer cannot keep
        # the copy running indefinitely. Later appended data belongs to a later
        # snapshot. Publish only after every byte has been copied successfully.
        while ($remaining -gt 0) {
            $count = [int][Math]::Min($buffer.Length, $remaining)
            $read = $sourceStream.Read($buffer, 0, $count)
            if ($read -eq 0) { throw "Source was shortened during the snapshot ($remaining bytes remaining)." }
            $destinationStream.Write($buffer, 0, $read)
            $remaining -= $read
        }
        $destinationStream.Dispose()
        $destinationStream = $null
        $sourceStream.Dispose()
        $sourceStream = $null
        Move-Item -LiteralPath $temporary -Destination $Destination -Force -ErrorAction Stop
        $temporary = $null
        [pscustomobject]@{ source = $Source; destination = $Destination; bytes = $length }
    } catch {
        throw "Could not snapshot '$Source' to '$Destination': $($_.Exception.Message)"
    } finally {
        if ($destinationStream) { $destinationStream.Dispose() }
        if ($sourceStream) { $sourceStream.Dispose() }
        if ($temporary -and (Test-Path -LiteralPath $temporary)) {
            Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
        }
    }
}

function Read-Final124Progress {
    param([Parameter(Mandatory)][string]$LogRoot, [Parameter(Mandatory)][string]$Name)
    function Read-RunLog([string]$Tag) {
        $path = Join-Path $LogRoot "$Name-$Tag.log"
        if (Test-Path -LiteralPath $path) { Get-Content -LiteralPath $path -ErrorAction Stop }
    }
    $stun = @(Read-RunLog 'stun')
    $signon = @(Read-RunLog 'signon')
    $bap = @(Read-RunLog 'bap')
    $dns = @(Read-RunLog 'dns-queries')
    [pscustomobject]@{
        StunProbes = @($stun -cmatch '^\S+ <- [\d.]+:\d+ len=\d+ ').Count
        StunReplies = @($stun -cmatch '^\S+ -> [\d.]+:\d+ len=\d+ ').Count
        DnsQueries = @($dns -cmatch '^\S+ \S+ \S+ -> ').Count
        SignonRequests = @($signon -cmatch '\] HTTP ["'']POST /SignOn(?:\s|\?)').Count
        SignonReplies = @($signon -cmatch '\] replied 200 SignOn success \(').Count
        ManifestReplies = @($signon -cmatch '\] replied 200 content manifest ').Count
        BapConnections = @($bap -cmatch '^\S+ \[[^\]]+\] connection from [\d.]+:\d+').Count
        BapFrames = @($bap -cmatch '^\S+ \[[^\]]+\] frame \d+ \(kind \d+\): ').Count
    }
}

function Get-Final124Verdict {
    param([Parameter(Mandatory)]$Progress)
    if ($Progress.BapFrames -gt 0) { return 'REACHED BAP (protocol frame received)' }
    if ($Progress.BapConnections -gt 0) { return 'BAP TCP CONNECTED (no protocol frame confirmed)' }
    if ($Progress.SignonReplies -gt 0) { return 'REACHED SIGN-ON (success reply sent; no BAP connection)' }
    if ($Progress.SignonRequests -gt 0) { return 'SIGN-ON REQUESTED (no success reply confirmed)' }
    'no SignOn or BAP progress confirmed'
}
