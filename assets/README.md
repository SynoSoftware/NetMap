# Runtime assets

The bootstrap has no map or geolocation assets. Stage 1's distribution requires:

- `world.bin`: copy unchanged from the documented TinyTorrent2 source and verify
  its recorded hash before porting the decoder.
- `country.mmdb`: the current DB-IP Country Lite database, acquired using the
  documented monthly policy. It is ignored by Git because it is refreshed data.
- `Attribution.md`: actual source revisions, dataset month, applicable notices
  and attribution links. Add required glyph/font files and their licences when
  their map controls are implemented.

Follow [Porting](../docs/agents/porting.md#reference-inspected) and the
[distribution contract](../docs/plans/rust-map.md#distribution-and-saved-data).
City data is downloaded runtime data, not a bundled source asset. Build/check
does not download or modify any data. Packaging without `-Scaffold` requires
the three files above; `-Scaffold` deliberately packages only the bootstrap and
notices so the build/installer pipeline can be exercised before the map port.
