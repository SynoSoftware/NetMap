#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $TestExecutable,
    [Parameter(Mandatory)][string] $Application,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [string] $CityDatabase,
    [ValidateRange(20, 180)][int] $TimeoutSeconds = 60,
    [ValidateNotNullOrEmpty()]
    [ValidateSet(
        'host::native_exploration::native_accepted_observations_preserve_group_and_row_identity',
        'host::native_foreground::native_host_pauses_presentations_for_actual_saver',
        'map::renderer::tests::native_active_and_idle_marks_draw_and_static_modes_stop_animation',
        'host::native_lifetime::native_worker_replies_survive_target_gap_and_replacement',
        'host::native_lifetime::native_local_locations_and_origin_survive_target_gap_and_replacement'
    )]
    [string[]] $Tests = @(
        'host::native_exploration::native_accepted_observations_preserve_group_and_row_identity',
        'host::native_foreground::native_host_pauses_presentations_for_actual_saver',
        'map::renderer::tests::native_active_and_idle_marks_draw_and_static_modes_stop_animation',
        'host::native_lifetime::native_worker_replies_survive_target_gap_and_replacement'
    )
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$TestExecutable = (Resolve-Path -LiteralPath $TestExecutable).Path
$Application = (Resolve-Path -LiteralPath $Application).Path
$localLocationsTest = 'host::native_lifetime::native_local_locations_and_origin_survive_target_gap_and_replacement'
$cityHash = '9E250F02722D1AD1780F88192F0E908AF285478A33E76B88B349C9181AE9D0F5'
$usesCity = $Tests -contains $localLocationsTest
if ($usesCity) {
    if ([string]::IsNullOrWhiteSpace($CityDatabase)) { throw 'The local-location check requires -CityDatabase pointing to the retained reviewed MMDB.' }
    $CityDatabase = (Resolve-Path -LiteralPath $CityDatabase).Path
    if ((Get-Item -LiteralPath $CityDatabase).Length -ne 126998165 -or
        (Get-FileHash -LiteralPath $CityDatabase -Algorithm SHA256).Hash -ne $cityHash) {
        throw 'Local-location source MMDB size/hash differs from the retained reviewed dataset.'
    }
}
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory.' }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    $isAdministrator = [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
} finally { $identity.Dispose() }
$fixtureProfile = Join-Path $EvidenceDirectory 'NetMap'
New-Item -ItemType Directory -Path $fixtureProfile | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $fixtureProfile 'preferences.txt')
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path

function Close-OwnedSavers([string] $Stderr) {
    $outcomes = @()
    if (-not (Test-Path -LiteralPath $Stderr)) { return $outcomes }
    foreach ($line in Get-Content -LiteralPath $Stderr) {
        if ($line -notmatch '^NETMAP_OWNED_SAVER (.+)$') { continue }
        $record = $Matches[1] | ConvertFrom-Json
        if (-not [string]::Equals([IO.Path]::GetFullPath($record.path), $Application, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Owned saver record did not match the supplied production executable.'
        }
        try { $child = [Diagnostics.Process]::GetProcessById([int]$record.pid) }
        catch [ArgumentException] { $outcomes += "Saver $($record.pid) already exited"; continue }
        try {
            if ($child.StartTime.ToUniversalTime().ToFileTimeUtc() -ne [long]$record.started -or
                -not [string]::Equals($child.MainModule.FileName, $Application, [StringComparison]::OrdinalIgnoreCase)) {
                throw 'Saver PID identity changed; cleanup refused.'
            }
            $child.CloseMainWindow() | Out-Null
            if (-not $child.WaitForExit(3000)) { $child.Kill(); $child.WaitForExit(5000) | Out-Null }
            $outcomes += "Verified owned saver $($record.pid) closed"
        } finally { $child.Dispose() }
    }
    $outcomes
}

$savedData = $env:LOCALAPPDATA
$savedApplication = $env:NETMAP_NATIVE_APP
$results = [ordered]@{
    TestExecutable = $TestExecutable
    TestExecutableSHA256 = (Get-FileHash -LiteralPath $TestExecutable -Algorithm SHA256).Hash
    Application = $Application
    ApplicationSHA256 = (Get-FileHash -LiteralPath $Application -Algorithm SHA256).Hash
    IsAdministrator = $isAdministrator
    Scope = 'Selected ordinary-token owned native fixtures; controlled facts/replies, actual /s foreground, Present attempts, Settings modal delivery, target replacement and native renderer checks; optional real local MMDB/origin/Scene completion. Each recorded test defines its actual coverage. No public discovery, HTTPS city download/install, UAC, screen saver selection, physical input, sleep/sign-in or full resource acceptance.'
    Checks = @()
    Passed = $false
}
$process = $null
try {
    if ($isAdministrator) { throw 'These native integration checks require an ordinary Windows token.' }
    if ($usesCity) {
        # Wrapper setup completes before Host exists. Production-shaped loading
        # and all subsequent reader/lookup work run on registered Work workers.
        $cityCopy = Join-Path $fixtureProfile 'city.mmdb'
        Copy-Item -LiteralPath $CityDatabase -Destination $cityCopy
        $copiedHash = (Get-FileHash -LiteralPath $cityCopy -Algorithm SHA256).Hash
        if ($copiedHash -ne $cityHash) { throw 'Isolated city MMDB copy failed its exact hash check.' }
        $results.Dataset = [ordered]@{ Source = $CityDatabase; Copy = $cityCopy; Bytes = 126998165; SHA256 = $copiedHash }
    }
    $env:LOCALAPPDATA = $EvidenceDirectory
    $env:NETMAP_NATIVE_APP = $Application
    for ($index = 0; $index -lt $tests.Count; $index++) {
        $stdout = Join-Path $EvidenceDirectory "$index.stdout.txt"
        $stderr = Join-Path $EvidenceDirectory "$index.stderr.txt"
        $check = [ordered]@{ Name = $tests[$index]; TimedOut = $false; Passed = $false; Cleanup = @() }
        $results.Checks += $check
        $process = Start-Process -FilePath $TestExecutable -ArgumentList @('--ignored', '--exact', $tests[$index], '--nocapture', '--test-threads=1') `
            -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
        $check.ProcessId = $process.Id
        $check.StartedUtc = $process.StartTime.ToUniversalTime().ToString('o')
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            $check.TimedOut = $true
            $check.Cleanup += @(Close-OwnedSavers $stderr)
            if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }
            throw "Native check timed out: $($tests[$index])"
        }
        $check.ExitCode = $process.ExitCode
        $check.Cleanup += @(Close-OwnedSavers $stderr)
        $text = Get-Content -LiteralPath $stdout -Raw
        if ($process.ExitCode -ne 0 -or $text -notmatch 'running 1 test' -or $text -notmatch '1 passed; 0 failed') {
            throw "Native check did not pass its exact test: $($tests[$index])"
        }
        if ($tests[$index] -eq 'host::native_foreground::native_host_pauses_presentations_for_actual_saver') {
            $cases = @(Get-Content -LiteralPath $stdout | ForEach-Object {
                if ($_ -match 'NETMAP_FOREGROUND_CASE (\{.+\})') { $Matches[1] | ConvertFrom-Json }
            })
            if ($cases.Count -ne 2) { throw 'Foreground check did not record both normal and Settings-modal cases.' }
            $check.Cases = $cases
        }
        if ($tests[$index] -eq 'host::native_lifetime::native_worker_replies_survive_target_gap_and_replacement') {
            $cases = @(Get-Content -LiteralPath $stdout | ForEach-Object {
                if ($_ -match 'NETMAP_LIFETIME_CASE (\{.+\})') { $Matches[1] | ConvertFrom-Json }
            })
            if ($cases.Count -ne 1) { throw 'Lifetime check did not record its target-gap case.' }
            $check.Cases = $cases
        }
        if ($tests[$index] -eq $localLocationsTest) {
            if ((Get-Content -LiteralPath $stderr -Raw) -match 'NETMAP_LOCAL_ORIGIN_DISCOVERY_REFUSED') {
                throw 'Local-location journey requested public discovery; the local-only guard refused it. Acceptance is incomplete.'
            }
            $cases = @(Get-Content -LiteralPath $stdout | ForEach-Object {
                if ($_ -match 'NETMAP_LOCAL_LOCATIONS_CASE (\{.+\})$') { $Matches[1] | ConvertFrom-Json }
            })
            if ($cases.Count -ne 1) { throw 'Local-location check did not record exactly one completed target-gap case.' }
            $case = $cases[0]
            if ($case.version -ne 1 -or $case.previous_version -ne 0 -or
                $case.origin_requests -ne 2 -or $case.stale_origin_replies -ne 2 -or
                $case.accepted_scenes -lt 1 -or $case.scene -le 0 -or
                $case.endpoint -eq 0 -or $case.old_map -eq 0 -or $case.replacement_map -eq 0 -or
                $case.rows -ne 5 -or $case.present_attempts -lt 1 -or $case.age_ms -lt 300000) {
                throw 'Local-location result record did not establish its required version/origin/Scene/native presentation contract.'
            }
            $check.Cases = $cases
            $check.DatasetSHA256 = (Get-FileHash -LiteralPath $cityCopy -Algorithm SHA256).Hash
            if ($check.DatasetSHA256 -ne $cityHash) { throw 'Native local-location journey changed its isolated city dataset.' }
        }
        $check.Passed = $true
        $process.Dispose(); $process = $null
    }
    $results.Passed = $true
} catch {
    $results.Error = $_.Exception.Message
    throw
} finally {
    if ($null -ne $process) {
        try { $check.Cleanup += @(Close-OwnedSavers $stderr) }
        catch { $results.CleanupError = $_.Exception.Message }
        finally {
            if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }
            $process.Dispose()
        }
    }
    $env:LOCALAPPDATA = $savedData
    $env:NETMAP_NATIVE_APP = $savedApplication
    $results | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json') -Encoding utf8
}
