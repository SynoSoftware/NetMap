#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $TestExecutable,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [ValidateSet('All', 'Navigation', 'Temporal')][string] $Mode = 'All'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$TestExecutable = (Resolve-Path -LiteralPath $TestExecutable).Path
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory.' }
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    if ([Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'This journey requires an ordinary Windows token.'
    }
} finally { $identity.Dispose() }
New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$cases = if ($Mode -eq 'All') { @('Navigation', 'Temporal') } else { @($Mode) }
$result = [ordered]@{
    Schema = 1
    TestExecutable = $TestExecutable
    TestExecutableSHA256 = (Get-FileHash -LiteralPath $TestExecutable -Algorithm SHA256).Hash
    WrapperSHA256 = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash
    ImageCapturePoint = 'Requested successful EndDraw before flip Present; phase clock/revision/model identify that captured frame. Capture requires successful subsequent Present. Contrast image asserts its supplied black clear pixel.'
    StartedUtc = [DateTime]::UtcNow.ToString('o')
    Scope = 'Native-only accepted supplied facts through Application and real Host/Map/Renderer. Owned numeric input/capture and actual Host resize; intermediate full-target BMPs. Real accepted-sample monotonic clock, default bright10/fade20 and zero-fade. Supplied contrast/reduced-motion policy, not OS settings. No matched-source parity, physical input/DPI, memory/allocation totals, GPU execution, collector/network or default product performance claim.'
    Cases = @()
    Passed = $false
}
$savedData = $env:LOCALAPPDATA
$savedEvidence = $env:NETMAP_NATIVE_DYNAMICS_DIRECTORY
$process = $null
try {
    foreach ($case in $cases) {
        $directory = Join-Path $EvidenceDirectory $case
        $profile = Join-Path $directory 'NetMap'
        New-Item -ItemType Directory -Path $profile | Out-Null
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $profile 'preferences.txt')
        $test = if ($case -eq 'Navigation') {
            'host::native_dynamics::native_map_navigation_updates_and_rebases_during_capture'
        } else {
            'host::native_dynamics::native_map_temporal_policy_and_retained_resources'
        }
        $bound = 65
        $entry = [ordered]@{ Case = $case; Name = $test; TimeoutSeconds = $bound; Passed = $false; TimedOut = $false }
        $result.Cases += $entry
        $env:LOCALAPPDATA = $directory
        $env:NETMAP_NATIVE_DYNAMICS_DIRECTORY = $directory
        $stdout = Join-Path $directory 'stdout.txt'
        $stderr = Join-Path $directory 'stderr.txt'
        $process = Start-Process -FilePath $TestExecutable -ArgumentList @('--ignored', '--exact', $test, '--nocapture', '--test-threads=1') `
            -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
        $entry.ProcessId = $process.Id
        $entry.StartedUtc = $process.StartTime.ToUniversalTime().ToString('o')
        if (-not $process.WaitForExit($bound * 1000)) {
            $entry.TimedOut = $true
            throw "The owned $case journey exceeded its watchdog."
        }
        $entry.ExitCode = $process.ExitCode
        $output = Get-Content -LiteralPath $stdout -Raw
        if ($entry.ExitCode -ne 0 -or $output -notmatch 'running 1 test' -or $output -notmatch '1 passed; 0 failed') {
            throw "The exact $case journey did not pass; retain its partial evidence."
        }
        if ((Get-Content -LiteralPath (Join-Path $directory 'complete.txt') -Raw).Trim() -ne '1') {
            throw "The $case journey did not record completion."
        }
        $phases = @(Get-Content -LiteralPath (Join-Path $directory 'phases.jsonl') | ForEach-Object { $_ | ConvertFrom-Json })
        $entry.Phases = $phases.Count
        if ($case -eq 'Navigation') {
            $rows = @(Import-Csv -LiteralPath (Join-Path $directory 'input-latency.tsv') -Delimiter "`t")
            if ($rows.Count -ne 24) { throw 'Navigation did not retain exactly 24 owned-input timing samples.' }
            $values = @(for ($index = 0; $index -lt $rows.Count; $index++) {
                $row = $rows[$index]
                $elapsed = [double]::Parse($row.owned_dispatch_to_present_return_us, [Globalization.CultureInfo]::InvariantCulture)
                if ([int]$row.step -ne $index + 1 -or -not [double]::IsFinite($elapsed) -or $elapsed -lt 0) {
                    throw 'Navigation retained an invalid owned-input timing sample.'
                }
                $elapsed
            })
            $ordered = @($values | Sort-Object)
            $entry.InputTiming = [ordered]@{
                Samples = $rows.Count
                Unit = 'microseconds'
                Median = ($ordered[11] + $ordered[12]) / 2
                P95NearestRank = $ordered[22]
                Worst = $ordered[23]
                Scope = 'Before each existing owned mouse dispatch through its last fresh successful current-revision/model Present API return, including existing dispatch/pump and explicit repaint. No armed capture readback during timing. Not first-response, physical input, OS input-queue, compositor or display latency. Preparation age is not used.'
            }
        }
        $entry.Images = @(Get-ChildItem -LiteralPath $directory -Filter '*.bmp' -File | ForEach-Object {
            $bytes = [IO.File]::ReadAllBytes($_.FullName)
            [ordered]@{ Name = $_.Name; Width = [BitConverter]::ToInt32($bytes, 18)
                Height = [Math]::Abs([BitConverter]::ToInt32($bytes, 22)); SHA256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
        })
        $entry.Evidence = @(Get-ChildItem -LiteralPath $directory -File | Where-Object Extension -In @('.tsv', '.jsonl', '.txt') | ForEach-Object {
            [ordered]@{ Name = $_.Name; SHA256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
        })
        $entry.Passed = $true
        $process.Dispose()
        $process = $null
    }
    $result.Passed = $true
} catch {
    $result.Error = $_.Exception.Message
    throw
} finally {
    # The wrapper owns exactly these child process handles. Never enumerate or
    # terminate by process name, directory or someone else's window identity.
    if ($null -ne $process) {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }
        $process.Dispose()
    }
    $env:LOCALAPPDATA = $savedData
    $env:NETMAP_NATIVE_DYNAMICS_DIRECTORY = $savedEvidence
    $result.CompletedUtc = [DateTime]::UtcNow.ToString('o')
    $result | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json') -Encoding utf8
}
