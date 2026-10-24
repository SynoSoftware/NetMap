#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $EvidenceDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory and isolated profile.' }
$data = Join-Path $EvidenceDirectory 'NetMap'
New-Item -ItemType Directory -Path $data | Out-Null
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $data 'preferences.txt')
$preferencesHash = (Get-FileHash -LiteralPath (Join-Path $data 'preferences.txt')).Hash
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class PlacementProof {
    public static uint Owner;
    delegate bool Callback(IntPtr window, IntPtr parameter);
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct Placement {
        public uint Length, Flags, Show;
        public Point Min, Max;
        public Rect Normal;
    }
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] static extern IntPtr GetDlgItem(IntPtr window, int id);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] static extern bool GetWindowPlacement(IntPtr window, ref Placement placement);
    [DllImport("user32.dll")] static extern bool SetWindowPlacement(IntPtr window, ref Placement placement);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] static extern IntPtr MonitorFromRect(ref Rect rect, uint flags);
    [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr window, int show);
    [DllImport("user32.dll")] static extern bool PostMessageW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, IntPtr wparam, StringBuilder text, uint flags, uint timeout, out IntPtr result);
    public static bool Owned(IntPtr window) {
        GetWindowThreadProcessId(window, out uint process);
        return window != IntPtr.Zero && process == Owner;
    }
    public static IntPtr Find() {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, parameter) => {
            if (Owned(window) && GetDlgItem(window, 1000) != IntPtr.Zero && IsWindowVisible(window)) { found = window; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static Placement Read(IntPtr window) {
        if (!Owned(window)) throw new InvalidOperationException("Owned window retired");
        var value = new Placement { Length = (uint)Marshal.SizeOf<Placement>() };
        if (!GetWindowPlacement(window, ref value)) throw new InvalidOperationException("Placement read failed");
        return value;
    }
    public static bool Ready(IntPtr window) {
        if (!Owned(window)) return false;
        var text = new StringBuilder(4096);
        return SendMessageTimeoutW(GetDlgItem(window, 1001), 13, (IntPtr)4096, text, 2, 1000, out _) != IntPtr.Zero
            && text.ToString().Contains("observed connections");
    }
    public static void Set(IntPtr window, Placement value) {
        if (!Owned(window) || !SetWindowPlacement(window, ref value)) throw new InvalidOperationException("Placement restore failed");
    }
    public static void Show(IntPtr window, int show) {
        if (!Owned(window)) throw new InvalidOperationException("Owned window retired");
        ShowWindow(window, show);
    }
    public static bool OnScreen(IntPtr window) {
        if (!Owned(window) || !GetWindowRect(window, out var rect)) return false;
        return MonitorFromRect(ref rect, 0) != IntPtr.Zero;
    }
    public static void Close(IntPtr window) {
        if (!Owned(window) || !PostMessageW(window, 16, IntPtr.Zero, IntPtr.Zero)) throw new InvalidOperationException("Owned close failed");
    }
}
'@

$script:process = $null
$script:window = [IntPtr]::Zero
$results = [Collections.Generic.List[object]]::new()
function Open-Map {
    $script:window = [IntPtr]::Zero
    $start = [Diagnostics.ProcessStartInfo]::new($Executable)
    $start.UseShellExecute = $false
    $start.RedirectStandardError = $true
    $start.Environment['LOCALAPPDATA'] = $EvidenceDirectory
    $script:process = [Diagnostics.Process]::Start($start)
    [PlacementProof]::Owner = $script:process.Id
    $deadline = [DateTime]::UtcNow.AddSeconds(12)
    do {
        if ($script:process.HasExited) { throw "NetMap exited early: $($script:process.StandardError.ReadToEnd())" }
        $script:window = [PlacementProof]::Find()
        if ($script:window -ne [IntPtr]::Zero -and [PlacementProof]::Ready($script:window)) { return }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'Owned visible window did not appear.'
}
function Close-Map([string] $Phase) {
    $before = [PlacementProof]::Read($script:window)
    [PlacementProof]::Close($script:window)
    if (-not $script:process.WaitForExit(8000)) { throw 'Owned application did not exit.' }
    $stderr = $script:process.StandardError.ReadToEnd()
    $results.Add([pscustomobject]@{
        Phase = $Phase; ProcessId = $script:process.Id; ExitCode = $script:process.ExitCode; Stderr = $stderr
        Show = $before.Show; Flags = $before.Flags
        Normal = @($before.Normal.Left, $before.Normal.Top, $before.Normal.Right, $before.Normal.Bottom)
        Saved = Get-Content -LiteralPath (Join-Path $data 'window.txt') -Raw
    })
    if ($script:process.ExitCode -ne 0 -or $stderr) { throw "Application failed: $stderr" }
    $script:process.Dispose()
    $script:process = $null
    $script:window = [IntPtr]::Zero
}
function Same-Bounds($Expected, $Actual) {
    foreach ($field in @('Left', 'Top', 'Right', 'Bottom')) {
        if ($Expected.$field -ne $Actual.$field) { throw "Normal bounds differ at ${field}: $($Expected.$field) vs $($Actual.$field)" }
    }
}
try {
    Open-Map
    $placement = [PlacementProof]::Read($window)
    $placement.Show = 1
    # Assign the complete value-type field; editing a nested struct in
    # PowerShell changes a boxed copy rather than WINDOWPLACEMENT itself.
    $normal = [PlacementProof+Rect]::new()
    $normal.Left = 100; $normal.Top = 100
    $normal.Right = 1000; $normal.Bottom = 700
    $placement.Normal = $normal
    [PlacementProof]::Set($window, $placement)
    Same-Bounds $normal ([PlacementProof]::Read($window).Normal)
    Close-Map 'save-normal'
    Open-Map
    Same-Bounds $normal ([PlacementProof]::Read($window).Normal)
    [PlacementProof]::Show($window, 3)
    Close-Map 'restore-normal-save-maximized'
    Open-Map
    if ([PlacementProof]::Read($window).Show -ne 3) { throw 'Maximized state was not restored.' }
    Same-Bounds $normal ([PlacementProof]::Read($window).Normal)
    [PlacementProof]::Show($window, 6)
    Close-Map 'minimize-maximized'
    Open-Map
    if ([PlacementProof]::Read($window).Show -ne 3) { throw 'Minimized close lost maximized restoration.' }
    Same-Bounds $normal ([PlacementProof]::Read($window).Normal)
    Close-Map 'restore-after-minimized-close'
    # This file belongs to the isolated fixture; emulate a removed monitor.
    [IO.File]::WriteAllText((Join-Path $data 'window.txt'), "NetMap window 1`n1000000,1000000,1000900,1000600`nfalse`n")
    Open-Map
    if (-not [PlacementProof]::OnScreen($window)) { throw 'Off-screen placement was not recovered.' }
    Close-Map 'recover-removed-monitor'
    if ((Get-FileHash -LiteralPath (Join-Path $data 'preferences.txt')).Hash -ne $preferencesHash) { throw 'Window persistence changed preferences.' }
    if (Test-Path -LiteralPath (Join-Path $data 'window.rollback')) { throw 'Successful close left a rollback transaction.' }
    $results | ConvertTo-Json | Tee-Object -FilePath (Join-Path $EvidenceDirectory 'result.json')
}
finally {
    $results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json') -Encoding utf8NoBOM
    if ($process -and -not $process.HasExited) {
        if ([PlacementProof]::Owned($window)) { [PlacementProof]::Close($window); [void]$process.WaitForExit(3000) }
        if (-not $process.HasExited) { $process.Kill(); [void]$process.WaitForExit(3000) }
    }
    if ($process) { $process.Dispose() }
}
