"""Admitted original-backed delegation sources shared with direct Rust regressions."""

from pathlib import Path
from typing import Final

from .case import Case

MEMBERSHIP: Final = (
    "array-numeric",
    "bitwise-precedence",
    "left-associative-chain",
    "no-default-method",
    "own-and-missing",
    "own-null-present",
    "scalar-missing",
    "short-circuit",
    "string-negative",
)
DELEGATION: Final = (
    "attach-own-and-chain",
    "own-shadows-parent",
    "chain-three",
    "detach-alias",
    "set-existing-parent",
    "set-nearest-parent",
    "newslot-own-shadow",
    "in-traverses-delegate",
    "foreach-own-only",
    "delete-own-reveals-parent",
    "delete-inherited-refuses",
    "clone-retains-delegate",
    "cycle-direct-atomic",
    "cycle-indirect-atomic",
    "receiver-error-before-delegate-type",
    "delegate-type-atomic",
    "parent-bare-root",
    "parent-nontable-error",
    "root-delegate-read-write",
    "closure-retained-delegate",
    "ordinary-slot-cycle",
    "delegated-method-this",
    "unicode-nul-key",
    "delegate-evaluation-order",
    "raw-in-skips-get-hook",
    "own-get-dead-hook",
    "own-newslot-dead-hook",
    "foreach-dead-nexti",
    "same-identity-dead-cmp",
    "builtin-type-dead-typeof",
    "immediate-meta-only",
)


def delegate_cases() -> list[Case]:
    fixtures = Path(__file__).resolve().parents[4] / "crates/ottd-script/tests"
    return [
        Case(
            f"{group}-{name}",
            (fixtures / group / f"{name}.nut").read_text(encoding="utf-8"),
            error_trace=True,
        )
        for group, names in (("membership", MEMBERSHIP), ("delegation", DELEGATION))
        for name in names
    ]
