# Connection model and native table evidence

2026-10-23. The pure observation model, native TCP reader and live application/map
delivery are implemented. The ordinary-token live map is observed below. Full
ETW/lifecycle coverage remains incomplete; this is not stage 1 acceptance.

## Implemented ownership

`connections/model.rs` owns application observation identities, TCP lifetime
candidates, UDP inactivity, measured-byte intervals, first publication and
retirement. Table capture intervals and complete versus partial passes constrain
correlation. A process episode is an explicit input; native allocation and
metadata are not established merely by the `ProcessId` type.

Measured totals include unattributed bytes. Ambiguous matching suppresses the
affected per-observation rates while retaining those totals. A provably retired
event cannot change its successor's rates. Delayed explicit closes can refine
inferred end bounds, but cannot overwrite a different explicit close. Source
times do not restart establishment age. Completed observations receive their
first publication even if they ended between samples, then retire independently
of selection or a delayed lookup. Live idle TCP facts survive their highlight.

`connections/tables.rs` reads the native owner-PID TCP tables for both families.
It uses SDK row sizes/offsets, normalizes endpoints through the same `Endpoint`
constructor, and preserves IPv6 scopes. A capture encloses both native reads;
per-family failures make it partial. Native buffers are bounded to 16 MiB per
family and three attempts. Listeners and terminal rows do not establish remote
connections; successful complete absence can retire an already observed one.
No table-derived bandwidth path exists.

## Observed checks

```powershell
cargo fmt --all --check
cargo test --locked --all-targets --target x86_64-pc-windows-msvc
cargo test --locked --target x86_64-pc-windows-msvc connections::tables::tests::native_capture_finds_both_idle_endpoints_and_excludes_their_listeners -- --ignored --exact --nocapture
```

Formatting passed. The final ordinary suite passed **66 tests**, with four
explicit native/network/storage checks ignored by default. This includes 23
connection-model tests and four native-table decoder tests. They cover short
connections, exact highlight boundaries, same-tuple successors, delayed/equal
timestamps, startup/table gaps, UDP inactivity and overlapping late evidence,
zero elapsed publication, unavailable versus zero rates, normalized/scoped
endpoints and conservation/order across 5,000 connections.

The separately selected native table test passed on the ordinary test process
token. It created only its own IPv4/IPv6 loopback listeners and idle client/server
pairs. Both endpoints of each pair appeared exactly once with this process's
PID and established state. At each listening address, only the accepted server
connection remained after listener filtering. Sockets closed with their owners;
the test process exited normally. This does not test external interfaces,
nonzero native IPv6 scope IDs, partial native failures or startup ETW races.

Final outputs are retained at:

- `V:/temp/netmap-connections-tests.txt`
- `V:/temp/netmap-connections-native-tables.txt`
- `V:/temp/netmap-connections-clippy.txt`

Clippy with `-D warnings` remains unsuccessful because unfinished production
consumers leave connection/native-table and existing geography/viewport code
unused. The final output contains only unused-code diagnostics, not a clean
stage acceptance result. No warning suppressions were added.

Independent six-pass source reviews and their merged corrections are retained
at `V:/temp/netmap-connections-model-review/review.md` and
`V:/temp/netmap-connections-tables-review/review.md`; both are clean. Review
caught two correction-created model regressions (conflicting explicit closes
and successor-rate masking), plus an incorrect listener/server assertion in the
native test. The final tests above ran after those corrections.

## Live map integration

`connections/collector.rs` owns source coordination and model publication.
`processes.rs` uses `SystemBasicProcessInformation` sequence numbers, supported
on this machine, without per-process opens. Two censuses bracket each TCP read;
only matching process instances supply attributed table rows. Retained census
continuity intervals resolve older network facts conservatively. Sequence
numbers are identity evidence, not process creation timestamps. Process
start/end/rundown integration is now implemented; its production native proof
remains pending as described below.

Work registers the aggregation, trace and location/model workers. The trace
consumer moves directly between workers; no trace creation, close, table read,
MMDB lookup or worker wait occurs on a window handler. One latest publication
slot replaces superseded snapshots, and the UI uses a nonwaiting take. Scene
requests are bounded and tagged; current dataset versions and request identities
gate acceptance. The location cache retains only current addresses. Unchanged
display facts reuse the prepared map model; navigation does not scan connection
observations. Source time is anchored before lookup, rather than restarted when
its scene arrives.

The ported map model now supplies country/coordinate grouping, 25-DIP region
merging, population-based placement and D'Hondt reach. The retained renderer
supplies split great-circle paths, gradient core/halo resources, shared glow,
dot bitmaps/sprite batches and establishment timing. Basic inspection and system
contrast/motion handling are now implemented; their exact observed scope and
remaining appearance/accessibility gates are in
[map interaction evidence](map-interaction.md).

The retained native command is:

```powershell
.\tests\native-connections.ps1 -Executable .\target\x86_64-pc-windows-msvc\debug\netmap.exe -EvidenceDirectory V:\temp\netmap-live-map-20261023-g
```

It opens only owned loopback sockets plus one idle TCP handshake to 1.1.1.1:443,
launches NetMap with isolated manual-origin preferences, observes repeated native
publication, resizes/captures its map and closes the owned process. On the current
Debug build it reported **144 observations**, hardware D3D11, a successful
external handshake, exit code **0** and empty stderr. The capture visibly shows
country destinations and guides. Evidence: `V:/temp/netmap-live-map-20261023-g/`.
Direct screen capture was unavailable; the fixture captured its owned map HWND
with PrintWindow. This does not establish foreground keyboard/touch behavior.

The first integrated runs exposed `D2DERR_WRONG_STATE` while drawing sprites.
Native draw tags isolated the error to the sprite stage. Drawing the prefiltered
dot bitmaps in aliased primitive mode and restoring the previous mode corrected
it; the same live-map command then passed. The native journey is retained as
the regression check, instead of adding a unit test that cannot exercise D2D.

Ordinary-token ETW creation returned access denied (5). The UI explicitly shows
TCP table observations and unavailable rates/UDP. Production elevated collection
was not exercised here. The coordinator currently withholds complete coverage
pending process lifecycle integration, even when a trace can start.
The live-map integration suite passed **72 tests**, with **5 ignored** checks at
that checkpoint (the later renderer fixture adds a sixth opt-in check). The
Debug native sample was 104,771,584 private bytes and 80,117,760 working-set
bytes; this single small-view sample is not optimized resource acceptance or a
matched source comparison. Warning-free Clippy remains open because remaining
traffic/exploration consumers are unfinished; no warning suppressions were added.

The existing native Settings journey also passed against this integrated binary:
`V:/temp/netmap-settings-live-map-20261023/`. Search, validation, raw drafts,
Apply/OK, resets with Cancel, failed-save retry, twelve reopen cycles, queued
close/reopen, restart and compact reflow passed while collection continued.
The sampled USER/handle counts were unchanged (56/534 before and after), and
the owned process exited with code 0. Foreground keyboard and real automatic
origin options were not enabled in that run.

## Process lifecycle and loss batch

The retained registry now combines native sequence-number censuses with ETW
process start/end/rundown facts. It preserves published process identities,
retains exits delivered before starts (including equal timestamps), binds births
inside a census after later continuity evidence, and keeps reused PIDs/object
keys separate. Rundown is observation evidence, not a claimed birth.

Loss invalidates trace continuity and persists a timestamp boundary across queued
network delivery. TCP bytes that could cross a missed replacement stay in
unattributed totals. Gap marking respects the boundary, so an old delayed opening
cannot mask an independently established successor. The existing application
status now consumes coverage, per-kind counts, directional rate availability and
ambiguity; its native status tooltip exposes the complete text.

The source review is clean at
`V:/temp/netmap-process-source-review/review.md`. The full ordinary suite passed
**83 tests**, with **8 explicit native checks ignored**. This includes lasting
regressions for exit ordering, census/birth overlap and queued loss/recovery.
Clippy still reports six unused production declarations awaiting their feature
consumers; the two new style diagnostics were corrected, with no suppressions.

`tests/native-trace.ps1` runs the raw adapter and integrated Collector fixtures
in separate owned hidden processes, capped at 25 seconds each. Each fixture has
an eight-second observation window. The integrated check requires exact own
IPv4/IPv6 TCP/UDP directional bytes, completed TCP retention, stable identities,
measured rates, and traffic from a short-lived owned child process. Normal and
caught-panic paths close the owned source and finish its registered consumer.
On timeout the wrapper terminates its retained parent process handle and stops
only the exact trace name announced by that PID; it does not enumerate traces
or terminate processes found by a potentially reused child PID.

Ordinary-token executions in
`V:/temp/netmap-production-trace-ordinary-20261023/` failed immediately with
Access denied / tracing rights unavailable, without owning a trace. Subsequent
user-approved elevated runs are recorded below.

The rebuilt Debug app separately passed `tests/native-connections.ps1 -Inspect`
at `V:/temp/netmap-source-status-20261023/`: 154 observations, truthful restricted
coverage, working map inspection, owned external handshake, exit 0 and empty
stderr. Private bytes were 116,207,616 and working set 97,304,576 in that small
view; these are neither Release resource acceptance nor a matched comparison.

## Elevated production collection and System-context traffic

Both eight-second production checks passed under the user's approved elevation
at `V:/temp/netmap-production-trace-elevated-20261023/`. Raw and Collector exited
0 without timeout; the raw check reported no decoder, callback-queue or native
loss, valid raw QPC, and 17 buffers of 16 KiB. The Collector verified exact own
and short-child IPv4/IPv6 directional bytes, ended TCP retention, stable process
instances and measured rates. These are controlled loopback results.

Passive traffic in that run exposed a separate physical-interface problem: over
99.8% of received bytes remained unattributed. The bounded aggregate diagnostic
at `V:/temp/netmap-production-trace-pids-20261023/` found 171,150,240 incoming TCP
bytes under payload PID 4, while PID 22444 had 163,841,424 outgoing bytes. Those
figures establish differing event contexts; they do not prove that every System
receive belongs to PID 22444. No foreign application was operated.

System-context TCP transfer/close events now require exactly one matching full-
endpoint lifetime in Model plus matching process-instance evidence in Processes.
The derived endpoint index retires with its histories. Missing evidence, birth
ties, competing owners and lifecycle gaps remain unattributed; genuine System
socket ownership is preserved. Unattributed bytes mark affected endpoint
lifetimes uncertain rather than unrelated sockets sharing a PID. Lasting tests
cover late delivery, changed owners, opening ties, loss/recovery, System-owned
sockets, index retirement and byte conservation.

The corrected Collector passed the same approved bounded check at
`V:/temp/netmap-system-owner-20261023/`, owned PID 29296, exit 0, no timeout.
Native samples now include attributed incoming application traffic. This run's
total traffic was far smaller than the earlier busy run, so its lower
unattributed fraction is not a matched high-load comparison. Full physical-
interface attribution acceptance remains open. The integrated ordinary suite
passes 87 tests, with nine explicit native checks ignored.

The opt-in external check at `V:/temp/netmap-external-source-20261023/` passed
exact owned TCP bytes (222 received, 61 sent) through both Raw and Collector.
Its receive events used the application's PID, not System. The follow-up at
`V:/temp/netmap-system-table-match-20261023/` found no TCP-table match in either
orientation for 2,628,580 passive System receive bytes. This does not justify
reclassifying them as UDP or assigning them to the torrent process.

The product now reports unmatched System TCP bytes as an explicit per-interval
attribution limitation, retaining captured aggregate totals/rates. The condition
clears after each publication. The integrated local-only Collector check passed
at `V:/temp/netmap-collector-integrated-20261023/`, owned PID 26076, exit 0, no
timeout, exact own/short-child TCP/UDP facts and no external request. The probe
now requires the literal environment value `1` for external work; an earlier
empty-value/presence bug had enabled the small HEAD request without `-External`.
Its regression rejects absent, empty and false values. Unknown System and scoped
IPv6 attribution remain explicit limitations, not claimed socket identities.

## Resumed elevated regression (2026-10-24)

`V:/temp/netmap-elevated-acceptance-20261024-a/collector-regression/result.json`
records both Raw and Collector passing on preserved optimized test
`5680939D1C26C4FBF94AF6CEBCAC18A85D68F6DABBD011C811CBE2F4E943DFEF`.
The approved elevated helper runs the unchanged eight-second checks separately;
neither times out and both exit 0. External work is false. These assert exact
owned IPv4/IPv6 TCP/UDP directional bytes, short-lived child facts, measured rates
and retained TCP identity under the existing fixture scope. The actual ordinary
viewer/elevated broker and all-process resource contracts remain separate.

The integrated Collector's whole test process is 1,904,640 private bytes before
construction, 3,407,872 after startup, and 6,651,904 at its sampled publication
maximum. After cleanup it is 2,158,592 bytes, 253,952 above baseline. Sampling is
not continuous and includes harness/workload/runtime retention; this is not
isolated collector ownership or a total product-memory acceptance claim.
Reported ETW buffers remain seventeen × 16 KiB (278,528 bytes); published whole-PC
observation counts range from 171 to 236. The owned child is outside that process
scope. Trace buffers and process Private Bytes remain distinct categories.

## Remaining integration

The resumed broker attempt is retained separately from Raw/Collector success.
Run `netmap-native-broker-20261024-a` stops before UAC because the wrapper uses
WindowsIdentity.Groups, which omits the logon SID. Native TokenLogonSid confirms
one group, and the wrapper changes to the product's documented native query.
Run b reaches matching ordinary/elevated logon, then hits three wrapper `==`
predicates (PowerShell requires `-eq`); its collector also exits 1 immediately.
Both failures remain preserved. Corrected run c, using diagnostic product
57F3A350, reports Windows error 24 through the retained standard stderr path.
The production token helper reports an eight-byte padded buffer for the
four-byte TokenElevation record. A direct read-only native comparison succeeds
with length four and fails with length eight/error 24; TokenLogonSid works with
either its exact forty-four or padded forty-eight bytes. The first correction
still fails in preserved test 4AD37A26: TokenElevation also rejects a null-buffer
size query with error 24 while reporting four required bytes. The final helper
uses a direct typed TOKEN_ELEVATION query and keeps the variable-length aligned
query only in logon_sid. Preserved optimized test 5B6F4ADF passes the native
current-token regression and all 107 ordinary tests (27 native opt-ins ignored).
This is an ABI query defect, not an authorization-policy failure.

Run `V:/temp/netmap-native-broker-20261024-d/` proceeds after user-approved
consent, using production 1EF9FF0A and test 5B6F4ADF. It proves actual measured
broker facts in all nine native table columns, but its full-screen saver exits.
The wrapper now launches the visible saver after ordinary startup, preserving
normal dismissal behavior. Run e cannot start its UAC helper inside the command
sandbox (0xc0000142); no collector starts. Run f outside that sandbox uses
packaged A461327D and test 5B6F4ADF: ordinary GENERIC_WRITE is rejected, actual
saver-only subscription survives 11.08 seconds, and collector 1 exits naturally
7.13 seconds after the saver process exits. Its replacement-source assertion
then fails on initial incomplete coverage. The bounded check now waits for
complete coverage, while still checking exact selection clearance on each new
epoch publication.

Preserved test 7925B3A0 passes 107 ordinary tests. Run g exposes another fixture
lifetime error: it finishes all Work while deliberately retaining accepted UI
facts, which can keep the retirement worker alive until the five-second fallback.
The existing fixture now transfers retirement join handles to the continuing
Work owner before finishing the old source. Product lifecycle behavior is
unchanged. Optimized test AA0DEEBB builds in 119 seconds, passes 107 ordinary
tests, and is preserved with its PDB under
`V:/temp/netmap-broker-retirement-20261024/`. Formatting/all-targets Release Clippy
pass. This last correction still needs its focused native check. Full
broker/source acceptance remains incomplete; preserve the partial successes and
all failed runs without treating them as a complete pass. The user requests
minimal screen disruption, so the remaining visible journey waits for an
announced attended batch.

Prove the implemented process lifecycle/rundown correlation alongside the current
census, using the owned ETW source and raw-QPC epoch described in
[native source evidence](native-source.md). Resolve scoped-event uncertainty
without guessing. Prove full coverage and measured traffic presentation; finish the
remaining retained map behavior. Native
external traffic, reuse, loss, busy-close, callback lifetime, publication latency
and total collection resource costs remain acceptance gates.
