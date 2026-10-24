#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $EvidenceDirectory
)

# Real TCP/table/UIA journey. Only owned loopback sockets and the launched app
# are changed. UIA executes in a bounded disposable STA client, never by passing
# a ListView item pointer across process boundaries or injecting global input.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory.' }
$data = Join-Path $EvidenceDirectory 'NetMap'
New-Item -ItemType Directory -Path $data | Out-Null
@'
NetMap preferences 1
detail=countries
speeds=bottom-right
bright=15
fade=5
origin=manual
latitude=0
longitude=0
desktop=false
monitor=
'@ | Set-Content -LiteralPath (Join-Path $data 'preferences.txt') -Encoding utf8NoBOM

$native = @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public sealed class ExplorationReadChangedException : Exception {
    public ExplorationReadChangedException(string message) : base(message) { }
}
public static class ExplorationProof {
    delegate bool Callback(IntPtr window, IntPtr parameter);
    [StructLayout(LayoutKind.Sequential)] struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct GuiInfo {
        public uint Size, Flags;
        public IntPtr Active, Focus, Capture, MenuOwner, MoveSize, Caret;
        public Rect Bounds;
    }
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr window, StringBuilder name, int capacity);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr window, int id);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int command);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] static extern bool GetGUIThreadInfo(uint thread, ref GuiInfo info);
    [DllImport("user32.dll")] static extern bool PostMessageW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam, uint flags, uint timeout, out IntPtr result);
    public static uint Owner(IntPtr window) { uint owner; GetWindowThreadProcessId(window, out owner); return owner; }
    public static string ClassName(IntPtr window, uint process) {
        if (window == IntPtr.Zero || Owner(window) != process) throw new Exception("Owned class-query window retired");
        var name = new StringBuilder(256);
        GetClassNameW(window, name, name.Capacity);
        return name.ToString();
    }
    public static IntPtr Find(uint process) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, unused) => {
            if (Owner(window) == process && GetDlgItem(window, 1000) != IntPtr.Zero) { found = window; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static IntPtr Focus(IntPtr window, uint process) {
        uint owner;
        uint thread = GetWindowThreadProcessId(window, out owner);
        if (window == IntPtr.Zero || owner != process) throw new Exception("Owned UI window retired");
        var info = new GuiInfo { Size = (uint)Marshal.SizeOf(typeof(GuiInfo)) };
        if (!GetGUIThreadInfo(thread, ref info)) throw new Exception("Owned focus query failed");
        return info.Focus;
    }
    public static IntPtr Send(IntPtr window, uint process, uint message, long wparam=0) {
        if (window == IntPtr.Zero || Owner(window) != process) throw new Exception("Owned test window retired");
        IntPtr result;
        if (SendMessageTimeoutW(window, message, (IntPtr)wparam, IntPtr.Zero, 0x22, 2000, out result) == IntPtr.Zero)
            throw new Exception("Owned UI did not answer a bounded numeric message");
        return result;
    }
    public static void Post(IntPtr window, uint process, uint message, long wparam=0) {
        if (window == IntPtr.Zero || Owner(window) != process) throw new Exception("Owned test window retired");
        if (!PostMessageW(window, message, (IntPtr)wparam, IntPtr.Zero)) throw new Exception("Owned queued message failed");
    }
}
'@
Add-Type -TypeDefinition $native
$native | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'native.cs') -Encoding utf8NoBOM

# Windows PowerShell uses the stock .NET Framework UIAutomation client. Running
# it separately bounds even an unresponsive accessibility provider call.
$uiaScript = @'
param([string] $EvidenceDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type -Path (Join-Path $EvidenceDirectory 'native.cs')
$inputState = Get-Content -LiteralPath (Join-Path $EvidenceDirectory 'input.json') -Raw | ConvertFrom-Json
$ownerId = [uint32]$inputState.ProcessId
$window = [IntPtr][long]$inputState.Window
$tableWindow = [ExplorationProof]::GetDlgItem($window, 1004)
$mapWindow = [ExplorationProof]::GetDlgItem($window, 1000)
$result = [ordered]@{ Passed = $false; Checks = @(); Rows = @(); Scope = 'Owned native ListView UIAutomation patterns, numeric commands, queued Escape and loopback table facts. No physical input, UAC, location fixture or direct observation-ID inspection.' }
$result.ReadRetries = 0
$script:readDeadline = $null

function Save-State {
    $result | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'uia-result.json') -Encoding UTF8
}
function Phase([string] $Name) {
    $Name | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'phase.txt') -Encoding ASCII
}
function Wait-Until([scriptblock] $Condition, [string] $Failure, [int] $Seconds = 15) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    $previousDeadline = $script:readDeadline
    if ($null -ne $previousDeadline -and $previousDeadline -lt $deadline) { $deadline = $previousDeadline }
    $script:readDeadline = $deadline
    try {
        do {
            try {
                if (& $Condition) { return }
            } catch {
                $retry = $false
                for ($error = $_.Exception; $null -ne $error; $error = $error.InnerException) {
                    if ($error -is [ExplorationReadChangedException] -or $error -is [System.Windows.Automation.ElementNotAvailableException]) { $retry = $true; break }
                }
                if (-not $retry -or [ExplorationProof]::Owner($tableWindow) -ne $ownerId) { throw }
                $result.ReadRetries++
                $result.LastReadRetry = $error.Message
            }
            if ([DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 150 }
        } while ([DateTime]::UtcNow -lt $deadline)
        throw $Failure
    } finally {
        $script:readDeadline = $previousDeadline
    }
}
function Command([int] $Id) {
    [ExplorationProof]::Send($window, $ownerId, 0x111, $Id) | Out-Null
}
function Describe-Element($Element) {
    $provider = [System.Windows.Automation.AutomationProperty]::LookupById(30107)
    [ordered]@{
        Name = $Element.Current.Name
        ClassName = $Element.Current.ClassName
        ControlType = $Element.Current.ControlType.ProgrammaticName
        FrameworkId = $Element.Current.FrameworkId
        NativeWindowHandle = $Element.Current.NativeWindowHandle
        ProcessId = $Element.Current.ProcessId
        Offscreen = $Element.Current.IsOffscreen
        ProviderDescription = if ($null -ne $provider) { [string]$Element.GetCurrentPropertyValue($provider) } else { 'Property unavailable in this client' }
        Patterns = @($Element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
    }
}
function Describe-Children($Element) {
    $children = @()
    $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
    $child = $walker.GetFirstChild($Element)
    for ($index = 0; $null -ne $child -and $index -lt 8; $index++) {
        $children += Describe-Element $child
        $child = $walker.GetNextSibling($child)
    }
    $children
}
function Cell-Text($Element) {
    if ($Element.Current.ProcessId -ne $ownerId) { throw 'UIA element belongs to another process' }
    $Element.Current.Name
}
function Grid-Cell([int] $Index, [int] $Column) {
    if ($Index -lt 0 -or $Column -lt 0 -or $Column -ge 9) { throw 'Invalid harness cell index' }
    if ($Index -ge $script:grid.Current.RowCount) { throw [ExplorationReadChangedException]::new('Row retired or reindexed before cell read') }
    try {
        $script:grid.GetItem($Index, $Column)
    } catch {
        for ($error = $_.Exception; $null -ne $error; $error = $error.InnerException) {
            # This exact row was in range immediately before GetItem. Do not
            # retry arbitrary argument, method, column or provider failures.
            if ($error -is [ArgumentOutOfRangeException] -and $error.ParamName -eq 'row') {
                throw [ExplorationReadChangedException]::new('Grid resized between row-bound check and GetItem')
            }
        }
        throw
    }
}
function Read-Cell([int] $Index, [int] $Column) {
    Cell-Text (Grid-Cell $Index $Column)
}
function Is-Controlled($Row, [string] $State = '') {
    $Row.Cells[1] -eq [string]$inputState.TrafficProcessId -and
        $Row.Cells[2] -eq 'TCP' -and $Row.Cells[3] -eq $inputState.LocalEndpoint -and
        $Row.Cells[4] -eq $inputState.RemoteEndpoint -and ($State -eq '' -or $Row.Cells[5] -eq $State)
}
function Read-Row([int] $Index) {
    $count = $script:grid.Current.RowCount
    $cells = @()
    for ($column = 0; $column -lt 9; $column++) {
        $cells += Read-Cell $Index $column
    }
    foreach ($column in 1, 2, 3, 4, 5) {
        if ((Read-Cell $Index $column) -cne $cells[$column]) { throw [ExplorationReadChangedException]::new('Row tuple or lifetime changed during multi-cell read') }
    }
    if ($script:grid.Current.RowCount -ne $count) { throw [ExplorationReadChangedException]::new('Row count changed during multi-cell read') }
    [pscustomobject]@{ Index = $Index; Cells = $cells }
}
function Tuple-Rows {
    $found = @()
    $count = $script:grid.Current.RowCount
    if ($count -gt 50000) { throw 'UIA search exceeded this controlled journey row bound' }
    for ($index = 0; $index -lt $count; $index++) {
        if ((Read-Cell $index 1) -ne [string]$inputState.TrafficProcessId) { continue }
        if ((Read-Cell $index 3) -ne $inputState.LocalEndpoint) { continue }
        if ((Read-Cell $index 4) -ne $inputState.RemoteEndpoint) { continue }
        $row = Read-Row $index
        if (-not (Is-Controlled $row)) { throw [ExplorationReadChangedException]::new('Controlled candidate reindexed during scan') }
        $found += $row
    }
    if ($script:grid.Current.RowCount -ne $count) { throw [ExplorationReadChangedException]::new('Row count changed during controlled-tuple scan') }
    $found
}
function Selection-Element([int] $Index) {
    $element = Grid-Cell $Index 0
    for ($ancestor = 0; $ancestor -lt 4 -and $null -ne $element; $ancestor++) {
        if ($element.Current.ProcessId -ne $ownerId) { throw 'UIA selection escaped owned process' }
        $pattern = $null
        if ($element.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) {
            return $pattern
        }
        $element = [System.Windows.Automation.TreeWalker]::RawViewWalker.GetParent($element)
    }
    throw 'Native report row does not expose SelectionItemPattern'
}
function Selected-Rows {
    $items = @($script:selection.Current.GetSelection())
    $before = ($items | ForEach-Object { [string]::Join(',', $_.GetRuntimeId()) }) -join ';'
    $rows = @()
    foreach ($item in $items) {
        if ($item.Current.ProcessId -ne $ownerId) { throw 'UIA selected item belongs to another process' }
        $selectedPattern = $null
        if (-not $item.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$selectedPattern)) { throw 'Selected item has no native SelectionItemPattern' }
        if (-not $selectedPattern.Current.IsSelected) { throw [ExplorationReadChangedException]::new('Selection changed before reading selected row') }
        $pattern = $null
        if (-not $item.TryGetCurrentPattern([System.Windows.Automation.GridItemPattern]::Pattern, [ref]$pattern)) {
            $first = $item.FindFirst([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::IsGridItemPatternAvailableProperty, $true))
            if ($null -eq $first -or -not $first.TryGetCurrentPattern([System.Windows.Automation.GridItemPattern]::Pattern, [ref]$pattern)) {
                if (-not $selectedPattern.Current.IsSelected) { throw [ExplorationReadChangedException]::new('Selected item retired during GridItem discovery') }
                throw 'Selected row does not expose its GridItem row index'
            }
        }
        $index = $pattern.Current.Row
        $rows += Read-Row $index
        if ($pattern.Current.Row -ne $index -or -not $selectedPattern.Current.IsSelected) { throw [ExplorationReadChangedException]::new('Selected item reindexed during row read') }
    }
    $after = (@($script:selection.Current.GetSelection()) | ForEach-Object { [string]::Join(',', $_.GetRuntimeId()) }) -join ';'
    if ($before -cne $after) { throw [ExplorationReadChangedException]::new('Selected runtime IDs changed during row read') }
    $rows
}
function Assert-Selected([string] $State) {
    $selected = @(Selected-Rows)
    if ($selected.Count -ne 1 -or -not (Is-Controlled $selected[0] $State)) {
        if ($State -eq 'Ended' -and $selected.Count -eq 0 -and @(Tuple-Rows | Where-Object { $_.Cells[5] -eq 'Ended' }).Count -eq 0) {
            throw [ExplorationReadChangedException]::new('Ended row retired between scan and selected-row read')
        }
        throw "Selection changed its controlled connection/state; expected $State"
    }
}
function Select-Controlled([string] $State) {
    Wait-Until {
        $rows = @(Tuple-Rows | Where-Object { $_.Cells[5] -eq $State })
        if ($rows.Count -ne 1) { return $false }
        $index = $rows[0].Index
        if (-not (Is-Controlled (Read-Row $index) $State)) { throw [ExplorationReadChangedException]::new('Selection target reindexed before Select') }
        (Selection-Element $index).Select()
        if (-not (Is-Controlled (Read-Row $index) $State)) { throw [ExplorationReadChangedException]::new('Selection target reindexed during Select') }
        Assert-Selected $State
        return $true
    } "Controlled $State row could not be selected consistently"
}

try {
    Command 114
    if ([ExplorationProof]::Focus($window, $ownerId) -ne $tableWindow) { throw 'Table only did not focus the native list' }
    if (-not [ExplorationProof]::IsWindowVisible($tableWindow) -or [ExplorationProof]::IsWindowVisible($mapWindow)) { throw 'Table only visibility failed' }
    Command 113
    if ([ExplorationProof]::Focus($window, $ownerId) -ne $mapWindow) { throw 'Map only did not focus the map' }
    Command 115
    if (-not [ExplorationProof]::IsWindowVisible($tableWindow) -or -not [ExplorationProof]::IsWindowVisible($mapWindow)) { throw 'Map and table visibility failed' }
    Command 114
    $table = [System.Windows.Automation.AutomationElement]::FromHandle($tableWindow)
    $header = [ExplorationProof]::Send($tableWindow, $ownerId, 0x101f)
    $result.NativeTable = [ordered]@{
        HWND = $tableWindow.ToInt64()
        ClassName = [ExplorationProof]::ClassName($tableWindow, $ownerId)
        Owner = [ExplorationProof]::Owner($tableWindow)
        RowCount = [ExplorationProof]::Send($tableWindow, $ownerId, 0x1004).ToInt64()
        View = [ExplorationProof]::Send($tableWindow, $ownerId, 0x108f).ToInt64()
        HeaderColumns = [ExplorationProof]::Send($header, $ownerId, 0x1200).ToInt64()
    }
    $result.UIABefore = Describe-Element $table
    $result.RawChildrenBefore = @(Describe-Children $table)
    $result.Patterns = @($table.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
    Save-State
    if ($result.Patterns.Count -eq 0) {
        # Stock Win32 proxies normally load automatically. Explicit registration
        # is scoped to this disposable client and changes no app or machine state.
        $proxyPath = Join-Path ([Runtime.InteropServices.RuntimeEnvironment]::GetRuntimeDirectory()) 'WPF/UIAutomationClientsideProviders.dll'
        $proxyAssembly = [Reflection.AssemblyName]::GetAssemblyName($proxyPath)
        $result.StockProxyAssembly = $proxyAssembly.FullName
        [System.Windows.Automation.ClientSettings]::RegisterClientSideProviderAssembly($proxyAssembly)
        $table = [System.Windows.Automation.AutomationElement]::FromHandle($tableWindow)
        $result.UIAAfterStockProxyRegistration = Describe-Element $table
        $result.RawChildrenAfterStockProxyRegistration = @(Describe-Children $table)
        $result.Patterns = @($table.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
        Save-State
    }
    $script:grid = $null
    $script:selection = $null
    if (-not $table.TryGetCurrentPattern([System.Windows.Automation.GridPattern]::Pattern, [ref]$script:grid)) { throw 'Native owner-data ListView lacks accessible GridPattern; journey blocked' }
    if (-not $table.TryGetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern, [ref]$script:selection)) { throw 'Native owner-data ListView lacks SelectionPattern; journey blocked' }
    if ($script:grid.Current.ColumnCount -ne 9) { throw 'Native exploration column count is not nine' }
    if ($table.Current.Name -cne 'Connections') { throw 'Native exploration accessible name is not Connections' }
    $result.Checks += 'Map/Table/Both visibility and focus; accessible Connections name'
    Save-State
    $script:initial = $null
    Wait-Until {
        $candidates = @(Tuple-Rows | Where-Object { $_.Cells[5] -eq 'Established' })
        if ($candidates.Count -ne 1) { return $false }
        $script:initial = $candidates[0]
        return $true
    } 'Controlled established loopback row did not become accessible'
    $result.Rows += $script:initial
    if ($script:initial.Cells[0] -eq 'Name unavailable' -or $script:initial.Cells[0] -notlike ($inputState.TrafficProcessName + '*')) { throw 'Accessible controlled process name missing or mismatched' }
    if ($script:initial.Cells[2] -ne 'TCP' -or $script:initial.Cells[6] -notmatch 'loopback') { throw 'Controlled TCP/loopback reason missing' }
    foreach ($column in 7, 8) {
        if ($script:initial.Cells[$column] -notmatch '^(Unavailable|[0-9]+ B/s)$') { throw 'Rate is neither measured nor explicitly unavailable' }
    }
    $result.Checks += 'Ready process/PID/tuple/protocol/loopback reason and explicit rate availability'
    Save-State
    Select-Controlled 'Established'
    [ExplorationProof]::Post($tableWindow, $ownerId, 0x100, 0x1b)
    Wait-Until { @(Selected-Rows).Count -eq 0 } 'Queued Escape did not clear the native selection'
    $result.Checks += 'Queued Escape clears native row selection'
    Save-State
    Select-Controlled 'Established'
    Command 116
    Wait-Until { @(Selected-Rows).Count -eq 0 } 'All connections did not clear selection'
    $result.Checks += 'All connections clears native row selection'
    Save-State
    Command 114
    Select-Controlled 'Established'
    $hold = [DateTime]::UtcNow.AddSeconds(22)
    $script:readDeadline = $hold
    try {
        do {
            Wait-Until { Assert-Selected 'Established'; return $true } 'Live selected row could not be read consistently'
            Start-Sleep -Milliseconds 250
        } while ([DateTime]::UtcNow -lt $hold)
    } finally { $script:readDeadline = $null }
    $result.Checks += 'Live native row and selection survive establishment expiry'
    Save-State
    Phase 'close-target'
    Wait-Until {
        if (@(Tuple-Rows).Count -ne 0) { return $false }
        $selected = @(Selected-Rows)
        if ($selected.Count -ne 0) {
            if ($selected.Count -eq 1 -and (Is-Controlled $selected[0])) { throw [ExplorationReadChangedException]::new('Tuple scan and selected row straddled closure') }
            throw 'Closing the faded live row selected another observation'
        }
        return $true
    } 'Faded live TCP did not retire and clear its exact row after closure'
    $result.Checks += 'Closing a TCP whose establishment already expired clears selection without requiring ended retention'
    Save-State
    Phase 'retired-long'
    Wait-Until { Test-Path -LiteralPath (Join-Path $EvidenceDirectory 'fresh.json') } 'Owned fresh TCP did not complete'
    $script:fresh = $null
    Wait-Until {
        $fresh = @(Tuple-Rows | Where-Object { $_.Cells[5] -eq 'Established' })
        if ($fresh.Count -ne 1) { return $false }
        if (@(Selected-Rows).Count -ne 0) { throw 'Fresh same-tuple identity inherited retired row selection' }
        $script:fresh = $fresh[0]
        return $true
    } 'Fresh controlled TCP row did not appear'
    Select-Controlled 'Established'
    $result.Checks += 'Fresh same-tuple TCP appears without inheriting retired row selection'
    Save-State
    Phase 'close-short'
    Wait-Until {
        $ended = @(Tuple-Rows | Where-Object { $_.Cells[5] -eq 'Ended' })
        if ($ended.Count -ne 1) { return $false }
        Assert-Selected 'Ended'
        $result.Rows += $ended[0]
        return $true
    } 'Closed controlled TCP did not retain its selected ended row'
    $result.Checks += 'Freshly closed TCP retains its selected ended row'
    Save-State
    Phase 'ended'
    Wait-Until { Test-Path -LiteralPath (Join-Path $EvidenceDirectory 'reconnected.json') } 'Owned same-tuple reconnect did not complete'
    Wait-Until {
        $rows = @(Tuple-Rows)
        if (@($rows | Where-Object { $_.Cells[5] -eq 'Established' }).Count -ne 1 -or @($rows | Where-Object { $_.Cells[5] -eq 'Ended' }).Count -ne 1) { return $false }
        Assert-Selected 'Ended'
        $result.Rows += $rows
        return $true
    } 'Ended and reconnected same-tuple rows did not coexist without retargeting selection'
    $result.Checks += 'Ended and reconnected same-tuple rows coexist without retargeting selection'
    Save-State
    Wait-Until {
        $rows = @(Tuple-Rows)
        if (@($rows | Where-Object { $_.Cells[5] -eq 'Ended' }).Count -ne 0) { Assert-Selected 'Ended'; return $false }
        if (@($rows | Where-Object { $_.Cells[5] -eq 'Established' }).Count -ne 1) { throw 'Live same-tuple successor disappeared at ended-row retirement' }
        $selected = @(Selected-Rows)
        if ($selected.Count -ne 0) {
            if ($selected.Count -eq 1 -and (Is-Controlled $selected[0] 'Ended')) { throw [ExplorationReadChangedException]::new('Ended row scan and selection read straddled an update') }
            throw 'Retired row selection moved to its same-tuple successor'
        }
        $result.Rows += $rows
        return $true
    } 'Ended row did not retire and clear its exact selection' 45
    $result.Checks += 'Exact ended-row retirement clears selection while its live successor remains unselected'
    $result.Passed = $true
} catch {
    $result.Error = $_.Exception.ToString()
    throw
} finally {
    Save-State
}
'@
$helperPath = Join-Path $EvidenceDirectory 'uia.ps1'
$uiaScript | Set-Content -LiteralPath $helperPath -Encoding utf8NoBOM

$savedData = $env:LOCALAPPDATA
$app = $null
$helper = $null
$listener = $null
$client = $null
$server = $null
$window = [IntPtr]::Zero
$reusingTuple = $false
$result = [ordered]@{
    ExecutableSHA256 = (Get-FileHash -LiteralPath $Executable).Hash
    StartedUtc = [DateTime]::UtcNow.ToString('o')
    Passed = $false
    Scope = 'Ordinary-token app; owned loopback TCP and exact HWNDs; UIA client with 120-second watchdog. No UAC, global input, Explorer or other applications changed.'
}
try {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $result.IsAdministrator = [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if ($result.IsAdministrator) { throw 'Run this ordinary-token journey without elevation.' }
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $accept = $listener.AcceptTcpClientAsync()
    $client = [Net.Sockets.TcpClient]::new([Net.Sockets.AddressFamily]::InterNetwork)
    if (-not $client.ConnectAsync([Net.IPAddress]::Loopback, $listener.LocalEndpoint.Port).Wait(3000)) { throw 'Owned loopback connect timed out' }
    if (-not $accept.Wait(3000)) { throw 'Owned loopback accept timed out' }
    $server = $accept.Result
    $local = $client.Client.LocalEndPoint
    $remote = $client.Client.RemoteEndPoint
    $result.LocalEndpoint = $local.ToString()
    $result.RemoteEndpoint = $remote.ToString()
    $env:LOCALAPPDATA = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
    $app = Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru
    $result.ProcessId = $app.Id
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        if ($app.HasExited) { throw 'Owned app exited during startup' }
        $window = [ExplorationProof]::Find($app.Id)
        if ($window -ne [IntPtr]::Zero) {
            [ExplorationProof]::ShowWindow($window, 4) | Out-Null
            if ([ExplorationProof]::IsWindowVisible($window)) { break }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($window -eq [IntPtr]::Zero -or -not [ExplorationProof]::IsWindowVisible($window)) { throw 'Owned ordinary window did not initialize' }
    $inputState = [ordered]@{
        ProcessId = $app.Id
        Window = $window.ToInt64()
        TrafficProcessId = $PID
        TrafficProcessName = [Diagnostics.Process]::GetCurrentProcess().ProcessName
        LocalEndpoint = $local.ToString()
        RemoteEndpoint = $remote.ToString()
    }
    $inputState | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'input.json') -Encoding utf8NoBOM
    $legacyPowerShell = Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
    $helper = Start-Process -FilePath $legacyPowerShell -WindowStyle Hidden -PassThru -ArgumentList @('-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-File', ('"' + $helperPath + '"'), '-EvidenceDirectory', ('"' + (Resolve-Path -LiteralPath $EvidenceDirectory).Path + '"')) -RedirectStandardOutput (Join-Path $EvidenceDirectory 'uia.stdout.txt') -RedirectStandardError (Join-Path $EvidenceDirectory 'uia.stderr.txt')
    $deadline = [DateTime]::UtcNow.AddSeconds(120)
    $handled = [Collections.Generic.HashSet[string]]::new()
    do {
        if ($app.HasExited) { throw 'Owned app exited during exploration journey' }
        $phasePath = Join-Path $EvidenceDirectory 'phase.txt'
        if (Test-Path -LiteralPath $phasePath) {
            $phase = (Get-Content -LiteralPath $phasePath -Raw).Trim()
            if (($phase -in @('close-target', 'close-short')) -and $handled.Add($phase)) {
                $client.Client.LingerState = [Net.Sockets.LingerOption]::new($true, 0)
                $server.Client.LingerState = [Net.Sockets.LingerOption]::new($true, 0)
                $client.Dispose(); $client = $null
                $server.Dispose(); $server = $null
            }
            if (($phase -in @('retired-long', 'ended')) -and $handled.Add($phase)) {
                $reusingTuple = $true
                $accept = $listener.AcceptTcpClientAsync()
                $client = [Net.Sockets.TcpClient]::new([Net.Sockets.AddressFamily]::InterNetwork)
                $client.Client.Bind($local)
                if (-not $client.ConnectAsync([Net.IPAddress]::Loopback, $remote.Port).Wait(3000)) { throw 'Owned same-tuple reconnect timed out' }
                if (-not $accept.Wait(3000)) { throw 'Owned same-tuple accept timed out' }
                $server = $accept.Result
                if ($client.Client.LocalEndPoint.ToString() -ne $local.ToString() -or $client.Client.RemoteEndPoint.ToString() -ne $remote.ToString()) { throw 'Reconnect did not preserve the exact controlled tuple' }
                $marker = if ($phase -eq 'retired-long') { 'fresh.json' } else { 'reconnected.json' }
                @{ LocalEndpoint = $local.ToString(); RemoteEndpoint = $remote.ToString(); Utc = [DateTime]::UtcNow.ToString('o') } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $EvidenceDirectory $marker) -Encoding utf8NoBOM
                $reusingTuple = $false
            }
        }
        if ($helper.HasExited) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $helper.HasExited) { throw 'Owned UIA client exceeded its 120-second watchdog; provider interaction is incomplete' }
    $helper.WaitForExit()
    $result.HelperExitCode = $helper.ExitCode
    $uiaResultPath = Join-Path $EvidenceDirectory 'uia-result.json'
    if (Test-Path -LiteralPath $uiaResultPath) { $result.UIAutomation = Get-Content -LiteralPath $uiaResultPath -Raw | ConvertFrom-Json }
    if ($helper.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $uiaResultPath) -or -not $result.UIAutomation.Passed) { throw 'Owned UIA journey failed; inspect preserved stdout/stderr and partial results' }
    $result.Passed = $true
} catch {
    $result.Error = $_.Exception.ToString()
    if ($reusingTuple) {
        for ($error = $_.Exception; $null -ne $error; $error = $error.InnerException) {
            if ($error -is [Net.Sockets.SocketException] -and $error.NativeErrorCode -eq 10048) {
                $result.Outcome = 'Incomplete'
                $result.IncompletePrecondition = 'Windows rejected owned exact-tuple TCP reuse with Socket 10048. Same-tuple/ended-row acceptance was not completed; this is neither a product failure nor a pass.'
                break
            }
        }
    }
    throw
} finally {
    if ($null -ne $helper) {
        if (-not $helper.HasExited) {
            $result.HelperCleanup = 'Owned UIA helper stopped after the parent journey ended'
            $helper.Kill(); $helper.WaitForExit(3000) | Out-Null
        }
        if ($helper.HasExited) { $result.HelperExitCode = $helper.ExitCode }
    }
    $uiaResultPath = Join-Path $EvidenceDirectory 'uia-result.json'
    if (Test-Path -LiteralPath $uiaResultPath) {
        try { $result.UIAutomation = Get-Content -LiteralPath $uiaResultPath -Raw | ConvertFrom-Json }
        catch { $result.PartialResultReadError = $_.Exception.Message }
    }
    $phasePath = Join-Path $EvidenceDirectory 'phase.txt'
    if (Test-Path -LiteralPath $phasePath) { $result.LastPhase = (Get-Content -LiteralPath $phasePath -Raw).Trim() }
    foreach ($socket in @($client, $server)) { if ($null -ne $socket) { $socket.Dispose() } }
    if ($null -ne $listener) { $listener.Stop() }
    if ($null -ne $app) {
        if (-not $app.HasExited) {
            if ($window -ne [IntPtr]::Zero -and [ExplorationProof]::Owner($window) -eq $app.Id) { [ExplorationProof]::Post($window, $app.Id, 0x111, 101) }
            if (-not $app.WaitForExit(8000)) { $result.Passed = $false; $result.CleanupFallback = 'Owned app process handle killed after bounded close'; $app.Kill(); $app.WaitForExit(3000) | Out-Null }
        }
        if ($app.HasExited) { $result.AppExitCode = $app.ExitCode; if ($app.ExitCode -ne 0) { $result.Passed = $false } }
    }
    $env:LOCALAPPDATA = $savedData
    $result.FinishedUtc = [DateTime]::UtcNow.ToString('o')
    $result | ConvertTo-Json -Depth 14 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json') -Encoding utf8NoBOM
}
if (-not $result.Passed) { throw 'Exploration cleanup did not complete cleanly; inspect result.json.' }
