#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [ValidateSet('Raw', 'Collector', 'Both')][string] $Check = 'Both',
    [switch] $External
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory.' }
New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$checks = [ordered]@{
    Raw = 'native_trace_preserves_loopback_bytes_raw_qpc_and_callback_lifetime'
    Collector = 'native_collector_preserves_short_processes_directional_bytes_and_rates'
}
$results = @()
foreach ($entry in $checks.GetEnumerator()) {
    if ($Check -ne 'Both' -and $Check -ne $entry.Key) { continue }
    $stdout = Join-Path $EvidenceDirectory "$($entry.Key).stdout.txt"
    $stderr = Join-Path $EvidenceDirectory "$($entry.Key).stderr.txt"
    $previous = [Environment]::GetEnvironmentVariable('NETMAP_TRACE_EXTERNAL', 'Process')
    try {
        [Environment]::SetEnvironmentVariable('NETMAP_TRACE_EXTERNAL', $(if ($External) { '1' } else { '0' }), 'Process')
        $process = Start-Process -FilePath $Executable -ArgumentList @(
            '--ignored', '--exact', "app::work::trace_checks::$($entry.Value)",
            '--nocapture', '--test-threads=1'
        ) -RedirectStandardOutput $stdout -RedirectStandardError $stderr -WindowStyle Hidden -PassThru
    }
    finally { [Environment]::SetEnvironmentVariable('NETMAP_TRACE_EXTERNAL', $previous, 'Process') }
    $timedOut = $false
    try {
        $timedOut = -not $process.WaitForExit(25000)
    }
    finally {
        if (-not $process.HasExited) {
            $process.Kill()
            $process.WaitForExit(3000) | Out-Null
        }
        $output = [string](Get-Content -LiteralPath $stderr -Raw)
        # A killed controller cannot run Drop. Stop only the exact GUID session
        # announced by this owned PID; never enumerate or adopt other traces.
        if ($timedOut) {
            $pattern = '(?m)^OwnedTraceName=(NetMap-' + $process.Id + '-\{?[0-9a-fA-F-]{36}\}?)\r?$'
            foreach ($match in [regex]::Matches($output, $pattern)) {
                $cleanup = Start-Process -FilePath "$env:SystemRoot\System32\logman.exe" -ArgumentList @(
                    'stop', $match.Groups[1].Value, '-ets'
                ) -WindowStyle Hidden -PassThru
                if (-not $cleanup.WaitForExit(5000)) { $cleanup.Kill() }
                $cleanup.Dispose()
            }
        }
    }
    $results += [pscustomobject]@{
        Check = $entry.Key
        External = [bool]$External
        ProcessId = $process.Id
        TimedOut = $timedOut
        ExitCode = $process.ExitCode
        ExecutableSHA256 = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash
        ResourceScope = @([regex]::Matches($output, '(?m)^CollectorResourceScope=(.+)\r?$') | ForEach-Object {
            $_.Groups[1].Value | ConvertFrom-Json
        })
        ResourceSamples = @([regex]::Matches($output, '(?m)^CollectorResource=(.+)\r?$') | ForEach-Object {
            $_.Groups[1].Value | ConvertFrom-Json
        })
        TraceResources = @([regex]::Matches($output, '(?m)^CollectorTraceResource=(.+)\r?$') | ForEach-Object {
            $_.Groups[1].Value | ConvertFrom-Json
        })
    }
    $process.Dispose()
}
$results | ConvertTo-Json -Depth 6 | Tee-Object -FilePath (Join-Path $EvidenceDirectory 'result.json')
if ($results | Where-Object { $_.TimedOut -or $_.ExitCode -ne 0 }) { exit 1 }
