# /// script
# requires-python = ">=3.11"
# dependencies = ["pytest"]
# ///
# Run: uv run --with pytest python -m pytest scripts/tests/order_state_evidence_test.py
from copy import deepcopy
from pathlib import Path

import pytest

from scripts.context_ci_support import sources
from scripts.depot_build_archive import bounded_paths, package_raw, verify_archive
from scripts.gameplay_foundations import digest
from scripts.order_state_capture import compare_capture
from scripts.order_state_evidence import command, compare_states
from scripts.world_check_support import Json, WorldCheckError, replace, write_json


def specimen() -> tuple[Json, Json, Json]:
    state: Json = {"backups": [{"pool_slot": 1, "id": 0}], "lists": []}
    action: Json = {"op": "snapshot", "label": "live"}
    native: Json = {
        "schema_version": 1,
        "case": "client",
        "role": {"networking": True, "server": False},
        "initial": state,
        "actions": [
            {
                "index": 0,
                "input": action,
                "before": state,
                "result": None,
                "after": state,
                "role": {"networking": True, "server": False},
            }
        ],
        "final": state,
        "final_role": {"networking": True, "server": False},
    }
    rust = deepcopy(native)
    _ = rust.pop("role")
    _ = rust.pop("final_role")
    rust["declared_context"] = "client"
    rust["exit_requested"] = False
    rust["load_receipt"] = None
    assert isinstance(rust["actions"], list)
    for row in rust["actions"]:
        assert isinstance(row, dict)
        _ = row.pop("role")
    return (
        native,
        rust,
        {
            "schema_version": 1,
            "case": "client",
            "role": "client",
            "exit": False,
            "actions": [action],
        },
    )


def test_complete_state_when_native_indices_differ_from_slots() -> None:
    native, rust, descriptor = specimen()
    compare_states(native, rust, descriptor)


@pytest.mark.parametrize("field", ["id", "pool_slot"])
def test_rejects_identity_normalization_when_loaded_slot_is_nonzero(field: str) -> None:
    native, rust, descriptor = specimen()
    assert isinstance(rust, dict)
    rust["initial"] = {"backups": [{"pool_slot": 1, "id": 0}], "lists": []}
    assert isinstance(rust["initial"], dict)
    rust["initial"]["backups"] = [{"pool_slot": 1, "id": 0, field: 7}]
    with pytest.raises(WorldCheckError, match="state"):
        compare_states(native, rust, descriptor)


def test_rejects_subset_when_both_outputs_drop_actions() -> None:
    native, rust, descriptor = specimen()
    assert isinstance(native, dict)
    assert isinstance(rust, dict)
    native["actions"] = []
    rust["actions"] = []
    with pytest.raises(WorldCheckError, match="membership"):
        compare_states(native, rust, descriptor)


def test_rejects_false_client_role_when_outputs_otherwise_match() -> None:
    native, rust, descriptor = specimen()
    assert isinstance(native, dict)
    native["role"] = {"networking": False, "server": False}
    with pytest.raises(WorldCheckError, match="role"):
        compare_states(native, rust, descriptor)


def test_rejects_extra_metadata_when_native_schema_changes() -> None:
    native, rust, descriptor = specimen()
    assert isinstance(native, dict)
    native["unannounced_observer_field"] = 7
    with pytest.raises(WorldCheckError, match="schema"):
        compare_states(native, rust, descriptor)


def test_rejects_boolean_schema_when_integer_version_required() -> None:
    native, rust, descriptor = specimen()
    assert isinstance(native, dict)
    native["schema_version"] = True
    with pytest.raises(WorldCheckError, match="identity"):
        compare_states(native, rust, descriptor)


def test_rejects_missing_artifact_when_literal_layout_requires_it(
    tmp_path: Path,
) -> None:
    with pytest.raises(WorldCheckError, match="membership"):
        _ = bounded_paths(tmp_path, {"required.json"})


def test_rejects_symlink_when_it_points_inside_evidence(tmp_path: Path) -> None:
    target = tmp_path / "target.json"
    _ = target.write_text("{}")
    (tmp_path / "alias.json").symlink_to(target)
    with pytest.raises(WorldCheckError, match="symlink"):
        _ = bounded_paths(tmp_path, {"target.json", "alias.json"})


def test_rejects_changed_archive_when_raw_file_hash_still_matches(
    tmp_path: Path,
) -> None:
    _ = (tmp_path / "raw.json").write_text("{}")
    package_raw(tmp_path)
    _ = (tmp_path / "evidence.tar.gz").write_bytes(b"altered")
    with pytest.raises(WorldCheckError, match="archive digest"):
        verify_archive(tmp_path)


def test_rejects_changed_source_when_pinned_hash_is_stale(tmp_path: Path) -> None:
    path = tmp_path / "source.py"
    _ = path.write_text("before")
    layout: Json = {"sources": {"source.py": digest(path)}}
    _ = path.write_text("after")
    with pytest.raises(WorldCheckError, match="source changed"):
        sources(tmp_path, layout)


def test_rejects_command_substitution_when_success_receipt_is_unchanged(
    tmp_path: Path,
) -> None:
    write_json(tmp_path / "argv.json", ["wrong-executable", "compare"])
    write_json(tmp_path / "process.json", {"returncode": 0, "expected": 0})
    with pytest.raises(WorldCheckError, match="arguments"):
        command(tmp_path, ["actual-executable", "compare"])


def test_rejects_failed_command_when_success_outputs_exist(tmp_path: Path) -> None:
    write_json(tmp_path / "argv.json", ["actual-executable", "compare"])
    write_json(tmp_path / "process.json", {"returncode": 1, "expected": 0})
    with pytest.raises(WorldCheckError, match="status"):
        command(tmp_path, ["actual-executable", "compare"])


def capture_document() -> Json:
    role: Json = {
        "networking": True,
        "server": False,
        "dedicated": False,
        "own_client_id": 2,
        "clients": [{"id": 1, "name": "native-order-client", "company": 255}],
    }
    return {
        "schema_version": 1,
        "case": "capture",
        "initial": {"backup": 7},
        "final": {"backup": 7},
        "role": deepcopy(role),
        "final_role": deepcopy(role),
        "actions": [],
    }


def test_capture_allows_monotonic_own_client_packet_arrival_only() -> None:
    left, right = capture_document(), capture_document()
    replace(
        right,
        ("final_role", "clients"),
        [
            {"id": 1, "name": "native-order-client", "company": 255},
            {"id": 2, "name": "native-order-client #1", "company": 255},
        ],
    )
    assert len(compare_capture(left, right, "native-order-client")) == 1


@pytest.mark.parametrize(
    ("path", "replacement"),
    [
        (("role", "networking"), False),
        (("role", "server"), True),
        (("role", "dedicated"), True),
        (("role", "own_client_id"), 3),
        (("role", "clients", 0, "id"), 9),
        (("role", "clients", 0, "name"), "other"),
        (("role", "clients", 0, "company"), 0),
        (("initial", "backup"), 8),
    ],
)
def test_capture_rejects_changes_outside_arrival_boundary(
    path: tuple[str | int, ...], replacement: Json
) -> None:
    left, right = capture_document(), capture_document()
    replace(right, path, replacement)
    with pytest.raises(WorldCheckError):
        _ = compare_capture(left, right, "native-order-client")


def test_capture_rejects_unrecognized_clients_location() -> None:
    left, right = capture_document(), capture_document()
    assert isinstance(right, dict)
    right["clients"] = []
    with pytest.raises(WorldCheckError):
        _ = compare_capture(left, right, "native-order-client")


def test_capture_rejects_client_disappearance() -> None:
    left, right = capture_document(), capture_document()
    replace(
        right,
        ("role", "clients"),
        [
            {"id": 1, "name": "native-order-client", "company": 255},
            {"id": 2, "name": "native-order-client #1", "company": 255},
        ],
    )
    with pytest.raises(WorldCheckError, match="monotonic"):
        _ = compare_capture(left, right, "native-order-client")
