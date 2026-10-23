# Runtime assets

The unchanged `world.bin` and attribution are tracked. Country data remains
generated/ignored. On a fresh checkout, prepare the pinned country database:

```powershell
pwsh -File .\scripts\prepare-assets.ps1
```

The script downloads the archive named in `country.json`, streams decompression,
checks the exact database length and SHA-256, then installs the verified file.
An existing matching file needs no download; a different file is preserved and
reported. Updating the bundled month requires updating the manifest and its
attribution together. Stage 1 requires:

- `world.bin`: copied unchanged from the documented TinyTorrent2 source; its
  recorded hash is retained in the porting guide.
- `country.mmdb`: DB-IP Country Lite pinned by `country.json`. It is ignored by
  Git because it is generated data.
- `Attribution.md`: actual source revisions, dataset month, applicable notices
  and attribution links. Fit is an ordinary labelled Win32 button with the
  platform font; this native control does not require shipping Lucide's font.

Follow [Porting](../docs/agents/porting.md#reference-inspected) and the
[distribution contract](../docs/plans/rust-map.md#distribution-and-saved-data).
City data is downloaded runtime data, not a bundled source asset. Build/check
does not download or modify any data. Packaging without `-Scaffold` requires
the three files above; `-Scaffold` packages the available assets and notices so
the build/installer pipeline can be exercised during implementation.

The fresh-download path was exercised in an isolated assets directory at
`V:/temp/netmap-fresh-assets-20261023/`: it produced the pinned 8,348,509-byte
database with its expected SHA-256 and removed download temporaries. The
existing-file path also verified the workspace copy without downloading.
