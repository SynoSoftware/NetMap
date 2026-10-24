# Installer

`netmap.iss` owns the Inno Setup installer. Compile it with
`pwsh -File scripts/installer.ps1` from the project root; use `-Scaffold` while
only the development bootstrap exists. The script builds a release executable,
stages one package and passes its Cargo version and exact payload to ISCC.
Use `-ISCCPath 'C:/path/to/ISCC.exe'` for a nonstandard compiler location.

The initial installer requires Inno Setup 7 and targets Windows 11 x64. It
installs for the current user under `%LOCALAPPDATA%/Programs/NetMap`, creates
a Start menu entry and supplies standard uninstall. The package includes
`netmap.exe` and a byte-identical `netmap.scr` beside the shared `assets/` and
notices. There is one compiled application and one copy of each asset.
Both executable names are installed and removed by the ordinary per-user
installer. It adds no startup task, service, automatic screen saver selection,
desktop integration or forced application launch.
Uninstall leaves runtime preferences and downloaded data in
`%LOCALAPPDATA%/NetMap` untouched. Collector privilege requirements remain
separate from the per-user installer.

Compiler discovery and packaging are in the scripts; version information comes
from Cargo.toml. Outputs live under `artifacts/packages/`, outside installer
sources. Packaging copies the same build output to both executable names before
archiving the payload.

Compilation does not run or install the generated setup executable. Screen saver
installation/selection must be an explicit action through the Windows workflow;
copying the `.scr` into this package does not select it or prove that Windows
lists, previews or runs it. The Settings entry for that deliberate workflow is
implemented separately. Windows selection, Preview, full-screen/input dismissal,
sign-in-on-resume and uninstall behavior still require native verification.
Signing remains pending.

Relevant Inno Setup documentation:

- [Compiler parameters](https://jrsoftware.org/ishelp/topic_compilercmdline.htm)
- [Preprocessor definitions](https://jrsoftware.org/ishelp/topic_isppcc.htm)
- [Installation privileges](https://jrsoftware.org/ishelp/topic_setup_privilegesrequired.htm)
- [Architecture selection](https://jrsoftware.org/ishelp/topic_setup_architecturesallowed.htm)
