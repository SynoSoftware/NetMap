#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $TestExecutable,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [ValidateRange(30, 180)][int] $TimeoutSeconds = 90
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$TestExecutable = (Resolve-Path -LiteralPath $TestExecutable).Path
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory and isolated profile.' }
New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$fixtureProfile = Join-Path $EvidenceDirectory 'NetMap'
New-Item -ItemType Directory -Path $fixtureProfile | Out-Null
$preferences = Join-Path $fixtureProfile 'preferences.txt'
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination $preferences
$stdout = Join-Path $EvidenceDirectory 'stdout.txt'
$stderr = Join-Path $EvidenceDirectory 'stderr.txt'
$test = 'host::native_desktop::native_desktop_disable_and_recovery_preserve_current_intent'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    $isAdministrator = [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
} finally { $identity.Dispose() }

function Read-Phases {
    if (-not (Test-Path -LiteralPath $stdout)) { return }
    foreach ($line in Get-Content -LiteralPath $stdout) {
        # Libtest can prefix the first record with its exact test name.
        if ($line -match 'NETMAP_DESKTOP_CASE (\{.+\})$') { $Matches[1] | ConvertFrom-Json }
    }
}

$result = [ordered]@{
    Test = $test
    TestExecutable = $TestExecutable
    TestExecutableSHA256 = (Get-FileHash -LiteralPath $TestExecutable -Algorithm SHA256).Hash
    IsAdministrator = $isAdministrator
    Scope = 'Real registered Explorer discovery and connected-monitor reads, held before ordinary reply delivery; actual owned menu/preference-save and native map-parent routing. Covers disable-before-reply and repeated recovery coalescing at 128x zoom, returning only the owned map to Window. Numeric display messages exercise the owned handler; no Explorer restart, monitor/topology mutation, physical input, public HTTP, UAC, installation or user-profile settings mutation.'
    InitialPreferencesSHA256 = (Get-FileHash -LiteralPath $preferences -Algorithm SHA256).Hash
    TimedOut = $false
    Phases = @()
    Passed = $false
}
$savedData = $env:LOCALAPPDATA
$process = $null
try {
    if ($isAdministrator) { throw 'Desktop race acceptance requires an ordinary Windows token.' }
    $env:LOCALAPPDATA = $EvidenceDirectory
    $process = Start-Process -FilePath $TestExecutable -ArgumentList @('--ignored', '--exact', $test, '--nocapture', '--test-threads=1') `
        -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
    $result.ProcessId = $process.Id
    $result.StartedUtc = $process.StartTime.ToUniversalTime().ToString('o')
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        $result.TimedOut = $true
        throw 'Owned Desktop race test exceeded its process watchdog.'
    }
    $result.ExitCode = $process.ExitCode
    $text = Get-Content -LiteralPath $stdout -Raw
    if ($process.ExitCode -ne 0 -or $text -notmatch 'running 1 test' -or $text -notmatch '1 passed; 0 failed') {
        throw 'Desktop race native exact test did not pass; retain raw output and completed phases.'
    }
    $phases = @(Read-Phases)
    if ($phases.Count -ne 4 -or ($phases.phase -join ',') -ne 'disabled,coalesced,attached,restored') {
        throw 'Desktop native journey did not record all four ordered phases.'
    }
    $disabled, $coalesced, $attached, $restored = $phases
    if ($disabled.id -le 0 -or $coalesced.obsolete_id -le $disabled.id -or
        $coalesced.current_id -le ($coalesced.obsolete_id + 1) -or $attached.id -ne $coalesced.current_id -or
        $coalesced.recoveries -ne 3 -or $restored.desktop -ne $false) {
        throw 'Desktop result did not establish distinct obsolete/current requests and final disablement.'
    }
    if ($disabled.map -eq 0 -or $disabled.parent -eq 0 -or $attached.parent -eq 0 -or
        $coalesced.map -ne $disabled.map -or $attached.map -ne $disabled.map -or $restored.map -ne $disabled.map -or
        $coalesced.parent -ne $disabled.parent -or $restored.parent -ne $disabled.parent -or $attached.parent -eq $disabled.parent) {
        throw 'Desktop native parent/owned-map continuity record is invalid.'
    }
    if ($disabled.monitors -lt 1 -or $coalesced.monitors -lt 1 -or $attached.monitors -lt 1 -or
        @($phases | Where-Object { $_.zoom -ne 128 -or $_.ticks -lt 1 }).Count -ne 0 -or
        $restored.ticks -le $disabled.ticks) {
        throw 'Desktop result did not establish current monitors, high zoom and message-pump progress.'
    }
    $lines = @(Get-Content -LiteralPath $preferences)
    if ($lines -notcontains 'desktop=false' -or $lines -notcontains 'origin=manual') {
        throw 'Actual final saved isolated preferences did not disable Desktop and retain manual origin.'
    }
    $result.FinalPreferencesSHA256 = (Get-FileHash -LiteralPath $preferences -Algorithm SHA256).Hash
    $result.SavedDesktop = $false
    $result.Passed = $true
} catch {
    $result.Error = $_.Exception.Message
    throw
} finally {
    if ($null -ne $process) {
        if (-not $process.HasExited) {
            # This descriptor is the exact process created above. Windows
            # removes its owned HWNDs; no Explorer or unrelated process is killed.
            $process.Kill()
            $result.CleanupTerminatedOwnedProcess = $true
            $result.CleanupExitObserved = $process.WaitForExit(5000)
        }
        $process.Dispose()
    }
    $env:LOCALAPPDATA = $savedData
    $result.Phases = @(Read-Phases)
    $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json') -Encoding utf8
}
