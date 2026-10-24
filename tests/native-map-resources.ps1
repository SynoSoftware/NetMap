#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $Executable,
    [Parameter(Mandatory)][string] $ReferenceDirectory,
    [Parameter(Mandatory)][string] $EvidenceDirectory,
    [ValidateRange(30, 1800)][int] $TimeoutSeconds = 1200,
    [switch] $GpuOnly,
    [switch] $VisualFamilies,
    [ValidateSet('control', 'presentation', 'text', 'treatment')][string] $ColdArm
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ($ColdArm -and $GpuOnly) { throw '-ColdArm and -GpuOnly are separate measurement modes.' }
if ($VisualFamilies -and ($ColdArm -or $GpuOnly)) { throw '-VisualFamilies is an images-only mode.' }
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$ReferenceDirectory = (Resolve-Path -LiteralPath $ReferenceDirectory).Path
if (Test-Path -LiteralPath $EvidenceDirectory) { throw 'Use a new evidence directory.' }

function Assert([bool] $condition, [string] $message) {
    if (-not $condition) { throw $message }
}

function Read-Json([string] $name) {
    Get-Content -LiteralPath (Join-Path $ReferenceDirectory $name) -Raw | ConvertFrom-Json
}

function Number($value) {
    ([double]$value).ToString('R', [Globalization.CultureInfo]::InvariantCulture)
}

function Argb($color) {
    '{0:x2}{1:x2}{2:x2}{3:x2}' -f [int]$color.A, [int]$color.R, [int]$color.G, [int]$color.B
}

function Write-VisualInputs($visual) {
    $index = [Collections.Generic.List[string]]::new()
    foreach ($case in $visual.cases) {
        Assert ($case.id -match '^[a-z0-9-]+$' -and $case.target_width -eq 1920 -and $case.target_height -eq 1080) 'Invalid visual case/extent.'
        Assert ($case.image.file -eq "reference-$($case.id).png" -and $case.image_sha256 -eq (Get-FileHash -LiteralPath (Join-Path $ReferenceDirectory $case.image.file)).Hash) 'Source image hash differs.'
        Assert (($case.theme -in @('light','dark')) -and [math]::Abs($case.dpi - [math]::Round($case.dpi)) -lt .000001 -and $case.dpi -eq $case.image.dpi -and [math]::Abs($case.dpi / 96 - $case.scale) -lt .000001) 'Unsupported visual theme/DPI.'
        $rows = [Collections.Generic.List[string]]::new()
        $rows.Add((@((Number $case.origin.longitude), (Number $case.origin.latitude), (Number $case.scale), $case.theme,
            (Argb $case.image.download), (Argb $case.image.background), (Argb $case.image.ink), (Argb $case.image.land), (Argb $case.image.edge), (Argb $case.image.upload)) -join "`t"))
        $detail = if ($case.cities) { 'cities' } else { 'countries' }
        $rows.Add((@('scene', $case.id, 12, 0, 0, 0, @($case.destinations).Count, 0, $detail) -join "`t"))
        foreach ($peer in $visual.peers) {
            Assert ($peer.country_name -notmatch "[`t`r`n]" -and $peer.city_name -notmatch "[`t`r`n]") 'Visual names contain delimiters.'
            Assert ($peer.state -eq 'connected' -and $peer.protocol -eq 'tcp' -and $peer.direction -eq 'outbound' -and $peer.receive_bytes_per_second -eq 0 -and $peer.send_bytes_per_second -eq 0) 'Visual peer policy changed.'
            $rows.Add((@('peer', $peer.endpoint, $peer.country_code, (Number $peer.longitude), (Number $peer.latitude), 0, 0, $peer.country_name, $peer.city_name) -join "`t"))
        }
        foreach ($destination in $case.destinations) {
            Assert ([string]$destination.Area -notmatch "[`t`r`n]") 'Destination area contains delimiters.'
            $rows.Add((@('destination', 1920, $destination.country_code, (Number $destination.longitude), (Number $destination.latitude), (Number $destination.Share),
                $destination.Rank, $destination.Count, $destination.Active, (Number $destination.receive_bytes_per_second), (Number $destination.send_bytes_per_second),
                ([string][bool]$destination.Estimated).ToLowerInvariant(), $destination.Area, ([string][bool]$destination.marked).ToLowerInvariant(), ([string][bool]$destination.pointed).ToLowerInvariant()) -join "`t"))
        }
        $rows | Set-Content -LiteralPath (Join-Path $EvidenceDirectory "visual-$($case.id).tsv") -Encoding utf8NoBOM
        $view = $case.viewport
        $index.Add((@($case.id, 1920, 1080, [int]$case.dpi, (Number $view.meridian), (Number $view.zoom), (Number $view.ratio), (Number $view.scale),
            (Number $view.offset_x), (Number $view.offset_y), (Number $view.center_x), (Number $view.center_y), (Number $view.width), (Number $view.height),
            ([string][bool]$visual.animations).ToLowerInvariant(), (Number $case.requested_view.center_x), (Number $case.requested_view.center_y), $case.requested_view.full_turns,
            $case.image.file, $case.image_sha256, $case.theme, $detail) -join "`t"))
    }
    $index | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'visual-cases.tsv') -Encoding utf8NoBOM
    Copy-Item -LiteralPath (Join-Path $ReferenceDirectory 'visual-families.json') -Destination (Join-Path $EvidenceDirectory 'visual-families.json')
}

$fixtureName = if ($VisualFamilies) { 'visual-families.json' } else { 'baseline-fixtures.json' }
if ($VisualFamilies) {
    $visual = Read-Json $fixtureName
    Assert ($visual.schema -eq 1 -and $visual.source_revision -eq '95be5335eff5261d4f94b2e77a4bc0db0dcbf151' -and -not $visual.high_contrast -and $visual.language -eq 'en') 'Unsupported visual manifest.'
    Assert ($visual.case_budget -eq 12 -and @($visual.peers).Count -eq 12 -and @($visual.cases).Count -gt 0 -and (@($visual.cases).Count + @($visual.failures).Count) -eq 12) 'Visual case accounting is incomplete.'
    Assert (@($visual.cases.id | Sort-Object -Unique).Count -eq @($visual.cases).Count) 'Duplicate visual case.'
    $referenceGpuFailed = $false
    $theme = 'per-case'
    $scale = 'per-case'
} else {
$fixtures = Read-Json 'baseline-fixtures.json'
$review = Read-Json 'review.json'
Assert ($null -eq $review.failure) 'The reference capture failed; retain its failure before comparing.'
Assert ($fixtures.source_revision -eq '95be5335eff5261d4f94b2e77a4bc0db0dcbf151') 'Unexpected reference revision.'
Assert ($fixtures.detail -eq 'countries') 'Only the countries-detail matched fixture is supported.'
Assert ($fixtures.schema -eq 1 -and $fixtures.selection -eq 'none' -and $fixtures.hover -eq 'none') 'Unexpected reference scene schema or selection.'
$referenceSamples = @($review.outcomes | Where-Object journey -eq 'netmap-reference-baseline')
$palettes = @($review.outcomes | Where-Object journey -eq 'reference palette')
Assert ($referenceSamples.Count -eq $fixtures.scenes.Count * 12) 'The reference memory matrix is incomplete.'
Assert ($palettes.Count -eq $fixtures.scenes.Count * 2) 'The reference palette matrix is incomplete.'
$referenceGpu = @(Get-Content -LiteralPath (Join-Path $ReferenceDirectory 'baseline-gpu.jsonl') | ForEach-Object { $_ | ConvertFrom-Json })
Assert ($referenceGpu.Count -eq $referenceSamples.Count) 'The reference GPU checkpoint matrix is incomplete.'
# A reference counter failure is retained as an evidence gap and does not
# prevent measuring native CPU/private bytes against the same source scenes.
$referenceGpuFailed = @($referenceGpu | Where-Object { $null -ne $_.error }).Count -gt 0
$scales = @($referenceSamples.rasterization_scale | Sort-Object -Unique)
Assert ($scales.Count -eq 1) 'Reference DPI changed; one stable scale is required for this comparison.'
$scale = [double]$scales[0]
Assert ([math]::Abs([math]::Round($scale * 96) / 96 - $scale) -lt 0.000001) 'Reference scale cannot be expressed by native integer DPI.'
$theme = ([string]$palettes[0].theme).ToLowerInvariant()
Assert ($theme -in 'light', 'dark') 'Unsupported reference theme.'
$download = $palettes[0].download
$accent = '{0:x2}{1:x2}{2:x2}{3:x2}' -f [int]$download.A, [int]$download.R, [int]$download.G, [int]$download.B
foreach ($palette in $palettes) {
    Assert (([string]$palette.theme).ToLowerInvariant() -eq $theme) 'Reference theme changed during the run.'
    Assert (($palette.download | ConvertTo-Json -Compress) -eq ($download | ConvertTo-Json -Compress)) 'Reference accent changed during the run.'
    # Each role is checked rather than assuming the OS theme resolved the
    # source XAML resources to the ported palette.
    $expected = if ($theme -eq 'dark') {
        @{ background = 'ff2c2c2c'; ink = 'ffffffff'; land = '19000000'; edge = '8bffffff'; upload = 'ffc5a3e6' }
    } else {
        @{ background = 'ffffffff'; ink = 'e4000000'; land = '06000000'; edge = '72000000'; upload = 'ff7756a3' }
    }
    foreach ($role in $expected.Keys) {
        $color = $palette.$role
        $actual = '{0:x2}{1:x2}{2:x2}{3:x2}' -f [int]$color.A, [int]$color.R, [int]$color.G, [int]$color.B
        Assert ($actual -eq $expected[$role]) "Reference $role palette differs from the production NetMap palette."
    }
}

if ($ColdArm) {
    $fixtures.scenes = @($fixtures.scenes | Where-Object name -eq 'active-100')
    Assert ($fixtures.scenes.Count -eq 1) 'Cold contribution requires the accepted active-100 scene.'
    Assert ((Get-FileHash -LiteralPath (Join-Path $ReferenceDirectory 'baseline-fixtures.json')).Hash -eq 'FB7586CF45FA7B0CE17E0EDAC6AFB84D16AE1BC0C13F21D9F60966BA105FF1BB') 'Cold contribution requires the already accepted exact exported facts.'
    Assert ($theme -eq 'dark' -and $scale -eq 1 -and $fixtures.origin.country_code -eq 'GB' -and $fixtures.origin.longitude -eq -.12 -and $fixtures.origin.latitude -eq 51.5) 'Cold contribution origin/theme/DPI differs.'
}
$lines = [Collections.Generic.List[string]]::new()
$lines.Add((@((Number $fixtures.origin.longitude), (Number $fixtures.origin.latitude), (Number $scale), $theme, $accent) -join "`t"))
foreach ($scene in $fixtures.scenes) {
    Assert ($scene.Name -match '^(idle|active|mixed)-[0-9]+$') 'Unexpected fixture scene name.'
    $peers = @($scene.peers)
    Assert ($peers.Count -gt 0 -and $peers.Count -le 10000) 'Invalid fixture size.'
    $active = @($peers | Where-Object { $_.receive_bytes_per_second + $_.send_bytes_per_second -gt 0 }).Count
    $down = ($peers | Measure-Object receive_bytes_per_second -Sum).Sum
    $up = ($peers | Measure-Object send_bytes_per_second -Sum).Sum
    Assert ($scene.connections -eq $peers.Count -and $scene.active -eq $active -and $scene.receive_bytes_per_second -eq $down -and $scene.send_bytes_per_second -eq $up) 'The exported scene aggregate is inconsistent.'
    $destinations = foreach ($width in 1920, 3840) {
        $samples = @($referenceSamples | Where-Object { $_.scene -eq $scene.Name -and $_.target_width -eq $width })
        Assert ($samples.Count -eq 6) 'Reference scene is missing lifetime phases.'
        $counts = @($samples.destinations | Sort-Object -Unique)
        Assert ($counts.Count -eq 1 -and $counts[0] -gt 0) 'Reference destinations changed during sampling.'
        foreach ($sample in $samples) {
            Assert ($sample.target_height -eq $width * 9 / 16) 'Reference target aspect ratio differs.'
            Assert ($sample.connections -eq $peers.Count -and $sample.active -eq $active -and $sample.Down -eq $down -and $sample.Up -eq $up) 'Reference aggregate facts differ from exported peers.'
            if ($sample.visible) {
                Assert ([math]::Abs($sample.map_width * $scale - $width) -lt .1 -and [math]::Abs($sample.canvas_width * $scale - $width) -lt .1) 'Reference width was clamped.'
                Assert ([math]::Abs($sample.map_height * $scale - $sample.target_height) -lt .1 -and [math]::Abs($sample.canvas_height * $scale - $sample.target_height) -lt .1) 'Reference height was clamped.'
            }
        }
        [int]$counts[0]
    }
    $lines.Add((@('scene', $scene.Name, $peers.Count, $active, (Number $down), (Number $up), $destinations[0], $destinations[1], 'countries') -join "`t"))
    foreach ($peer in $peers) {
        Assert ($peer.country_code -match '^[A-Z]{2}$' -and $null -ne $peer.longitude -and $null -ne $peer.latitude) 'The fixture needs exact known-country coordinates.'
        Assert ($peer.receive_bytes_per_second -ge 0 -and $peer.send_bytes_per_second -ge 0) 'Invalid fixture rate.'
        Assert ($peer.protocol -eq 'tcp' -and $peer.direction -eq 'outbound' -and $peer.state -eq 'connected') 'Unsupported reference connection facts.'
        $lines.Add((@('peer', $peer.endpoint, $peer.country_code, (Number $peer.longitude), (Number $peer.latitude), (Number $peer.receive_bytes_per_second), (Number $peer.send_bytes_per_second)) -join "`t"))
    }
    foreach ($palette in @($palettes | Where-Object scene -eq $scene.Name)) {
        Assert (@($palette.destinations).Count -eq $destinations[[int]([int]$palette.Width -eq 3840)]) 'Reference destination manifest is incomplete.'
        foreach ($destination in $palette.destinations) {
            Assert ([string]$destination.Area -notmatch "[`t`r`n]") 'Unexpected delimiter in a destination area.'
            $lines.Add((@('destination', $palette.Width, $destination.country_code, (Number $destination.longitude), (Number $destination.latitude), (Number $destination.Share), $destination.Rank, $destination.Count, $destination.Active, (Number $destination.receive_bytes_per_second), (Number $destination.send_bytes_per_second), ([string][bool]$destination.Estimated).ToLowerInvariant(), $destination.Area) -join "`t"))
        }
    }
}

}
New-Item -ItemType Directory -Path $EvidenceDirectory | Out-Null
$EvidenceDirectory = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
if ($VisualFamilies) { Write-VisualInputs $visual }
else { $lines | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'fixture.tsv') -Encoding utf8NoBOM }
$repo = Split-Path $PSScriptRoot -Parent
$provenance = [ordered]@{
    configuration = 'Release'; executable = $Executable
    executable_bytes = (Get-Item -LiteralPath $Executable).Length
    executable_sha256 = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash
    reference_directory = $ReferenceDirectory
    reference_fixture_sha256 = (Get-FileHash -LiteralPath (Join-Path $ReferenceDirectory $fixtureName)).Hash
    imported_fixture_sha256 = (Get-FileHash -LiteralPath (Join-Path $EvidenceDirectory $(if ($VisualFamilies) { 'visual-cases.tsv' } else { 'fixture.tsv' }))).Hash
    reference_review_sha256 = (Get-FileHash -LiteralPath (Join-Path $ReferenceDirectory 'review.json')).Hash
    world_sha256 = (Get-FileHash -LiteralPath (Join-Path $repo 'assets/world.bin')).Hash
    harness_sha256 = (Get-FileHash -LiteralPath $PSCommandPath).Hash
    rust_fixture_sha256 = (Get-FileHash -LiteralPath (Join-Path $repo 'src/map/renderer/resource_checks.rs')).Hash
    cargo_lock_sha256 = (Get-FileHash -LiteralPath (Join-Path $repo 'Cargo.lock')).Hash
    cargo_profile_sha256 = (Get-FileHash -LiteralPath (Join-Path $repo 'Cargo.toml')).Hash
    toolchain = (Get-Content -LiteralPath (Join-Path $repo 'rust-toolchain.toml') -Raw)
    platform = [Environment]::OSVersion.VersionString
    time_utc = [DateTime]::UtcNow.ToString('o'); theme = $theme; scale = $scale
    measurement_scope = if ($GpuOnly) { 'Native GPU timing only; no memory phases, resource comparison or image pass' } else { 'Baseline memory/CPU/allocation phases, then native GPU timing, then images' }
    process_scope = 'Whole Release Rust test executable, one deterministic map, harness/model/world retained when renderer dropped; no collector, MMDB, network, profile or product app startup'
    limitations = @(
        'Fixed render targets clipped by a 1280x900 native host; physical 4K presentation and physical input latency are unmeasured.',
        'Paint elapsed time includes Present waits. Thread CPU and frame-start intervals are separate; neither measures GPU execution. FILETIME CPU samples have OS accounting granularity.',
        'GPU counters measure allocations per process/adapter, not GPU execution. Retained geometry/effect allocation counters are unavailable without production instrumentation.',
        'Native D3D11 timestamps are collected in a separate pass after baseline CPU/memory and before readbacks. They measure the GPU timeline from main D2D BeginDraw through EndDraw, before Present; route/dot preparation prepass is excluded. Submission gaps and GPU scheduling/preemption can be included; this is not full-frame GPU time or GPU busy-unit utilization.',
        'GPU query batches use 30 redraws at Fit and each settled Europe zoom, then a 60-step drag. They do not time progressive zoom rebuilds. Each query is read once without flushing/waiting; pending, disjoint, invalid, incomplete, skipped and unavailable samples remain explicit gaps. Source GPU execution timing remains unavailable.',
        'Country/city strings and torrent roles are not imported into PC connection presentation. Rates/coordinates/country identities/counts match; marker roles follow production directional traffic.',
        'Long static bright hold preserves source idle marks; active NetMap directional pulses are included in native cost. Source lacks those added product pulses.',
        'No MMDB is opened, so mapped dataset residency is inapplicable here. Live collector/process totals require a separate controlled measurement.',
        'Peaks are sampled after frames and at the end of each interval, not an allocation trace. Driver/runtime retention and process allocator variation remain visible.',
        'NetMap baseline retains the world/model/test runtime. Compare repeated runs and renderer drop/recreate deltas; it is not exact map allocation isolation.'
    )
}
$provenance | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'provenance.json') -Encoding utf8NoBOM
if ($VisualFamilies) {
    $provenance.measurement_scope = 'Images-only visual families; no memory/GPU matrix, cold arms or pixel threshold'
    $provenance.process_scope = 'Production Renderer/Model/Viewport; exact source palette/view/destination/highlight import; partial source coverage retained'
    $provenance['source_policy'] = $visual.source_policy
    $provenance['native_policy'] = $visual.native_policy
    $provenance['location_policy'] = $visual.location_policy
    $provenance['animations'] = $visual.animations
    $provenance.limitations = @('No physical presentation, input latency or OS High Contrast/reduced-motion mutation', 'Source idle roles differ from native establishment marks; native readback includes production default Speeds absent from source map renderer', 'Hover/selection are supplied fixture identities; this does not validate pointer input', 'Full-turn cases import normalized static viewports; they do not prove a gesture or navigation/cache lifetime', 'No numerical pixel threshold or automatic visual-acceptance claim')
    $provenance['input_hashes'] = @(Get-ChildItem -LiteralPath $EvidenceDirectory -Filter 'visual-*.tsv' -File | ForEach-Object { [ordered]@{file = $_.Name; sha256 = (Get-FileHash -LiteralPath $_.FullName).Hash} })
    $provenance | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'provenance.json') -Encoding utf8NoBOM
}
if ($ColdArm) {
    $provenance.measurement_scope = 'Cold map contribution schema 1; one fresh arm, primary checkpoints at common-setup ages 30/40/50 seconds; no baseline matrix, GPU execution queries, images or navigation'
    $provenance.process_scope = 'Whole Release Rust test process; identical imported facts/native host in all arms; control has no device; presentation holds device/target after two blank Presents; text adds production Speeds with empty Prepared and two text Presents; world/arranged model/map renderer only in treatment; no collector or MMDB'
    $provenance['cold_arm'] = $ColdArm
    $provenance['checkpoint_seconds'] = @(30, 40, 50)
    $provenance['scene_clock_policy'] = 'Existing Scene.prepare: now = 60 seconds + elapsed since preparation; bright = 86400 seconds; fade = 0; active native directional pulses. This is the accepted fixture hold, not default product steady state.'
    $provenance['arm_content'] = switch ($ColdArm) {
        control { 'Imported facts and common host only; no device or arranged scene' }
        presentation { 'Production Device::new/attach/select/apply_appearance/begin/end; two blank Presents; live device/target retained; no world/model/renderer/routes/outlines/DirectWrite' }
        text { 'Production Device and speeds::Speeds new/prepare/draw; default Speed and Traffic unavailable overlay in two Presents; empty-world/model Map::prepared input; no map arrangement/geometry/routes/effects' }
        treatment { 'Production map through Renderer; includes world/model, map caches, directional pulses and default Speeds/DirectWrite initialization' }
    }
    if ($ColdArm -eq 'presentation') { $provenance.scene_clock_policy = 'Not applicable; no scene prepared, no map clock/highlight/text; only two blank Presents' }
    if ($ColdArm -eq 'text') { $provenance.scene_clock_policy = 'Not applicable; empty Map::prepared only supplies viewport/appearance/default Speeds; no arranged scene or highlight; two text Presents' }
    $provenance.limitations = @(
        'One fresh arm/trio does not establish independent repetition; compare selected arms using identical input/code/binary hashes.',
        'Reference includes common WinUI/CLR/engine/MMDB startup; native uses its test host. Compare within-application treatment-minus-control effects.',
        'Fixed 4K target is partially clipped; no physical 4K presentation claim. Native active directional pulses are included; source traffic behavior differs.',
        'Allocation counters are collected after each primary checkpoint with explicit errors; they are not GPU execution timings. Natural runtime/driver retention remains in totals.',
        'Actual checkpoint lateness and process/common/readiness ages are recorded; no forced GC, working-set trim, image capture or navigation precedes primary samples.',
        'Presentation/text are startup diagnostics, not lighter map implementations. Text-minus-presentation includes DirectWrite/font first use plus minimal empty Prepared ownership; it is not an allocation trace. Full-map residual retains geometry/effects/behavior differences.'
    )
    $provenance | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'provenance.json') -Encoding utf8NoBOM
}

$process = $null
$counterJob = $null
$lastSample = 0
$gpuFailed = $false
$previous = [Environment]::GetEnvironmentVariable('NETMAP_MAP_RESOURCE_DIRECTORY', 'Process')
$previousGpuOnly = [Environment]::GetEnvironmentVariable('NETMAP_MAP_RESOURCE_GPU_ONLY', 'Process')
$previousColdArm = [Environment]::GetEnvironmentVariable('NETMAP_MAP_RESOURCE_COLD_ARM', 'Process')
$previousVisual = [Environment]::GetEnvironmentVariable('NETMAP_MAP_RESOURCE_VISUAL_FAMILIES', 'Process')
try {
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_DIRECTORY', $EvidenceDirectory, 'Process')
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_GPU_ONLY', $(if ($GpuOnly) { '1' } else { '0' }), 'Process')
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_COLD_ARM', $ColdArm, 'Process')
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_VISUAL_FAMILIES', $(if ($VisualFamilies) { '1' } else { $null }), 'Process')
    $process = Start-Process -FilePath $Executable -ArgumentList @(
        '--ignored', '--exact', 'map::renderer::resource_checks::native_matched_map_resources', '--nocapture', '--test-threads=1'
    ) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $EvidenceDirectory 'stdout.txt') -RedirectStandardError (Join-Path $EvidenceDirectory 'stderr.txt')
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_DIRECTORY', $previous, 'Process')
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_GPU_ONLY', $previousGpuOnly, 'Process')
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_COLD_ARM', $previousColdArm, 'Process')
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_VISUAL_FAMILIES', $previousVisual, 'Process')
    $deadline = [DateTime]::UtcNow.AddSeconds($(if ($ColdArm) { [math]::Min($TimeoutSeconds, 120) } elseif ($VisualFamilies) { [math]::Min($TimeoutSeconds, 600) } else { $TimeoutSeconds }))
    while (-not $process.WaitForExit(50)) {
        Assert ([DateTime]::UtcNow -lt $deadline) 'Native map resource journey exceeded its total timeout.'
        $path = Join-Path $EvidenceDirectory 'baseline-phase.json'
        if (-not (Test-Path -LiteralPath $path)) { continue }
        $stream = [IO.FileStream]::new($path, [IO.FileMode]::Open, [IO.FileAccess]::Read, ([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
        $reader = [IO.StreamReader]::new($stream)
        try { $phase = $reader.ReadToEnd() | ConvertFrom-Json }
        finally { $reader.Dispose() }
        if ($phase.sample_id -le $lastSample) { continue }
        Assert ($phase.process_id -eq $process.Id -and $phase.sample_id -eq $lastSample + 1) 'GPU checkpoint is not the next phase of the owned test process.'
        # Counter collection is bounded and happens after the target's CPU
        # sampling interval. Only this owned job is stopped on failure.
        $counterJob = Start-Job -ArgumentList $process.Id -ScriptBlock {
            param($ownedId)
            $ErrorActionPreference = 'Stop'
            try {
                $sample = Get-Counter -Counter '\GPU Process Memory(*)\Dedicated Usage', '\GPU Process Memory(*)\Shared Usage'
                $rows = @($sample.CounterSamples | Where-Object InstanceName -match "^pid_${ownedId}_" | ForEach-Object {
                    [pscustomobject]@{ instance = $_.InstanceName; path = $_.Path; bytes = $_.CookedValue; status = $_.Status; time = $_.Timestamp.ToUniversalTime().ToString('o') }
                })
                $errorText = if (-not $rows.Count) { 'No GPU process-memory instances matched the owned PID.' }
                elseif ($rows | Where-Object status -ne 0) { 'GPU process-memory counters have invalid status.' }
                else { $null }
                [pscustomobject]@{ counters = $rows; error = $errorText }
            } catch { [pscustomobject]@{ counters = @(); error = $_.Exception.Message } }
        }
        $waitSeconds = [math]::Max(1, [math]::Min(8, [int]($deadline - [DateTime]::UtcNow).TotalSeconds))
        if (Wait-Job -Job $counterJob -Timeout $waitSeconds) {
            $gpu = Receive-Job -Job $counterJob
        } else {
            Stop-Job -Job $counterJob
            $gpu = [pscustomobject]@{ counters = @(); error = 'GPU counter collection timed out.' }
        }
        Remove-Job -Job $counterJob -Force
        $counterJob = $null
        $gpuFailed = $gpuFailed -or $null -ne $gpu.error
        [ordered]@{
            sample_id = $phase.sample_id; time_utc = [DateTime]::UtcNow.ToString('o'); process_id = $process.Id
            scene = $phase.scene; cycle = $phase.cycle; visible = $phase.visible
            target_width = $phase.target_width; target_height = $phase.target_height
            cold_arm = $ColdArm
            counters = @($gpu.counters); error = $gpu.error
            scope = 'Per owned process/adapter GPU memory; not GPU execution or physical presentation'
        } | ConvertTo-Json -Depth 6 -Compress | Add-Content -LiteralPath (Join-Path $EvidenceDirectory 'baseline-gpu.jsonl') -Encoding utf8NoBOM
        $lastSample = [int]$phase.sample_id
        $lastSample | Set-Content -LiteralPath (Join-Path $EvidenceDirectory "baseline-phase-$lastSample.continue") -Encoding utf8NoBOM
    }
    Assert ($process.ExitCode -eq 0) "Native map resource test exited with $($process.ExitCode). See stderr.txt."
    Assert (Test-Path -LiteralPath (Join-Path $EvidenceDirectory 'complete.txt')) 'The requested native measurement passes did not finish.'
    $expectedSamples = if ($VisualFamilies) { 0 } elseif ($ColdArm) { 3 } elseif ($GpuOnly) { 0 } else { $fixtures.scenes.Count * 12 }
    Assert ($lastSample -eq $expectedSamples) 'The NetMap memory/GPU checkpoint matrix differs from the requested scope.'
    $records = @(Get-Content -LiteralPath (Join-Path $EvidenceDirectory 'measurements.jsonl') | ForEach-Object { $_ | ConvertFrom-Json })
    if ($VisualFamilies) {
        $images = @($records | Where-Object journey -eq 'visual-image')
        Assert ($images.Count -eq @($visual.cases).Count) 'Native visual image set is incomplete.'
        $results = foreach ($image in $images) {
            $path = Join-Path $EvidenceDirectory $image.file
            $bytes = [IO.File]::ReadAllBytes($path)
            Assert ([BitConverter]::ToInt32($bytes,18) -eq $image.width -and -[BitConverter]::ToInt32($bytes,22) -eq $image.height) 'Native image extent differs.'
            [ordered]@{native = $image; native_sha256 = (Get-FileHash -LiteralPath $path).Hash; source = @($visual.cases | Where-Object id -eq $image.id)[0]}
        }
        [ordered]@{schema = 1; images = @($results); source_failures = @($visual.failures); coverage_complete = (@($visual.failures).Count -eq 0); scope = $provenance.measurement_scope} |
            ConvertTo-Json -Depth 30 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'visual-images.json') -Encoding utf8NoBOM
        Write-Output "Completed $($images.Count) native visual counterparts; source gaps: $(@($visual.failures).Count). Evidence: $EvidenceDirectory"
        return
    }
    if ($ColdArm) {
        $samples = @($records | Where-Object journey -eq 'map-contribution')
        Assert ($samples.Count -eq 3 -and ($samples.checkpoint_seconds -join ',') -eq '30,40,50') 'The cold checkpoints are incomplete.'
        Assert (@($samples | Where-Object { $_.schema -ne 1 -or $_.arm -ne $ColdArm -or $_.target_width -ne 3840 -or $_.target_height -ne 2160 }).Count -eq 0) 'Cold sample provenance differs.'
        Assert (@($samples | Where-Object { $_.ready -ne ($ColdArm -ne 'control') }).Count -eq 0) 'The cold arm readiness differs.'
        if ($ColdArm -eq 'presentation') {
            Assert (@($samples | Where-Object { $null -ne $_.destinations -or $null -ne $_.scene -or $_.scene_arranged -or $_.blank_presents -ne 2 -or -not $_.graphics_mode }).Count -eq 0) 'Blank presentation metadata differs.'
        }
        if ($ColdArm -eq 'text') {
            Assert (@($samples | Where-Object { $null -ne $_.destinations -or $null -ne $_.scene -or $_.scene_arranged -or $_.blank_presents -ne 0 -or $_.text_presents -ne 2 -or ($_.text_content -join '|') -ne 'Speed|Traffic unavailable' -or -not $_.graphics_mode }).Count -eq 0) 'Production text metadata differs.'
        }
        [ordered]@{ schema = 1; arm = $ColdArm; samples = $samples; gpu_allocation_gaps = $gpuFailed; scope = $provenance.measurement_scope } |
            ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'cold-contribution.json') -Encoding utf8NoBOM
        Write-Output "Completed cold $ColdArm arm: 3 primary checkpoints. GPU allocation gaps: $gpuFailed. Evidence: $EvidenceDirectory"
        return
    }
    $samples = @($records | Where-Object journey -eq 'netmap-map-baseline')
    Assert ($samples.Count -eq $lastSample) 'The measurement file is incomplete.'
    $gpuTimings = @($records | Where-Object journey -eq 'gpu-main-draw')
    Assert ($gpuTimings.Count -eq $fixtures.scenes.Count * 2 * 9) 'The native GPU timing batch matrix is incomplete.'
    $gpuTimingGaps = @($gpuTimings | Where-Object {
        $null -ne $_.unavailable -or $null -eq $_.gpu_main_draw_ms -or $_.pending -gt 0 -or $_.disjoint -gt 0 -or
        $_.invalid -gt 0 -or $_.incomplete -gt 0 -or $_.skipped -gt 0 -or $_.error_hresult.Count -gt 0
    }).Count
    [ordered]@{
        batches = $gpuTimings.Count; batches_with_gaps = $gpuTimingGaps
        scope = 'Native main D2D draw GPU timeline elapsed, excluding route/dot prepass and Present; not full-frame GPU time'
        source_comparison = 'Reference GPU execution timing is unavailable'
        results = $gpuTimings
    } | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'gpu-main-draw.json') -Encoding utf8NoBOM
    if (-not $GpuOnly) {
        $comparisons = foreach ($scene in $fixtures.scenes) {
            foreach ($width in 1920, 3840) {
                foreach ($visible in $false, $true) {
                    $measured = @($samples | Where-Object { $_.scene -eq $scene.Name -and $_.target_width -eq $width -and $_.visible -eq $visible })
                    $source = @($referenceSamples | Where-Object { $_.scene -eq $scene.Name -and $_.target_width -eq $width -and $_.visible -eq $visible })
                    [pscustomobject]@{
                        scene = $scene.Name; width = $width; visible = $visible
                        netmap_private_bytes = @($measured.private_bytes); reference_private_bytes = @($source.private_bytes)
                        netmap_working_set = @($measured.working_set); reference_working_set = @($source.working_set)
                        netmap_handles = @($measured.handles); netmap_gdi = @($measured.gdi_objects); netmap_user = @($measured.user_objects)
                        note = 'Raw three-cycle samples; repeat warmed process runs to assess variation and growth. No automatic memory-acceptance claim.'
                    }
                }
            }
        }
        $comparisons | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'comparison.json') -Encoding utf8NoBOM
        Assert (-not ($gpuFailed -or $referenceGpuFailed)) 'One or more NetMap/reference GPU checkpoints are unavailable/invalid. Measurements are retained; GPU memory acceptance remains incomplete.'
    }
    if ($GpuOnly) {
        Write-Output "Completed native GPU timing only; memory/resource comparison was not run. $gpuTimingGaps GPU timing batches have explicit gaps. Evidence: $EvidenceDirectory"
    } else {
        Write-Output "Completed $lastSample matched memory phases; $gpuTimingGaps GPU timing batches have explicit gaps. Evidence: $EvidenceDirectory"
    }
} finally {
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_DIRECTORY', $previous, 'Process')
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_GPU_ONLY', $previousGpuOnly, 'Process')
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_COLD_ARM', $previousColdArm, 'Process')
    [Environment]::SetEnvironmentVariable('NETMAP_MAP_RESOURCE_VISUAL_FAMILIES', $previousVisual, 'Process')
    if ($counterJob) { Stop-Job -Job $counterJob; Remove-Job -Job $counterJob -Force }
    if ($process) {
        if (-not $process.HasExited) {
            $process.Kill()
            $null = $process.WaitForExit(3000)
        }
        $process.Dispose()
    }
}
