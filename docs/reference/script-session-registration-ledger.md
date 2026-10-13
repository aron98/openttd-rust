# Source-derived session registration ledger

Pin `14ec60f248547d4d062a1160f0fc26d742319888`; `src/3rdparty/squirrel/squirrel/sqbaselib.cpp`. Bare build: i64/f32, GC enabled, EXPORT_DEFAULT_SQUIRREL_FUNCTIONS absent. This is literal source metadata, not new runtime/native observation.

Every row allocates a fresh native closure, even when function symbols repeat. Registration order is the row order within each table. Arity includes receiver: positive exact, negative minimum; zero unchecked. Masks are signed i64 values; nullopt means an empty vector, `-1` accepts any type, unions OR raw tags. Parameter validation checks only min(actual nargs, mask count). All initial outers are empty and environment Null; name and table key share the real interned string.

| Registry | Entry | Native function identity | Arity | Exact typemask | Compiled masks | Line |
|---|---|---|---:|---|---|---:|
| table | len | `default_delegate_len` | 1 | `t` | `[32]` | 412 |
| table | rawget | `table_rawget` | 2 | `t` | `[32]` | 413 |
| table | rawset | `table_rawset` | 3 | `t` | `[32]` | 414 |
| table | rawdelete | `table_rawdelete` | 2 | `t` | `[32]` | 415 |
| table | rawin | `container_rawexists` | 2 | `t` | `[32]` | 416 |
| table | weakref | `obj_delegate_weakref` | 1 | nullopt | `[]` | 417 |
| table | tostring | `default_delegate_tostring` | 1 | `.` | `[-1]` | 418 |
| table | clear | `obj_clear` | 1 | `.` | `[-1]` | 419 |
| array | len | `default_delegate_len` | 1 | `a` | `[64]` | 622 |
| array | append | `array_append` | 2 | `a` | `[64]` | 623 |
| array | extend | `array_extend` | 2 | `aa` | `[64, 64]` | 624 |
| array | push | `array_append` | 2 | `a` | `[64]` | 625 |
| array | pop | `array_pop` | 1 | `a` | `[64]` | 626 |
| array | top | `array_top` | 1 | `a` | `[64]` | 627 |
| array | insert | `array_insert` | 3 | `an` | `[64, 6]` | 628 |
| array | remove | `array_remove` | 2 | `an` | `[64, 6]` | 629 |
| array | resize | `array_resize` | -2 | `an` | `[64, 6]` | 630 |
| array | reverse | `array_reverse` | 1 | `a` | `[64]` | 631 |
| array | sort | `array_sort` | -1 | `ac` | `[64, 768]` | 632 |
| array | slice | `array_slice` | -1 | `ann` | `[64, 6, 6]` | 633 |
| array | weakref | `obj_delegate_weakref` | 1 | nullopt | `[]` | 634 |
| array | tostring | `default_delegate_tostring` | 1 | `.` | `[-1]` | 635 |
| array | clear | `obj_clear` | 1 | `.` | `[-1]` | 636 |
| string | len | `default_delegate_len` | 1 | `s` | `[16]` | 688 |
| string | tointeger | `default_delegate_tointeger` | 1 | `s` | `[16]` | 689 |
| string | tofloat | `default_delegate_tofloat` | 1 | `s` | `[16]` | 690 |
| string | tostring | `default_delegate_tostring` | 1 | `.` | `[-1]` | 691 |
| string | slice | `string_slice` | -1 | ` s n  n` | `[16, 6, 6]` | 692 |
| string | find | `string_find` | -2 | `s s n ` | `[16, 16, 6]` | 693 |
| string | tolower | `string_tolower` | 1 | `s` | `[16]` | 694 |
| string | toupper | `string_toupper` | 1 | `s` | `[16]` | 695 |
| string | weakref | `obj_delegate_weakref` | 1 | nullopt | `[]` | 696 |
| number | tointeger | `default_delegate_tointeger` | 1 | `n\|b` | `[14]` | 701 |
| number | tofloat | `default_delegate_tofloat` | 1 | `n\|b` | `[14]` | 702 |
| number | tostring | `default_delegate_tostring` | 1 | `.` | `[-1]` | 703 |
| number | tochar | `number_delegate_tochar` | 1 | `n\|b` | `[14]` | 704 |
| number | weakref | `obj_delegate_weakref` | 1 | nullopt | `[]` | 705 |
| closure | call | `closure_call` | -1 | `c` | `[768]` | 785 |
| closure | pcall | `closure_pcall` | -1 | `c` | `[768]` | 786 |
| closure | acall | `closure_acall` | 2 | `ca` | `[768, 64]` | 787 |
| closure | pacall | `closure_pacall` | 2 | `ca` | `[768, 64]` | 788 |
| closure | weakref | `obj_delegate_weakref` | 1 | nullopt | `[]` | 789 |
| closure | tostring | `default_delegate_tostring` | 1 | `.` | `[-1]` | 790 |
| closure | bindenv | `closure_bindenv` | 2 | `c x\|y\|t` | `[768, 49184]` | 791 |
| closure | getinfos | `closure_getinfos` | 1 | `c` | `[768]` | 792 |
| generator | getstatus | `generator_getstatus` | 1 | `g` | `[1024]` | 808 |
| generator | weakref | `obj_delegate_weakref` | 1 | nullopt | `[]` | 809 |
| generator | tostring | `default_delegate_tostring` | 1 | `.` | `[-1]` | 810 |
| thread | call | `thread_call` | -1 | `v` | `[4096]` | 891 |
| thread | wakeup | `thread_wakeup` | -1 | `v` | `[4096]` | 892 |
| thread | getstatus | `thread_getstatus` | 1 | `v` | `[4096]` | 893 |
| thread | weakref | `obj_delegate_weakref` | 1 | nullopt | `[]` | 894 |
| thread | tostring | `default_delegate_tostring` | 1 | `.` | `[-1]` | 895 |
| class | getattributes | `class_getattributes` | 2 | `y.` | `[16384, -1]` | 920 |
| class | setattributes | `class_setattributes` | 3 | `y..` | `[16384, -1, -1]` | 921 |
| class | rawin | `container_rawexists` | 2 | `y` | `[16384]` | 922 |
| class | weakref | `obj_delegate_weakref` | 1 | nullopt | `[]` | 923 |
| class | tostring | `default_delegate_tostring` | 1 | `.` | `[-1]` | 924 |
| class | instance | `class_instance` | 1 | `y` | `[16384]` | 925 |
| instance | getclass | `instance_getclass` | 1 | `x` | `[32768]` | 936 |
| instance | rawin | `container_rawexists` | 2 | `x` | `[32768]` | 937 |
| instance | weakref | `obj_delegate_weakref` | 1 | nullopt | `[]` | 938 |
| instance | tostring | `default_delegate_tostring` | 1 | `.` | `[-1]` | 939 |
| weakref | ref | `weakref_ref` | 1 | `r` | `[65536]` | 950 |
| weakref | weakref | `obj_delegate_weakref` | 1 | nullopt | `[]` | 951 |
| weakref | tostring | `default_delegate_tostring` | 1 | `.` | `[-1]` | 952 |
| base | assert | `base_assert` | 2 | nullopt | `[]` | 253 |
| base | print | `base_print` | 2 | nullopt | `[]` | 254 |
| base | array | `base_array` | -2 | `.n` | `[-1, 6]` | 260 |
| base | type | `base_type` | 2 | nullopt | `[]` | 261 |

Defaults: table=8, array=15, string=9, number=5, closure=8, generator=3, thread=5, class=6, instance=4, weakref=3. Total66; base4; 44 distinct native function symbols. `append` and `push` share array_append but are different closure objects. Shared helper function identities remain the same across distinct table entries.

## Metamethod map and non-function root data

`sqstate.cpp:100–140`, `sqobject.h:14–53`: map allocated first with requested capacity17 (native table rounds to32); below key→integer entries are inserted in order. Each key is also retained once by the metamethod string vector.

| Index | Macro | Interned key |
|---:|---|---|
| 0 | MM_ADD | `_add` |
| 1 | MM_SUB | `_sub` |
| 2 | MM_MUL | `_mul` |
| 3 | MM_DIV | `_div` |
| 4 | MM_UNM | `_unm` |
| 5 | MM_MODULO | `_modulo` |
| 6 | MM_SET | `_set` |
| 7 | MM_GET | `_get` |
| 8 | MM_TYPEOF | `_typeof` |
| 9 | MM_NEXTI | `_nexti` |
| 10 | MM_CMP | `_cmp` |
| 11 | MM_CALL | `_call` |
| 12 | MM_CLONED | `_cloned` |
| 13 | MM_NEWSLOT | `_newslot` |
| 14 | MM_DELSLOT | `_delslot` |
| 15 | MM_TOSTRING | `_tostring` |
| 16 | MM_NEWMEMBER | `_newmember` |
| 17 | MM_INHERITED | `_inherited` |

System strings, in order: null, table, array, closure, string, userdata, integer, float, userpointer, function, generator, thread, class, instance, bool. `_types` is allocated empty; `_constructoridx` retains `constructor`. Registry and const table start empty. These strings/prototypes are not collectable GC-chain nodes.

Root entries after the four base closures: `_version_` = `Squirrel 2.2.5 stable - With custom OpenTTD modifications`, `_charsize_`=1, `_intsize_`=8, `_floatsize_`=4. Base registration pushes root, then each key/closure and metadata, creates slot/pops values, writes the four data entries, pops root. Friend initialization repeats these allocations/replacements on the shared root.

Source allocation accounting: thirteen shared tables + root table + one primary VM +66default/4baseclosures =85 collectables. This is a source ledger cross-check against the original85-node observation; the unknown observed chain nodes were not passively type-classified. String interning/table-node allocations and RefTable capacity are separate from this collectable count.
