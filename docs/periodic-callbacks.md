# Periodic bookkeeping callbacks

Schema 1 `simulate-callbacks` also accepts these typed callback variants:

- `{"kind":"house_year","state":...}` runs the original economy YEAR/TOWN
  map scan: every completed house tile ages by one, up to 255. The whole raw map
  and RNG are supplied and returned; non-house/incomplete-house fields survive.
- `{"kind":"company_year","state":...}` runs economy YEAR/COMPANY expense
  rollover. Each company's complete 3×13 signed table becomes `[zero, old0,
  old1]`. `show_finances` must be false because windows and sounds are outside
  this headless boundary.
- `{"kind":"station_month","state":...}` runs economy MONTH/STATION status
  rollover for all 64 cargo slots in each modeled station. Bit 3 receives old
  bit 4, then bit 4 clears; other bits and the adjacent rating scalars survive.
  Waypoints are not station callback objects.

Wrap a variant in `{"schema_version":1,"callback":...}`. Examples and expected
results are in the `periodic_cases` of `reference/callbacks.json`. The public API
uses strict typed state and rejects unknown fields, missing map tiles, incomplete
cargo arrays, and invalid or unordered native object IDs.

These are explicit callback entrypoints, not a full-world tick. Monthly town
growth/ratings, station cargo distribution/ratings, company economic simulation,
vehicle operation, and other callbacks are not implied. Station state is the
status/rating scalar boundary; cargo packets, routes, and other station subsystems
are not serialized or simulated here.

The independent native probe selects each original registered timer by trigger
and priority, invokes it through the original `TimerManager`, and restores the
registry between families. No callback bodies are copied into oracle code.
`bash scripts/check-callbacks.sh` compares all native output fields through the
library and real command, repeats command execution, exercises rejected requests,
and checks byte-reproducible native fixture output.
