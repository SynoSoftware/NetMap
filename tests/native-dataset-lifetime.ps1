#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $TestExecutable,
    [Parameter(Mandatory)][string] $CityDatabase,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [ValidateRange(30, 180)][int] $TimeoutSeconds = 90
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$TestExecutable = (Resolve-Path -LiteralPath $TestExecutable).Path
$CityDatabase = (Resolve-Path -LiteralPath $CityDatabase).Path
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory.' }
$sourceHash = (Get-FileHash -LiteralPath $CityDatabase -Algorithm SHA256).Hash
if ($sourceHash -ne '9E250F02722D1AD1780F88192F0E908AF285478A33E76B88B349C9181AE9D0F5' -or
    (Get-Item -LiteralPath $CityDatabase).Length -ne 126998165) { throw 'Use the retained validated city source specified for this batch.' }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    $isAdministrator = [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
} finally { $identity.Dispose() }
New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$result = [ordered]@{
    TestExecutable = $TestExecutable
    TestExecutableSHA256 = (Get-FileHash -LiteralPath $TestExecutable -Algorithm SHA256).Hash
    CityDatabase = $CityDatabase
    SourceSHA256 = $sourceHash
    IsAdministrator = $isAdministrator
    Scope = 'Actual registered city operation using flushed, production-validated local candidates; real replacement-before-reopen failure, rollback/retry, modal Settings availability via real reader reports, and sole-install-job Host fallback followed by canonical/backup restart reads. This is an explicit boundary interruption, not interruption inside an atomic OS call. No public HTTP, UAC, global input, OS settings or alternate installer.'
    Checks = @()
    Passed = $false
}
$tests = @(
    @{ Name = 'host::native_dataset::native_dataset_reopen_failure_rolls_back_and_retry_installs'; Mode = 'rollback'; Profile = 'rollback'; Seed = $true; Exit = 0 },
    @{ Name = 'host::native_dataset::native_settings_revalidates_cities_after_actual_dataset_changes'; Mode = 'availability'; Profile = 'availability'; Seed = $false; Exit = 0 },
    @{ Name = 'host::native_dataset::native_dataset_replacement_survives_host_fallback'; Mode = 'fallback'; Profile = 'interrupted'; Seed = $true; Exit = 1 },
    @{ Name = 'host::native_dataset::native_dataset_restart_reads_replacement_and_recoverable_backup'; Mode = 'restart'; Profile = 'interrupted'; Seed = $false; Exit = 0 }
)

function Require-Event($Events, [string] $Name) {
    $found = @($Events | Where-Object event -EQ $Name)
    if ($found.Count -ne 1) { throw "Expected one dataset event '$Name'." }
    $found[0]
}
function Check-CityFile([string] $Path) {
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -ne $sourceHash) {
        throw "Owned dataset content differs from the validated source: $Path"
    }
}

$savedData = $env:LOCALAPPDATA
$savedSource = $env:NETMAP_TEST_CITY_DATABASE
$process = $null
try {
    if ($isAdministrator) { throw 'Dataset lifetime checks require an ordinary Windows token.' }
    $env:NETMAP_TEST_CITY_DATABASE = $CityDatabase
    foreach ($test in $tests) {
        $profileRoot = Join-Path $EvidenceDirectory $test.Profile
        $data = Join-Path $profileRoot 'NetMap'
        if ($test.Mode -ne 'restart') {
            New-Item -ItemType Directory -Path $data | Out-Null
            Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $data 'preferences.txt')
            if ($test.Seed) { Copy-Item -LiteralPath $CityDatabase -Destination (Join-Path $data 'city.mmdb') }
        }
        $env:LOCALAPPDATA = $profileRoot
        $stdout = Join-Path $EvidenceDirectory "$($test.Mode).stdout.txt"
        $stderr = Join-Path $EvidenceDirectory "$($test.Mode).stderr.txt"
        $check = [ordered]@{ Name = $test.Name; Mode = $test.Mode; Profile = $profileRoot; TimedOut = $false; Passed = $false }
        $result.Checks += $check
        $process = Start-Process -FilePath $TestExecutable -ArgumentList @('--ignored', '--exact', $test.Name, '--nocapture', '--test-threads=1') `
            -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
        $check.ProcessId = $process.Id
        $started = $process.StartTime.ToUniversalTime()
        $check.StartedUtc = $started.ToString('o')
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            $check.TimedOut = $true
            if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }
            throw "Dataset lifetime check timed out: $($test.Mode)"
        }
        $check.ExitObservedTickMs = [Environment]::TickCount64
        $check.ExitCode = $process.ExitCode
        $output = Get-Content -LiteralPath $stdout -Raw
        $events = @(Get-Content -LiteralPath $stdout | ForEach-Object {
            if ($_ -match 'NETMAP_DATASET (\{.+\})') { $Matches[1] | ConvertFrom-Json }
        })
        $check.Events = $events
        if ($check.ExitCode -ne $test.Exit -or $output -notmatch 'running 1 test' -or
            ($test.Exit -eq 0 -and $output -notmatch '1 passed; 0 failed')) { throw 'The exact dataset test did not reach its expected result.' }
        switch ($test.Mode) {
            'rollback' {
                $proof = Require-Event $events 'rollback_retry_verified'
                $boundary = Require-Event $events 'replacement_before_reopen'
                $returned = Require-Event $events 'boundary_returned'
                if (-not $boundary.backup_flushed -or -not $boundary.target_displaced -or
                    -not $returned.released -or
                    $proof.rollback_version -ne $proof.before_version -or $proof.retry_version -ne ($proof.before_version + 1) -or
                    -not $proof.same_lookup -or -not $proof.actual_reopen_error) { throw 'Actual reopen failure/rollback/retry evidence is incomplete.' }
                Check-CityFile (Join-Path $data 'city.mmdb')
                Check-CityFile (Join-Path $data 'city-displaced-new.evidence')
                if (@(Get-ChildItem -LiteralPath $data -Filter '*.backup').Count -ne 0) { throw 'Successful retry left an installer backup.' }
            }
            'availability' {
                $proof = Require-Event $events 'settings_availability_verified'
                if ($proof.ready_version -le $proof.missing_version -or $proof.withdrawn_version -le $proof.ready_version -or
                    -not $proof.raw_latitude_retained -or -not $proof.stale_cities_rejected) { throw 'Modal current-availability/draft evidence is incomplete.' }
                Check-CityFile (Join-Path $data 'city-withdrawn.evidence')
                if (Test-Path -LiteralPath (Join-Path $data 'city.mmdb')) { throw 'Withdrawn canonical dataset still exists.' }
                if ((Get-Content -LiteralPath (Join-Path $data 'preferences.txt') -Raw) -notmatch '(?m)^detail=countries\r?$') { throw 'Rejected Cities draft changed saved preferences.' }
            }
            'fallback' {
                $boundary = Require-Event $events 'replacement_before_reopen'
                if (-not $boundary.backup_flushed -or $boundary.target_displaced -or @($events | Where-Object event -EQ 'boundary_returned').Count -ne 0) {
                    throw 'Installer was not genuinely held after successful replacement.'
                }
                $shutdown = @(Get-Content -LiteralPath $stderr | ForEach-Object {
                    if ($_ -match 'NETMAP_SHUTDOWN (\{.+\})') { $Matches[1] | ConvertFrom-Json }
                })
                $check.Shutdown = $shutdown
                if (@($shutdown | Where-Object kind -NE 'dataset-fallback').Count -ne 0) { throw 'Fallback evidence contains a different scoped check.' }
                $replaced = Require-Event $shutdown 'dataset_replaced_before_reopen'
                $close = Require-Event $shutdown 'close_requested'
                $removed = Require-Event $shutdown 'window_removed'
                $held = Require-Event $shutdown 'dataset_held_through_finish'
                $finish = Require-Event $shutdown 'finish_started'
                $fallback = Require-Event $shutdown 'fallback_initiated'
                if ($replaced.previous_version -eq 0 -or -not $replaced.backup_flushed -or $close.window -eq 0 -or
                    $close.window -ne $removed.window -or $removed.tick_ms -lt $close.tick_ms -or
                    $held.tick_ms -lt $removed.tick_ms -or $finish.tick_ms -lt $held.tick_ms -or $finish.allowance_ms -ne 5000 -or
                    $fallback.elapsed_ms -lt 4990 -or $fallback.tick_ms -ge ($replaced.boundary_tick_ms + $replaced.gate_maximum_ms) -or
                    @($shutdown | Where-Object event -EQ 'finish_completed').Count -ne 0) { throw 'Sole-install-job fallback lifetime evidence is incomplete.' }
                $check.FallbackDeadlineLatenessUs = $fallback.deadline_lateness_us
                $check.FallbackToExitObservedMs = $check.ExitObservedTickMs - $fallback.tick_ms
                Check-CityFile (Join-Path $data 'city.mmdb')
                $backups = @(Get-ChildItem -LiteralPath $data -Filter '*.mmdb.backup')
                if ($backups.Count -ne 1) { throw 'Interrupted installer did not retain its single flushed backup.' }
                Check-CityFile $backups[0].FullName
            }
            'restart' {
                $proof = Require-Event $events 'restart_verified'
                if ($proof.backup_version -le $proof.canonical_version -or -not $proof.same_lookup -or -not $proof.backup_reader) { throw 'Actual restart canonical/backup readers were not verified.' }
                Check-CityFile (Join-Path $data 'city-withdrawn.evidence')
                $backups = @(Get-ChildItem -LiteralPath $data -Filter '*.mmdb.backup')
                if ($backups.Count -ne 1) { throw 'Restart lost the retained recoverable backup.' }
                Check-CityFile $backups[0].FullName
            }
        }
        $ownedId = $process.Id
        $process.Dispose(); $process = $null
        $candidate = Get-Process -Id $ownedId -ErrorAction SilentlyContinue
        $sameInstance = $false
        if ($null -ne $candidate) {
            try { $sameInstance = $candidate.StartTime.ToUniversalTime() -eq $started }
            finally { $candidate.Dispose() }
        }
        $check.OwnInstanceAbsent = -not $sameInstance
        $check.RemovalObservedTickMs = [Environment]::TickCount64
        if ($sameInstance) { throw 'Exited owned dataset process remains enumerated.' }
        $check.Files = @(Get-ChildItem -LiteralPath $data -File | Select-Object Name, Length)
        $check.Passed = $true
    }
    $result.SourceUnchanged = (Get-FileHash -LiteralPath $CityDatabase -Algorithm SHA256).Hash -eq $sourceHash
    if (-not $result.SourceUnchanged) { throw 'Retained source hash changed.' }
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
    $env:NETMAP_TEST_CITY_DATABASE = $savedSource
    $result | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json') -Encoding utf8
}
