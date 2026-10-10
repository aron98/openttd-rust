from __future__ import annotations

import sys
from copy import deepcopy
from pathlib import Path

import pytest

from scripts.context_ci_support import sources
from scripts.depot_build_archive import bounded_paths, package_raw, verify_archive
from scripts.depot_removal_pair import command_metadata, projection
from scripts.gameplay_foundations import digest, require_test
from scripts.ordered_sale_controls import validate_negative
from scripts.ordered_sale_evidence import allocation_probe, membership, witnesses
from scripts.ordered_sale_run import SELECTOR, OrderedSaleRun, member
from scripts.world_check_support import Json, WorldCheckError, at, replace, write_json


def specimen() -> tuple[Json, Json]:
    role: Json = {
        "networking": False,
        "server": False,
        "dedicated": False,
        "own_client_id": 0,
        "clients": [],
    }
    action: Json = {"op": "save", "label": "after"}
    state: Json = {
        "orders": {"backups": [{"pool_slot": 1, "id": 0}]},
        "tiles": [[32, 4]],
        "depot": {"road": {"0": [2]}},
    }
    native: Json = {
        "schema_version": 1,
        "case": "road",
        "role": deepcopy(role),
        "final_role": deepcopy(role),
        "initial": deepcopy(state),
        "final": deepcopy(state),
        "actions": [
            {
                "index": 0,
                "input": action,
                "before": deepcopy(state),
                "after": deepcopy(state),
                "role": deepcopy(role),
                "result": {
                    "path": "after.sav",
                    "before": deepcopy(state),
                    "after": deepcopy(state),
                    "role": deepcopy(role),
                },
            }
        ],
    }
    descriptor: Json = {
        "schema_version": 1,
        "case": "road",
        "role": "sp",
        "exit": True,
        "actions": [action],
    }
    return native, descriptor


def test_projection_retains_complete_state_without_mutating_raw_input() -> None:
    native, descriptor = specimen()
    before = deepcopy(native)
    result = projection(native, descriptor)
    assert native == before
    assert at(result, ("initial",)) == at(before, ("initial",))
    assert at(result, ("final",)) == at(before, ("final",))


@pytest.mark.parametrize(
    "path",
    [
        ("role", "networking"),
        ("role", "server"),
        ("role", "dedicated"),
        ("actions", 0, "role", "server"),
        ("actions", 0, "result", "role", "server"),
    ],
)
def test_role_mutation_is_not_projected_away(path: tuple[str | int, ...]) -> None:
    native, descriptor = specimen()
    replace(native, path, replacement=True)
    with pytest.raises(WorldCheckError):
        _ = projection(native, descriptor)


@pytest.mark.parametrize("value", [[], ["one"], ["one", "one"], ["two", "one"]])
def test_exact_case_membership_rejects_empty_subset_duplicate_and_reordering(
    value: Json,
) -> None:
    with pytest.raises(WorldCheckError, match="membership"):
        membership(value, ["one", "two"])


def test_actual_test_parser_rejects_zero_and_two_tests() -> None:
    for count in (0, 2):
        with pytest.raises(WorldCheckError):
            require_test(
                "\n".join(
                    (
                        "test wanted ... ok",
                        f"test result: ok. {count} passed; 0 failed; 0 ignored;",
                    )
                ),
                "wanted",
            )


@pytest.mark.parametrize("extra", ["orders", "clients", "ignored"])
def test_native_metadata_has_no_arbitrary_projection_escape(extra: str) -> None:
    payload: dict[str, Json] = {
        "receipt": {"test": {}, "exec": None, "result": {}},
        "native_metadata": {
            "test": {
                "error_id": 65535,
                "extra_error_id": 65535,
                "owner": 255,
                extra: 0,
            },
            "result": {"error_id": 65535, "extra_error_id": 65535, "owner": 255},
        },
    }
    with pytest.raises(WorldCheckError, match="metadata"):
        command_metadata(payload)


def test_path_escape_and_symlink_are_rejected(tmp_path: Path) -> None:
    with pytest.raises(WorldCheckError):
        _ = member(tmp_path, "../outside")
    real = tmp_path / "real"
    _ = real.write_text("state")
    (tmp_path / "alias").symlink_to(real)
    with pytest.raises(WorldCheckError):
        _ = member(tmp_path, "alias")
    with pytest.raises(WorldCheckError):
        _ = bounded_paths(tmp_path, {"real", "alias"})


def test_pinned_source_and_archived_bytes_reject_independent_corruption(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source.rs"
    _ = source.write_text("fn canonical() {}\n")
    layout: Json = {"sources": {"source.rs": digest(source)}}
    sources(tmp_path, layout)
    package_raw(tmp_path)
    _ = source.write_text("fn changed() {}\n")
    with pytest.raises(WorldCheckError):
        sources(tmp_path, layout)
    with pytest.raises(WorldCheckError):
        verify_archive(tmp_path)


def test_command_witness_rejects_matching_but_unexecuted_trace() -> None:
    native: Json = {
        "actions": [
            {
                "index": 0,
                "input": {"op": "command"},
                "result": {"receipt": {"posted": False, "exec": None}},
            }
        ]
    }
    with pytest.raises(WorldCheckError, match="membership"):
        witnesses(
            native,
            [{"index": 0, "receipt": {"posted": True, "exec": {"success": True}}}],
        )


@pytest.mark.parametrize(
    "mutation", ["zero-argv", "subset-argv", "receipt", "subset-status"]
)
def test_negative_admission_binds_real_command_receipts(
    tmp_path: Path, mutation: str
) -> None:
    runner = tmp_path / "runner"
    _ = runner.write_bytes(b"isolated validation specimen")
    binaries: Json = {"runner": {"executable": str(runner), "sha256": digest(runner)}}
    job = OrderedSaleRun(tmp_path, tmp_path, tmp_path / "native", binaries, None)
    controls = tmp_path / "controls"
    (controls / "zero").mkdir(parents=True)
    (controls / "subset").mkdir()
    zero = "test result: ok. 0 passed; 0 failed; 0 ignored;\n"
    _ = (controls / "zero/stdout.log").write_text(zero)
    _ = (controls / "subset/stderr.log").write_text(
        "refuses subset or inherited case environment"
    )
    write_json(controls / "zero/process.json", {"returncode": 0, "expected": 0})
    write_json(controls / "subset/process.json", {"returncode": 1, "expected": 1})
    write_json(
        controls / "zero/argv.json",
        [str(runner), "--exact", "runtime::ordered_sale_native::absent", "--ignored"],
    )
    write_json(
        controls / "subset/argv.json",
        [
            "env",
            "OTTD_ORDERED_SALE_CASE=matrix/estimate",
            sys.executable,
            str(tmp_path / "scripts/check-ordered-sale.py"),
        ],
    )
    try:
        require_test(zero, SELECTOR)
    except WorldCheckError as error:
        write_json(
            controls / "zero-rejected.json", {"rejected": True, "reason": str(error)}
        )
    validate_negative(job)
    match mutation:
        case "zero-argv":
            write_json(controls / "zero/argv.json", ["echo", "0 tests"])
        case "subset-argv":
            write_json(controls / "subset/argv.json", ["echo", "refuses subset"])
        case "receipt":
            write_json(
                controls / "zero-rejected.json", {"rejected": False, "reason": "fake"}
            )
        case "subset-status":
            write_json(
                controls / "subset/process.json", {"returncode": 0, "expected": 0}
            )
        case _:
            pytest.fail("unknown admission corruption fixture")
    with pytest.raises(WorldCheckError):
        validate_negative(job)


@pytest.mark.parametrize(
    "path",
    [
        ("initial", "orders", "list_pool", "occupied", 0),
        ("actions", 0, "after", "orders", "list_pool", "occupied", 0),
        ("actions", 0, "after", "orders", "list_pool", "slots"),
        ("actions", 0, "after", "orders", "lists", 0, "members", 0),
    ],
)
def test_allocation_probe_rejects_pool_and_shared_identity_corruption(
    path: tuple[str | int, ...],
) -> None:
    native: Json = {
        "initial": {"orders": {"list_pool": {"occupied": [1], "slots": 128}}},
        "actions": [
            {
                "after": {
                    "orders": {
                        "list_pool": {"occupied": [0, 1], "slots": 128},
                        "lists": [{"id": 0, "members": [4]}],
                    }
                }
            }
        ],
    }
    allocation_probe(native)
    replace(native, path, 999)
    with pytest.raises(WorldCheckError):
        allocation_probe(native)
