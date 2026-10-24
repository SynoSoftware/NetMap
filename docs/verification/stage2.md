# Stage 2 integration evidence

Status: implementation in progress. Neither Stage 2 nor the outstanding Stage 1
visual/resource acceptance is complete. The user authorized concurrent Stage 2
implementation on 2026-10-23 and deferred all UAC and attended checks until the end.

## First integration batch

The retained map implementation now supplies ordinary-window exploration and
additional screen saver targets from the same application facts. Native modules
provide bounded Explorer discovery, reversible passive attachment, notification
icon commands, full-screen display surfaces and parent-owned preview. The entry
parser accepts `/s`, `/p <HWND>`, `/p:<HWND>` and `/c[:HWND]`; configuration uses
the existing Settings shell without starting collection. The package includes
`netmap.scr` copied from the same executable, sharing the existing assets.

Desktop and Screen saver are actual lazy Settings pages. Desktop enable and
monitor use the existing preference/draft/save owner. Explicit screen saver
operations run through Work; none is launched by startup or these checks.

Exploration uses a native virtual report ListView. Ready text and membership are
prepared on workers from accepted observations and existing map groups. The
collection publisher supplies current endpoint/lifetime facts independently of
location work; current scene replies enrich those rows with location. Known
membership survives a pending location update only for the same surviving ID.
New same-tuple identities never inherit membership or native row selection.

The first coordinator check identified SDK flag/import/visibility mismatches;
those were corrected. A focused independent review is recorded at
`V:/temp/netmap-stage2-review/review.md`. Its corrections cover common-control
registration for standalone `/c`, current rows before location, pending-row
identity, Escape before native dialog-key processing, selected-row/map agreement,
opening an already-selected marker and hiding map navigation in Table only.
The first optimized suite passes 100 tests with ten opt-ins ignored. Product
build `15892` passed; 1,380,352 bytes, SHA-256
`73FC172EF92BCFAF49365429BDDBD0CB357DD469895F6E9A80A6757A89109F5C`.
`V:/temp/netmap-stage2-fourth-20261023/result.json` passes normal view commands
with 225 ordinary-token rows, standalone `/c` with five native categories/no map,
and `/p` child resize/parent teardown. Every owned process exited cleanly.
Three earlier harness failures are preserved; they checked visibility under a
hidden STARTUPINFO launch before explicitly showing the owned target. The fourth
run corrects the harness. These checks establish native composition/lifetime,
not physical input, desktop placement or final product resource acceptance.

## Shared collection integration

Source adds the same Collector in an explicit elevated role and read-only local
snapshot transport. Viewer workers prefer the authenticated broker when present,
fall back to local truthful coverage when unavailable, and clear old identities
when the source changes. Epoch/sequence checks prevent replay across reconnect;
known row membership cannot cross source generations. Broker retention reads this
user's committed preferences, and collection ends after the final subscriber's
reconnect grace. Native acceptance of this privilege/lifetime path is deferred.

The existing process registry caches bounded names only after native sequence
evidence validates the same instance before/after the query. Ended observations
retain known names; expired interest removes them. Wire version 2 carries names
with full process IDs and validates frame/text bounds. No metadata I/O occurs in
ListView callbacks. Normal host foreground notifications suspend covered drawing
and use owned-window delivery through Settings' nested modal pump.

Focused review: `V:/temp/netmap-stage2-review/collection-integration.md`, corrected
foreground delivery defect, no unresolved material source finding. The optimized
suite passes 101 tests, with ten native opt-ins ignored (session `73582`). Release
build `84039` passes; 1,469,440 bytes, SHA-256
`DCF3497584EB3961FC3C382ACACADAE868D67E2D7FAB4CDD3E16508AD980549B`.
`V:/temp/netmap-stage2-integrated-20261023/result.json` passes normal view commands
with 241 ordinary-token rows, standalone configuration and preview teardown.
Clippy passes with warnings denied except the existing dead-code allowance.

The first desktop check did not pass. The sandboxed run could not add the tray
icon; the normal-token run outside the tool sandbox succeeded at that step but
failed before attachment with `0x80070006`. Both retained the usable normal map
and reported the failure. Evidence is preserved at
`V:/temp/netmap-stage2-desktop-diagnostic-20261023/result.json` and
`V:/temp/netmap-stage2-desktop-unsandboxed-20261023/result.json`. No UAC or
administrator token was requested. The next source batch adds operation-specific
native error context and native split-gutter dragging. The contextual run at
`V:/temp/netmap-stage2-desktop-context-20261023/result.json` narrowed the failure
to extended-style updates: reentrant native messages can overwrite last-error
when the previous style was zero. The fix verifies the resulting native style.

## Native hosting milestone

Release build `83070` passes; 1,474,560 bytes, SHA-256
`9EB4E6A91A8DBF967CD271D11A5A0D926DD39FD6957202435361479A21B0C87C`.
`V:/temp/netmap-stage2-hosts-20261023/result.json` passes (session `64840`):

- Native table focus, split-gutter resize, capture release and Escape rollback.
- 100 Open map/close-to-desktop cycles: 50 fitted, 50 at 128x zoom; the same map
  HWND survives, with WorkerW parent and passive styles/hit-test contracts.
- Standalone configuration, preview resize/parent teardown and clean exits.
- Full-screen `/s` targets match both reported displays, including negative
  coordinates; no exploration children; owned key-message dismissal exits cleanly.

These are owned-window structural checks, without global input or UAC. They do
not establish visual placement/input, display sleep, sign-in or mixed-DPI behavior.
Coarse private bytes changed from 109,662,208 initially to 158,179,328 after the
last zoomed cycle; differing view/cache warm-up and live facts prevent treating
that series as a settled memory-growth acceptance result. USER/GDI counts did
not grow across the sampled cycles. Keep full measurements in the JSON.

## Integrated review and correction batch

The final six-pass source review is recorded under
`V:/temp/netmap-integrated-review/` (spec, paths, flow, architecture, conventions,
elegance). Seven consolidated findings were corrected: pending desktop
disable/recovery ordering, preference-lock contention terminating the broker,
unplottable reason text, persistent search labeling, large plain payload disposal
and duplicate graphics resources across saver displays. Elegance found no
material simplification in its stated owner/interface coverage. Reports retain
scope and hashes; they do not claim every line was inspected or runtime acceptance.
The focused correction delta is clean across all six affected-code passes in
`V:/temp/netmap-integrated-review/delta.md`. Optimized suite `40488` passes 105
tests, ten native opt-ins ignored; Clippy all-targets passes. Release `16594`
passes; 1,539,584 bytes, SHA-256
`E02868EBF9937F69730BCDDAEB3A89FBE204249A908F7252AFC18F252F9A735D`.
`V:/temp/netmap-stage2-finalfix-hosts-20261023/result.json` repeats the full native
hosting milestone successfully against that executable, with 338 initial rows
and 404 after the 100 transitions; every owned launch exits cleanly.

Settings journey `76073` also passes against this executable, with captures in
`V:/temp/netmap-settings-finalfix-20261023/`: search, draft/validation, Apply,
reset/Cancel, save failure/retry, OK, queued close/reopen, restart, native scrolling
and compact/narrow reflow. The persistent search label is readable at the
simulated 400-pixel width. Twelve reopen cycles keep handle/USER counts unchanged
(560/63); GDI falls from 94 to 85. Keyboard injection and Automatic origin were
not requested for this run. These are scoped native checks, not final accessibility
or product-memory acceptance. The existing ignored renderer fixture passes in
session `55061`: two owned native targets use the same context/factory, draw and
read expected pixels at different extents/DPI, detach/reattach, release the final
target's resources and recreate successfully. Its test-only extension received
a clean focused review. It does not force a real device removal or establish
physical mixed-DPI presentation.

Installer compilation passes through `scripts/installer.ps1`. Outputs are in
`artifacts/packages/20261024-025658-684-df3047c7/`: setup 9,381,695 bytes and ZIP
10,437,890 bytes. The archive contains byte-identical `netmap.exe`/`netmap.scr`
matching the Release hash above, one shared world/country asset set and notices.
No generated setup was launched; selection, installation/uninstall and signing
remain unverified. Final formatting and Clippy pass after the test extension.

## Native exploration and foreground integration

The next retained batch separates queue polling from the existing typed reply
acceptance and centralizes final retirement in `Application::finish`. Both
production paths preserve their ordering. Optional test-only Host hooks supply
controlled accepted facts through these owners; they are absent from product
builds. Device startup no longer creates and discards capability-proof geometry,
sprites or effects; Outlines/Routes still create and use the actual resources.
No measured memory saving is inferred from that removal.

Focused review `V:/temp/netmap-integrated-review/native-integration.md` is clean
after correcting the optional frame observer to require both drawing and
presentation success. The fixture preserves the original publication clock.
Formatting and explicit x64 all-targets Release Clippy pass. Optimized suite
`10664` passes 106 ordinary tests, with 12 native/network opt-ins ignored.
Test executable SHA-256:
`C7305C850126DCAE2C64935BB6A1FCE588CA88AE27DFF13BE9C542E5B1351FFB`.

`tests/native-host-integration.ps1` passes all three exact native checks on an
ordinary token in `V:/temp/netmap-native-host-integration-20261024-final/`:

- Actual owner-data ListView callbacks return nine cells for declared accepted
  observations, including distinct applications, loopback and a public location
  miss. Native map marker/guide clicks filter authoritative country membership;
  a supplied coordinate selects its separate group. All/Escape restore rows
  without moving the viewport; row selection agrees with map place selection.
- Pending lookup preserves surviving selected identity/membership. Removing
  selected ended A clears its native selection while same-tuple live B remains;
  B survives faded presentation. Source reset cannot inherit reused IDs. Ready
  updates preserve native scroll, and both split orientations preserve their
  independent proportions and surviving selection.
- A real owned production `/s` process becomes foreground. The normal Host's
  actual Present-attempt count stays unchanged for at least 600 ms while covered,
  despite forced repaint and acceptance of a newer scene. It resumes with that
  exact model/newer revision and current clock age. The same check passes inside
  the actual Settings modal loop. Final counts hold at 32 then resume to 36
  normally, and hold at 45 then resume to 49 modally; resumed ages are 200.781 s
  and 400.751 s for sample anchors 200/400 s.
- The retained renderer pixel/static-mode/two-target lifetime fixture passes
  after removing the constructor proof calls. All owned processes exit zero;
  neither case needs timeout cleanup.

These observations prove native presentation of controlled accepted facts and
real saver foreground delivery. They do not substitute for live ETW retention,
MMDB lookup, application endpoint delivery across a target gap, physical input,
screen saver installation/selection, scanout or full resource acceptance. The
earlier real socket-reuse error remains an incomplete external-source check.

Installer compilation and payload inspection pass. Current package:
`artifacts/packages/20261024-040753-081-f1a72138/`, setup 9,382,322 bytes, ZIP
10,439,404 bytes. Its byte-identical EXE/SCR match the product used by the final
native run: 1,540,608 bytes, SHA-256
`D779778C0C1AEE9C9D57D518516158B203255DD5BAB68B11087BDA9F61301880`.
No setup was launched. The first native pass at
`V:/temp/netmap-native-host-integration-20261024-a/` also passes against its
recorded earlier build hash; it is not the final package's provenance.
The final run's `binaries/` retains both verified executables at these hashes so
subsequent Cargo builds cannot overwrite that evidence.

## Work replies across a map target gap

`V:/temp/netmap-cold-contribution-20261024-a/native-lifetime/result.json`
passes three selected native checks on ordinary Windows token
(`IsAdministrator=false`). The preserved optimized test binary has SHA-256
`5306F59B6DAC26F6EE30597E42CA332894A5411CDD5CAC7FA372E3B72A9B010A`;
the product/package remains `D779778C…`, unchanged by this test-only batch.

The new lifetime check gates two typed Scene replies in one registered Work
worker and delivers them through the actual application-owned Endpoint while
the owned map HWND is destroyed. It observes exactly one accepted current
generation (3) and one discarded stale generation (2), with no replay on empty
wakes or reattachment. The replacement production map presents the accepted
Model, preserves the non-fitted viewport and original sample clock (200.849
seconds at the observed frame), and resumes the four current native table rows.
Tooltip/accessibility/renderer retirement and final owned-process cleanup use
their existing owners. Its focused independent review is clean at
`V:/temp/netmap-integrated-review/lifetime-review.md`.

The common native fixture now creates the real Endpoint and keeps it through
Application finalization. Affected exploration and actual normal/modal `/s`
foreground checks pass again on that fixture. Formatting, explicit x64
all-target Release Clippy and optimized test compilation pass. This batch does
not establish origin/dataset-specific transitions during a gap, genuinely
pending I/O cancellation, physical Explorer replacement or full resource
acceptance. Those remain separate checks.

## Close during pending I/O and uncooperative work

`V:/temp/netmap-discriminating-20261024-a/shutdown/result.json` passes both
exact native shutdown checks on preserved optimized test SHA-256
`727C2E3E60AFA9748D0B02C9B1154A4582DCADFFA94D9C6775558173330077CA`.
Formatting, all-targets Release Clippy and 106 ordinary tests pass (15 opt-ins
ignored). Focused review is clean at
`V:/temp/netmap-integrated-review/shutdown-review.md`. Product/package D779778C
is unchanged. The wrapper confirms IsAdministrator=false, no watchdog timeout
and both exact owned process instances absent after exit.

The HTTP case uses registered Work server/client jobs on an owned ephemeral
loopback socket. The server declares 131,072 bytes, sends 65,536, then withholds
the remainder. The actual asynchronous Request consumes the whole prefix,
issues its next read into the retained stable buffer and observes 200 ms without
completion/error before reporting readiness. Only then does the UI post the
real WM_CLOSE. The server deliberately remains open through cancellation until
Request teardown and callback-context reclamation; EOF cannot stand in for
successful cancellation. Production Request::wait returns Cancelled, Drop
returns and the weak callback context expires. Window removal takes 16 ms;
cooperative Work finalization takes 73 ms, with normal test/process exit 0.

The separate case registers a cancellation-ignoring 20-second gate with its
sender retained inside the job. WM_CLOSE again removes the window in 16 ms.
The unchanged Work five-second deadline initiates actual own-process fallback
at recorded elapsed 5,004 ms, with 4,712 microseconds deadline lateness. The
wrapper observes expected exit 1 another 15 ms after the fallback timestamp;
there is no gate-returned or cooperative-finish marker. Later process-instance
absence is observed separately. These are measured stages, not a guarantee of
kernel process-removal latency.

All test setup and observers are cfg(test); Work remains the single execution
and shutdown authority. No UAC, certificate, public download or global input
is used. Plain-loopback Request lifetime does not establish HTTPS behavior,
city-install/reopen integrity, pending preference commit integrity or elevated
trace cleanup; those acceptance items retain their separate scope.

The existing native dataset recovery check subsequently passes separately on
the same preserved 727C2E3E test binary:
`V:/temp/netmap-native-integrity-20261024-a/result.json`. It uses a retained
owned 126,998,165-byte city MMDB (SHA-256
`9E250F02722D1AD1780F88192F0E908AF285478A33E76B88B349C9181AE9D0F5`),
without downloading it again. A second live mapping blocks replacement: the
old version and actual lookup survive, the flushed backup remains usable after
owner recreation, and a later install/reopen succeeds and removes the backup.
The source hash is unchanged and the isolated temporary directory is empty.
This is an ordinary-token native file/recovery pass, separate from interruption
of an install during Host shutdown or forced process fallback.

## Local MMDB and origin results across target replacement

The actual native test passes on preserved optimized test F47D775F in
`V:/temp/netmap-acceptance-20261024-b/local-locations/`: exit 0, one passed,
1.52 seconds, no public-discovery refusal or stderr. The wrapper initially
fails to recognize its marker because Rust prefixes the line with the test
name. Its original `result.json` remains Passed=false. The corrected wrapper's
exact case-validation block is evaluated against the preserved stdout/stderr
and copied dataset in `parse-validation.json`, with hashes and all assertions
passing. This is a successful native case plus corrected offline parsing;
it is not a newly executed green wrapper. The focused source/fix/parser review
is clean at `V:/temp/netmap-integrated-review/local-locations-review.md`.

A registered Work job loads the real shared Geolocation reader from bundled
country data and an isolated copy of the retained city MMDB (exact hash recorded
above). It gates the actual version-1 availability report until map NCDESTROY
has detached Renderer/accessibility; the application-lived Endpoint remains.
Production local Origin and Scene workers then use that same reader/version.
Two controlled discovery answers seed only the local lookup state; a scoped
test-only guard prevents HTTP discovery even if a real network notification
invalidates the setup. No alternate reader, grouping or acceptance path is used.

The current report, city availability and two real local origin answers update
the application while no map target exists. Fit adopts the first meridian
(-122.085); the second answer preserves the nonfitted geographic centre and
zoom. Native rows agree with actual MMDB places, and loopback stays unlocated.
The actual pre-load version-0 report and two late Origin replies cannot replace
current state. Those Origin replies arrive after Ready; this does not separately
discriminate pending-request identity and dataset-version guards.

The replacement uses the production native map class and owners. Its successful
Present must contain the exact accepted Model pointer and current sample age;
five native virtual rows expose actual coordinate precision. The recorded result
has three accepted/two discarded scenes, 21 Present attempts and age 301,108 ms.
Empty endpoint wakes and reattachment do not replay the result. The affected
existing Scene-only target-gap case also passes in `scene-regression/result.json`
(one accepted/one discarded, four rows). The final ordinary suite passes 106
tests with 16 opt-ins ignored; formatting and all-targets Release Clippy pass.
This proves local load/lookup/result lifetime across a presentation gap, not
HTTPS download/install, adapter mutation or process-wide resource acceptance.

## Settings Screen saver Preview command

`V:/temp/netmap-acceptance-20261024-b/preview-revised-with-scr.json` and
`preview-revised-without-scr.json` pass against fresh private copies of packaged
D779778C. Only the actual Preview command (resource ID 1163) is invoked; Install
and Windows settings commands are untouched. The focused review and guard/logging
delta are clean at `V:/temp/netmap-integrated-review/saver-preview-review.md`.

With the adjacent byte-identical SCR present, Work/Shell launches that exact
private file. The harness retains its process handle and verifies path/birth
time before touching native windows. Two visible saver surfaces cover
(0,0)–(3440,1440) and (-1920,0)–(0,1080), both 96 DPI. Application status receives
asynchronous success. Numeric WM_CLOSE to an owned saver HWND ends all surfaces
and the retained process exits 0. The same Settings dialog remains usable;
saved preferences are unchanged and the parent also exits 0.

Without the SCR, status first explains the absent package file, then the same
Preview command receives asynchronous Windows file-not-found error 2. No process
at that private path appears during the two-second observation. Settings remains
usable and preferences unchanged; parent exit is 0.

The first attempt at `preview-with-scr.json` failed with a harness null-value
error before completion. Its originating line is unproven because the initial
outer catch retained no stack. The corrected harness guards nullable unrelated
process metadata and saves phase/full exception/source stack before cleanup;
the later successful checks retain fresh isolated profiles and leave the original
failure unchanged. No copied process remains. This checks the app's Preview
command and owned programmatic dismissal, not Windows saver installation,
selection, physical input, display sleep or sign-in behavior. No UAC, registry
mutation or global input is used.

## Settings close and reopen during an actual pending save

`V:/temp/netmap-visual-settings-20261024-c/pending-settings/result.json` passes
on preserved optimized test D124A0C6, ordinary token, exit 0. The exact native
test completes in 0.85 seconds; its stderr is empty. The clean focused review
and shared-hook coverage are recorded at
`V:/temp/netmap-integrated-review/pending-settings-review.md`.

A scoped next-save gate holds the actual registered preferences worker before
the existing storage call. The UI requires worker entry and the real Saving
phase with accepted bright=17, while committed preferences still differ. It
submits through the old dialog's real OK path, verifies disabled editors/reset/
commit and enabled search/navigation/Cancel, then creates the lazy Desktop page
and verifies its controls are disabled too. WM_CLOSE ends that modal generation
while the application save remains pending.

A separate real Settings show observes the same pending save in a new dialog
generation. Its newly created Desktop controls also remain disabled. Releasing
the worker permits the existing storage/save/reply path to complete. The new
dialog remains open despite the old OK intent, shows committed bright=17 with
a clean draft/baseline and disabled Apply, and reenables its editors. A further
200ms observation precedes Cancel. After process exit, the wrapper independently
checks actual bright=17 on disk and no preferences.rollback file.

This establishes accepted-save lifetime and modal-generation delivery without
another preference authority or simulated completion. It does not prove an
interrupted atomic replacement, physical keyboard/accessibility or layout.
All new gates/drivers and shared registrations are cfg(test); product/package
D779778C is unchanged. No public network, UAC, saver action or global input was
used, and no owned process remains.

## Pending-save Host exit and fallback

The new Exit mode passes in
`V:/temp/netmap-lifetime-20261024-d/settings-exit/result.json`, using preserved
optimized test 61160460 (full SHA-256 in the batch summary). Both exact tests
use ordinary tokens and fresh isolated profiles. The focused combined review
is clean at `V:/temp/netmap-integrated-review/lifetime-exit-desktop-review.md`.

Each case reuses the real modal OK acceptance, worker-entry gate and modal close,
then posts actual Host WM_CLOSE while accepted bright=17 is still pending and
committed bright=10 remains authoritative. Normal completion removes the HWND
in 47ms; only then does the fixture release that same save worker. Existing
Application/Work finalization takes 12ms, the exact test passes in 0.51 seconds
and process exit is 0. The actual saved file contains bright=17 with no rollback.

The fallback case removes its HWND in 32ms and retains the same sole save gate
through finalization, with no extra uncooperative job. The existing five-second
fallback initiates after 5007ms (7.271ms deadline lateness), before the gate's
ten-second bound. No gate return/release or finish-completed event occurs.
The owned process exits with the expected code 1; exit and instance absence
are observed 16ms and 31ms after the fallback record, respectively. A worker
read through the production parser establishes a usable bright=10 baseline;
after exit its file is byte-identical and has no rollback record. These checks
prove accepted-save lifetime and preservation before storage, not interruption
during atomic replacement. Existing blocked-replacement recovery has its own
earlier evidence.

The affected Reopen mode also passes separately on 61160460, retaining actual
commit17, newer-dialog survival and absent rollback. Formatting, all-targets
Release Clippy with the existing dead-code exception and 106 ordinary tests
pass (20 opt-ins ignored). Product/package D779778C is unchanged; no owned process
remains. No UAC, public network, global input or system setting was used.

## Desktop disable and delayed recovery

`V:/temp/netmap-lifetime-20261024-d/desktop-recovery/result.json` passes on
61160460, ordinary token, exit 0; the exact native test takes 3.52 seconds and
stderr is empty. This is actual Explorer discovery, current monitor reads,
registered Work replies, native menu commands and committed preference saves,
with only reply delivery held by a private test gate. It supplies no synthetic
Attachment or alternate completion path.

The first actual discovery (id 1, two monitors) is held while the Desktop menu
saves disablement. After the worker finishes and its ordinary Endpoint/Host
messages drain, the same map remains under the visible normal Host. Re-enabling
starts a fresh actual discovery (id 2), positively proving the old lifecycle
completed. One direct recovery and two owned WM_DISPLAYCHANGE notifications
coalesce to current request id 5. Delivering obsolete id 2 starts that fresh
discovery without attaching. Only release of current id 5 establishes a valid
WorkerW attachment; final menu disable returns the exact same map to the original
Host parent and persists desktop=false/manual origin.

Zoom remains 128x, with meridian and geographic centre preserved across changed
host sizes. The native timer advances at all four checkpoints (25/28/30/32),
including while replies are held. The combined focused review is clean. Failure
cleanup owns only the test map/window; no Explorer window is destroyed. This
proves current discovery and owned recovery-message ordering, not physical
monitor changes, actual Explorer restart, icon input, mixed DPI or host resource
acceptance. No UAC, registry/topology mutation, public HTTP or global input was
used; no owned process remains.

## Dataset transaction lifetime and Settings availability

2026-10-24. All four cases pass in
`V:/temp/netmap-dataset-lifetime-20261024-e/result.json`, on preserved optimized
test `3126C73B874168278FF870FBDB601B2A04C1F9AD2BA6DC420CD91E36562E0267`.
They use fresh owned profiles and the retained 126,998,165-byte city MMDB
(`9E250F02…`); its source hash is unchanged. No download, UAC, physical input or
system mutation runs. Every owned process is observed absent afterward.

- After successful native replacement and flushing the previous backup, the
  fixture displaces only its new target before production reader reopening.
  Actual reopen failure rolls back to the usable previous dataset at version 1;
  retry succeeds at version 2 with the same lookup. Both candidate files pass
  the production validator. The native case takes 11.59 seconds.
- Modal Settings sees real Missing → Ready → Missing reader reports at versions
  1/2/3. Apply rechecks Cities availability, keeps the invalid raw latitude across
  page routing and never commits the rejected Cities choice. The case takes
  7.48 seconds; saved preferences remain Countries.
- Host close while the sole registered installer is held after replacement and
  before reopening removes its HWND within the tick counter's resolution. The
  actual five-second fallback starts at 5005 ms, with 5571 µs deadline lateness;
  exit 1 is observed 15 ms later. The gate has not returned and cleanup has not
  completed. Canonical and independently flushed backup files retain the exact
  source hash.
- Restart reads the interrupted canonical file, then the existing recovery
  loader reads the retained backup after the canonical file is withdrawn.
  Both lookups are usable and agree; the report advances from version 1 to 2.

This interrupts an explicit boundary between successful replacement and reader
reopening, not an atomic Windows call. Old and new candidates intentionally have
identical contents; this proves transaction/recovery/version behavior rather than
different-generation record values. The focused source review is clean at
`V:/temp/netmap-integrated-review/dataset-lifetime-review.md`.

## Remaining implementation and unattended verification

The 2026-10-24 completion-gap audit at
`V:/temp/netmap-integrated-review/acceptance-gaps.md` separates missing native
feature wiring evidence from deferred UAC and attended work. It prioritizes real
owner-data cell/selection/expiry, concurrent foreground suspension, monitor/draft
effects and application results crossing presentation gaps. Existing narrow
passes do not prove those broader journeys.

An additional source ordering defect was fixed during this audit: a new desktop
request could leave an older completed result waiting for Host, or consume an
in-flight topology refresh without repeating discovery. Application now clears
ready results on request and coalesces requested/running identities; Work tags
completion with its initiating ID. Only the latest request publishes, with one
fresh discovery after obsolete work completes. The lasting regression passes
(`58526`) and the focused delta review is clean at
`V:/temp/netmap-integrated-review/desktop-freshness.md`. Clippy passes; the new
Release `24445` passes; 1,540,608 bytes, SHA-256
`2C0E2568809BC2DA91DA33E5E612BEB58DB1709983AA971DF6F0364EB55564C5`.
`V:/temp/netmap-stage2-discovery-20261024/result.json` passes against this binary:
100 desktop transitions (50 fitted, 50 at 128x), retained map HWND, normal native
views/split, configuration and preview teardown. There are 211 initial rows and
231 after the transitions. Sampled USER/GDI counts do not grow (45/23 initially,
41/20 finally); the live, differently warmed memory series is not settled memory
acceptance. Earlier binary/package hashes above retain their stated scope.

Installer compilation and package inspection also pass for this binary. Current
outputs are in `artifacts/packages/20261024-032615-064-dbf42d40/`: setup
9,382,708 bytes and ZIP 10,438,868 bytes, with byte-identical EXE/SCR matching
`2C0E2568…`, shared assets and notices. No installer was launched.

The first real-loopback exploration journey at
`V:/temp/netmap-native-exploration-20261024-a/result.json` stops before row checks:
the native ListView does not expose GridPattern to this UI Automation client.
The owned app exits cleanly. This is a failed harness/provider precondition, not
proof of connection selection or expiry; inspect supported native accessibility
before adapting the journey. No new product control/provider is justified solely
to satisfy this harness.

The bounded diagnostic at `V:/temp/netmap-native-exploration-20261024-b/`
identifies `SysListView32`, report view and nine columns. Explicitly registering
the installed Microsoft client-side providers in the disposable STA client
restores Grid/Selection/Table patterns and reads all nine controlled loopback
cells correctly, including the process name and explicitly unavailable rates.
That run then fails on a harness-only managed SelectionPattern method mismatch;
selection/expiry remains unproved until the corrected journey completes.

Run `V:/temp/netmap-native-exploration-20261024-c/` additionally passes native
selection, Escape/All clearing and live-row persistence beyond the establishment
highlight. Its later scan races a shrinking live row count and fails out of
range; it does not prove retirement or same-tuple reconnection. The stock proxy
also exposes a real naming defect (the preceding zoom label becomes the table's
accessible name). Host now labels the native table through its existing
IAccPropServices owner and clears the annotation before child destruction;
`V:/temp/netmap-integrated-review/table-name.md` is a clean focused source review.
The final correction annotates both MSAA and UI Automation name properties,
following the existing help-property pattern; cleanup clears both. Release
`47999` passes (1,541,120 bytes, SHA-256
`4C2D3A371FA18A6A12F7288BC7F0E329992DF546F924F45B0FF5AFD3E237295D`),
with formatting and explicit x64 all-targets Release Clippy passing. The focused
review covers this refinement. It preserves the desktop-discovery fix.

Run `V:/temp/netmap-native-exploration-20261024-d/` verifies the stock provider's
exact Connections name, nine controlled cells, Map/Table/Both focus/visibility,
selection, Escape/All and live-row persistence beyond the full highlight. One
detected live-grid read race is retried within the existing deadline. The
`retired-long` phase additionally establishes that closing the faded connection
removed its row and cleared selection. The next exact-tuple reconnect fails
in the owned socket driver with Windows error 10048; the app exits cleanly.
The overall journey fails. Ended-row retention, same-tuple coexistence and final
selection expiry remain unverified by this native run. Do not repeat unchanged
socket-reuse attempts or treat the partial checks as full acceptance.
The retained harness now saves every completed checkpoint before advancing its
phase and preserves partial results when the socket driver fails. It classifies
exact error 10048 as incomplete acceptance while keeping Passed=false. Root
parses both parent and embedded STA scripts successfully; this logging-only
change was not used to rerun the unchanged socket precondition or rewrite run d.

Installer compilation and payload inspection pass for this final binary.
Current package `artifacts/packages/20261024-033804-962-b3628737/` contains
byte-identical EXE/SCR matching `4C2D3A37…`, setup 9,382,199 bytes and ZIP
10,439,130 bytes. Earlier packages above retain their earlier scope; none was
installed.

Hosting Settings journey `40513` passes on `2C0E2568…`, with captures in
`V:/temp/netmap-settings-hosting-20261024-a/`. Desktop draft values survive page
search, Cancel preserves the committed file byte for byte, and Apply targets
`\\.\DISPLAY4` at (-1920, 0)–(0, 1080) while retaining map HWND and Settings owner.
Open map restores the native parent; reopening preserves the committed choice;
Desktop Reset/Apply restores disabled/automatic defaults and preserves other
pages. Screen saver search/status and all three named actions are present, page
Reset is absent and navigation leaves preferences clean. No saver action runs;
the build directory correctly reports its adjacent SCR absent. Twelve Settings
reopens retain handle/USER/GDI counts (438/38/36). Every owned launch exits cleanly.
Physical input, topology changes, saver actions and final resource acceptance
retain their separate scope.

- Verify the integrated broker/fallback/name paths against the current product;
  source review does not prove native privilege behavior. Normal and modal
  foreground suspension are proved above under their stated fixture scope.
- Verify actual Explorer restart and monitor/topology changes beyond the
  completed controlled disable/recovery, connected-display Settings and
  host-cycle journeys. Shared application replies across a temporary target gap
  retain their separately recorded Scene/local-MMDB scope above.
- Verify live collector/MMDB-to-list delivery, physical keyboard/accessibility
  and Screen saver explicit actions. Controlled accepted-fact owner-data,
  identity/expiry, filtering and both split orientations pass above.
- Matched 100-observation resource journey `49903` completes against the updated
  production renderer (`V:/temp/netmap-shared-renderer-resources-20261024/`): 36
  memory phases and all GPU timing batches, with zero explicit timing gaps. It
  reuses the completed reference capture; failed source matrices were not rerun.
  Both idle BMPs are byte-identical to the preceding renderer's captures; the
  1080p active pair retains geometry/effects on visual inspection, with different
  pulse positions. This is scoped comparison, not full visual acceptance. Earlier
  Stage 1 values do not establish current host/list totals. The
  [current numeric comparison](matched-map.md#current-shared-renderer-rerun-2026-10-24)
  records lower totals than the reference UI, higher post-navigation means than
  the preceding native run, stable lifetime counts and zero GPU allocations
  after detach. Full resource acceptance remains open.

## Deferred unattended tooling boundaries

2026-10-24. The lead executes installed `VSDiagnostics.exe help` with exit 0
and checks the shipped agent configurations. There is no ready native-memory
configuration in that inventory, and the documented CLI supplies no native-heap
snapshot recipe. NativeMemoryCollectionAgent.dll is present; its configuration
and snapshot protocol are unverified. The documented native Heap Profiling path
uses the Visual Studio UI and restart/snapshots. The available computer interface
is browser-only; no unattended native-profiler automation is established.
No IDE, profiler session, service action or
target launch is performed. Ordinary native heap profiling is not claimed to be
impossible or permission-denied. See the retained
`V:/temp/netmap-integrated-review/memory-attribution-next-step.md` and Microsoft's
[CLI documentation](https://learn.microsoft.com/en-us/visualstudio/profiling/profile-apps-from-command-line?view=vs-2022)
and [native heap workflow](https://learn.microsoft.com/en-us/visualstudio/profiling/memory-usage?view=visualstudio).

The actual-device-loss gap also has no established safe process-scoped trigger
for this D3D11 renderer. The installed windows 0.62.2 bindings and Microsoft's
[ID3D11Device5 contract](https://learn.microsoft.com/en-us/windows/win32/api/d3d11_4/nn-d3d11_4-id3d11device5)
provide fences, while Device4 registers removal notifications; neither provides
the D3D12-only RemoveDevice operation. Microsoft's D3D11 test procedure uses
`dxcap -forcetdr`, which resets the adapter and affects other Direct3D apps.
See [D3D11 device-loss testing](https://learn.microsoft.com/en-us/windows/uwp/gaming/handling-device-lost-scenarios)
and the [Microsoft DirectXTK guidance](https://github.com/microsoft/DirectXTK/wiki/DeviceResources/f240e153c34caa712af23fd1537edc644f6435fb).
No GPU reset, tool installation, API migration or fake removal is performed.
Existing source review and detach/recreation evidence retain their narrower
scope; actual removal and recovery remain unverified.

## Deferred until the user returns

The user has returned and authorized the bounded UAC journeys and brief attended
screen saver/desktop checks, restoring prior saver settings. This list records
the earlier deferral, with current outcomes below; it does not revoke that
authorization.

- Installed WPR VirtualAllocation capture for the unresolved private-memory
  attribution, with the recorder elevated and the preserved target ordinary.
  Zero-loss blank-device and full-map captures now complete; see
  [allocation attribution](matched-map.md#resumed-native-allocation-attribution-2026-10-24).
  Changed-renderer savings remain unmeasured and memory acceptance remains unmet.
- Elevated collector regression/resource rerun and normal-client/elevated-broker
  ACL, source epoch, cancellation and final-subscriber lifetime checks.
  Raw/Collector regressions pass. Broker run f proves measured native rows,
  saver-only survival and natural first-collector exit; full replacement/final
  lifecycle acceptance remains open after fixture corrections. See
  [connection evidence](connections.md#remaining-integration).
- Windows screen saver installation/selection/configuration, physical input
  dismissal, display sleep and sign-in-on-resume.
- Actual mixed-DPI displays, precision touchpad, screen reader, High Contrast
  and system-motion journeys where they require user/environment changes.
- Attended desktop icon input, Show desktop, Explorer restart and host-cycle
  visual acceptance on the actual Windows build.
- Actual device-loss/recovery validation in an attended environment that can
  safely accommodate its adapter-wide effects; no automatic TDR is authorized.

No deferred item is a pass. Keep the original source timeout results and Stage 1
resource limitations in [matched-map.md](matched-map.md).
