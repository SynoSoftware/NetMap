# Matched map measurements

## Resumed native allocation attribution (2026-10-24)

User-authorized UAC capture keeps the target ordinary. Preserved optimized test
5680939D runs the existing active100/4K imported fixture; no source case is retried
and no forced GC or working-set trim is used. These are diagnostic traces,
not uninstrumented acceptance measurements.

- `V:/temp/netmap-elevated-acceptance-20261024-a/`: initial blank-device trace,
  345,976 lost events; partial `-tle` decoding is diagnostic only.
- Run b: exported file-mode buffers increase from twenty to eighty per collector;
  blank Device/Presents trace has zero lost events/buffers. Symbolized native
  stacks establish driver/device initialization activity. The trace's outstanding
  allocation sum is not reconciled with process Private Bytes.
- Run c: UAC canceled before recording/target; preserved as failed setup. The user
  explicitly authorizes retry d, whose full-map trace loses 37,606 events and
  cannot establish complete allocation totals.
- Run e: same full-map target and eighty system buffers; custom native-only
  profile preserves the exported system keywords/stacks while omitting ancillary
  managed/JScript event collectors. Trace has zero lost events/buffers; strict
  symbolized Xperf decoding succeeds without ignoring loss. Target PID 24892
  starts at 10:08:30.384 UTC; trace ends at 10:08:38.467 UTC and the target
  completes its existing fifty-second diagnostic normally. Seven-second process
  Private Bytes is 196,661,248, an instrumented observation, not an acceptance
  baseline. Global tracked outstanding commitment is 171,868 KiB at trace end.

In e, a 34,496-KiB retained stack passes through D2D PushLayer/CreateBitmaps and
native texture/graphics backing allocation; another 19,616-KiB stack is image/
effect tile rendering. Gradient-stop realization also appears in the retained
stacks. Kernel graphics frames are provenance, not proof of a kernel-pool or
exclusive process-private charge. Reserved, cumulative committed, outstanding
committed and process Private Bytes must not be equated. The profile and trace
endpoints differ between b/e, so selected sums are not a matched subtraction.
See `V:/temp/netmap-integrated-review/current-allocation-attribution.md` for
bounded interpretation and unresolved attribution.

The retained allocation changes are now measured on preserved test 5B6F4ADF,
using the unchanged active-100/4K cold fixture and three 30/40/50-second primary
checkpoints per process. `V:/temp/netmap-renderer-allocation-20261024-a/` measures
158.846 MiB mean Private Bytes inside the command sandbox; run b on the ordinary
desktop measures 161.908 MiB, with 90.277 MiB working set. The separate ordinary
desktop never-map control is 2.754 MiB private and 17.283 MiB working set. Thus b
minus that independent control estimates 159.154 MiB map contribution, versus
the prior native 183.67–184.39 MiB and reference 144.05–144.92 MiB. This is a
useful reduction, but remains about 14–15 MiB above the reference contribution.
These runs are not newly repeated matched pairs; environment differences between
a/b and the single shared control limit the comparison. Treatment GPU allocation
queries succeed; b averages 128.137 MiB dedicated and 19.465 MiB shared allocation.
The control's absent GPU counter instances remain gaps. No
collector or MMDB is included in these deterministic map fixtures. The
lower-map-memory criterion remains unmet.

The next bounded source check finds no equivalent sharing for core gradients:
their stop positions and alpha depend on each guide's scaled length, unlike the
two identical halo collections. Sharing only by upload/download color would
change the source appearance. No speculative cache, approximation or renderer
rewrite is introduced while the remaining native measurements are deferred.
The existing gradient collection already requests 8-bit UNORM, so there is no
higher-precision gradient buffer to remove. No precision change is justified.

Changed-renderer interaction checks in `V:/temp/netmap-dynamics-20261024-h/`
pass Navigation (17.33 seconds) and Temporal on the same preserved test. Root
inspected six full-target images: dense-before-pan, turn-12, selected resize,
activity-first, fade-resumed and supplied High Contrast. Their routes, glow and
fades show no observed clipping. The earlier run g inside the command sandbox
hits its dispatch bound before recording phases; retain it as failed rather
than changing that bound. The user requests minimal screen disruption: remaining
native input/hosting checks are batched for an announced attended session.

## Native dynamic navigation and temporal presentation (2026-10-24)

`V:/temp/netmap-dynamics-20261024-f/result.json` passes both exact native cases
on preserved optimized test
`7142D77AD777443D3F597ADEC8CBFFEEDB6BEF9378F838FA9224CF343D92D5F9`.
Navigation takes 13.94 seconds; temporal presentation takes 36.77 seconds.
Root inspected all 22 full-target BMPs: twelve navigation and ten temporal
images, at actual 96 DPI, mostly 1264×713 with the resized target 1044×813.

The navigation owner handles 24 captured moves through a complete longitude
turn, with a fresh successful Present of the current model/revision at every
step. The final centre is -0.01709 degrees from its starting longitude, within
the fixture's native-coordinate quantization bound. Four fast moves at 35.53x
cross the retained fill window while membership changes; unknown/manual origin
rebasing cancels capture and late completions cannot restore the old grip.
The selected Canada group survives an actual Host resize and retains geographic
guide identities. The images show consistent outlines/fills and view changes;
the tightly clustered destinations overlap near the edge of the revealed view.
This is not a dense 4K or selected-first rebuild performance matrix.

For default bright10/fade20, Present count remains 31 from age 101.755 through
the quiet hold. The fade has a 33 ms wake at age 110.401; hidden time adds no
Presents, and resume captures the current age 121.035 with visibly dimmer marks.
Age 130.504 is quiet Idle with the establishment marks expired. Send-only,
receive-only and bidirectional facts submit their measured pulse directions;
zero/unavailable rates submit none. Pulse positions advance while live guide,
stroke and glow identities remain retained. Supplied reduced motion and supplied
black/white contrast suppress pulses and schedule only their later transition;
contrast removes glow, retains guides and produces black-background/white-ink
pixels. Bright3/fade0 becomes Idle after its actual boundary. Selection/hover
can still affect the observed native frame; these are supplied facts and policy,
not live ETW or actual Windows appearance-setting delivery.

The first run e passes its transition assertions but its post-Present image
readback samples the rotated buffer. Its captures are invalid as current-frame
evidence; `capture-validation.json` preserves that failure without rewriting the
original result. The corrected cfg(test) path copies after successful EndDraw
and before flip Present, then correlates the image with successful current-frame
metadata. A native High Contrast background assertion guards against recurrence.
This changes capture instrumentation, not product rendering. Focused review and
correction deltas are clean in
`V:/temp/netmap-integrated-review/map-dynamics-review.md`.

There is no new reference capture or unchanged failed source retry. These checks
do not establish matched-source parity, physical mouse/touchpad or mixed-DPI
delivery, allocation totals, GPU execution or lower memory use.

## Owned navigation input timing (2026-10-24)

`V:/temp/netmap-input-timing-20261024-a/result.json` passes Navigation in
14.60 seconds on preserved optimized test
`5680939D1C26C4FBF94AF6CEBCAC18A85D68F6DABBD011C811CBE2F4E943DFEF`.
The executable and PDB are retained in
`V:/temp/netmap-input-timing-build-20261024-a/`; PDB SHA-256 is
`87C52FA2EBE913736CD46C815C19E80D59F7CF2AB60A578B90C7C0A5D1424E81`.

The existing 24 complete-turn moves now retain monotonic input-start and
successful Present-return timestamps, elapsed microseconds and accepted revision
in `Navigation/input-latency.tsv`. Each interval starts without image readback
armed and must end with a fresh successful Present of the current model/revision.
The timestamp is recorded immediately after the actual Present API returns;
the earlier preparation age is not used as a return timestamp.

Median is 33.017 ms, nearest-rank p95 49.747 ms and worst 66.418 ms. These measure
the existing owned synchronous dispatch, message pump and explicit repaint
through their last matching Present return. Multiple Presents can fall within
one interval: this is not first-response latency. Physical input, OS input-queue,
compositor and display latency are excluded. No matched reference or general
performance threshold is established by these controlled native samples.

Formatting, all-targets Release Clippy and the 76-second optimized build pass;
the existing Navigation assertions pass and the owned process exits 0. Focused
instrumentation review is clean in
`V:/temp/netmap-integrated-review/input-timing-review.md`. The change is test-only;
release/package A60AF2EB is unchanged. Temporal and failed source matrices are
not rerun; the earlier inspected frame evidence retains its own binary scope.

## Scoped ordinary-product component accounting (2026-10-24)

All 19 phases complete in
`V:/temp/netmap-product-resources-20261024-g/{result,summary}.json` on final
release `A60AF2EB81D437EBBDB42729E6BE5F0D337DAA07F512C42F0C9C10F94CC76C19`.
Two fresh manual-origin profiles use actual changing restricted connection
facts: 113–157 parent rows during sampling. Each phase has three process samples
over approximately one second after a one-second settle. Values below are mean
process private / working-set MiB, not isolated component allocations.

| Profile / phase | Private / working set MiB |
| --- | ---: |
| Country-only visible map | 100.28 / 78.52 |
| Country-only All connections | 103.65 / 81.90 |
| First Settings Map page | 109.34 / 87.29 |
| All lazy pages created, Screen saver selected | 109.19 / 86.69 |
| Settings closed / after three warm reopens | 109.19 / 86.83 → 109.24 / 87.01 |
| City-data visible map / All connections | 102.18 / 80.71 → 105.84 / 84.11 |
| City Location Settings | 111.05 / 88.47 |
| Parent after Preview cycles 1 / 2 / 3 | 111.87 / 93.86 → 112.17 / 94.17 → 112.27 / 94.46 |
| Parent after final Settings close | 112.09 / 94.45 |
| Separate Preview processes 1 / 2 / 3 | 156.47 / 112.30 → 155.53 / 109.87 → 154.54 / 109.59 |

Each actual Settings Preview launches the private byte-identical SCR with two
display surfaces, then exits 0 on owned close. Both parents exit 0; all five
held process instances are observed exited without a kill. Preferences and the
retained city copy remain unchanged. CPU windows and handle/GDI/USER samples
are retained separately. Parent handles after the three previews are 574/575/574;
the first-launch increase is not attributed. Country Settings GDI rises from
26 after its first close to 36 after three warm reopens, while private memory
changes by 0.05 MiB. This diagnostic does not establish settled leak freedom.

All 19 GPU allocation queries return valid counters, sampled after CPU windows:
parent map dedicated/shared allocations are approximately 26.4/14.1 MiB
(country) and 26.7/15.9 MiB (city); Preview children are approximately
104–108/34–35 MiB. These are allocations, not GPU execution or exclusive memory.
All mapped-residency queries report unresolved MEM_MAPPED filename error 1006.
Their partial named-MMDB records are invalid as totals; residency remains
unavailable, rather than zero or the 126,998,165-byte file length.

The initial f attempt fails before table visibility because the hidden startup
keeps the parent hidden; its partial measurement is not a visible-map baseline.
The corrected wrapper requests a nonactivating show for the exact owned HWND
and verifies parent visibility before any sample. Source review and narrow
corrections are clean in
`V:/temp/netmap-integrated-review/product-resources-review.md`.
Live facts, layout/cache warming and separate processes differ between phases;
their differences cannot be assigned solely to list, Settings or MMDB ownership.
No renderer/reference sweep or elevated collector measurement is repeated.
The separate approximately 184 versus 144–145 MiB map-contribution objective
remains unmet and still requires the deferred allocation investigation.

## Named MMDB residency follow-up (2026-10-24)

`V:/temp/netmap-mmdb-residency-20261024-h/{result,mapping}.json` passes one
read-only query on unchanged release A60AF2EB. A fresh visible ordinary-token
city profile reaches restricted TCP/manual-origin/local-city readiness with
132 observed connections. Both expected private copies match the loaded device
paths and recorded source hashes. The parent exits 0 without a kill; preferences
and the retained city dataset remain unchanged.

| Exact mapped dataset | Virtual bytes | Resident bytes | Shareable resident bytes |
| --- | ---: | ---: | ---: |
| Country | 8,351,744 | 466,944 | 466,944 |
| City | 127,000,576 | 966,656 | 966,656 |

The city file is 126,998,165 bytes; its file length is not residency. These are
point-in-time page observations after a one-second settle, not private commitment
or exclusive memory. Shareable means the page can be shared, not that another
process currently shares it. See Microsoft's
[working-set page attributes](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-psapi_working_set_ex_block).

The prior wrapper demanded a filename for every MEM_MAPPED region and reported
error 1006. Windows includes pagefile and physical mappings in that category.
The wrapper now uses documented
[allocation classification](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/ns-memoryapi-win32_memory_region_information)
through [QueryVirtualMemoryInformation](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-queryvirtualmemoryinformation)
before requesting filenames. This run traverses 895 regions, classifies 20 file
and 28 pagefile regions, and reports zero unknown, unresolved or truncated views.
Any classification, filename or page-query error still invalidates the result.

The exact wrapper hash is
`3C3D7BA2DFBC5D3347F253F9FEE14D2D84D1A4857D80372DC1129DCD7B53AB61`;
focused review is clean in
`V:/temp/netmap-integrated-review/resource-followup.md`. This is a new scoped
observation; it does not repair the earlier nineteen invalid query totals,
attribute the map's private-memory excess, or repeat list/Settings/Preview/GPU
sampling. The lower-map-memory objective remains unmet.

## Current shared-renderer rerun (2026-10-24)

`V:/temp/netmap-shared-renderer-resources-20261024/` completes the same Dark/100
reference journey after the Stage 2 resource-sharing correction: 36 memory/GPU
allocation phases, 54 main-draw GPU timing batches with no explicit gaps/errors,
and six BMPs. Native test session `49903` exits zero after 304.31 seconds. Test
executable SHA-256 is
`DC2966CA31796132793C7E52F98FFC0309C03F556BE28635839748065EDE28EA`;
this is the optimized test binary, distinct from the product/package executable.
The completed reference capture below was reused; failed larger source runs
were not repeated. Detailed analysis is retained in
`V:/temp/netmap-integrated-review/current-resources.md`.

Mean visible private / working set in MiB over three lifetime cycles:

| Scene | Current native | Previous native | Reference UI |
| --- | ---: | ---: | ---: |
| 1080 idle | 123.22 / 104.37 | 123.82 / 98.55 | 234.70 / 286.75 |
| 1080 active | 171.77 / 128.07 | 154.35 / 112.88 | 311.24 / 310.07 |
| 1080 mixed | 169.83 / 121.53 | 150.14 / 108.76 | 326.06 / 322.49 |
| 4K idle | 188.46 / 148.85 | 179.81 / 139.51 | 367.20 / 363.26 |
| 4K active | 242.65 / 175.24 | 207.64 / 141.33 | 433.18 / 382.82 |
| 4K mixed | 240.32 / 177.13 | 213.55 / 143.03 | 445.66 / 387.70 |

Current totals remain below the reference UI, whose separate engine adds cost,
but five private means and all working-set means exceed the previous native run.
The largest increases are 35.01 MiB private and 34.10 MiB working set. Larger
post-navigation cycle-1 peaks mostly release by cycle 2: visible private changes
from cycle 0 to 2 range from -1.39 to +5.19 MiB. These single process runs do not
isolate map cost, prove repeatable savings or establish a cause for the variation.
Do not describe the rerun as an across-the-board memory reduction.

Every detach checkpoint returns dedicated/shared GPU allocation to zero. GDI
remains 30, attachment adds five USER objects and about 131 handles, and counts
stay stable within scene cycles. Absolute counts already exceed the earlier
binary at the first hidden checkpoint; that offset remains unexplained.
Settled paint/Present p95 is lower in all six current cases, while the 4K mixed
64x zoom has a higher single-run total/tail. Main-draw GPU timestamps exclude
the route/dot prepass and Present; they are not full-frame GPU parity. Idle
samples schedule no redraws, while active/mixed samples schedule pulse frames.

Both idle BMPs exactly match the earlier renderer's bytes. Visual inspection of
the 1080 active pair retains geometry/effects with changed pulse positions;
this does not complete the full visual matrix. Whole-product live-collector/MMDB
cost, independent warmed repetitions, larger/Light comparisons, physical input
and mixed-DPI acceptance remain open. Earlier sections below describe retained
historical measurements, not the current binary.

### Attribution audit

The later read-only audit at
`V:/temp/netmap-integrated-review/map-contribution.md` identifies a concrete
shortfall in the current comparison. Root independently recomputed all 18 paired
visible-minus-hidden private deltas from the raw JSON: each native delta exceeds
its reference counterpart. Final-cycle deltas in MiB are:

| Scene | Current native | Reference UI |
| --- | ---: | ---: |
| 1080 idle | 50.11 | 5.52 |
| 1080 active | 47.20 | 10.45 |
| 1080 mixed | 52.93 | 5.98 |
| 4K idle | 55.14 | 8.22 |
| 4K active | 55.61 | 7.35 |
| 4K mixed | 61.38 | 14.59 |

These measure reactivation in already exposed processes, with different retained
geography/model lifetimes; they are not whole-map allocation estimates. Both
initial hidden samples follow rendering warmup, source hidden GPU allocation
remains substantial, and GC/allocator reclamation changes private commit between
phases. Source PeerMap reloads geography/rearranges on attachment while the
native fixture retains its prepared world/model. The lower native process totals
and larger native reactivation deltas are both observations; neither isolates
the map contribution.

The smallest justified changed comparison uses fresh never-rendered control and
map-loaded treatment processes for the existing active-100/4K fitted case, with
raw exported facts common and the same timestamped sampling policy. World/model
preparation belongs to treatment for the whole-map question. It must branch
before source map warmup and seeding, rather than rerun the current matrix or
subtract retained costs. This comparison and independent repetitions remain
unperformed. Removing discarded Device constructor proof objects in the next
code batch eliminates redundant startup calls; no measured saving or explanation
of the historical count offset is inferred from that edit.

### Fresh never-map comparison (2026-10-24)

The changed comparison is now implemented and run. Evidence and exact source
capture paths are indexed by `initial-runs.jsonl`, `remaining-runs.jsonl` and
`failed-run.json` in `V:/temp/netmap-cold-contribution-20261024-a/`.
`paired-effects.json` contains the independently recomputed paired results.
Every arm is a fresh process with the same accepted active-100/4K facts and a
common host. Controls never decode the world or create a map/renderer. Primary
samples occur at common-setup ages 30/40/50 seconds; no forced GC, trimming,
navigation, readback or GPU execution queries precede them. Pair 2 reverses arm
order. Actual ages/lateness and natural source GC counters remain in the raw data.

Native uses preserved optimized test SHA-256
`5306F59B6DAC26F6EE30597E42CA332894A5411CDD5CAC7FA372E3B72A9B010A`;
the disposable reference UI DLL is
`2F4FBE6C8FB65F7706A0EA942F0FB919413AB1E4C4A65D3931BE9082E9ADC5FB`,
with unchanged engine `90A7A947…`. Current/prechange reference binaries, compiler
logs and input/code provenance are preserved. Product/package D779778C is
unchanged. The focused fixture review is clean at
`V:/temp/netmap-integrated-review/cold-contribution-review.md`.

Mean private bytes over the three checkpoints, and treatment minus its own
independent control, in MiB:

| Application / pair | Control private | Map private | Private increment | Working-set increment |
| --- | ---: | ---: | ---: | ---: |
| Native 1 | 2.23 | 186.06 | 183.83 | 77.82 |
| Native 2 | 2.23 | 185.90 | 183.67 | 74.93 |
| Native 3 | 2.22 | 186.61 | 184.39 | 75.14 |
| Reference 1 | 126.72 | 270.77 | 144.05 | 74.99 |
| Reference 2 | 129.09 | 274.00 | 144.92 | 75.81 |

Both valid reference treatments settle in 1.80–2.15 seconds; native treatments
settle in 1.99–2.01 seconds. Maximum checkpoint lateness is 35.67 ms. Third
reference treatment `Capture-peers-8c6c7abe…` fails the unchanged 20-second
readiness predicate and supplies no primary samples. Its failure/store/provenance
remain intact; its successful control is not paired with another run's treatment.
There are eleven completed arms / 33 primary checkpoints, rather than three
completed pairs per application. Do not repeat that unchanged failure or extend
the deadline to turn it into a pass. All owned processes have exited.

GPU allocations are separate from private bytes and working set. Native map
treatments average 152.25 MiB dedicated and 19.61–19.98 MiB shared, with valid
counters. Native never-map controls have no matching process GPU counter
instances; their records are explicit gaps, not manufactured zeros. Corrected
reference pair 2 averages 44.28 / 3.06 MiB dedicated/shared in control and
378.25 / 32.50 MiB in treatment. The source wrapper originally received nested
counter samples as strings across Start-Job serialization; pair 1's GPU records
are preserved as gaps. Filtering/projecting scalar rows inside the bounded job
fixes subsequent collection without changing C# or rebuilding the UI. Do not
infer reference pair 1 GPU values from pair 2, or sum overlapping memory categories.

**The private-memory reduction objective remains unmet.** Native whole-process
totals are lower in this fixture, but its measured private increment exceeds
both completed reference increments. The allocation-path investigation at
`V:/temp/netmap-integrated-review/cold-allocation-paths.md` finds no demonstrated
retained CPU/world/model duplication explaining the roughly 40 MiB difference.
Requested flip buffers, cached effect rectangles and geometry payload sizes do
not establish private-commit attribution.

Scope remains important: each application has its own runtime/common UI;
reference common startup already uses WinUI text/MMDB. Native initializes
DirectWrite and draws its speed readout; standalone source PeerMap omits the
surrounding speed readout. Native retains the declared 60-second scene clock,
86,400-second establishment hold and directional pulses, rather than product
defaults or source torrent-role animation semantics. Logical 4K targets are
clipped by their hosts; physical source backing/atlas ownership is unmeasured.
This is an incremental whole-map fixture comparison, not identical-overlay or
algorithm-only attribution, full-product/collector totals, or complete resource
acceptance. A small blank native device/presentation control is the next proposed
discriminating measurement; it has not been implemented or run. Removing required
text, effects or buffering is not justified by these measurements.

### Device and presentation diagnostic (2026-10-24)

`V:/temp/netmap-discriminating-20261024-a/` preserves one fresh native
control/presentation/treatment trio, `arm-summary.json`, all primary/counter
records, build logs and optimized test binary SHA-256
`727C2E3E60AFA9748D0B02C9B1154A4582DCADFFA94D9C6775558173330077CA`.
The parent and shutdown wrappers confirm an ordinary token. Formatting,
all-targets Release Clippy and the ordinary suite pass (106 passed, 15 ignored).
The two-file focused review is clean at
`V:/temp/netmap-integrated-review/presentation-control-review.md`.

All arms retain the same accepted imported active-100 facts and common windows.
Presentation creates the actual production Device/4K target and retains it
after two blank Presents; it never decodes World, prepares a scene or initializes
routes, outlines or DirectWrite. Treatment uses the production renderer and
existing unmeasured draw scheduler. Primary samples remain at common-setup ages
30/40/50 seconds, without preceding navigation, readback, query batches or trims.

| Arm | Mean private MiB | Mean working set MiB | Dedicated GPU MiB | Shared GPU MiB |
| --- | ---: | ---: | ---: | ---: |
| Common control | 2.21 | 13.78 | unavailable | unavailable |
| Blank production presentation | 56.76 | 37.07 | 74.21 | 0.86 |
| Complete map treatment | 186.37 | 89.09 | 152.25 | 19.91 |

Presentation minus control is 54.55 MiB private and 23.29 MiB working set.
Treatment minus presentation is 129.62 MiB private and 52.02 MiB working set.
The complete treatment minus common control is 184.17 MiB private, consistent
with the previous native pairs. Presentation and treatment are ready at 264 ms
and 1,985 ms; worst checkpoint lateness is 36.78 ms. GPU values are unchanged
across their three valid samples. All three control counter gaps remain explicit,
not zero allocations. Private, working-set and GPU categories must not be added.

This separates architectural first use under the recorded conditions; it does
not identify individual allocation owners. The map residual still combines
World/model, geometry, effects, text and active frame behavior, including native
directional pulses. The prior reference includes common WinUI/CLR/text/device
startup in its own control. Subtracting this new blank presentation from native
while retaining the old reference control is not a newly matched comparison.
One trio is not independent repetition, and this fixture remains partially
clipped rather than physically presenting 4K. The private-memory acceptance
shortfall remains open; no production renderer or reference source was changed.

### Production text startup diagnostic (2026-10-24)

`V:/temp/netmap-acceptance-20261024-b/text-summary.json` records a fresh paired
blank-presentation/text measurement from preserved optimized test SHA-256
`F47D775F6CC6A00F1CC12A6ADF88FCCB0CD59A1DC52516F640D89A20AEA85AB8`.
Both arms use the accepted imported facts, same production Device/4K target,
two Presents and the same 30/40/50-second checkpoint policy. Text obtains an
empty Prepared through Map and invokes production Speeds initialization,
layout and drawing of the default Speed/Traffic unavailable overlay. It loads
no World and arranges/draws no map geometry, guides or effects. The explicit
arm retains its Device, text resources and empty Prepared through sampling.

| Arm | Mean private MiB | Mean working set MiB | Dedicated GPU MiB | Shared GPU MiB |
| --- | ---: | ---: | ---: | ---: |
| Blank presentation | 56.52 | 37.03 | 74.21 | 0.85 |
| Production default text | 60.51 | 42.13 | 74.79 | 1.50 |

The startup difference is 3.99 MiB private, 5.10 MiB working set, 0.57 MiB
dedicated GPU and 0.64 MiB shared GPU. These categories remain separate. Both
sets have three valid allocation samples with no gaps; ready times are 280 ms
and 313 ms, and worst checkpoint lateness is 10.38 ms. Raw records, provenance,
code/input hashes and build logs remain beside the summary. The focused source
review is clean at `V:/temp/netmap-integrated-review/text-control-review.md`.

This isolates an architectural first-use difference including font/runtime and
minimal empty Prepared ownership, not precise allocation ownership or every
traffic-text scenario. It is one pair, and the target is still partially clipped.
It does not explain the earlier approximately 40 MiB native/reference private
increment difference. Geometry/effects/runtime retention remain combined in
the map residual; no product text/renderer rewrite or acceptance claim follows.

### Next allocation-attribution step

The bounded tooling investigation is recorded at
`V:/temp/netmap-integrated-review/memory-attribution-next-step.md`. Root confirms
that installed `wpr -profiles` lists VirtualAllocation; WPA is installed, and
the installed GPUView capture script explicitly requires administrator mode.
Only profile listing was run; no tracing session or profiler target was started.
Microsoft's [VirtualAllocation workflow](https://learn.microsoft.com/en-us/windows/apps/develop/performance/disk-memory)
requires elevated recording and analyzes Process / Commit Type / Commit Stack /
Size in WPA. This is the next discriminating investigation of the 54.55 MiB
blank-device first-use cost and approximately 40 MiB incremental shortfall.
It is deferred with UAC work, rather than repeating unchanged memory sweeps.

Capture before first device creation, retaining the ordinary-token target and
its exact fixture. Inspect retained commit by allocating stack/module; runtime
or driver symbols may remain incomplete, and the trace does not replace separate
GPU accounting. The initial VirtualAllocation profile needs no IFEO heap setting.
The PDB emitted beside D124A0C6 is preserved with its hash in the batch's
`symbols.json`; this Release profile did not request Rust debug information, so
source-level stack quality is unverified. Instrumented totals are diagnostic,
not new resource-acceptance measurements.

Visual Studio's native heap component is installed, but ordinary-token capture
for Rust and coverage of direct virtual/graphics allocations are unverified.
Installed CLI profiles provide no ready native-heap configuration; do not invent
one or substitute managed allocation counts. No profiler, service, registry,
installation or system setting was changed. The memory objective remains unmet;
no allocation cause or renderer optimization follows from tool availability.

### Retained visual inventory

`V:/temp/netmap-integrated-review/visual-inventory.md` identifies six reusable
full-target reference images in capture `52b0c8b2…`, paired with the six existing
shared-renderer BMPs. They cover only Dark, Countries, London, fitted 100-fact
idle/active/mixed scenes at 1080p and 4K. At that inventory's cutoff, other
required themes, selected states, precision modes and named geographic edge
cases lacked complete counterparts. The bounded visual-family batch below
adds current counterparts without relabelling these older images.
Completed timing phases inside failed captures do not supply visual evidence.

Root inspected all six pairs: the 1080p images at their supplied size and the
4K images displayed at 2048 by 1152. World placement, boundaries, activity fills
and active guide placement agree at those viewing scales. This is an overview
inspection, not a full-resolution marker, edge or effect comparison. Visible differences include
source role markers versus NetMap's directional/establishment marks and the
native speed overlay, which the source renderer-only capture omits. The native
fixture deliberately holds establishment visibility for 86,400 seconds rather
than using product defaults; its faint idle guides therefore cannot establish
default idle visual parity. This limited inspection does not establish complete
visual parity or relabel the older test binary's images as current product evidence.

### Matched visual families (2026-10-24)

Twelve new source/native pairs complete without case failures. Native evidence
is `V:/temp/netmap-visual-settings-20261024-c/native-visual/`; reference capture
is `V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-96cf1a01-a450-4f9b-ab7c-29ae0b3ef6b7/captures/`.
The reference Release capture DLL is A9E33CFC; the preserved optimized Rust test
executable is D124A0C6 (9,394,688 bytes). Full hashes, generated imports, image
hashes and source/binary provenance remain in those directories and the batch's
`reference-build.json` and `batch-summary.json`. Product/package D779778C is
unchanged. Prechange disposable harness and binaries are preserved in
`V:/temp/netmap-visual-families-prechange-20261024/`; TinyTorrent2 is untouched.

All cases use the same twelve supplied-coordinate, connected idle TCP facts,
English, 96 DPI, complete 1920x1080 targets, actual animation-enabled policy and
normal contrast. Source prepares through its actual MapModel/PeerMap, exports
actual destinations, palette, viewport and highlight membership, and reads back
its production renderer. Native checks conservation and every destination's
placement/share/group facts before importing that actual viewport and highlight.
Both use their existing renderer and unchanged settling bounds. There is no
memory/GPU timing matrix in this images-only mode. Native completes its exact
opt-in test in 23.63 seconds; formatting, all-targets Release Clippy with the
existing dead-code exception and 106 ordinary tests pass (17 opt-ins ignored).
The focused independent capture/import review is clean at
`V:/temp/netmap-integrated-review/visual-families-review.md`.

Root viewed every source/native image at original 1920x1080 size:

| Cases | Observed agreement |
| --- | --- |
| Light/Dark Countries fits; Light/Dark Cities fits | World extent, borders, country fills and destination placement. Countries has nine destinations; Cities has twelve. |
| Tokyo and Buenos Aires origins, Cities fits | Rebased geography, cuts and destination placement. |
| Fiji and Russia cuts, Cities at 6x | Coastline detail, fills and markers across the cut. |
| Edge before/after a requested longitude turn, Cities at 3x | Equivalent settled view; before/after files are byte-identical within each renderer. |
| Canadian islands, Cities at 32x, supplied point (-110,72) selected | Island geometry, fills, centre marker and selected ring placement. |
| Norway, Cities at 128x, supplied point (8,61) hovered | Detailed geometry and centre hover-ring placement. |

Native retains its establishment policy (introduced at zero, now 60 seconds,
86,400-second bright hold, zero fade), origin marker, faint guides and default
Speed/Traffic unavailable overlay. Source unknown-role idle peers omit those
guides, and its renderer-only image excludes the text overlay. The selected
native guide is consequently visible where the source idle guide is absent.
These are disclosed semantic/capture differences, not pixel-parity evidence.
No unexplained geographic mismatch was observed in these twelve settled views;
no numerical pixel threshold was introduced.

The +360 requested centre is normalized by source SetView, so this proves static
equivalence, not a full-turn gesture or cache lifetime. Selection/hover identities
are supplied; this is not pointer, keyboard or accessibility proof. Parent
windows still clip physical presentation despite complete target readback.
High Contrast, reduced motion, dense/4K selected scenes, navigation/input and
resource acceptance remain separate outstanding requirements. No OS setting,
UAC, installation or global input was used; no owned process remains.

## Scope and retained evidence

The first completed comparison uses the pinned TinyTorrent2 map revision
`95be5335eff5261d4f94b2e77a4bc0db0dcbf151`, optimized x64 builds, 96 DPI,
Countries detail and the same exported 100-observation idle/active/mixed facts.
Both renderers use 1920x1080 and 3840x2160 targets, clipped by their native
1280x900 parents. This does not establish physical 4K input/presentation.

- Reference measurements: `V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-517a2496-8ee7-4db3-9009-6c71fa8bb01f/captures/`.
  All 36 fitted phases and GPU checkpoints completed with no counter errors.
  Two independent 4K navigation journeys timed out with pending source rebuilds;
  those failures remain recorded and are not passing parity evidence.
- Reference images: `V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-52b0c8b2-e13d-4f20-88d9-7e467a61c533/captures/`.
  All six full-target images passed readback through the existing prepared
  renderer/device/palette. These exclude XAML overlays and physical presentation;
  no resource measurements are claimed from this images-only run.
- Corrected native measurements/images: `V:/temp/netmap-matched-dark-100-glowfix-20261023/`.
  The test exited zero after 263.49 seconds, completing 36 memory phases,
  36 GPU checkpoints with zero counter errors, and six target BMPs. Provenance,
  individual timings, peak samples and cycle values remain in that directory.
  Test executable SHA-256:
  `BCC6D67B600391B27AA4EA78E975F029B50DE4FE473386C501BC3917129D1C94`.
- Numeric analysis: `V:/temp/netmap-resource-checks/RESULTS.md`.
  The failed first native run at `V:/temp/netmap-matched-dark-100-20261023/`
  remains diagnostic evidence and is excluded from the results below.

The native test uses the production renderer/model/viewport, without a collector,
MMDB, network access or ordinary application startup. Reference figures below
cover its UI process; its separate engine adds approximately 5.14–5.38 MiB private
and 21.97–22.05 MiB working set. These scopes are not final product totals.

## Observed memory

MiB, **native / reference UI**, averaged over three detach/recreate cycles in
one process per implementation:

| Target and scene | Visible private | Visible working set | Visible dedicated GPU | Visible shared GPU |
| --- | ---: | ---: | ---: | ---: |
| 1080p idle | 123.82 / 234.70 | 98.55 / 286.75 | 46.18 / 224.37 | 11.12 / 48.70 |
| 1080p active | 154.35 / 311.24 | 112.88 / 310.07 | 67.89 / 458.70 | 12.35 / 69.81 |
| 1080p mixed | 150.14 / 326.06 | 108.76 / 322.49 | 65.95 / 650.52 | 12.31 / 73.32 |
| 4K idle | 179.81 / 367.20 | 139.51 / 363.26 | 119.04 / 1432.92 | 34.34 / 108.46 |
| 4K active | 207.64 / 433.18 | 141.33 / 382.82 | 152.29 / 2424.11 | 26.27 / 129.13 |
| 4K mixed | 213.55 / 445.66 | 143.03 / 387.70 | 150.09 / 3321.76 | 27.60 / 128.37 |

These runs establish lower measured native process totals, not an isolated map
private-memory reduction. Hidden/visible subtraction is distorted by retention:
the reference uses a shared-device path whose actual device lifetime remains
unverified, and native post-navigation private peaks can be reclaimed during
reattachment. Four paired private deltas
favor the reference; one native delta is negative because memory was reclaimed.
Reference dedicated GPU allocation rises from 105 MiB initially to 3579.27 MiB
finally. Its retention cause remains unexplained. GPU allocation counters do not
measure GPU execution or establish disjoint physical residency categories.

The source teardown removes the map's visual, disposes its explicit paths,
realizations, brushes, effects, fills and marker resources, and clears the
renderer/canvas references. The fixture verifies those references are null;
it does not prove native device destruction or release of every interop reference.
Native teardown drops the entire renderer/device, clears D3D bindings and flushes;
Flush is not a GPU-completion wait. Remaining reference allocations cannot yet
be attributed to reachable map caches, a leak or a particular algorithm.

Native hidden GPU allocations return to zero at every measured detach. Final
minus first visible private values range from -6.86 to +4.65 MiB, with larger
middle-cycle peaks mostly released. Sampled warmup private peaks at 323.46 MiB.
GDI remains 7, visible handles 473–475 and hidden handles 337–341. USER rises once
from 20/15 visible/hidden to 21/16 during active 4K and remains there. This is an
unexplained retained object, not evidence of exact resource-count equality or a
demonstrated progressive leak. Independent runs must establish variation.

## Renderer correction and visual scope

The first native run timed out at active 4K Europe-4x. Shared outline deferrals
were clearing the route-glow cache, repeatedly consuming preparation time before
remaining rebuilds could finish. Glow now invalidates on its own path/halo
replacement, with existing model/view/origin/palette/DPI invalidation preserved.
Actual deferred halo preparation precedes the cache-key return. The same
independent reviewer cleared the focused correction in
`V:/temp/netmap-stage1-review/review.md`.

Under the unchanged deadline, native active 4K zooms 2/4/8/16/32/64/128x settle
in 81/102/37/25/10/9/7 draws. The completed matrix exercises the former failure.
The corrected Release suite passes 93 ordinary tests, and the native marker,
palette, country-activity, expiry and static-scheduling check passes separately.
Clippy passes with `-D warnings -A dead_code`; the existing unused declarations
remain an unfinished integration gate, without source-level suppression.

The rebuilt product executable is 1,264,128 bytes, SHA-256
`50620291FA31D802DDC6A0710227F7A5B42687AEEA42D4565C140BDE6F028C43`.

The inspected full 4K active images retain matching world placement, country
shading and guide arrangement. Crop inspection confirms the horizontal Canada–US
border exists in both; no extra Atlantic longitude seam was established.
Native moving points are the specified additional traffic pulses. A small known
numerical difference remains: source border alpha truncates to bytes twice;
native brush opacity retains floats (69/255 versus 69.5/255 at this fitted DPI).
That can change a pixel by approximately one RGB level. This scoped inspection
does not establish every scene, theme, DPI or effect's visual acceptance.

## Separate native GPU timing

The reviewed test-only timestamp pass completed in 117.45 seconds at
`V:/temp/netmap-main-gpu-dark-100-20261023/`, against the completed source
100-observation facts. It has 54 batch records, zero baseline memory phases and
no resource comparison or images. Its optimized test executable SHA-256 is
`00B26A71E212A2CB82FC2A834D199583D77006EEA4CFB91B640D7443EC0AB2DC`.
Formatting, Release compilation, Clippy with the existing dead-code exception
and 93 ordinary tests pass on these inputs. The focused independent source
review is retained in `V:/temp/netmap-stage1-review/review.md`.

Device's hooks and query ownership compile only in tests. The timestamps bracket
main Direct2D BeginDraw through successful EndDraw, before Present. They exclude
route/dot preparation, Present and scanout, and can include CPU submission gaps,
GPU scheduling and preemption. They measure GPU-timeline elapsed time, not GPU
busy-unit utilization or full-frame cost. Query creation is after every baseline
memory/CPU phase in a normal run; this validation used `-GpuOnly` instead.

Fitted redraw milliseconds from available timestamp samples:

| Target and scene | Valid / issued | Pending | Median | p95 | Worst |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1080p idle | 30 / 30 | 0 | 2.26 | 7.20 | 9.88 |
| 1080p active | 30 / 30 | 0 | 2.79 | 9.77 | 10.38 |
| 1080p mixed | 30 / 30 | 0 | 3.29 | 11.11 | 11.57 |
| 4K idle | 29 / 30 | 1 | 17.16 | 39.54 | 42.79 |
| 4K active | 29 / 30 | 1 | 38.09 | 50.59 | 51.41 |
| 4K mixed | 29 / 30 | 1 | 30.32 | 42.84 | 43.08 |

Every batch has valid samples: 1,742 of 1,800 issued intervals completed at
collection, with 58 pending. No disjoint, invalid, incomplete, skipped,
unavailable or error results were recorded. Twenty-seven of 54 batches have
pending samples. Their raw
counts and all zoom/drag batches remain in `gpu-main-draw.json`. Collection reads
each query once with DONOTFLUSH, without a query wait/spin or forced flush.
Pending samples do not become zero or enter these quantiles; the missing tail
can affect their representativeness. The 4K values merit further analysis, but
the interval includes submission/scheduling and has no matched reference GPU
timestamps, so it cannot establish a GPU-busy regression or performance parity.

## Additional source failures

The combined 100/200/1000-observation source run at
`V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-81a06b2d-6402-495a-a8ee-f4d8e1597761/`
stopped after 60 fitted phases when its active-100 4K fitted scene failed to
settle. A fresh-process 200-observation run at
`V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-eefc98c8-9b76-468f-a3e5-89f20b6b1205/`
stopped after 32 phases when recovery from mixed-200 4K navigation remained
rebuild-pending at Fit. Both are terminal failures, with diagnostics preserved.
They cannot serve as complete matched 200/1000 baselines. The source renderer
and settling deadlines remain unchanged.

The separate 1000-observation source run at
`V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-a4766e62-f515-41ae-bcc4-b60fac2449f1/`
also failed, after 135.3 seconds and 24 fitted phases. Its fitted active 4K scene
had 146 destinations and 579 prepared callbacks but remained rebuild-pending
through the unchanged 20-second deadline. All three larger source launches are
terminal; no unchanged-source retry is currently running.

## Unmet acceptance

Stage 1 remains incomplete. Required work includes independent warmed repeats,
200/larger sets, High Contrast and remaining theme/navigation/visual cases,
explaining resource retention and establishing an attributable map-memory
reduction, separate live-collector/MMDB/product totals, complete and comparable
GPU execution/allocation diagnostics, input latency and outstanding physical accessibility,
touchpad, mixed-DPI and session/display checks. Existing source draw callbacks
and native paint times include different submission/presentation boundaries;
they cannot be compared as equivalent GPU time.
