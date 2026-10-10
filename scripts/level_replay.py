from scripts.replay_controls import Mutation, value_control
from scripts.replay_matrix import ReplayMatrix
from scripts.terraform_replay import (
    original_continuation,
    resumed_execution,
    wrong_order,
)


def verify_level(matrix: ReplayMatrix, selected: list[str]) -> None:
    if "level-partial-cash" in selected:
        base = matrix.artifacts / "level-partial-cash"
        for mutation in [
            Mutation(
                "level-extra-money",
                base / "compare/native-results.json",
                ("actions", 0, "receipt", "returns", "result", "additional_money"),
            ),
            Mutation(
                "level-prefix",
                base / "native/final.world.json",
                ("chunks", "MAPH", "bytes", 2056),
            ),
        ]:
            value_control(matrix, mutation)
    if "level-basic" in selected:
        wrong_order(matrix, "level")
    if "level-resume" in selected:
        resumed_execution(matrix, "level")
        original_continuation(matrix, "level")
