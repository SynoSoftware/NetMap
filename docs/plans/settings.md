# NetMap Settings specification

Status: implementation in progress, 2026-10-23. The native shell, three lazy
pages, preference/draft rules and asynchronous recoverable persistence are
present. Native journey, layout/accessibility and runtime-effect acceptance
remain open; see the software specification's current implementation status.

This specification owns Settings content, search, reset, editing and command
behavior. The [map specification](rust-map.md) owns the settings' effects on the
product. [Architecture](../agents/architecture.md) owns code placement, work,
persistence and native lifetimes. Use the Goz source map below when implementing
the shell; the screenshot is a layout reference rather than a pixel contract.

## Purpose and scope

Use one native Settings dialog to configure the map, connection visibility and
location. Open it with **Settings…** from the normal window's menu. There is one
dialog per application; another invocation activates it. It is owned by the
interactive window and uses the native modal dialog loop on the existing UI
thread. Collection, worker replies and map painting continue while it is open.
In stage 2, the screen saver's `/c` entry opens this same shell and saved
preferences, using its validated supplied owner when present. Configuration
alone does not require a full-screen map or a new connection collector.

Stage 1 includes the complete shell: search and Clear, category navigation,
lazy child pages, page Reset, Reset all, OK, Cancel, Apply and a native status
bar. Stage 2 adds its actual hosting settings to this same shell. Create pages
only for available features; stage 1 has no empty or disabled future categories.

## Window and content tree

The tree below is the information hierarchy. The left navigation is a flat
native single-selection ListView, as in Goz; page sections are not nested
navigation nodes.

```text
Settings
|-- Search settings [edit]                         Clear [button]
|-- Categories [native ListView] | Selected page [native child dialog]
|   |-- Map                     | Heading: Map
|   |                           |-- Display
|   |                           |   |-- Detail: Countries / Cities [combo]
|   |                           |   `-- Speeds: Top-left / Bottom-left /
|   |                           |               Bottom-right [combo]
|   |                           |-- Note: Windows appearance and motion settings apply
|   |                           `-- Reset [page button, lower right]
|   |-- Connections             | Heading: Connections
|   |                           |-- New connections
|   |                           |   |-- Bright for: [seconds edit + up/down]
|   |                           |   `-- Fade out:   [seconds edit + up/down]
|   |                           |-- Traffic
|   |                           |   `-- Explanation: measured send/receive moves
|   |                           |       toward destination/origin; it does not
|   |                           |       restart establishment brightness
|   |                           `-- Reset
|   `-- Location                | Heading: Location
|                               |-- Map origin
|                               |   |-- Automatic / Manual [radio buttons]
|                               |   |-- Latitude / Longitude [manual edits]
|                               |   |-- Current origin and lookup state [text]
|                               |   `-- Automatic discovery disclosure [text]
|                               |-- Location data
|                               |   |-- Country dataset: version / attribution [text/link]
|                               |   |-- City dataset: absent / version / state [text]
|                               |   `-- Get city data / Update / Cancel [action buttons]
|                               `-- Reset
|-- Reset all [button]                         OK    Cancel    Apply
`-- Status bar: Settings - <page or search results> | <edit/save/operation state>

Stage 2 additions, when implemented:
Categories
|-- Desktop
|   |-- Enable desktop background [checkbox]
|   |-- Monitor [native combo of current displays]
|   |-- Explanation of Close-to-desktop and tray Exit behavior
|   `-- Reset
`-- Screen saver
    |-- Current installation/selection status [text]
    |-- Install or update / Open Windows screen saver settings [actions]
    |-- Preview [action using the same saver/map]
    `-- No editable preferences initially; page Reset is omitted
```

Every editor maps to an existing product choice. Map starts in Countries; Cities
availability follows installed usable city data. Keep the current committed
detail if its data becomes unavailable and show the product's country fallback;
explain the unavailable choice and provide the Location page route. Speed
placement keeps TinyTorrent2's default when ported, recorded with that source
revision. Do not invent a different default in the dialog.

Connections uses the defaults, duration rules and traffic behavior in
[Connection visibility and traffic](rust-map.md#connection-visibility-and-traffic).
Show seconds explicitly. Keep incomplete or invalid numeric text while editing;
validate parsing and arithmetic, including overflow, before acceptance. Up/down
controls are an input convenience; typed and pasted text receive the same checks.
The Traffic section explains behavior rather than adding an independent activity
timer or a second set of animation preferences.

Origin defaults to Automatic. Manual origin is a supplied latitude and longitude
in decimal degrees: latitude -90 through 90, longitude -180 through 180, finite
values only. Label north/east as positive. Manual fields are enabled only in
Manual mode; unfinished text survives mode and page switches and is validated
only when Manual is selected. Applying Manual suppresses automatic discovery;
returning to Automatic requests current discovery through the existing origin
owner. State that Automatic sends one public-address discovery request to the
specified provider, then performs local geolocation. Current origin and dataset
status describe accepted facts, distinct from the pending edits.

Dataset and screen saver buttons are explicit operations, separate from editing
preferences. Their labels and adjacent text explain that they act immediately;
Cancel does not uninstall a dataset or undo an operation already completed.
Dataset Cancel requests cancellation of that operation through its existing
owner. An operation continues if Settings closes, unless explicitly cancelled;
reopening shows its application-owned state. Preserve the previous usable data
on failure. Stage 2 installation/selection follows the
[screen saver contract](rust-map.md#windows-screensaver), including its permissions.

## Search and navigation

Search filters categories using page names, section headings, setting labels,
choice captions and a small explicit synonym list where useful (for example
brightness, dim, fade, traffic, origin and city data). Match the visible feature
vocabulary, not connection data, edit values or status/error strings. Keep this
small caption catalogue beside the page definitions; use it for native captions
and search so the two cannot drift. Future pages are not searchable.

Adapt Goz's deterministic matcher: split into letter/digit words, remove access
key markers, ignore case, and require every query word to prefix some word on
the page. Thus `fade` finds Fade out and `ade` does not. Empty or whitespace-only
input lists every page. Search is a small synchronous pass over this catalogue
on edit notification; it needs no worker, polling timer or debounce service.

Preserve the selected page if it still matches; otherwise select the first
matching page in normal category order. With no matches, clear navigation
selection, hide pages and show a native **No settings match your search** label.
Keep search, Clear and footer commands available. Search never deletes a draft,
changes settings or re-creates existing pages. Clearing restores all categories,
keeps the current page when possible, otherwise restores the last shown page
or Map, and focuses the search edit. Clear is enabled when the edit contains
text. Bold matching ordinary captions with a shared native font, following Goz;
headings retain their normal heading treatment. Native focus remains visible.

Initially show Map and focus the category list. Arrow keys select categories;
Tab/Shift+Tab traverse search, Clear, navigation, the shown page and footer in a
stable order. Hidden pages have no tab stops. Ctrl+F focuses and selects search
text; Escape follows native Cancel behavior. Clear has its own accessible name
and keyboard access. Enter in search moves focus to the selected category (or
stays in search for no results); elsewhere the normal default OK behavior
applies, respecting native combo/edit handling. No global keyboard hook is needed.

## Draft, validation and saving

Opening copies the committed editable preferences into one draft and baseline.
The draft, including invalid raw editor text, owns pending values across pages
and filtering. Controls display that draft; hidden controls are not a second
store. Dirty state is derived from differences against the baseline plus pending
invalid edits. Programmatic fills ignore their own change notifications with a
locally scoped fill guard.

| Command or state | Required result |
| --- | --- |
| Page Reset | Put only that page's editable preferences at canonical application defaults, clear their rejected text and refresh those controls. Other pages and operational facts stay intact. Enable when the page differs from defaults or has rejected text. |
| Reset all | Confirm “Restore all editable preferences to defaults? Choose Apply or OK to save.” Then reset every available page's editable preferences in the draft, including filtered/uncreated pages. Stay open; Cancel can discard this reset. |
| Apply | Enabled when edits are pending. Validate all applicable fields, then submit one complete accepted snapshot through the settings owner. Invalid input leaves the draft open with a field-specific message. |
| OK | With no edits, close. Otherwise validate and submit as Apply, then close only on successful commit. |
| Cancel, Escape or title-bar Close | Discard edits since the last successful Apply and close. Already accepted saves and explicit operations retain their application lifetime. |
| Saving | Show Saving…; disable preference editors, reset and commit commands until the reply. Navigation, search and Cancel remain usable. No UI-thread wait. Newly shown pages inherit the disabled state. |
| Save succeeds | Publish the committed snapshot, apply its runtime effects through the normal update path, and advance the baseline. Apply stays open; OK closes if the dialog still exists. |
| Save fails | Keep prior committed values/file and the complete draft. Show a concise persistent error and allow retry with Apply/OK; retain focus/input. |

Page Reset and Reset all never delete downloaded data, clear observations,
change window placement, install/uninstall a screen saver or directly change
Windows configuration. Session pan/zoom and selection are outside the draft.
Defaults live in the settings authority, not as parallel constants in pages,
resources and reset handlers. Future Desktop reset restores that page's default
preference (disabled); a hosting transition happens only after commit.

Validation chooses the first invalid applicable field in category/control
order. Clear a hiding search filter, select/create its page, show the reason
beside the field or in a native validation balloon, and focus/select the editor.
Do not silently clamp, replace invalid text or accept just the visible page.
Re-read accepted availability before commit; for example, a newly unavailable
Cities choice must be explained rather than accepted against stale page state.

Use a small explicit edit/save state (editing or saving an identified snapshot
with Apply/OK intent), rather than interacting saving/closing/applied flags.
Writes use the existing serialized atomic replacement path. Other saved facts,
such as window placement, remain owned by Application and cannot be overwritten
by a stale preferences draft. A close during saving detaches the dialog; its
accepted request still finishes at the application owner. Late replies never
touch destroyed controls, reopen Settings or close a newer dialog instance.
If reopened while a save is pending, the new dialog shows that application-owned
saving state and keeps preference edits disabled until it settles. It then reads
the current committed baseline; the earlier dialog's OK intent cannot close it.
Release borrows before modal confirmation, native control calls and callbacks.

## Native shell, layout and resources

Adapt Goz's `DIALOGEX` shell, `DS_CONTROL` child pages, native report ListView,
Edit, Button, radio, combo, up/down, Static, SysLink and status-bar controls.
Create each page on first display, retain it until Settings closes and show
only the selected page. Keep fonts and image lists alive until their controls
are destroyed. Route child notifications to the same draft/command path.

Use dialog units, system-scaled Segoe UI and measured captions. Goz's 400 by
349 dialog-unit shell is a starting reference, not a required empty size. Size
to the largest NetMap page and footer; measure the complete unfiltered category
list once so typing does not resize it. Use the screenshot's hierarchy and
spacing with compact native sections. Small bundled glyphs may accompany text;
their accessible meaning comes from labels. Keep icon resources proportional
to the actual commands and carry their notices.

At the minimum supported size, every editor and footer command fits without
clipping. At normal size, sections have readable spacing; at wider sizes, stretch
search, page and status content while preserving navigation/editor widths.
Make the native dialog resizable with this coordinated layout and a measured
minimum size. Recompute minimum/layout for DPI, font and display-work-area
changes; verify at 100%, 150% and 200% on the available work area. If a page cannot
fit at that work area, provide native scrolling for page content while search,
navigation and footer remain reachable. Do not shrink text to make it fit.

The status bar uses two parts: page/search context and the most relevant state
(validation/save error, saving, current explicit operation, unsaved changes or
Ready, in that order). Collection coverage remains in the main map summary; do
not invent Goz's service status for NetMap. Descriptive page status accompanies
disabled actions, absent data, progress and recoverable failure. Update only
changed text/enabled states; asynchronous status refresh cannot reset editor
text, caret, focus, scroll or the selected page.

Use native focus, access keys, accessible names and disabled states. Match the
application's supported Windows appearance/High Contrast contract and react to
system changes. Search bolding remains legible without color. Settings has no
animation or recurring presentation timer. Each native child paints itself;
use supported buffering and changed-position batching rather than a whole-window
canvas, backbuffer or blanket redraw suppression.

## Goz source map

Reference root: [TinyTorrent2/3rdParty/goz](../../../TinyTorrent2/3rdParty/goz).
Inspected 2026-10-23 at nested repository HEAD
`3a4cb4d402625cb45825310906d9261d90ccebb1`, with local modifications in the
relevant UI sources. This describes the inspected working tree, not a clean
commit snapshot. Before porting, re-read those files/diffs and record the actual
source revisions or content hashes copied, with applicable notices from
[LICENSE](../../../TinyTorrent2/3rdParty/goz/LICENSE).

Paths below are relative to that root. Adapt the named mechanisms into the
existing NetMap owners; source names are navigation aids, not required APIs.

| Source and entry points | Carry into NetMap | Deliberate adaptation |
| --- | --- | --- |
| `crates/gozler-ui/resources/gozler-ui.rc`: `IDD_SETTINGS`, `IDD_SETTINGS_GENERAL` and other `DS_CONTROL` pages | Search/Clear, page ListView, placeholder/no-match region, footer, native section/control spacing and page Reset placement. | Replace Goz pages and labels with the content tree above; add seconds up/down editors and NetMap layout. |
| `crates/gozler-ui/resources/ids.h`, `crates/gozler-ui/build.rs`, `src/core/ids.rs` | One resource-ID header used by RC and generated Rust IDs, duplicate/reserved-ID checks and native resource embedding. | Keep just NetMap's IDs/resources; Cargo owns version information. No Goz language-generation build pipeline. |
| `crates/gozler-ui/src/app/settings.rs`: `SettingsDialog`, `show_page`, `fit_page_list`, `search_changed`, `highlight_page`, `refuse`, `update_apply` | One shell, lazy retained pages, stable measured navigation width, filtered-page selection, native caption bolding, hidden-field validation routing and button availability. | Use one NetMap draft and application-lived asynchronous save path; retain invalid text on page switches. |
| `crates/gozler-ui/src/core/settings.rs`: `SettingsPage`, `SettingsDraft`, `restore_defaults`, `type_number`, `refused` | Typed page identity, baseline/draft separation, canonical page defaults and invalid-text representation. | Implement only NetMap preferences. Reset all edits the draft here; Goz's deferred whole-application reset on dialog close is not the requested semantics. |
| `crates/gozler-ui/src/core/captions.rs`: `settings_pages_matching`, `settings_page_hits`, `words` and their tests | Case-insensitive word-prefix search over captions, matching-caption emphasis and small deterministic tests. | Use NetMap's single page/caption catalogue; omit Goz's broad localization/caption infrastructure unless the product later needs it. |
| `crates/gozler-ui/src/win/dialog.rs`: `run_modal`, `create_page`, `size_buttons`, `size_page_buttons` | Native modal loop, initial focus, child-page creation, font lifetime and measured button sizing. | Port narrow operations into Host; a reusable all-purpose dialog trait/message framework is unnecessary for this shell. |
| `crates/gozler-ui/src/win/controls.rs`: `Edit`, `Button`, `IconList`, `StatusBar`; `src/core/layout.rs`: button/status sizing; `src/app/settings.rs`: `add_status_bar` | Relevant common-control operations, ListView buffering/selection, native status parts and measured layout. | Keep native wrappers at Host and Settings layout at its dialog owner. Add the small up/down operation needed here. |

For table rows using shortened `src/` paths, the base is `crates/gozler-ui/`.
Goz's service IPC, indexing, Include configuration, file operations, elevation
state, service polling timers, startup registry behavior and complete multi-crate
architecture are outside this port. Copy/adapt shell mechanisms, not those
features or a second worker/configuration system.

## Acceptance and implementation order

Implement as part of stage 1's real-data window, before stage 1 acceptance:

1. Establish the single preference/default/draft rules, page catalogue and
   resource IDs. Verify page-scoped/global reset, Cancel after reset, successful
   Apply then Cancel, inactive manual-field validation and rejected-text retention.
2. Port the native shell and lazy pages, then search/clear/no-match/validation
   navigation. Verify multiword prefix search, access-key captions, page switches,
   zero matches and a hidden invalid field without losing edits.
3. Connect commits and explicit operations to their existing application owners.
   Exercise save failure/retry, close during save, reopen before an old reply,
   city download failure/cancellation and an availability change while editing.
   Previous committed settings/data survive; no stale reply affects a new HWND.
4. Check keyboard-only operation, native accessibility, DPI/work-area layout,
   system appearance/High Contrast and resize/search/page-switch painting.
   Repeat the same interactions to verify clipping and flicker behavior.
5. Measure open/closed idle cost, page creation and repeated reopen/close handle
   counts. Search performs no I/O, status uses pushed replies, resources are
   released on close and the shell adds no recurring idle timer. Record results
   with the existing resource acceptance evidence, not as presumed source proof.

Stage 2 adds Desktop and Screen saver to the same catalogue, templates, draft
and command owners. Verify their hosting effects and explicit actions under the
stage 2 lifetime/permission contracts; no second Settings window or config store.
