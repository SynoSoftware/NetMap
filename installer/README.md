# Installer

`netmap.iss` owns the Inno Setup installer. Compile it with
`pwsh -File scripts/installer.ps1` from the project root; use `-Scaffold` while
only the development bootstrap exists. The script builds a release executable,
stages one package and passes its Cargo version and exact payload to ISCC.
Use `-ISCCPath 'C:/path/to/ISCC.exe'` for a nonstandard compiler location.

The initial installer requires Inno Setup 7 and targets Windows 11 x64. It
installs for the current user under `%LOCALAPPDATA%/Programs/NetMap`, creates
a Start menu entry and supplies standard uninstall. It adds no startup task,
service, desktop/screen saver integration or forced application launch.
Uninstall leaves runtime preferences and downloaded data in
`%LOCALAPPDATA%/NetMap` untouched. Installation elevation and the future
collector's privilege requirements are separate decisions.

Compiler discovery and packaging are in the scripts; version information comes
from Cargo.toml. Outputs live under `artifacts/packages/`, outside installer
sources. Compilation does not run or install the generated setup executable.
Signing and stage 2 `.scr` packaging are still pending.

Relevant Inno Setup documentation:

- [Compiler parameters](https://jrsoftware.org/ishelp/topic_compilercmdline.htm)
- [Preprocessor definitions](https://jrsoftware.org/ishelp/topic_isppcc.htm)
- [Installation privileges](https://jrsoftware.org/ishelp/topic_setup_privilegesrequired.htm)
- [Architecture selection](https://jrsoftware.org/ishelp/topic_setup_architecturesallowed.htm)
