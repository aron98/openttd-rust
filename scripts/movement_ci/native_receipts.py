"""Re-admit actual native argv, protocol, baseline identities and full checkpoints."""

from __future__ import annotations

from pathlib import Path

from .baseline import FileInput, bind
from .events import validate
from .protocol import CASES, labels, validate_request
from .value import DECODE, EvidenceError, array, field, integer, require, string


def native_results(output: Path, *, complete: bool = True) -> tuple[Path, ...]:
    """Partition the exact native and CLI result namespaces; reject unknown results."""
    expected = {
        case.name.replace("-", "_") + "_" + suffix
        for case in CASES
        for suffix in (
            "prepare",
            "zero",
            "discover",
            "trace",
            "off",
            "original_from_rust",
        )
    }
    cli = {
        case.name.replace("-", "_") + "_" + suffix
        for case in CASES
        for suffix in (
            "cli",
            "cli_zero",
            "cli_crossing",
            "cli_from_original",
            "cli_from_rust",
        )
    }
    observed = {p.parent.name for p in output.glob("*/results.json")}
    require("unknown process results", condition=observed <= expected | cli)
    if complete:
        require(
            "complete native process roster", condition=observed & expected == expected
        )
        require("complete CLI process roster", condition=observed & cli == cli)
    return tuple(output / name / "results.json" for name in sorted(observed & expected))


def verify(output: Path) -> None:
    for result_path in native_results(output):
        directory = result_path.parent
        name = directory.name
        process = DECODE((directory / "process.json").read_text())
        require(
            "native host/process failure",
            condition=field(process, "exit_code") == 0
            and field(process, "classification") == "native_process_exit",
        )
        protocol = DECODE((directory / "protocol.json").read_text())
        if not isinstance(protocol, dict):
            raise EvidenceError("protocol object required")
        validate_request(protocol)
        invocation = DECODE((directory / "invocation.json").read_text())
        require(
            "protocol input hash",
            condition=FileInput.capture(directory / "protocol.json").digest
            == field(invocation, "protocol_sha256"),
        )
        require(
            "configuration input hash",
            condition=FileInput.capture(directory / "openttd.cfg").digest
            == field(invocation, "config_sha256"),
        )
        for row in array(field(invocation, "source_inputs")):
            require(
                "actual compiled native and harness source changed",
                condition=FileInput.capture(Path(string(field(row, "path")))).digest
                == field(row, "sha256"),
            )
        source = FileInput.capture(Path(string(field(invocation, "input"))))
        require(
            "actual loaded input unchanged",
            condition=source.digest == field(invocation, "input_sha256"),
        )
        argv = array(field(invocation, "argv"))
        native = Path(string(argv[0]))
        require(
            "actual native executable unchanged",
            condition=FileInput.capture(native).digest
            == field(invocation, "native_sha256"),
        )
        require(
            "actual argv includes exact loaded input",
            condition=argv[-3:] == [str(source.path), "-d", "sl=2"],
        )
        candidates = tuple(
            FileInput(
                Path(string(field(row, "path"))),
                integer(field(row, "bytes")),
                string(field(row, "sha256")),
                integer(field(row, "device")),
                integer(field(row, "inode")),
            )
            for row in array(field(invocation, "baseline_inventory"))
        )
        _ = bind(
            DECODE((directory / "baseline.json").read_text()),
            candidates,
            native.parent / "baseset",
        )
        result = DECODE((directory / "results.json").read_text())
        require(
            "actual original outcome",
            condition=field(result, "outcome") == field(invocation, "expected_outcome")
            and field(result, "observer_failure") == "none",
        )
        validate(directory, result)
        calls = integer(field(result, "calls"))
        wanted = labels(calls)
        if name.endswith("_prepare"):
            wanted[1:1] = ["settings_after", "built"]
        require(
            "complete ordered checkpoint receipt",
            condition=[
                field(row, "label") for row in array(field(result, "checkpoints"))
            ]
            == wanted,
        )
        for suffix in ("world", "schema", "derived", "content", "runtime", "movement"):
            require(
                "native checkpoint file membership",
                condition={p.name for p in directory.glob(f"*.{suffix}.json")}
                == {f"{label}.{suffix}.json" for label in wanted},
            )
