#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $CityDatabase,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [switch] $MappingsOnly,
    [ValidateRange(90, 600)][int] $TimeoutSeconds = 240
)

# Actual release-product accounting, separate from controlled renderer fixtures.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ([IntPtr]::Size -ne 8) { throw 'Use 64-bit PowerShell for the native memory structures.' }
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$CityDatabase = (Resolve-Path -LiteralPath $CityDatabase).Path
$assets = Join-Path (Split-Path -Parent $Executable) 'assets'
foreach ($name in 'world.bin', 'country.mmdb') {
    if (-not (Test-Path -LiteralPath (Join-Path $assets $name) -PathType Leaf)) { throw "Missing adjacent product asset: $name" }
}
$cityHash = '9E250F02722D1AD1780F88192F0E908AF285478A33E76B88B349C9181AE9D0F5'
if ((Get-Item -LiteralPath $CityDatabase).Length -ne 126998165 -or
    (Get-FileHash -LiteralPath $CityDatabase -Algorithm SHA256).Hash -ne $cityHash) { throw 'Retained City MMDB size/hash differs.' }
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a fresh evidence directory.' }
New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$package = Join-Path $EvidenceDirectory 'product'
New-Item -ItemType Directory -Path $package | Out-Null
$privateExe = Join-Path $package 'netmap.exe'
$privateSaver = Join-Path $package 'netmap.scr'
Copy-Item -LiteralPath $Executable -Destination $privateExe
Copy-Item -LiteralPath $Executable -Destination $privateSaver
Copy-Item -LiteralPath $assets -Destination (Join-Path $package 'assets') -Recurse
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try { $administrator = [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) }
finally { $identity.Dispose() }

# Same owned-HWND/system-message pattern as native-settings.ps1. Pointer-bearing
# ListView messages are deliberately absent; only numeric LVM_GETITEMCOUNT is read.
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
public static class ProductResources {
    delegate bool Callback(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, Callback callback, IntPtr parameter);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr window, int id);
    [DllImport("user32.dll")] static extern int GetDlgCtrlID(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] static extern bool ShowWindowAsync(IntPtr window, int command);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr window, StringBuilder text, int capacity);
    [DllImport("user32.dll", SetLastError=true)] public static extern uint GetGuiResources(IntPtr process, uint kind);
    [DllImport("user32.dll")] static extern bool PostMessageW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, IntPtr wparam, IntPtr lparam, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageTimeoutW")] static extern IntPtr ReadText(IntPtr window, uint message, IntPtr capacity, StringBuilder text, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageTimeoutW")] static extern IntPtr WriteText(IntPtr window, uint message, IntPtr unused, string text, uint flags, uint timeout, out IntPtr result);
    public static bool Owned(IntPtr window, uint process) { GetWindowThreadProcessId(window, out uint owner); return window != IntPtr.Zero && owner == process; }
    public static void Show(IntPtr window, uint process) {
        if (!Owned(window,process) || !ShowWindowAsync(window,4)) throw new InvalidOperationException("Owned nonactivating show request failed");
    }
    public static IntPtr Find(uint process, int child) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, unused) => { if (Owned(window, process) && GetDlgItem(window, child) != IntPtr.Zero) { found=window; return false; } return true; }, IntPtr.Zero);
        return found;
    }
    public static IntPtr Child(IntPtr parent, uint process, int id) {
        if (!Owned(parent, process)) throw new InvalidOperationException("Retired owned parent");
        IntPtr found=IntPtr.Zero;
        EnumChildWindows(parent, (window, unused) => { if (GetDlgCtrlID(window)==id) { found=window; return false; } return true; }, IntPtr.Zero);
        return found;
    }
    public static void Post(IntPtr window, uint process, uint message, long wparam=0) {
        if (!Owned(window,process) || !PostMessageW(window,message,(IntPtr)wparam,IntPtr.Zero)) throw new InvalidOperationException("Owned post failed");
    }
    public static long Send(IntPtr window, uint process, uint message) {
        if (!Owned(window,process) || SendMessageTimeoutW(window,message,IntPtr.Zero,IntPtr.Zero,2,2000,out var result)==IntPtr.Zero) throw new InvalidOperationException("Owned numeric query timed out");
        return result.ToInt64();
    }
    public static string Text(IntPtr window, uint process) {
        var text=new StringBuilder(4096);
        if (!Owned(window,process) || ReadText(window,13,(IntPtr)text.Capacity,text,2,2000,out _) == IntPtr.Zero) throw new InvalidOperationException("Owned text query timed out");
        return text.ToString();
    }
    public static void SetText(IntPtr window, uint process, string text) {
        if (!Owned(window,process) || WriteText(window,12,IntPtr.Zero,text,2,2000,out _) == IntPtr.Zero) throw new InvalidOperationException("Owned edit update timed out");
    }
    public static IntPtr[] Savers(uint process) {
        var windows=new List<IntPtr>();
        EnumWindows((window,unused) => { var name=new StringBuilder(256); if (Owned(window,process) && GetClassNameW(window,name,name.Capacity)>0 && name.ToString()=="NetMap.ScreenSaver") windows.Add(window); return true; },IntPtr.Zero);
        return windows.ToArray();
    }

    [StructLayout(LayoutKind.Sequential)] struct Region { public IntPtr Base, AllocationBase; public uint AllocationProtect; public UIntPtr Size; public uint State, Protect, Type; }
    // SDK memoryapi.h WIN32_MEMORY_REGION_INFORMATION, x64 offsets 0/8/12/16/24.
    // C# has no native bit fields: expose the documented fields individually.
    [StructLayout(LayoutKind.Sequential)] struct Allocation {
        public IntPtr Base; public uint Protect, Kinds; public UIntPtr Size, Commit;
        public bool MappedDataFile => (Kinds & (1u<<1))!=0;
        public bool MappedPageFile => (Kinds & (1u<<3))!=0;
        public bool MappedPhysical => (Kinds & (1u<<4))!=0;
        public bool DirectMapped => (Kinds & (1u<<5))!=0;
        public uint NativeFlags => Kinds;
    }
    [StructLayout(LayoutKind.Sequential)] struct Page { public IntPtr Address; public UIntPtr Attributes; }
    [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access, bool inherit, uint process);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll",SetLastError=true)] static extern UIntPtr VirtualQueryEx(IntPtr process, IntPtr address, out Region region, UIntPtr size);
    [DllImport("api-ms-win-core-memory-l1-1-4.dll",SetLastError=true)] static extern bool QueryVirtualMemoryInformation(IntPtr process, IntPtr address, int informationClass, out Allocation information, UIntPtr size, out UIntPtr returned);
    [DllImport("psapi.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern uint GetMappedFileNameW(IntPtr process, IntPtr address, StringBuilder name, uint capacity);
    [DllImport("psapi.dll",SetLastError=true)] static extern bool QueryWorkingSetEx(IntPtr process, [In,Out] Page[] pages, uint bytes);
    public sealed class Mapping { public string File; public ulong VirtualBytes, ResidentBytes, SharedResidentBytes; }
    public sealed class Mappings { public Mapping[] Files; public string Error; public int Regions, UnresolvedRegions, TruncatedNames, FileRegions, PageFileRegions, PhysicalRegions, UnknownRegions, NativeStructureSize, ReturnedStructureSize; public string Scope="Read-only mapped data/direct file page residency at query time; documented pagefile/physical views excluded. Shared residency is not private commitment or exclusive ownership."; }
    public static Mappings ReadMappings(uint process) {
        var result=new Mappings();
        result.NativeStructureSize=Marshal.SizeOf<Allocation>();
        var files=new Dictionary<string,Mapping>(StringComparer.OrdinalIgnoreCase);
        var handle=OpenProcess(0x410,false,process);
        if (handle==IntPtr.Zero) { result.Error="OpenProcess query/read failed: "+Marshal.GetLastWin32Error(); result.Files=new Mapping[0]; return result; }
        var timer=Stopwatch.StartNew();
        try {
            if (result.NativeStructureSize!=32) throw new InvalidOperationException("WIN32_MEMORY_REGION_INFORMATION is not the supported x64 32-byte layout");
            ulong address=0;
            while (address<0x7fffffff0000UL) {
                if (++result.Regions>65536 || timer.ElapsedMilliseconds>2000) throw new InvalidOperationException("Mapped-region query exceeded its bounded scope");
                if (VirtualQueryEx(handle,(IntPtr)(long)address,out var region,(UIntPtr)Marshal.SizeOf<Region>())==UIntPtr.Zero) {
                    int error=Marshal.GetLastWin32Error();
                    if (error==87) break;
                    throw new InvalidOperationException("VirtualQueryEx failed: "+error);
                }
                ulong size=region.Size.ToUInt64(), start=(ulong)region.Base.ToInt64();
                if (size==0 || start+size<=address) throw new InvalidOperationException("Invalid virtual-memory region bounds");
                address=start+size;
                if (region.State!=0x1000 || region.Type!=0x40000) continue;
                // VirtualQueryEx reports data, pagefile and physical views as
                // MEM_MAPPED. Query their documented allocation kind first;
                // an unnamed pagefile view is not a failed MMDB filename query.
                if (!QueryVirtualMemoryInformation(handle,region.Base,0,out var allocation,(UIntPtr)result.NativeStructureSize,out var returned))
                    throw new InvalidOperationException("MemoryRegionInfo classification failed: "+Marshal.GetLastWin32Error());
                result.ReturnedStructureSize=checked((int)returned.ToUInt64());
                if (result.ReturnedStructureSize!=32 || allocation.Base!=region.AllocationBase)
                    throw new InvalidOperationException("MemoryRegionInfo size/allocation identity did not match the supported query");
                if (allocation.MappedDataFile || allocation.DirectMapped) result.FileRegions++;
                else if (allocation.MappedPageFile) { result.PageFileRegions++; continue; }
                else if (allocation.MappedPhysical) { result.PhysicalRegions++; continue; }
                else {
                    result.UnknownRegions++;
                    if (result.Error==null) result.Error="Unknown MEM_MAPPED allocation kind: 0x"+allocation.NativeFlags.ToString("x");
                    continue;
                }
                var name=new StringBuilder(2048);
                uint length=GetMappedFileNameW(handle,region.Base,name,(uint)name.Capacity);
                if (length==0) {
                    result.UnresolvedRegions++;
                    if (result.Error==null) result.Error="Unresolved committed MEM_MAPPED region; GetMappedFileNameW error: "+Marshal.GetLastWin32Error();
                    continue;
                }
                if (length>=(uint)name.Capacity-1) {
                    result.TruncatedNames++;
                    if (result.Error==null) result.Error="Mapped filename reached buffer capacity; mapping identity is unavailable (GetMappedFileNameW error: "+Marshal.GetLastWin32Error()+")";
                    continue;
                }
                string file=name.ToString();
                if (!file.EndsWith("\\city.mmdb",StringComparison.OrdinalIgnoreCase) && !file.EndsWith("\\country.mmdb",StringComparison.OrdinalIgnoreCase)) continue;
                if (!files.TryGetValue(file,out var mapping)) { mapping=new Mapping {File=file}; files.Add(file,mapping); }
                mapping.VirtualBytes+=size; // contiguous VirtualQueryEx span, not whole allocation Size
                int count=checked((int)((size+(ulong)Environment.SystemPageSize-1)/(ulong)Environment.SystemPageSize));
                if (count>65536) throw new InvalidOperationException("MMDB residency page query exceeds retained dataset bounds");
                var pages=new Page[count];
                for(int i=0;i<count;i++) pages[i].Address=(IntPtr)(long)(start+(ulong)i*(ulong)Environment.SystemPageSize);
                if (!QueryWorkingSetEx(handle,pages,checked((uint)(count*Marshal.SizeOf<Page>())))) throw new InvalidOperationException("QueryWorkingSetEx failed: "+Marshal.GetLastWin32Error());
                foreach(var page in pages) { ulong flags=page.Attributes.ToUInt64(); if ((flags&1)!=0) { mapping.ResidentBytes+=(ulong)Environment.SystemPageSize; if ((flags&(1UL<<15))!=0) mapping.SharedResidentBytes+=(ulong)Environment.SystemPageSize; } }
            }
        } catch(Exception error) { result.Error=error.Message; }
        finally { CloseHandle(handle); }
        result.Files=new List<Mapping>(files.Values).ToArray();
        return result;
    }
}
'@

$result = [ordered]@{
    Executable = $Executable; ExecutableSHA256 = (Get-FileHash -LiteralPath $Executable).Hash
    PrivateExecutable = $privateExe; PrivateExecutableSHA256 = (Get-FileHash -LiteralPath $privateExe).Hash
    PrivateSaverSHA256 = (Get-FileHash -LiteralPath $privateSaver).Hash
    Assets = @(Get-ChildItem -LiteralPath $assets -File | ForEach-Object { @{ Name=$_.Name; Bytes=$_.Length; SHA256=(Get-FileHash -LiteralPath $_.FullName).Hash; CopySHA256=(Get-FileHash -LiteralPath (Join-Path (Join-Path $package 'assets') $_.Name)).Hash } })
    City = @{ Source=$CityDatabase; Bytes=126998165; SHA256=$cityHash }
    IsAdministrator = $administrator
    Scope = 'Two fresh actual product arms with changing restricted live facts: country-only then city-mapped. Settings first/lazy retained pages, warm reopen, visible/hidden list, and three real Settings Preview children. Fixed settling intervals are warming diagnostics, not allocation causality, memory acceptance, fixed-fact parity, frame/GPU execution timing or elevated collector totals. No public discovery/download, install/OS mutation, UAC or global input.'
    MappingScope = 'Virtual mapped bytes and valid/shareable resident MMDB pages are separate from process private bytes/working set. No trimming or file touching. Any Error invalidates the mapping measurement; partial file records are not totals, and unavailable does not mean zero residency.'
    MappingSources = @(
        'https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/ns-memoryapi-win32_memory_region_information',
        'https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-queryvirtualmemoryinformation',
        'https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualqueryex',
        'https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getmappedfilenamew',
        'https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-queryworkingsetex'
    )
    MappingsOnly = [bool]$MappingsOnly
    Samples = @(); Lifecycles = @(); Arms = @(); Cleanup = @(); Passed = $false
}
if ($MappingsOnly) {
    $result.Scope = 'One fresh ordinary-token actual city-profile product after restricted/manual-origin/local-data and visible-window readiness; one named country/city MMDB residency query after one-second settle, then owned-parent close. Read-only allocation classification distinguishes data/direct-file views from pagefile/physical views; all unknown/API/name/query errors remain unavailable. No list, Settings, Preview, GPU or nineteen-phase rerun. No residency inferred from file size.'
}
$owned = [Collections.Generic.List[Diagnostics.Process]]::new()
$deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
$savedData = $env:LOCALAPPDATA
$counterJob = $null
$process = $null
$window = [IntPtr]::Zero
$dialog = [IntPtr]::Zero
$previewSince = [DateTime]::MinValue
$firstPreviewSince = [DateTime]::MinValue
$arm = ''

function Require([bool] $Value, [string] $Reason) { if (-not $Value) { throw $Reason } }
function Check-Deadline { Require ([DateTime]::UtcNow -lt $deadline) 'Product resource journey exceeded its overall watchdog.' }
function Wait-Ready([scriptblock] $Condition, [string] $Reason) {
    $until = [DateTime]::UtcNow.AddSeconds(15)
    do {
        Check-Deadline
        if ($process.HasExited) { throw "Owned parent exited during $Reason with $($process.ExitCode)." }
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $until)
    throw $Reason
}
function Native-Child([int] $Id) { [ProductResources]::Child($dialog, $process.Id, $Id) }
function Open-Settings {
    [ProductResources]::Post($window, $process.Id, 0x111, 107)
    Wait-Ready { $script:dialog = [ProductResources]::Find($process.Id, 1100); $dialog -ne [IntPtr]::Zero } 'Settings did not open.'
}
function Search-Page([string] $Search, [int] $Control) {
    [ProductResources]::SetText((Native-Child 1100), $process.Id, $Search)
    Wait-Ready { $child = Native-Child $Control; $child -ne [IntPtr]::Zero -and [ProductResources]::IsWindowVisible($child) } "Settings page $Search did not become ready."
}
function Close-Settings {
    [ProductResources]::Post($dialog, $process.Id, 0x111, 2)
    Wait-Ready { [ProductResources]::Find($process.Id, 1100) -eq [IntPtr]::Zero } 'Settings Cancel did not close.'
    $script:dialog = [IntPtr]::Zero
}
function GPU([int] $OwnedId) {
    # Reuse native-map-resources' bounded post-sample counter collection.
    $script:counterJob = Start-Job -ArgumentList $OwnedId -ScriptBlock {
        param($ownedId)
        try {
            $sample = Get-Counter -Counter '\GPU Process Memory(*)\Dedicated Usage', '\GPU Process Memory(*)\Shared Usage' -ErrorAction Stop
            $rows = @($sample.CounterSamples | Where-Object InstanceName -match "^pid_${ownedId}_" | ForEach-Object {
                @{ Instance=$_.InstanceName; Path=$_.Path; Bytes=$_.CookedValue; Status=$_.Status; TimeUtc=$_.Timestamp.ToUniversalTime().ToString('o') }
            })
            $errorText = if (-not $rows.Count) { 'No matching GPU allocation counter instance.' } elseif (@($rows | Where-Object Status -ne 0).Count) { 'Invalid GPU counter status.' } else { $null }
            @{ Counters=$rows; Error=$errorText; Scope='Per owned PID/adapter allocations after CPU sampling; not GPU execution.' }
        } catch { @{ Counters=@(); Error=$_.Exception.Message } }
    }
    try {
        if (Wait-Job -Job $counterJob -Timeout 5) { Receive-Job -Job $counterJob }
        else { Stop-Job -Job $counterJob; @{ Counters=@(); Error='GPU allocation query timed out.' } }
    } finally { Remove-Job -Job $counterJob -Force; $script:counterJob = $null }
}
function Sample([string] $Phase, [Diagnostics.Process] $Target = $process) {
    Check-Deadline
    Start-Sleep -Milliseconds 1000
    $Target.Refresh()
    Require (-not $Target.HasExited) "Owned target exited before sample $Phase."
    $start = [DateTime]::UtcNow
    $cpu = $Target.TotalProcessorTime.TotalMilliseconds
    $values = @()
    for ($index=0; $index -lt 3; $index++) {
        if ($index) { Start-Sleep -Milliseconds 500 }
        $Target.Refresh()
        $values += [ordered]@{ TimeUtc=[DateTime]::UtcNow.ToString('o'); PrivateBytes=$Target.PrivateMemorySize64; WorkingSetBytes=$Target.WorkingSet64; Handles=$Target.HandleCount; GDI=[ProductResources]::GetGuiResources($Target.Handle,0); USER=[ProductResources]::GetGuiResources($Target.Handle,1) }
    }
    $endCpu = $Target.TotalProcessorTime.TotalMilliseconds
    $end = [DateTime]::UtcNow
    $duration = ($end-$start).TotalMilliseconds
    $cpuDelta = $endCpu-$cpu
    $status = if (-not $process.HasExited) { [ProductResources]::Text([ProductResources]::GetDlgItem($window,1001),$process.Id) } else { $null }
    $table = [ProductResources]::GetDlgItem($window,1004)
    $mapBounds=[ProductResources+Rect]::new()
    Require ([ProductResources]::GetClientRect([ProductResources]::GetDlgItem($window,1000),[ref]$mapBounds)) 'Parent map dimensions unavailable.'
    $sample = [ordered]@{ Arm=$arm; Phase=$Phase; ProcessId=$Target.Id; StartedUtc=$Target.StartTime.ToUniversalTime().ToString('o'); SampleStartUtc=$start.ToString('o'); SampleEndUtc=$end.ToString('o'); SampleWindowMs=$duration; CpuMs=$cpuDelta; SettleMs=1000; Values=$values; ParentStatus=$status; ParentVisible=[ProductResources]::IsWindowVisible($window); ParentRows=[ProductResources]::Send($table,$process.Id,0x1004); ParentTableVisible=[ProductResources]::IsWindowVisible($table); ParentMapBounds=$mapBounds; Mappings=[ProductResources]::ReadMappings($Target.Id); GPU=(GPU $Target.Id) }
    $result.Samples += $sample
    $sample | ConvertTo-Json -Depth 8 -Compress | Add-Content -LiteralPath (Join-Path $EvidenceDirectory 'samples.jsonl') -Encoding utf8NoBOM
}
function Find-Preview([switch] $All) {
    foreach ($candidate in [Diagnostics.Process]::GetProcesses()) {
        if ($null -eq $candidate) { continue }
        $retain = $false
        try {
            $null = $candidate.Handle
            if (-not $candidate.HasExited -and $candidate.StartTime.ToUniversalTime() -ge $previewSince -and
                [StringComparer]::OrdinalIgnoreCase.Equals($candidate.MainModule.FileName,$privateSaver)) {
                $retain=$true
                if ($All) { $candidate } else { return $candidate }
            }
        } catch { }
        finally { if (-not $retain) { $candidate.Dispose() } }
    }
    if (-not $All) { return $null }
}
function Close-OwnedTarget([Diagnostics.Process] $Target) {
    $cleanup = [ordered]@{ ProcessId=$Target.Id; Killed=$false; ExitObserved=$false }
    try {
        if (-not $Target.HasExited) {
            foreach ($surface in @([ProductResources]::Savers($Target.Id))) { [ProductResources]::Post($surface,$Target.Id,0x10) }
            $settingsWindow=[ProductResources]::Find($Target.Id,1100)
            if ($settingsWindow -ne [IntPtr]::Zero) { [ProductResources]::Post($settingsWindow,$Target.Id,0x111,2) }
            $main=[ProductResources]::Find($Target.Id,1000)
            if ($main -ne [IntPtr]::Zero) { [ProductResources]::Post($main,$Target.Id,0x10) }
            if (-not $Target.WaitForExit(3000)) { $Target.Kill(); $cleanup.Killed=$true; $cleanup.ExitObserved=$Target.WaitForExit(5000) }
        }
        if ($Target.HasExited) { $cleanup.ExitObserved=$true; $cleanup.ExitCode=$Target.ExitCode }
    } catch {
        $cleanup.Error=$_.Exception.ToString()
        try {
            if (-not $Target.HasExited) { $Target.Kill(); $cleanup.Killed=$true; $cleanup.ExitObserved=$Target.WaitForExit(5000) }
        } catch { $cleanup.TerminationError=$_.Exception.ToString() }
    } finally { $result.Cleanup += $cleanup; $Target.Dispose() }
}

try {
    Require (-not $administrator) 'Use an ordinary Windows token.'
    Require ($result.ExecutableSHA256 -eq $result.PrivateExecutableSHA256 -and $result.ExecutableSHA256 -eq $result.PrivateSaverSHA256) 'Private EXE/SCR copy hashes differ.'
    Require (@($result.Assets | Where-Object { $_.SHA256 -ne $_.CopySHA256 }).Count -eq 0) 'Copied product asset hashes differ.'
    $arms = if ($MappingsOnly) { @('city') } else { @('country','city') }
    foreach ($armName in $arms) {
        $arm = $armName
        $profileRoot = Join-Path $EvidenceDirectory $arm
        $data = Join-Path $profileRoot 'NetMap'
        New-Item -ItemType Directory -Path $data | Out-Null
        $preferences = Join-Path $data 'preferences.txt'
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'fixtures/manual-preferences.txt') -Destination $preferences
        $preferenceHash = (Get-FileHash -LiteralPath $preferences).Hash
        if ($arm -eq 'city') {
            Copy-Item -LiteralPath $CityDatabase -Destination (Join-Path $data 'city.mmdb')
            Require ((Get-FileHash -LiteralPath (Join-Path $data 'city.mmdb')).Hash -eq $cityHash) 'Isolated city copy hash differs.'
        }
        $env:LOCALAPPDATA = $profileRoot
        $process = Start-Process -FilePath $privateExe -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $profileRoot 'stdout.txt') -RedirectStandardError (Join-Path $profileRoot 'stderr.txt')
        $owned.Add($process)
        $armRecord = [ordered]@{ Name=$arm; ProcessId=$process.Id; StartedUtc=$process.StartTime.ToUniversalTime().ToString('o'); ReadinessUtc=$null; ExitCode=$null }
        $result.Arms += $armRecord
        Wait-Ready { $script:window=[ProductResources]::Find($process.Id,1000); $window -ne [IntPtr]::Zero } 'Owned product map did not open.'
        [ProductResources]::Show($window,$process.Id)
        Wait-Ready { [ProductResources]::IsWindowVisible($window) } 'Owned nonactivating main-window show did not complete.'
        Wait-Ready {
            $text=[ProductResources]::Text([ProductResources]::GetDlgItem($window,1001),$process.Id)
            $text -match 'Capture restricted' -and $text -match 'Manual origin: 0, 0' -and $text -match 'located; .* unmapped' -and $text -notmatch 'Country data (is missing|unavailable)' -and
                ($arm -ne 'city' -or $text -match 'City data ready')
        } 'Actual restricted facts/local location readiness did not settle.'
        $armRecord.ReadinessUtc=[DateTime]::UtcNow.ToString('o')
        if ($MappingsOnly) {
            Start-Sleep -Milliseconds 1000
            Check-Deadline
            $mapping = [ProductResources]::ReadMappings($process.Id)
            $expected = @((Join-Path (Join-Path $package 'assets') 'country.mmdb'), (Join-Path $data 'city.mmdb'))
            $matched = @()
            foreach ($expectedPath in $expected) {
                # Native device paths have a different volume prefix. Match the
                # complete fresh private directory suffix, not just the basename.
                $fullPath=[IO.Path]::GetFullPath($expectedPath)
                $suffix=$fullPath.Substring([IO.Path]::GetPathRoot($fullPath).Length-1)
                $files=@($mapping.Files | Where-Object { $_.File.EndsWith($suffix,[StringComparison]::OrdinalIgnoreCase) })
                if ($files.Count -ne 1 -or $files[0].VirtualBytes -eq 0) {
                    if ($null -eq $mapping.Error) { $mapping.Error="Expected private MMDB mapping was not identified uniquely: $fullPath" }
                } else {
                    $matched += @{ ExpectedPath=$fullPath; DevicePath=$files[0].File; SHA256=$(if ([IO.Path]::GetFileName($fullPath) -eq 'city.mmdb') { $cityHash } else { ($result.Assets | Where-Object Name -eq 'country.mmdb').CopySHA256 }) }
                }
            }
            $result.Mapping = [ordered]@{ ProcessId=$process.Id; StartedUtc=$process.StartTime.ToUniversalTime().ToString('o'); QueriedUtc=[DateTime]::UtcNow.ToString('o'); WindowsVersion=[Environment]::OSVersion.VersionString; ParentVisible=[ProductResources]::IsWindowVisible($window); ParentStatus=[ProductResources]::Text([ProductResources]::GetDlgItem($window,1001),$process.Id); Available=($null -eq $mapping.Error); ExpectedPaths=$expected; LoadedFiles=$matched; Measurement=$mapping }
            $result.Mapping | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'mapping.json') -Encoding utf8
            [ProductResources]::Post($window,$process.Id,0x10)
            Require ($process.WaitForExit(6000) -and $process.ExitCode -eq 0) 'Owned mapping parent did not exit normally.'
            $armRecord.ExitCode=$process.ExitCode
            Require ((Get-FileHash -LiteralPath $preferences).Hash -eq $preferenceHash) 'Mapping query changed preferences.'
            Require ((Get-FileHash -LiteralPath (Join-Path $data 'city.mmdb')).Hash -eq $cityHash) 'Mapping query changed city data.'
            $window=[IntPtr]::Zero
            continue
        }
        Sample 'map-baseline'
        [ProductResources]::Post($window,$process.Id,0x111,116)
        Wait-Ready { [ProductResources]::IsWindowVisible([ProductResources]::GetDlgItem($window,1004)) } 'All connections did not show native table.'
        Sample 'all-connections'
        [ProductResources]::Post($window,$process.Id,0x111,113)
        Wait-Ready { -not [ProductResources]::IsWindowVisible([ProductResources]::GetDlgItem($window,1004)) } 'Map-only did not hide native table.'
        if ($arm -eq 'country') {
            Open-Settings
            Wait-Ready { [ProductResources]::IsWindowVisible((Native-Child 1113)) } 'Initial Map settings page missing.'
            Sample 'settings-first-map'
            foreach ($page in @(@('connections','connections',1121),@('location','longitude',1133),@('desktop','desktop',1150),@('screen saver','screen saver',1160))) {
                Search-Page $page[1] $page[2]
                Sample "settings-lazy-$($page[0])"
            }
            Close-Settings
            Sample 'settings-closed'
            for ($cycle=1; $cycle -le 3; $cycle++) {
                Open-Settings
                Search-Page 'desktop' 1150
                Close-Settings
            }
            Sample 'settings-three-reopens-closed'
        } else {
            Open-Settings
            Search-Page 'longitude' 1133
            Wait-Ready { [ProductResources]::Text((Native-Child 1138),$process.Id) -match 'City.*ready|City.*[0-9]{4}-[0-9]{2}' } 'Location page did not report real City availability.'
            Sample 'settings-city-location'
            Search-Page 'screen saver' 1160
            Wait-Ready { [ProductResources]::Text((Native-Child 1160),$process.Id) -match '^Screen saver file is available' } 'Private SCR availability did not settle.'
            for ($cycle=1; $cycle -le 3; $cycle++) {
                $previewSince=[DateTime]::UtcNow
                if ($firstPreviewSince -eq [DateTime]::MinValue) { $firstPreviewSince=$previewSince }
                [ProductResources]::Post($dialog,$process.Id,0x111,1163)
                $script:saver=$null
                Wait-Ready { $script:saver=Find-Preview; $null -ne $saver } 'Actual Settings Preview did not launch its private SCR.'
                $owned.Add($saver)
                Wait-Ready { @([ProductResources]::Savers($saver.Id)).Count -gt 0 } 'Owned Preview had no native saver targets.'
                Sample "preview-$cycle-live" $saver
                $surfaces=@([ProductResources]::Savers($saver.Id))
                Require ($surfaces.Count -gt 0) 'Owned Preview targets retired before close.'
                [ProductResources]::Post($surfaces[0],$saver.Id,0x10)
                Require ($saver.WaitForExit(6000) -and $saver.ExitCode -eq 0) 'Owned Preview did not exit normally.'
                $result.Lifecycles += @{ Cycle=$cycle; ProcessId=$saver.Id; StartedUtc=$saver.StartTime.ToUniversalTime().ToString('o'); Surfaces=$surfaces.Count; ExitCode=$saver.ExitCode }
                Sample "preview-$cycle-parent-after-exit"
            }
            Close-Settings
            Sample 'preview-three-cycles-settings-closed'
            Require ((Get-FileHash -LiteralPath (Join-Path $data 'city.mmdb')).Hash -eq $cityHash) 'Resource journey changed the city dataset.'
        }
        Require ((Get-FileHash -LiteralPath $preferences).Hash -eq $preferenceHash) 'Read-only Settings/Preview resource journey changed preferences.'
        [ProductResources]::Post($window,$process.Id,0x10)
        Require ($process.WaitForExit(6000) -and $process.ExitCode -eq 0) 'Owned parent did not exit normally.'
        $armRecord.ExitCode=$process.ExitCode
        $window=[IntPtr]::Zero
        $dialog=[IntPtr]::Zero
    }
    $result.Passed=if ($MappingsOnly) { $result.Mapping.Available } else { $true }
} catch {
    $result.Error=$_.Exception.ToString()
    $result.Invocation=$_.InvocationInfo.PositionMessage
    $result.ScriptStackTrace=$_.ScriptStackTrace
    throw
} finally {
    if ($null -ne $counterJob) { Stop-Job -Job $counterJob; Remove-Job -Job $counterJob -Force }
    foreach ($target in $owned) { Close-OwnedTarget $target }
    # Scan AFTER launchers have exited/been terminated: Shell may finish an
    # accepted Preview during parent cleanup. Close every exact private-path,
    # birth-time match observed in this bounded five-second late-launch window.
    if ($firstPreviewSince -ne [DateTime]::MinValue) {
        $previewSince=$firstPreviewSince
        $scanUntil=[DateTime]::UtcNow.AddSeconds(5)
        do {
            foreach ($late in @(Find-Preview -All)) { Close-OwnedTarget $late }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $scanUntil)
        $result.LatePreviewScanSeconds=5
    }
    if (@($result.Cleanup | Where-Object { -not $_.ExitObserved }).Count) {
        $result.Passed=$false
        $result.CleanupIncomplete=$true
    }
    $env:LOCALAPPDATA=$savedData
    $result.RecordedUtc=[DateTime]::UtcNow.ToString('o')
    $result | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'result.json') -Encoding utf8
}
