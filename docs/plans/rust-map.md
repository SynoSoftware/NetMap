# NetMap software specification

Status: planned product; build/installer scaffold present. Written 2026-10-22;
reviewed 2026-10-23. World rendering and basic navigation are implemented;
live TCP plotting is integrated; full collection and map acceptance remain in progress.

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

Implementation is authorized through both planned stages, in their stated order.
Current implementation ownership is split into independent batches; the lead
alone builds and runs native checks against stable inputs. Shared contracts change
only through the lead. Architectural prerequisites settle before dependent work.
At each completed stage, one independent adversarial reviewer challenges its
requirements, execution paths, integration, ownership and material complexity.
Compatible fixes return to the same reviewer within affected scope; widen only
for concrete evidence. Independent implementation may continue while reviewed
inputs stay stable. Reserve `code-review deep` and its six specialists for the
whole integrated plan, then assign fixes to existing owners and rerun affected
checks. Retain concise review outcomes and unresolved risks here.

| Batch | Exclusive editing owner and files | Dependency / completion evidence |
| --- | --- | --- |
| Native collection correctness | Retained `etw_reference` agent: `src/connections.rs`, `src/connections/`, `src/app/work/trace_checks.rs`, `tests/native-trace.ps1` | Keep Snapshot/Publication and worker-launch contracts unchanged unless agreed with lead. Resolve System-context receive evidence without guessed process ownership; exact bounded native traffic and lasting identity/conservation checks. |
| Matched reference fixture | Retained `reference_baseline` agent: disposable `V:/temp/netmap-reference-20261023-a71c/` only | Repair the loaded-scene/fixed-extent fixture and provide one build/run handoff. No edits to TinyTorrent2, no operation of its running engine, no builds or native runs. |
| Application and native presentation integration | Lead: Application/Work/Host, persistence, resources, project docs and packaging scripts | Own shared interfaces, Release placement correction, integration, sole builds and all native execution. |

The collection owner must not edit application/hosting contracts, and the lead
must not edit collection-owned files while that batch is active. Reference work
has no product-source dependency. Reuse each agent for related follow-ups.

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
the lead's WM_SIZE-based restoration correction is implemented and awaits the
combined build/native rerun. Earlier Debug success is not Release acceptance.

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
