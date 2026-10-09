# v362 saved-world boundary

`World` owns every table and map plane. It validates the pinned schemas and native
field semantics, resolves saved pointers, reconstructs the fields in
`DerivedState`, and stages field/tile batches before validating the final state.
It never executes commands, script code, NewGRF callbacks or game ticks.
Object creation/deletion and changing list cardinality are not exposed mutation
operations. Existing scalar/string fields and existing primitive array elements
can be edited; whole primitive array replacement must preserve cardinality.
All nested fields remain addressable with exact field names and list positions.

## Schema provenance and regeneration

Both manifests target the revision in `upstream.toml`:
`14ec60f248547d4d062a1160f0fc26d742319888` (15.3 / save 362).
`schema-v362.json` records the actual native table headers in
`fixtures/generated-v362.sav`. `native-schema-v362.json` records native `SaveLoad`
command, file/memory types, fixed-array lengths, pointer targets and children,
exported by `reference/world*` from the original engine's active descriptors.
The semantic manifest adds the source commit to the native JSON export.
No runtime dependency on a fixture binary or C++ engine is needed.

Regenerate only after building the pinned reference with the world oracle patch:

```sh
WORLD_SCHEMA_OUTPUT="$PWD/crates/ottd-save/src/world/schema-v362.json" \
  cargo test -p ottd-save --test world_schema generate_schema_manifest -- --ignored
cmake -DORACLE="$PWD/.reference/snapshot-build/openttd" \
  -DRUN_DIR="$PWD/.omo/evidence/world-schema-refresh" \
  -DCONFIG="$PWD/scripts/reference.cfg" \
  -DINPUT="$PWD/fixtures/world/populated-v362.sav" -DTICKS=1 \
  -P scripts/check-world-reference.cmake
WORLD_NATIVE_SCHEMA_INPUT="$PWD/.omo/evidence/world-schema-refresh/schema.json" \
WORLD_NATIVE_SCHEMA_OUTPUT="$PWD/crates/ottd-save/src/world/native-schema-v362.json" \
  cargo test -p ottd-save --test world_schema generate_native_descriptor_catalog -- --ignored
```

Review changes against `src/saveload/*_sl.cpp` before committing. Every corpus
save must match the wire manifest; the native driver should additionally compare
its freshly exported descriptor catalog with the semantic manifest.

## Complete chunk policies

The 62 current-version chunks are classified explicitly below. Unknown chunks,
obsolete chunk names, duplicate chunks and missing current chunks reject.

| Policy | Chunks |
| --- | --- |
| Typed map dimensions and exact tile planes | `MAPS`, `MAPT`, `MAPH`, `MAPO`, `MAP2`, `M3LO`, `M3HI`, `MAP5`, `MAPE`, `MAP7`, `MAP8` |
| Sparse tagged vehicle pool | `VEHS` |
| Company, town, industry, station/waypoint pools | `PLYR`, `CITY`, `INDY`, `STNN` |
| Orders, cargo and supporting object pools | `ORDL`, `BKOR`, `CAPA`, `CAPY`, `DEPT`, `ROAD`, `OBJS`, `ERNW`, `ENGN`, `GRPS`, `PSAC`, `SIGN`, `SUBS`, `GOAL`, `STPA`, `STPE`, `LEAT`, `LEAE`, `LGRP`, `LGRJ` |
| Content definitions/mappings and script strings | `NGRF`, `EIDS`, `IIDS`, `TIDS`, `HIDS`, `OBID`, `APID`, `ATID`, `RAIL`, `ROTT`, `GSTR` |
| Settings, world globals, schedules, counters and metadata | `PATS`, `DATE`, `SRND`, `ECMY`, `CHTS`, `ANIT`, `LGRS`, `IBLD`, `ITBL`, `CMDL`, `CMPU`, `VIEW`, `GLOG` |
| Script configuration plus separately serialized state | `AIPL`, `GSDT` |

The global handlers `ANIT`, `CHTS`, `DATE`, `VIEW`, `ECMY`, `IBLD`, `LGRS`,
`MAPS`, `PATS` and `GSDT` accept at most one occupied table row, matching native
`SlIterateArray` cardinality checks. Native loaders permit no row and ignore its
index. This API preserves that behavior for the optional globals; its existing
typed map/date/settings and script contracts require their index-zero record.

All table fields, including metadata and future-gameplay values, decode to their
exact primitive/recursive wire types. The structural runtime is an additional
validated projection, not a claim that every saved scalar has gameplay behavior.
`MAPS` and all other current table headers are pinned exactly. Presentation
metadata (`VIEW`, `GLOG`) remains decoded and preserved, not silently discarded.
Legacy-only handlers `PRIC`, `CAPR`, `ENGS`, `ORDR`, `OPTS`, `NAME`, `CHKP`, `STNS`
and `WRGN` are rejected by this v362-only world API; the lossless container codec
continues to handle earlier supported containers separately.

`AIPL` has fifteen configuration records. An active AI company's record also
contains the native running name, settings and version, followed by script saved
data. Inactive companies must not carry that tail. `GSDT` has one configuration
record and a presence-prefixed saved-data tail. `scripts.rs` checks the native
saved object grammar (64-bit integers, terminated byte strings, arrays, key/value
tables, booleans, nulls and class instances), including the native depth limit.
It preserves those bytes exactly and never instantiates or executes Squirrel.
The source is `script/script_instance.cpp`, `saveload/ai_sl.cpp` and
`saveload/game_sl.cpp`. Content identities, parameters, map labels and persistent
storage are ordinary decoded tables and survive unrelated mutations.

## Restored fields and limits

The complete modeled structural result is exactly the public `DerivedState`:
vehicle `previous`, `first`, `previous_shared`; order-list head/membership/manual
count/all-duration/timetabled-duration; cargo list ownership/packet order/count/
weighted transit periods/feeder share; direct group children; station roadstop
chains; persistent-storage owners; cargo-payment reverse vehicle binding.
Native `uint32` cargo counts and `uint64` weighted periods wrap; native Money
feeder shares saturate. IDs exported in these fields are true pool indices;
canonical saved JSON retains wire references (zero null, otherwise index+1).
Deleted cargo source station IDs are preserved rather than falsely treated as
required pointers. Invalid group-parent owner/type references produce no derived
parent link, following native recovery; cycles reject instead of hanging.

Strict corruption checks can reject malformed saves that native recovery would
repair. Invalid enum/command-level combinations unrelated to the named
structural model do not acquire gameplay semantics merely through decoding.
Native command validation, costs and construction are stage 3/4 work. In
particular, generic saved-field editing is not a substitute for a game command.

Content-dependent vehicle physics/capacity/specification caches, house population,
station specification/catchment caches, content-dependent infrastructure counts,
prices and airport noise are explicitly deferred to stage 4 by the approved
boundary. No placeholder values for those caches are exported.
