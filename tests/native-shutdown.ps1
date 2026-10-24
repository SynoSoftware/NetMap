#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $TestExecutable,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [ValidateRange(15, 60)][int] $TimeoutSeconds = 25
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$TestExecutable = (Resolve-Path -LiteralPath $TestExecutable).Path
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory.' }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    $isAdministrator = [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
} finally { $identity.Dispose() }
$fixtureProfile = Join-Path $EvidenceDirectory 'NetMap'
New-Item -ItemType Directory -Path $fixtureProfile | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $fixtureProfile 'preferences.txt')
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path

function Require-Event($Events, [string] $Name) {
    $found = @($Events | Where-Object event -EQ $Name)
    if ($found.Count -ne 1) { throw "Expected exactly one shutdown event '$Name'." }
    $found[0]
}

$tests = @(
    @{ Name = 'host::native_shutdown::native_close_cancels_an_outstanding_http_body_read'; Kind = 'http'; Exit = 0 },
    @{ Name = 'host::native_shutdown::native_close_initiates_five_second_fallback_for_uncooperative_work'; Kind = 'uncooperative'; Exit = 1 }
)
$savedData = $env:LOCALAPPDATA
$results = [ordered]@{
    TestExecutable = $TestExecutable
    TestExecutableSHA256 = (Get-FileHash -LiteralPath $TestExecutable -Algorithm SHA256).Hash
    IsAdministrator = $isAdministrator
    Scope = 'Owned normal Host close, registered loopback WinHTTP Request body-read cancellation and callback reclamation; separate artificial uncooperative Work gate and real five-second fallback initiation. Process exit/removal observations are separate from HWND removal and fallback timing. No HTTPS dataset installation, public network, certificates, UAC, global input or kernel exit deadline proof.'
    Checks = @()
    Passed = $false
}
$process = $null
try {
    if ($isAdministrator) { throw 'Native shutdown checks require an ordinary Windows token.' }
    $env:LOCALAPPDATA = $EvidenceDirectory
    for ($index = 0; $index -lt $tests.Count; $index++) {
        $test = $tests[$index]
        $stdout = Join-Path $EvidenceDirectory "$index.stdout.txt"
        $stderr = Join-Path $EvidenceDirectory "$index.stderr.txt"
        $check = [ordered]@{ Name = $test.Name; Kind = $test.Kind; TimedOut = $false; Passed = $false }
        $results.Checks += $check
        $process = Start-Process -FilePath $TestExecutable -ArgumentList @('--ignored', '--exact', $test.Name, '--nocapture', '--test-threads=1') `
            -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
        $check.ProcessId = $process.Id
        $started = $process.StartTime.ToUniversalTime()
        $check.StartedUtc = $started.ToString('o')
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            $check.TimedOut = $true
            if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }
            throw "Native shutdown check timed out: $($test.Name)"
        }
        $check.ExitObservedTickMs = [Environment]::TickCount64
        $check.ExitCode = $process.ExitCode
        $events = @(Get-Content -LiteralPath $stderr | ForEach-Object {
            if ($_ -match 'NETMAP_SHUTDOWN (\{.+\})') { $Matches[1] | ConvertFrom-Json }
        })
        $check.Events = $events
        if ($events.Count -eq 0 -or @($events | Where-Object kind -NE $test.Kind).Count -ne 0) {
            throw 'Missing or mismatched scoped shutdown evidence.'
        }
        if ($check.ExitCode -ne $test.Exit) { throw "Unexpected native test exit: $($check.ExitCode)." }
        $closed = Require-Event $events 'close_requested'
        $removed = Require-Event $events 'window_removed'
        $finish = Require-Event $events 'finish_started'
        if ($closed.window -eq 0 -or $closed.window -ne $removed.window -or
            $removed.tick_ms -lt $closed.tick_ms -or $finish.tick_ms -lt $removed.tick_ms -or
            $finish.allowance_ms -ne 5000) { throw 'Native close/finalization ordering or allowance was incorrect.' }
        if ($test.Kind -eq 'http') {
            $pending = Require-Event $events 'body_read_pending'
            $withheld = Require-Event $events 'server_withholding'
            $cancelled = Require-Event $events 'request_wait_cancelled'
            $dropped = Require-Event $events 'request_drop_returned'
            $reclaimed = Require-Event $events 'callback_context_reclaimed'
            $released = Require-Event $events 'server_released_after_client'
            $client = Require-Event $events 'client_finished'
            $server = Require-Event $events 'server_finished'
            $completed = Require-Event $events 'finish_completed'
            if ($pending.prefix_bytes -ne 65536 -or $pending.no_completion_ms -ne 200 -or $pending.buffer -eq 0 -or
                $withheld.sent_bytes -ne 65536 -or $withheld.declared_bytes -ne 131072 -or
                $pending.tick_ms -gt $closed.tick_ms -or $cancelled.tick_ms -lt $closed.tick_ms -or
                $dropped.tick_ms -lt $cancelled.tick_ms -or $reclaimed.tick_ms -lt $dropped.tick_ms -or
                $released.tick_ms -lt $reclaimed.tick_ms -or -not $client.passed -or -not $server.passed -or
                $completed.tick_ms -lt $client.tick_ms -or $completed.tick_ms -lt $server.tick_ms -or
                @($events | Where-Object event -EQ 'fallback_initiated').Count -ne 0) {
                throw 'Outstanding-read cancellation or cooperative teardown evidence was incomplete.'
            }
            $output = Get-Content -LiteralPath $stdout -Raw
            if ($output -notmatch 'running 1 test' -or $output -notmatch '1 passed; 0 failed') {
                throw 'The exact cooperative shutdown test did not pass.'
            }
        } else {
            $gate = Require-Event $events 'gate_waiting'
            $fallback = Require-Event $events 'fallback_initiated'
            if ($gate.maximum_ms -ne 20000 -or $gate.tick_ms -gt $closed.tick_ms -or
                $fallback.tick_ms -lt ($finish.tick_ms + 4990) -or $fallback.elapsed_ms -lt 4990 -or
                @($events | Where-Object event -In @('gate_returned', 'finish_completed')).Count -ne 0) {
                throw 'The artificial gate did not reach the real five-second fallback.'
            }
            $check.FallbackDeadlineLatenessUs = $fallback.deadline_lateness_us
            $check.FallbackToExitObservedMs = $check.ExitObservedTickMs - $fallback.tick_ms
        }

        # WaitForExit proves termination; separately observe that this exact
        # PID/start-time instance is absent from process enumeration. This is not
        # a claim about destruction of every outstanding kernel process handle.
        $ownedId = $process.Id
        $process.Dispose(); $process = $null
        $removalDeadline = [DateTime]::UtcNow.AddSeconds(5)
        do {
            $candidate = Get-Process -Id $ownedId -ErrorAction SilentlyContinue
            $sameInstance = $false
            if ($null -ne $candidate) {
                try { $sameInstance = $candidate.StartTime.ToUniversalTime() -eq $started }
                finally { $candidate.Dispose() }
            }
            if (-not $sameInstance) { break }
            Start-Sleep -Milliseconds 20
        } while ([DateTime]::UtcNow -lt $removalDeadline)
        $check.OwnInstanceAbsent = -not $sameInstance
        $check.RemovalObservedTickMs = [Environment]::TickCount64
        if ($sameInstance) { throw 'The exited owned process remained enumerated beyond the observation deadline.' }
        $check.Passed = $true
    }
    $results.Passed = $true
} catch {
    $results.Error = $_.Exception.Message
    throw
} finally {
    if ($null -ne $process) {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }
        $process.Dispose()
    }
    $env:LOCALAPPDATA = $savedData
    $results | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json') -Encoding utf8
}
