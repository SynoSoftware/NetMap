#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $EvidenceDirectory
)

# Uses only the window belonging to the process launched below. No global
# keyboard hook, connection fixture, process-name stop or other app's capture.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class NavigationProof {
    public static uint Owner;
    delegate bool Callback(IntPtr window, IntPtr parameter);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] struct MouseInput { public int X, Y; public uint Data, Flags, Time; public UIntPtr Extra; }
    [StructLayout(LayoutKind.Sequential)] struct KeyInput { public ushort Key, Scan; public uint Flags, Time; public UIntPtr Extra; }
    [StructLayout(LayoutKind.Explicit, Size=40)] struct Input {
        [FieldOffset(0)] public uint Type;
        [FieldOffset(8)] public MouseInput Mouse;
        [FieldOffset(8)] public KeyInput Keyboard;
    }
    [StructLayout(LayoutKind.Sequential)] struct CursorInfo { public uint Size, Flags; public IntPtr Cursor; public Point Position; }
    [StructLayout(LayoutKind.Sequential)] public struct GuiThread {
        public uint Size, Flags;
        public IntPtr Active, Focus, Capture, MenuOwner, MoveSize, Caret;
        public Rect CaretRect;
    }
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr window, int id);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int command);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint thread, ref GuiThread info);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr window, ref Point point);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern uint SendInput(uint count, Input[] inputs, int size);
    [DllImport("user32.dll")] static extern bool GetCursorInfo(ref CursorInfo info);
    [DllImport("user32.dll")] static extern IntPtr LoadCursorW(IntPtr instance, IntPtr name);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] static extern IntPtr GetMenu(IntPtr window);
    [DllImport("user32.dll")] static extern IntPtr GetSubMenu(IntPtr menu, int index);
    [DllImport("user32.dll")] static extern uint GetMenuState(IntPtr menu, uint command, uint flags);
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
    public static IntPtr Send(IntPtr window, uint message, long wparam=0, long lparam=0) {
        if (!Owned(window)) throw new InvalidOperationException("Test window was retired");
        if (SendMessageTimeoutW(window, message, (IntPtr)wparam, (IntPtr)lparam, 2, 2000, out var result) == IntPtr.Zero)
            throw new InvalidOperationException("Owned window did not answer message " + message);
        return result;
    }
    public static uint ViewCommandState(IntPtr window, uint command) {
        if (!Owned(window)) throw new InvalidOperationException("Owned window retired");
        var menu = GetSubMenu(GetMenu(window), 1);
        if (menu == IntPtr.Zero) throw new InvalidOperationException("View menu missing");
        Send(window, 0x117, menu.ToInt64(), 1);
        var state = GetMenuState(menu, command, 0);
        if (state == uint.MaxValue) throw new InvalidOperationException("View command missing");
        return state;
    }
    public static string Text(IntPtr window) {
        if (!Owned(window)) throw new InvalidOperationException("Test control was retired");
        var value = new StringBuilder(4096);
        if (SendMessageTimeoutW(window, 13, (IntPtr)4096, value, 2, 2000, out _) == IntPtr.Zero)
            throw new InvalidOperationException("Owned control did not answer text request");
        return value.ToString();
    }
    public static GuiThread Thread(IntPtr window) {
        if (!Owned(window)) throw new InvalidOperationException("Test window was retired");
        var info = new GuiThread { Size = (uint)Marshal.SizeOf<GuiThread>() };
        if (!GetGUIThreadInfo(GetWindowThreadProcessId(window, out _), ref info))
            throw new InvalidOperationException("Owned UI thread unavailable");
        return info;
    }
    public static long Position(int x, int y) => (ushort)x | ((long)(ushort)y << 16);
    public static bool Owned(IntPtr window) {
        GetWindowThreadProcessId(window, out uint process);
        return window != IntPtr.Zero && process == Owner;
    }
    public static void Button(bool down) {
        var input = new Input { Type = 0, Mouse = new MouseInput { Flags = down ? 2u : 4u } };
        if (SendInput(1, new[] { input }, Marshal.SizeOf<Input>()) != 1)
            throw new InvalidOperationException("Owned-window mouse input was rejected");
    }
    public static bool Cursor(int id) {
        var info = new CursorInfo { Size = (uint)Marshal.SizeOf<CursorInfo>() };
        return GetCursorInfo(ref info) && info.Cursor == LoadCursorW(IntPtr.Zero, (IntPtr)id);
    }
    public static void Key(ushort key, bool control) {
        var down = new Input { Type = 1, Keyboard = new KeyInput { Key = key } };
        var up = new Input { Type = 1, Keyboard = new KeyInput { Key = key, Flags = 2 } };
        var inputs = control ? new[] {
            new Input { Type = 1, Keyboard = new KeyInput { Key = 0x11 } }, down, up,
            new Input { Type = 1, Keyboard = new KeyInput { Key = 0x11, Flags = 2 } }
        } : new[] { down, up };
        if (SendInput((uint)inputs.Length, inputs, Marshal.SizeOf<Input>()) != inputs.Length)
            throw new InvalidOperationException("Owned-window keyboard input was rejected");
    }
}
'@

function Get-ProofImage([IntPtr] $Map, [string] $Path) {
    $rect = [NavigationProof+Rect]::new()
    $point = [NavigationProof+Point]::new()
    if (-not [NavigationProof]::GetClientRect($Map, [ref]$rect) -or
        -not [NavigationProof]::ClientToScreen($Map, [ref]$point)) { throw 'Map rectangle unavailable.' }
    # Exclude native navigation controls; compare only the rendered map.
    $bitmap = [Drawing.Bitmap]::new($rect.Right, [Math]::Max(1, $rect.Bottom - 80))
    try {
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try { $graphics.CopyFromScreen($point.X, $point.Y, 0, 0, $bitmap.Size) }
        finally { $graphics.Dispose() }
        $bitmap.Save($Path, [Drawing.Imaging.ImageFormat]::Png)
        (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
    }
    finally { $bitmap.Dispose() }
}

function Assert-Proof([bool] $Condition, [string] $Failure) {
    if (-not $Condition) { throw $Failure }
}
function Get-ProofView([IntPtr] $Map) {
    $view = [regex]::Match([NavigationProof]::Text($Map), 'View: [-0-9.]+×; latitude [-0-9.]+; longitude [-0-9.]+\.')
    if (-not $view.Success) { throw 'Map does not expose its current viewport.' }
    $view.Value
}
function Wait-Proof([scriptblock] $Condition, [string] $Failure) {
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $deadline)
    throw $Failure
}

$profile = Join-Path $EvidenceDirectory ('profile-' + [Guid]::NewGuid().ToString('N'))
$data = Join-Path $profile 'NetMap'
New-Item -ItemType Directory -Path $data | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $data 'preferences.txt')
$start = [Diagnostics.ProcessStartInfo]::new($Executable)
$start.UseShellExecute = $false
$start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
$start.Environment['LOCALAPPDATA'] = $profile
$process = [Diagnostics.Process]::Start($start)
[NavigationProof]::Owner = $process.Id
$window = [IntPtr]::Zero
$mouseDown = $false
$savedCursor = [NavigationProof+Point]::new()
[void][NavigationProof]::GetCursorPos([ref]$savedCursor)
try {
    Assert-Proof ($process.WaitForInputIdle(5000)) 'The launched window did not become input idle.'
    $window = [NavigationProof]::Find($process.Id)
    Assert-Proof ($window -ne [IntPtr]::Zero) 'No map window found for the launched PID.'
    $map = [NavigationProof]::GetDlgItem($window, 1000)
    $status = [NavigationProof]::GetDlgItem($window, 1001)
    $fit = [NavigationProof]::GetDlgItem($window, 1002)
    $zoom = [NavigationProof]::GetDlgItem($window, 1003)
    Wait-Proof { [NavigationProof]::Text($status) -match 'observed connections' } 'Live map did not finish loading.'
    [void][NavigationProof]::ShowWindow($window, 5)
    Assert-Proof ([NavigationProof]::SetWindowPos($window, [IntPtr](-1), 40, 40, 1100, 760, 0x40)) 'Could not position the owned window.'
    [void][NavigationProof]::SetForegroundWindow($window)
    Wait-Proof { [NavigationProof]::GetForegroundWindow() -eq $window } 'Owned window did not receive foreground input.'
    Wait-Proof { [NavigationProof]::Thread($window).Focus -eq $map } 'Map did not receive initial focus.'
    Assert-Proof (-not [NavigationProof]::IsWindowVisible($fit)) 'Fit is visible in the fitted overview.'
    Assert-Proof (([NavigationProof]::ViewCommandState($window, 108) -band 8) -ne 0) 'Countries is not the checked detail command.'
    Assert-Proof (([NavigationProof]::ViewCommandState($window, 109) -band 3) -ne 0) 'Cities is enabled without usable city data.'
    [void][NavigationProof]::Send($window, 0x111, 109)
    Assert-Proof (([NavigationProof]::ViewCommandState($window, 108) -band 8) -ne 0) 'Unavailable Cities command changed committed detail.'

    for ($index = 0; $index -lt 4; $index++) { [void][NavigationProof]::Send($window, 0x111, 102) }
    Wait-Proof { [NavigationProof]::Text($zoom) -eq '2.4×' } 'Zoom menu did not change the viewport.'
    Assert-Proof ([NavigationProof]::IsWindowVisible($fit)) 'Fit was not shown after zooming.'
    Start-Sleep -Milliseconds 700
    Get-ProofImage $map (Join-Path $EvidenceDirectory 'zoom.png') | Out-Null
    $before = Get-ProofView $map
    Assert-Proof ([NavigationProof]::GetForegroundWindow() -eq $window) 'Foreground changed before the owned-window drag.'
    $start = [NavigationProof+Point]::new(); $start.X = 500; $start.Y = 250
    $end = [NavigationProof+Point]::new(); $end.X = 620; $end.Y = 300
    [void][NavigationProof]::ClientToScreen($map, [ref]$start)
    [void][NavigationProof]::ClientToScreen($map, [ref]$end)
    [void][NavigationProof]::SetCursorPos($start.X, $start.Y)
    [NavigationProof]::Button($true); $mouseDown = $true
    Start-Sleep -Milliseconds 50
    [void][NavigationProof]::SetCursorPos($end.X, $end.Y)
    Wait-Proof { [NavigationProof]::Thread($window).Capture -eq $map } 'Pan did not capture the mouse.'
    Wait-Proof { [NavigationProof]::Cursor(32646) } 'Captured pan did not show the native pan cursor.'
    Start-Sleep -Milliseconds 700
    Get-ProofImage $map (Join-Path $EvidenceDirectory 'pan.png') | Out-Null
    Assert-Proof ([NavigationProof]::Text($zoom) -eq '2.4×') 'The drag changed zoom unexpectedly.'
    Assert-Proof ((Get-ProofView $map) -ne $before) 'Captured pan did not change the viewport.'
    [void][NavigationProof]::PostMessageW($map, 0x100, [IntPtr]0x1B, [IntPtr]::Zero)
    Wait-Proof { [NavigationProof]::Thread($window).Capture -eq [IntPtr]::Zero } 'Escape did not release capture.'
    [NavigationProof]::Button($false); $mouseDown = $false
    Wait-Proof { [NavigationProof]::Cursor(32512) } 'Cancel did not restore the native arrow cursor.'
    Start-Sleep -Milliseconds 700
    Get-ProofImage $map (Join-Path $EvidenceDirectory 'cancel.png') | Out-Null
    Assert-Proof ((Get-ProofView $map) -eq $before) 'Escape did not restore the viewport.'
    Assert-Proof ([NavigationProof]::GetForegroundWindow() -eq $window) 'Foreground changed before owned-window keyboard input.'
    [NavigationProof]::Key(0x27, $true)
    Start-Sleep -Milliseconds 700
    Get-ProofImage $map (Join-Path $EvidenceDirectory 'keyboard-pan.png') | Out-Null
    Assert-Proof ((Get-ProofView $map) -ne $before) 'Ctrl+Right did not pan.'
    [NavigationProof]::Key(0x25, $true)
    Start-Sleep -Milliseconds 700
    Get-ProofImage $map (Join-Path $EvidenceDirectory 'keyboard-restored.png') | Out-Null
    Assert-Proof ((Get-ProofView $map) -eq $before) 'Opposite horizontal keyboard pans did not restore the view.'

    [void][NavigationProof]::PostMessageW($map, 0x100, [IntPtr]9, [IntPtr]::Zero)
    Wait-Proof { [NavigationProof]::Thread($window).Focus -eq $fit } 'Tab did not reach the native Fit button.'
    $point = [NavigationProof+Point]::new()
    $point.X = 500; $point.Y = 250
    Assert-Proof ([NavigationProof]::ClientToScreen($map, [ref]$point)) 'Map screen anchor unavailable.'
    [void][NavigationProof]::Send($fit, 0x20A, (120 -shl 16), [NavigationProof]::Position($point.X, $point.Y))
    Wait-Proof { [NavigationProof]::Text($zoom) -eq '3.1×' } 'Wheel input with Fit focused did not reach the map.'
    [void][NavigationProof]::Send($fit, 0xF5)
    Wait-Proof { -not [NavigationProof]::IsWindowVisible($fit) } 'Fit button did not restore the overview.'
    Assert-Proof ([NavigationProof]::Thread($window).Focus -eq $map) 'Fit did not restore map focus.'
    [void][NavigationProof]::PostMessageW($map, 0x100, [IntPtr]0x6B, [IntPtr]::Zero)
    Wait-Proof { [NavigationProof]::Text($zoom) -eq '1.2×' } 'Numpad Plus did not zoom.'
    [void][NavigationProof]::PostMessageW($map, 0x100, [IntPtr]0x6D, [IntPtr]::Zero)
    Wait-Proof { [NavigationProof]::Text($zoom) -eq '1.0×' } 'Numpad Minus did not return to 1×.'
    # Minimum zoom can retain a geographic offset, just as in the reference.
    # Fit is the command that guarantees a fitted overview.
    [void][NavigationProof]::Send($window, 0x111, 104)
    Wait-Proof { -not [NavigationProof]::IsWindowVisible($fit) } 'Fit did not restore the overview after zoom out.'
    [void][NavigationProof]::Send($map, 0x203, 1, [NavigationProof]::Position(500, 250))
    Wait-Proof { [NavigationProof]::IsWindowVisible($fit) } 'Double-click did not zoom.'
    Assert-Proof ([NavigationProof]::GetForegroundWindow() -eq $window) 'Foreground changed before Ctrl+0.'
    [NavigationProof]::Key(0x30, $true)
    Wait-Proof { -not [NavigationProof]::IsWindowVisible($fit) } 'Ctrl+0 did not restore the overview.'
    if ([NavigationProof]::GetForegroundWindow() -eq $window) {
        [void][NavigationProof]::SetCursorPos($savedCursor.X, $savedCursor.Y)
    }
    [void][NavigationProof]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
    Assert-Proof ($process.WaitForExit(5000)) 'Owned process did not exit after Close.'
    Assert-Proof ($process.ExitCode -eq 0) 'Owned process exited with a failure.'
    [pscustomobject]@{
        Process = $process.Id
        ZoomMenu = $true; CapturedPan = $true; EscapeRestoresView = $true
        DetailMenuAvailability = $true; UnavailableCitiesRejected = $true
        NativeTabAndFit = $true; FocusedButtonWheel = $true
        NumpadZoom = $true; DoubleClickZoom = $true; NativePanCursor = $true
        ControlArrowPan = $true; ControlZeroFit = $true; ExitCode = $process.ExitCode
        Evidence = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
    } | ConvertTo-Json
}
finally {
    if ($mouseDown) { [NavigationProof]::Button($false) }
    if ($window -ne [IntPtr]::Zero -and [NavigationProof]::GetForegroundWindow() -eq $window) {
        [void][NavigationProof]::SetCursorPos($savedCursor.X, $savedCursor.Y)
    }
    if (-not $process.HasExited) {
        if ([NavigationProof]::Owned($window)) { [void][NavigationProof]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) }
        if (-not $process.WaitForExit(5000)) { $process.Kill(); $process.WaitForExit() }
    }
    $process.Dispose()
}
