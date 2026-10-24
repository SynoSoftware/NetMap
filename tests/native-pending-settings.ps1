#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $TestExecutable,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [ValidateRange(15, 60)][int] $TimeoutSeconds = 25,
    [ValidateSet('Reopen', 'CommitOnExit', 'FallbackOnExit', 'Exit')][string] $Mode = 'Reopen'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$TestExecutable = (Resolve-Path -LiteralPath $TestExecutable).Path
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory.' }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    $isAdministrator = [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
} finally { $identity.Dispose() }
if ($Mode -eq 'Exit') {
    New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
    $EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
    $batch = [ordered]@{ Mode = 'Exit'; IsAdministrator = $isAdministrator; Checks = @(); Passed = $false }
    try {
        foreach ($exitMode in @('CommitOnExit', 'FallbackOnExit')) {
            $directory = Join-Path $EvidenceDirectory $exitMode
            & $PSCommandPath -TestExecutable $TestExecutable -EvidenceDirectory $directory -TimeoutSeconds $TimeoutSeconds -Mode $exitMode
            $batch.Checks += (Get-Content -LiteralPath (Join-Path $directory 'result.json') -Raw | ConvertFrom-Json)
        }
        $batch.Passed = $true
    } catch {
        $batch.Error = $_.Exception.Message
        throw
    } finally {
        $batch | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json') -Encoding utf8
    }
    return
}

function Require-Event($Events, [string] $Name) {
    $found = @($Events | Where-Object event -EQ $Name)
    if ($found.Count -ne 1) { throw "Expected exactly one pending-save shutdown event '$Name'." }
    $found[0]
}

$fixtureProfile = Join-Path $EvidenceDirectory 'NetMap'
New-Item -ItemType Directory -Path $fixtureProfile | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $fixtureProfile 'preferences.txt')
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$test = switch ($Mode) {
    'Reopen' { 'host::native_settings::native_pending_save_survives_settings_close_and_reopen' }
    'CommitOnExit' { 'host::native_settings::native_host_close_commits_an_accepted_pending_save' }
    'FallbackOnExit' { 'host::native_settings::native_host_close_preserves_preferences_when_pending_save_reaches_fallback' }
}
$preferences = Join-Path $fixtureProfile 'preferences.txt'
$original = [Convert]::ToBase64String([IO.File]::ReadAllBytes($preferences))
$stdout = Join-Path $EvidenceDirectory 'stdout.txt'
$stderr = Join-Path $EvidenceDirectory 'stderr.txt'
$savedData = $env:LOCALAPPDATA
$process = $null
$result = [ordered]@{
    TestExecutable = $TestExecutable
    TestExecutableSHA256 = (Get-FileHash -LiteralPath $TestExecutable -Algorithm SHA256).Hash
    Name = $test
    Mode = $Mode
    IsAdministrator = $isAdministrator
    Scope = if ($Mode -eq 'Reopen') {
        'One accepted real registered preferences save held by a private next-save worker gate; production modal Settings close/reopen, saving-state controls including lazy Desktop, actual storage/reply completion and clean newer dialog baseline. Owned numeric UI messages only; no public network, UAC, installation, screen saver actions or physical-input/layout proof.'
    } else {
        'Real modal close followed by actual Host WM_CLOSE while its sole accepted save remains registered and held before storage. Release after HWND removal proves normal finalization/commit; retaining that same gate proves real fallback initiation and last-committed byte preservation. A production-parser baseline read plus post-exit byte identity establishes fallback parseability. Pre-storage holding does not prove interruption during atomic replacement; HWND removal, fallback timing and process-exit/removal observations are separate. No extra uncooperative worker, UAC, public network or global input.'
    }
    OriginalPreferencesSHA256 = (Get-FileHash -LiteralPath $preferences -Algorithm SHA256).Hash
    TimedOut = $false
    Passed = $false
}
try {
    if ($isAdministrator) { throw 'This check requires an ordinary Windows token.' }
    $env:LOCALAPPDATA = $EvidenceDirectory
    $process = Start-Process -FilePath $TestExecutable -ArgumentList @('--ignored', '--exact', $test, '--nocapture', '--test-threads=1') `
        -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    $result.ProcessId = $process.Id
    $started = $process.StartTime.ToUniversalTime()
    $result.StartedUtc = $started.ToString('o')
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        $result.TimedOut = $true
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }
        throw 'The owned pending-Settings check exceeded its watchdog.'
    }
    $result.ExitObservedTickMs = [Environment]::TickCount64
    $result.ExitCode = $process.ExitCode
    $output = Get-Content -LiteralPath $stdout -Raw
    if ($Mode -ne 'FallbackOnExit' -and ($result.ExitCode -ne 0 -or $output -notmatch 'running 1 test' -or $output -notmatch '1 passed; 0 failed')) {
        throw 'The exact pending-Settings test did not pass.'
    }
    if ($Mode -eq 'Reopen') {
        $cases = @(Get-Content -LiteralPath $stdout | ForEach-Object {
            if ($_ -match 'NETMAP_PENDING_SETTINGS (\{.+\})') { $Matches[1] | ConvertFrom-Json }
        })
        if ($cases.Count -ne 1) { throw 'The exact pending-Settings test did not record its complete evidence.' }
        $case = $cases[0]
        $result.Case = $case
        if ($case.old_window -eq 0 -or $case.new_window -eq 0 -or $case.save -eq 0 -or $case.bright -ne 17 -or
            -not $case.worker_entered -or -not $case.old_detached -or -not $case.new_observed_pending -or
            -not $case.lazy_desktop_disabled -or -not $case.new_survived_old_ok -or -not $case.clean_baseline) {
            throw 'Native saving/detachment/observer evidence was incomplete.'
        }
    } else {
        $events = @(Get-Content -LiteralPath $stderr | ForEach-Object {
            if ($_ -match 'NETMAP_SHUTDOWN (\{.+\})') { $Matches[1] | ConvertFrom-Json }
        })
        $result.Events = $events
        $kind = if ($Mode -eq 'CommitOnExit') { 'preferences-commit' } else { 'preferences-fallback' }
        if (@($events | Where-Object kind -NE $kind).Count -ne 0) { throw 'Shutdown evidence contains a different scoped check.' }
        $entered = Require-Event $events 'preferences_worker_entered'
        $accepted = Require-Event $events 'preferences_accepted'
        $closed = Require-Event $events 'close_requested'
        $removed = Require-Event $events 'window_removed'
        $finish = Require-Event $events 'finish_started'
        if ($entered.baseline_bright -eq 17 -or $entered.baseline_bright -ne $accepted.committed_bright -or $entered.gate_maximum_ms -ne 10000 -or
            $accepted.save -eq 0 -or $accepted.desired_bright -ne 17 -or $accepted.committed_bright -eq 17 -or
            -not $accepted.modal_detached -or $accepted.modal_window -eq 0 -or $closed.window -eq 0 -or
            $closed.window -ne $removed.window -or $entered.tick_ms -gt $accepted.tick_ms -or
            $accepted.tick_ms -gt $closed.tick_ms -or $closed.tick_ms -gt $removed.tick_ms -or
            $removed.tick_ms -gt $finish.tick_ms -or $finish.allowance_ms -ne 5000) {
            throw 'Accepted-save entry, modal/Host close or finalization evidence was incomplete.'
        }
        if ($Mode -eq 'CommitOnExit') {
            $release = Require-Event $events 'preferences_release_after_window'
            $returned = Require-Event $events 'preferences_gate_returned'
            $completed = Require-Event $events 'finish_completed'
            if ($release.tick_ms -lt $removed.tick_ms -or $returned.tick_ms -lt $release.tick_ms -or
                -not $returned.released -or $completed.tick_ms -lt $returned.tick_ms -or
                @($events | Where-Object event -EQ 'fallback_initiated').Count -ne 0) {
                throw 'Normal exit did not release and finish the actual accepted save after HWND removal.'
            }
        } else {
            $held = Require-Event $events 'preferences_held_through_finish'
            $fallback = Require-Event $events 'fallback_initiated'
            if ($result.ExitCode -ne 1 -or $held.tick_ms -lt $removed.tick_ms -or $held.tick_ms -gt $finish.tick_ms -or
                $fallback.elapsed_ms -lt 4990 -or $fallback.tick_ms -lt ($finish.tick_ms + 4990) -or
                $fallback.tick_ms -ge ($entered.tick_ms + $entered.gate_maximum_ms) -or
                @($events | Where-Object event -In @('preferences_gate_returned', 'preferences_release_after_window', 'finish_completed')).Count -ne 0) {
                throw 'The accepted save gate did not remain outstanding through the real five-second fallback.'
            }
            $result.FallbackDeadlineLatenessUs = $fallback.deadline_lateness_us
            $result.FallbackToExitObservedMs = $result.ExitObservedTickMs - $fallback.tick_ms
        }
    }

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
    $result.OwnInstanceAbsent = -not $sameInstance
    $result.RemovalObservedTickMs = [Environment]::TickCount64
    if ($sameInstance) { throw 'The exact exited test process remained enumerated beyond its observation bound.' }
    # Inspect only the isolated profile after the exact owned process exits.
    $saved = Get-Content -LiteralPath $preferences -Raw
    if ($Mode -eq 'FallbackOnExit') {
        $result.OriginalBytesPreserved = [Convert]::ToBase64String([IO.File]::ReadAllBytes($preferences)) -ceq $original
        if (-not $result.OriginalBytesPreserved) { throw 'Fallback changed the last committed preferences bytes.' }
    } elseif ($saved -notmatch '(?m)^bright=17\r?$') { throw 'The actual saved preferences did not retain bright=17.' }
    $rollback = Join-Path $fixtureProfile 'preferences.rollback'
    $result.RollbackAbsent = -not (Test-Path -LiteralPath $rollback)
    if (-not $result.RollbackAbsent) { throw 'Preferences retained an uncommitted rollback record.' }
    $result.PreferencesSHA256 = (Get-FileHash -LiteralPath $preferences -Algorithm SHA256).Hash
    $result.Passed = $true
} catch {
    $result.Error = $_.Exception.Message
    throw
} finally {
    if ($null -ne $process) {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }
        $process.Dispose()
    }
    $env:LOCALAPPDATA = $savedData
    $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json') -Encoding utf8
}
