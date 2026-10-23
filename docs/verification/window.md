# Normal-window placement

Observed 2026-10-23 with `tests/native-placement.ps1`, Debug x64 build, isolated
LOCALAPPDATA at `V:/temp/netmap-window-placement-20261023/`.

Five owned app launches passed normal-bounds save/restart, maximized save/restart,
closing while minimized from maximized, restoring the prior normal bounds, and
recovery of a saved rectangle at `(1000000,1000000)` onto an existing monitor.
All five exited 0 with empty stderr. The preference-file hash stayed unchanged;
the committed window file had no leftover rollback transaction.

Application owns placement outside the Settings draft. Work reads it before the
first show and performs the final write after close without blocking the window.
Host passes workspace coordinates between GetWindowPlacement and
SetWindowPlacement; Windows performs off-screen relocation. Pan/zoom remain
session-only. Preferences and placement share one recoverable storage mechanism,
with separate file/lock names so a stale Settings draft cannot overwrite bounds.

The existing Settings journey also passed after storage extraction at
`V:/temp/netmap-placement-settings-20261023/`, including raw drafts, hidden-field
validation, resets/Cancel, Apply/OK, failed-save retry, queued close/reopen and
restart. Twelve reopen cycles held 552 handles and 59 USER objects; GDI counts
fell from 87 to 79. Foreground keyboard and Automatic-origin options were off.
The ordinary suite passes 86 tests with nine explicit native checks ignored.
Formatting passes. Strict Clippy still reports six known unused production
declarations; checking with only dead-code warnings excluded passes. No source
warning suppression was added.

Physical mixed-DPI/monitor-removal transitions, changed DPI between launches,
end-session timing and matched Release resource acceptance remain unverified.
The native fixture simulates removed-monitor coordinates; it does not change
Windows display configuration. Formal review awaits the substantial deliverable
boundary.

## Release follow-up in progress

The initial Release package compiled with Inno Setup and correct copied asset
hashes, but its minimized-close journey exposed a real restoration defect:
GetWindowPlacement returned show state 2 with zero flags after minimizing a
maximized window. Evidence is at
`V:/temp/netmap-release-placement-diagnosis-20261023/result.json`.
The Debug pass above does not establish Release correctness.

Host now retains the last non-minimized WM_SIZE state rather than relying on
those flags. The native journey also waits for startup collection readiness
before changing placement, checks the requested bounds explicitly, and preserves
phase diagnostics on failure. This correction awaits the combined build/native
run while the collection owner completes its independent source batch.
