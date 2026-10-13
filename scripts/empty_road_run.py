from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.context_ci_support import invocation_fields
from scripts.empty_road_evidence import labels, pair, trace
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    write_json,
)


@dataclass(frozen=True, slots=True)
class EmptyRun:
    job: ControlRun
    binaries: Json
    fixtures: Json

    @property
    def cli(self) -> str:
        return text(at(self.binaries, ("cli", "retained")))

    def protocol(self, name: str) -> Json:
        return at(self.fixtures, ("protocols", name))

    def native(self, name: str, source: Path, protocol: str) -> None:
        output = self.job.output / "native" / name
        before = digest(source)
        descriptor = self.job.output / "protocols" / (name + ".json")
        write_json(descriptor, self.protocol(protocol))
        _ = self.job.run(
            "native-" + name,
            [
                "cmake",
                f"-DORACLE={self.job.oracle}",
                f"-DRUN_DIR={output}",
                f"-DINPUT={source}",
                f"-DREPLAY={descriptor}",
                f"-DCONFIG={self.job.root}/scripts/reference.cfg",
                "-P",
                str(self.job.root / "scripts/check-replay-native.cmake"),
            ],
        )
        fields = invocation_fields(self.job.root, output / "invocation.txt")
        expected_sources = sorted(
            f"SOURCE_SHA256={digest(path)} {path.relative_to(self.job.root)}"
            for extension in ("*.hpp", "*.patch")
            for path in (self.job.root / "reference").glob(extension)
        )
        recorded_sources = sorted(
            line
            for line in (output / "invocation.txt").read_text().splitlines()
            if line.startswith("SOURCE_SHA256=")
        )
        if recorded_sources != expected_sources:
            raise WorldCheckError("Empty-road native source invocation differs")
        for key, path in (
            ("INPUT", source),
            ("REPLAY", descriptor),
            ("ORACLE", self.job.oracle),
        ):
            compare(fields[key + "_SHA256"], digest(path))
            compare(str(Path(fields[key]).resolve()), str(path.resolve()))
        compare(digest(source), before)
        compare(read_json(output / "actions.json"), self.protocol(protocol))
        trace(self.protocol(protocol), read_json(output / "results.json"), native=True)
        exported = self.job.run(
            "input-export-" + name, [self.cli, "world", str(source), "--view", "saved"]
        )
        compare(decode_json(exported.stdout), read_json(output / "initial.world.json"))
        write_json(
            output / "input-binding.json",
            {
                "path": str(source),
                "before_sha256": before,
                "after_sha256": digest(source),
                "protocol_sha256": digest(descriptor),
                "native_sha256": digest(self.job.oracle),
            },
        )

    def rust(self, name: str, source: Path, protocol: str) -> None:
        output = self.job.output / "rust" / name
        before = digest(source)
        _ = self.job.run(
            "rust-" + name,
            [
                self.cli,
                "replay-world",
                str(source),
                str(self.job.output / "protocols" / (name + ".json")),
                str(output),
            ],
        )
        compare(digest(source), before)
        native = self.job.output / "native" / name
        pair(self.protocol(protocol), native, output)
        for label in labels(self.protocol(protocol)):
            for view in ("world", "derived"):
                _ = self.job.run(
                    f"compare-{name}-{label}-{view}",
                    [
                        self.cli,
                        "compare",
                        str(native / f"{label}.{view}.json"),
                        str(output / f"{label}.{view}.json"),
                    ],
                )
            saved = output / f"{label}.sav"
            exported = self.job.run(
                f"decode-{name}-{label}",
                [self.cli, "world", str(saved), "--view", "saved"],
            )
            compare(
                decode_json(exported.stdout), read_json(native / f"{label}.world.json")
            )
            write_json(
                output / f"{label}.decoded-world.json", decode_json(exported.stdout)
            )
        write_json(
            output / "input-binding.json",
            {
                "path": str(source),
                "before_sha256": before,
                "after_sha256": digest(source),
                "cli_sha256": digest(Path(self.cli)),
            },
        )

    def execute(self) -> None:
        (self.job.output / "protocols").mkdir()
        (self.job.output / "rust").mkdir()
        seed = self.job.root / text(at(self.fixtures, ("seed",)))
        compare(digest(seed), at(self.fixtures, ("seed_sha256",)))
        self.native("build", seed, "build")
        for row in sequence(at(self.fixtures, ("pairs",))):
            name, protocol = text(at(row, ("id",))), text(at(row, ("protocol",)))
            source = self.job.output / text(at(row, ("input",)))
            if not source.resolve().is_relative_to(self.job.output):
                raise WorldCheckError("Escaped empty-road source")
            self.native(name, source, protocol)
            self.rust(name, source, protocol)
            print("PASS empty-road pair " + name, flush=True)
