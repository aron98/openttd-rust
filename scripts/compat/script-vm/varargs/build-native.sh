#!/bin/sh
# Passive v09 observer retained unchanged from the approved design epoch.
set -eu
if [ "$#" -ne 2 ]; then echo 'usage: build-native.sh PINNED_OPENTTD_CHECKOUT OUTPUT' >&2; exit 64; fi
checkout=$1
out=$2
pin=14ec60f248547d4d062a1160f0fc26d742319888
harness=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
test "$(git -C "$checkout" rev-parse HEAD)" = "$pin"
test ! -e "$out"
mkdir -p "$out"
out=$(CDPATH= cd -- "$out" && pwd)
git -C "$checkout" archive "$pin" src | tar -x -C "$out"
cp "$harness/observe.cpp" "$harness/varargs.hpp" "$out/"
for header in strings constants root_slots arrays tables calls closures; do
 cp "$harness/../$header.hpp" "$out/"
done
cd "$out"
set -x
c++ -std=c++20 -O3 -DNDEBUG -DPOINTER_IS_64BIT -DWITH_ASSERT -DUNIX -DFMT_HEADER_ONLY -I src/3rdparty/squirrel/include observe.cpp src/3rdparty/squirrel/squirrel/*.cpp src/core/string_consumer.cpp src/core/utf8.cpp -o observe
set +x
shasum -a 256 observe observe.cpp ./*.hpp src/3rdparty/squirrel/squirrel/*.cpp src/3rdparty/squirrel/include/squirrel.h src/core/string_consumer.cpp src/core/utf8.cpp > native-sha256.txt
printf 'pin=%s\ninteger_bits=64\nfloat_bits=32\ngc=enabled\n' "$pin" > variant.txt
