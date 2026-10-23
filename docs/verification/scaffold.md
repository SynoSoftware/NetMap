# Scaffold verification

Observed 2026-10-23 on Windows 11 x64, OS build 10.0.26200.0. This is bootstrap
evidence, not acceptance of the map, collector, Settings or desktop host.

## Build and packaging

- Pinned Rust 1.99.1, rustfmt and Clippy are installed. Cargo.lock is present.
- `scripts/check.ps1` passed formatting, Clippy with warnings denied, and Cargo
  test. There are zero domain tests at this stage.
- Debug and Release compilation passed, including Windows RC compilation,
  embedded manifest, generated resource IDs and Cargo-derived version 0.1.0.
- Build/installer scripts ran successfully from `V:/temp` and preserved the
  caller's working directory. Cargo metadata respected a target-directory
  override containing spaces.
- Normal packaging correctly refused missing `assets/world.bin` before building
  a distribution. `-Scaffold` successfully staged the executable/notices and ZIP.
- Inno Setup 7.1.0 compiled the setup from that exact payload. The installer was
  not executed; install, upgrade and uninstall behavior remain unverified.
- PowerShell AST parsing and manifest XML parsing passed. The skill's regex
  PowerShell checker reported no dangerous operations. Its warnings about
  `foreach`, `--output-dir` and missing per-file error policy are false positives:
  these are language syntax/compiler arguments, and common.ps1 supplies the
  shared terminating-error policy.

The final scaffold installer is under
`artifacts/packages/20261023-141948-956-3eaed133/`. The setup is 2,942,418 bytes;
the ZIP is 122,956 bytes. Generated artifacts are ignored by Git.

The project was subsequently named **NetMap**. Formatting, Clippy, Cargo test
and Release/installer compilation passed again with `netmap.exe`, the NetMap
product resources and `installer/netmap.iss`. The renamed installer is under
`artifacts/packages/20261023-144124-068-10ab9c9d/`. Artifact/probe names and memory
figures below retain the original run's provenance; memory was not remeasured
for this naming change. The checkout directory remains `C:/SynoSoftware/InetMap`.

## Runtime and memory

The temporary probe launched only the Release bootstrap with a hidden window,
found its native dialog/Static child by the launched process ID, resized it from
800 to 480 pixels wide and observed the child width follow the client area.
It then took eight samples at 250 ms intervals and posted WM_CLOSE to that
instance. Exit completed within the probe's five-second bound with code 0.
The probe is retained at `V:/temp/inetmap-bootstrap-smoke.ps1`.

| Measurement | Observed value |
| --- | --- |
| Release executable | 227,328 bytes (222 KiB) |
| Private bytes across samples | 4,706,304 bytes (about 4.5 MiB) |
| Working set across samples | 26,894,336–27,000,832 bytes (about 25.7 MiB) |
| Process CPU consumed during the two-second idle sample | 0 ms at the process counter's resolution |
| Direct PE imports after static CRT linking | kernel32, user32, ntdll, oleaut32 and Windows synchronization API set |

There is no separate VC runtime DLL import, bundled UI framework, WebView or
async runtime. These process measurements include Windows loader/control costs;
working set and private bytes are different quantities. The bootstrap owns no
worker threads, polling timers, datasets, map geometry or GPU resources. Hidden
window samples do not establish visible rendering cost, long-term idle behavior,
final desktop memory use or the required reduction against TinyTorrent2.

The [resource acceptance contract](../plans/rust-map.md#resource-acceptance) still
requires matched map workloads, collection overhead, mapped data residency and
dedicated/shared GPU measurements once those owners are implemented.
