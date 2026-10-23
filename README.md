# NetMap

A lightweight native Windows app that visualizes your PC's TCP/UDP connections
and live traffic on a world map, with planned desktop and screensaver modes.
Built in Rust with Win32, porting TinyTorrent2's map without its UI framework.
**Current state: development scaffold only.**
The executable opens an ordinary native window with a scaffold notice. The map,
collector and Settings dialog are planned and are not implemented yet.

The build has one Cargo package and one direct runtime dependency, `windows`,
with default features disabled and only used Win32 API features enabled.
Resources compile at build time. There is no UI framework, WebView, async runtime
or background polling in this bootstrap. Release builds use thin LTO, one codegen
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
pwsh -File .\scripts\build.ps1
pwsh -File .\scripts\build.ps1 -Configuration Release
pwsh -File .\scripts\check.ps1
```

Build prints the executable path. Scripts use `--locked` and the x64 MSVC target;
Cargo.toml/Cargo.lock own dependencies and `scripts/common.ps1` owns the scripted
target. `CARGO_TARGET_DIR` is respected through Cargo metadata. Direct Cargo use:

```powershell
cargo build --locked --target x86_64-pc-windows-msvc
cargo fmt --all --check
cargo clippy --locked --all-targets --target x86_64-pc-windows-msvc -- -D warnings
cargo test --locked --all-targets --target x86_64-pc-windows-msvc
```

Pure tests will live beside their implemented owners. The bootstrap currently
has no domain tests; a successful compile is not map or collection acceptance.

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
