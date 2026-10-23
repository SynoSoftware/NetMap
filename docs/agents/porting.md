# Porting TinyTorrent2

Mandatory when copying source/assets or changing map math, rendering, navigation
or optimization behavior. The [software specification](../plans/rust-map.md)
owns visible behavior and stage acceptance; this guide owns the required
reference and native porting mechanisms. Verify affected rows rather than
substituting a simpler permanent rendering path.

## Reference inspected

The sibling project is `C:/SynoSoftware/TinyTorrent2`. Its inspected map files
were clean at commit `95be5335eff5261d4f94b2e77a4bc0db0dcbf151`.
Check for subsequent map changes before implementation and record the source
revision actually ported. Keep TinyTorrent2 untouched.

Paths below are relative to that project:

| Source | What to carry over |
| --- | --- |
| `app/src/Torrents/MapModel.cs` | Observations, grouping, destination identities, estimated placement, descriptions and selection marks. |
| `app/src/Torrents/MapViewport.cs` | Projection, longitude wrapping, fitted scale, latitude correction, pan, anchored zoom, bounds and reveal. |
| `app/src/Torrents/MapRenderer.cs` | Drawing order, geometry, resource ownership, effects, hit testing and every existing optimization. |
| `app/src/Torrents/PeerMap.xaml.cs` | Preparation coalescing, observation-to-frame flow, inspection, appearance updates and resource lifetime. |
| `app/src/Torrents/PeerMap/Navigation.cs` | Mouse and keyboard behavior, drag cancellation, focus and navigation controls. |
| `app/src/Torrents/PeerMap.xaml` | Map control layout, palette roles and compact zoom/Fit controls. |
| `lib/Lucide` | Existing map-control glyphs and their font/licence resources, where needed for visual parity. |
| `app/src/Shared/Geography.cs` | Exact `world.bin` decoding and shared country geometry. |
| `app/src/Shared/Geolocation.cs` | Local MMDB lookup, country/city precision and dataset replacement behavior. |
| `app/src/Assets/world.bin` and `app/src/Assets/Attribution.md` | Existing geography asset and its provenance. |
| `docs/contracts/peer-location.md` | Map behavior, estimated distribution, arc treatment and navigation contract. |
| `docs/contracts/architecture.md`, peer-map ownership section | Proven division between model, viewport, interaction and rendering. |
| `docs/contracts/ui.md`, Peers section, and `app/src/Resources/en.json`, `peer_map` | Existing map commands, descriptions, speed placement and accessible interaction. |
| `app/tests/DistributionChecks.ps1` | Existing estimated-distribution cases. |
| `app/src/Shell/MainWindow/CapturePeers.cs` | Synthetic scenes, geographic edge cases, navigation and performance journeys. |
| `docs/images/full-screen-map.png` | Visual reference; source and current behavior take precedence where the image is older. |

The inspected `world.bin` is 5,641,035 bytes, SHA-256
`C27F41D8C00E87AA71ECE3A0A4134B9E6779D15302922207B8956BB2272023D3`.
Reuse it unchanged initially. Preserve its shared sections, country codes,
anchors, populated places and simplification levels. Decode .NET's length-prefixed
UTF-8 strings and little-endian numbers explicitly; do not assume a Rust struct
has the file's layout. Carry the applicable source notices and asset attribution
with copied or translated material.

For the native Settings shell, use the
[Goz source map](../plans/settings.md#goz-source-map). It identifies the dialog
templates, search/reset/draft mechanisms, native controls and build resources
to adapt, including the inspected working-tree provenance and scope exclusions.
Its content and behavior belong to that specification; Goz is not a runtime
dependency or a reason to duplicate NetMap's owners.

## Native port, not a second design

The ownership and algorithms are reusable; the C#/WinUI object model is not a
runtime dependency. Translate the existing math and retained state into ordinary
Rust values. Replace property-change notifications with direct owner calls and
one pending UI preparation, rather than reproducing a binding framework in Rust.

| TinyTorrent2 mechanism | Native Rust equivalent |
| --- | --- |
| Win2D `CanvasControl` | Win32 map HWND, Direct3D 11 device with BGRA support, DXGI swap chain and Direct2D device context targeting its surface. Use the native device-context path required by the existing effects and realizations. |
| `CanvasPathBuilder` / `CanvasGeometry` | `ID2D1PathGeometry`, geometry sink and transformed geometry; retain the source's coordinate system and alternate-fill rules. |
| `CanvasCachedGeometry` | `ID2D1GeometryRealization` filled/stroked realizations through `ID2D1DeviceContext1` or later, with the same scale-aware tolerance and lifetime. |
| `CanvasCommandList`, Crop and GaussianBlur | Direct2D command lists and native effects, preserving the shared glow cache and its invalidation inputs. |
| Arc compositing layer | Native Direct2D layer with group opacity and radial opacity brush, so the source's 80% cap and origin mask apply after the arcs are combined. |
| Marker sprite batch | `ID2D1SpriteBatch` through `ID2D1DeviceContext3` or later; reuse dot bitmaps and restore drawing state after the batch. |
| `DispatcherQueueTimer` / `Invalidate` | UI-thread invalidation and one shared timer while interpolation, the specified establishment fade/traffic pulse or pending rebuild work needs frames. Coalesce invalidations; retain direct redraw on navigation. |
| XAML text, theme brushes, Fit button and flyout | DirectWrite map text, the source palette roles, a focusable native Fit command and native popup/menu commands. Port the visual placement and semantics without requiring WinUI. |
| XAML pointer/focus/automation | Native pointer capture, focus, tooltip and UI Automation exposure of the map, its help/selection, and actionable controls. Drawing similar pixels alone does not retain keyboard/accessibility behavior. |

Retain a single device/renderer per active presentation process, with reusable
resources below it. On resize/DPI change, detach the swap-chain target before
resizing its buffers, recreate size/DPI-dependent resources and keep valid
geographic paths. Handle device-removed/reset through the renderer's resource
lifetime, not by restarting the connection model. Hardware acceleration is the
normal path; if a software device is needed, use the same renderer and report
that mode in diagnostics instead of introducing another drawing implementation.

Keep the source's numeric precision and matrix composition order: geographic
locations use doubles where the source does; viewport vectors and GPU geometry
use floats. C# `MathF.Round` uses midpoint-to-even, while Rust `round()` uses a
different tie rule. Use `round_ties_even()` for the ported wrap, outline-detail
and width-quantization calculations. Verify positive/negative half-turns,
quarter/half-width thresholds, zero-sized views and nearly coincident/antipodal
routes before accepting numerical parity. Do not add arbitrary epsilons or a
different projection to make mismatched tests pass.

## Optimization parity checklist

Every row is required porting work. A Rust rewrite is not complete merely
because it draws similar outlines. Port the native equivalents and verify their
lifetime and invalidation behavior.

| Existing optimization | Required Rust behavior |
| --- | --- |
| Encoded outline detail | Keep per-vertex simplification levels; select half-DIP detail in powers of two. Shared border sections are stored once and drawn once. |
| Retained observations | Ignore identical samples; locate only new addresses or changed datasets; format location descriptions lazily. |
| Preparation outside drawing | Coalesce observation/resize changes; do not arrange groups or publish model changes from drawing, hit testing or pan. |
| Stable destination identities | Reuse routes, arc paths, brushes and displayed activity by destination key. Release removed routes instead of retaining old observations indefinitely. |
| Geographic geometry | Retain paths in map coordinates. Apply view transforms rather than rebuilding every path on pan or resize. |
| Visibility culling | Cull outline extents, border pieces, routes including glow margins, and markers before drawing; test the relevant longitude turns. |
| Bounded border pieces | Keep 512-point piece grouping and lazy path construction so one large border does not become an unbounded stroke rebuild. |
| GPU geometry realizations | Use filled/stroked Direct2D geometry realizations, not plain geometry drawing on every frame as the permanent path. |
| Shared rebuild allowance | Preserve the 6 ms per-frame preparation allowance for initial tracing, detail changes, realizations and route brushes. This is an allowance checked between operations, not a hard whole-frame deadline. |
| Progressive rebuild priority | Prioritize selected routes, then missing strokes, then stale strokes. Draw valid older resources while replacements wait and request another frame only while work remains. |
| Scale-aware strokes | Widen after both axis scales, retain the scale with the realization and compensate its drawing transform. Preserve 5% route and 10% border scale tolerances. |
| Border ink stability | Preserve the half-DIP/one-physical-pixel minimum and inverse-opacity compensation while a cached stroke has an older scale. |
| Bounded fill cache | Preserve the three-view clipping window and half-view movement threshold, longitude-turn reuse, Sutherland-Hodgman cropping and alternate fill for holes. |
| Fill correctness during fast pan | Retain whole geometry where the country fits or spans two turns. Replace a clipped fill that no longer covers the visible country even when the allowance has expired. |
| Per-route paint | Reuse core/halo radial gradients and geometry realizations. Adjust brush transforms and opacity; rebuild only for the source's actual colour, scale or width changes. |
| Width quantization | Preserve quarter-DIP core-width and half-DIP halo-width steps to avoid rebuilding every interpolated width. |
| Cached shared glow | Retain the command list and cropped blur output while view, size, palette, arrangement and its activity inputs are unchanged. Invalidate after deferred geometry replacement. Keep the new directional pulse in sharp guide ink so its phase does not rebuild the blur cache. |
| Batched markers | Reuse white dot bitmaps, tint them in a sprite batch, and reuse realized selection rings at each radius. Do not issue a new bitmap allocation for each destination. |
| Event-driven frames | Redraw on accepted changes, navigation, inspection, appearance, pending rebuilds, active interpolation, establishment fades or measured traffic pulses. Share the source's 33 ms timer; stop it when those visible effects settle. Static holds use the next deadline wake, not a permanent animation loop or a timer per connection. |
| Explicit resource lifetime | Release device resources on device loss/shutdown; release unused native resources when presentation stops. Keep plain map state independent of COM/GPU lifetime. |

## Technical references

- [Rust for Windows](https://learn.microsoft.com/en-us/windows/dev-environment/rust/rust-for-windows)
  for native bindings.
- [Direct2D device contexts](https://learn.microsoft.com/en-us/windows/win32/direct2d/devices-and-device-contexts),
  [geometry realizations in Rust](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Graphics/Direct2D/struct.ID2D1DeviceContext1.html)
  and [Direct2D sprite batches](https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/Direct2D/what-s-new-in-direct2d-for-windows-8-consumer-preview.md)
  for native counterparts of the current Win2D optimizations.
- [SetParent](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setparent)
  for window style and cross-process DPI behavior. This documents reparenting,
  not Explorer's live-wallpaper arrangement.
- [Lively's desktop host source](https://github.com/lively-community/lively/blob/core-separation/src/Lively/Lively/Core/WinDesktopCore.cs)
  as implementation evidence for Explorer hosting behavior, not a dependency
  or source to copy into NetMap.
- [GetExtendedTcpTable](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getextendedtcptable)
  and [GetExtendedUdpTable](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getextendedudptable)
  for socket-table capabilities.
- [TCP ETW events](https://learn.microsoft.com/en-us/windows/win32/etw/tcpip),
  [UDP ETW events](https://learn.microsoft.com/en-us/windows/win32/etw/udpip),
  [UDP endpoint/PID/size fields](https://learn.microsoft.com/en-us/windows/win32/etw/udpip-typegroup1)
  and [SystemTraceProvider sessions](https://learn.microsoft.com/en-us/windows/win32/etw/configuring-and-starting-a-systemtraceprovider-session)
  for the selected collector approach and privilege requirements.
- [TCP connect/accept correlation fields](https://learn.microsoft.com/en-us/windows/win32/etw/tcpip-typegroup2)
  and [receive/disconnect correlation fields](https://learn.microsoft.com/en-us/windows/win32/etw/tcpip-typegroup1)
  document connection identifiers for correlating native records; observation
  identities still distinguish retained and current TCP incarnations.
- [ExitProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-exitprocess)
  and [TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess)
  distinguish normal-exit deadlocks from own-process termination and pending-I/O
  constraints; neither establishes an unconditional kernel-teardown deadline.
- [ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)
  documents replacement failures that must retain or recover the prior usable
  settings/dataset rather than assuming a failed replace leaves its path intact.
- [ProcessTrace](https://learn.microsoft.com/en-us/windows/win32/api/evntrace/nf-evntrace-processtrace)
  and [CloseTrace](https://learn.microsoft.com/en-us/windows/win32/api/evntrace/nf-evntrace-closetrace)
  for blocking-consumer and pending-close behavior; shutdown must retain callback
  ownership until the consumer actually finishes.
- [Direct2D layer opacity and mask](https://learn.microsoft.com/en-us/windows/win32/api/d2d1_1/ns-d2d1_1-d2d1_layer_parameters1)
  for group compositing, and [Rust float rounding](https://doc.rust-lang.org/std/primitive.f32.html#method.round_ties_even)
  versus [MathF.Round](https://learn.microsoft.com/en-us/dotnet/api/system.mathf.round)
  for numeric conversion parity.
- [Windows screensaver behavior](https://learn.microsoft.com/en-us/windows/win32/lwef/screen-saver-library)
  and [screensaver settings](https://support.microsoft.com/en-us/windows/configure-a-screen-saver-in-windows-a9dc2a0c-dc8e-9161-d270-aaccc252082a)
  for the stage 2 Windows integration.
- [ipify's documented public-address endpoint](https://www.ipify.org/)
  for the default-route origin proposal; endpoint geolocation remains local.
- [Rust MMDB reader](https://github.com/oschwald/maxminddb-rust)
  and the local TinyTorrent2 geolocation contract for dataset handling.
- [Cargo release profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
  for measured release-size tuning.
