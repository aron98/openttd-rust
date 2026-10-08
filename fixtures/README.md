# Save corpus

| File | Source | Save version |
| --- | --- | --- |
| `generated-v362.sav` | OpenTTD 15.3, 64 x 64 map, seed 12345, OpenGFX 7.1; 500 null-driver ticks | 362 |
| `upstream-regression-v308.sav` | `regression/regression/test.sav` at the pinned upstream commit | 308 |
| `upstream-stationlist-v211.sav` | `regression/stationlist/test.sav` at the pinned upstream commit | 211 |

The upstream fixtures are retained verbatim under OpenTTD's GPL-2.0 license.
They provide populated historical states in addition to the small generated map.
They do not establish coverage of every feature, save version, or mod.

SHA-256:

```text
1bcf1edd1ca409a520d08c1b2dbff48a143fa4f499b52fc4a660f07e69f94f2f  generated-v362.sav
95377d5076bdd55eed6e3c85cb65c4c4ac46d80e34b11cdf4e1235f6055250a3  upstream-regression-v308.sav
9f30d56635848d37aa2cfbfefac009f67bb582a005bdb7e8e151ba43a454361f  upstream-stationlist-v211.sav
```

Generate another current-version fixture in a new directory:

```sh
cmake -DORACLE="$PWD/.reference/build/openttd" \
  -DRUN_DIR="$PWD/.artifacts/new-fixture" \
  -DCONFIG="$PWD/scripts/reference.cfg" -DINPUT=GENERATE -DTICKS=500 \
  -P scripts/run-reference.cmake
```

The resulting file is `.artifacts/new-fixture/save/autosave/exit.sav`.
Fresh generation is not yet asserted to be byte-reproducible across platforms;
the checked-in fixture and its hash are the stable test input.
