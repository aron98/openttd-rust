"""Bind actual consumed ordered baselines to immutable pre-launch file inputs."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path

from .value import Json, array, field, integer, keys, require, same, string


@dataclass(frozen=True, slots=True)
class FileInput:
    path: Path
    size: int
    digest: str
    device: int
    inode: int

    @classmethod
    def capture(cls, path: Path) -> FileInput:
        require(
            "baseline must be an absolute regular file",
            condition=path.is_absolute() and path.is_file() and (not path.is_symlink()),
        )
        before = path.stat()
        require("unusable filesystem identity", condition=before.st_ino > 0)
        data = path.read_bytes()
        after = path.stat()
        require(
            "input changed while hashing",
            condition=(before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
            == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
            and len(data) == before.st_size,
        )
        return cls(
            path,
            len(data),
            hashlib.sha256(data).hexdigest(),
            before.st_dev,
            before.st_ino,
        )

    def verify(self) -> None:
        require(
            "baseline input changed during process",
            condition=self == FileInput.capture(self.path),
        )


@dataclass(frozen=True, slots=True)
class SelectedInput:
    inventory: FileInput
    actual: FileInput


def bind(
    receipt: Json, candidates: tuple[FileInput, ...], baseset: Path
) -> tuple[SelectedInput, SelectedInput]:
    """Reject guessed/missing paths, reordered inputs and any pre/post byte drift."""
    keys(
        receipt,
        {
            "schema_version",
            "scope",
            "arm_seam",
            "consume_seam",
            "arms",
            "loads",
            "ignored_menu_arms",
            "ignored_loads",
            "ordered_sources",
        },
    )
    require("baseline schema", condition=same(field(receipt, "schema_version"), 1))
    require(
        "baseline scope",
        condition=field(receipt, "scope") == "actual-save-load-baseset-selection",
    )
    require(
        "baseline arm seam",
        condition=field(receipt, "arm_seam") == "AfterLoadGame.before.GfxLoadSprites",
    )
    require(
        "baseline consume seam",
        condition=field(receipt, "consume_seam") == "LoadNewGRF.entry",
    )
    require(
        "baseline lifecycle",
        condition=integer(field(receipt, "arms"))
        == integer(field(receipt, "loads"))
        == 1,
    )
    require(
        "baseline counters",
        condition=integer(field(receipt, "ignored_menu_arms")) >= 0
        and integer(field(receipt, "ignored_loads")) >= 0,
    )
    rows = array(field(receipt, "ordered_sources"))
    require("baseline exact ordered pair", condition=len(rows) == 2)
    require(
        "duplicate baseline candidate",
        condition=len({item.path for item in candidates}) == len(candidates),
    )
    selected: list[SelectedInput] = []
    for index, row in enumerate(rows):
        keys(
            row,
            {"ordinal", "path", "filename", "grfid", "parameters", "flags", "bytes"},
        )
        require("baseline order", condition=integer(field(row, "ordinal")) == index)
        path = Path(string(field(row, "path")))
        require(
            "baseline path escaped baseset",
            condition=path.resolve().is_relative_to(baseset.resolve()),
        )
        actual = FileInput.capture(path)
        matches = [
            item
            for item in candidates
            if (item.device, item.inode) == (actual.device, actual.inode)
        ]
        require(
            "actual baseline physical identity missing or ambiguous",
            condition=len(matches) == 1,
        )
        item = matches[0]
        item.verify()
        require(
            "actual baseline byte identity changed",
            condition=item.digest == actual.digest
            and item.size == actual.size
            and item.path.samefile(actual.path),
        )
        require(
            "baseline observed byte count",
            condition=integer(field(row, "bytes")) == item.size,
        )
        filename = string(field(row, "filename"))
        if index == 0:
            require(
                "original builtin requested filename",
                condition=filename == "OPENTTD.GRF",
            )
            require(
                "original builtin resolved basename",
                condition=path.name in {"OPENTTD.GRF", "openttd.grf"},
            )
        else:
            require(
                "baseline filename/path attribution",
                condition=not Path(filename).is_absolute()
                and ".." not in Path(filename).parts
                and path == baseset / filename,
            )
        require(
            "baseline grfid", condition=0 <= integer(field(row, "grfid")) <= 4294967295
        )
        require("baseline flags", condition=0 <= integer(field(row, "flags")) <= 255)
        require(
            "baseline parameters",
            condition=all(
                0 <= integer(value) <= 4294967295
                for value in array(field(row, "parameters"))
            ),
        )
        selected.append(SelectedInput(item, actual))
    require(
        "baseline original builtin must be first",
        condition=selected[0].actual.path.name in {"openttd.grf", "OPENTTD.GRF"},
    )
    require(
        "baseline duplicate source",
        condition=(selected[0].actual.device, selected[0].actual.inode)
        != (selected[1].actual.device, selected[1].actual.inode),
    )
    return (selected[0], selected[1])
