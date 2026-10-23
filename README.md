# NetMap

A lightweight native Windows app that visualizes your PC's TCP/UDP connections
and live traffic on a world map, with planned desktop and screensaver modes.
Built in Rust with Win32, porting TinyTorrent2's map without its UI framework.
**Current state: implementation in progress.**
The executable draws the bundled world in a native GPU window and loads its
geography away from the UI thread. Mouse wheel/double-click zoom, drag pan,
Plus/Minus, Ctrl+Arrow pan and Ctrl+0 Fit work with native menus and controls.
The Location menu downloads/updates optional city data, reports progress and
supports cancellation. Installed country/city data uses read-only memory mapping.
File > Settings opens a native searchable dialog with Map, Connections and
Location pages, draft resets, validation and asynchronous Apply/OK persistence.
Its city-data actions use the same application operations as the Location menu.
Automatic origin uses a bounded public-address HTTPS request and local location
lookup; a saved Manual origin suppresses discovery. Origin changes preserve the
geographic view during zoom/pan. Live connection plotting and basic inspection
are integrated. The map follows Windows appearance/motion settings, shades
country activity and displays whole-PC speeds in the saved corner.
Window position, restored bounds and maximized state survive restart separately
from editable preferences. The map exposes a stable accessible name and current
description through the native HWND accessibility proxy. Production
ETW proof and complete map acceptance remain in progress. Evidence is recorded in
[verification evidence](docs/verification/native-source.md).

The build has one Cargo package, native `windows` bindings and the small
`windows-numerics` value types, with default features disabled and only used
Win32 API features enabled.
Local MMDB decoding uses `maxminddb`/`memmap2`; streamed gzip decoding uses
`flate2`'s Rust backend. HTTP uses native WinHTTP with asynchronous cancellation
on registered workers.
Resources compile at build time. There is no UI framework, WebView or async
runtime. Release builds use thin LTO, one codegen
unit and stripped debug information. The x64 target statically links the C
runtime so installation needs no separate Visual C++ redistributable. Final
memory/CPU/GPU acceptance still needs
the complete renderer/collector measurements defined in the plan.

## Prerequisites

- Windows 11 x64.
- Rust through rustup; `rust-toolchain.toml` pins the toolchain, rustfmt and Clippy.
- Visual Studio Build Tools with Desktop development with C++ and a Windows SDK
  (MSVC linker, resource compiler and headers).
- PowerShell 7 for the scripts.
- Inno Setup 7 for installer compilation only.

No global PATH edits or administrator session are needed for compilation. Cargo
and the resource helper discover the installed MSVC/SDK tools. The first build
may download the pinned Rust toolchain and locked crates.

## Build and check

Run from the project root (the scripts also work from other working directories):

```powershell
pwsh -File .\scripts\prepare-assets.ps1
pwsh -File .\scripts\build.ps1
pwsh -File .\scripts\build.ps1 -Configuration Release
pwsh -File .\scripts\check.ps1
```

Asset preparation downloads and verifies the country database pinned in
`assets/country.json`. It runs separately from builds and checks.
Build copies available runtime assets beside the executable and prints its path.
Scripts use `--locked` and the x64 MSVC target;
Cargo.toml/Cargo.lock own dependencies and `scripts/common.ps1` owns the scripted
target. `CARGO_TARGET_DIR` is respected through Cargo metadata. Direct Cargo use:

```powershell
cargo build --locked --target x86_64-pc-windows-msvc
cargo fmt --all --check
cargo clippy --locked --all-targets --target x86_64-pc-windows-msvc -- -D warnings
cargo test --locked --all-targets --target x86_64-pc-windows-msvc
```

Direct Cargo builds do not copy assets. Run with `--assets <directory>` to use
the repository's `assets` directory. Pure geography and viewport tests live
beside their owners; passing them is not map or collection acceptance.

The interactive navigation check launches and closes only its own instance:

```powershell
pwsh -File .\tests\native-navigation.ps1 -Executable <built-netmap.exe> -EvidenceDirectory <output-directory>
```

It briefly gives that window focus to exercise native input and captures only
its map. Leave mouse and keyboard idle during the check.

The window-placement check uses an isolated profile to verify normal/maximized
restart, minimized close and recovery from off-screen saved coordinates:

```powershell
pwsh -File .\tests\native-placement.ps1 -Executable <built-netmap.exe> -EvidenceDirectory <new-output-directory>
```

The city-data check uses a new temporary profile, downloads roughly 60 MiB from
DB-IP, checks cancellation/installation/restart and retains the resulting data
as evidence:

```powershell
pwsh -File .\tests\native-city-data.ps1 -Executable <built-netmap.exe> -ProfileDirectory <new-temporary-directory>
```

The Settings journey uses an isolated temporary profile and no downloads:

```powershell
pwsh -File .\tests\native-settings.ps1 -Executable <built-netmap.exe> -EvidenceDirectory <new-output-directory>
```

It checks search, raw edits, validation, reset/Cancel, Apply/OK, save failure/
retry, narrow/compact reflow, scroll/focus, reopen, process restart and native
resource counts. Compact sizing is simulated on its owned window, without
changing display settings.
Optional `-Keyboard` requires its owned dialog to obtain foreground focus before
sending input. See [Settings evidence](docs/verification/settings.md) for the
observed results and remaining acceptance work.
The optional `-AutomaticOrigin` also exercises real public-address discovery,
local lookup, manual supersession and close/restart. It makes external HTTPS
requests but changes no adapters or routes; see [Origin evidence](docs/verification/origin.md).

## Package and installer

```powershell
pwsh -File .\scripts\package.ps1 -Scaffold
pwsh -File .\scripts\installer.ps1 -Scaffold
```

Each command builds Release and creates a unique output directory under
`artifacts/packages/`. Package produces an executable/notices folder and ZIP;
installer also compiles an unsigned Inno Setup executable from that exact folder.
It does not launch the installer or alter the running application.

Once the documented [runtime assets](assets/README.md) are ready, omit
`-Scaffold` to require and package them. Scaffold packages are development-only;
neither package mode certifies completion of the planned product. The
[installer folder](installer/README.md) owns setup sources and installation scope.

## Project documents

- [Product and stage requirements](docs/plans/rust-map.md)
- [Settings content tree and Goz source map](docs/plans/settings.md)
- [Architecture and intended project tree](docs/agents/architecture.md)
- [Implementation order and verification](docs/agents/development.md)
- [Porting source and optimization requirements](docs/agents/porting.md)
- [Domain vocabulary](CONTEXT.md)
- [Observed scaffold checks and memory](docs/verification/scaffold.md)

Create the planned modules as their responsibilities are implemented; the
scaffold does not add empty feature files or a second authority for their rules.
The sibling TinyTorrent2 checkout remains a read-only reference.
