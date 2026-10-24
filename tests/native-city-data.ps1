#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $ProfileDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (Test-Path -LiteralPath $ProfileDirectory) { throw 'Use a new test profile directory.' }
New-Item -ItemType Directory -Path $ProfileDirectory | Out-Null
$ProfileDirectory = (Resolve-Path -LiteralPath $ProfileDirectory).Path
$data = Join-Path $ProfileDirectory 'NetMap'
New-Item -ItemType Directory -Path $data | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $data 'preferences.txt')
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class CityProof {
    public static uint Owner;
    delegate bool Callback(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr window, int id);
    [DllImport("user32.dll")] public static extern IntPtr GetMenu(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetSubMenu(IntPtr menu, int index);
    [DllImport("user32.dll")] public static extern uint GetMenuState(IntPtr menu, uint id, uint flags);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, IntPtr wparam, StringBuilder text, uint flags, uint timeout, out IntPtr result);
    public static IntPtr Find(uint process) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, parameter) => {
            GetWindowThreadProcessId(window, out uint owner);
            if (owner == process && GetDlgItem(window, 1000) != IntPtr.Zero) { found = window; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static bool Owned(IntPtr window) {
        GetWindowThreadProcessId(window, out uint process);
        return window != IntPtr.Zero && process == Owner;
    }
    public static void Send(IntPtr window, uint message, long wparam=0) {
        if (!Owned(window)) throw new InvalidOperationException("Test window was retired");
        if (SendMessageTimeoutW(window, message, (IntPtr)wparam, IntPtr.Zero, 2, 2000, out _) == IntPtr.Zero)
            throw new InvalidOperationException("Owned UI did not answer message " + message);
    }
    public static string Text(IntPtr window) {
        if (!Owned(window)) throw new InvalidOperationException("Test control was retired");
        var text = new StringBuilder(1024);
        if (SendMessageTimeoutW(window, 13, (IntPtr)1024, text, 2, 2000, out _) == IntPtr.Zero)
            throw new InvalidOperationException("Owned UI did not answer text request");
        return text.ToString();
    }
}
'@

$process = $null
$window = [IntPtr]::Zero
$peakPrivate = 0L
$peakWorking = 0L
function Start-Proof {
    $script:window = [IntPtr]::Zero
    $start = [Diagnostics.ProcessStartInfo]::new($Executable)
    $start.UseShellExecute = $false
    $start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $start.Environment['LOCALAPPDATA'] = $ProfileDirectory
    $script:process = [Diagnostics.Process]::Start($start)
    [CityProof]::Owner = $process.Id
    if (-not $process.WaitForInputIdle(5000)) { throw 'Owned process did not become input idle.' }
    $script:window = [CityProof]::Find($process.Id)
    if ($window -eq [IntPtr]::Zero) { throw 'Owned map window was not found.' }
    $script:status = [CityProof]::GetDlgItem($window, 1001)
    $script:zoom = [CityProof]::GetDlgItem($window, 1003)
    $script:menu = [CityProof]::GetSubMenu([CityProof]::GetMenu($window), 2)
}
function Wait-Proof([scriptblock] $Condition, [string] $Failure, [int] $Seconds = 10) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    do {
        if ($process.HasExited) { throw "Owned process exited during the check: $($process.ExitCode)" }
        $process.Refresh()
        $script:peakPrivate = [Math]::Max($peakPrivate, $process.PrivateMemorySize64)
        $script:peakWorking = [Math]::Max($peakWorking, $process.WorkingSet64)
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "$Failure Last status: $([CityProof]::Text($status))"
}
function Stop-Proof {
    if ([CityProof]::Owned($window)) { [void][CityProof]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) }
    if (-not $process.WaitForExit(6000)) { throw 'Owned process did not close.' }
    if ($process.ExitCode -ne 0) { throw "Owned process exit code: $($process.ExitCode)" }
    $process.Dispose()
    $script:process = $null
    $script:window = [IntPtr]::Zero
}
function Assert-NoFailure {
    if ([CityProof]::Text($status) -match '— City data:') { throw [CityProof]::Text($status) }
}

try {
    Start-Proof
    Wait-Proof {
        [CityProof]::Send($window, 0x117, $menu.ToInt64())
        ([CityProof]::GetMenuState($menu, 105, 0) -band 3) -eq 0
    } 'City data command never became available.'
    [CityProof]::Send($window, 0x111, 105)
    Wait-Proof { Assert-NoFailure; [CityProof]::Text($status) -match 'Downloading city data: [1-9]' } 'Transfer did not start.' 120
    [CityProof]::Send($window, 0x111, 102)
    if ([CityProof]::Text($zoom) -ne '1.2×') { throw 'Navigation was not responsive during download.' }
    [CityProof]::Send($window, 0x111, 106)
    Wait-Proof { Assert-NoFailure; [CityProof]::Text($status) -notmatch '(Downloading|Unpacking|Validating|Installing) city data' } 'Download cancellation did not settle.' 40
    $data = Join-Path $ProfileDirectory 'NetMap'
    if (Test-Path -LiteralPath (Join-Path $data 'city.mmdb')) { throw 'Cancelled first download installed a database.' }
    if (@(Get-ChildItem -LiteralPath $data -File -Filter 'city*').Count -ne 0) { throw 'Cancelled download retained partial files.' }
    [CityProof]::Send($window, 0x111, 105)
    Wait-Proof {
        Assert-NoFailure
        $text = [CityProof]::Text($status)
        $text -match 'City data ready' -and $text -notmatch '(Downloading|Unpacking|Validating|Installing) city data'
    } 'City download/install did not complete.' 600
    $city = Get-Item -LiteralPath (Join-Path $data 'city.mmdb')
    $hash = (Get-FileHash -LiteralPath $city.FullName -Algorithm SHA256).Hash
    $installedStatus = [CityProof]::Text($status)
    Stop-Proof
    Start-Proof
    Wait-Proof { [CityProof]::Text($status) -match 'City data ready' } 'Installed data did not reopen on restart.'
    Stop-Proof
    [pscustomobject]@{
        CancelledTransferCleanly = $true; NavigationDuringDownload = $true
        Installed = $true; ReopenedAfterRestart = $true
        CityBytes = $city.Length; SHA256 = $hash; Status = $installedStatus
        PeakSampledPrivateBytes = $peakPrivate; PeakSampledWorkingSetBytes = $peakWorking
        ProfileDirectory = $ProfileDirectory
    } | ConvertTo-Json
}
finally {
    if ($null -ne $process) {
        if (-not $process.HasExited) {
            if ([CityProof]::Owned($window)) { [void][CityProof]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) }
            if (-not $process.WaitForExit(6000)) { $process.Kill(); $process.WaitForExit() }
        }
        $process.Dispose()
    }
}
