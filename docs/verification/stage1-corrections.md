# Stage 1 integration corrections

The focused integration review and affected-contract follow-up are retained at
`V:/temp/netmap-stage1-review/review.md`. The reviewer inspected actual modified
and untracked implementation, not just the documentation snapshot commit.
Seven confirmed findings and two corrective follow-up paths are closed in source;
this is not complete Stage 1 acceptance.

The integrated corrections preserve the existing owners:

- Map Model classifies each observation once as receive-dominant, send-dominant
  or unknown. Ties, zero and unavailable rates are unknown. Renderer uses the
  source's five-DIP offsets, directional colors and five/eight-DIP selection
  rings; High Contrast uses system ink. Selected route resources get the shared
  rebuild allowance before ordinary routes and outlines.
- Work admits the five persistent roles alongside city installation, a pending
  preference save and the independently required final placement save. The
  bounded admission regression holds seven roles busy and verifies the final
  saved placement without waiting from a UI handler.
- View and the map context menu expose Countries/Cities through the existing
  preference save authority. Origin opens the existing Location settings page,
  including generation-checked reuse of an already-open dialog. Main status
  exposes the settings owner's error if a direct menu save fails.
- Host owns session/display-power registrations, independent availability facts
  and drawing pause/resume. Optional registration failure keeps the map usable
  with a monitoring warning. The flip-model swap chain does not return
  `DXGI_STATUS_OCCLUDED`; no unused occlusion/status-test mechanism is retained.
- An actual origin transition clears transient pointed inspection while keeping
  the selected place. Whole-PC transferring counts derive on the collector's
  publication pass from positive measured observation rates, including unmapped
  endpoints and counting bidirectional observations once. Location counts are
  shown only for the matching presented scene/snapshot/dataset generation;
  otherwise status reports that locations are updating.

## Integration-batch verification

The optimized x64 MSVC test executable
`target/x86_64-pc-windows-msvc/release/deps/netmap-04e4c18e08571f24.exe`
passes **93 ordinary tests; ten native/network checks remain opt-in**. The new
durable regressions cover marker-count conservation, origin hover/selection,
busy final-placement admission, measured transferring counts, independent
session/display/suspend facts and optional registration failure.
Toolchain: rustc 1.99.1 (`58cf8f9c3`, LLVM 23.1.1). Test executable SHA-256:
`799155B2BDA395264AADFDE52E01D799789758A5CD4613EBA6FC0AF53D1C99AA`.

The same Release executable passed
`map::renderer::tests::native_active_and_idle_marks_draw_and_static_modes_stop_animation`.
It reads actual Direct2D target pixels for three co-located directional marks,
verifies their five-DIP offsets and colors, and checks High Contrast ink at all
three positions. Existing palette, country-activity, expiry, static scheduling,
speed placement and undrawable-view checks also passed in that native journey.

Formatting passes. Clippy passes with `-D warnings -A dead_code`; the known
unused declarations remain an explicit unfinished integration gate rather than
source-level warning suppression. The resource measurement module compiles as
an opt-in Release check and has not yet produced an accepted matched comparison.

The current Release product build passed; its executable is 1,263,104 bytes.
Its updated Settings journey at
`V:/temp/netmap-stage1-settings-corrections-20261023/` passed with physical
keyboard input enabled, direct Origin opening Location, reuse of the existing
dialog while preserving its raw draft, and all search/reset/validation/save/
retry/reopen/restart checks. Twelve reopened dialogs left handles at 520, USER
objects at 34 and GDI objects at 37; the application exited with code zero.
These resource samples are lifecycle evidence, not matched performance
acceptance; the independent reference compilation ran during this UI journey.

The five-launch Release placement journey at
`V:/temp/netmap-stage1-placement-corrections-20261023/` preserved the exact
`(100,100,1000,700)` normal bounds, maximized/minimized restoration and off-screen
recovery. Every owned process exited zero with empty stderr.

The current foreground navigation journey at
`V:/temp/netmap-stage1-navigation-corrections-20261023/` passed physical pan,
capture/cursor, cancel, keyboard/zoom/Fit behavior and direct native detail menu
availability. Countries was checked; Cities was disabled without usable city
data, and a direct unavailable Cities command did not change committed detail.
Owned PID 26764 exited zero. A subsequent fixture-only correction restores its
saved cursor before closing the foreground window; it does not change product
behavior or the recorded navigation results.

Real session/display notifications,
dense selected-first rebuild timing, mixed DPI, precision touchpad, screen-reader
use and matched reference/resource acceptance remain separate native evidence
requirements. Source review and the scoped passes above do not establish them.

The subsequent retained-glow correction, updated build hashes and completed
first matched matrix are recorded in [matched map measurements](matched-map.md).
Those later checks supersede the resource module's pending status above without
retroactively changing the scope of the earlier UI journeys.
