# Distribution notices

`Goz-MIT.txt` covers adapted native Settings shell and word-prefix search
mechanisms from Goz's inspected working tree. The source hashes are recorded
in the Settings implementation evidence.

`windows-rs-MIT.txt` carries the MIT notice for Microsoft's `windows` crate and
its Windows binding support crates. Keep this directory in packaged builds.
Add each dependency/asset's applicable notice when it becomes part of the
distributed application. Build-only tools are not runtime frameworks.

`TinyTorrent2-MPL-2.0.txt` covers the translated geography, viewport and renderer sources.
The bundled map geography and country dataset have provenance in
`assets/Attribution.md`; `DB-IP.txt` covers country and optional city data.
Fit uses a labelled native button and the Windows platform font. Lucide is not
used or redistributed by that control; no separate glyph font is required. See
[assets](../../assets/README.md) and the [porting guide](../../docs/agents/porting.md).

The versioned text files retain notices for MMDB/mapping/gzip dependencies and
their runtime support crates. Their exact versions come from Cargo.lock. MIT
is selected for dual MIT/Apache crates and 0BSD for adler2; maxminddb uses ISC.
