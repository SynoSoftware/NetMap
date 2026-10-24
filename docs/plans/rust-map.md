# NetMap software specification

Status: Stage 1 acceptance remains open; Stage 2 implementation is authorized and underway.
The native map, Settings, local geolocation and TCP/UDP collector are present;
source privilege restrictions and remaining visual/resource gates are recorded
below. Written 2026-10-22; reviewed 2026-10-23.

This document owns intended product behavior, scope, technical constraints and
acceptance criteria. It does not claim that implementation or platform checks
already exist. [CONTEXT.md](../../CONTEXT.md) owns the domain language.

Mandatory implementation guidance lives in [Architecture](../agents/architecture.md),
[Porting](../agents/porting.md) and [Development](../agents/development.md).
Those documents own module organization, native mechanisms and working
procedures; this specification owns the result they must deliver.
The [Settings specification](settings.md) owns the dialog's content tree, search,
resets, editing behavior and Goz shell porting requirements.

## Implementation status

Current batch (2026-10-24): collector token queries and renderer allocation
changes are built together. Formatting and all-targets Release Clippy pass;
preserved optimized test 5B6F4ADF passes 107 ordinary tests, with 27 native opt-ins
ignored. The combined all-targets Release build takes 138 seconds. The retained
native regression covers the actual current-token identity/elevation query;
TokenElevation uses a direct typed four-byte record, while variable-length
TokenLogonSid retains aligned storage. Earlier failed builds and native error-24
evidence remain in [connection verification](../verification/connections.md#remaining-integration).
No authorization policy changes.

Production Release A461327D is 1,543,680 bytes. Its byte-identical EXE/SCR payload,
ZIP and compiled Inno Setup installer are in
`artifacts/packages/20261024-110358-559-292bc7d0/`; the installer is built, not
installed. The normal production-feature package build and all-targets build
have distinct hashes; broker run d uses private all-targets product 1EF9FF0A and
test 5B6F4ADF. The user approves the waiting consent prompt; run d starts and
proves measured native cells, then its saver is dismissed. Corrected wrapper run
f on packaged A461327D proves eleven seconds of saver-only subscription and a
natural collector exit 7.13 seconds after the saver exits. Its replacement-source
assertion stops at initial incomplete coverage; the fixture now waits within its
existing bound. Preserved test 7925B3A0 passes 107 ordinary tests. Subsequent run g
exposes test-only Work retirement waiting on deliberately retained UI facts.
The fixture correction keeps all retirement join handles with the continuing
Work owner while retiring the old source; production behavior is unchanged.
Optimized test AA0DEEBB builds in 119 seconds and passes all 107 ordinary tests;
formatting/all-targets Release Clippy pass. It is preserved in
`V:/temp/netmap-broker-retirement-20261024/`. This correction still needs its
focused native run; full broker acceptance is open. Packaged A461327D is unchanged.

User authorization covers the bounded allocation/collector/broker journeys and
brief attended screen saver/desktop checks, with prior saver settings restored.
Raw/Collector checks already pass on preserved test 5680939D; zero-loss native
allocation traces identify D2D group-layer, image/effect and gradient realization
costs. See [allocation evidence](../verification/matched-map.md#resumed-native-allocation-attribution-2026-10-24).
Reviewed routes.rs 68EBFE79 tightens conservative group-layer bounds and shares
identical halo gradient stops at their existing owner. The ordinary-desktop
measurement estimates 159.15 MiB map contribution versus the prior native
183.67–184.39 MiB, but still exceeds the reference's 144.05–144.92 MiB. Repeated
matched-pair acceptance remains open. Changed-renderer Navigation/Temporal pass
on 5B6F4ADF outside the command sandbox. Next: finish the focused broker
correction and remaining memory acceptance, then the authorized attended
journeys. Additional review rounds or test infrastructure require user approval.
Lead remains the single builder/process runner. Neither stage is accepted.
The user requests minimal screen disruption: keep builds/code checks in the
background, use off-screen rendering where the check remains equivalent, and
batch the necessary attended desktop/saver/input journeys into brief announced
sessions. Preserve their acceptance criteria; avoid visible test launches while
the user is working. All owned native test windows have exited.
Current acceptance is blocked pending the user's availability for that brief
attended batch. The user explicitly requests waiting until they say they are
ready; do not launch desktop checks before then. Resume with preserved test
AA0DEEBB and packaged A461327D, not another build or broad review. The memory
shortfall and unverified physical/platform cases remain incomplete.

Earlier release/package checkpoint: A60AF2EB (1,540,608-byte
EXE and byte-identical SCR). Settings accessibility passes all seven checkpoints
after a native Categories label fixes the stock provider's mistaken Search-label
association. The ineffective dialog COM annotation is removed. See
[Settings evidence](../verification/settings.md#native-accessibility-provider-check).
Focused resource follow-ups pass on unchanged A60AF2EB. Twenty visible Settings
reopen cycles hold process handles at 482; GDI stays within 24–25 and USER within
42–44 during measured cycles. This does not reproduce sustained per-dialog GDI
growth. Private memory rises by 3.57 MiB through final idle with changing live
facts, so allocation ownership and leak freedom remain unproved. See
[Settings lifetime evidence](../verification/settings.md#settings-reopen-resource-follow-up).
A separate single-query city-profile run identifies both exact MMDB mappings:
country has 466,944 resident bytes and city 966,656, independently of their
virtual/file lengths. The wrapper now distinguishes documented file mappings
from pagefile/physical mappings; the earlier error-1006 records stay unavailable.
Both wrapper-only changes have clean focused review; no Rust rebuild is needed.
The final ordinary timing follow-up passes Navigation on preserved optimized
test 5680939D: 24 owned-dispatch-to-last-matching-Present-return samples have
median 33.017 ms, nearest-rank p95 49.747 ms and worst 66.418 ms. This includes
the existing message pump and explicit repaint, excluding physical input, OS
input queues and compositor/display latency; it is not a matched-source latency
comparison. See [timing scope](../verification/matched-map.md#owned-navigation-input-timing-2026-10-24).
One optimized build takes 76 seconds and Navigation 14.60 seconds, within the
five-minute estimate. Formatting, all-targets Release Clippy and focused timing
review pass. Temporal/source matrices are not repeated; cfg(test)-only changes
leave product/package A60AF2EB unchanged.
The corrected dynamic/temporal map journeys pass on preserved optimized test
7142D77A; root inspected all 22 correlated full-target images. Actual dataset
rollback/retry, open-Settings availability changes, held-install fallback and
canonical/backup restart all pass on preserved test 3126C73B. See
[map evidence](../verification/matched-map.md#native-dynamic-navigation-and-temporal-presentation-2026-10-24)
and [transaction evidence](../verification/stage2.md#dataset-transaction-lifetime-and-settings-availability).

Scoped final-product accounting completes 19 phases, both parents and three
two-display Preview lifecycles, all exiting 0. Their original mapping-residency
queries remain unavailable; the separate follow-up above supplies one later
scoped observation. Changing live facts and cold/warm differences prevent
isolated allocation or leak-freedom claims. See
[component accounting](../verification/matched-map.md#scoped-ordinary-product-component-accounting-2026-10-24).
Formatting, final all-targets Release Clippy and native corrections pass. The
106 ordinary tests pass on 3126C73B; the later correction changes capture
instrumentation and a native label, with no pure-rule change. Both optimized
test builds take 77 seconds; final product compilation takes 45.28 seconds.
Focused source reviews and affected correction deltas are clean. Original
provider failures, stale-buffer captures and hidden-start accounting failure
are preserved separately; no failed source matrix is retried.

The ordinary profiler follow-up runs installed VSDiagnostics help successfully,
but finds no supplied native-memory CLI configuration or documented snapshot
recipe. The native heap UI path remains unverified and needs a supported
interactive session; no profiler session or IDE is launched. Direct3D 11's
documented device-loss test forces an adapter TDR, affecting other Direct3D apps;
no scoped device-removal hook is established for NetMap's D3D11 device. Actual
device-loss acceptance remains deferred, not replaced by a fake HRESULT test.
See [remaining platform proof](../verification/stage2.md#deferred-unattended-tooling-boundaries).
The remaining acceptance depends on the deferred allocation/collector work and
attended Windows behavior, with scoped ordinary measurement gaps retained.
The approximately 184 versus 144–145 MiB isolated map-contribution objective is
still unmet. Both stages remain incomplete; no UAC, installation or attended
step has run. Continue from the deferred checklist, not another unchanged map
or source sweep. The built installer/ZIP is in
`artifacts/packages/20261024-073148-008-8c5a63d2/`; it was inspected, not installed.

Historical unattended checkpoint (2026-10-24): goal blocked pending the user's return or
an authorized environment change. The same deferred elevation/attended
dependencies remain across the resource/timing, profiler-boundary and current
checkpoint turns. No owned app, profiler or build remains running. Existing
ordinary-token evidence still reports collector access denied (error 5), and
the lower-map-memory objective is unmet. Resume with the consolidated
[deferred checklist](../verification/stage2.md#deferred-until-the-user-returns),
starting with allocation attribution and the real elevated collector/broker
journey. Preserve all failed source cases and scoped evidence; neither stage
is accepted and no requirement is waived. Do not restart unchanged matrices,
repeat integrated review or manufacture substitute passes while these
dependencies are unchanged.

The preceding unattended lifetime batch passes on preserved
optimized test 61160460 in `V:/temp/netmap-lifetime-20261024-d/`. Actual Desktop
disable-before-delivery and repeated recovery coalescing pass at 128x, using real
discovery, preference saves and WorkerW parenting. Accepted pending-save Host
close passes for normal commit and real five-second fallback; last committed
preferences survive the latter byte for byte. The affected Settings Reopen case
also passes. See [Desktop evidence](../verification/stage2.md#desktop-disable-and-delayed-recovery)
and [save-exit evidence](../verification/stage2.md#pending-save-host-exit-and-fallback).
One combined optimized build takes 73 seconds; formatting, all-targets Release
Clippy with the existing dead-code exception and 106 ordinary tests pass, with
20 opt-ins ignored. Focused lifetime review is clean. Product/package D779778C
is unchanged; no owned process/build remains and no UAC or attended step ran.

The preceding visual-family/pending-save batch completes
in `V:/temp/netmap-visual-settings-20261024-c/` on preserved optimized test
D124A0C6. Twelve source/native 1080p pairs complete without case failures;
root inspected all twenty-four full-target images. Geography, outlines, fills
and destination/highlight placement agree in these settled views, with the
recorded establishment-guide/text differences. Actual pending-save/modal
close/reopen passes, including the newer dialog surviving the older OK intent
and actual committed disk preferences. See [visual evidence](../verification/matched-map.md#matched-visual-families-2026-10-24)
and [pending-save evidence](../verification/stage2.md#settings-close-and-reopen-during-an-actual-pending-save).
Formatting, all-targets Release Clippy with the existing dead-code exception
and 106 ordinary tests pass, with 17 opt-ins ignored. Both focused reviews are
clean. Product/package D779778C is unchanged. No owned process/build remains;
no UAC, installation, registry change or attended input was used.

The preceding local-lookup/text/Preview batch remains preserved on F47D775F in
`V:/temp/netmap-acceptance-20261024-b/`; its evidence retains that binary's scope.

Completed lifetime batch ownership: `connection_exploration` delivered the
private Desktop driver/gate/wrapper; `desktop_native` extended the retained
Settings driver/gate/wrapper and existing shutdown observation helpers. Lead
owned shared registrations/hooks, documentation, build and runs. One combined
build and focused review sufficed; no correction rebuild or failed retry. The
roughly twenty-minute batch stayed within the 20–30 minute estimate. Native
Desktop takes 3.52 seconds; the commit case takes 0.51 seconds; the fallback
reaches its existing five-second deadline. These remain pre-storage save and
controlled recovery-message proofs. Actual atomic-commit interruption, Explorer
restart/topology, physical input/DPI and whole-product resources retain their
separate scope. UAC/attended acceptance is deferred; memory acceptance is unmet.

The current completion-evidence audit is finished at
`V:/temp/netmap-integrated-review/current-completion-audit.md`. It accounts for
both specifications, named journeys and all twenty optimization rows without
repeating the completed product review. Its four ordinary-token priorities now
have the scoped results linked above. The lower-memory objective, full matched
larger scenes, complete product/collector totals, physical input and actual
Windows setting delivery remain incomplete. Lead remains the sole builder and
project-document editor.

Completed batch ownership: `map_resource_checks` kept coupled navigation,
temporal rendering and capture corrections together; `desktop_native` owned the
dataset lifetime/availability driver; `hosting_settings` owned provider diagnosis
and the native Categories-label correction; `connection_exploration` owned scoped
component accounting. Lead owned shared seams, builds, runs and integration.
Independent reviews overlapped implementation; correction reviews stayed within
their affected contracts. Two coordinated Rust build rounds were necessary:
native observations exposed a stale capture buffer and a provider association
that a source-only review could not establish. No product architecture or
framework was added. The roughly forty-minute map/dataset integration round
stayed near its revised 35–45 minute estimate; the provider diagnosis exceeded
its initial 10–15 minute standalone estimate and ran alongside that work.

The next useful memory diagnostic is settled: installed WPR VirtualAllocation
with WPA Commit Stack analysis, recorded before first native Device creation.
Root confirms the installed profile list; Microsoft's recording workflow needs
administrator access, so recording is deferred with UAC. No tracing/profiler,
registry/service or installation action was attempted. D124's emitted PDB is
preserved before subsequent builds; Release symbol quality remains unverified.
See [allocation-attribution next step](../verification/matched-map.md#next-allocation-attribution-step).
Do not rerun unchanged failed source arms or infer a cause from coarse counters.

Real shared MMDB loading, two production local origin lookups and Scene work
complete across a genuine map-HWND gap; current version/rows, fitted/nonfitted
view invariants and exact accepted-model Present all pass. The native test exits
0; its wrapper initially rejects Rust's prefixed output. The corrected parser's
exact validation passes against the preserved logs/dataset, without a native
rerun or rewriting the original Passed=false record. The affected existing
Scene/target-gap case also passes. Late origin replies are tested after Ready;
pending id/version guard discrimination remains the pure tests' scope.

Settings' actual Preview command now passes with the packaged SCR and with the
adjacent file absent. The owned saver covers both connected displays and exits
0 on its owned WM_CLOSE; missing-file error delivery leaves Settings usable and
preferences unchanged. An initial harness null-value failure is preserved;
guarded process enumeration and full failure diagnostics precede the passing
run. No Windows saver selection or installation is performed.

The paired blank-presentation/text diagnostic measures 56.52/60.51 MiB private
memory: the actual default DirectWrite overlay adds 3.99 MiB. Its small startup
increment does not explain the earlier approximately 40 MiB map-contribution
shortfall. Geometry/effects/runtime retention remain combined; resource
acceptance stays open. See [text diagnostic](../verification/matched-map.md#production-text-startup-diagnostic-2026-10-24)
and [local/Preview evidence](../verification/stage2.md#local-mmdb-and-origin-results-across-target-replacement).
The updated rust-win32-ui guidance remains mapped onto the established owners.

Completed batch ownership: `map_resource_checks` delivered the existing native
fixture/wrapper and disposable reference capture helpers/wrapper; `desktop_native`
delivered the private Host/Work pending-save fixture and wrapper. Lead owned
shared registrations, integration, documentation, all builds and native runs.
One reference build (91.6 seconds) and one optimized Rust test build (74 seconds)
were sufficient; no correction rebuild or unchanged failed matrix rerun. The
roughly 20-minute batch remained within its 20–30 minute estimate. Native checks
take 23.63 seconds for the twelve images and 0.85 seconds for pending Settings.
Source preparation/readback bounds remain twenty seconds per case. The static
full-turn pairs normalize to the same viewport; they do not prove a gesture.
Physical/system High Contrast, motion, input and resource acceptance remain open.
On 2026-10-24 the user confirmed using the reread updated
rust-win32-ui skill: apply its ownership, native-control, responsiveness and
proportional-verification guidance to existing owners, without architectural
changes merely to match its reference module names or a prototype.

Previous completed batch on 727C2E3E is preserved in
`V:/temp/netmap-discriminating-20261024-a/`: the memory trio and native shutdown
results below retain that binary's scope.

The fresh native control/presentation/full-map trio uses identical imported
active-100/4K facts and 30/40/50-second endpoints. Mean private memory is
2.21/56.76/186.37 MiB. Blank production device/presentation first use adds
54.55 MiB; the full-map residual above it is 129.62 MiB. This is one diagnostic
trio, not independent repetition or allocation-ownership proof. World, geometry,
effects, text and differing frame behavior remain combined in that residual.
See [presentation diagnostic](../verification/matched-map.md#device-and-presentation-diagnostic-2026-10-24).
No renderer redesign is justified yet. The earlier never-map comparison remains
terminal: three native pairs and two reference pairs complete, with reference
treatment 3 failing before sampling. Its unmet private-memory objective and
scope limits remain recorded; do not repeat that unchanged source failure.

The actual Host WM_CLOSE path now passes with a genuinely outstanding WinHTTP
body read: window removal 16 ms, Request cancellation and callback reclamation,
cooperative Work finalization 73 ms. The separate cancellation-ignoring gate
reaches the existing five-second fallback (4.712 ms deadline lateness); process
exit/removal observations are recorded separately. All jobs use Work. The
private plain-loopback setup does not prove HTTPS or dataset commit integrity.
See [shutdown evidence](../verification/stage2.md#close-during-pending-io-and-uncooperative-work).

Completed unattended batch: `map_resource_checks` exclusively owns the resource
fixture/wrapper and adds a `text` diagnostic using the actual production speed
overlay above the blank Device baseline. Use existing Map preparation with no
World or arranged geometry; preserve the prior trio and keep startup attribution
separate from allocation ownership. `connection_exploration` exclusively owns
`app/native_checks.rs`, `app/work/native_checks.rs`, `host/native_lifetime.rs`
and the native-host wrapper. Extend the existing map-target-gap journey to real
local MMDB loading/lookup, versioned availability/origin/Scene acceptance and
replacement presentation, using the retained owned city database. All jobs use
Work; no public discovery, repeated download or product test mode is needed.
Shared contracts, documentation, builds and native runs remain with lead.
Independently, `hosting_settings` exclusively owns `tests/native-settings.ps1`
for a focused opt-in Screen saver Preview journey, bypassing the completed
general Settings loops. Use fresh owned copies of the frozen packaged EXE/SCR
and a missing-SCR case. Invoke only Preview through the real native page/Work
command; identify the exact copied SCR process and close only its owned HWNDs.
Verify async success/failure, retained Settings usability and unchanged saved
preferences. No global input, installation, selection or registry writes.
The batch required two optimized builds: focused review strengthened exact
model observation and prevented accidental public discovery in the local-only
fixture. Script-only corrections addressed prefixed Rust output and nullable
process enumeration. Preserve all failures; do not repeat the successful native
lookup or completed resource matrices merely to obtain another green wrapper.
Keep UAC and attended acceptance deferred. Next substantial unattended work is
the missing matched visual families and remaining memory/component attribution,
or pending-save/file-integrity integration; existing local load/lookup is not
an actual city installation across the gap.

The existing blocked dataset-replacement/restart check also passes on preserved
727C2E3E in `V:/temp/netmap-native-integrity-20261024-a/`. It uses the retained
126,998,165-byte city MMDB, preserves its exact hash, checks old reader/version
and backup recovery after replacement failure, then successful reopen/retry.
The isolated temporary directory is empty afterward. This checks file recovery,
not interruption of an install during native Host shutdown. Older entries below
retain batch history.

Implementation is authorized through both planned stages. On 2026-10-23 the user
explicitly authorized starting Stage 2 while Stage 1 acceptance remains open.
This supersedes the strict implementation sequence, not either stage's acceptance
criteria. Stop repeating unchanged failed reference runs; retain their evidence.
The user will be away: defer every UAC prompt and user-attended check until the
final acceptance checklist. Continue implementation and unattended checks; do
not treat deferred checks as passes or launch another elevation meanwhile.

Current handoff (2026-10-24): both stages' source is integrated. The final
six-specialist review's seven consolidated findings are fixed, with a clean
focused correction delta. Optimized tests, native host transitions, two-target
renderer pixels/lifetime, Settings and installer compilation pass. The current
100-observation resource journey completes all 36 phases and GPU timing batches
without explicit timing gaps; neither stage's complete acceptance is
claimed. [Stage 2 evidence](../verification/stage2.md) records the current binary,
package, checks and remaining work. The batch assignments and earlier build/run
entries below retain investigation history; do not restart completed batches
or unchanged failed reference runs from those entries.

Current unattended acceptance batch: `connection_exploration` owns only
`tests/native-exploration.ps1` (real loopback rows/selection/retention), and
`hosting_settings` owns only `tests/native-settings.ps1` (optional Stage 2 page
journey through the existing harness). Lead owns documentation and runs both
against frozen Release `2C0E2568809BC2DA91DA33E5E612BEB58DB1709983AA971DF6F0364EB55564C5`.
No UAC, topology change, installation or Windows screen saver selection is part
of this batch. The first exploration attempt has a failed UI Automation pattern
precondition; preserve its evidence and inspect native accessibility before
changing the harness. Desktop freshness's focused regression/review and 100
native transitions pass; see Stage 2 evidence. Package the final binary after
the acceptance batch, preserving earlier packages and failed run evidence.
Hosting Settings journey now passes on that frozen binary (selected display,
Cancel/Apply/reset, retained HWND/owner, Screen saver status/actions without
invocation). Exploration proves cell text and live selection through fade but
its subsequent live-grid scan raced row removal. Its harness is being corrected
as one read/selection batch. The diagnostic also found the stock ListView proxy
using zoom text as its accessible name; lead's native annotation fix uses the
existing accessibility owner and has a clean focused review. Final Release
`4C2D3A371FA18A6A12F7288BC7F0E329992DF546F924F45B0FF5AFD3E237295D`
verifies that exact stock-provider name, live selection through fade and clearing
on closure. The native socket driver then receives Windows error 10048 on
same-tuple reconnect; ended/reconnect acceptance remains open. Preserve the failed
run, without repeating unchanged socket-reuse attempts. Formatting, explicit x64
all-targets Release Clippy and installer compilation pass; current package and
scoped native results are recorded in Stage 2 evidence. No UAC or attended check
was launched.
Resumption: current product source and package are frozen. Do not repeat the
completed Settings/100-cycle/resource matrices to re-establish their stated
scope. Remaining unattended integration work is identified in
`V:/temp/netmap-integrated-review/acceptance-gaps.md`: deterministic native
ended/group selection where actual tuple reuse is unavailable, concurrent saver
foreground suspension, and application replies crossing presentation gaps.
Full resource/visual acceptance and the deferred UAC/attended checklist remain
open. The latest exploration script's final change preserves partial evidence;
both script parsers pass, without another native run.

Completed native integration batch (2026-10-24): lead extracts the existing
typed reply acceptance from queue polling into `Application::accept_reply`,
without changing reply behavior. `connection_exploration` exclusively owns new
test-only `app/native_checks.rs` and `host/native_exploration.rs`; lead registers
their modules and one scoped test hook after normal Host/control creation. The
ignored fixture uses accepted Connections/Scene replies, Model/Rows, actual map
hits and native in-process ListView notifications for controlled geography,
ended identity, pending lookup and expiry. It supplies no product fixture mode,
alternate routing/grouping or ad hoc worker. Root alone builds/runs with a bounded
owned-process watchdog. `desktop_native` exclusively owns new test-only
`host/native_foreground.rs` and `tests/native-host-integration.ps1`, reusing that
application fixture for the real `/s` child, actual foreground notifications,
normal and modal-Settings drawing gates. Lead owns optional test-only actual
Present-attempt and successful-frame observations in Map/Renderer/Device; they
are absent from the product binary. Queue delivery across a target gap remains
a separate acceptance item. Lead also removes redundant graphics-constructor
objects that existed only to prove APIs already used by Outlines/Routes. This
removes unused startup work; no measured memory saving is inferred from it.
The focused native-integration review is clean after its observer correction.
Formatting/Clippy pass, optimized suite `10664` passes 106 tests with 12 opt-ins
ignored, and all three selected native checks pass in
`V:/temp/netmap-native-host-integration-20261024-final/`. This covers controlled
geographic/row identity and both split orientations, actual `/s` foreground
suspension/resume in normal and Settings-modal operation, and retained renderer
pixels/two-target lifetime. Final package product hash is
`D779778C0C1AEE9C9D57D518516158B203255DD5BAB68B11087BDA9F61301880`;
the test binary is `C7305C850126DCAE2C64935BB6A1FCE588CA88AE27DFF13BE9C542E5B1351FFB`.
No UAC, installation or attended input was used. See Stage 2 evidence for scope.

Resumption after that deliverable: source and current package are frozen, with
no pending build/native process. The next resource measurement must change its
control: `V:/temp/netmap-integrated-review/map-contribution.md` establishes that
all 18 current visible/hidden private deltas are larger than the reference, but
the warmed baselines and different geography/model lifetimes prevent whole-map
attribution. Root independently verifies those pairs. Do not repeat the unchanged
matrix or infer map savings from lower total process memory.
`V:/temp/netmap-integrated-review/cold-control-seams.md` locates the exact early
reference-capture and native pre-World branches for a fresh never-map control
versus production-map treatment using the existing exported active-100/4K facts.
That changed measurement is prepared only, not implemented/run. Keep the sibling
TinyTorrent2 untouched; any reference fixture edit belongs to the disposable
copy. Remaining endpoint/target-gap and genuinely pending operation checks also
stay open, with UAC/attended acceptance deferred until the user returns.

Current resource batch (2026-10-24): `map_resource_checks` owns only
`src/map/renderer/resource_checks.rs`, `tests/native-map-resources.ps1` and the
disposable reference capture files identified in `cold-control-seams.md`.
Lead owns integration, documentation and all builds/native runs. Settled
measurement contract: fresh never-map and production-map processes import the
same accepted `active-100` facts and establish the same common host per app;
sample at 30, 40 and 50 seconds after that common setup. Treatment must be ready
before the first checkpoint or the run is incomplete. Record actual checkpoint
lateness, UTC, process age, common-setup age and treatment-ready age. No forced
GC, working-set trimming, screenshots, navigation or GPU-query setup before
primary samples. Country MMDB/runtime startup remains common in the reference;
world/model/renderer preparation belongs to treatment. Preserve source and binary
provenance before changing the disposable fixture. First build/run one pair per
application; if the comparison is valid, obtain three independent pairs, with
alternating launch order and no invented numerical acceptance threshold.
Expected work is one implementation/integration round plus focused fixes if
needed, roughly 20–40 minutes including bounded native sampling. This is a
fixture-only change; retain the frozen product/package and previous evidence.

Independent lifetime fixture batch: `connection_exploration` owns only new
test-only `host/native_lifetime.rs`, `app/work/native_checks.rs` and the existing
`app/native_checks.rs` helper. Lead registers their private modules and scoped
Host hook, with a cfgtest observation at the existing Scene acceptance decision.
The fixture keeps the real application Endpoint alive, completes a registered
Work job while the owned map target is absent, then creates a replacement target.
It must prove current/stale Scene delivery counts, retained model/rows/view/clock
and a successful replacement Present. Application remains the sole reply owner;
controlled worker delivery does not establish genuine pending I/O cancellation.
Lead batches its build with the resource fixture and runs only affected opt-ins.
Both focused reviews are clean after removing per-frame diagnostic sampling
from cold rendering. Reference Release capture build and Rust formatting,
Clippy and optimized test compilation pass. Native lifetime/affected exploration
and actual normal/modal saver checks pass on preserved test binary
`5306F59B6DAC26F6EE30597E42CA332894A5411CDD5CAC7FA372E3B72A9B010A`;
evidence is `V:/temp/netmap-cold-contribution-20261024-a/native-lifetime/`.
The frozen product/package remains D779778C. The changed cold comparison is
terminal with three native pairs, two reference pairs and one failed reference
treatment; the fresh comparison above records its unmet private-memory objective.
Reference prechange/current binaries are preserved beside their source and build
provenance. The first sandboxed reference restore failed with NU1301; the
ordinary-token build outside that tool sandbox succeeds. No UAC was used.

Current Stage 2 batch: lead owns startup, Application/Work contracts, Host integration
and all builds. Desktop/tray native operations and exploration native composition
are independent modules with exclusive editing owners. Collection continues to
have one owner; the collector-role seam must reuse it. Keep source snapshots and
map grouping authoritative, and preserve application-lived replies across hosts.
The pending elevated check is now terminal (session 44898, helper exit 1):
`V:/temp/netmap-collector-resources-20261023/result.json` contains measured resource
samples but reports test exit 101. Diagnose the failed assertion before claiming
collection acceptance; no repeated elevation is pending.
Current implementation ownership is split into independent batches; the lead
alone builds and runs native checks against stable inputs. Shared contracts change
only through the lead. Architectural prerequisites settle before dependent work.
At each completed feature batch or integration milestone, one independent
adversarial reviewer challenges its requirements, failure paths, integration,
ownership and material complexity.
Compatible fixes return to the same reviewer within affected scope; widen only
for concrete evidence. Independent implementation may continue while reviewed
inputs stay stable. Reserve `code-review deep` and its six specialists for the
whole integrated plan, then assign fixes to existing owners and rerun affected
checks. Retain concise review outcomes and unresolved risks here.

| Batch | Exclusive editing owner and files | Dependency / completion evidence |
| --- | --- | --- |
| Native collection correctness | Retained `etw_reference` agent: `src/connections.rs`, `src/connections/`, `src/app/work/trace_checks.rs`, `tests/native-trace.ps1` | Keep Snapshot/Publication and worker-launch contracts unchanged unless agreed with lead. Resolve System-context receive evidence without guessed process ownership; exact bounded native traffic and lasting identity/conservation checks. |
| Matched reference fixture | Retained `reference_baseline` agent: disposable `V:/temp/netmap-reference-20261023-a71c/` only | Repair the loaded-scene/fixed-extent fixture and provide one build/run handoff. No edits to TinyTorrent2, no operation of its running engine, no builds or native runs. |
| Matched NetMap resource checks | Fresh `map_resource_checks` agent: `src/map/renderer/resource_checks.rs` and `tests/native-map-resources.ps1` only | Match the reference scene facts, render extents and phases through the production renderer; provide test-only checks and one build/run handoff. Lead registers the test module and alone builds/runs it. No production behavior or dependency changes. |
| Stage 1 adversarial review | Fresh `stage1_review` agent: read-only product inspection; `V:/temp/netmap-stage1-review/review.md` only | Review retained Stage 1 requirements, execution paths, integration, ownership and material complexity against stable product source. Separate defects from missing acceptance evidence; compatible fixes return to this reviewer. |
| Application and native presentation integration | Lead: Application/Work/Host, persistence, resources, project docs and packaging scripts | Own shared interfaces, Release placement correction, integration, sole builds and all native execution. |

The collection owner must not edit application/hosting contracts, and the lead
must not edit collection-owned files while that batch is active. Reference work
has no product-source dependency. Reuse each agent for related follow-ups.

The completed Stage 1 integration review and focused correction delta are at
`V:/temp/netmap-stage1-review/review.md`. Confirmed correction work includes
per-kind destination markers and wide selection rings, capacity for a final
placement write during simultaneous city/settings work, direct native detail
and origin commands, and drawing suspension for inactive display/session.
The completed review also identified transient hover on origin
changes, selected-route rebuild priority and the missing transferring count.
The lead has integrated these seven corrections, with durable marker/count,
worker-admission, origin-hover and presentation-state regressions. The same
reviewer has cleared the affected source contracts. The integrated Release suite
passes 93 tests, with ten native/network opt-ins; the native renderer confirms
three offset directional marker colors and High Contrast ink. Current Release
Settings (including real keyboard input and Origin routing), five-launch exact
placement and foreground navigation/detail-menu journeys pass. See
[correction evidence](../verification/stage1-corrections.md) for scope and paths.
Full stage acceptance remains pending. Matched resource checks are registered test-only
work and do not add a product fixture mode.
The correction review caught and removed a fatal startup dependency on optional
session notifications. Microsoft also documents that the retained flip-model
swap chain cannot return `DXGI_STATUS_OCCLUDED`; native session/display/suspend
facts own drawing availability. This platform applicability correction removes
the unused status-test mechanism, with notification failures shown as monitoring
limitations while the map remains usable.

The repaired reference fixture's Release build passed. Its first 100-peer run
at `V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-b9ad491f-e5a2-4333-bfd3-adee277c4f59/`
failed its idle-route assertion during a restore callback before the new renderer
was prepared. The readiness correction was rebuilt. The next run at
`V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-bbddd442-2be1-40e0-9d6b-528c96fd30c1/`
completed 26 fitted memory/GPU phases with zero GPU-counter errors, including
loaded 3840x2160 active scenes, before the source's Europe-zoom settling check
timed out. The partial evidence is preserved. The subsequent run at
`V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-517a2496-8ee7-4db3-9009-6c71fa8bb01f/`
completed all 36 fitted memory/GPU phases for 100-connection idle, active and
mixed scenes at 1920x1080 and 3840x2160, with zero counter errors. Two source
4K navigation journeys still timed out with pending geometry rebuilds; they
remain failed, independently of the completed fitted matrix.
The XAML image capture was ancestor-clipped. A separate images-only run at
`V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-52b0c8b2-e13d-4f20-88d9-7e467a61c533/`
passed six full-target readbacks through the unchanged prepared renderer and
its existing device/palette. The inspected 4K active image contains the full
world. These images exclude XAML overlays and physical 4K presentation, and
their allocations occur outside the resource measurements.

The first matched native run at
`V:/temp/netmap-matched-dark-100-20261023/` failed the active 4K Europe-4x warmup
after 26 fitted phases. Shared outline deferrals were invalidating route glow
and repeatedly spending the preparation allowance before remaining rebuilds
could finish. The retained renderer now invalidates glow only when one of its
own paths or halo realizations changes, while preserving the existing view,
model, origin, palette and DPI invalidations. The same independent reviewer
cleared this focused correction. The corrected run at
`V:/temp/netmap-matched-dark-100-glowfix-20261023/` completed all 36 memory/GPU
phases and six image readbacks; active 4K Europe zooms through 128x now settle
under the unchanged deadline. The updated Release build, 93 ordinary tests,
native renderer check and Clippy with the existing dead-code exception pass.
The measured process totals are lower, but isolated map-private reduction and
repeatability remain unproven because resource retention distorts subtraction.
See [matched measurements](../verification/matched-map.md) for actual scopes,
values, image inspection and the remaining acceptance gaps.
The combined 100/200/1000-observation Dark reference batch at
`V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-81a06b2d-6402-495a-a8ee-f4d8e1597761/`
exited with failure after 356.9 seconds. It completed 60 memory/GPU phases
(all 54 1080p phases and six idle-100 4K phases), then the source's fitted
active-100 4K scene remained rebuild-pending for 610 prepared callbacks and hit
the unchanged 20-second settling deadline. This is a source fitted-scene
failure, in addition to its earlier high-zoom failures. Preserve the partial
results; do not accept them as a completed full matrix. Remaining counts will
use the existing per-count fresh-process entry to avoid accumulating all nine
scenes in one process; source renderer and deadlines remain unchanged.
Session `66705` is terminal. No builds run during resource sampling.
The fresh-process 200-observation source run at
`V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-eefc98c8-9b76-468f-a3e5-89f20b6b1205/`
also exited with failure, after 315.2 seconds and 32 fitted phases. Its final
mixed-200 4K navigation recovery could not settle at Fit (476 prepared callbacks,
rebuild-pending at 20 seconds). Three failed navigation/recovery records remain
in its captures. Session `93649` is terminal. The incomplete source run is not
an eligible complete matched baseline for the native wrapper.

The native GPU timing milestone is implemented in test-only Device hooks and
the existing resource fixture. One focused independent review is clean; Release
compilation, formatting, Clippy with the existing dead-code exception and 93
ordinary tests pass. The GPU-only native validation at
`V:/temp/netmap-main-gpu-dark-100-20261023/` completed all 54 batches, each with
valid samples; 27 also have pending samples, explicitly excluded from quantiles.
Main D2D drawing is bracketed before
Present, excluding preparation and scanout. Raw GetData preserves S_FALSE;
bounded one-read queries report pending/disjoint/invalid/error gaps explicitly.
Queries run after all memory phases; `-GpuOnly` validates this instrumentation
without repeating those phases or claiming a resource comparison. Product
Release paths are unaffected by these cfg(test) additions.

The user authorized Stage 2 implementation while these acceptance items remain
open. Native timing results
and their scope/coverage limitations are in the matched measurements record.
No production reference renderer is changed and no timed-out navigation is
accepted as passing parity. Full matched acceptance remains open.

The existing 1000-observation Dark reference fixture at
`V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-a4766e62-f515-41ae-bcc4-b60fac2449f1/`
failed after 135.3 seconds and 24 fitted phases. Its active-1000 fitted 4K
scene retained 146 destinations but remained rebuild-pending for 579 prepared
callbacks through the unchanged 20-second settling deadline. Session `45661`
is terminal; the original source is unchanged and no retry is currently running.
GPU-only session `88261`, test build `76686` and the other failed source sessions
are also terminal. The GPU pass collected
1,742 valid intervals and 58 pending intervals, with no other query failures.
Stage 2 implementation is now authorized; neither stage is accepted yet.

The retained collection owner has extended its existing bounded local-only
native Collector check with before/during/cleanup
process resource samples and actual owned trace buffer statistics. The scope
excludes UI/MMDB/GPU and the short child; observed publication maxima are not
claimed as isolated allocation peaks. There is no new collector, workload or
production publication interface. The existing resource fixture now uses the
same official Windows PSAPI record rather than a handwritten memory ABI.
The ProcessStatus SDK feature is a test-only dev dependency. Both handoffs are
frozen and one focused independent review is clean. Release build `55117`,
Clippy `22791` with the existing dead-code exception, formatting and 93 ordinary
tests pass. The optimized test executable is
`target/x86_64-pc-windows-msvc/release/deps/netmap-91ac044f5449248b.exe`, SHA-256
`9936EE5BA3A79DEBE7562B1769920F49E5420D9E2534490932CC305F550718B4`.
Formatting-only changes followed compilation; no source behavior changed.

The previously authorized elevated local-only Collector resource check completed
in session `44898` (helper 21920 exit 1, test 24152 exit 101), without timeout.
Its result at `V:/temp/netmap-collector-resources-20261023/result.json` records
1,511,424 bytes sampled private increase and 2,482,176 bytes working-set increase,
with sixteen 16 KiB trace buffers. These are sampled process deltas, not isolated
collector allocation peaks or a successful collection acceptance check. Delayed
completed-TCP events exposed lifecycle ordering that leaves controlled bytes
unattributed; the collection owner is fixing that retained behavior. All new UAC
and user-attended checks are deferred until the final checklist as requested.

Stage 2 first integration batch is in progress: native passive desktop/tray
operations, shared-facts multi-display saver hosting and virtual connection
exploration. Lead owns startup, Application/Work/Host and map target integration;
`desktop_native` owns desktop/tray plus .scr packaging; `screensaver_host` owns
`src/host/screensaver.rs`; `connection_exploration` owns `src/app/exploration.rs`
and `src/app/work/scene.rs`; `etw_reference` retains Connections and the local
read-only broker transport. Lead alone builds/runs stable inputs. No Stage 2
runtime acceptance is claimed from source implementation.

The first Stage 2 optimized test suite (session `85413`) passes 100 tests, with
ten native/network opt-ins ignored. It includes the delayed completed-TCP,
transport codec, launch and stable exploration identity regressions. SDK binding
mismatches from the first check were corrected. Product Release build `15892`
passed. Its 1,380,352-byte executable, SHA-256
`73FC172EF92BCFAF49365429BDDBD0CB357DD469895F6E9A80A6757A89109F5C`,
passes unattended native normal map/table commands with 225 ordinary-token rows,
standalone `/c` with five categories and no map, and `/p` parent resize/teardown,
all with clean exits. Evidence: `V:/temp/netmap-stage2-fourth-20261023/result.json`.
Three preceding harness runs failed because the owned target was launched hidden
and its first ShowWindow obeyed STARTUPINFO; the corrected harness explicitly
shows its owned presentation. Preserve those failures; they are not product
visibility regressions. The focused milestone
review is at `V:/temp/netmap-stage2-review/review.md`; review fixes address
standalone `/c` common controls, current rows before location, pending membership,
Escape, filter-only refresh and preserving hover while reconciling row place.
See [Stage 2 evidence and deferred checklist](../verification/stage2.md).

The next integrated batch adds the explicit `--collector` role, same-logon
read-only broker subscription and local fallback, source-epoch reset, bounded
validated process-name caching, normal-host saver foreground suspension and
worker-only monitor/Windows selection facts. Only the user's capture command
requests elevation; startup, saver and preview never do. Broker lifetime follows
remaining subscribers, with bounded initial/reconnect grace and existing cleanup
fallback; it reads canonical committed retention preferences. The independent
review at `V:/temp/netmap-stage2-review/collection-integration.md` found a native
modal-loop delivery defect; it is corrected by posting foreground changes to the
live owned window. No unresolved material source finding remains in that batch.
Coordinator check `10058` and optimized suite `73582` pass: 101 tests, ten native
opt-ins ignored. Release build `84039` passes (1,469,440 bytes, SHA-256
`DCF3497584EB3961FC3C382ACACADAE868D67E2D7FAB4CDD3E16508AD980549B`). The integrated
native smoke passes with 241 ordinary-token rows, standalone configuration and
preview teardown. Elevated transport/runtime acceptance remains deferred.
The desktop harness's unattended 100-cycle path stopped before its first cycle:
the tool sandbox rejected tray IPC, and a normal-token run outside that sandbox
then reached an invalid-handle failure. Both preserved the usable normal map.
The following batch supplies native operation context and split-gutter dragging;
Release `83070` then passes native structural checks: 100 desktop/window cycles
(50 fitted, 50 at 128x), same map HWND, splitter capture/Escape, `/c`, `/p` and
two-display `/s` coverage/owned key dismissal. Actual visual/icon/DPI and settled
resource acceptance remain open. Final six-pass source review produced seven
consolidated findings, all corrected in one batch with a clean six-pass focused
delta. Optimized suite `40488` passes 105 tests (ten native opt-ins ignored), and
Clippy all-targets passes. Release `16594` is 1,539,584 bytes, SHA-256
`E02868EBF9937F69730BCDDAEB3A89FBE204249A908F7252AFC18F252F9A735D`; the full native
hosting journey repeats successfully, as does the Settings draft/search/save/
failure/reflow/restart journey. Two-target native pixels/lifetime and final
packaging pass. The updated matched-resource journey (`49903`) also completes;
its scope remains deterministic map-only testing, not full product-memory or
physical presentation acceptance. Details and evidence paths are in
[Stage 2 integration evidence](../verification/stage2.md).
Reread and adopted the updated Rust Win32 UI skill on user request;
existing project ownership remains authoritative.

The native graphics/collection proof in Development step 1.1 remains open while
independent map/navigation and location work progresses through steps 1.2–1.4.
The lead owns implementation, integration and all builds.
Reference-baseline investigation is read-only delegated work; it may inspect
existing captures/binaries and their launch contracts, but does not edit or build
TinyTorrent2, operate its running instance or use its normal data profile.
ETW source investigation may prepare a disposable native probe under
`V:/temp/netmap-etw-proof/`; the lead alone builds/runs it. It is proof tooling,
not a second application collector, and must leave only its own trace resources
and preserve its result/schema evidence when retired.

Step 1.1 completes only after recording the matched source baseline, a working
native GPU window with DPI/resize/close behavior, and observed TCP/UDP IPv4/IPv6
events, attribution, measured bytes, privilege behavior and owned-session cleanup.
The map port may proceed independently of a collection limitation, but dependent
live behavior must wait for its source proof. Desktop hosting remains stage 2.
Current evidence: [bootstrap checks](../verification/scaffold.md) and the
native hardware GPU window, off-UI geography load and viewport/decoder tests in
[native source evidence](../verification/native-source.md). Full-map memory
acceptance and mixed-DPI checks remain unverified.
[Native source proof](../verification/native-source.md) confirms the elevated
loopback ETW source and an ordinary-token access denial; the remaining source
gates and matched loaded/4K reference baseline are not yet complete. An isolated
empty-map reference has drawn successfully; it is not a loaded-scene comparison.
The geography/viewport port and optimized country outlines are present, including
shared borders, bounded pieces, cropped fill/realization caches and the shared
rebuild allowance. A native capture shows the world; the latest ordinary suite
passes 86 tests with nine explicit native/network/storage checks ignored by default.
Basic navigation now includes anchored wheel/double-click zoom, mouse pan,
capture/cancel handling, keyboard zoom/pan/Fit and bottom-centre native controls.
Live TCP destinations and guides now render through the ported grouping model.
Basic hover/selection, arrow inspection/reveal, inspected-place zoom, descriptions
and a native tooltip are implemented. The map queries Windows contrast/motion
settings; High Contrast uses system background/text colours and solid ink, while
reduced motion uses static marks and timed removal. Native renderer fixtures
verify mixed idle/active sprites, normal pulse scheduling, static deadlines,
expired selected hit rejection and a temporarily undrawable view. The live
inspection journey verifies posted key-message routing, native description text,
zoom and Escape clearing. Source Light/Dark palettes, native Windows accent
colors, country activity shading, retained selection-ring realizations and the
saved speed corner are implemented. The shared native renderer fixture verifies
Light/Dark background readback, active/idle country shading and highlight expiry;
owned app captures show all three speed positions and a compact fitted window.
The retained display review is clean at
`V:/temp/netmap-map-display-review/review.md`. Physical input, tooltip display,
screen-reader use and matched visual/resource parity remain acceptance work; see
[map interaction evidence](../verification/map-interaction.md). Full ETW collection
also remains unfinished. Unused
ported records await their remaining consumers, so warning-free Clippy is not yet
an accepted gate.

The native HWND accessibility proxy now exposes a stable map name/ID, pane role,
keyboard focusability and the current inspection/coverage HelpText. An independent
UIA client verified selected and cleared states; this does not establish screen-
reader or physical-input acceptance. Normal-window placement now loads off-UI
before first show and saves independently of Settings. Its native restart journey
passed normal/maximized bounds, minimized-close restoration and off-screen
recovery; see [window evidence](../verification/window.md). Formal review of this
retained implementation is deferred to the substantial deliverable boundary.
The first Release placement run exposed zero native flags on minimized close;
the lead's WM_SIZE-based restoration correction passed the integrated Release
five-launch journey with exact requested bounds. Native foreground navigation
also passed on the live map. The latest ordinary suite passes 87 tests; nine
native checks remain explicitly opt-in. Unmatched System TCP traffic is now
reported per interval without guessed ownership or loss of captured totals.
The integrated local-only Collector check passed. Matched resource and remaining
platform acceptance still await evidence; earlier narrow passes do not establish
the complete stage.

Bounded native checks for this milestone answer whether the specified ETW source
can observe controlled loopback/external traffic under the current rights, and
whether the required Direct2D/D3D/DXGI mechanisms work on this Windows build.
Connections owns the source check and Renderer owns the graphics check. Stop each
check once its required events/resource lifecycle are demonstrated or a concrete
platform failure is recorded; do not introduce a second collector or renderer.

The earlier plan review is retained at
`V:/temp/inetmap-rust-map-review-20261023-01a1687c/review.md`. New stage reviews
must record their scope and result here before dependent stages are accepted.
The first native-foundation review is at
`V:/temp/netmap-native-foundation-review/review.md`. Its two reply-lifetime
findings have been corrected and its independent delta review is clean. The new
world-rendering review at `V:/temp/netmap-world-review/review.md` is clean after
correcting swap-chain retirement order. Navigation review at
`V:/temp/netmap-navigation-review/review.md` is clean after capture-cursor and
Fit-focused wheel corrections. Native navigation evidence is recorded with the
source proof. The lead continues as the sole product editor/builder; read-only
helpers may inspect dataset/HTTP contracts for the next location work.
City data now downloads/unpacks/validates/installs through registered workers,
with native Location menu commands and status-only progress. Read-only mapped
country/city readers, immutable-file guards and replacement/recovery are present.
Native download/cancel/restart and blocked-replacement/backup recovery checks
passed; full local lookup integration awaits the connection/model work.
City source review and its fixture/storage/cleanup delta are clean at
`V:/temp/netmap-city-review/review.md`. Native proof scripts validate HWND process
ownership and retire handles between runs before cleanup can post Close.

Settings is now being implemented by the lead in its planned owners:
`app/settings.rs` (preferences, defaults, draft validation, catalogue and save
coordination), its private native dialog/storage modules, and Host's narrow
native dialog/control operations. There is one accepted save at a time; the
registered worker finishes a save independently of dialog cancellation. Startup
reads preferences on the geography worker before dependent origin work exists.
The native shell and three lazy pages are present, with search/reset/Apply/OK
paths. Independent six-pass review and its corrective delta are clean at
`V:/temp/netmap-settings-review/review.md`. The owned native journey passes
search, retained raw drafts, hidden/same-focus validation, page/global reset
with Cancel, Apply/OK, failed-save retry, queued close/reopen, real process
restart and repeated resource-lifetime checks. See
[Settings evidence](../verification/settings.md) for exact native scope and
resource samples. Actual keyboard input still needs a foreground-capable
desktop; physical mixed-DPI/work-area transitions, High Contrast/UIA, attribution link action and
precisely held-pending save timing remain acceptance work. Connection effects
and speed/detail consumers remain incomplete; the Settings
shell is not full stage 1 acceptance. Origin startup must consume the accepted
preference read before deciding whether automatic discovery is permitted;
loading or failed reads cannot establish that a saved Manual choice is absent.
Automatic/manual origin is integrated. `app/origin.rs`
owns choice transitions, generations, dataset matching, debounce and retry;
the existing Work owner runs one bounded origin/request/watch worker. Both route
and interface notifications retain a coalesced invalidation outside the reply
queue and invalidate stale origin facts before completions are accepted. Native
registrations and potentially waiting cancellation belong to that worker.
Manual mode and failed preference reads start no discovery. Dataset replacement
reuses the current public address for local lookup. Map rebasing ends the drag
at its current view and uses the existing Host capture/update path. A 30-second
whole discovery deadline uses the same endpoint timer as debounce/backoff.
Independent six-pass review and its corrective delta are clean at
`V:/temp/netmap-origin-review/review.md`. The owned native journey verifies real
public-address discovery/local country lookup, manual supersession, restart and
clean close. See [Origin evidence](../verification/origin.md) for exact scope,
the restricted-environment failure and remaining network-change/native-view
gates. Settings work-area fitting and native narrow/compact page reflow are now
integrated, with clean source review at
`V:/temp/netmap-settings-layout-review/review.md`. The combined isolated journey
at `V:/temp/netmap-origin-layout-native-final/` verifies initial containment,
simulated compact layout, Settings persistence/validation, real origin lookup,
manual supersession and close/restart. The fixture does not change display
settings; physical mixed-DPI, keyboard and accessibility gates remain open.
This is not stage 1 acceptance.
Country acquisition for a fresh checkout is now verified through the pinned
manifest and isolated preparation check. Fit uses a labelled native button, so
no separate glyph font is required. Final distribution and resource acceptance
remain open.

Connections now has its planned pure model and private native TCP table reader.
The lead remains sole editor/builder. The model gives each observation an
application-owned identity, retains completed TCP/expired UDP through publication
and establishment/activity needs, and accounts attributed plus unattributed
bytes before any snapshot coalescing. Source timestamps may correlate only
unambiguous lifetimes; the unusable native connid is not substituted with the
tuple. Table captures retain their interval bounds and partial reads do not
establish absence. This is work in progress, not production collection proof.
Fixed-clock tests cover same-tuple reconnects, late/equal timestamps, startup
overlap, UDP inactivity, visibility boundaries and byte conservation. The owned
native loopback check finds both idle TCP endpoints in IPv4/IPv6 with correct
PIDs and excludes listeners. See [Connections evidence](../verification/connections.md)
for the exact scope and remaining integration gates.
The pure-model review and its merged deltas are retained at
`V:/temp/netmap-connections-model-review/review.md`; current source review is
clean, as is the separate table-reader review at
`V:/temp/netmap-connections-tables-review/review.md`. Final formatting and the
ordinary/native tests passed; Clippy still reports unused code awaiting its
production consumers. Bounded IPv4/IPv6 owner-PID captures retain normalized
endpoints and partial-read evidence. The owned ETW adapter is present, using
verified v2 event prefixes, one raw-QPC clock, a bounded nonwaiting callback queue
and owned controller/consumer lifetimes. Work now registers aggregation and
consumption separately; native consumer ownership passes directly between
workers. Native process censuses use Windows sequence numbers and conservative
capture bounds, preserving PID-reuse uncertainty for delayed events. Coalesced
snapshots reach Application; a separate registered worker caches current-version
locations and prepares map groups. Collection timestamps survive lookup and UI
delivery, so late results cannot restart establishment age. Unchanged display
facts reuse the prepared model and retained geometry.

The live normal-window path is observed on an ordinary token, including a
controlled external TCP handshake, repeated publication, country destinations,
great-circle guides, marker sprites and clean close. ETW access was denied:
the UI explicitly reports table-only TCP observations with unavailable rates
and UDP. This is an intermediate coverage state, not the final product scope.
The production ETW consumer now decodes process start/end/rundown evidence and
binds it to sequence-number censuses through one process registry. Loss boundaries
survive queued delivery and keep uncertain TCP bytes unattributed. Application
reports coverage, directional rate availability and attribution uncertainty.
The retained source review is clean at
`V:/temp/netmap-process-source-review/review.md`. The latest ordinary suite passes
86 tests, with nine explicit native checks ignored. User-approved elevated runs
passed both bounded production trace checks, including exact own/short-child
IPv4/IPv6 directional bytes, raw QPC and cleanup. Passive physical traffic exposed
System-context incoming TCP events; unique temporal endpoint/process correlation
is now implemented, with reuse/loss/conservation regressions and a passing native
collector rerun. That rerun had a different, low traffic workload and does not
prove high-load attribution. Ordinary execution is still denied ETW access.
Scoped-event correlation and the remaining source
acceptance gates are still unfinished. See
[Connections evidence](../verification/connections.md) for the exact proof limits.
The user's reported NetMap access-error dialog was confirmed in Windows events
for the earlier 16:17 Settings run. An isolated empty-buffer DrawTextW call
reproduces the pointer-0x2 access violation; the current binary's existing guard
prevents it, and its repeated native journey exits normally. Exact attribution
of the historical exception remains inferred without its stack. A native
empty/populated/cleared-label regression was added; see
[crash evidence](../verification/crash-20261023.md).

## Target

NetMap is TinyTorrent2's working connections map, converted to Rust and fed by
the whole PC instead of the torrent engine. Preserve the map's appearance,
projection, navigation, rendering effects, architecture and optimizations.
The person should recognize the same map and be able to move around it in the
same way. The work is a port with a new observation source, followed by new
presentation modes and deeper connection exploration.

The port must also pursue a lighter application, especially lower memory use
than the existing map. Preserve the complete map while reducing avoidable
allocation and retained resources; Rust alone is not evidence of improvement.
The [resource acceptance criteria](#resource-acceptance) require that comparison.

Deliver it in two stages, in this order:

| Stage | Person's experience | Required result |
| --- | --- | --- |
| 1. Converted map and all PC connections | Open an ordinary movable, resizable Windows window. See live destinations and measured receive/send activity for the PC. Pan, zoom, Fit and use the existing map's basic hover/selection behavior. | The complete optimized Rust map works with the whole-PC observation source and local geolocation. Its visual and performance parity is verified before more hosting modes are added. |
| 2. Background, screensaver and exploration | Put that same map behind desktop icons, use it as a Windows screensaver, or open it as an interactive window. Click a destination or connection guide to examine the actual connections and applications represented there. | New hosts and drill-down consume the existing map and connection facts; they do not create another renderer, projection or collector implementation. |

Stage 1 is a useful deliverable in its own right. Desktop attachment, tray-based
hosting transitions, screensaver installation/preview and a connection-exploration
pane are stage 2 work, not prerequisites for starting or accepting stage 1.
Existing map hit testing, selected-route rendering and hover descriptions still
belong to the port; the new click-to-explore workflow comes later.

### What all PC connections means

The scope is this Windows PC, across applications, system processes and network
adapters, for IPv4 and IPv6 TCP connections and observed UDP flows. QUIC appears
as UDP traffic; the application does not need to decrypt it to place its remote
endpoint. Existing TCP connections must appear at startup, including idle ones.
Short-lived TCP connections and UDP exchanges observed while monitoring must
not disappear between periodic table reads.

UDP has no TCP-style established/closed state. A UDP destination can only be
known after traffic to or from it has been observed. Show its last-seen time;
use a 30-second inactivity window initially to distinguish successive observed
UDP flows, without describing expiry as proof that a socket closed. An unused
bound UDP socket has no known remote destination. No collector can recover an
unseen exchange that ended before monitoring began.

Short-lived connections remain visible through the configurable
[establishment highlight](#connection-visibility-and-traffic), independently of
their socket lifetime and the byte-measurement interval. Retain completed TCP
and expired UDP observations while that highlight or current measured-activity
presentation still needs them, then remove them after their bytes are accounted
for. Include retained observations in each latest snapshot; selection, delayed
lookup and a hidden window do not extend their deadlines. Describe ended TCP
as ended and UDP by last-seen time. A hidden window resumes at the current age
without replaying expired highlights or old traffic.

The geographic map shows locatable remote destinations. Loopback, LAN,
link-local and unknown-location connections remain in the observation model
and summary counts, with their reason for not being plotted. A listener is a
local endpoint, not a remote connection. Stage 2's connection list exposes these
facts even when there is no geographic marker. Access to a process name may be
restricted; that does not remove its PID or an otherwise observable connection.

Each observation has a stable identity. Process instance, protocol and full
local/remote endpoints describe its traffic; successive TCP connections can
reuse those endpoints and still remain distinct, including while an ended
observation is retained. Multiple observations at one destination aggregate
visually without losing their identities or double-counting bidirectional
traffic. Activity means measured bytes during a sampling interval, not connection
count or TCP state.
Missing measurements and lost trace events are visible coverage conditions,
not zero traffic. VPN and proxy endpoints are the observable endpoints; the map
does not claim to reveal destinations hidden beyond them.
The scope is host-visible TCP/UDP networking, not named pipes, arbitrary IP
protocols or connections hidden inside a virtual machine's separate network
stack. Show host-visible virtual-adapter traffic without claiming guest process
attribution that Windows did not supply.

### Product and technical constraints

Use Rust with Microsoft's `windows` crate, Win32, Direct2D, DirectWrite,
Direct3D 11 and DXGI. Keep one map implementation and one authoritative
observation model. Use native GPU drawing and the existing bundled geography.
TinyTorrent2 remains the reference and is not changed by this work.
Desktop operation must keep memory and idle cost low. Use native OS facilities
without bundling a UI framework, browser engine or async runtime. The accepted
result must satisfy [resource acceptance](#resource-acceptance), including the
collector and all active hosts, rather than inferring memory savings from the
language or executable size.

Stage 1 starts and closes as an ordinary window. Pan/zoom are session-only, as
in TinyTorrent2; save window placement and map-detail choice. Stage 2 adds host
selection and appropriate lifetime rules without changing the map's behavior.

All existing map optimizations are stage 1 requirements. Background/screensaver
hosting and richer exploration are the only planned feature deferrals; a
TCP-only or simulated-activity viewer is an intermediate development result,
not the completed whole-PC stage.

### Stage 1 window and visible states

Open directly on the fitted world map, initially in Countries detail as in
TinyTorrent2. The normal title bar moves/resizes the window. Keep the current
map's compact Fit/zoom controls and speed placement; native menu commands expose
Countries/Cities, city-data installation/update, origin, Settings and Exit.
Keep normal Win32 chrome and common controls. The native
[Settings dialog](settings.md) includes Map, Connections and Location pages,
search, page/global reset and OK/Cancel/Apply with a status bar. Commands that
also appear in menus use the same preference and operation owners.
There is no dashboard, sidebar, tray lifetime or connection-exploration list in
this stage. Choose the lightest native composition that keeps commands reachable.

A compact summary reports observed connections, those transferring measured
bytes and those with map locations. Its scope is the accepted whole-PC snapshot;
grouping several connections into one marker does not change those totals.
Receive/send totals include unplottable observations from the same measurement
window. Distinguish currently retained connections from recently completed
traffic in their descriptions; do not label a closed flow as a live socket.

| Condition | What the person sees |
| --- | --- |
| Collection warming up | The world appears immediately; observations/locations arrive as available and rates wait for a valid interval. |
| No current or retained observations | The normal world map and a zero-connection summary; no fabricated markers or animated activity. |
| Origin unavailable | Located destinations remain visible, arcs are omitted and the map describes the missing origin. |
| Some endpoints cannot be located | The valid map remains usable; the summary accounts for unmapped observations. |
| City data absent | Countries works; Cities is disabled and Get city data is available, matching the source. |
| Collection lacks access or has lost events | The map remains usable with an accurate coverage state; incomplete rates are not presented as complete measurements. |
| Required map/country asset is damaged or missing | A concise failure identifies the missing resource; no silent substitute map/provider is used. |

### Connection visibility and traffic

Establishment and traffic are separate signals. A newly observed connection
starts bright, stays at full establishment visibility for Bright for, then
linearly fades to zero over Fade out. Initial defaults are 10 seconds bright
and 20 seconds fading; both are saved, configurable durations. Bright for is a
positive whole number of seconds; Fade out may be zero for immediate removal
after the bright period. Accepting changed durations applies them to existing
highlight ages without restarting an observation or resurrecting a retired one.

Start the establishment clock once, when the observation first enters a
published snapshot. A connection that opens and closes between samples still
gets that first snapshot and highlight. TCP entries already present at startup
use first observation, not an invented original open time; the first datagram
introduces a UDP flow. Traffic, close events, polling, lookup completion,
selection and redraw do not restart the establishment clock. An unlocatable
observation remains represented in the summary, without a fabricated map point.

Actual measured receive/send activity produces a subtle directional pulse on
the existing guide: send moves toward the destination, receive toward the origin.
Bidirectional traffic shows both directions. The pulse follows the current
measured interval and existing bounded activity interpolation; unavailable or
zero activity produces no traffic pulse. Establishment brightness alone never
claims bytes transferred. The establishment highlight may finish while a busy
connection continues to pulse; a live but idle TCP connection remains in the
observation model and stage 2's All connections even when its map trace is hidden.

Destinations still aggregate observations. The establishment visibility of a
group is the maximum of its contributing highlights; directional activity uses
their measured rates. A new connection may brighten a shared destination, but
does not reset older identities or their expiry. Retention alone creates no new
traffic pulse or repeated bytes; bytes captured in the current interval may
still pulse even if that connection has ended. Once neither establishment nor
current activity contributes, its map trace disappears; live facts remain
available and completed facts retire under the retention rule above. Hidden
marks are excluded from hit testing and the map's inspection cycle.

Keep High Contrast marks solid and readable. In High Contrast or reduced motion,
replace moving pulses and gradual dimming with a static activity indication and
timed removal at the same deadline. Selection/focus stays legible without pinning
completed observations. These temporal effects are the deliberate extension to
the source map; its spatial landing fade, projection and navigation remain.

### Distribution and saved data

Stage 1 distributes the release executable with `assets/world.bin`, country MMDB
and required notices/font resources. Keep each asset once; the distribution does
not depend on the sibling TinyTorrent2 checkout or its build outputs. Save city
MMDB, settings and download temporaries under `%LOCALAPPDATA%/NetMap`, not beside
an executable installed in a read-only directory. Save only the chosen window
placement, detail, speed position, origin and visibility durations; live
connection history is not persisted. Screensaver packaging is stage 2 work.

Use the existing monthly country-data acquisition policy when preparing a
distribution. Runtime city updates download/unpack to a temporary file and
validate it before installation. Keep the previous
usable dataset on failure and invalidate lookups only when a new version is
installed. Never reload the MMDB or world geometry for each connection sample.

### Responsiveness and exit

The map remains responsive while collection, location lookup, public-origin
discovery or dataset installation is pending. Minimized or inactive presentation
stops drawing and resumes with current observations; collection continues
independently. Navigation does not wait for the observation publication interval.

Exit removes visible windows promptly. Application cleanup has an initial
five-second deadline for cooperative completion or initiation of its own-process
termination fallback. Eventual Windows process removal can take longer while
pending I/O completes or is cancelled; it is measured separately. Closing a
desktop-enabled window or dismissing a screensaver follows the stage 2 lifetime
rules rather than stopping a collector still in use.

Cancellation, replacement failure or fallback preserves the last committed
settings and a usable dataset. A pending settings choice may not have been
saved. Dataset updates retain the previous usable data until the new reader
opens successfully.

## Stage 2: hosting and exploration target

This section specifies the next product stage. None of its Windows hosting
experiments or new exploration UI blocks completion of the stage 1 normal window.

### Desktop background

Add explicit window and desktop hosting modes when both exist. One command owns
each transition, whether invoked from the tray or the window menu. Screensaver
activation is a Windows launch/lifetime path using the same compiled map modules.

- Background mode is opt-in; remember the chosen mode after it is enabled.
  Provide Open map, Use as desktop background and Exit through the tray. In
  window mode, the title bar moves the window; dragging inside the map pans it.
- Desktop mode is passive. Icons, icon selection, drag-and-drop, desktop menus
  and ordinary shortcuts remain Explorer's. Do not use global input hooks or
  steal focus to make the wallpaper interactive. Open map is the way to inspect
  or reposition the map, then return it to the background.
- Preserve zoom, geographic centre, map detail, inspected destination and
  selection through a mode change. Resize only constrains the view as the
  existing viewport already does. Clear transient hover and end an active drag
  cleanly before moving hosts. Preserve the normal window's bounds separately.
- When desktop background is enabled, closing the normal window returns the map
  to the desktop. Otherwise closing it exits, as in stage 1. Exit in the tray
  always shuts down the application. Save window placement, chosen monitor and
  map detail; keep zoom/pan session-only, matching TinyTorrent2's map.
- Initially show the background map on one chosen monitor, defaulting to the
  primary monitor. The normal window can move between monitors. Verify mixed
  DPI, negative monitor coordinates, taskbar placement and monitor removal.
  Simultaneous independent desktop maps on every monitor are outside stage 2's
  initial background implementation.
- Recover desktop attachment after Explorer recreates its windows. Restore
  native window/tray resources as necessary while retaining plain map state.
  Exiting removes only NetMap's windows; the person's existing wallpaper stays
  underneath and becomes visible again.

The [host implementation guidelines](../agents/architecture.md#stage-2-hosts-and-exploration)
own Explorer attachment and native target replacement. This stage is accepted
only after desktop placement, input and DPI work on the target Windows 11 build.

If background attachment fails, keep the normal window usable and show a clear
failure through the tray. Do not silently substitute a topmost overlay, static
wallpaper generator or a separate wallpaper engine. Desktop mode is not accepted
until it actually works.

### Windows screensaver

Provide a real Windows-selectable `.scr` entry using the same Rust application
modules. Support full-screen execution, the settings preview parent window and
configuration entry (`/s`, `/p <HWND>` and `/c` forms), with native argument
handling and resource cleanup. Installation/selection is a deliberate user
action through the standard Windows screensaver workflow.

Full-screen mode renders the live map without exploration controls. Follow
normal screensaver dismissal on input and leave sign-in-on-resume to Windows.
A click dismisses the saver; it does not open connection details on top of it.
Preview stays within its supplied parent and does not capture input or start a
second full-screen surface. Do not display false sample traffic in a product
preview when live collection is unavailable.

Use the same dataset/configuration policy in every launch path. Windows may
launch the saver separately from the normal application; live collection follows
the [privilege and lifetime policy](#privileges-and-lifecycle). Cover connected
displays in full-screen mode with a consistent observation feed. Pause
covered/hidden drawing in an already-running application.

Verify Windows settings selection, Preview, full-screen launch, input dismissal,
sign-in-on-resume, display sleep, multi-monitor DPI and preview-window teardown.
The screensaver must not prevent the display from sleeping or replace Windows'
lock/sign-in behavior.

### Click-to-explore connections

In the ordinary window, clicking a destination marker or route opens the
connections represented there.
Country detail includes that country's observations; coordinate detail includes
that location; an estimated destination clearly identifies its country-level
precision. Clicking a representative population point never claims exact
knowledge of a connection's city.

The connection list shows application/process name where accessible, PID,
protocol, local and remote address/port, TCP state or UDP last-seen time,
country/city precision and measured receive/send rates where available. All
connections is the unfiltered entry, including unplottable endpoints. Selecting
a row highlights the corresponding destination/route through the existing map
selection mechanism. Escape/clear returns to all observations without moving
the viewport. Keep selection identity stable through updates. A completed TCP
connection is shown as ended while its observation remains retained under
[connection visibility](#connection-visibility-and-traffic); UDP keeps its
last-seen/flow-expiry distinction. When a completed observation retires, remove
the row and clear that exact row's selection, without selecting another
connection or extending retention. A selected destination group may remain
while it still represents other accepted
observations. Reused PIDs/sockets never inherit the expired connection's identity.

Keep the related Map only, Table only and Map and table choices. Port the source's
fitted-scale arrangement decision and per-orientation split proportions. All
lists, counts, descriptions and map highlights read the same accepted snapshot.
This stage explores local connection facts; packet content, remote-machine
inspection, firewall controls and historical traffic recording are outside it.

From the desktop, Open map enters this normal-window exploration experience.
Desktop icons retain their input behavior. Exploration never runs on the lock
screen or intercepts screensaver dismissal.

## Map parity requirements

The Rust map must preserve the following behavior. Differences needed for PC
connection facts belong in the observation mapping, not scattered renderer forks.

### Origin and viewport

When the origin changes, a nonfitted view keeps the same geographic centre and
zoom, subject to the existing vertical bounds. A fitted overview stays fitted
and follows the current origin's central meridian. Finish any active drag at
its current view and clear transient hover before applying the change.

This applies when an origin becomes known, an automatic or manual origin
changes, or an origin becomes unavailable. Later drag completion must not
restore the view from before that change.

### Projection, placement and navigation

- Use the existing equirectangular projection, origin-centred meridian and
  latitude-dependent horizontal correction. Fit remains the 2:1 full-world
  overview. Preserve small/undrawable-view handling and finite polar bounds.
- Preserve shortest great-circle guides, antimeridian splitting and wrapped
  country copies without drawing an artificial border at the geographic cut.
  Guides represent endpoint locations, not measured network routes.
- Preserve Countries and Cities detail, coordinate grouping, mixed precision,
  country anchors and missing-outline territories. Unknown locations remain
  unknown; they are not placed at zero latitude/longitude.
- Preserve 25-DIP region merging at fitted-map scale, population-based placement,
  D'Hondt reach and population-weighted traffic allocation. Zoom and pan must
  not redistribute observations. Keep country-only placement visibly estimated.
- Port the distribution cases for 90/10 and 50/20/10 populations. The bundled
  places were pruned around TinyTorrent2's 200-connection default; verify larger
  PC connection counts conserve every observation and total rate rather than
  assuming the asset contains an unlimited placement distribution.
- Wheel zoom stays pointer-anchored, including fractional deltas, at 1.25 times
  per notch; double-click zooms one step. Preserve the 1x to 128x range.
- Left-drag pans after the existing 4-DIP threshold, with pointer capture and
  the pan cursor only while dragging. Escape/capture loss restores the prior
  view when cancelling. A drag does not inspect a location.
- Preserve endless horizontal pan, vertical constraints, geographic centre on
  resize and direct input without added easing. Connection/location updates do
  not recenter the view; origin changes follow [Origin and viewport](#origin-and-viewport).
- Preserve Plus/Minus with or without Ctrl, main-keyboard/numpad handling,
  Ctrl+Arrow pan by one eighth of the view, Ctrl+0 Fit,
  arrow-key inspection/reveal, Escape clearing and native context-menu keys.
- Preserve marker-first hit testing (12-DIP radius), route inspection, selected
  route/marker highlights, descriptions and accessible keyboard help. Countries
  themselves do not acquire new hover or selection states.
- Keep the compact zoom label and Fit button at bottom centre, shown when the
  view is not fitted, with correct focus restoration. Keep native Zoom in,
  Zoom out, Fit and detail commands and the existing Fit glyph. Reuse only the
  required icon/font resources. No permanent toolbar is added.

### Appearance, effects and map-related controls

- Match the existing land, boundary, download, upload and neutral palette roles,
  subdued country activity shading, route brightness profile and marker sizes.
  Start from the current TinyTorrent2 palette rather than inventing a new theme.
- Keep the source's location-marker style, per-kind offsets, origin marker,
  selection rings and sharp selected route cores. Keep explanatory counts and
  precision in descriptions rather than relying on colour alone; apply the
  [establishment visibility](#connection-visibility-and-traffic) to connection marks.
- Keep the fixed logarithmic rate scale, normalized to 5 MiB/s, and the existing
  1.6-second activity interpolation. Add only the establishment fade and measured
  directional pulse specified above, using the same renderer; no particle system.
- Preserve the 30-DIP destination landing fade, halo strength
  `0.5 * 0.35 * strength^2`, one shared 1.5-DIP Gaussian blur and sharp arc cores.
- Composite arcs and glow together, apply the existing adaptive origin mask over
  40 DIPs and cap that layer at 80% opacity. Preserve the source arc-count formula
  and its eight-to-twenty-arc transition. Keep markers outside that layer.
- Keep widths, marker radii, hit targets and fades in screen-space DIPs under
  nonuniform scale and DPI changes. Preserve the source's selected 2-DIP routes.
- Preserve Light/Dark behavior, High Contrast solid lines without glow/dimming,
  reduced-motion behavior, focus indicators and accessible inspection text.
- Preserve the map speed display and its Top-left, Bottom-left and Bottom-right
  choices, with zoom/Fit independent of that placement. Show only measured
  receive/send totals and keep unmapped traffic in totals when it is available.
- Stage 1 presents the map with its existing navigation and basic descriptions.
  Stage 2 adds the associated Map only, Table only and Map and table behavior
  through the connection-exploration workflow defined above. Preserve the
  renderer's existing hit-testing and highlight machinery during the initial
  port, so exploration later consumes it without a replacement implementation.

Torrent selection, seed/leecher identities, Library navigation and torrent
commands are not PC connection concepts. Translate labels and observation roles
at their owner; do not copy those product semantics into the map or import the
whole torrent application to retain its map architecture.
For PC observations, the corresponding marker kinds are receive-dominant,
send-dominant and unknown, based on measured traffic where available. A
bidirectional connection remains one observation with both rates; it is not
counted twice to create separate colours. This is a domain adaptation, not a
second marker implementation.

All [optimization parity requirements](../agents/porting.md#optimization-parity-checklist)
are mandatory in stage 1 alongside these visible behaviors.

## Connection and location facts

Stage 1 combines existing TCP endpoint facts with observed TCP/UDP traffic,
including byte measurements. Initially publish reconciled observations every
two seconds, independently of rendering and input.

### Collection and identity

- Existing IPv4/IPv6 TCP endpoints, including idle ones, appear at startup.
  Connections created or ended during startup reconciliation must not be lost
  or counted twice. Periodic reconciliation settles current TCP state and
  missed close events.
- An observed UDP flow has a remote endpoint only after a datagram supplies it.
  Local socket ownership alone does not establish a destination. Preserve IPv6
  scopes; IPv4-mapped addresses have one consistent identity.
- Each observation has an identity independent of its endpoint tuple. Distinct
  TCP lifetimes (incarnations) remain distinct even when the same process reuses
  a tuple while the ended observation is retained. This also applies to a
  connection first discovered in the startup table. Repeated evidence about one
  incarnation does not create duplicate observations, and late events for an
  ended incarnation must not change its live successor.
- A process's creation time may be unavailable; it stays unknown, and PID alone
  is not a durable process-instance identity. When loss or missing evidence
  prevents reliable correlation, expose that uncertainty rather than inheriting
  a prior observation's counters or selection. UDP identity describes the
  observed endpoint flow within its inactivity window, not a proven socket
  lifetime; traffic after that inactivity expiry starts a new observation even
  if the preceding flow's establishment highlight has not finished.
- Activity is measured send/receive bytes counted once over the actual elapsed
  monotonic interval. TCP retransmission and lifecycle records are not additional
  independent application transfers. These are observed transport rates and can
  differ from torrent payload rates. Both directions belong to the same
  observation.
- Completed-TCP and UDP observation retention follows
  [the target](#what-all-pc-connections-means), independently of byte intervals.
  Retained ended facts stay in each latest snapshot and remain eligible for
  current-dataset local lookup. After their measured interval, rates become zero
  when coverage remains valid; retention does not replay the final transfer.
  Retiring a completed observation removes lookup and exact-row selection interest
  without indefinite pinning. Fading a live TCP mark does not remove its facts.
  Summary counts distinguish retained ended TCP from current TCP and
  observed UDP. Coalescing presentation snapshots must not lose accepted bytes.
- Loopback, private, LAN, link-local and unknown-location endpoints remain in
  observations and totals with their unplottable reason. Access-denied process
  metadata does not discard an otherwise valid connection.
- Missing rates stay distinct from zero. Connection count and established TCP
  state are not bandwidth measurements. The coverage/status line distinguishes
  warming up, complete collection, restricted access and observed event loss.
  Product activity is never fabricated; deterministic parity fixtures may use
  synthetic rates.

### Privileges and lifecycle

Full collection may require an elevated launch or explicitly provisioned tracing
rights. Stage 1 can use an explicitly elevated application as its initial full
collection path; a normal launch reports insufficient coverage clearly. The
port does not change group membership, install a service or add a collector
process without a demonstrated need.

Interrupted prior runs must not terminate unrelated trace sessions. After sleep
or network changes, collection recovers and reports intervals with missing facts.
Collection startup and shutdown follow [responsiveness and exit](#responsiveness-and-exit).

In stage 2, ordinary-user presentation can consume a read-only, user-restricted
collector feed if collection rights require it. Screensaver preview and dismissal
must not request elevation. The collector implementation remains shared.

### Geolocation and origin

- Use the same DB-IP Lite country/city MMDB data family as TinyTorrent2. Country
  data is available locally; city data is optional, with the same fallback and
  disabled-Cities behavior. Lookups reflect the installed dataset version.
- Include existing attribution and monthly provenance. Endpoint lookup stays
  local. Installing/updating city data is an explicit action and retains the
  previous usable dataset on failure.
- TinyTorrent2 gets its public origin address from its engine. NetMap's default
  overview obtains the current public egress address with one HTTPS request to
  https://api64.ipify.org, then geolocates it locally. Run this on startup and a
  debounced route/adapter change, with timeout/backoff on failure, never once per
  remote endpoint. Disclose the public-address request and permit a saved manual
  origin instead. No remote connection list or destination addresses are
  submitted to the origin provider.
- The origin denotes the default-route egress location, not a measured starting
  coordinate for every socket on every adapter. Manual origin overrides that
  display choice. Origin changes follow [the viewport contract](#origin-and-viewport).
  If lookup is unavailable or stale after a network change, keep destinations
  visible and indicate unknown origin instead of drawing misleading arcs. With
  multiple concurrent VPN/proxy routes, report the limitation of one overview
  origin rather than claiming per-connection path knowledge.

ETW privileges/coverage and the real origin path remain early acceptance gates.
Report a concrete platform limitation before changing the target; a TCP-only
or no-rate result does not complete whole-PC support.

## Evidence and implementation gates

The source map has been studied; no Rust renderer or native collector has been
built during planning. Distinguish established behavior from the checks still
needed on this machine:

| Decision | Evidence now | Check before dependent work |
| --- | --- | --- |
| Reuse the model/viewport/renderer structure and every listed optimization | Inspected TinyTorrent2 source, behavior contracts, asset and existing capture journeys. | Pin the ported revision; compare matched Rust scenes and measurements. |
| Direct2D can express the current Win2D mechanisms | Native geometry realization, effect, layer and sprite-batch interfaces exist and have Rust bindings. | Create the required device context and verify effect/bitmap/DPI behavior in the native window. |
| ETW plus tables is the whole-PC source | Microsoft documents TCP/UDP events with endpoint, PID and size facts; tables supply existing TCP endpoints. | Verify schemas, privileges, attribution, rates and event loss with controlled workloads on the target Windows build. |
| A default-route origin can use public-address discovery and local geolocation | The provider documents an IPv4/IPv6 public-address endpoint; TinyTorrent2 already uses the local MMDB data family. | Verify the actual network/proxy route, timeout/failure behavior and refresh without disrupting the view. |
| Desktop and screensaver can consume the accepted map | Existing Explorer-host practice and Windows' screensaver launch model provide integration approaches. | Stage 2 proves the host-specific DPI, privilege, input and lifetime behavior; no claim of working hosting is made before those checks. |

The early native checks settle technical feasibility, not a redesign of the
target. If a check fails, fix its owner or report the concrete product tradeoff;
do not add an alternative implementation that silently becomes the default.

## Resource acceptance

Matched reference scenes must show no unexplained performance regression and no
growth across repeated settled navigation or host-lifetime cycles. Establish
TinyTorrent2's memory baseline before implementing the renderer. Demonstrate a
repeatable reduction in steady-state private bytes for matched map workloads,
beyond run-to-run variation, with working set, mapped dataset residency and
dedicated/shared GPU memory reported alongside it. Moving allocations to another
process or memory category does not establish a reduction.

Distinguish the map's contribution from the complete TinyTorrent2 application's
cost. Report total NetMap memory, including any collector process, and measure
the added cost of live whole-PC collection separately from deterministic map
fixtures. A comparison against unrelated torrent features alone cannot establish
that the map became lighter. If reduction cannot be demonstrated, report the
measured shortfall and its cause for the user's decision before accepting the
lighter-memory objective; bounded memory alone does not satisfy it.

Record actual executable size, bundled assets, optional data and runtime
dependencies separately. Resource savings must preserve the required map
behavior, connection coverage and responsiveness.

[Development and verification](../agents/development.md) owns the implementation
sequence, concrete acceptance journeys and measurement procedure.

## Completion criteria by stage

Stage 1 is complete when the ordinary Rust window reproduces TinyTorrent2's map
appearance and navigation, satisfies the
[architecture](../agents/architecture.md) and
[optimization parity requirements](../agents/porting.md#optimization-parity-checklist), and real
whole-PC TCP/UDP observations drive it with truthful rates/location precision.
Existing and short-lived connections, access limitations and event loss have
been checked, including delayed lookup/coalescing, configurable establishment
hold/fade, measured directional activity, retention expiry and no byte replay.
Worker shutdown and late replies follow the
[nonblocking lifetime contract](../agents/architecture.md#background-work-and-shutdown).
The observation source/privilege behavior and default-route origin
are working. Native visual checks, focused tests and matched resource
measurements satisfy [resource acceptance](#resource-acceptance); actual sizes
replace preliminary estimates.

Stage 2 is complete when that accepted map additionally works as an opt-in
desktop background and a native Windows screensaver, and click-to-explore works
in its ordinary window. Host changes preserve the view, Explorer and screensaver
lifetime/input rules hold, and all exploration views agree with one observation
model. Repeated host launches/transitions and changing selections do not leak
resources or create duplicate observation/renderer implementations. Dismissal
does not wait for native work or terminate a collector still in use elsewhere.

Both stages preserve the original requirement for one authority and elegant,
ordinary code. Any departure from the source map is recorded with its reason
and evidence; it is not hidden by calling an unfinished port lightweight.
