#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Run: python3 scripts/check-contract.py --validate | --list | --run baseline."""

import argparse
import json
from pathlib import Path
import sys
import tempfile

from contract_model import ContractError
from contract_run import run_contract
from contract_validate import load_contract


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--validate", action="store_true")
    action.add_argument("--list", action="store_true")
    action.add_argument("--run", metavar="DRIVER_OR_SCENARIO")
    parser.add_argument(
        "--contract", type=Path, default=Path("compatibility/contract.json")
    )
    parser.add_argument(
        "--run-dir", type=Path, help="fresh report directory; must not exist"
    )
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    try:
        contract = load_contract(root / args.contract, root)
        if args.validate:
            print(
                f"VALID: {len(contract.scenarios)} scenario declarations; no scenarios executed"
            )
            return 0
        if args.list:
            for scenario in contract.scenarios:
                print(
                    f"{scenario.id}\t{scenario.status.value}\t{scenario.scope.value}\t{scenario.driver or '-'}"
                )
            return 0
        if args.run_dir is None:
            parent = root / ".omo/evidence"
            parent.mkdir(parents=True, exist_ok=True)
            # Reserve a unique name; run_contract itself requires an absent directory.
            directory = Path(tempfile.mkdtemp(prefix="contract-", dir=parent))
            directory.rmdir()
        else:
            directory = args.run_dir.resolve()
        passed = run_contract(contract, args.run, root, directory)
        print(f"Artifacts: {directory}")
        print(
            "PASS: selected drivers"
            if passed
            else "FAIL: see report.json and driver logs"
        )
        return 0 if passed else 1
    except (ContractError, OSError, json.JSONDecodeError, ValueError) as error:
        print(f"contract error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
