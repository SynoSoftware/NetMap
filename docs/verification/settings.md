# Native Settings implementation evidence

2026-10-23. This is implementation evidence, not stage 1 acceptance. The
[Settings specification](../plans/settings.md) remains the behavior authority.

## Adapted source

Read-only Goz reference: `../TinyTorrent2/3rdParty/goz`, observed nested HEAD
`561452d1ef92287efa3d2ec91848d9a7030be1b8`. The initial specification described
an earlier HEAD with local edits; the actual consumed working-file SHA-256
hashes are:

| File relative to `crates/gozler-ui/` | SHA-256 |
| --- | --- |
| `resources/gozler-ui.rc` | `FD449AEBABCF54EB3E72A757A3F3DB3CB9FD63CD589400D5512F531E5BFC4E1C` |
| `src/core/captions.rs` | `1AD2B0FE4C06DA94D6E4D8E5E6568FE91404A125D4B62F89260E1892E8CCE86A` |
| `src/app/settings.rs` | `D88C84A44EDADAD2BDE2834A08887BC97261D65576E1DF2F1EC09D80A948C997` |

Adapted the DIALOGEX/DS_CONTROL composition, retained lazy pages, scoped fill
guard, catalogue word-prefix matcher, caption bolding and filtered selection.
NetMap has its own single raw draft and application-lived serialized save path;
no Goz application configuration, IPC, services or worker system was copied.
The MIT notice is shipped as `installer/notices/Goz-MIT.txt`.
Speed placement defaults to Bottom-right, from TinyTorrent2's
`app/src/Shell/InspectorPane.xaml.cs`, MapView.SpeedCorner, source revision
`95be5335eff5261d4f94b2e77a4bc0db0dcbf151`.

## Ownership and integrity

One application Settings owner serializes saves and publishes preferences only
after successful commit. Loading and saving run on registered Work workers;
search and editing perform no file/network access. The dialog has one raw draft
and baseline; hidden controls never become the draft store. Each shell registers
an application-generated identity and retires its HWND registration at destroy.
Replies go through the application endpoint. An earlier shell's OK intent cannot
close a later shell; reopened pending saves are observed without inheriting intent.

The versioned small UTF-8 file is `preferences.txt` under the existing NetMap
local data directory. It contains only editable preferences. Same-directory
temporaries are created exclusively and flushed before rename. The flushed
`preferences.rollback` snapshot remains authoritative until removal commits the
new canonical file. A failed/partial replacement or interrupted commit can thus
recover the previous accepted snapshot even if the new canonical file exists.
Accepted writes continue after dialog cancellation; process finalization retains
the existing bounded cleanup policy. A non-waiting exclusive open of the empty
`preferences.lock` file serializes other NetMap processes too. A contending
process gets a retryable error. Both startup reads and saves acquire this lock,
and a save reads the current disk snapshot before preparing rollback. This
prevents a reader from observing the replacement before rollback removal.
Native handle lifetime releases the
lock even after a crash; the file's continued existence never means locked.

## Checks so far

`tests/native-settings.ps1` launches only its owned executable with an isolated
LOCALAPPDATA profile. Its HWND reads, messages and cleanup validate process
ownership. Initial checks exposed two ambiguous test queries, not product
failures: `origin` also matches the Connections traffic caption, and `city`
also matches Map's missing-data explanation. Location tests now use `manual`.
The first immediate PrintWindow screenshot preceded completed native painting;
the screenshot helper now waits briefly and uses the normal native print path.

The accepted final journey for this increment at `V:/temp/netmap-settings-native-l/` passed
multiword prefix/no-match/clear, invalid raw text across hidden pages, validation
focus, Apply/reopen, page Reset followed by Cancel, inactive manual text,
blocked-save failure/retry and native scroll/focus reveal. It also passed
Reset-all confirmation followed by Cancel, OK commit, queued OK/Cancel/reopen,
same-focus validation after scrolling the editor away, and a real process
restart reading the saved value. Both owned processes exited 0. The queued
schedule does not deterministically hold the write pending until reopening.
Initial native restart verification once read the dialog before initialization
had finished; the harness now waits for visible/editable readiness, and commit/
reset controls start disabled in the template until the model enables them.

Twelve complete shell/page reopen cycles kept handles at 526 and USER resources
at 55; GDI resources fell from 95 to 87. Warmed closed/open private bytes both
measured 68,808,704. One-second CPU samples measured 15.625 ms closed and 0 ms
open. These short whole-process samples with a hidden map do not isolate first
page creation cost or establish full-map release resource acceptance. Screenshots
include ordinary Map/Location and short Location/validation views. No recurring
Settings timer is present in source.

The optional real-input run could not acquire foreground focus, both ordinarily
and through an interactive-desktop escalation. The ownership/foreground guard
stopped before sending input. Ctrl+F/Enter/Tab therefore remain unverified;
native focus messages used for scroll checks are not a physical Tab journey.
Pure/integrated Rust checks passed 28 tests with three pre-existing native/
network checks ignored by default; the final lock correction additionally passed
the six focused Settings tests. Formatting and diff whitespace pass. Clippy
still rejects only the pre-existing unused ported geography/viewport/lookup
consumers; no suppression was added.

Independent source review is retained at
`V:/temp/netmap-settings-review/review.md`. Its two confirmed findings were
missing noncancellable/failure operation status and incomplete scrolling/focus
reveal. Both are fixed. The corrective delta also found and resolved a
cross-process read/commit race by sharing the save lock with startup reads.
All six final source passes are clean. Scrolling now
moves every direct child HWND with the existing changed-position batch, including
children outside the viewport. This follows Microsoft's
[ScrollWindowEx restriction](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-scrollwindowex)
for partially clipped child controls. Native position, keyboard focus and
validation use the same scroll/reveal path.

## Work-area and compact layout increment

Settings now constrains its initial window and DPI/display-change suggestions
to the selected monitor's work area. Font replacement precedes DPI sizing so
the tracking minimum uses current font-derived dialog units. Normal size keeps
the existing category/page arrangement; narrow forms stack labels over fields
and measure wrapped explanations. At widths below the normal tracking minimum,
the same native category list moves above the full-width page and Reset all
moves to the reset row. Native page scrolling retains its own position. No new
control type or UI framework was introduced.

The final combined journey at
`V:/temp/netmap-origin-layout-native-final/` passes initial work-area containment,
all three pages at 550 scaled pixels and an owned simulated 400-pixel compact
width, horizontal control containment, footer nonoverlap and focus/scroll reveal.
The compact fixture explicitly bypasses normal tracking minimum and asserts the
actual resulting width. It does not change desktop settings or establish a
physical mixed-DPI result. Earlier run `native-i` was clamped to the normal
minimum and is not compact-width proof. `native-k` and the final combined run
use the corrected fixture. Physical keyboard input was not sent.

The combined run also repeats the complete prior Settings journey and real
Automatic discovery/local country lookup, Manual supersession and close/restart.
Both processes exit 0. Twelve reopen cycles retain handles at 382, GDI at 35 and
USER at 29. Closed/open private bytes measure 67,428,352/67,362,816; short idle
CPU samples both measure zero. As before, these are hidden-map whole-process
samples, not complete map/collector resource acceptance.

Native checks caught a new empty-label measurement stall during lazy Location
page creation. Empty text now needs no DrawText call; the complete page-switch
journey passes with that correction. Independent sizing review and its deltas
are clean at `V:/temp/netmap-settings-layout-review/review.md`. They also corrected
repeated combo resizing by using its collapsed native bounds and
[CB_SETMINVISIBLE](https://learn.microsoft.com/en-us/windows/win32/controls/cb-setminvisible)
for list extent, and footer/radio clipping below the normal minimum by reflowing
the existing shell. Final suite: 39 passed, three explicit native/network checks
ignored. Clippy still reports only the incomplete port's unused consumers.

Remaining: physical mixed-DPI/work-area transitions, High Contrast and UI Automation validation,
precisely scheduled close/reopen during a pending save, isolated creation cost,
attribution link action and downstream map detail/speed/connection effects.
Automatic/manual runtime origin now has separate
[integration evidence](origin.md), including a repeated complete Settings
journey against the integrated build. No full Settings or stage 1 acceptance
is asserted.
