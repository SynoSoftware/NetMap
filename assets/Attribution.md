# Map geography and source

`world.bin` is copied unchanged from TinyTorrent2 at
`95be5335eff5261d4f94b2e77a4bc0db0dcbf151`, file
`app/src/Assets/world.bin`. SHA-256:
`C27F41D8C00E87AA71ECE3A0A4134B9E6779D15302922207B8956BB2272023D3`.

It derives from Natural Earth 5.1's 1:10m admin-0 countries (without Antarctica)
and 10m populated places, using POP_MAX for estimated placement weights. Shared
borders are stored once, with per-vertex simplification levels. Natural Earth
data is [public domain](https://www.naturalearthdata.com/about/terms-of-use/).
Population weights do not establish any individual connection's city.

The geography decoder and viewport port are based on TinyTorrent2's
`Geography.cs`, `MapViewport.cs` and `MapRenderer.cs` at that revision. Their source-code licence
is Mozilla Public License 2.0, retained in
`installer/notices/TinyTorrent2-MPL-2.0.txt`; covered ported files carry that
notice. The original source is at
[SynoSoftware/TinyTorrent2](https://github.com/SynoSoftware/TinyTorrent2).

## IP geolocation

IP Geolocation by [DB-IP](https://db-ip.com). The bundled `country.mmdb` is
DB-IP Country Lite, October 2026, copied from the reference's cached
`dbip-country-lite-2026-10.mmdb` without changes (8,348,509 bytes). SHA-256:
`DBD70CCFA2A13627EAF4913D19920A1A229F1B3C5439AC01C509995B6364202A`.

The country dataset and optional city downloads are licensed under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), with the notice in
`installer/notices/DB-IP.txt`. [Monthly downloads](https://db-ip.com/db/lite.php)
and the [city schema](https://db-ip.com/db/format/ip-to-city-lite/mmdb.html)
are supplied by DB-IP. City data remains optional runtime data.

Map glyphs are not yet bundled; record their notices when added.
