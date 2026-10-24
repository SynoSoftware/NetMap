#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $TestExecutable,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [switch] $ElevateCollector,
    [switch] $Supervisor,
    [ValidateRange(120, 300)][int] $TimeoutSeconds = 180
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$TestExecutable = (Resolve-Path -LiteralPath $TestExecutable).Path
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class BrokerLogonToken {
    [StructLayout(LayoutKind.Sequential)] struct TokenGroups { public uint Count; public IntPtr Sid; public uint Attributes; }
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool GetTokenInformation(SafeAccessTokenHandle token, int kind, IntPtr buffer, uint capacity, out uint length);
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool ConvertSidToStringSidW(IntPtr sid, out IntPtr text);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr allocation);
    public static string Read(SafeAccessTokenHandle token) {
        // TokenLogonSid returns TOKEN_GROUPS with exactly one logon SID. Unlike
        // WindowsIdentity.Groups, this query does not filter that token group.
        uint length;
        if (GetTokenInformation(token, 28, IntPtr.Zero, 0, out length) || Marshal.GetLastWin32Error() != 122)
            throw new InvalidOperationException("TokenLogonSid size query did not return ERROR_INSUFFICIENT_BUFFER");
        if (length < Marshal.SizeOf<TokenGroups>() || length > 65536)
            throw new InvalidOperationException("TokenLogonSid buffer length is outside its bound");
        uint capacity = length;
        var buffer = Marshal.AllocHGlobal((int)capacity);
        try {
            if (!GetTokenInformation(token, 28, buffer, capacity, out length)) throw new Win32Exception(Marshal.GetLastWin32Error());
            if (length < Marshal.SizeOf<TokenGroups>() || length > capacity) throw new InvalidOperationException("TokenLogonSid returned an invalid length");
            var groups = Marshal.PtrToStructure<TokenGroups>(buffer);
            if (groups.Count != 1 || groups.Sid == IntPtr.Zero) throw new InvalidOperationException("One current logon SID is required");
            IntPtr text;
            if (!ConvertSidToStringSidW(groups.Sid, out text)) throw new Win32Exception(Marshal.GetLastWin32Error());
            try { return Marshal.PtrToStringUni(text) ?? throw new InvalidOperationException("Logon SID conversion returned no text"); }
            finally { LocalFree(text); }
        } finally { Marshal.FreeHGlobal(buffer); }
    }
}
'@
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    $administrator = [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    # Identity owns this SafeAccessTokenHandle through both synchronous native
    # calls and SID conversion. Each allocation is freed by its native owner.
    $logonSid = [BrokerLogonToken]::Read($identity.AccessToken)
} finally { $identity.Dispose() }

function Save-Json($Value, [string] $Path) {
    $temporary = $Path + '.writing'
    $Value | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $temporary -Encoding utf8
    [IO.File]::Move($temporary, $Path, $true)
}
function Wait-Until([scriptblock] $Condition, [string] $Reason, [int] $Seconds = 20) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    do {
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw $Reason
}
function Read-Json([string] $Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return $null }
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}
function Read-TraceNames([string] $Label) {
    $queryOut = Join-Path $EvidenceDirectory "$Label.stdout.tmp"
    $queryErr = Join-Path $EvidenceDirectory "$Label.stderr.tmp"
    $query = $null
    try {
        $query = Start-Process -FilePath "$env:SystemRoot/System32/logman.exe" -ArgumentList @('query', '-ets') -WindowStyle Hidden -PassThru -RedirectStandardOutput $queryOut -RedirectStandardError $queryErr
        if (-not $query.WaitForExit(5000)) { $query.Kill(); throw 'Bounded trace inventory timed out.' }
        if ($query.ExitCode -ne 0) { throw 'Trace inventory failed; no unknown session will be stopped.' }
        [regex]::Matches((Get-Content -LiteralPath $queryOut -Raw), '(?m)^\s*(NetMap-\d+-\{?[0-9a-fA-F-]{36}\}?)\s+') | ForEach-Object { $_.Groups[1].Value }
    } finally {
        if ($null -ne $query) { $query.Dispose() }
        Remove-Item -LiteralPath $queryOut, $queryErr -ErrorAction SilentlyContinue
    }
}

if ($Supervisor) {
    if (-not $administrator) { throw 'The collector supervisor requires the explicitly approved elevated token.' }
    $EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
    $inputState = Read-Json (Join-Path $EvidenceDirectory 'input.json')
    if ($null -eq $inputState -or $inputState.LogonSid -ne $logonSid -or
        $inputState.Executable -cne $Executable -or
        (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash -ne $inputState.ExecutableSHA256 -or
        (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash -ne $inputState.ScriptSHA256) {
        throw 'Supervisor inputs, same-logon identity or reviewed artifact hashes changed.'
    }
    $previousProfile = $env:LOCALAPPDATA
    $owned = $null
    $records = @()
    $result = [ordered]@{ Passed = $false; IsAdministrator = $administrator; LogonSid = $logonSid; Collectors = @() }
    try {
        $env:LOCALAPPDATA = $inputState.Profile
        Save-Json ([ordered]@{ ProcessId = $PID; LogonSid = $logonSid; Elevated = $true }) (Join-Path $EvidenceDirectory 'supervisor-ready.json')
        foreach ($number in 1, 2) {
            Wait-Until { (Test-Path -LiteralPath (Join-Path $EvidenceDirectory "start-$number")) -or (Test-Path -LiteralPath (Join-Path $EvidenceDirectory 'stop')) } 'Collector start request did not arrive.' $TimeoutSeconds
            if (Test-Path -LiteralPath (Join-Path $EvidenceDirectory 'stop')) { break }
            # Capture before launch: PID reuse must never make an orphaned
            # session from an earlier process look like this collector's own.
            $baselineTraces = @(Read-TraceNames "trace-baseline-$number")
            $owned = Start-Process -FilePath $Executable -ArgumentList '--collector' -WindowStyle Hidden -PassThru `
                -RedirectStandardOutput (Join-Path $EvidenceDirectory "collector-$number.stdout.txt") `
                -RedirectStandardError (Join-Path $EvidenceDirectory "collector-$number.stderr.txt")
            $ownedPattern = '^NetMap-' + $owned.Id + '-\{?[0-9a-fA-F-]{36}\}?$'
            $record = [ordered]@{ Number = $number; ProcessId = $owned.Id; StartedUtc = $owned.StartTime.ToUniversalTime().ToString('o'); Exited = $false; NaturalExit = $false; OwnedTraceNames = @(); ExcludedPreexistingTraceNames = @($baselineTraces | Where-Object { $_ -match $ownedPattern }) }
            $records += $record
            Save-Json $record (Join-Path $EvidenceDirectory "collector-$number.json")
            $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
            while (-not $owned.HasExited) {
                if ((Test-Path -LiteralPath (Join-Path $EvidenceDirectory 'stop')) -or [DateTime]::UtcNow -ge $deadline) { break }
                Start-Sleep -Milliseconds 100
            }
            # Success always relies on the production subscriber grace. Failure
            # first closes the ordinary clients, allowing the same natural exit.
            if (-not $owned.HasExited) { $owned.WaitForExit(20000) | Out-Null }
            if (-not $owned.HasExited) {
                $record.CleanupFallback = $true
                # Stop only newly observed PID/GUID names absent before launch.
                # Baseline exclusions also protect sessions from recycled PIDs.
                try {
                    $record.OwnedTraceNames = @(Read-TraceNames "trace-cleanup-$number" | Where-Object { $_ -match $ownedPattern -and $baselineTraces -notcontains $_ })
                    foreach ($trace in $record.OwnedTraceNames) {
                        $cleanup = Start-Process -FilePath "$env:SystemRoot/System32/logman.exe" -ArgumentList @('stop', $trace, '-ets') -WindowStyle Hidden -PassThru
                        try {
                            if (-not $cleanup.WaitForExit(5000)) { $cleanup.Kill(); throw 'Owned trace cleanup timed out.' }
                            if ($cleanup.ExitCode -ne 0) { throw 'Owned trace cleanup failed.' }
                        } finally { $cleanup.Dispose() }
                    }
                } catch {
                    $record.TraceCleanupError = $_.Exception.Message
                    throw
                }
                if (-not $owned.WaitForExit(5000)) { $owned.Kill(); $owned.WaitForExit(5000) | Out-Null }
            } else { $record.NaturalExit = $true }
            $record.Exited = $owned.HasExited
            $record.ExitCode = if ($owned.HasExited) { $owned.ExitCode } else { $null }
            $record.ExitedUtc = [DateTime]::UtcNow.ToString('o')
            Save-Json $record (Join-Path $EvidenceDirectory "collector-$number.json")
            $owned.Dispose()
            $owned = $null
            if (-not $record.NaturalExit -or $record.ExitCode -ne 0) { throw 'Production collector did not exit cleanly through subscriber grace.' }
        }
        $result.Passed = $records.Count -eq 2
    } catch {
        $result.Error = $_.Exception.Message
        $result.ScriptStackTrace = $_.ScriptStackTrace
        $result.InvocationInfo = [ordered]@{ ScriptName = $_.InvocationInfo.ScriptName; ScriptLineNumber = $_.InvocationInfo.ScriptLineNumber; OffsetInLine = $_.InvocationInfo.OffsetInLine; Line = $_.InvocationInfo.Line; PositionMessage = $_.InvocationInfo.PositionMessage }
    } finally {
        $env:LOCALAPPDATA = $previousProfile
        if ($null -ne $owned) {
            # Trace inventory/stop failures must not bypass process retirement.
            # This descriptor still identifies exactly the elevated child we
            # created; no name lookup or unknown trace is used for termination.
            $record.NaturalExit = $false
            $record.CleanupFallback = $true
            try {
                if (-not $owned.HasExited) {
                    $owned.Kill()
                    if (-not $owned.WaitForExit(5000)) { throw 'Exact owned collector did not exit after termination.' }
                }
            } catch {
                $record.ProcessCleanupError = $_.Exception.Message
            } finally {
                $record.Exited = $owned.HasExited
                $record.ExitCode = if ($owned.HasExited) { $owned.ExitCode } else { $null }
                $record.ExitedUtc = [DateTime]::UtcNow.ToString('o')
                $result.CleanupCollectorStillRunning = -not $owned.HasExited
                $owned.Dispose()
                Save-Json $record (Join-Path $EvidenceDirectory "collector-$number.json")
            }
        }
        $result.Collectors = $records
        Save-Json $result (Join-Path $EvidenceDirectory 'supervisor-result.json')
    }
    if (-not $result.Passed) { exit 1 }
    exit 0
}

if ($administrator) { throw 'Run the viewer wrapper with an ordinary token; only its explicitly approved supervisor elevates.' }
if (-not $ElevateCollector) { throw 'The reviewed broker journey requires -ElevateCollector for its one UAC supervisor launch.' }
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory.' }
New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$private = Join-Path $EvidenceDirectory 'product'
$profile = Join-Path $EvidenceDirectory 'profile'
New-Item -ItemType Directory -Path $private, (Join-Path $profile 'NetMap') | Out-Null
$sourceExecutable = $Executable
$Executable = Join-Path $private 'netmap.exe'
Copy-Item -LiteralPath $sourceExecutable -Destination $Executable
$saverPath = Join-Path $private 'netmap.scr'
Copy-Item -LiteralPath $sourceExecutable -Destination $saverPath
Copy-Item -LiteralPath (Join-Path (Split-Path -Parent $sourceExecutable) 'assets') -Destination (Join-Path $private 'assets') -Recurse
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $profile 'NetMap/preferences.txt')
$prefix = 'Local\NetMap-Broker-' + [guid]::NewGuid().ToString('D')
$events = @{}
foreach ($name in 'pause', 'resume', 'finish') {
    $events[$name] = [Threading.EventWaitHandle]::new($false, [Threading.EventResetMode]::ManualReset, "$prefix-$name")
}
$stdout = Join-Path $EvidenceDirectory 'viewer.stdout.txt'
$stderr = Join-Path $EvidenceDirectory 'viewer.stderr.txt'
$test = 'host::native_broker::native_elevated_broker_sources_and_viewer_rows'
$result = [ordered]@{
    Passed = $false; Test = $test; IsAdministrator = $administrator; LogonSid = $logonSid
    Executable = $Executable; SourceExecutable = $sourceExecutable
    ExecutableSHA256 = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash
    TestExecutable = $TestExecutable; TestExecutableSHA256 = (Get-FileHash -LiteralPath $TestExecutable -Algorithm SHA256).Hash
    ScriptSHA256 = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash
    Scope = 'Ordinary native Application/Work, actual authenticated read-only broker client and production elevated collectors; controlled measured loopback to real virtual ListView, exact old selection cleared by source replacement, real ordinary production window and full-screen saver subscriptions, saver-only survival and natural last-client grace. No wrong-logon/remote-peer rejection, installed-saver activation, public HTTP, physical input, Explorer mutation or byte-conservation claim. Raw/Collector regression remains its own exact check.'
    Phases = @(); Sequences = @(); TimedOut = $false
}
Save-Json ([ordered]@{ Executable = $Executable; ExecutableSHA256 = $result.ExecutableSHA256; ScriptSHA256 = $result.ScriptSHA256; Profile = $profile; LogonSid = $logonSid }) (Join-Path $EvidenceDirectory 'input.json')

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class BrokerWindows {
    public delegate bool Callback(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr data);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] static extern bool PostMessageW(IntPtr window, uint message, UIntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr window, int command);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] static extern IntPtr GetDlgItem(IntPtr parent, int id);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "SendMessageTimeoutW")] static extern IntPtr ReadText(IntPtr window, uint message, IntPtr capacity, System.Text.StringBuilder text, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassNameW(IntPtr window, System.Text.StringBuilder text, int count);
    public static string Class(IntPtr window) { var text = new System.Text.StringBuilder(128); GetClassNameW(window, text, text.Capacity); return text.ToString(); }
    public static string Status(IntPtr parent, uint pid) {
        uint owner;
        GetWindowThreadProcessId(parent, out owner);
        if (owner != pid) throw new InvalidOperationException("Retired ordinary window");
        var window = GetDlgItem(parent, 1001);
        GetWindowThreadProcessId(window, out owner);
        var text = new System.Text.StringBuilder(4096);
        // WM_GETTEXT is a system-marshalled message, not a private pointer
        // payload. Both the parent and status HWND must belong to this PID.
        if (window == IntPtr.Zero || owner != pid || ReadText(window, 13, (IntPtr)text.Capacity, text, 2, 2000, out _) == IntPtr.Zero)
            throw new InvalidOperationException("Owned status read failed or timed out");
        return text.ToString();
    }
    public static IntPtr[] Windows(uint pid) {
        var found = new System.Collections.Generic.List<IntPtr>();
        EnumWindows((window, data) => { uint owner; GetWindowThreadProcessId(window, out owner); if (owner == pid) found.Add(window); return true; }, IntPtr.Zero);
        return found.ToArray();
    }
    public static void Close(uint pid) {
        foreach (var window in Windows(pid)) { uint owner; GetWindowThreadProcessId(window, out owner); if (owner == pid) PostMessageW(window, 0x10, UIntPtr.Zero, IntPtr.Zero); }
    }
}
'@
function Records([string] $Marker) {
    if (-not (Test-Path -LiteralPath $stdout)) { return }
    foreach ($line in Get-Content -LiteralPath $stdout) {
        if ($line -match ([regex]::Escape($Marker) + ' (\{.+\})$')) { $Matches[1] | ConvertFrom-Json }
    }
}
function Wait-Phase([string] $Name, [int] $Seconds = 30) {
    Wait-Until {
        if ($viewer.HasExited) { throw "Ordinary native fixture exited before $Name." }
        @((Records 'NETMAP_BROKER_CASE') | Where-Object phase -eq $Name).Count -eq 1
    } "Native broker phase $Name did not arrive." $Seconds
}
function Close-Owned($Process) {
    if ($null -eq $Process -or $Process.HasExited) { return }
    [BrokerWindows]::Close([uint32]$Process.Id)
    if (-not $Process.WaitForExit(6000)) {
        $Process.Kill()
        if (-not $Process.WaitForExit(5000)) { throw 'Owned ordinary subscriber did not exit.' }
    }
}
function Collector([int] $Number) { Read-Json (Join-Path $EvidenceDirectory "collector-$Number.json") }
function Wait-Collector([int] $Number, [bool] $Exited, [int] $Seconds = 25) {
    Wait-Until {
        $state = Collector $Number
        $null -ne $state -and $state.Exited -eq $Exited
    } "Owned collector $Number did not reach exited=$Exited." $Seconds
}

$supervisorProcess = $null
$viewer = $null
$window = $null
$saver = $null
$previousProfile = $env:LOCALAPPDATA
$previousEvents = $env:NETMAP_BROKER_EVENTS
try {
    $shell = (Get-Process -Id $PID).Path
    # The caller explicitly opts in to this one reviewed UAC launch. The
    # supervisor's children inherit only the isolated profile it sets itself.
    $arguments = @('-NoProfile', '-File', ('"' + $PSCommandPath + '"'), '-Supervisor', '-Executable', ('"' + $Executable + '"'), '-TestExecutable', ('"' + $TestExecutable + '"'), '-EvidenceDirectory', ('"' + $EvidenceDirectory + '"'), '-TimeoutSeconds', $TimeoutSeconds)
    $supervisorProcess = Start-Process -FilePath $shell -ArgumentList $arguments -Verb RunAs -WindowStyle Hidden -PassThru
    Wait-Until { Test-Path -LiteralPath (Join-Path $EvidenceDirectory 'supervisor-ready.json') } 'Approved elevated supervisor did not start.' 45
    New-Item -ItemType File -Path (Join-Path $EvidenceDirectory 'start-1') | Out-Null
    Wait-Collector 1 $false
    $env:LOCALAPPDATA = $profile
    $env:NETMAP_BROKER_EVENTS = $prefix
    $window = Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru
    $viewer = Start-Process -FilePath $TestExecutable -ArgumentList @('--ignored', '--exact', $test, '--nocapture', '--test-threads=1') -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $result.ViewerProcessId = $viewer.Id
    $result.WindowProcessId = $window.Id
    Wait-Until {
        if ($window.HasExited) { throw 'The actual ordinary product subscriber exited during startup.' }
        $ownedWindows = @([BrokerWindows]::Windows([uint32]$window.Id) | Where-Object { [BrokerWindows]::Class($_) -eq 'NetMap.Window' })
        foreach ($hwnd in $ownedWindows) { [BrokerWindows]::ShowWindowAsync($hwnd, 4) | Out-Null }
        @($ownedWindows | Where-Object { [BrokerWindows]::IsWindowVisible($_) }).Count -gt 0
    } 'The owned product window did not become ready.'
    Wait-Phase 'measured'
    $releaseWindow = @([BrokerWindows]::Windows([uint32]$window.Id) | Where-Object { [BrokerWindows]::Class($_) -eq 'NetMap.Window' })[0]
    Wait-Until {
        if ($window.HasExited) { throw 'Ordinary release window exited before shared capture readiness.' }
        $status = [BrokerWindows]::Status($releaseWindow, [uint32]$window.Id)
        if ($status -match 'Capture complete' -and $status -match ';\s*[1-9]\d* transferring with measured traffic') {
            $result.ReleaseWindowStatus = $status
            $result.ReleaseWindowStatusUtc = [DateTime]::UtcNow.ToString('o')
            return $true
        }
        return $false
    } 'Ordinary release window did not show complete capture with measured transfers while controlled traffic was live.' 15
    # This probes OS access only. Snapshot/authentication decoding remains in
    # the production Rust Client used above; no PowerShell wire model exists.
    $writeProbe = [IO.Pipes.NamedPipeClientStream]::new('.', "NetMap-$logonSid-v2", [IO.Pipes.PipeDirection]::Out)
    try {
        $denied = $false
        try { $writeProbe.Connect(1000) } catch [UnauthorizedAccessException] { $denied = $true }
        if (-not $denied) { throw 'Ordinary GENERIC_WRITE pipe access was not rejected.' }
        $result.OrdinaryWriteRejected = $true
    } finally { $writeProbe.Dispose() }
    # Start the attended full-screen saver after the ordinary windows settle:
    # their startup activation must not dismiss it. Hidden STARTUPINFO would
    # also override the saver's first ShowWindow on its primary surface.
    $saver = Start-Process -FilePath $saverPath -ArgumentList '/s' -WindowStyle Normal -PassThru -RedirectStandardOutput (Join-Path $EvidenceDirectory 'saver.stdout.txt') -RedirectStandardError (Join-Path $EvidenceDirectory 'saver.stderr.txt')
    $result.SaverProcessId = $saver.Id
    Wait-Until {
        if ($saver.HasExited) { throw 'The actual saver exited during startup; see saver.stderr.txt.' }
        @([BrokerWindows]::Windows([uint32]$saver.Id) | Where-Object { [BrokerWindows]::Class($_) -eq 'NetMap.ScreenSaver' -and [BrokerWindows]::IsWindowVisible($_) }).Count -gt 0
    } 'The owned full-screen saver did not become visible.'
    $events.pause.Set() | Out-Null
    Wait-Phase 'paused'
    Close-Owned $window
    $result.WindowExitCode = $window.ExitCode
    if ($window.ExitCode -ne 0) { throw 'Ordinary product window did not exit cleanly.' }
    $alone = [DateTime]::UtcNow
    $survival = $alone.AddSeconds(11)
    do {
        if ($saver.HasExited) { throw 'The actual saver was dismissed during its subscriber-only phase; see saver.stderr.txt.' }
        if ((Collector 1).Exited) { throw 'The actual collector exited while its saver subscriber remained alive.' }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $survival)
    $result.SaverOnlyAliveMs = [int]([DateTime]::UtcNow - $alone).TotalMilliseconds
    $closeRequested = [DateTime]::UtcNow
    $result.LastSubscriberCloseRequest1Utc = $closeRequested.ToString('o')
    Close-Owned $saver
    $result.SaverExitCode = $saver.ExitCode
    if ($saver.ExitCode -ne 0) { throw 'Actual saver subscriber did not exit cleanly.' }
    $lastClosed = [DateTime]::UtcNow
    $result.LastSubscriberExit1Utc = $lastClosed.ToString('o')
    Wait-Collector 1 $true
    $result.LastSubscriberExit1Ms = [int]([DateTime]::UtcNow - $lastClosed).TotalMilliseconds
    $result.CloseRequestToCollectorExit1Ms = [int]([DateTime]::UtcNow - $closeRequested).TotalMilliseconds
    if ($result.LastSubscriberExit1Ms -lt 5000) { throw 'Collector1 exited before its final-subscriber grace could be established.' }
    $first = Collector 1
    if (-not $first.NaturalExit -or $first.ExitCode -ne 0) { throw 'Collector1 required fallback or failed.' }
    New-Item -ItemType File -Path (Join-Path $EvidenceDirectory 'start-2') | Out-Null
    Wait-Collector 2 $false
    $events.resume.Set() | Out-Null
    Wait-Phase 'reconnected'
    $closeRequested = [DateTime]::UtcNow
    $result.LastSubscriberCloseRequest2Utc = $closeRequested.ToString('o')
    $events.finish.Set() | Out-Null
    if (-not $viewer.WaitForExit(15000)) { $result.TimedOut = $true; throw 'Native source fixture did not finish.' }
    $result.ViewerExitCode = $viewer.ExitCode
    if ($viewer.ExitCode -ne 0) { throw 'Native source fixture failed; preserve its raw records.' }
    $lastClosed = [DateTime]::UtcNow
    $result.LastSubscriberExit2Utc = $lastClosed.ToString('o')
    Wait-Collector 2 $true
    $result.LastSubscriberExit2Ms = [int]([DateTime]::UtcNow - $lastClosed).TotalMilliseconds
    $result.CloseRequestToCollectorExit2Ms = [int]([DateTime]::UtcNow - $closeRequested).TotalMilliseconds
    if ($result.LastSubscriberExit2Ms -lt 5000) { throw 'Collector2 exited before its final-subscriber grace could be established.' }
    if (-not $supervisorProcess.WaitForExit(10000)) { throw 'Elevated supervisor did not exit after both natural collector exits.' }
    $supervision = Read-Json (Join-Path $EvidenceDirectory 'supervisor-result.json')
    if ($null -eq $supervision -or -not $supervision.Passed -or $supervisorProcess.ExitCode -ne 0) { throw 'Elevated production lifecycle did not pass.' }
    $phases = @(Records 'NETMAP_BROKER_CASE')
    $sequences = @(Records 'NETMAP_BROKER_SEQUENCE')
    if (($phases.phase -join ',') -cne 'measured,paused,reconnected,finished' -or
        $phases[0].epoch -ceq $phases[2].epoch -or $phases[2].selection_cleared -ne $true -or
        $phases[0].rx -le 0 -or $phases[0].tx -le 0 -or $phases[0].native_cells -ne 9 -or
        $sequences.Count -ne 2 -or @($sequences | Where-Object { $_.first -lt 1 -or $_.last -lt $_.first -or $_.count -lt 1 }).Count -ne 0) {
        throw 'Broker typed source, native measured cells or sequence records are incomplete.'
    }
    $result.Passed = $true
} catch {
    $result.Error = $_.Exception.Message
    $result.ScriptStackTrace = $_.ScriptStackTrace
    $result.InvocationInfo = [ordered]@{ ScriptName = $_.InvocationInfo.ScriptName; ScriptLineNumber = $_.InvocationInfo.ScriptLineNumber; OffsetInLine = $_.InvocationInfo.OffsetInLine; Line = $_.InvocationInfo.Line; PositionMessage = $_.InvocationInfo.PositionMessage }
} finally {
    if ($null -ne $viewer -and $viewer.HasExited) { $result.ViewerExitCode = $viewer.ExitCode }
    # Retire ordinary subscriptions before requesting supervisor cleanup. Their
    # process descriptors are exact children, never name-based global matches.
    foreach ($process in @($viewer, $window, $saver)) {
        try { Close-Owned $process } catch { $result.Passed = $false; $result.CleanupError = $_.Exception.Message }
    }
    New-Item -ItemType File -Path (Join-Path $EvidenceDirectory 'stop') -Force | Out-Null
    if ($null -ne $supervisorProcess) {
        $result.SupervisorExitObserved = $supervisorProcess.WaitForExit(45000)
        if (-not $result.SupervisorExitObserved) { $result.Passed = $false; $result.CleanupSupervisorIncomplete = $true }
        $supervisorProcess.Dispose()
    }
    foreach ($process in @($viewer, $window, $saver)) { if ($null -ne $process) { $process.Dispose() } }
    foreach ($event in $events.Values) { $event.Dispose() }
    $env:LOCALAPPDATA = $previousProfile
    $env:NETMAP_BROKER_EVENTS = $previousEvents
    $result.Phases = @(Records 'NETMAP_BROKER_CASE')
    $result.Sequences = @(Records 'NETMAP_BROKER_SEQUENCE')
    $result.Collectors = @((Collector 1), (Collector 2))
    Save-Json $result (Join-Path $EvidenceDirectory 'result.json')
}
if (-not $result.Passed) { throw 'Broker acceptance incomplete; see result.json and preserved raw output.' }
$result | ConvertTo-Json -Depth 10
