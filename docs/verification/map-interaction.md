# Map interaction and static rendering

2026-10-23. This records an implemented normal-window feature batch, not stage 1
acceptance. Full collection, dark palette, matched visuals, accessibility and
resource acceptance remain open in the software specification.

## Implemented behavior

Application owns the pointed destination and selected place. Renderer consumes
that inspection without a second selection model. Hover and click use the
retained marker-first (12-DIP) and guide hit testing. A drag clears transient
pointing without choosing a place. Plain arrows cycle visible destinations and
reveal them; Ctrl+arrows retain the existing pan path. Plus/minus zoom around
the inspected place. Escape first cancels a drag, or clears inspection when no
drag exists. Selection is a place identity and clears when its final group
retires, rather than retargeting an unrelated destination after regrouping.
Selected rings draw above marker sprites. Unmapped observations remain counted.

Descriptions include grouped counts, activity availability and location
precision. A standard Win32 tooltip uses Host-owned callback text; current
description and keyboard help also populate the map HWND's native text. Native
status updates occur on accepted facts/input or graphics-mode changes, rather
than reconstructing descriptions during every animation-only frame.

Host reads High Contrast and client-area animation settings without changing
them, and handles system colour/theme/setting notifications. High Contrast uses
the system window/text colours, solid guides, opaque boundaries and no glow.
High Contrast and reduced motion replace gradual dimming and moving pulses with
static activity marks and the same removal deadline. Geometry remains retained.
Visible guides continue scheduling their fade even when their markers are out
of view. Guide drawing and hit testing share the single-longitude-turn bounds.
An undrawable temporary view produces no invalid transforms or recurring frames.

## Native evidence

```powershell
.\tests\native-connections.ps1 -Executable .\target\x86_64-pc-windows-msvc\debug\netmap.exe -EvidenceDirectory V:\temp\netmap-inspection-20261023-final -Inspect
cargo test --locked --target x86_64-pc-windows-msvc map::renderer::tests::native_active_and_idle_marks_draw_and_static_modes_stop_animation -- --ignored --exact --nocapture
```

The live journey passed with **153 observations**, including **43 unmapped**, a
successful controlled external TCP handshake, populated inspection text,
inspected-place zoom and Escape clearing. Its owned process exited with code
**0**, with empty stderr. Screenshot and JSON are retained in the named evidence
directory. It posts native focus/key messages through the real application loop;
this does not prove physical foreground keyboard or touch input. The ordinary
token still reports ETW access denial and unavailable traffic rates/UDP.

The renderer fixture passed on a hidden owned HWND with deterministic supplied
observations. It exercises the actual Direct2D path for mixed idle/active sprite
batches, bidirectional activity and glow, a 33-ms normal activity wake, three-second
static removal deadlines, supplied High Contrast background readback, selected
expired-mark exclusion from hit testing, and a 1-by-1-pixel view. It injects only
that fixture's appearance policy; it does not change Windows display, theme or
motion settings. Readback warms both flip-chain buffers before inspecting the
current target, because Present rotates the back buffer.

The ordinary suite passes **72 tests**, with **6 ignored** checks. The selected
native renderer check passes separately. Debug compilation and formatting pass.
Clippy with warnings denied remains blocked by existing unused code for the
remaining collection/exploration consumers; warnings have not been suppressed.

## Appearance and saved display options

The retained display batch adds one source palette authority, native Windows
accent/appearance reads on the existing startup worker, country activity shading,
cached DirectWrite speed text and a DPI-specific marker-ring realization. Color
refresh uses a channel with capacity one and no idle polling; shutdown disconnects
the worker's wait. No XAML/UI framework or async runtime was added.

Country shading uses the source's byte-precision blend, logarithmic strength and
1.6-second interpolation. Establishment age still controls idle shading expiry;
measured activity remains independent. Its transitions use the existing shared
frame schedule. Speed text consumes whole-PC snapshot rates, including unplottable
connections, and displays unavailable measurements explicitly. It retains layouts
between rate/width changes and honors each saved corner. Host supplies only actual,
visible navigation bounds, so hidden Fit controls do not displace the readout.

Independent six-pass review is clean at
`V:/temp/netmap-map-display-review/review.md`. It caught the hidden-controls
exclusion issue; the correction was independently read back. The extended native
GPU fixture passes with exact Light `[255,255,255]`, Dark `[44,44,44]` and supplied
High Contrast `[0,0,0]` background readback, different active/idle estimated-country
pixels, and removal of the idle highlight at its deadline. It also draws measured
receive/send layouts at each corner. That loop exercises native layout/drawing;
the actual corner placement is separately observed in app captures below.

`tests/native-connections.ps1` completed with saved `top-left`, `bottom-left` and
`bottom-right` preferences. Captures at
`V:/temp/netmap-map-display-{top-left,bottom-left,bottom-right}-20261023/`
show the readout at the chosen corner. The bottom-right run includes zoom and
inspection with visible Fit controls. The compact fitted capture at
`V:/temp/netmap-map-display-compact-20261023/` shows the bottom-right readout at
its 16-DIP margin with navigation hidden. Each owned process exited 0 with empty
stderr. The actual Windows color query supplied Dark in these runs; no system
theme or motion settings were changed.

The Settings journey passed against this build at
`V:/temp/netmap-map-display-settings-20261023/`: save/reset/reopen, search/raw
drafts, failed-save retry, compact reflow, queued close and restart. Twelve reopen
cycles retained 544 handles and 59 USER objects; GDI counts fell from 83 to 79.
Physical foreground keyboard and real Automatic origin options were not enabled.
The ordinary suite passed **83 tests**, with **8 ignored** checks; the extended
native renderer check passed separately. Clippy still reports the same six unused
production declarations for remaining consumers, with no added style diagnostics
or warning suppressions. These Debug runs are not matched resource acceptance.

## Native accessibility

The map uses IAccPropServices annotations on its existing HWND proxy. Name stays
`Network map`, AutomationId stays `NetMap.Map`, role is pane, and keyboard
focusability remains true. Current inspection or coverage/origin text and keyboard
instructions are copied into HelpText without retaining a model borrow across
COM/native calls. Annotations clear before HWND retirement; the UI COM apartment
outlives its interfaces.

An independent native UIAutomation client passed selected and Escape-cleared
queries at `V:/temp/netmap-accessibility-20261023/`, against owned app PID 23472.
It read name, ID, role, enabled/focusable state, positive bounds and the legacy
accessible pattern. HelpText changed from inspected United States connections to
the current capture/coverage summary. Both clients and the app exited 0. This
proves native property exposure/readback, not screen-reader use or physical key
input. The retained accessibility review is at
`V:/temp/netmap-accessibility-review/review.md`.

## Still unverified or incomplete

Physical keyboard/mouse/touchpad and mixed-DPI transitions; actual tooltip display
and screen-reader behavior; real High Contrast/motion setting changes;
dense-source visual/resource comparisons; and complete live
ETW/UDP/process-lifecycle coverage. Native text readback alone is not complete
UIA/MSAA verification. The deterministic fixture's supplied rates do not prove
the production collector's rate or pulse direction pipeline.
