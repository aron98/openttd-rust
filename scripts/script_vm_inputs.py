"""Current ScriptVM Cargo, native-observer and Python producer closure."""

from pathlib import Path
from typing import Final

from scripts.owned_restore_sources import crate_inputs, regular

OBSERVER_INPUTS: Final = (
    "observe.cpp",
    "strings.hpp",
    "constants.hpp",
    "root_slots.hpp",
    "arrays.hpp",
    "tables.hpp",
    "calls.hpp",
    "closures.hpp",
)


def source_names(root: Path) -> tuple[str, ...]:
    names = set(crate_inputs(root, ("ottd-script",), ("src", "tests", "examples")))
    names.update(
        regular(root, root / "scripts/compat/script-vm" / name)
        for name in (*OBSERVER_INPUTS, "build-native.sh")
    )
    names.update(
        regular(root, path)
        for subtree in ("tables", "varargs")
        for path in (root / "scripts/compat/script-vm" / subtree).rglob("*")
        if path.is_file() and "__pycache__" not in path.parts
    )
    names.update(
        regular(root, root / name)
        for name in (
            "scripts/script_vm_inputs.py",
            "scripts/owned_restore_sources.py",
            "scripts/script_vm_provenance.py",
            "scripts/tests/script_vm_tables_bindings_test.py",
            "scripts/tests/test_script_vm_table_storage.py",
            ".github/workflows/script-tables.yml",
        )
    )
    names.update(
        regular(root, path)
        for part in ("roots", "arrays", "branches")
        for path in (root / "scripts").glob(f"script_vm_{part}*.py")
        if path.is_file()
    )
    return tuple(sorted(names))
