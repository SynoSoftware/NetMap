# Native source proof

Step 1.1 evidence, observed 2026-10-23 on Windows build 10.0.26200.0. The initial
disposable feasibility proof is distinguished below from the current production
collector checks. Complete whole-PC source acceptance remains open.

## ETW

Disposable proof tooling is at `V:/temp/netmap-etw-proof/`. It uses native ETW
and TDH through a C# diagnostic process; none of that runtime/tooling ships in
NetMap. The source and invocation are retained beside these reports:

- `ordinary.txt`: StartTrace returned 5 (Access denied), without owning a session.
- `elevated.txt`: after the user's approved UAC launch, the uniquely named
  SystemTraceProvider session started and stopped successfully. The consumer
  returned from ProcessTrace before callback allocations were released.

The eight-second elevated run generated same-process IPv4/IPv6 loopback TCP
transfers of 4,096 bytes and 2,048 bytes back, plus UDP datagrams of 777 bytes
and 333 bytes back. It observed 24 relevant events: TCP connect/accept/close and
both traffic directions for TCP and UDP, across both address families. Decoded
sizes matched those workloads, with zero decode failures, callback failures,
lost events or lost real-time buffers. Actual allocation was 17 buffers at
16 KiB each (272 KiB), not an estimate from requested minima.

An important schema distinction was demonstrated by controlled endpoint ports:
TCP v2 receive records put the receiving local endpoint in `saddr/sport`; UDP
v2 receive records preserve packet direction and put the local receiver in
`daddr/dport`. The [native decoder contract](../agents/architecture.md#native-collection)
owns normalization. Copying a generic receive swap would split one TCP lifetime
into two unrelated observations.

The final local `pair.txt` check used parent PID 1844 and its single owned child
PID 29552. IPv4/IPv6 TCP pairs were established before tracing. Before StartTrace
and again after tracing started while they remained idle, GetExtendedTcpTable
returned exactly one established row for each process and endpoint tuple.
Subsequent TCP sends of 4,096/2,048 bytes and UDP sends of 777/333 bytes were
attributed to the correct sender and receiver PIDs in both address families.
Twenty relevant events decoded without required-prefix/callback failure, event
loss or real-time buffer loss. Child, sockets, consumer and the owned trace all
finished cleanly. This proves two-process source attribution and table visibility;
it does not implement startup-race reconciliation in NetMap.

Remaining source gates: external-interface traffic, production startup
reconciliation, ambiguous-incarnation handling, event-loss behavior, close during
busy traffic and production callback/worker lifetime.
The follow-up eight-second `correlation.txt` capture recorded full TDH metadata
and bounded raw own-process payloads. It confirmed the exact `connid` property:
TDH declares an eight-byte pointer, but these records have only four zero bytes
remaining at that position. Lookup fails with 15005
(`ERROR_EVT_INVALID_EVENT_DATA`). The event-header PID was `0xffffffff`; payload
PID, endpoints and measured sizes decoded successfully. The run again completed
with 24 relevant events, zero required-prefix/callback failures and owned-session
cleanup. This does not establish a sanctioned alternate pointer layout or a
usable connection ID. Bounds checks must reject that suffix, not read past it.

Without usable correlation, process instance and tuple locate lifetime
candidates; they are not observation identity. Source timestamps may route an
event only to an unambiguous lifetime. A late event preceding a successor's birth
cannot update that successor. Equal times, missing lifecycle evidence and
uncertain tuple reuse leave the event unattributed, while measured bytes remain
in authoritative totals with a coverage limitation. Selection and lookup replies
still carry the application-owned observation identity. Same-process loopback
alone does not establish complete whole-PC support; a repeated identical ID
probe is not needed.

Production-adapter source inspection found an important clock distinction:
the disposable probe requested QPC in `Wnode.ClientContext`, but did not set
`PROCESS_TRACE_MODE_RAW_TIMESTAMP` on its consumer. ETW therefore converted its
callback timestamps to FILETIME units. The production consumer must request raw
timestamps and convert them, table capture bounds and publication times through
one QPC epoch/frequency; the probe's reported times are not raw-QPC evidence.
See Microsoft's [consumer timestamp contract](https://learn.microsoft.com/en-us/windows/win32/api/evntrace/ns-evntrace-event_trace_logfilew).
The verified v2 payload prefix contains no IPv6 scope/interface ID. A scoped
link-local event needs unique table/native scope evidence before matching an
existing scoped endpoint; zero must not silently merge ambiguous interfaces.

The native TCP table reader now being implemented uses OWNER_PID_ALL for both
families and SDK row sizes/offsets, bounds its allocation/retries, and retains
per-family failures instead of treating a failed read as absence. Microsoft
documents IPv6 scopes and ports in network byte order in
[MIB_TCP6ROW_OWNER_PID](https://learn.microsoft.com/en-us/windows/win32/api/tcpmib/ns-tcpmib-mib_tcp6row_owner_pid).
Its owned idle-loopback check now passes for IPv4/IPv6 client/server endpoints
and listener exclusion; see [Connections evidence](connections.md). Production
collection, process-instance allocation and ETW/table reconciliation are not
yet integrated.

Primary references inspected:

- [Owned system sessions](https://learn.microsoft.com/en-us/windows/win32/etw/configuring-and-starting-a-systemtraceprovider-session)
- [TCP events](https://learn.microsoft.com/en-us/windows/win32/etw/tcpip)
- [Microsoft's current kernel-event parser](https://raw.githubusercontent.com/microsoft/perfview/main/src/TraceEvent/Parsers/KernelTraceEventParser.cs)
- [TDH property lookup cost](https://learn.microsoft.com/en-us/windows/win32/etw/using-tdhgetproperty-to-consume-event-data)

### Production collector

The user-approved bounded Rust Raw and Collector checks now pass elevated,
including exact own/short-child IPv4/IPv6 TCP/UDP bytes, stable identities,
raw-QPC timestamps and owned-session/consumer cleanup. Ordinary launch still
receives Access denied. Physical incoming traffic exposed System-context payload
PIDs, requiring socket-lifetime correlation rather than an assumed application
PID. The correction and proof limits are recorded in
[Connections evidence](connections.md#elevated-production-collection-and-system-context-traffic).
Neither the controlled loopbacks nor the quieter corrected run prove complete
external/scoped/high-load collection or stage 1 acceptance.

## Native graphics foundation

The Debug executable was exercised with
`V:/temp/netmap-native-window.ps1` on the interactive desktop. It created the
hardware D3D11/DXGI/Direct2D target, filled/stroked-capable context, sprite batch
and blur/crop effects, loaded all 264 countries off the UI thread, resized from
an 800×600 to a 480×300 outer window, presented the expected clear colour, and
exited normally after WM_CLOSE. Map child extents were 784×513 and 464×213 at
96 DPI. Desktop pixel readback was `0x00F5F5F5`; a GDI child DC could not read
the DXGI presentation, and restricted desktop capture returned an invalid pixel.

Observed private memory was 65,400,832 bytes and working set 44,089,344 bytes;
two idle seconds added zero measured CPU milliseconds. These are GPU-window
foundation measurements, not full-map resource acceptance. Mixed-DPI movement,
device loss, full map drawing and loaded connection scenes remain unverified.

An independent foundation review found two lifetime issues: thread-message wakes
could be lost inside native modal loops, and an already queued reply could touch
retired presentation handles. The corrected path uses one application-lived
message-only HWND, rejects retired replies, and checks live presentation before
native refresh. The endpoint survives worker finalization. A native-dispatch
regression test covers acceptance and retirement. Review record:
`V:/temp/netmap-native-foundation-review/review.md`; its independent delta is
clean. Clippy also identified two safety-comment placements and Copy-receiver
conventions, which were corrected; unused ported model/navigation fields still
prevent warning-free stage acceptance.

The first native world capture is `V:/temp/netmap-world-first.png`. It shows
country fills/borders at 784×513 DIPs with the source geography, shared borders,
512-point border pieces, progressive tracing/realizations, clipped fill windows
and a 6-ms shared rebuild allowance. Thirteen focused tests passed, including
crop and bounded-piece cases. This Debug run settled at 80,277,504 private bytes
and 62,812,160 working-set bytes; its two-second sample added 15.625 CPU ms.
It resized and closed successfully. This is an intermediate empty-world result:
no measured connections, complete appearance modes or matched memory acceptance
is claimed. Its rendering review is now clean at
`V:/temp/netmap-world-review/review.md` after correcting final swap-chain release
before the D3D destruction flush.

### Basic navigation

`tests/native-navigation.ps1` exercised the Debug window on the interactive
desktop (owned PID 21136, normal exit 0). Evidence is under
`V:/temp/netmap-navigation-native-2/`. Menu and numpad zoom, double-click,
captured mouse pan and the native pan cursor worked. Escape released capture,
restored the arrow, and restored the same map pixels. Ctrl+Right then Ctrl+Left
also restored the same rendered view. Tab reached the native Fit button; wheel
input with that button focused zoomed the map. Fit and Ctrl+0 restored the
overview and map focus. This check uses real owned-window input; the desktop
must be left idle while it runs. A prior synthetic drag was cancelled by real
button-state messages, so it was replaced with actual input; another pixel
comparison was inconclusive and is not accepted evidence.
The final integrated Debug build repeated the journey successfully in owned
PID 13904, exit 0, with evidence at `V:/temp/netmap-navigation-native-final/`.

The independent six-pass source review is clean at
`V:/temp/netmap-navigation-review/review.md`. Five pure navigation tests cover
threshold/capture failure, capture reentry, successor-drag identity, cancellation,
fractional wheel anchoring and zoom limits. The current ordinary suite passes
21 tests, with three explicit native/network tests ignored by default. Unused
geography/model/lookup consumers still prevent warning-free Clippy.
Native mixed-DPI, precision touchpad, capture loss to another application,
inspection, UI Automation, Fit glyph and complete appearance checks remain open.

### Local datasets and city download

The country asset is DB-IP Country Lite October 2026, 8,348,509 bytes, copied
unchanged from the reference cache. Its SHA-256 and CC BY 4.0 attribution are
recorded in `assets/Attribution.md`. Local readers use `maxminddb 0.32.0` and
`memmap2 0.9.12`; file guards deny write/delete opens throughout each mapping.
A native file-sharing test verifies that protection, then writable access after
the mapping is released. A local lookup confirms the country code without
inventing a city coordinate.

Native WinHTTP uses one worker-local asynchronous request and fixed 64-KiB
buffer. Completion callbacks copy only small facts; per-callback/reserved Arcs
retain the channel until final HANDLE_CLOSING. Source review is clean at
`V:/temp/netmap-http-review/review.md`. Two explicit network tests passed in
owned PID 12072, exit 0: a complete public-address response (not logged) and
DB-IP country transfer cancellation between chunks. Logs are
`V:/temp/netmap-http-proof.out` and `.err`. Cancellation while a native read is
still outstanding remains a separate unverified timing case.

`tests/native-city-data.ps1` passed with a fresh redirected LOCALAPPDATA under
`V:/temp/netmap-city-native-20261023-b/`. The owned application cancelled an
active transfer with no installed/partial files, remained responsive to Zoom,
then downloaded, decompressed, fully verified and installed the October 2026
city database. It reopened that data after normal process close/restart. The
result is 126,998,165 bytes, SHA-256
`9E250F02722D1AD1780F88192F0E908AF285478A33E76B88B349C9181AE9D0F5`.
Peak sampled private bytes were 91,619,328; working set 196,931,584. The check's
map window stayed hidden, so these are dataset-operation costs rather than
complete visible-map resource acceptance. Verification touches mapped pages
and uses temporary offset sets despite streaming archive buffers. No real user
profile was changed. An earlier harness run completed installation but timed
out because its status matcher omitted Host's graphics prefix; that matcher
was corrected before the accepted run.

The explicit native transaction test
`blocked_replacement_preserves_version_and_backup_survives_restart` passed in
11.63 seconds against that owned city file. A second mapped reader blocked
replacement: prior location and version 41 remained, served from its flushed
backup. Removing the canonical file and restarting recovered that backup. A
subsequent successful installation reopened the canonical file, advanced its
new lifetime's version to 2 and removed the obsolete backup. Test fixtures were
removed; the downloaded B profile remains for reproducible checks.

The independent city review is at `V:/temp/netmap-city-review/review.md`.
Its progress-only GPU redraw finding was corrected: status replies update the
native status control, while accepted map changes request rendering. All source
passes and fixture/Box/cleanup delta are clean. The review caught a test-script
stale-HWND path on a second launch failure; scripts now clear handles between
runs and check the launched PID before reads or cleanup Close. Post-replacement
reopen-failure injection, callback timing, shutdown during commit, dataset peak
cost in the complete app and default-route origin remain open acceptance gates.

## Reference baseline

Current source map owners are clean at TinyTorrent2 revision
`95be5335eff5261d4f94b2e77a4bc0db0dcbf151`; their last map-specific commit is
`1faab3f7d244481fbeee8e66f08738de500f5036`. Existing captures are historical
evidence, with insufficient source/configuration/GPU provenance for a matched
optimized baseline. They must not be presented as that baseline.

A clean committed-source snapshot is prepared at
`V:/temp/netmap-reference-20261023-a71c/`. Only this disposable copy may be
modified to isolate IPC mutexes, pipe namespace, profile and diagnostic logs,
and add baseline instrumentation. It uses a separate real artifacts directory
and reads existing third-party libraries. The original checkout and running
TinyTorrent instance are preserved. The isolated Release build succeeded and its
bounded capture completed without failure. Evidence is under
`V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-521ce127-e990-4976-8056-92578d8d8a0b/`.

That capture is diagnostic evidence, not an accepted matched baseline: although
the containing windows were 1920×1080 and 3840×2160, the actual map was only
1898×314 and 3818×314 DIPs. No measured frame callbacks were observed. Private
memory settled near 243 MiB and 384 MiB respectively, but those values include
the reference application's other UI and retained resources. Hiding an already
created map does not isolate its allocations.

The corrected capture uses the source's maximized inspector, first-drawn-frame
acknowledgement and detach/restore resource lifecycle, with screenshots taken
only after measurements. It needed the interactive desktop and an unoccluded
owned window to receive Win2D frames. Evidence:
`V:/temp/netmap-reference-20261023-a71c/artifacts/evidence/Capture-peers-d644cbb7-5e02-4378-abdb-7f2be608cf14/`.
The 1080 case produced a 1898×933-DIP map and visible samples of 252,796,928
to 257,683,456 private bytes. Detached samples varied from 232,005,632 to
250,437,632 bytes; these include the rest of the source application and GC
variation. A screenshot confirms real world drawing, but no connection markers
or routes: the harness's intended 100-peer fixture did not remain in the active
model. Treat this as an empty-map reference, not a 100-connection baseline.

The requested 4K window changed extent during detach/restore (ultimately a
1712×1252-DIP map), so it is not a matched 4K baseline. Process GPU counters
were recorded in `V:/temp/netmap-reference-20261023-a71c/gpu-12176.csv`:
peak dedicated allocation 844,980,224 and shared allocation 53,710,848 bytes
on the active adapter. Those mixed-phase whole-process maxima cannot isolate
map contribution. Stable loaded fixtures, fixed actual extents and matched
per-scene GPU measurements remain acceptance work. The original running engine
was preserved; the isolated capture exited and stopped its own engine.
