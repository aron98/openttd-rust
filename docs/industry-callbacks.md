# Original-economy industry monthly callback

`simulate-callbacks` accepts this schema1 callback variant:

```json
{"kind":"industry_month","phase":{"month":0,"year":2001,"days_since_last_month":31},"state":{}}
```

Wrap it in `{"schema_version":1,"callback":...}`. Native examples and complete
state are in `reference/callbacks.json` under `industry_cases`.

This runs the entire original economy MONTH/INDUSTRY body in a closed supported
domain: original economy (`economy_type=0`), no NewGRF, original industry types
0..36, and positive production levels. The native monthly production-change
function returns before policy/RNG/news work under these conditions, and the
positive level prevents the deletion branch. Rust rejects other contexts.

Implemented effects are builder desired-count growth and complete industry
statistics rollover: all61 current/monthly/quarterly/yearly records, the raw64-bit
validity mask, produced-year updates, accepted waiting averages, and accumulator
reset. Optional accepted history and invalid cargo255 follow the native skip
branches; other cargo bytes follow native `IsValidCargoType` exactly. Raw waiting,
production rates, last acceptance dates, and unrelated history bits survive.
Native `Industry::industries` counts are derived from the supplied complete sorted
pool; the probe populates that native cache with every actual industry ID.

The phase is the actual callback-entry context, before the timer resets
`days_since_last_month` or rewinds the maximum year. Consequently phase.year and
last_prod_year can be5000001. A post-tick clock snapshot is not interchangeable
with this phase. The native probe captures these values in a timer observer
before the original registered INDUSTRY callback runs.

The supported domain does not implement smooth/frozen industry policy, random
production changes, closures, NewGRF, daily builder attempts, industry tile
production, cargo delivery, or a full-world tick. Each unsupported request fails
without outputting a partially updated state.

`bash scripts/check-callbacks.sh` verifies fresh original callback results through
the library and CLI, repeatability, the committed corpus, and unsupported-context
rejections. Industry cases exercise empty and populated pools, optional histories,
partial masks, all aggregation boundaries, zero/large average denominators,
fund-only and backlog growth gates, rectangular/large maps, and maximum-year
callback phases.
