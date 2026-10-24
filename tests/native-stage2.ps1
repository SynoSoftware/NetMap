#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [switch] $Desktop,
    [switch] $FullScreen
)

# Unattended, ordinary-token checks of only the exact processes/windows owned
# below. No physical input, elevation, screen saver selection or Explorer change.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory.' }
$data = Join-Path $EvidenceDirectory 'NetMap'
New-Item -ItemType Directory -Path $data | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $data 'preferences.txt')
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class Stage2Proof {
    delegate bool Callback(IntPtr window, IntPtr parameter);
    delegate bool MonitorCallback(IntPtr monitor, IntPtr dc, ref Rect bounds, IntPtr parameter);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct GuiInfo {
        public uint Size, Flags;
        public IntPtr Active, Focus, Capture, MenuOwner, MoveSize, Caret;
        public Rect CaretBounds;
    }
    public sealed class SaverTarget {
        public long HWND { get; set; }
        public Rect Bounds { get; set; }
        public bool Visible { get; set; }
        public bool Popup { get; set; }
        public bool Parentless { get; set; }
        public int Children { get; set; }
    }
    public sealed class SaverSnapshot {
        public List<Rect> Monitors { get; set; } = new List<Rect>();
        public List<SaverTarget> Targets { get; set; } = new List<SaverTarget>();
    }
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr window, int id);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr window);
    [DllImport("user32.dll")] static extern IntPtr GetShellWindow();
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr window, StringBuilder name, int capacity);
    [DllImport("user32.dll", EntryPoint="GetWindowLongPtrW")] public static extern IntPtr Style(IntPtr window, int index);
    [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flags);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int command);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] static extern int MapWindowPoints(IntPtr from, IntPtr to, ref Rect rect, uint count);
    [DllImport("user32.dll")] static extern bool GetGUIThreadInfo(uint thread, ref GuiInfo info);
    [DllImport("user32.dll")] static extern bool PostMessageW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] static extern bool EnumDisplayMonitors(IntPtr dc, IntPtr clip, MonitorCallback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern bool DestroyWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr CreateWindowExW(uint ex, string cls, string text, uint style, int x, int y, int width, int height, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr parameter);
    [DllImport("user32.dll")] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageTimeoutW")] static extern IntPtr ReadText(IntPtr window, uint message, IntPtr capacity, StringBuilder text, uint flags, uint timeout, out IntPtr result);
    public static string ClassName(IntPtr window) {
        var name = new StringBuilder(256);
        GetClassNameW(window, name, name.Capacity);
        return name.ToString();
    }
    public static uint Owner(IntPtr window) {
        GetWindowThreadProcessId(window, out uint process);
        return process;
    }
    public static GuiInfo Gui(IntPtr window, uint process) {
        uint thread = GetWindowThreadProcessId(window, out uint owner);
        if (window == IntPtr.Zero || owner != process) throw new Exception("Owned focus window retired");
        var info = new GuiInfo { Size = (uint)Marshal.SizeOf<GuiInfo>() };
        if (!GetGUIThreadInfo(thread, ref info)) throw new Exception("Owned UI thread state unavailable");
        return info;
    }
    public static Rect Bounds(IntPtr window, uint process, IntPtr parent) {
        if (Owner(window) != process || (parent != IntPtr.Zero && Owner(parent) != process))
            throw new Exception("Owned pane window retired");
        var previous = SetThreadDpiAwarenessContext((IntPtr)(-4));
        if (previous == IntPtr.Zero) throw new Exception("Physical pane coordinate query unavailable");
        try {
            if (!GetWindowRect(window, out var rect)) throw new Exception("Owned pane bounds unavailable");
            if (parent != IntPtr.Zero) MapWindowPoints(IntPtr.Zero, parent, ref rect, 2);
            return rect;
        } finally { SetThreadDpiAwarenessContext(previous); }
    }
    public static void Post(IntPtr window, uint process, uint message, long wparam=0, long lparam=0) {
        if (window == IntPtr.Zero || Owner(window) != process) throw new Exception("Owned queued-input window retired");
        if (!PostMessageW(window, message, (IntPtr)wparam, (IntPtr)lparam)) throw new Exception("Owned test message could not be queued");
    }
    public static SaverSnapshot SaverWindows(uint process) {
        // Read both monitor and popup bounds in physical per-monitor coordinates,
        // then restore only the calling harness thread's original DPI context.
        var previous = SetThreadDpiAwarenessContext((IntPtr)(-4));
        if (previous == IntPtr.Zero) throw new Exception("Physical monitor coordinate query unavailable");
        try {
            var snapshot = new SaverSnapshot();
            MonitorCallback monitor = (IntPtr handle, IntPtr dc, ref Rect bounds, IntPtr parameter) => {
                snapshot.Monitors.Add(bounds); return true;
            };
            if (!EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, monitor, IntPtr.Zero)) throw new Exception("Monitor enumeration failed");
            Callback target = (window, parameter) => {
                if (Owner(window) != process || ClassName(window) != "NetMap.ScreenSaver") return true;
                GetWindowRect(window, out var bounds);
                int children = 0;
                Callback child = (handle, unused) => { if (Owner(handle) == process) children++; return true; };
                EnumChildWindows(window, child, IntPtr.Zero);
                snapshot.Targets.Add(new SaverTarget { HWND = window.ToInt64(), Bounds = bounds,
                    Visible = IsWindowVisible(window), Popup = (Style(window, -16).ToInt64() & 0x80000000L) != 0,
                    Parentless = GetParent(window) == IntPtr.Zero, Children = children });
                return true;
            };
            if (!EnumWindows(target, IntPtr.Zero)) throw new Exception("Owned saver target enumeration failed");
            return snapshot;
        } finally { SetThreadDpiAwarenessContext(previous); }
    }
    public static IntPtr DesktopParent(IntPtr map, uint process) {
        GetWindowThreadProcessId(map, out uint owner);
        if (map == IntPtr.Zero || owner != process) throw new Exception("Owned map window retired");
        var parent = GetParent(map);
        GetWindowThreadProcessId(parent, out uint host);
        GetWindowThreadProcessId(GetShellWindow(), out uint shell);
        var cls = ClassName(parent);
        return shell != 0 && host == shell && (cls == "WorkerW" || cls == "Progman") ? parent : IntPtr.Zero;
    }
    public static string Text(IntPtr window, uint process) {
        GetWindowThreadProcessId(window, out uint owner);
        if (window == IntPtr.Zero || owner != process) throw new Exception("Owned text window retired");
        var text = new StringBuilder(8192);
        if (ReadText(window, 0xD, (IntPtr)text.Capacity, text, 0x22, 2000, out var result) == IntPtr.Zero)
            throw new Exception("Owned text window did not answer");
        return text.ToString();
    }
    public static IntPtr Find(uint process, IntPtr parent) {
        IntPtr found = IntPtr.Zero;
        Callback match = (window, parameter) => {
            GetWindowThreadProcessId(window, out uint owner);
            if (owner == process && (parent != IntPtr.Zero || GetDlgItem(window, 1000) != IntPtr.Zero || GetDlgItem(window, 1102) != IntPtr.Zero)) { found = window; return false; }
            return true;
        };
        if (parent == IntPtr.Zero) EnumWindows(match, IntPtr.Zero); else EnumChildWindows(parent, match, IntPtr.Zero);
        return found;
    }
    public static IntPtr Send(IntPtr window, uint process, uint message, long wparam=0, long lparam=0) {
        GetWindowThreadProcessId(window, out uint owner);
        if (window == IntPtr.Zero || owner != process) throw new Exception("Owned test window retired");
        if (SendMessageTimeoutW(window, message, (IntPtr)wparam, (IntPtr)lparam, 2, 2000, out var result) == IntPtr.Zero)
            throw new Exception("Owned test window did not answer");
        return result;
    }
}
'@

function Wait-Until([scriptblock] $Condition, [string] $Failure) {
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    throw $Failure
}

function Read-View([IntPtr] $Map, [int] $ProcessId) {
    $text = [Stage2Proof]::Text($Map, $ProcessId)
    if ($text -notmatch 'View: (?<Zoom>[0-9.]+)×; latitude (?<Latitude>-?[0-9.]+); longitude (?<Longitude>-?[0-9.]+)\.') {
        throw 'The owned map did not publish its viewport description'
    }
    [pscustomobject]@{
        Zoom = [double]::Parse($Matches.Zoom, [Globalization.CultureInfo]::InvariantCulture)
        Latitude = [double]::Parse($Matches.Latitude, [Globalization.CultureInfo]::InvariantCulture)
        Longitude = [double]::Parse($Matches.Longitude, [Globalization.CultureInfo]::InvariantCulture)
    }
}

function Read-Panes([IntPtr] $Window, [IntPtr] $Map, [IntPtr] $Table, [int] $ProcessId) {
    $mapBounds = [Stage2Proof]::Bounds($Map, $ProcessId, $Window)
    $tableBounds = [Stage2Proof]::Bounds($Table, $ProcessId, $Window)
    $stacked = $tableBounds.Top -ge $mapBounds.Bottom
    [pscustomobject]@{
        Map = $mapBounds
        Table = $tableBounds
        Stacked = $stacked
        X = if ($stacked) { [int](($mapBounds.Left + $mapBounds.Right) / 2) } else { [int](($mapBounds.Right + $tableBounds.Left) / 2) }
        Y = if ($stacked) { [int](($mapBounds.Bottom + $tableBounds.Top) / 2) } else { [int](($mapBounds.Top + $mapBounds.Bottom) / 2) }
    }
}

function Point-Parameter([int] $X, [int] $Y) {
    [long](($X -band 0xffff) -bor (($Y -band 0xffff) -shl 16))
}

function Same-Bounds($Left, $Right) {
    [Math]::Abs($Left.Left - $Right.Left) -le 1 -and [Math]::Abs($Left.Top - $Right.Top) -le 1 -and
        [Math]::Abs($Left.Right - $Right.Right) -le 1 -and [Math]::Abs($Left.Bottom - $Right.Bottom) -le 1
}

function Assert-Desktop([IntPtr] $Window, [IntPtr] $Map, [int] $ProcessId, [double] $Zoom) {
    Wait-Until {
        -not [Stage2Proof]::IsWindowVisible($Window) -and
            [Stage2Proof]::DesktopParent($Map, $ProcessId) -ne [IntPtr]::Zero
    } 'The owned map did not attach under Explorer while hiding its normal window'
    $client = [Stage2Proof+Rect]::new()
    if (-not [Stage2Proof]::IsWindowVisible($Map) -or -not [Stage2Proof]::GetClientRect($Map, [ref]$client) -or
        $client.Right -le 0 -or $client.Bottom -le 0) {
        throw 'The attached map was hidden or had an empty client area'
    }
    $style = [Stage2Proof]::Style($Map, -16).ToInt64()
    $extended = [Stage2Proof]::Style($Map, -20).ToInt64()
    if (($style -band 0x48000000) -ne 0x48000000 -or ($style -band 0x10000) -ne 0 -or
        ($extended -band 0x08000020) -ne 0x08000020) {
        throw 'The desktop map did not retain passive child/no-activate styles'
    }
    if ([Stage2Proof]::Send($Map, $ProcessId, 0x84).ToInt64() -ne -1 -or
        [Stage2Proof]::Send($Map, $ProcessId, 0x21).ToInt64() -ne 3) {
        throw 'The desktop map did not decline hit testing and mouse activation'
    }
    $view = Read-View $Map $ProcessId
    if ([Math]::Abs($view.Zoom - $Zoom) -gt 0.001) { throw 'Desktop attachment changed map zoom' }
}

function Read-Resources([Diagnostics.Process] $OwnedProcess, [string] $Label, [int] $Cycles) {
    $OwnedProcess.Refresh()
    [pscustomobject]@{
        Label = $Label
        CompletedCycles = $Cycles
        Handles = $OwnedProcess.HandleCount
        USER = [Stage2Proof]::GetGuiResources($OwnedProcess.Handle, 1)
        GDI = [Stage2Proof]::GetGuiResources($OwnedProcess.Handle, 0)
        PrivateBytes = $OwnedProcess.PrivateMemorySize64
        WorkingSetBytes = $OwnedProcess.WorkingSet64
    }
}

function Read-WindowDiagnostic([IntPtr] $Window, [int] $ProcessId) {
    $owner = [Stage2Proof]::Owner($Window)
    $state = [ordered]@{ HWND = $Window.ToInt64(); Owner = $owner; Owned = $Window -ne [IntPtr]::Zero -and $owner -eq $ProcessId }
    if ($state.Owned) {
        $parentWindow = [Stage2Proof]::GetParent($Window)
        $state.Class = [Stage2Proof]::ClassName($Window)
        $state.Visible = [Stage2Proof]::IsWindowVisible($Window)
        $state.Style = '0x{0:X}' -f [Stage2Proof]::Style($Window, -16).ToInt64()
        $state.ExtendedStyle = '0x{0:X}' -f [Stage2Proof]::Style($Window, -20).ToInt64()
        $state.ParentHWND = $parentWindow.ToInt64()
        $state.ParentClass = [Stage2Proof]::ClassName($parentWindow)
        $state.ParentOwner = [Stage2Proof]::Owner($parentWindow)
    }
    [pscustomobject]$state
}

$savedData = $env:LOCALAPPDATA
$process = $null
$window = [IntPtr]::Zero
$map = [IntPtr]::Zero
$parent = [IntPtr]::Zero
$cleanupMessage = 0x111
$cleanupCommand = 101
$results = [ordered]@{ ExecutableSHA256 = (Get-FileHash -LiteralPath $Executable).Hash; DesktopRequested = [bool]$Desktop; FullScreenRequested = [bool]$FullScreen; Checks = @(); Passed = $false }
try {
    $env:LOCALAPPDATA = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
    $process = Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru
    $window = [IntPtr]::Zero
    Wait-Until { $script:window = [Stage2Proof]::Find($process.Id, [IntPtr]::Zero); [Stage2Proof]::GetDlgItem($window, 1000) -ne [IntPtr]::Zero } 'Normal window did not initialize'
    [Stage2Proof]::ShowWindow($window, 4) | Out-Null
    Wait-Until {
        # The first ShowWindow obeys STARTUPINFO's hidden launch. Subsequent
        # calls explicitly show only this exact process's test presentation.
        [Stage2Proof]::ShowWindow($window, 4) | Out-Null
        [Stage2Proof]::IsWindowVisible($window)
    } 'Owned window did not become visible'
    $map = [Stage2Proof]::GetDlgItem($window, 1000)
    $table = [Stage2Proof]::GetDlgItem($window, 1004)
    if ($table -eq [IntPtr]::Zero) { throw 'Native connection list missing' }
    [Stage2Proof]::Send($window, $process.Id, 0x111, 114) | Out-Null
    if ([Stage2Proof]::IsWindowVisible($map) -or -not [Stage2Proof]::IsWindowVisible($table)) { throw 'Table-only visibility failed' }
    if ([Stage2Proof]::Gui($window, $process.Id).Focus -ne $table) { throw 'Table-only command did not focus the native list' }
    Wait-Until { [Stage2Proof]::Send($table, $process.Id, 0x1004).ToInt64() -gt 0 } 'Ordinary-token TCP rows did not appear'
    $rowCount = [Stage2Proof]::Send($table, $process.Id, 0x1004).ToInt64()
    [Stage2Proof]::Send($window, $process.Id, 0x111, 115) | Out-Null
    if (-not [Stage2Proof]::IsWindowVisible($map) -or -not [Stage2Proof]::IsWindowVisible($table)) { throw 'Map/table visibility failed' }
    $before = Read-Panes $window $map $table $process.Id
    $point = Point-Parameter $before.X $before.Y
    [Stage2Proof]::Send($window, $process.Id, 0x201, 1, $point) | Out-Null
    if ([Stage2Proof]::Gui($window, $process.Id).Capture -ne $window) { throw 'The owned split gutter did not acquire capture' }
    $moved = Point-Parameter ($before.X + $(if ($before.Stacked) { 0 } else { 40 })) ($before.Y + $(if ($before.Stacked) { 40 } else { 0 }))
    [Stage2Proof]::Send($window, $process.Id, 0x200, 1, $moved) | Out-Null
    [Stage2Proof]::Send($window, $process.Id, 0x202, 0, $moved) | Out-Null
    $committed = Read-Panes $window $map $table $process.Id
    if ((Same-Bounds $before.Map $committed.Map) -or (Same-Bounds $before.Table $committed.Table)) { throw 'Dragging the native gutter did not resize both panes' }
    if ([Stage2Proof]::Gui($window, $process.Id).Capture -ne [IntPtr]::Zero) { throw 'Finishing the split drag retained mouse capture' }
    [Stage2Proof]::Send($window, $process.Id, 0x201, 1, (Point-Parameter $committed.X $committed.Y)) | Out-Null
    $moved = Point-Parameter ($committed.X - $(if ($committed.Stacked) { 0 } else { 30 })) ($committed.Y - $(if ($committed.Stacked) { 30 } else { 0 }))
    [Stage2Proof]::Send($window, $process.Id, 0x200, 1, $moved) | Out-Null
    # Queue Escape through the ordinary message loop so navigation_key, rather
    # than a window-procedure approximation, cancels the captured split drag.
    [Stage2Proof]::Post($window, $process.Id, 0x100, 0x1b)
    Wait-Until {
        $restored = Read-Panes $window $map $table $process.Id
        (Same-Bounds $committed.Map $restored.Map) -and (Same-Bounds $committed.Table $restored.Table) -and
            [Stage2Proof]::Gui($window, $process.Id).Capture -eq [IntPtr]::Zero
    } 'Escape did not restore the initial split fraction and release capture'
    $results.Split = [ordered]@{ Before = $before; Committed = $committed; EscapeRestored = $true; Scope = 'Owned window messages and native bounds/focus/capture; physical pointer behavior remains separate.' }
    [Stage2Proof]::Send($window, $process.Id, 0x111, 117) | Out-Null
    [Stage2Proof]::Send($window, $process.Id, 0x111, 113) | Out-Null
    if (-not [Stage2Proof]::IsWindowVisible($map) -or [Stage2Proof]::IsWindowVisible($table)) { throw 'Map-only visibility failed' }
    if ($Desktop) {
        # Only these commands target the owned app. Explorer's windows are
        # queried for structural identity; the harness never messages them.
        $results.DesktopCyclesCompleted = 0
        $results.DesktopProcessId = $process.Id
        $results.DesktopNormalHWND = $window.ToInt64()
        $results.DesktopMapHWND = $map.ToInt64()
        $results.DesktopResources = @()
        $results.DesktopAcceptanceScope = 'Structural attachment, passive native contracts, repeated owned host transitions and viewport preservation; visual input, mixed DPI, Explorer restart and memory acceptance remain separate.'
        Wait-Until { [Stage2Proof]::Text($map, $process.Id) -match 'View: ' } 'Initial map world/view did not become ready'
        [Stage2Proof]::Send($window, $process.Id, 0x111, 104) | Out-Null
        $expectedZoom = 1.0
        [Stage2Proof]::Send($window, $process.Id, 0x111, 111) | Out-Null
        Assert-Desktop $window $map $process.Id $expectedZoom
        $desktopParent = [Stage2Proof]::GetParent($map)
        $results.DesktopParentClass = [Stage2Proof]::ClassName($desktopParent)
        $results.DesktopResources += Read-Resources $process 'Initial fitted desktop' 0
        $deadline = [DateTime]::UtcNow.AddMinutes(3)
        for ($cycle = 1; $cycle -le 100; $cycle++) {
            if ([DateTime]::UtcNow -ge $deadline) { throw 'The 100-cycle desktop check exceeded its three-minute deadline' }
            [Stage2Proof]::Send($window, $process.Id, 0x111, 112) | Out-Null
            Wait-Until {
                [Stage2Proof]::IsWindowVisible($window) -and [Stage2Proof]::GetParent($map) -eq $window
            } "Open map did not restore its original native parent at cycle $cycle"
            if ([Stage2Proof]::GetDlgItem($window, 1000) -ne $map) { throw 'A host transition unexpectedly replaced the owned map HWND' }
            if (([Stage2Proof]::Style($map, -16).ToInt64() -band 0x08000000) -ne 0) { throw 'Open map remained disabled' }
            if ([Math]::Abs((Read-View $map $process.Id).Zoom - $expectedZoom) -gt 0.001) { throw 'Open map changed map zoom' }
            if ($cycle -eq 51) {
                # Menu commands use the production zoom owner; this cap also
                # fails promptly if the map never acquired a drawable extent.
                for ($step = 0; $step -lt 40 -and (Read-View $map $process.Id).Zoom -lt 128.0; $step++) {
                    [Stage2Proof]::Send($window, $process.Id, 0x111, 102) | Out-Null
                }
                $expectedZoom = 128.0
                if ((Read-View $map $process.Id).Zoom -ne $expectedZoom) { throw 'The ordinary map did not reach maximum zoom' }
            }
            [Stage2Proof]::Send($window, $process.Id, 0x10) | Out-Null
            Assert-Desktop $window $map $process.Id $expectedZoom
            if ($process.HasExited) { throw 'Closing the desktop-enabled normal window exited the application' }
            $results.DesktopCyclesCompleted = $cycle
            if ($cycle -in @(10, 50, 60, 100)) {
                # Discrete samples are observations, not an allocator peak or
                # resource acceptance benchmark. No arbitrary growth threshold.
                Start-Sleep -Milliseconds 250
                $results.DesktopResources += Read-Resources $process "Desktop zoom $expectedZoom" $cycle
            }
        }
        # The retained table stays under our hidden normal window. Its bounded
        # native reply establishes responsiveness after cycling, not ETW coverage.
        $results.RowsAfterDesktopCycles = [Stage2Proof]::Send($table, $process.Id, 0x1004).ToInt64()
        [Stage2Proof]::Send($window, $process.Id, 0x111, 101) | Out-Null
        $results.Checks += 'Desktop enable; 100 Open map/close-to-desktop cycles (50 fitted, 50 zoom 128); retained map HWND; passive styles/hit-test contracts; final Exit from desktop'
    } else {
        [Stage2Proof]::Send($window, $process.Id, 0x10) | Out-Null
    }
    if (-not $process.WaitForExit(10000) -or $process.ExitCode -ne 0) { throw 'Normal close failed' }
    $results.Checks += "Normal map/table commands, $rowCount ordinary-token rows, clean exit"
    $process.Dispose(); $process = $null; $window = [IntPtr]::Zero

    $process = Start-Process -FilePath $Executable -ArgumentList '/c' -WindowStyle Hidden -PassThru
    $configuration = [IntPtr]::Zero
    Wait-Until { $script:configuration = [Stage2Proof]::Find($process.Id, [IntPtr]::Zero); [Stage2Proof]::GetDlgItem($configuration, 1102) -ne [IntPtr]::Zero } 'Standalone Settings did not initialize'
    $categories = [Stage2Proof]::GetDlgItem($configuration, 1102)
    if ([Stage2Proof]::Send($categories, $process.Id, 0x1004).ToInt64() -ne 5) { throw 'Standalone Settings category count is not five' }
    if ([Stage2Proof]::GetDlgItem($configuration, 1000) -ne [IntPtr]::Zero) { throw 'Configuration unexpectedly opened a map' }
    [Stage2Proof]::Send($configuration, $process.Id, 0x111, 2) | Out-Null
    if (-not $process.WaitForExit(10000) -or $process.ExitCode -ne 0) { throw 'Standalone Settings close failed' }
    $results.Checks += 'Standalone configuration, five native categories, no map, clean exit'
    $process.Dispose(); $process = $null

    $parent = [Stage2Proof]::CreateWindowExW(0, 'STATIC', 'NetMap owned preview check', 0x10CF0000, 20, 20, 360, 240, [IntPtr]::Zero, [IntPtr]::Zero, [IntPtr]::Zero, [IntPtr]::Zero)
    if ($parent -eq [IntPtr]::Zero) { throw 'Owned preview parent creation failed' }
    $process = Start-Process -FilePath $Executable -ArgumentList @('/p', $parent.ToInt64().ToString()) -WindowStyle Hidden -PassThru
    $preview = [IntPtr]::Zero
    Wait-Until { $script:preview = [Stage2Proof]::Find($process.Id, $parent); $preview -ne [IntPtr]::Zero } 'Preview child missing'
    [Stage2Proof]::SetWindowPos($parent, [IntPtr]::Zero, 20, 20, 500, 300, 0x14) | Out-Null
    Wait-Until {
        $outer = [Stage2Proof+Rect]::new(); $inner = [Stage2Proof+Rect]::new()
        [Stage2Proof]::GetClientRect($parent, [ref]$outer) -and [Stage2Proof]::GetClientRect($preview, [ref]$inner) -and $outer.Right -eq $inner.Right -and $outer.Bottom -eq $inner.Bottom
    } 'Preview did not follow parent client size'
    [Stage2Proof]::DestroyWindow($parent) | Out-Null; $parent = [IntPtr]::Zero
    if (-not $process.WaitForExit(10000) -or $process.ExitCode -ne 0) { throw 'Preview parent teardown failed' }
    $results.Checks += 'Preview child, parent resize, parent teardown and clean exit'
    $process.Dispose(); $process = $null

    if ($FullScreen) {
        # This optional visible launch is a bounded owned saver target check.
        # It does not install/select a saver, inject global input or test lock.
        $process = Start-Process -FilePath $Executable -ArgumentList '/s' -WindowStyle Normal -PassThru
        $cleanupMessage = 0x10
        $cleanupCommand = 0
        $saver = $null
        Wait-Until {
            if ($process.HasExited) { throw 'The owned full-screen saver exited before structural verification' }
            $script:saver = [Stage2Proof]::SaverWindows($process.Id)
            if ($saver.Targets.Count -gt 0) { $script:window = [IntPtr]$saver.Targets[0].HWND }
            $saver.Monitors.Count -gt 0 -and $saver.Targets.Count -eq $saver.Monitors.Count -and
                @($saver.Targets | Where-Object { -not $_.Visible }).Count -eq 0
        } 'Full-screen targets did not initialize on every reported display'
        $remaining = [Collections.Generic.List[object]]::new()
        foreach ($monitor in $saver.Monitors) { $remaining.Add($monitor) }
        foreach ($target in $saver.Targets) {
            $match = @($remaining | Where-Object { Same-Bounds $_ $target.Bounds })
            if ($match.Count -ne 1 -or -not $target.Popup -or -not $target.Parentless -or $target.Children -ne 0) {
                throw 'An owned full-screen target did not cover exactly one display without exploration controls'
            }
            $remaining.Remove($match[0]) | Out-Null
        }
        $results.FullScreen = $saver
        $results.FullScreenScope = 'Owned top-level popup rectangles cover reported monitors, no child exploration controls, owned Escape dismissal; physical input, pixels, sleep and sign-in remain separate.'
        [Stage2Proof]::Send([IntPtr]$saver.Targets[0].HWND, $process.Id, 0x100, 0x1b) | Out-Null
        if (-not $process.WaitForExit(10000) -or $process.ExitCode -ne 0) { throw 'Owned full-screen Escape dismissal failed' }
        $results.Checks += 'Standalone /s targets cover reported monitors; no exploration children; owned key-message dismissal and clean exit'
    }
    $results.Passed = $true
} catch {
    $results.Error = $_.Exception.Message
    if ($Desktop -and $null -ne $process) {
        try {
            # Capture the product's own failure before Exit restores/retires its
            # windows. WM_GETTEXT is supported by the single-part status bar;
            # unlike SB_GETTEXT it safely marshals the buffer across processes.
            $diagnostic = [ordered]@{
                ProcessAlive = -not $process.HasExited
                Normal = Read-WindowDiagnostic $window $process.Id
                Map = Read-WindowDiagnostic $map $process.Id
            }
            if ([Stage2Proof]::Owner($window) -eq $process.Id) {
                $status = [Stage2Proof]::GetDlgItem($window, 1001)
                try { $diagnostic.Status = [Stage2Proof]::Text($status, $process.Id) }
                catch { $diagnostic.StatusReadError = $_.Exception.Message }
            }
            $results.DesktopFailure = $diagnostic
        } catch {
            $results.DesktopDiagnosticError = $_.Exception.Message
        }
    }
    throw
} finally {
    if ($parent -ne [IntPtr]::Zero) { [Stage2Proof]::DestroyWindow($parent) | Out-Null }
    if ($null -ne $process) {
        if (-not $process.HasExited -and $window -ne [IntPtr]::Zero) {
            try {
                # The normal app's Exit bypasses close-to-desktop; the saver
                # uses its own WM_CLOSE path. Both retire owned native targets.
                [Stage2Proof]::Send($window, $process.Id, $cleanupMessage, $cleanupCommand) | Out-Null
                $process.WaitForExit(5000) | Out-Null
            } catch { }
        }
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }
        $process.Dispose()
    }
    $env:LOCALAPPDATA = $savedData
    $results | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json')
}
