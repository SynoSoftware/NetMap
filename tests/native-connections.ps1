#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [switch] $Inspect,
    [switch] $Compact,
    [ValidateSet('top-left', 'bottom-left', 'bottom-right')][string] $SpeedCorner = 'bottom-right',
    [string] $AutomationClient
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if ($AutomationClient) { $AutomationClient = (Resolve-Path -LiteralPath $AutomationClient).Path }
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory.' }
New-Item -ItemType Directory -Path (Join-Path $EvidenceDirectory 'NetMap') -Force | Out-Null
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination (Join-Path $EvidenceDirectory 'NetMap/preferences.txt')
if ($SpeedCorner -ne 'bottom-right') {
    $preferences = Join-Path $EvidenceDirectory 'NetMap/preferences.txt'
    $text = (Get-Content -LiteralPath $preferences -Raw).Replace('speeds=bottom-right', "speeds=$SpeedCorner")
    Set-Content -LiteralPath $preferences -Value $text -NoNewline -Encoding utf8NoBOM
}
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class ConnectionProof {
    public static uint Owner;
    delegate bool Callback(IntPtr window, IntPtr parameter);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr window, int id);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr window, ref Point point);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr window, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, IntPtr wparam, StringBuilder text, uint flags, uint timeout, out IntPtr result);
    public static bool Owned(IntPtr window) {
        GetWindowThreadProcessId(window, out uint process);
        return window != IntPtr.Zero && process == Owner;
    }
    public static IntPtr Find() {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, parameter) => {
            if (Owned(window) && GetDlgItem(window, 1000) != IntPtr.Zero) { found=window; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static string Text(IntPtr window) {
        if (!Owned(window)) throw new InvalidOperationException("Owned control was retired");
        var text = new StringBuilder(4096);
        if (SendMessageTimeoutW(window, 13, (IntPtr)4096, text, 2, 2000, out _) == IntPtr.Zero)
            throw new InvalidOperationException("UI did not answer");
        return text.ToString();
    }
    public static string Windows() {
        var text = new StringBuilder();
        EnumWindows((window, parameter) => {
            if (Owned(window)) {
                text.AppendLine(Text(window));
                EnumChildWindows(window, (child, ignored) => { text.AppendLine(Text(child)); return true; }, IntPtr.Zero);
            }
            return true;
        }, IntPtr.Zero);
        return text.ToString();
    }
}
'@

function Test-MapAccessibility {
    param([string] $Phase)
    if (-not $AutomationClient) { return }
    $start = [Diagnostics.ProcessStartInfo]::new($AutomationClient)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in @('--ignored', '--exact', 'host::accessibility::tests::native_map_has_a_stable_name_current_help_and_keyboard_access', '--nocapture', '--test-threads=1')) {
        $start.ArgumentList.Add($argument)
    }
    $start.Environment['NETMAP_UIA_WINDOW'] = $map.ToInt64().ToString()
    $start.Environment['NETMAP_UIA_PROCESS'] = $process.Id.ToString()
    $probe = [Diagnostics.Process]::Start($start)
    try {
        $stdout = $probe.StandardOutput.ReadToEndAsync()
        $stderr = $probe.StandardError.ReadToEndAsync()
        if (-not $probe.WaitForExit(15000)) {
            $probe.Kill()
            throw 'Owned accessibility client timed out.'
        }
        Set-Content -LiteralPath (Join-Path $EvidenceDirectory "uia-$Phase.stdout.txt") -Value $stdout.GetAwaiter().GetResult() -Encoding utf8NoBOM
        $errorText = $stderr.GetAwaiter().GetResult()
        Set-Content -LiteralPath (Join-Path $EvidenceDirectory "uia-$Phase.stderr.txt") -Value $errorText -Encoding utf8NoBOM
        if ($probe.ExitCode -ne 0) { throw "Accessibility query failed ($($probe.ExitCode)): $errorText" }
    }
    finally {
        if (-not $probe.HasExited) { $probe.Kill() }
        $probe.Dispose()
    }
}

$process = $null
$window = [IntPtr]::Zero
$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$client = [Net.Sockets.TcpClient]::new()
$accepted = $null
$external = [Net.Sockets.TcpClient]::new()
try {
    $listener.Start()
    $client.Connect($listener.LocalEndpoint)
    $accepted = $listener.AcceptTcpClient()
    # One idle TCP handshake gives the map a known public destination. No
    # application payload is sent; failure remains an explicit proof limitation.
    $connected = $false
    try { $connected = $external.ConnectAsync('1.1.1.1', 443).Wait(3000) -and $external.Connected } catch { }
    $start = [Diagnostics.ProcessStartInfo]::new($Executable)
    $start.UseShellExecute = $false
    $start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $start.RedirectStandardError = $true
    $start.Environment['LOCALAPPDATA'] = $EvidenceDirectory
    $process = [Diagnostics.Process]::Start($start)
    [ConnectionProof]::Owner = $process.Id
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    $text = ''
    do {
        if ($process.HasExited) { throw "NetMap exited early ($($process.ExitCode)): $($process.StandardError.ReadToEnd())" }
        $window = [ConnectionProof]::Find()
        if ($window -ne [IntPtr]::Zero) {
            $text = [ConnectionProof]::Text([ConnectionProof]::GetDlgItem($window, 1001))
            if ($text -match '[1-9][0-9]* observed connections') { break }
        }
        Start-Sleep -Milliseconds 50
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($text -notmatch '[1-9][0-9]* observed connections') { throw "No live TCP observations: $text" }
    $width = if ($Compact) { 360 } else { 1100 }
    $height = if ($Compact) { 320 } else { 720 }
    [void][ConnectionProof]::SetWindowPos($window, [IntPtr]::Zero, 40, 40, $width, $height, 0x40)
    # Exercise repeated native captures and scene replies, not only startup.
    Start-Sleep -Milliseconds 5200
    if (-not [ConnectionProof]::Owned($window) -or [ConnectionProof]::GetDlgItem($window, 1000) -eq [IntPtr]::Zero) {
        throw "Live map stopped during repeated publication: $([ConnectionProof]::Windows())"
    }
    $text = [ConnectionProof]::Text([ConnectionProof]::GetDlgItem($window, 1001))
    $inspected = $false
    if ($Inspect) {
        $map = [ConnectionProof]::GetDlgItem($window, 1000)
        [void][ConnectionProof]::PostMessageW($window, 7, [IntPtr]::Zero, [IntPtr]::Zero)
        [void][ConnectionProof]::PostMessageW($map, 256, [IntPtr]39, [IntPtr]::Zero)
        $deadline = [DateTime]::UtcNow.AddSeconds(2)
        do {
            Start-Sleep -Milliseconds 25
            $text = [ConnectionProof]::Text([ConnectionProof]::GetDlgItem($window, 1001))
            $inspected = $text -match '[0-9]+ connections · [0-9]+ active ·'
        } while (-not $inspected -and [DateTime]::UtcNow -lt $deadline)
        if (-not $inspected) { throw "Arrow inspection did not describe a destination: $text" }
        $help = [ConnectionProof]::Text($map)
        if ($help -notmatch '[0-9]+ connections · [0-9]+ active ·' -or -not $help.Contains('Arrow keys inspect')) { throw "Map accessible description missing: $help" }
        [void][ConnectionProof]::PostMessageW($map, 256, [IntPtr]0x6b, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 150
        if ([ConnectionProof]::Text([ConnectionProof]::GetDlgItem($window, 1003)) -eq '1.0×') { throw 'Inspection zoom did not change the view.' }
    }
    $map = [ConnectionProof]::GetDlgItem($window, 1000)
    Test-MapAccessibility 'current'
    $rect = [ConnectionProof+Rect]::new()
    $point = [ConnectionProof+Point]::new()
    if (-not [ConnectionProof]::GetClientRect($map, [ref]$rect) -or -not [ConnectionProof]::ClientToScreen($map, [ref]$point)) {
        if ($process.HasExited) { throw "Map exited ($($process.ExitCode)): $($process.StandardError.ReadToEnd())" }
        throw "Map rectangle unavailable. Owned windows: $([ConnectionProof]::Windows())"
    }
    $bitmap = [Drawing.Bitmap]::new($rect.Right, $rect.Bottom)
    try {
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try {
            try { $graphics.CopyFromScreen($point.X, $point.Y, 0, 0, $bitmap.Size) }
            catch {
                $dc = $graphics.GetHdc()
                try { if (-not [ConnectionProof]::PrintWindow($map, $dc, 2)) { throw 'Owned-window capture unavailable.' } }
                finally { $graphics.ReleaseHdc($dc) }
            }
        } finally { $graphics.Dispose() }
        $bitmap.Save((Join-Path $EvidenceDirectory 'live-map.png'), [Drawing.Imaging.ImageFormat]::Png)
    } finally { $bitmap.Dispose() }
    $process.Refresh()
    $result = [ordered]@{ pid=$process.Id; status=$text; externalHandshake=$connected; inspection=$inspected; privateBytes=$process.PrivateMemorySize64; workingSet=$process.WorkingSet64 }
    if ($Inspect) {
        [void][ConnectionProof]::PostMessageW($map, 256, [IntPtr]0x1b, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 100
        if ([ConnectionProof]::Text([ConnectionProof]::GetDlgItem($window, 1001)) -match '[0-9]+ connections · [0-9]+ active ·') { throw 'Escape did not clear inspection.' }
        Test-MapAccessibility 'cleared'
    }
    [void][ConnectionProof]::PostMessageW($window, 16, [IntPtr]::Zero, [IntPtr]::Zero)
    if (-not $process.WaitForExit(8000)) { throw 'Owned NetMap did not exit after Close.' }
    $result.exitCode = $process.ExitCode
    $result.stderr = $process.StandardError.ReadToEnd()
    $result | ConvertTo-Json | Tee-Object -FilePath (Join-Path $EvidenceDirectory 'result.json')
    if ($process.ExitCode -ne 0 -or $result.stderr.Length -ne 0) { throw 'NetMap did not exit cleanly.' }
} finally {
    if ($null -ne $process) {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $process.Dispose()
    }
    $client.Dispose()
    if ($null -ne $accepted) { $accepted.Dispose() }
    $external.Dispose()
    $listener.Stop()
}
