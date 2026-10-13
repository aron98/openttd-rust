# SPDX-License-Identifier: GPL-2.0-only
"""Compile exact native arithmetic functions; this is not a whole-world proof."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.tree_growth_run import run
from scripts.world_check_support import Json, WorldCheckError, write_json


def function(body: str, signature: str) -> str:
    first = body.index(signature)
    opening = body.index("{", first)
    depth = 0
    for index in range(opening, len(body)):
        if body[index] == "{":
            depth += 1
        elif body[index] == "}":
            depth -= 1
            if depth == 0:
                return body[first : index + 1]
    raise WorldCheckError(f"Unterminated original function: {signature}")


def probe(source: Path, output: Path) -> Path:
    output.mkdir(parents=True)
    inputs = {
        "ceil": source / "src/core/math_func.hpp",
        "scale": source / "src/map_func.h",
        "counter": source / "src/tree_cmd.cpp",
        "bounds": source / "src/map_type.h",
    }
    fragments = {
        "ceil": function(
            inputs["ceil"].read_text(), "constexpr uint CeilDiv(uint a, uint b)"
        ),
        "scale": function(
            inputs["scale"].read_text(), "static inline uint ScaleBySize(uint n)"
        ),
        "counter": function(
            inputs["counter"].read_text(), "bool DecrementTreeCounter()"
        ),
    }
    bounds = [
        line
        for line in inputs["bounds"].read_text().splitlines()
        if line.startswith(
            (
                "static const uint MIN_MAP_SIZE_BITS",
                "static const uint MAX_MAP_SIZE_BITS",
            )
        )
    ]
    if len(bounds) != 2:
        raise WorldCheckError("Original map bounds missing")
    generated = output / "counter.cpp"
    _ = generated.write_text(
        """// SPDX-License-Identifier: GPL-2.0-only
// Exact pinned OpenTTD 15.3 functions; only their environment is isolated.
#include <cstdint>
#include <limits>
#include <iostream>
using uint = unsigned int;
uint gx, gy;
"""
        + fragments["ceil"]
        + "\nstruct Map {\n"
        + "static uint LogX() { return gx; }\nstatic uint LogY() { return gy; }\n"
        + fragments["scale"]
        + "\n};\n"
        + "\n".join(bounds)
        + "\nuint8_t _trees_tick_ctr;\n"
        + fragments["counter"]
        + "\n"
        + r"""
int main() {
    for (gx = 6; gx <= 12; ++gx) for (gy = 6; gy <= 12; ++gy) {
        for (uint old = 0; old <= 255; ++old) {
            _trees_tick_ctr = static_cast<uint8_t>(old);
            const bool wrapped = DecrementTreeCounter();
            std::cout << "{\"width\":" << (1U << gx) << ",\"height\":" << (1U << gy)
                << ",\"old\":" << old
                << ",\"next\":" << static_cast<uint>(_trees_tick_ctr)
                << ",\"underflow\":" << (wrapped ? "true" : "false")
                << ",\"scale1\":" << Map::ScaleBySize(1)
                << ",\"scale16\":" << Map::ScaleBySize(16) << "}\n";
        }
    }
}
"""
    )
    binary = output / "counter-probe"
    run(
        [
            "clang++",
            "-std=c++20",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-O2",
            str(generated),
            "-o",
            str(binary),
        ],
        output / "compile",
    )
    run([str(binary)], output / "execute")
    fragment_json: dict[str, Json] = dict(fragments)
    write_json(
        output / "bindings.json",
        {
            "scope": "isolated native arithmetic only; no full large-world claim",
            "source_files": {
                name: {"path": str(path), "sha256": digest(path)}
                for name, path in inputs.items()
            },
            "exact_function_text": fragment_json,
            "generated_sha256": digest(generated),
            "binary_sha256": digest(binary),
        },
    )
    return output / "execute/logs/process/stdout.log"


class Arguments(argparse.Namespace):
    source: Path = Path()
    output: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument("--source", required=True, type=Path)
    _ = parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(namespace=Arguments())
    print(probe(args.source.resolve(), args.output.resolve()))


if __name__ == "__main__":
    main()
