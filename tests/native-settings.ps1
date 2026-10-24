#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [switch] $Keyboard,
    [switch] $AutomaticOrigin,
    [switch] $Hosting,
    [switch] $SaverPreview,
    [switch] $Accessibility,
    [switch] $Lifetime
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ($SaverPreview -and ($Keyboard -or $AutomaticOrigin -or $Hosting)) {
    throw 'SaverPreview is a focused journey; do not combine it with the general Settings switches.'
}
if ($Accessibility -and ($Keyboard -or $AutomaticOrigin -or $Hosting -or $SaverPreview)) {
    throw 'Accessibility is a focused journey; do not combine it with other Settings switches.'
}
if ($Lifetime -and ($Keyboard -or $AutomaticOrigin -or $Hosting -or $SaverPreview -or $Accessibility)) {
    throw 'Lifetime is a focused journey; do not combine it with other Settings switches.'
}
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory and isolated profile.' }
New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$data = Join-Path $EvidenceDirectory 'NetMap'
New-Item -ItemType Directory -Path $data | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $data 'preferences.txt')
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class SettingsProof {
    public static uint Owner;
    delegate bool Callback(IntPtr window, IntPtr parameter);
    delegate bool MonitorCallback(IntPtr monitor, IntPtr dc, IntPtr rect, IntPtr parameter);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct MonitorInfo { public uint Size; public Rect Monitor, Work; public uint Flags; }
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct MonitorInfoEx {
        public uint Size; public Rect Monitor, Work; public uint Flags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string Device;
    }
    [StructLayout(LayoutKind.Sequential)] public struct GuiThread {
        public uint Size, Flags;
        public IntPtr Active, Focus, Capture, MenuOwner, MoveSize, Caret;
        public Rect CaretRect;
    }
    [StructLayout(LayoutKind.Sequential)] struct KeyInput { public ushort Key, Scan; public uint Flags, Time; public UIntPtr Extra; }
    [StructLayout(LayoutKind.Explicit, Size=40)] struct Input {
        [FieldOffset(0)] public uint Type;
        [FieldOffset(8)] public KeyInput Keyboard;
    }
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, Callback callback, IntPtr parameter);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr window, StringBuilder name, int capacity);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr parent, int id);
    [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] static extern bool ShowWindowAsync(IntPtr window, int command);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] static extern IntPtr MonitorFromWindow(IntPtr window, uint flags);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern bool GetMonitorInfoW(IntPtr monitor, ref MonitorInfo info);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern bool GetMonitorInfoW(IntPtr monitor, ref MonitorInfoEx info);
    [DllImport("user32.dll")] static extern bool EnumDisplayMonitors(IntPtr dc, IntPtr rect, MonitorCallback callback, IntPtr parameter);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr window);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr window, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint thread, ref GuiThread info);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern uint SendInput(uint count, Input[] inputs, int size);
    [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint kind);
    [DllImport("user32.dll")] static extern bool PostMessageW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, IntPtr wparam, StringBuilder text, uint flags, uint timeout, out IntPtr result);
    public static bool Owned(IntPtr window) {
        GetWindowThreadProcessId(window, out uint owner);
        return window != IntPtr.Zero && owner == Owner;
    }
    public static void ShowOwned(IntPtr window) {
        if (!Owned(window)) throw new InvalidOperationException("Retired owned presentation");
        ShowWindowAsync(window, 4);
    }
    public static IntPtr Find(int child) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, parameter) => {
            if (Owned(window) && GetDlgItem(window, child) != IntPtr.Zero) { found = window; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static IntPtr[] SaverWindows(uint process) {
        var result = new List<IntPtr>();
        EnumWindows((window, parameter) => {
            if (SaverWindow(window, process)) result.Add(window);
            return true;
        }, IntPtr.Zero);
        return result.ToArray();
    }
    static bool SaverWindow(IntPtr window, uint process) {
        GetWindowThreadProcessId(window, out uint owner);
        var name = new StringBuilder(256);
        return window != IntPtr.Zero && owner == process && GetClassNameW(window, name, name.Capacity) != 0
            && name.ToString() == "NetMap.ScreenSaver";
    }
    public static void CloseSaver(IntPtr window, uint process) {
        if (!SaverWindow(window, process) || !PostMessageW(window, 0x10, IntPtr.Zero, IntPtr.Zero))
            throw new InvalidOperationException("Unable to close the retained process's saver window");
    }
    public static IntPtr Child(IntPtr parent, int id) {
        if (!Owned(parent)) throw new InvalidOperationException("Retired test dialog");
        IntPtr found = IntPtr.Zero;
        EnumChildWindows(parent, (window, parameter) => {
            if (GetDlgCtrlID(window) == id) { found = window; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static IntPtr Send(IntPtr window, uint message, long wparam=0, long lparam=0) {
        if (!Owned(window)) throw new InvalidOperationException("Retired test HWND");
        if (SendMessageTimeoutW(window, message, (IntPtr)wparam, (IntPtr)lparam, 2, 2000, out var result) == IntPtr.Zero)
            throw new InvalidOperationException("Owned UI did not answer " + message);
        return result;
    }
    public static void Post(IntPtr window, uint message, long wparam=0) {
        if (!Owned(window) || !PostMessageW(window, message, (IntPtr)wparam, IntPtr.Zero))
            throw new InvalidOperationException("Unable to post to owned window");
    }
    public static void SetText(IntPtr window, string text) {
        if (!Owned(window)) throw new InvalidOperationException("Retired edit");
        var pointer = Marshal.StringToHGlobalUni(text);
        try { Send(window, 12, 0, pointer.ToInt64()); }
        finally { Marshal.FreeHGlobal(pointer); }
    }
    public static string Text(IntPtr window) {
        if (!Owned(window)) throw new InvalidOperationException("Retired control");
        var value = new StringBuilder(4096);
        if (SendMessageTimeoutW(window, 13, (IntPtr)value.Capacity, value, 2, 2000, out _) == IntPtr.Zero)
            throw new InvalidOperationException("Owned UI did not answer text query");
        return value.ToString();
    }
    public static IntPtr Focus(IntPtr window) {
        if (!Owned(window)) throw new InvalidOperationException("Retired dialog");
        var info = new GuiThread { Size = (uint)Marshal.SizeOf<GuiThread>() };
        if (!GetGUIThreadInfo(GetWindowThreadProcessId(window, out _), ref info)) throw new InvalidOperationException("UI thread missing");
        return info.Focus;
    }
    public static void Resize(IntPtr window, int height) {
        if (!Owned(window) || !GetWindowRect(window, out var rect) || !SetWindowPos(window, IntPtr.Zero, 0, 0, rect.Right - rect.Left, height, 0x16))
            throw new InvalidOperationException("Owned resize failed");
    }
    public static void ResizeTo(IntPtr window, int width, int height, bool force=false) {
        if (!Owned(window) || !SetWindowPos(window, IntPtr.Zero, 0, 0, width, height, force ? 0x416u : 0x16u))
            throw new InvalidOperationException("Owned resize failed");
    }
    public static bool WithinWorkArea(IntPtr window) {
        if (!Owned(window)) throw new InvalidOperationException("Retired shell");
        var info = new MonitorInfo { Size = (uint)Marshal.SizeOf<MonitorInfo>() };
        return GetWindowRect(window, out var rect) && GetMonitorInfoW(MonitorFromWindow(window, 2), ref info)
            && rect.Left >= info.Work.Left && rect.Top >= info.Work.Top && rect.Right <= info.Work.Right && rect.Bottom <= info.Work.Bottom;
    }
    public static Rect DisplayRect(string device) {
        bool found = false;
        Rect result = default;
        if (!EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, (monitor, dc, rect, parameter) => {
            var info = new MonitorInfoEx { Size = (uint)Marshal.SizeOf<MonitorInfoEx>() };
            if (!GetMonitorInfoW(monitor, ref info)) throw new InvalidOperationException("Display information unavailable");
            if (String.Equals(info.Device, device, StringComparison.Ordinal)) { result = info.Monitor; found = true; }
            return true;
        }, IntPtr.Zero) || !found) throw new InvalidOperationException("Selected display is no longer connected");
        return result;
    }
    public static bool FitsHorizontally(IntPtr child) {
        if (!Owned(child)) throw new InvalidOperationException("Retired page child");
        var parent = GetParent(child);
        return GetWindowRect(child, out var rect) && GetWindowRect(parent, out var outer) && GetClientRect(parent, out var client)
            && rect.Left >= outer.Left && rect.Right <= outer.Left + client.Right;
    }
    public static bool Above(IntPtr label, IntPtr editor) {
        if (!Owned(label) || !Owned(editor)) throw new InvalidOperationException("Retired form row");
        return GetWindowRect(label, out var a) && GetWindowRect(editor, out var b) && a.Bottom <= b.Top;
    }
    public static bool Overlap(IntPtr first, IntPtr second) {
        if (!Owned(first) || !Owned(second)) throw new InvalidOperationException("Retired command");
        return GetWindowRect(first, out var a) && GetWindowRect(second, out var b)
            && a.Left < b.Right && b.Left < a.Right && a.Top < b.Bottom && b.Top < a.Bottom;
    }
    public static bool Revealed(IntPtr child) {
        if (!Owned(child)) throw new InvalidOperationException("Retired page child");
        return GetWindowRect(child, out var rect) && GetWindowRect(GetParent(child), out var page) && rect.Top >= page.Top && rect.Bottom <= page.Bottom;
    }
    public static void Key(IntPtr dialog, ushort key, bool control=false) {
        if (!Owned(dialog) || GetForegroundWindow() != dialog) throw new InvalidOperationException("Owned dialog does not have foreground input");
        var down = new Input { Type = 1, Keyboard = new KeyInput { Key = key } };
        var up = new Input { Type = 1, Keyboard = new KeyInput { Key = key, Flags = 2 } };
        var inputs = control ? new[] {
            new Input { Type = 1, Keyboard = new KeyInput { Key = 0x11 } }, down, up,
            new Input { Type = 1, Keyboard = new KeyInput { Key = 0x11, Flags = 2 } }
        } : new[] { down, up };
        if (SendInput((uint)inputs.Length, inputs, Marshal.SizeOf<Input>()) != inputs.Length) throw new InvalidOperationException("Owned input rejected");
    }
}
'@

$process = $null
$window = [IntPtr]::Zero
$dialog = [IntPtr]::Zero
$blocker = $null
$errorRead = $null
$saverProcess = $null
$previewRequestedUtc = $null
$previewPhase = 'Starting owned parent'
$uiaClient = $null
$lifetimeResult = $null
$lifetimeClock = $null
$saverPath = Join-Path (Split-Path -Parent $Executable) 'netmap.scr'
function Assert-Proof([bool] $Condition, [string] $Message) { if (-not $Condition) { throw $Message } }
function Wait-Proof([scriptblock] $Condition, [string] $Message) {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        if ($process.HasExited) { throw "Owned process exited: $($process.ExitCode)" }
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 40
    } while ([DateTime]::UtcNow -lt $deadline)
    throw $Message
}
function Open-Proof([int] $Command = 107) {
    [SettingsProof]::Post($window, 0x111, $Command)
    Wait-Proof { $script:dialog = [SettingsProof]::Find(1100); $dialog -ne [IntPtr]::Zero } 'Settings did not open.'
    Wait-Proof { [SettingsProof]::IsWindowVisible($dialog) -and [SettingsProof]::IsWindowEnabled([SettingsProof]::Child($dialog, 1)) } 'Preferences never became editable.'
}
function Close-Proof {
    [SettingsProof]::Post($dialog, 0x10)
    Wait-Proof { [SettingsProof]::Find(1100) -eq [IntPtr]::Zero } 'Settings did not close.'
    $script:dialog = [IntPtr]::Zero
}
function Search-Proof([string] $Text) { [SettingsProof]::SetText([SettingsProof]::Child($dialog, 1100), $Text) }
function Click-Proof([int] $Id) { [void][SettingsProof]::Send([SettingsProof]::Child($dialog, $Id), 0xF5) }
function Edit-Proof([int] $Id, [string] $Text) { [SettingsProof]::SetText([SettingsProof]::Child($dialog, $Id), $Text) }
function Read-Proof([int] $Id) { [SettingsProof]::Text([SettingsProof]::Child($dialog, $Id)) }
function Select-Proof([int] $Id, [int] $Index) {
    $combo = [SettingsProof]::Child($dialog, $Id)
    Assert-Proof ([SettingsProof]::Send($combo, 0x14E, $Index).ToInt64() -eq $Index) 'Native combo rejected the requested selection.'
    [void][SettingsProof]::Send([SettingsProof]::GetParent($combo), 0x111, (0x10000 -bor $Id), $combo.ToInt64())
}
function Apply-Proof {
    Click-Proof 1105
    Wait-Proof { [SettingsProof]::IsWindowEnabled([SettingsProof]::Child($dialog, 1)) } 'Save did not settle.'
}
function Save-Image([string] $Name) {
    Start-Sleep -Milliseconds 150
    $rect = [SettingsProof+Rect]::new()
    Assert-Proof ([SettingsProof]::GetWindowRect($dialog, [ref]$rect)) 'Dialog bounds unavailable.'
    $bitmap = [Drawing.Bitmap]::new($rect.Right - $rect.Left, $rect.Bottom - $rect.Top)
    try {
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try {
            $dc = $graphics.GetHdc()
            try { Assert-Proof ([SettingsProof]::PrintWindow($dialog, $dc, 0)) 'Owned dialog screenshot failed.' }
            finally { $graphics.ReleaseHdc($dc) }
        } finally { $graphics.Dispose() }
        $bitmap.Save((Join-Path $EvidenceDirectory $Name), [Drawing.Imaging.ImageFormat]::Png)
    } finally { $bitmap.Dispose() }
}
function Find-SaverProcess([DateTime] $Since) {
    # Root supplies a fresh private package path. Hold the native process handle
    # before checking its path/birth time, then retain that exact object for all
    # later queries and cleanup. Inaccessible unrelated processes are skipped.
    foreach ($candidate in [Diagnostics.Process]::GetProcesses()) {
        if ($null -eq $candidate) { continue }
        $retain = $false
        try {
            $null = $candidate.Handle
            if ($candidate.HasExited) { continue }
            $started = $candidate.StartTime
            if ($null -eq $started -or $started.ToUniversalTime() -lt $Since) { continue }
            $module = $candidate.MainModule
            if ($null -eq $module) { continue }
            if ([StringComparer]::OrdinalIgnoreCase.Equals($module.FileName, $saverPath)) {
                $retain = $true
                return $candidate
            }
        } catch [ComponentModel.Win32Exception] {
            # Protected processes cannot belong to this ordinary-token launch.
        } catch [InvalidOperationException] {
            # A process that exited during enumeration is not a live match.
        } finally {
            if (-not $retain) { $candidate.Dispose() }
        }
    }
    return $null
}

function Invoke-AccessibilityProof {
    $nativeSource = @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class SettingsAccessibility {
    public static uint Owner;
    // Framework's default proxy loader dereferences reflected types while
    // walking the stack. Keep a declared frame ahead of PowerShell's dynamic
    // methods, whose ReflectedType can be null.
    [System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
    public static void RegisterProviders(string path) {
        System.Windows.Automation.ClientSettings.RegisterClientSideProviderAssembly(System.Reflection.AssemblyName.GetAssemblyName(path));
    }
    delegate bool Callback(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, IntPtr wparam, StringBuilder text, uint flags, uint timeout, out IntPtr result);
    [DllImport("oleacc.dll")] static extern int AccessibleObjectFromWindow(IntPtr window, uint objectId, ref Guid interfaceId, [MarshalAs(UnmanagedType.Interface)] out object accessible);
    public static void Check(IntPtr window) {
        uint process;
        GetWindowThreadProcessId(window, out process);
        if (window == IntPtr.Zero || process != Owner) throw new InvalidOperationException("Owned Settings window retired");
    }
    public static IntPtr Child(IntPtr parent, int id) {
        Check(parent);
        IntPtr result = IntPtr.Zero;
        EnumChildWindows(parent, delegate(IntPtr window, IntPtr parameter) {
            if (GetDlgCtrlID(window) == id) { result = window; return false; }
            return true;
        }, IntPtr.Zero);
        Check(result);
        return result;
    }
    public static long Send(IntPtr window, uint message, long wparam, long lparam) {
        Check(window);
        IntPtr result;
        if (SendMessageTimeoutW(window, message, new IntPtr(wparam), new IntPtr(lparam), 2, 2000, out result) == IntPtr.Zero)
            throw new InvalidOperationException("Owned Settings did not answer a bounded native message");
        return result.ToInt64();
    }
    public static void SetText(IntPtr window, string text) {
        IntPtr value = Marshal.StringToHGlobalUni(text);
        try { Send(window, 12, 0, value.ToInt64()); }
        finally { Marshal.FreeHGlobal(value); }
    }
    public static string Text(IntPtr window) {
        Check(window);
        var text = new StringBuilder(4097);
        IntPtr result;
        if (SendMessageTimeoutW(window, 13, new IntPtr(text.Capacity), text, 2, 2000, out result) == IntPtr.Zero)
            throw new InvalidOperationException("Owned Settings did not answer its bounded text read");
        return text.ToString();
    }
    public static string AccessibleName(IntPtr window) {
        Check(window);
        Guid interfaceId = new Guid("618736E0-3C3D-11CF-810C-00AA00389B71");
        object accessible;
        Marshal.ThrowExceptionForHR(AccessibleObjectFromWindow(window, 0xFFFFFFFC, ref interfaceId, out accessible));
        try { return ((Accessibility.IAccessible)accessible).get_accName(0); }
        finally { if (accessible != null) Marshal.ReleaseComObject(accessible); }
    }
}
'@
    $clientSource = @'
param([string] $EvidenceDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$result = [ordered]@{
    Passed = $false; Phase = 'Initialize STA client'; Checks = @(); Controls = @(); Discrepancies = @()
    ProviderRegistration = 'Not attempted'; RawTree = 'settings-uia-tree.json'
    Scope = 'Stock Windows UI Automation names, types, enabled states and supported patterns on exact owned native Settings HWNDs; numeric page/search/draft commands. No physical keyboard, screen-reader, DPI, shell action, global input or saved preference changes.'
}
function Save-State {
    $result | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'settings-uia-result.json') -Encoding UTF8
}
function Require([bool] $Condition, [string] $Message) { if (-not $Condition) { throw $Message } }
function Checkpoint([string] $Name) {
    $result.Checks += [ordered]@{
        Name = $Name; Completed = $true
        SemanticDiscrepancies = @($result.Discrepancies | Where-Object { $_.Phase -eq $result.Phase }).Count
    }
    Save-State
}
function Native([string] $Name) {
    $property = $state.Ids.PSObject.Properties[$Name]
    Require ($null -ne $property) "Resource ID is unavailable: $Name"
    [SettingsAccessibility]::Child($dialogWindow, [int]$property.Value)
}
function Element([string] $Name) {
    $element = [System.Windows.Automation.AutomationElement]::FromHandle((Native $Name))
    Require ($null -ne $element -and $element.Current.ProcessId -eq $ownerId) "UIA ownership mismatch: $Name"
    $element
}
function Describe($Element, [string] $Id) {
    $current = $Element.Current
    [ordered]@{
        Id = $Id; Phase = $result.Phase; Name = $current.Name; AutomationId = $current.AutomationId
        Type = $current.ControlType.ProgrammaticName; Enabled = $current.IsEnabled
        ClassName = $current.ClassName; FrameworkId = $current.FrameworkId
        HWND = $current.NativeWindowHandle; ProcessId = $current.ProcessId; Offscreen = $current.IsOffscreen
        Patterns = @($Element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
    }
}
function Inspect([string] $Id, [string] $Name, [string[]] $Types, [bool] $Enabled = $true) {
    $element = Element $Id
    $metadata = Describe $element $Id
    $result.Controls += $metadata
    Save-State
    foreach ($comparison in @(
        @('Name', (-not [string]::IsNullOrWhiteSpace($metadata.Name) -and ($metadata.Name -replace '&', '') -match $Name), $Name, $metadata.Name),
        @('ControlType', ($Types -contains $metadata.Type), ($Types -join ' or '), $metadata.Type),
        @('Enabled', ($metadata.Enabled -eq $Enabled), $Enabled, $metadata.Enabled)
    )) {
        if (-not $comparison[1]) {
            $result.Discrepancies += [ordered]@{
                Id = $Id; Phase = $result.Phase; Property = $comparison[0]
                Expected = $comparison[2]; Actual = $comparison[3]
            }
        }
    }
    Save-State
    $element
}
function Value([string] $Id) {
    $pattern = $null
    Require ((Element $Id).TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) "ValuePattern unsupported: $Id"
    $pattern.Current.Value
}
function Click([string] $Id) { [void][SettingsAccessibility]::Send((Native $Id), 0xF5, 0, 0) }
function Search([string] $Text) {
    [SettingsAccessibility]::SetText((Native 'IDC_SEARCH'), $Text)
}
function Capture-Tree {
    $nodes = New-Object System.Collections.ArrayList
    $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
    function Walk($Element, [int] $Depth) {
        if ($Depth -gt 7 -or $nodes.Count -ge 256) { return }
        if ($Element.Current.ProcessId -ne $ownerId) { return }
        $metadata = Describe $Element ''
        $metadata['Depth'] = $Depth
        [void]$nodes.Add($metadata)
        $child = $walker.GetFirstChild($Element)
        while ($null -ne $child -and $nodes.Count -lt 256) {
            Walk $child ($Depth + 1)
            $child = $walker.GetNextSibling($child)
        }
    }
    Walk ([System.Windows.Automation.AutomationElement]::FromHandle($dialogWindow)) 0
    [ordered]@{ Nodes = @($nodes); Limit = 256; DepthLimit = 7; AtNodeLimit = $nodes.Count -eq 256 } |
        ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'settings-uia-tree.json') -Encoding UTF8
}
try {
    Save-State
    Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, Accessibility
    Add-Type -Path (Join-Path $EvidenceDirectory 'settings-uia-native.cs') -ReferencedAssemblies @(
        [System.Windows.Automation.ClientSettings].Assembly.Location,
        [System.Windows.Automation.ControlType].Assembly.Location,
        [Accessibility.IAccessible].Assembly.Location
    )
    $state = Get-Content -LiteralPath (Join-Path $EvidenceDirectory 'settings-uia-input.json') -Raw | ConvertFrom-Json
    $ownerId = [int]$state.ProcessId
    $dialogWindow = [IntPtr][long]$state.Dialog
    [SettingsAccessibility]::Owner = $ownerId
    [SettingsAccessibility]::Check($dialogWindow)
    $proxyPath = Join-Path ([Runtime.InteropServices.RuntimeEnvironment]::GetRuntimeDirectory()) 'WPF/UIAutomationClientsideProviders.dll'
    if (Test-Path -LiteralPath $proxyPath) {
        [SettingsAccessibility]::RegisterProviders($proxyPath)
        $result.ProviderRegistration = $proxyPath
    } else { $result.ProviderRegistration = 'Stock client-side provider assembly absent; using available native providers' }
    $result.NativeCategoryWindowText = [SettingsAccessibility]::Text((Native 'IDC_PAGES'))
    Save-State
    $result.NativeCategoryAccessibleName = [SettingsAccessibility]::AccessibleName((Native 'IDC_PAGES'))
    $categoryLabel = (Element 'IDC_PAGES').Current.LabeledBy
    $result.CategoryLabeledBy = if ($null -eq $categoryLabel) { $null } else { $categoryLabel.Current.Name }
    Save-State
    $result.Phase = 'Persistent shell names and initial states'
    [void](Inspect 'IDC_SEARCH_LABEL' 'Search\s+settings' @('ControlType.Text'))
    [void](Inspect 'IDC_SEARCH' 'Search\s+settings' @('ControlType.Edit'))
    [void](Inspect 'IDC_CLEAR' 'Clear' @('ControlType.Button') $false)
    [void](Inspect 'IDC_PAGES_LABEL' 'Categor' @('ControlType.Text'))
    $navigation = Inspect 'IDC_PAGES' 'Categor' @('ControlType.List', 'ControlType.DataGrid')
    foreach ($command in @(@('IDOK', 'OK'), @('IDCANCEL', 'Cancel'), @('IDC_RESET_ALL', 'Reset\s+all'))) {
        [void](Inspect $command[0] $command[1] @('ControlType.Button'))
    }
    [void](Inspect 'IDC_APPLY' 'Apply' @('ControlType.Button') $false)
    Require ([SettingsAccessibility]::Send((Native 'IDC_PAGES'), 0x1004, 0, 0) -eq 5) 'Settings category count is not five'
    Checkpoint 'Persistent search label/Edit name, Clear/category/footer names/types and initial enabled states'

    $result.Phase = 'Map page'
    Search 'speeds'
    [void](Inspect 'IDC_DETAIL' 'Detail' @('ControlType.ComboBox'))
    [void](Inspect 'IDC_SPEEDS' 'Speeds' @('ControlType.ComboBox'))
    [void](Inspect 'IDC_LOCATION_SETTINGS' 'Location' @('ControlType.Button'))
    Checkpoint 'Map combos and Location command expose semantic native names'

    $result.Phase = 'Connections page and retained editor draft'
    Search 'fade bright'
    [void](Inspect 'IDC_BRIGHT' 'Bright' @('ControlType.Edit'))
    [void](Inspect 'IDC_FADE' 'Fade' @('ControlType.Edit'))
    Require ((Value 'IDC_BRIGHT') -eq '10' -and (Value 'IDC_FADE') -eq '20') 'Initial duration values differ from the isolated preferences'
    $bright = $null
    Require ((Element 'IDC_BRIGHT').TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$bright)) 'Duration ValuePattern is unsupported'
    $bright.SetValue('17')
    [void](Inspect 'IDC_APPLY' 'Apply' @('ControlType.Button'))
    Checkpoint 'Duration ValuePattern changes only the editable draft'

    $result.Phase = 'Location page and mode-derived disabled states'
    Search 'manual'
    [void](Inspect 'IDC_AUTOMATIC' 'Automatic' @('ControlType.RadioButton'))
    [void](Inspect 'IDC_MANUAL' 'Manual' @('ControlType.RadioButton'))
    [void](Inspect 'IDC_LATITUDE' 'Latitude' @('ControlType.Edit'))
    [void](Inspect 'IDC_LONGITUDE' 'Longitude' @('ControlType.Edit'))
    [void](Inspect 'IDC_GET_CITY' '(Get|Update)\s+city' @('ControlType.Button'))
    [void](Inspect 'IDC_CANCEL_CITY_DATA' 'Cancel\s+download' @('ControlType.Button') $false)
    Click 'IDC_AUTOMATIC'
    [void](Inspect 'IDC_LATITUDE' 'Latitude' @('ControlType.Edit') $false)
    [void](Inspect 'IDC_LONGITUDE' 'Longitude' @('ControlType.Edit') $false)
    Click 'IDC_MANUAL'
    Checkpoint 'Location editors and actions are named; Automatic disables coordinates in UIA'

    $result.Phase = 'Desktop page'
    Search 'desktop'
    $desktop = Inspect 'IDC_DESKTOP_ENABLED' 'Enable\s+desktop' @('ControlType.CheckBox')
    [void](Inspect 'IDC_MONITOR' 'Monitor' @('ControlType.ComboBox'))
    $toggle = $null
    Require ($desktop.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$toggle)) 'Desktop checkbox TogglePattern is unsupported'
    Require ($toggle.Current.ToggleState -eq [System.Windows.Automation.ToggleState]::Off) 'Desktop draft is unexpectedly enabled'
    Click 'IDC_DESKTOP_ENABLED'
    Require ($toggle.Current.ToggleState -eq [System.Windows.Automation.ToggleState]::On) 'Native checkbox draft change did not reach UIA'
    Click 'IDC_DESKTOP_ENABLED'
    Checkpoint 'Desktop checkbox/monitor names and native TogglePattern reflect the draft'

    $result.Phase = 'Screen saver page without executing actions'
    Search 'screen saver'
    foreach ($command in @(@('IDC_INSTALL_SAVER', 'Install'), @('IDC_SAVER_WINDOWS', 'Windows\s+screen\s+saver'), @('IDC_PREVIEW_SAVER', 'Preview'))) {
        [void](Inspect $command[0] $command[1] @('ControlType.Button'))
    }
    Require (-not [SettingsAccessibility]::IsWindowVisible((Native 'IDC_RESET_PAGE'))) 'Screen saver page exposes Reset'
    Checkpoint 'Screen saver commands have native Button names; no actions executed and Reset omitted'

    $result.Phase = 'No-match navigation and Clear restoration'
    Search 'fade'
    Require ((Value 'IDC_BRIGHT') -eq '17') 'Draft did not survive visiting all lazy pages'
    Search 'no-setting-can-match-this'
    [void](Inspect 'IDC_SEARCH' 'Search\s+settings' @('ControlType.Edit'))
    [void](Inspect 'IDC_CLEAR' 'Clear' @('ControlType.Button'))
    [void](Inspect 'IDC_NO_MATCH' 'No\s+settings\s+match' @('ControlType.Text'))
    Require ([SettingsAccessibility]::IsWindowVisible((Native 'IDC_NO_MATCH'))) 'No-match explanation is hidden'
    Require ([SettingsAccessibility]::Send((Native 'IDC_PAGES'), 0x1004, 0, 0) -eq 0) 'No-match navigation still contains categories'
    $selection = $null
    Require ($navigation.TryGetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern, [ref]$selection)) 'Category SelectionPattern is unsupported'
    Require (@($selection.Current.GetSelection()).Count -eq 0) 'No-match UIA navigation retains selection'
    Click 'IDC_CLEAR'
    Require ((Value 'IDC_SEARCH') -eq '') 'Clear did not clear the native search value'
    Require ([SettingsAccessibility]::Send((Native 'IDC_PAGES'), 0x1004, 0, 0) -eq 5) 'Clear did not restore all five categories'
    Require ((Value 'IDC_BRIGHT') -eq '17') 'Clear failed to restore the last page with its retained draft'
    [void](Inspect 'IDC_CLEAR' 'Clear' @('ControlType.Button') $false)
    Checkpoint 'Typed search stays named; no-match explains empty navigation/selection; Clear restores draft and categories'
    $result.Phase = 'Capture owned dialog tree'
    Capture-Tree
    Require ($result.Discrepancies.Count -eq 0) 'Settings UIA semantic discrepancies were observed; inspect all collected page/control metadata'
    $result.Passed = $true
    $result.Phase = 'Complete'
    Save-State
    exit 0
} catch {
    $result.Error = $_.Exception.ToString()
    $result.ScriptStackTrace = $_.ScriptStackTrace
    $result.Invocation = $_.InvocationInfo.PositionMessage
    Save-State
    try { Capture-Tree } catch { $result.TreeCaptureError = $_.Exception.ToString(); Save-State }
    exit 1
}
'@
    $nativeSource | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'settings-uia-native.cs') -Encoding utf8NoBOM
    $clientPath = Join-Path $EvidenceDirectory 'settings-uia-client.ps1'
    $clientSource | Set-Content -LiteralPath $clientPath -Encoding utf8NoBOM
    $ids = @{ IDOK = 1; IDCANCEL = 2 }
    foreach ($line in Get-Content -LiteralPath (Join-Path $PSScriptRoot '../resources/ids.h')) {
        if ($line -match '^#define\s+(\w+)\s+(\d+)\s*$') { $ids[$Matches[1]] = [int]$Matches[2] }
    }
    @{ ProcessId = $process.Id; Dialog = $dialog.ToInt64(); Ids = $ids } | ConvertTo-Json -Depth 3 |
        Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'settings-uia-input.json') -Encoding utf8NoBOM
    $legacyPowerShell = Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
    $script:uiaClient = Start-Process -FilePath $legacyPowerShell -WindowStyle Hidden -PassThru -ArgumentList @(
        '-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', ('"' + $clientPath + '"'),
        '-EvidenceDirectory', ('"' + $EvidenceDirectory + '"')
    ) -RedirectStandardOutput (Join-Path $EvidenceDirectory 'settings-uia.stdout.txt') -RedirectStandardError (Join-Path $EvidenceDirectory 'settings-uia.stderr.txt')
    $null = $uiaClient.Handle
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    while (-not $uiaClient.WaitForExit(200)) {
        Assert-Proof (-not $process.HasExited) 'Owned app exited while the Settings UIA client was running.'
        Assert-Proof ([DateTime]::UtcNow -lt $deadline) 'Owned Settings UIA client exceeded its 60-second provider watchdog.'
    }
    $resultPath = Join-Path $EvidenceDirectory 'settings-uia-result.json'
    Assert-Proof (Test-Path -LiteralPath $resultPath) 'Settings UIA client returned no result; inspect preserved stdout/stderr.'
    $uiaResult = Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json
    Assert-Proof ($uiaClient.ExitCode -eq 0 -and $uiaResult.Passed) 'Settings UIA acceptance failed; inspect checkpointed metadata, tree and stack.'
    $uiaResult
}

try {
    $start = [Diagnostics.ProcessStartInfo]::new($Executable)
    $start.UseShellExecute = $false
    $start.RedirectStandardError = $true
    $start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $start.Environment['LOCALAPPDATA'] = $EvidenceDirectory
    $process = [Diagnostics.Process]::Start($start)
    $errorRead = $process.StandardError.ReadToEndAsync()
    [SettingsProof]::Owner = $process.Id
    Assert-Proof ($process.WaitForInputIdle(5000)) 'Owned app did not become input idle.'
    $window = [SettingsProof]::Find(1000)
    Assert-Proof ($window -ne [IntPtr]::Zero) 'Owned main window missing.'
    if ($Lifetime) {
        $lifetimeClock = [Diagnostics.Stopwatch]::StartNew()
        Wait-Proof {
            [SettingsProof]::ShowOwned($window)
            [SettingsProof]::IsWindowVisible($window)
        } 'Owned lifetime presentation did not become visible.'
        # Match the visible product's All connections -> Map only path before
        # sampling; the closed-cycle invariant below keeps that scope explicit.
        [void][SettingsProof]::Send($window, 0x111, 116)
        [void][SettingsProof]::Send($window, 0x111, 113)
        $file = Join-Path $data 'preferences.txt'
        $preferencesHash = (Get-FileHash -LiteralPath $file).Hash
        $lifetimeResult = [ordered]@{
            Mode = 'Lifetime'; Passed = $false; Phase = 'Initial closed sample'
            Executable = $Executable; ExecutableSHA256 = (Get-FileHash -LiteralPath $Executable).Hash
            ProcessId = $process.Id; ProcessStartedUtc = $process.StartTime.ToUniversalTime().ToString('O')
            WarmupReopens = 3; MeasuredReopens = 20; SettleMilliseconds = 150; DeadlineSeconds = 45
            Samples = @(); Scope = 'One isolated ordinary-token process, all five native Settings pages, identical closed reopen cycles and final idle samples. Live connection/background facts remain present. This diagnostic records process-level trends; it does not assign allocation ownership or prove absence of leaks.'
        }
        function Save-LifetimeState {
            $lifetimeResult | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'lifetime-result.json') -Encoding utf8
        }
        function Sample-Lifetime([string] $Phase, [int] $Cycle) {
            Assert-Proof ($lifetimeClock.Elapsed.TotalSeconds -lt 45) 'Settings lifetime diagnostic exceeded its 45-second journey deadline.'
            Assert-Proof (-not $process.HasExited -and [SettingsProof]::Find(1100) -eq [IntPtr]::Zero) 'Lifetime sample requires the owned app alive and Settings fully closed.'
            Assert-Proof ([SettingsProof]::IsWindowVisible($window)) 'Lifetime sample requires the ordinary main presentation visible.'
            $process.Refresh()
            $lifetimeResult.Samples += [ordered]@{
                Phase = $Phase; Cycle = $Cycle; ElapsedMilliseconds = $lifetimeClock.Elapsed.TotalMilliseconds
                Gdi = [SettingsProof]::GetGuiResources($process.Handle, 0)
                User = [SettingsProof]::GetGuiResources($process.Handle, 1)
                Handles = $process.HandleCount; PrivateBytes = $process.PrivateMemorySize64
                WorkingSetBytes = $process.WorkingSet64; CpuMilliseconds = $process.TotalProcessorTime.TotalMilliseconds
                MainWindowVisible = [SettingsProof]::IsWindowVisible($window)
            }
            Save-LifetimeState
        }
        function Cycle-Lifetime([string] $Phase, [int] $Cycle) {
            $lifetimeResult.Phase = "$Phase $Cycle"
            Save-LifetimeState
            Assert-Proof ($lifetimeClock.Elapsed.TotalSeconds -lt 45) 'Settings lifetime diagnostic exceeded its journey deadline.'
            Open-Proof
            foreach ($page in @(@('speeds', 1113), @('fade', 1121), @('manual', 1133), @('desktop', 1150), @('screen saver', 1163))) {
                Search-Proof $page[0]
                Assert-Proof ([SettingsProof]::IsWindowVisible([SettingsProof]::Child($dialog, $page[1]))) "Lifetime cycle did not create/show page $($page[0])."
            }
            Close-Proof
            Assert-Proof ($process.WaitForInputIdle(1000)) 'Owned app did not return to input idle after Settings closed.'
            Start-Sleep -Milliseconds 150
            Sample-Lifetime $Phase $Cycle
        }
        Sample-Lifetime 'BeforeSettings' 0
        Cycle-Lifetime 'FirstClose' 0
        for ($cycle = 1; $cycle -le 3; $cycle++) { Cycle-Lifetime 'Warmup' $cycle }
        $warmed = $lifetimeResult.Samples[-1]
        for ($cycle = 1; $cycle -le 20; $cycle++) { Cycle-Lifetime 'Measured' $cycle }
        for ($idle = 1; $idle -le 2; $idle++) {
            $lifetimeResult.Phase = "Closed idle $idle"
            Start-Sleep -Milliseconds 1000
            Sample-Lifetime 'ClosedIdle' $idle
        }
        $measured = @($lifetimeResult.Samples | Where-Object { $_.Phase -eq 'Measured' })
        $firstFive = @($measured | Select-Object -First 5)
        $lastFive = @($measured | Select-Object -Last 5)
        $finalIdle = $lifetimeResult.Samples[-1]
        $summary = [ordered]@{}
        foreach ($metric in @('Gdi', 'User', 'Handles', 'PrivateBytes', 'WorkingSetBytes')) {
            $firstRange = $firstFive | ForEach-Object { $_[$metric] } | Measure-Object -Minimum -Maximum -Average
            $lastRange = $lastFive | ForEach-Object { $_[$metric] } | Measure-Object -Minimum -Maximum -Average
            $summary[$metric] = [ordered]@{
                Warmed = $warmed[$metric]; FirstFiveMin = $firstRange.Minimum; FirstFiveMax = $firstRange.Maximum; FirstFiveMean = $firstRange.Average
                LastFiveMin = $lastRange.Minimum; LastFiveMax = $lastRange.Maximum; LastFiveMean = $lastRange.Average
                MeasuredFinalMinusWarmed = [long]$measured[-1][$metric] - [long]$warmed[$metric]
                FinalIdleMinusWarmed = [long]$finalIdle[$metric] - [long]$warmed[$metric]
                LastFiveEntirelyAboveFirstFive = $lastRange.Minimum -gt $firstRange.Maximum
            }
        }
        $lifetimeResult['Summary'] = $summary
        $lifetimeResult['FinalNativeCountPlateau'] = $summary.Gdi.LastFiveMin -eq $summary.Gdi.LastFiveMax -and $summary.User.LastFiveMin -eq $summary.User.LastFiveMax
        $lifetimeResult['PreferencesUnchanged'] = (Get-FileHash -LiteralPath $file).Hash -eq $preferencesHash
        Assert-Proof ($lifetimeResult.PreferencesUnchanged) 'Closed Settings cycles changed the isolated saved preferences.'
        $lifetimeResult.Phase = 'Closing owned parent'
        Save-LifetimeState
        [SettingsProof]::Post($window, 0x10)
        Assert-Proof ($process.WaitForExit(6000) -and $process.ExitCode -eq 0) 'Owned lifetime parent did not exit cleanly.'
        Assert-Proof ($errorRead.GetAwaiter().GetResult() -eq '') 'Owned lifetime parent wrote an error.'
        Assert-Proof ((Get-FileHash -LiteralPath $file).Hash -eq $preferencesHash) 'Owned exit changed saved preferences after the Settings cycles.'
        Assert-Proof ($lifetimeClock.Elapsed.TotalSeconds -lt 45) 'Settings lifetime diagnostic exceeded its complete journey deadline.'
        $lifetimeResult['ParentExitCode'] = $process.ExitCode
        $lifetimeResult['ElapsedMilliseconds'] = $lifetimeClock.Elapsed.TotalMilliseconds
        $lifetimeResult['ResourceAcceptance'] = 'Diagnostic only; classify the recorded warm series before attributing any growth. A positive cold/warm delta alone is not a product leak finding.'
        $lifetimeResult.Passed = $true
        $lifetimeResult.Phase = 'Complete'
        Save-LifetimeState
        $lifetimeResult | ConvertTo-Json -Depth 8
        return
    }
    if ($Accessibility) {
        $file = Join-Path $data 'preferences.txt'
        $preferencesHash = (Get-FileHash -LiteralPath $file).Hash
        Open-Proof
        $originalDialog = $dialog
        $uiaResult = Invoke-AccessibilityProof
        Assert-Proof ([SettingsProof]::Find(1100) -eq $originalDialog) 'UIA journey replaced the existing Settings owner.'
        Close-Proof
        Assert-Proof ((Get-FileHash -LiteralPath $file).Hash -eq $preferencesHash) 'Cancelling the UIA draft changed saved preferences.'
        [SettingsProof]::Post($window, 0x10)
        Assert-Proof ($process.WaitForExit(6000) -and $process.ExitCode -eq 0) 'Owned accessibility parent failed to exit.'
        Assert-Proof ($errorRead.GetAwaiter().GetResult() -eq '') 'Owned accessibility parent wrote an error.'
        [ordered]@{
            Mode = 'Accessibility'; Passed = $true; Executable = $Executable
            ExecutableSHA256 = (Get-FileHash -LiteralPath $Executable).Hash
            UIAutomation = $uiaResult; ProviderWatchdogSeconds = 60
            SettingsOwnerRetained = $true; CancelPreservedPreferences = $true
            ParentExitCode = $process.ExitCode; EvidenceDirectory = $EvidenceDirectory
        } | ConvertTo-Json -Depth 10
        return
    }
    if ($SaverPreview) {
        $previewPhase = 'Checking private package and preexisting processes'
        $file = Join-Path $data 'preferences.txt'
        $preferencesHash = (Get-FileHash -LiteralPath $file).Hash
        $hasSaver = Test-Path -LiteralPath $saverPath -PathType Leaf
        $saverHash = if ($hasSaver) { (Get-FileHash -LiteralPath $saverPath).Hash } else { $null }
        $executableHash = (Get-FileHash -LiteralPath $Executable).Hash
        if ($hasSaver) { Assert-Proof ($saverHash -eq $executableHash) 'Owned EXE/SCR package bytes differ.' }
        $existingSaver = Find-SaverProcess ([DateTime]::MinValue)
        if ($null -ne $existingSaver) {
            $existingSaver.Dispose()
            throw 'The supplied private SCR path already has a running process; ownership is ambiguous.'
        }
        $previewPhase = 'Opening Screen saver settings'
        Open-Proof
        Search-Proof 'screen saver'
        Assert-Proof ([SettingsProof]::Send([SettingsProof]::Child($dialog, 1102), 0x1004).ToInt64() -eq 1) 'Screen saver search did not isolate its category.'
        Wait-Proof { (Read-Proof 1160) -ne '' -and (Read-Proof 1160) -notmatch '^Checking screen saver availability' } 'Screen saver availability did not settle.'
        $initialStatus = Read-Proof 1160
        if ($hasSaver) {
            Assert-Proof ($initialStatus -match '^Screen saver file is available') 'Packaged Screen saver status did not report the adjacent SCR.'
        } else {
            Assert-Proof ($initialStatus -match '^Screen saver file is not present beside NetMap\.') 'Missing-SCR status did not explain unavailable package data.'
        }
        foreach ($id in @(1161, 1162, 1163)) {
            $action = [SettingsProof]::Child($dialog, $id)
            Assert-Proof ([SettingsProof]::IsWindowVisible($action) -and [SettingsProof]::IsWindowEnabled($action) -and (Read-Proof $id) -ne '') "Screen saver command $id is missing, disabled or unnamed."
        }
        Assert-Proof ((Read-Proof 1163) -eq '&Preview') 'The Preview command ID no longer matches the resource authority.'
        Assert-Proof (-not [SettingsProof]::IsWindowVisible([SettingsProof]::Child($dialog, 1106))) 'Screen saver page unexpectedly exposes preference Reset.'
        $originalDialog = $dialog
        $previewPhase = 'Requesting native Preview'
        $previewRequestedUtc = [DateTime]::UtcNow
        Click-Proof 1163 # IDC_PREVIEW_SAVER; never Install (1161) or Windows settings (1162).
        $saverWindows = @()
        $saverIdentity = $null
        if ($hasSaver) {
            $previewPhase = 'Identifying exact copied saver process'
            Wait-Proof { $script:saverProcess = Find-SaverProcess $previewRequestedUtc; $null -ne $saverProcess } 'Preview did not launch the exact copied SCR process.'
            $saverIdentity = [ordered]@{
                ProcessId = $saverProcess.Id; Path = $saverProcess.MainModule.FileName
                StartedUtc = $saverProcess.StartTime.ToUniversalTime().ToString('O')
            }
            $previewPhase = 'Verifying native saver windows'
            Wait-Proof {
                if ($saverProcess.HasExited) { throw "Owned saver exited before its windows were verified: $($saverProcess.ExitCode)" }
                $script:saverWindows = @([SettingsProof]::SaverWindows($saverProcess.Id))
                $saverWindows.Count -gt 0 -and @($saverWindows | Where-Object { -not [SettingsProof]::IsWindowVisible($_) }).Count -eq 0
            } 'The exact copied saver did not create visible native saver windows.'
            $surfaceEvidence = @($saverWindows | ForEach-Object {
                $bounds = [SettingsProof+Rect]::new()
                Assert-Proof ([SettingsProof]::GetWindowRect($_, [ref]$bounds)) 'Owned saver bounds unavailable.'
                [ordered]@{ HWND = $_.ToInt64(); Bounds = $bounds; Dpi = [SettingsProof]::GetDpiForWindow($_) }
            })
            Wait-Proof { (Read-Proof 1160) -match '^Windows command launched\.' } 'Preview success was not delivered through the application status owner.'
            # One owned WM_CLOSE ends this saver loop and all its displays.
            $previewPhase = 'Closing owned saver'
            [SettingsProof]::CloseSaver($saverWindows[0], $saverProcess.Id)
            Assert-Proof ($saverProcess.WaitForExit(10000)) 'Owned Preview saver did not close within its cleanup deadline.'
            Assert-Proof ($saverProcess.ExitCode -eq 0) "Owned Preview saver failed: $($saverProcess.ExitCode)"
            $saverIdentity['ExitCode'] = $saverProcess.ExitCode
            Assert-Proof (@([SettingsProof]::SaverWindows($saverProcess.Id)).Count -eq 0) 'Owned saver left native surfaces behind.'
        } else {
            $previewPhase = 'Verifying missing-SCR asynchronous failure'
            $surfaceEvidence = @()
            Wait-Proof { (Read-Proof 1160) -match '\(os error 2\)' } 'Missing-SCR Preview did not deliver its asynchronous file-not-found error.'
            $observationDeadline = [DateTime]::UtcNow.AddSeconds(2)
            do {
                $script:saverProcess = Find-SaverProcess $previewRequestedUtc
                Assert-Proof ($null -eq $saverProcess) 'Missing-SCR Preview unexpectedly launched a process at the private saver path.'
                Start-Sleep -Milliseconds 80
            } while ([DateTime]::UtcNow -lt $observationDeadline)
        }
        $previewPhase = 'Verifying retained Settings and preferences'
        $finalStatus = Read-Proof 1160
        Assert-Proof ([SettingsProof]::Find(1100) -eq $originalDialog) 'Preview or its reply replaced the existing Settings dialog.'
        Search-Proof 'bright'
        Assert-Proof ([SettingsProof]::IsWindowVisible([SettingsProof]::Child($dialog, 1121)) -and (Read-Proof 1121) -eq '10') 'Settings was unusable after the Preview result.'
        Assert-Proof (-not [SettingsProof]::IsWindowEnabled([SettingsProof]::Child($dialog, 1105))) 'Preview dirtied editable preferences.'
        Assert-Proof ((Get-FileHash -LiteralPath $file).Hash -eq $preferencesHash) 'Preview changed the saved preferences.'
        Save-Image 'preview-returned-settings.png'
        $previewPhase = 'Closing owned parent'
        Close-Proof
        [SettingsProof]::Post($window, 0x10)
        Assert-Proof ($process.WaitForExit(6000) -and $process.ExitCode -eq 0) 'Owned parent app did not exit cleanly after Preview.'
        Assert-Proof ($errorRead.GetAwaiter().GetResult() -eq '') 'Owned parent app wrote an error.'
        [ordered]@{
            Mode = 'SaverPreview'; Passed = $true; Executable = $Executable; ExecutableSHA256 = $executableHash
            AdjacentSaver = $hasSaver; SaverPath = $saverPath; SaverSHA256 = $saverHash
            PreviewRequestedUtc = $previewRequestedUtc.ToString('O'); SaverProcess = $saverIdentity; SaverSurfaces = $surfaceEvidence
            InitialStatus = $initialStatus; FinalStatus = $finalStatus
            ParentSettingsRetained = $true; ParentSettingsUsable = $true; PreferencesUnchanged = $true
            MissingSaverObservationMilliseconds = $(if ($hasSaver) { 0 } else { 2000 })
            ParentExitCode = $process.ExitCode; EvidenceDirectory = $EvidenceDirectory
            Scope = 'Real native Preview command through Work/Shell launch; exact private SCR path and birth-time ownership, visible saver HWNDs and WM_CLOSE teardown, asynchronous missing-file failure when absent. No global input, installation/selection, registry mutation, UAC or physical dismissal proof.'
        } | ConvertTo-Json -Depth 5
        return
    }
    Open-Proof 110
    Wait-Proof { [SettingsProof]::IsWindowVisible([SettingsProof]::Child($dialog, 1133)) } 'Origin menu did not open Location settings.'
    $existingDialog = $dialog
    Search-Proof 'fade'
    Edit-Proof 1121 '17'
    [SettingsProof]::Post($window, 0x111, 110)
    Wait-Proof { [SettingsProof]::IsWindowVisible([SettingsProof]::Child($dialog, 1133)) } 'Origin command did not navigate the existing dialog.'
    Assert-Proof ([SettingsProof]::Find(1100) -eq $existingDialog) 'Origin command created another Settings owner.'
    Search-Proof 'fade'
    Assert-Proof ((Read-Proof 1121) -eq '17') 'Origin command discarded the existing draft.'
    Close-Proof
    Open-Proof
    Assert-Proof ([SettingsProof]::WithinWorkArea($dialog)) 'Initial Settings window exceeded its work area.'
    Save-Image 'map.png'
    $original = [SettingsProof+Rect]::new()
    Assert-Proof ([SettingsProof]::GetWindowRect($dialog, [ref]$original)) 'Initial dialog bounds unavailable.'
    $scale = [SettingsProof]::GetDpiForWindow($dialog) / 96.0
    foreach ($layoutWidth in @(550, 400)) {
    [SettingsProof]::ResizeTo($dialog, [int]($layoutWidth * $scale), [int](460 * $scale), $layoutWidth -eq 400)
    if ($layoutWidth -eq 400) {
        $compact = [SettingsProof+Rect]::new()
        Assert-Proof ([SettingsProof]::GetWindowRect($dialog, [ref]$compact) -and $compact.Right - $compact.Left -eq [int](400 * $scale)) 'Compact fixture did not bypass normal tracking minimum.'
    }
    foreach ($query in @('manual', 'bright', 'speeds')) {
        Search-Proof $query
        $ids = switch ($query) {
            'manual' { @(1130, 1131, 1132, 1133, 1134, 1135, 1136, 1138, 1139, 1140, 1141) }
            'bright' { @(1120, 1121, 1122, 1123, 1124, 1125) }
            'speeds' { @(1110, 1111, 1112, 1113, 1114, 1115, 1116, 1117) }
        }
        foreach ($id in $ids) { Assert-Proof ([SettingsProof]::FitsHorizontally([SettingsProof]::Child($dialog, $id))) "Narrow page clipped control $id." }
        foreach ($id in @(1, 2, 1104, 1105, 1106)) { Assert-Proof ([SettingsProof]::FitsHorizontally([SettingsProof]::Child($dialog, $id))) "Narrow footer clipped control $id." }
        Assert-Proof (-not [SettingsProof]::Overlap([SettingsProof]::Child($dialog, 1104), [SettingsProof]::Child($dialog, 1))) 'Compact Reset all overlapped OK.'
        if ($query -eq 'manual') {
            Assert-Proof ([SettingsProof]::Above([SettingsProof]::Child($dialog, 1132), [SettingsProof]::Child($dialog, 1133))) 'Narrow origin fields did not stack.'
            [void][SettingsProof]::Send($dialog, 0x28, [SettingsProof]::Child($dialog, 1100).ToInt64(), 1)
            $action = [SettingsProof]::Child($dialog, 1139)
            [void][SettingsProof]::Send($dialog, 0x28, $action.ToInt64(), 1)
            Assert-Proof ([SettingsProof]::Revealed($action)) 'Narrow Location action could not be reached.'
            Save-Image "narrow-location-$layoutWidth.png"
        }
    }
    }
    [SettingsProof]::ResizeTo($dialog, $original.Right - $original.Left, $original.Bottom - $original.Top)
    Click-Proof 1101
    if ($Keyboard) {
    [void][SettingsProof]::SetForegroundWindow($dialog)
    [SettingsProof]::Key($dialog, 0x46, $true)
    Wait-Proof { [SettingsProof]::Focus($dialog) -eq [SettingsProof]::Child($dialog, 1100) } 'Ctrl+F did not focus search.'
    Search-Proof 'manual'
    [SettingsProof]::Key($dialog, 0x0D)
    Wait-Proof { [SettingsProof]::Focus($dialog) -eq [SettingsProof]::Child($dialog, 1102) } 'Enter in search did not focus navigation.'
    Click-Proof 1101
    [SettingsProof]::Key($dialog, 0x0D)
    Wait-Proof { [SettingsProof]::Focus($dialog) -eq [SettingsProof]::Child($dialog, 1102) } 'Clear/Enter did not return to navigation.'
    [SettingsProof]::Key($dialog, 0x26)
    Wait-Proof { [SettingsProof]::IsWindowVisible([SettingsProof]::Child($dialog, 1121)) } 'Arrow key did not select Connections.'
    [SettingsProof]::Key($dialog, 0x09)
    Wait-Proof { [SettingsProof]::Focus($dialog) -eq [SettingsProof]::Child($dialog, 1121) } 'Tab did not enter the visible page before the footer.'
    }
    Search-Proof 'FADE bri'
    Assert-Proof ([SettingsProof]::IsWindowVisible([SettingsProof]::Child($dialog, 1121))) 'Multiword prefix search failed.'
    Edit-Proof 1121 ''
    Edit-Proof 1124 '0'
    Search-Proof 'manual'
    Assert-Proof ([SettingsProof]::IsWindowVisible([SettingsProof]::Child($dialog, 1139))) 'Location search failed.'
    Search-Proof 'no-setting-will-match'
    Assert-Proof ([SettingsProof]::IsWindowVisible([SettingsProof]::Child($dialog, 1103))) 'No-match explanation missing.'
    Click-Proof 1101
    Click-Proof 1105
    Assert-Proof ((Read-Proof 1100) -eq '') 'Validation did not clear the filter.'
    Assert-Proof ((Read-Proof 1121) -eq '') 'Invalid raw text was lost across page/search switches.'
    Assert-Proof ([SettingsProof]::Focus($dialog) -eq [SettingsProof]::Child($dialog, 1121)) 'Hidden invalid field did not receive focus.'
    Edit-Proof 1121 '25'
    Apply-Proof
    Assert-Proof (-not [SettingsProof]::IsWindowEnabled([SettingsProof]::Child($dialog, 1105))) 'Successful Apply left edits pending.'
    Close-Proof
    $file = Join-Path $EvidenceDirectory 'NetMap\preferences.txt'
    Assert-Proof ((Get-Content -LiteralPath $file -Raw) -match 'bright=25') 'Preferences were not written.'
    Open-Proof
    Search-Proof 'bright'
    Assert-Proof ((Read-Proof 1121) -eq '25') 'Committed baseline did not survive reopening.'
    Click-Proof 1106
    Assert-Proof ((Read-Proof 1121) -eq '10' -and (Read-Proof 1124) -eq '20') 'Page reset did not restore canonical duration defaults.'
    Search-Proof 'manual'
    Click-Proof 1131
    Edit-Proof 1133 'unfinished'
    Click-Proof 1130
    Search-Proof 'bright'
    Search-Proof 'manual'
    Assert-Proof ((Read-Proof 1133) -eq 'unfinished') 'Inactive manual text was lost.'
    Save-Image 'location.png'
    Close-Proof
    Assert-Proof ((Get-Content -LiteralPath $file -Raw) -match 'bright=25') 'Cancel saved a pending reset.'
    Open-Proof
    Search-Proof 'bright'
    Edit-Proof 1121 '31'
    $blocker = [IO.File]::Open($file, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    Apply-Proof
    Assert-Proof ([SettingsProof]::IsWindowEnabled([SettingsProof]::Child($dialog, 1105))) 'Failed save did not allow retry.'
    Assert-Proof ((Read-Proof 1121) -eq '31') 'Failed save lost the draft.'
    $blocker.Dispose(); $blocker = $null
    Apply-Proof
    Assert-Proof ((Get-Content -LiteralPath $file -Raw) -match 'bright=31') 'Retry did not commit.'
    Close-Proof
    Open-Proof
    Search-Proof 'manual'
    Click-Proof 1131
    Edit-Proof 1133 'invalid latitude'
    [SettingsProof]::Resize($dialog, 460)
    $action = [SettingsProof]::Child($dialog, 1139)
    [void][SettingsProof]::Send($dialog, 0x28, $action.ToInt64(), 1)
    Assert-Proof ([SettingsProof]::Revealed($action)) 'Keyboard focus did not reveal an offscreen page action.'
    Save-Image 'short-location.png'
    [void][SettingsProof]::Send([SettingsProof]::GetParent($action), 0x115, 7)
    Click-Proof 1105
    $latitude = [SettingsProof]::Child($dialog, 1133)
    Assert-Proof ([SettingsProof]::Focus($dialog) -eq $latitude -and [SettingsProof]::Revealed($latitude)) 'Validation did not reveal a scrolled-out field.'
    Save-Image 'short-validation.png'
    [void][SettingsProof]::Send([SettingsProof]::GetParent($latitude), 0x115, 7)
    Assert-Proof (-not [SettingsProof]::Revealed($latitude) -and [SettingsProof]::Focus($dialog) -eq $latitude) 'Same-focus validation fixture did not scroll the editor out.'
    [void][SettingsProof]::Send($dialog, 0x111, 1)
    Assert-Proof ([SettingsProof]::Revealed($latitude) -and [SettingsProof]::Focus($dialog) -eq $latitude) 'Validation did not reveal an editor that already had focus.'
    Close-Proof
    Open-Proof
    [SettingsProof]::Post($dialog, 0x111, 1104)
    $confirmation = [IntPtr]::Zero
    Wait-Proof { $script:confirmation = [SettingsProof]::Find(6); $confirmation -ne [IntPtr]::Zero } 'Reset all did not confirm.'
    [void][SettingsProof]::Send([SettingsProof]::GetDlgItem($confirmation, 6), 0xF5)
    Search-Proof 'bright'
    Assert-Proof ((Read-Proof 1121) -eq '10' -and (Read-Proof 1124) -eq '20') 'Reset all did not reset the complete draft.'
    Close-Proof
    Assert-Proof ((Get-Content -LiteralPath $file -Raw) -match 'bright=31') 'Cancel after Reset all changed saved preferences.'
    Open-Proof
    Search-Proof 'bright'
    Edit-Proof 1121 '37'
    Click-Proof 1
    Wait-Proof { [SettingsProof]::Find(1100) -eq [IntPtr]::Zero } 'OK did not close after saving.'
    $dialog = [IntPtr]::Zero
    Assert-Proof ((Get-Content -LiteralPath $file -Raw) -match 'bright=37') 'OK closed without saving.'
    Open-Proof
    Search-Proof 'bright'
    Edit-Proof 1121 '44'
    [SettingsProof]::Post($dialog, 0x111, 1)
    [SettingsProof]::Post($dialog, 0x111, 2)
    [SettingsProof]::Post($window, 0x111, 107)
    Wait-Proof { (Get-Content -LiteralPath $file -Raw) -match 'bright=44' } 'Accepted save did not survive queued close.'
    Start-Sleep -Milliseconds 200
    $dialog = [SettingsProof]::Find(1100)
    Assert-Proof ($dialog -ne [IntPtr]::Zero) 'Earlier OK intent closed the reopened dialog.'
    Wait-Proof { [SettingsProof]::IsWindowEnabled([SettingsProof]::Child($dialog, 1)) } 'Reopened save did not settle.'
    Search-Proof 'bright'
    Assert-Proof ((Read-Proof 1121) -eq '44') 'Reopened dialog did not read the current accepted baseline.'
    Close-Proof
    # Warmed native resource counts: shell/pages/fonts must retire on each close.
    $process.Refresh()
    $before = @{ Handles = $process.HandleCount; Gdi = [SettingsProof]::GetGuiResources($process.Handle, 0); User = [SettingsProof]::GetGuiResources($process.Handle, 1) }
    for ($iteration = 0; $iteration -lt 12; $iteration++) { Open-Proof; Search-Proof 'manual'; Search-Proof 'fade'; Close-Proof }
    $process.Refresh()
    $after = @{ Handles = $process.HandleCount; Gdi = [SettingsProof]::GetGuiResources($process.Handle, 0); User = [SettingsProof]::GetGuiResources($process.Handle, 1) }
    Assert-Proof ($after.Gdi -le $before.Gdi -and $after.User -le $before.User -and $after.Handles -le $before.Handles + 2) 'Repeated Settings use leaked native resources.'
    $process.Refresh()
    $closedPrivate = $process.PrivateMemorySize64
    $closedCpu = $process.TotalProcessorTime.TotalMilliseconds
    Start-Sleep -Milliseconds 1000
    $process.Refresh()
    $closedIdleCpu = $process.TotalProcessorTime.TotalMilliseconds - $closedCpu
    Open-Proof; Search-Proof 'manual'; Search-Proof 'fade'
    $process.Refresh()
    $openPrivate = $process.PrivateMemorySize64
    $openCpu = $process.TotalProcessorTime.TotalMilliseconds
    Start-Sleep -Milliseconds 1000
    $process.Refresh()
    $openIdleCpu = $process.TotalProcessorTime.TotalMilliseconds - $openCpu
    Close-Proof
    $automaticStatus = $null
    if ($AutomaticOrigin) {
        Open-Proof
        Search-Proof 'manual'
        Click-Proof 1130
        Apply-Proof
        $deadline = [DateTime]::UtcNow.AddSeconds(75)
        do {
            $automaticStatus = [SettingsProof]::Text([SettingsProof]::GetDlgItem($window, 1001))
            if ($automaticStatus -match '(Approximate automatic origin:|Automatic origin:)') { break }
            if ($process.HasExited) { throw 'Owned app exited during origin discovery.' }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        Assert-Proof ($automaticStatus -match '(Approximate automatic origin:|Automatic origin:)' -and $automaticStatus -notmatch 'Network-change detection unavailable') "Real automatic origin failed: $automaticStatus"
        # Start another discovery and supersede it with a committed Manual
        # choice; subsequent worker completion must never restore Automatic.
        Click-Proof 1131
        Edit-Proof 1133 '5.8'
        Edit-Proof 1135 '-55.1'
        Apply-Proof
        Click-Proof 1130
        Apply-Proof
        Click-Proof 1131
        Apply-Proof
        Wait-Proof { [SettingsProof]::Text([SettingsProof]::GetDlgItem($window, 1001)) -match 'Manual origin: 5.8, -55.1' } 'Manual did not supersede Automatic.'
        Start-Sleep -Milliseconds 500
        Assert-Proof ([SettingsProof]::Text([SettingsProof]::GetDlgItem($window, 1001)) -match 'Manual origin: 5.8, -55.1') 'A stale automatic result replaced Manual.'
        Save-Image 'origin.png'
        # Close while a fresh automatic request may still be pending. Native
        # callback drain and HTTP cancellation must fit the existing deadline.
        Click-Proof 1130
        Apply-Proof
        Close-Proof
    }
    $hostingResult = $null
    if ($Hosting) {
        # Owned HWND messages only: no global input, saver commands, display
        # changes or elevation. All edits use this run's isolated preferences.
        $baselineHash = (Get-FileHash -LiteralPath $file).Hash
        $baselineOtherFields = @(Get-Content -LiteralPath $file | Where-Object { $_ -notmatch '^(desktop|monitor)=' }) -join "`n"
        $ownedMap = [SettingsProof]::GetDlgItem($window, 1000)
        Assert-Proof ($ownedMap -ne [IntPtr]::Zero) 'Hosting fixture lost the normal map.'
        Open-Proof
        Click-Proof 1101
        Assert-Proof ([SettingsProof]::Send([SettingsProof]::Child($dialog, 1102), 0x1004).ToInt64() -eq 5) 'Settings did not list both hosting categories.'
        Search-Proof 'desktop background'
        Assert-Proof ([SettingsProof]::Send([SettingsProof]::Child($dialog, 1102), 0x1004).ToInt64() -eq 1) 'Desktop search did not select one category.'
        $desktopPage = [SettingsProof]::GetParent([SettingsProof]::Child($dialog, 1150))
        Assert-Proof ([SettingsProof]::IsWindowVisible($desktopPage) -and [SettingsProof]::Text([SettingsProof]::GetDlgItem($desktopPage, 1110)) -eq 'Desktop') 'Desktop native page did not appear.'
        Assert-Proof ([SettingsProof]::Send([SettingsProof]::Child($dialog, 1150), 0xF0).ToInt64() -eq 0) 'Isolated desktop preference was not initially disabled.'
        Assert-Proof ([SettingsProof]::Send([SettingsProof]::Child($dialog, 1152), 0x147).ToInt64() -eq 0) 'Default monitor did not follow the primary display.'
        Wait-Proof { [SettingsProof]::Send([SettingsProof]::Child($dialog, 1152), 0x146).ToInt64() -gt 1 } 'Current connected displays did not populate the native combo.'
        Select-Proof 1152 1
        $monitorCaption = Read-Proof 1152
        $monitorName = $monitorCaption -replace ' \(primary\)$', ''
        $monitorBounds = [SettingsProof]::DisplayRect($monitorName)
        Click-Proof 1150
        Assert-Proof ([SettingsProof]::Send([SettingsProof]::Child($dialog, 1150), 0xF0).ToInt64() -eq 1 -and [SettingsProof]::IsWindowEnabled([SettingsProof]::Child($dialog, 1105))) 'Desktop edits did not enter the draft.'
        Search-Proof 'screen saver'
        Search-Proof 'desktop background'
        Assert-Proof ((Read-Proof 1152) -eq $monitorCaption -and [SettingsProof]::Send([SettingsProof]::Child($dialog, 1150), 0xF0).ToInt64() -eq 1) 'Hosting draft was lost across search/page changes.'
        Save-Image 'desktop-draft.png'
        Close-Proof
        Assert-Proof ((Get-FileHash -LiteralPath $file).Hash -eq $baselineHash) 'Cancel changed the saved desktop or monitor preferences.'
        Assert-Proof ([SettingsProof]::GetParent($ownedMap) -eq $window) 'Cancelled desktop edits changed the host.'
        Open-Proof
        Search-Proof 'desktop'
        Assert-Proof ([SettingsProof]::Send([SettingsProof]::Child($dialog, 1150), 0xF0).ToInt64() -eq 0 -and [SettingsProof]::Send([SettingsProof]::Child($dialog, 1152), 0x147).ToInt64() -eq 0) 'Cancelled hosting values survived reopening.'
        Select-Proof 1152 1
        Assert-Proof ((Read-Proof 1152) -eq $monitorCaption) 'Connected display choices changed during the hosting journey.'
        Click-Proof 1150
        Apply-Proof
        $saved = Get-Content -LiteralPath $file -Raw
        Assert-Proof ($saved -match '(?m)^desktop=true\r?$' -and $saved -match ('(?m)^monitor=' + [regex]::Escape($monitorName) + '\r?$')) 'Apply did not persist the chosen hosting preferences.'
        Wait-Proof { [SettingsProof]::GetParent($ownedMap) -ne $window } 'Committed desktop preference did not attach the retained map.'
        $actualBounds = [SettingsProof+Rect]::new()
        Assert-Proof ([SettingsProof]::GetWindowRect($ownedMap, [ref]$actualBounds)) 'Attached map bounds unavailable.'
        Assert-Proof ($actualBounds.Left -eq $monitorBounds.Left -and $actualBounds.Top -eq $monitorBounds.Top -and $actualBounds.Right -eq $monitorBounds.Right -and $actualBounds.Bottom -eq $monitorBounds.Bottom) 'Committed monitor did not determine desktop map bounds.'
        Assert-Proof ([SettingsProof]::Find(1100) -eq $dialog) 'Hosting Apply replaced or closed the Settings owner.'
        [SettingsProof]::Post($window, 0x111, 112)
        Wait-Proof { [SettingsProof]::GetParent($ownedMap) -eq $window } 'Open map did not restore the normal native parent.'
        Close-Proof
        Open-Proof
        Search-Proof 'desktop'
        Assert-Proof ([SettingsProof]::Send([SettingsProof]::Child($dialog, 1150), 0xF0).ToInt64() -eq 1 -and (Read-Proof 1152) -eq $monitorCaption) 'Committed desktop/monitor choices did not round-trip through reopening.'
        Assert-Proof ([SettingsProof]::IsWindowVisible([SettingsProof]::Child($dialog, 1106)) -and [SettingsProof]::IsWindowEnabled([SettingsProof]::Child($dialog, 1106))) 'Desktop page Reset was unavailable for nondefault choices.'
        Click-Proof 1106
        Assert-Proof ([SettingsProof]::Send([SettingsProof]::Child($dialog, 1150), 0xF0).ToInt64() -eq 0 -and [SettingsProof]::Send([SettingsProof]::Child($dialog, 1152), 0x147).ToInt64() -eq 0) 'Desktop Reset did not restore disabled/automatic monitor defaults.'
        Apply-Proof
        $restored = Get-Content -LiteralPath $file -Raw
        Assert-Proof ($restored -match '(?m)^desktop=false\r?$' -and $restored -match '(?m)^monitor=\r?$') 'Desktop reset was not committed.'
        Assert-Proof ((@(Get-Content -LiteralPath $file | Where-Object { $_ -notmatch '^(desktop|monitor)=' }) -join "`n") -eq $baselineOtherFields) 'Desktop operations changed another page''s committed preferences.'
        Assert-Proof ([SettingsProof]::GetParent($ownedMap) -eq $window) 'Disabling desktop did not leave the map in its normal host.'
        Search-Proof 'screen saver'
        Assert-Proof ([SettingsProof]::Send([SettingsProof]::Child($dialog, 1102), 0x1004).ToInt64() -eq 1) 'Screen saver search did not select one category.'
        $saverPage = [SettingsProof]::GetParent([SettingsProof]::Child($dialog, 1160))
        Assert-Proof ([SettingsProof]::IsWindowVisible($saverPage) -and [SettingsProof]::Text([SettingsProof]::GetDlgItem($saverPage, 1110)) -eq 'Screen saver') 'Screen saver native page did not appear.'
        Wait-Proof { (Read-Proof 1160) -ne '' -and (Read-Proof 1160) -notmatch '^Checking screen saver availability' } 'Screen saver availability status did not settle.'
        $saverStatus = Read-Proof 1160
        foreach ($id in @(1161, 1162, 1163)) {
            $action = [SettingsProof]::Child($dialog, $id)
            Assert-Proof ([SettingsProof]::IsWindowVisible($action) -and [SettingsProof]::IsWindowEnabled($action) -and (Read-Proof $id) -ne '') "Screen saver action $id was missing, disabled or unnamed."
        }
        Assert-Proof (-not [SettingsProof]::IsWindowVisible([SettingsProof]::Child($dialog, 1106))) 'Screen saver page exposed a preference Reset command.'
        Assert-Proof (-not [SettingsProof]::IsWindowEnabled([SettingsProof]::Child($dialog, 1105))) 'Read-only screen saver navigation dirtied preferences.'
        Save-Image 'screen-saver.png'
        Close-Proof
        $hostingResult = [ordered]@{
            CancelPreservedFile = $true; DraftRetainedAcrossSearch = $true
            DesktopApplied = $true; Monitor = $monitorName; MonitorCaption = $monitorCaption
            MonitorBounds = $monitorBounds; AttachedMapBounds = $actualBounds
            RetainedMapHWND = $ownedMap.ToInt64(); RetainedSettingsOwner = $true
            CommittedChoicesReopened = $true; DesktopResetApplied = $true; OtherPagesPreserved = $true
            SaverStatus = $saverStatus; SaverActionsPresent = $true; SaverResetOmitted = $true
            Scope = 'Owned native control messages, isolated preferences, existing connected display and exact map bounds; no physical input, topology changes, saver actions, UAC or visual acceptance.'
        }
    }
    [SettingsProof]::Post($window, 0x10)
    Assert-Proof ($process.WaitForExit(6000)) 'Owned app failed to exit.'
    Assert-Proof ($process.ExitCode -eq 0) 'Owned app exited with failure.'
    Assert-Proof ($errorRead.GetAwaiter().GetResult() -eq '') 'Owned app wrote an error.'
    $process.Dispose(); $process = $null
    $window = [IntPtr]::Zero; $dialog = [IntPtr]::Zero
    $process = [Diagnostics.Process]::Start($start)
    $errorRead = $process.StandardError.ReadToEndAsync()
    [SettingsProof]::Owner = $process.Id
    Assert-Proof ($process.WaitForInputIdle(5000)) 'Restarted app did not become input idle.'
    $window = [SettingsProof]::Find(1000)
    Open-Proof
    Search-Proof 'bright'
    Assert-Proof ((Read-Proof 1121) -eq '44') "Saved preferences did not load after process restart: $(Read-Proof 1121)"
    Close-Proof
    [SettingsProof]::Post($window, 0x10)
    Assert-Proof ($process.WaitForExit(6000) -and $process.ExitCode -eq 0) 'Restarted app failed to close.'
    [pscustomobject]@{
        PrefixSearch = $true; HiddenValidation = $true; RawDraftRetained = $true
        ApplyReopen = $true; PageResetCancel = $true; InactiveManualRetained = $true
        FailedSaveRetry = $true; ReopenCycles = 12; Before = $before; After = $after
        ScrolledKeyboardReveal = $true; ScrolledValidationReveal = $true
        SameFocusValidationReveal = $true
        KeyboardSearchEnterTab = [bool]$Keyboard; ResetAllCancel = $true; OkCommit = $true
        QueuedCloseReopen = $true; ProcessRestart = $true
        InitialWithinWorkArea = $true; NarrowPageReflow = $true
        NativeOriginCommand = $true; ExistingOriginDraftRetained = $true
        ClosedPrivateBytes = $closedPrivate; OpenPrivateBytes = $openPrivate
        ClosedIdleCpuMillisecondsPerSecond = $closedIdleCpu; OpenIdleCpuMillisecondsPerSecond = $openIdleCpu
        RealAutomaticOrigin = [bool]$AutomaticOrigin; AutomaticStatus = $automaticStatus
        HostingRequested = [bool]$Hosting; Hosting = $hostingResult
        EvidenceDirectory = $EvidenceDirectory; ExitCode = $process.ExitCode
    } | ConvertTo-Json -Depth 4
}
catch {
    if ($Lifetime -and $null -ne $lifetimeResult) {
        $lifetimeResult['Error'] = $_.Exception.ToString()
        $lifetimeResult['ScriptStackTrace'] = $_.ScriptStackTrace
        $lifetimeResult['Invocation'] = $_.InvocationInfo.PositionMessage
        Save-LifetimeState
    }
    if ($Accessibility) {
        $accessibilityFailure = $_
        [ordered]@{
            Mode = 'Accessibility'; Passed = $false; Error = $accessibilityFailure.Exception.ToString()
            ScriptStackTrace = $accessibilityFailure.ScriptStackTrace
            Invocation = $accessibilityFailure.InvocationInfo.PositionMessage
            Executable = $Executable; RecordedUtc = [DateTime]::UtcNow.ToString('O')
        } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'settings-accessibility-failure.json') -Encoding utf8
    }
    if ($SaverPreview) {
        $previewFailure = $_
        [ordered]@{
            Mode = 'SaverPreview'; Passed = $false; Phase = $previewPhase
            Error = $previewFailure.Exception.Message; Exception = $previewFailure.Exception.ToString()
            ScriptStackTrace = $previewFailure.ScriptStackTrace
            Invocation = $previewFailure.InvocationInfo.PositionMessage
            FullyQualifiedErrorId = $previewFailure.FullyQualifiedErrorId
            Executable = $Executable; SaverPath = $saverPath
            PreviewRequestedUtc = $previewRequestedUtc; RecordedUtc = [DateTime]::UtcNow.ToString('O')
        } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'saver-preview-failure.json') -Encoding utf8
    }
    throw
}
finally {
    if ($null -ne $blocker) { $blocker.Dispose() }
    try {
        if ($null -ne $uiaClient) {
            try {
                if (-not $uiaClient.HasExited) {
                    $uiaClient.Kill()
                    Assert-Proof ($uiaClient.WaitForExit(6000)) 'Owned UIA helper did not terminate after its watchdog.'
                }
            } finally { $uiaClient.Dispose() }
        }
    } finally {
        try {
            if ($null -ne $process) {
                if (-not $process.HasExited) {
                    if ([SettingsProof]::Owned($dialog)) { [SettingsProof]::Post($dialog, 0x10) }
                    if ([SettingsProof]::Owned($window)) {
                        if ($Hosting) { [SettingsProof]::Post($window, 0x111, 101) }
                        else { [SettingsProof]::Post($window, 0x10) }
                    }
                    if (-not $process.WaitForExit(6000)) { $process.Kill(); $process.WaitForExit() }
                }
                if ($null -ne $errorRead) { $stderr = $errorRead.GetAwaiter().GetResult(); if ($stderr) { Write-Output $stderr } }
                $process.Dispose()
            }
        } finally {
            # Retire the parent first so a command already accepted by Work cannot
            # spawn after this scan. Only the unique private SCR and this request's
            # birth-time window can be adopted; retain its handle before touching it.
            if ($SaverPreview -and $null -ne $previewRequestedUtc) {
                if ($null -eq $saverProcess) { $saverProcess = Find-SaverProcess $previewRequestedUtc }
                if ($null -ne $saverProcess) {
                    try {
                        if (-not $saverProcess.HasExited) {
                            $ownedWindows = @([SettingsProof]::SaverWindows($saverProcess.Id))
                            if ($ownedWindows.Count -gt 0) {
                                try { [SettingsProof]::CloseSaver($ownedWindows[0], $saverProcess.Id) }
                                catch [InvalidOperationException] { Write-Warning 'The owned saver window retired during cleanup; checking the retained process.' }
                            }
                            if (-not $saverProcess.WaitForExit(6000)) {
                                $saverProcess.Kill()
                                Assert-Proof ($saverProcess.WaitForExit(6000)) 'Retained saver process did not terminate during bounded cleanup.'
                            }
                        }
                    } finally { $saverProcess.Dispose() }
                }
            }
        }
    }
}
