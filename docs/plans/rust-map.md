# NetMap software specification

Status: planned product; build/installer scaffold present. Written 2026-10-22;
reviewed 2026-10-23. Map, collection and Settings are not implemented yet.

This document owns intended product behavior, scope, technical constraints and
acceptance criteria. It does not claim that implementation or platform checks
already exist. [CONTEXT.md](../../CONTEXT.md) owns the domain language.

Mandatory implementation guidance lives in [Architecture](../agents/architecture.md),
[Porting](../agents/porting.md) and [Development](../agents/development.md).
Those documents own module organization, native mechanisms and working
procedures; this specification owns the result they must deliver.
The [Settings specification](settings.md) owns the dialog's content tree, search,
resets, editing behavior and Goz shell porting requirements.

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
