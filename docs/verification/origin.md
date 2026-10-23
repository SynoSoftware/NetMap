# Origin integration evidence

2026-10-23. This records the implemented origin path, not full-map or stage 1
acceptance. The source review is retained at
`V:/temp/netmap-origin-review/review.md`; its six passes and corrective delta
are clean.

## Implemented ownership

`app/origin.rs` owns Automatic/Manual choice, request identity, stale-result
rejection, dataset matching, network debounce and failure backoff. Application
waits for the accepted preference read before permitting discovery. Manual and
unreadable preferences start no public-address request. Saving a changed origin
uses the same owner as startup.

The registered Work origin worker owns native HTTP, local mapped lookup and
route/interface registrations. One active request and one command slot bound
outstanding work. A coalesced atomic network hint cannot be lost to a full reply
queue, and native callbacks neither send through that queue nor wait. Native
notification cancellation/drain stays on the worker; the process cleanup
deadline remains independently runnable.

Automatic uses `https://api64.ipify.org/`, accepts at most 128 response bytes and
looks up the returned address locally. Discovery has a 30-second total deadline;
failure retries after 5, 10, 20 seconds, up to 300 seconds. Route/interface hints
invalidate the old point immediately and debounce for two seconds. Installing
location data reuses the known address without another public request. Missing
coordinates use a visibly estimated bundled country anchor; an unknown origin
has no point. Settings explains that this is one overview egress and can differ
from individual VPN/proxy paths.

Viewport rebasing preserves a nonfitted geographic center/zoom and finishes an
active drag at its current position. It uses the existing native capture/update
path. Later mouse-up/cancel cannot restore coordinates from the old meridian.

## Observed checks

The owned native journey ran with:

```powershell
.\tests\native-settings.ps1 -Executable .\target\x86_64-pc-windows-msvc\debug\netmap.exe -EvidenceDirectory V:\temp\netmap-origin-native-b -AutomaticOrigin
```

This run used an isolated LOCALAPPDATA profile and the ordinary process token
outside the agent's restricted execution environment. Native route/interface
registration succeeded. Actual discovery plus local country MMDB lookup produced
`Automatic origin: Suriname (country estimate).` No public address or response
body was retained. Committed Manual superseded fresh Automatic work and stayed
accepted after subsequent worker delivery. Closing after starting another
Automatic request, then restarting with its saved preferences, both exited 0
within the harness's six-second process-removal check. The schedule permits an
in-flight request; it does not deterministically hold a native read pending.

The same journey passed search, raw draft retention, hidden/same-focus validation,
page/global reset with Cancel, failed-save retry, Apply/OK, queued close/reopen
and process restart. Twelve warmed reopen cycles held handles at 360, GDI at 36
and USER at 26. Closed/open private bytes both measured 65,855,488; each one-second
idle CPU sample measured zero. These are short whole-process hidden-map samples,
not full renderer/collector resource acceptance. `origin.png` and the Settings
screenshots remain in the evidence directory. Physical keyboard input was not
part of this run.

The first restricted-environment run at `V:/temp/netmap-origin-native-a` failed
discovery with WinHTTP `0x80072F99` (client certificate has no private key) and
showed retry backoff. Repeating the same application path outside that boundary
passed. Certificate validation and system proxy policy were not changed.

The final Rust suite passed 38 tests, with three existing explicit native/network
checks ignored by default. Added cases cover manual/failed-read suppression,
route bursts, stale results, dataset reuse, bounded retries, total discovery
timeout, country precision, drag rebasing and full reply queues. Review found
one shutdown gap: an already queued network hint could rearm the timer after
retirement. Application now rejects retired work before consuming hints, and
the regression asserts unchanged origin/view and no rearmed timer. Formatting
passes. Clippy still rejects the unused geography/viewport members awaiting
connection rendering; no warning suppression was added.

The subsequent combined Settings layout/origin journey at
`V:/temp/netmap-origin-layout-native-final/` repeated real Automatic country
lookup, Manual supersession, in-flight close/restart and the full Settings
journey against the integrated build. Both processes exited 0. Its additional
compact-layout scope and resource samples are in [Settings evidence](settings.md).
The integrated suite passes 39 tests with the same three explicit checks ignored.

## Remaining acceptance

Actual network-change callback delivery/debounce during a route or adapter
change remains unverified; no user routes, adapters or proxies were changed.
Native fitted/high-zoom/drag origin transitions, deterministic outstanding-read
cancellation, mixed-DPI/work-area/High Contrast/UIA and the loaded map resource
comparison remain open. Origin markers/guides await the connection renderer.

Primary contracts:
[ipify](https://www.ipify.org/),
[NotifyRouteChange2](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-notifyroutechange2),
[NotifyIpInterfaceChange](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-notifyipinterfacechange)
and [CancelMibChangeNotify2](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-cancelmibchangenotify2).
