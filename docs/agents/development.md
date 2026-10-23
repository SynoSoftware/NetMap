# Development and verification

Mandatory for implementation, review and verification. Read Code, names and
comments and Change and verification before changing code; read the affected
stage and shared validation before implementing or accepting that stage.

The [software specification](../plans/rust-map.md) owns the target and acceptance
criteria. [Architecture](architecture.md) owns structure and lifetimes;
[porting](porting.md) owns the source reference and required map mechanisms.
The build/installer scaffold and temporary native bootstrap are present. The map,
collection and Settings steps below still describe required work, not evidence
of completion. Use the [current build commands](../../README.md#build-and-check)
and the existing scripts for checks and packages.

## Code, names and comments

Write correct, elegant, easy-to-read code with minimal complexity. Choose the
simplest appropriate algorithms, state representations and control flow, with
intent and ownership obvious on first reading. Correctness, security and
reliability take precedence over brevity.

- Choose algorithms and data structures for the expected scale. Among equally
  effective solutions, prefer straightforward control flow over cleverness.
  Use standard Rust and native APIs directly; a wrapper earns its place by
  owning a real invariant, resource or boundary.
- **One responsibility, one owner, one authoritative implementation.** Find and
  use the feature's established owners and contracts before adding behavior.
  Where a needed authority is missing or responsibilities are unclear, establish
  or refactor that owner within the feature instead of adding a competing path.
  Commands, menus and gestures for the same operation share its availability rule
  and execution path. Store facts and decisions; derive presentation. Retain
  computed state only for a concrete performance or lifetime need, beside the
  owner that invalidates it.
- **Refactor when it materially improves the feature.** Simplify code, remove
  duplication and competing rules, clarify ownership, and make responsibilities
  easier to understand. Preserve established contracts, not poor internal
  structure merely to minimize the diff. Apply this judgment to current work and
  relevant deferred findings, respecting explicit user deferrals and keeping
  unrelated redesigns outside the change. Abstractions and function boundaries
  earn their place when they clarify a responsibility or hide necessary
  complexity; reject indirection, ceremony and navigation cost without that
  benefit. Every layer needs a concrete use in the stage being implemented.
- **Choose the right model for the behavior.** Evaluate booleans, enums (Rust's
  discriminated unions), state machines, polymorphism and ordinary branching
  according to the problem. Use booleans for independent facts, enums for mutually
  exclusive states, state machines for constrained transitions, and polymorphism
  when implementations differ in behavior. Interacting flags, impossible
  combinations, repeated conditions and scattered transitions signal that the
  model may be wrong. Reconsider the representation instead of adding more
  branches. Choose the simplest model that expresses the rules correctly.
  Let types rule out invalid states; keep platform-defined flags and sentinels
  at their native boundary. In Win32, interaction, work and resource lifetimes
  belong to their model owner rather than behavioral flags scattered across
  message handlers.
- Use typed inputs, replies and failures at boundaries so callers can distinguish
  outcomes they handle differently. Make dependencies and side effects explicit
  at the call: constructors receive dependencies, and methods that save, send or
  change shared state say so in their names.
  Validate external data once at its owning boundary; internal code uses that
  accepted representation.
- Use one canonical term per concept. Let modules, types and ownership carry
  context; names add only distinctions still needed at the use site. Prefer
  domain and platform vocabulary to generic Manager/Helper/Data names or invented
  abbreviations. Every word must preserve meaning, not repeat the owner or type.
- Keep related rules, supporting records and caches beside their owner. Split
  when authority, lifetime or thread ownership differs, rather than by file
  length or mechanical role.
- Comments preserve non-obvious constraints, invariants, units, safety or
  lifetime obligations that code cannot express. Give each explanation one home.
  Keep task history in the change report and update comments the change makes
  inaccurate; leave unrelated knowledge intact.

## Change and verification

1. **Trace first.** State assumptions and an observable completion condition.
   Identify the affected state, control, work and resource owners and follow
   their callers through updates and shutdown before editing. Resolve material
   ambiguity before dependent changes. Scale the plan to the task.
2. **Change the feature coherently.** Apply the refactoring policy above, match
   existing style, and preserve behavior outside the request. Remove obsolete
   paths and orphans your change creates rather than leaving two implementations;
   report unrelated problems.
   Use patch edits that preserve existing encoding/BOM. Follow `.gitattributes`
   when it owns text normalization; otherwise preserve existing line endings
   and use LF for new files.
3. **Review the result.** Trace correctness and failure/lifetime paths, then
   ownership and simplicity, then reread the corrected result for naming and
   comments. Recheck findings after fixes. Finish when affected paths are
   accounted for and every remaining abstraction and state has a concrete need.
4. **Verify the actual change.** Use existing evidence and the smallest check
   that resolves each remaining uncertainty. Finish edits and source review
   before compiling unless later work needs a compiler/runtime answer. Reuse
   current outputs; documentation-only changes need no build. Use the repository's
   actual manifest, toolchain and scripts once present rather than importing
   TinyTorrent2's build commands or creating speculative tooling.

Each new test names a specific failure and asserts the observable outcome it
would change. Prefer pure tests for numerical parity, aggregation, transitions,
effect ordering and stale replies; use native checks for uncertain platform,
input, paint and lifetime behavior. Avoid tests that mirror implementation or
pin incidental wording. Run formatting, Clippy and focused tests for Rust changes;
stage acceptance also requires the matched visual/resource measurements below.

For painting fixes, repeat the same offending interaction before and after;
paint counts alone do not prove flicker is gone. Review changed work paths for
blocking calls, unowned threads, late replies and close behavior. Report observed
results separately from source deductions, with remaining gaps. Protect the
owner's data and running applications, and close only instances you launched.

## Test placement

Put pure tests beside the owning implementation with `#[cfg(test)]`;
extract a private `tests.rs` child only when useful. Shared deterministic inputs
live in `tests/fixtures/`. Add native journey scripts under `tests/` when those
checks exist. This layout needs neither a second crate nor a library target
created solely to expose internals to tests. Keep capture fixtures out of the
production observation path.

## Stage 1 implementation: converted map and whole-PC feed

### 1.1 Establish the reference, native window and collection proof

Pin the current TinyTorrent2 map revision and available capture fixtures. Record
the matched reference memory baseline under
[shared validation](#shared-validation-and-resource-measurements) before renderer
implementation, so its costs guide the port rather than becoming a late surprise.
Create the minimal Rust/Win32 ordinary window with a GPU-backed Direct2D surface and
normal DPI/resize/close behavior. Separately exercise the chosen ETW session and
TCP/UDP reconciliation on the actual Windows 11 build before relying on it for
the live feed. Do not implement WorkerW attachment or screensaver code here.

Verify: a normal movable/resizable window on mixed-DPI displays, clean startup
and shutdown, existing TCP connections, short-lived TCP events, UDP/IPv6
send/receive events, process attribution and actual byte deltas. Record tracing
privileges, event schemas, event-loss reporting and measured overhead. If the
selected source cannot meet the target, settle that concrete issue before
building dependent live behavior; the normal-window map port can still progress.
Close during active trace delivery and delayed native work: the window must
disappear without joining a worker, callbacks retain valid state until finished,
and cleanup/fallback initiation follows the [shutdown deadline](../plans/rust-map.md#responsiveness-and-exit). Record
eventual OS process removal separately rather than asserting an unconditional
five-second exit bound.

### 1.2 Port geography, viewport and model

Copy the existing map asset and translate Geography, MapViewport and MapModel.
Use deterministic observations corresponding to TinyTorrent2's scenes; keep
Windows collection out of the model. Carry source attribution into NetMap.

Verify: exact asset decoding, country identities, geographic transforms,
antimeridian/polar behavior, pointer anchoring, resize, distribution, precision,
selection identities and conservation of connection/rate totals. Add focused
Rust tests for these observable behaviors, including the source distribution
cases and high connection counts. Use the existing algorithm and asset rather
than introducing a second geography/projection path.
For origin transitions, cover unknown/known/manual/stale origins while fitted
and at high zoom: a nonfitted geographic centre stays fixed, a fitted overview
follows its stated meridian policy, and ending a drag cannot restore coordinates
from the old meridian after rebasing.

### 1.3 Port the complete renderer and existing map interaction

Translate Win2D resources to Direct2D geometry realizations, effects, command
lists, brushes and sprite batches. Preserve ownership, draw order, formulas,
cache keys and scheduling. Translate PeerMap's map navigation, basic
selection/hover and compact controls to the native window. Implement every
optimization alongside its rendering behavior; do not make a naive renderer
the default and postpone optimization parity.

Verify: matched scenes and view sizes against TinyTorrent2 in Light, Dark and
High Contrast; geometry cuts and holes; dense/idle/selected routes; unknown
locations; reduced motion; stable DIP sizes; all existing map navigation and
focus behavior; and no recurring frames after interpolation, fades, pulses and
rebuilds finish. During a static bright hold, check that the continuous timer is
off and only the next transition is scheduled. Verify pulse direction with
send-only, receive-only and bidirectional fixtures; zero/unavailable rates never
pulse. Check High Contrast and reduced motion use the static alternative.
Check native precision-touchpad input. Tick every [optimization row](porting.md#optimization-parity-checklist) against code
and observed behavior. New connection drill-down/list UI remains stage 2.

### 1.4 Feed the map with real PC observations

Connect the proven collection owner and local MMDB lookup to the same map model.
Add process-instance identities, byte intervals, flow lifetime and coalesced
snapshots. Add dataset availability/update commands, public-origin discovery
and the coverage/status presentation. Keep basic map descriptions accurate
when names, coordinates or rates are unavailable.
Implement the [Settings specification](../plans/settings.md), including its full
native shell and stage 1 pages, through the existing settings/work owners. Its
acceptance sequence covers draft, search, reset, save and native interaction;
complete those checks before stage 1 acceptance.

Verify: IPv4/IPv6 endpoint identity against an independent Windows connection
listing; controlled TCP and UDP send/receive workloads; connection creation and
removal between table polls; existing idle TCP; UDP retention; PID/port reuse;
restricted process metadata; event-loss detection; location misses; absent or
corrupt city data; and network/VPN changes. Check byte conservation before and
after visual grouping. Check private/LAN endpoints remain counted without false
geographic placement. There is no synthetic activity in the product path.
With cached locations, measure controlled traffic from event receipt through
accepted snapshot to the visible marker/rate update. Check that publication
follows the specified two-second cadence without an accumulating display backlog;
report trace delivery, uncached lookup delay and activity interpolation separately.
Add a fixed-clock case for a TCP connection that closes before the first sample:
delay its uncached location lookup and supersede a presentation snapshot, then
accept a later snapshot before retention expires. Its ended identity still
appears, its bytes occur in exactly one measurement interval, and expiry removes
it despite selection or an unresolved lookup. A window resumed after expiry
does not replay it. Add a same-process, same-tuple TCP reset/reconnect before
the old observation expires: the ended and live incarnations coexist with
independent byte totals and lookup interest; the ended observation expires on
its own deadline while the live successor remains. A delayed event for the old
incarnation cannot add bytes to or close the new one.
Use a fixed clock to check first publication of a connection shorter than the
sampling interval, the exact bright/fade boundaries, zero-duration fade and saved
duration changes. New traffic and close events do not restart establishment;
live idle TCP remains in the model after its mark fades. A shared destination
uses its newest contributing establishment without resetting older identities;
retired observations never replay bytes. Verify pulse/fade frames reuse geometry
and blur caches, and measure their added CPU/GPU cost under resource acceptance.
Also close/cancel during origin discovery and city
download/install, change manual origin during an older request, and check stale
replies, dataset integrity and callback/reader lifetime under the [shutdown policy](architecture.md#background-work-and-shutdown).

### 1.5 Measure parity and accept the first deliverable

Complete the normal-window behavior, visual and performance checks below and
record actual EXE, asset, memory and collection costs. Ship/accept the useful
stage 1 map independently of the later hosting work. Do not mark stage 1 done
with unfinished optimization parity, no real UDP coverage or fabricated rates.

## Stage 2 implementation: background, screensaver and exploration

### 2.1 Add desktop hosting to the accepted map

Prove the Explorer attachment on the target Windows build, then implement the
desktop/window transition in the Host owner and expose the native tray commands.
Preserve the accepted renderer and viewport. Only add the separate collector role
if the stage 1 access result requires it for normal-user presentation.

Verify: placement behind icons, untouched desktop input, Show desktop, mode
transitions, window bounds, mixed DPI, monitor removal, Explorer restart and
shutdown revealing the prior wallpaper. Switch desktop/window 100 times while
traffic changes, including at 128x zoom. Check view continuity and settled native
resource counts/memory. A desktop-host failure does not invalidate the already
working stage 1 normal window.
Start origin/uncached location lookup or city installation in Window, replace
the map HWND during the move to Desktop, then complete the work. Still-current
application-owned results must update origin/dataset availability/version and
locations exactly once through the current target; a reply for a retired
presentation-specific request must be discarded. Check completion during the
temporary gap with no presentation attached as well.

### 2.2 Add the native Windows screensaver entry

Add `.scr` packaging and full-screen, preview and configuration launch paths to
the existing native host. Reuse the map modules and the proven collector policy;
do not recreate a separate saver scene or simulation.

Verify: Windows selection/configuration and Preview; small preview resizes and
parent destruction; full-screen display coverage; normal input dismissal;
sign-in-on-resume; no focus/UAC prompts on activation; display sleep; restricted
collection context; and cleanup after repeated launches. Ensure an existing
application is not needlessly drawing behind the full-screen saver.
Keep the normal/background application running while the saver activates and
dismisses, then verify that it resumes current observations and dismissal leaves
any still-needed collector running. Repeat preview launches when full collection
is unavailable: show truthful coverage without an elevation prompt or sample traffic,
and release collection resources when their final owner exits.

### 2.3 Add click-to-explore on the same connection facts

Connect the ported hit testing and selection to a native connection details/list
view. Add the related map/table choices and layout behavior from TinyTorrent2.
Keep the authoritative snapshot, selected destination, row identity and highlight
mapping in their existing owners.

Verify: marker and arc clicks open exactly their contributing observations;
country estimates and supplied coordinates select the correct groups; multiple
applications sharing a destination remain distinguishable; unplottable flows
are available through All connections; updates/process exit do not retarget
selection; retiring a completed observation clears exact row selection without
pinning it; fading a live TCP mark retains its row. Clear/Escape restores the full
view; row selection highlights the map;
and focus/keyboard/split behavior remains usable. Background and screensaver
paths do not intercept these interactions.
Repeat the same-tuple reconnect with the ended connection's row selected:
selection does not move to its live successor and clears when the ended row
expires. A selected place remains selected while the successor or other
observations still contribute to it.

## Shared validation and resource measurements

Use TinyTorrent2's existing capture journeys as the reference: Tokyo and Buenos
Aires origins, Fiji/Russia at the cut, one complete longitude turn, Europe zoom
through 128x, Canadian islands at 32x, Norway at 128x, fast drags, and changing
observations during a drag. Use the same deterministic scenes, hardware, DPI
and viewport dimensions when comparing. Reuse available reference captures;
if new ones are needed, follow TinyTorrent2's own build instructions and keep
its source untouched.

Record preparation/draw CPU time, frame intervals and input latency separately
from GPU execution. Report median, p95 and worst frame/interval, pending rebuild
work, geometry/effect allocations, private bytes, working set and dedicated/shared
GPU memory. In stage 1 include settled, changing-data, resize and navigation
scenarios at 1080p and 4K, with 100, 200 and larger connection sets. Stage 2 adds
desktop mode switches, screensaver/preview launches and connection exploration.
There must be no unexplained regression against the matched reference and no
growth across repeated settled navigation or host-lifetime cycles.
For memory comparisons, record the source revision, release configuration,
dataset versions, scene and process scope. Repeat warmed runs with the same
detail, viewport and connection facts, recording settled values and transient
peaks. Use source map-hidden/map-visible measurements with otherwise unchanged
application state to estimate its contribution; disclose runtime retention and
measurement variation instead of treating that difference as exact isolation.
Measure NetMap's deterministic map path and then the incremental live collector
cost under a controlled workload, including trace buffers and retained flows.
Include all NetMap processes in product totals and record MMDB residency and
GPU allocations separately; a file mapping's size is not its resident memory.
Assess the results against the specification's
[resource acceptance criteria](../plans/rust-map.md#resource-acceptance).
Include close/dismissal while trace callbacks, origin HTTP and city installation
are active: measure prompt window removal, cleanup deadline/fallback initiation
and eventual OS process removal separately. Verify the last committed settings
and a usable dataset survive cancellation, replacement failure and fallback.

Use optimized x64 MSVC release builds and record the toolchain alongside locked
dependencies. Evaluate stripping, LTO and codegen settings against the actual binary;
do not sacrifice behavior to meet earlier guessed EXE figures. Run formatting,
Clippy and the focused tests, then complete native visual/interaction checks.

Report executable code size, bundled assets, optional MMDB data and any runtime
dependencies separately. The existing map asset alone is about 5.38 MiB; an EXE
embedding it cannot fit the earlier sub-3-MB estimates. A 4K BGRA surface alone
is about 31.6 MiB before swap-chain/effect copies. Memory acceptance is measured
against the reference baseline and the specification's resource criteria, not an
invented language-based number. Use measured costs to refine the distribution
budget without weakening the lower-memory objective.
