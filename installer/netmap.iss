; Compile through scripts/installer.ps1. Cargo is the version authority.
#ifndef AppVersion
  #error AppVersion is required; use scripts/installer.ps1.
#endif
#ifndef AppFileVersion
  #error AppFileVersion is required; use scripts/installer.ps1.
#endif
#ifndef PayloadDir
  #error PayloadDir is required; use scripts/installer.ps1.
#endif

[Setup]
AppId={{FD9BEE48-5BB4-4EF0-BCE9-1A83362DD084}
AppName=NetMap
AppVersion={#AppVersion}
VersionInfoVersion={#AppFileVersion}
DefaultDirName={localappdata}\Programs\NetMap
DefaultGroupName=NetMap
PrivilegesRequired=lowest
SetupArchitecture=x64
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.22000
UninstallDisplayIcon={app}\netmap.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
OutputBaseFilename=NetMap-setup

[Files]
; Both entry names use one compiled application and the same adjacent assets.
; Inno owns their normal per-user installation and uninstall removal.
Source: "{#PayloadDir}\netmap.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\netmap.scr"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\*"; DestDir: "{app}"; Excludes: "netmap.exe,netmap.scr"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\NetMap"; Filename: "{app}\netmap.exe"; WorkingDir: "{app}"
