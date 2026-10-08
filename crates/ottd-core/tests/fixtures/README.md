# Primitive vectors

`primitives.json` is emitted by the instrumented OpenTTD 15.3 C++ executable at
upstream revision `14ec60f248547d4d062a1160f0fc26d742319888`. It is not generated
by Rust. The patch and runner are in `reference/` and
`scripts/run-snapshot-reference.cmake`.

Generate a fresh oracle run with `INPUT=GENERATE`, `TICKS=1` and
`scripts/reference.cfg`; the runner writes this file beside `snapshot.json`.
The fixture records randomizer values and states, scaling with limits including
zero and UINT32_MAX, valid leap/century/max-year dates, and 64x64 map coordinates.
`cargo test -p ottd-core` always replays this checked-in fixture.
