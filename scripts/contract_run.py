"""Execute existing comparison drivers and retain fresh evidence receipts."""

from dataclasses import asdict, dataclass
import os
import signal
import json
from pathlib import Path
import subprocess
import time

from contract_model import Contract, ContractError, Driver, Status
from contract_validate import local_path


@dataclass(frozen=True, slots=True)
class DriverResult:
    id: str
    argv: tuple[str, ...]
    passed: bool
    exit_code: int | None
    artifacts: str | None
    error: str | None
    elapsed_seconds: float


def run_driver(driver: Driver, root: Path, directory: Path) -> DriverResult:
    directory.mkdir(parents=True, exist_ok=False)
    artifact_root = (
        None if driver.artifact_root is None else local_path(root, driver.artifact_root)
    )
    prior = (
        set()
        if artifact_root is None or not artifact_root.exists()
        else set(artifact_root.iterdir())
    )
    start = time.monotonic()
    exit_code = None
    error = None
    artifacts = None
    with (
        (directory / "stdout.log").open("w") as stdout,
        (directory / "stderr.log").open("w") as stderr,
    ):
        try:
            with subprocess.Popen(
                driver.argv,
                cwd=root,
                stdout=stdout,
                stderr=stderr,
                start_new_session=True,
            ) as process:
                try:
                    exit_code = process.wait(timeout=driver.timeout_seconds)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    exit_code = process.wait()
                    error = f"driver timeout after {driver.timeout_seconds} seconds"
        except OSError as failure:
            error = str(failure)
    output = (directory / "stdout.log").read_text()
    if error is None and exit_code != 0:
        error = f"driver exit status {exit_code}"
    if error is None and driver.success_text not in output:
        error = "driver success observation missing"
    if error is None and artifact_root is not None:
        announcements = [
            line.removeprefix("Artifacts: ")
            for line in output.splitlines()
            if line.startswith("Artifacts: ")
        ]
        if len(announcements) != 1:
            error = "expected one fresh artifact directory announcement"
        else:
            candidate = Path(announcements[0]).resolve()
            if (
                candidate.parent != artifact_root
                or candidate in prior
                or not candidate.is_dir()
            ):
                error = "artifact directory is stale, missing or outside declared root"
            else:
                artifacts = str(candidate)
                for pattern in driver.artifact_globs:
                    matches = list(candidate.glob(pattern))
                    if not matches or any(
                        not p.resolve().is_relative_to(candidate)
                        or not p.is_file()
                        or p.stat().st_size == 0
                        for p in matches
                    ):
                        error = f"missing or empty required evidence: {pattern}"
                        break
    result = DriverResult(
        driver.id,
        driver.argv,
        error is None,
        exit_code,
        artifacts,
        error,
        round(time.monotonic() - start, 3),
    )
    (directory / "result.json").write_text(json.dumps(asdict(result), indent=2) + "\n")
    return result


def run_contract(
    contract: Contract, selected: str, root: Path, directory: Path
) -> bool:
    drivers = {driver.id: driver for driver in contract.drivers}
    scenarios = {scenario.id: scenario for scenario in contract.scenarios}
    if selected in scenarios:
        scenario = scenarios[selected]
        if scenario.status is Status.FUTURE:
            raise ContractError(
                f"{selected}: unimplemented requirement; no execution or support claim"
            )
        chosen = {scenario.driver}
    elif selected == "baseline":
        chosen = set(drivers)
    elif selected in drivers:
        chosen = {selected}
    else:
        raise ContractError(f"unknown scenario/driver: {selected}")
    directory.mkdir(parents=True, exist_ok=False)
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    dirty = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=root,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    results = []
    for driver in contract.drivers:
        if driver.id in chosen:
            print(f"Running {driver.id}: {' '.join(driver.argv)}", flush=True)
            results.append(run_driver(driver, root, directory / driver.id))
    by_id = {result.id: result for result in results}
    observations = []
    for scenario in contract.scenarios:
        result = by_id.get(scenario.driver)
        observations.append(
            {
                "id": scenario.id,
                "declared_status": scenario.status.value,
                "scope": scenario.scope.value,
                "run_status": "not_run"
                if result is None
                else ("pass" if result.passed else "fail"),
                "driver": scenario.driver,
                "expected": scenario.expected,
                "evidence": []
                if result is None
                else [
                    str(directory / result.id / "result.json"),
                    str(directory / result.id / "stdout.log"),
                    result.artifacts,
                ],
            }
        )
    report = {
        "schema_version": 1,
        "manifest_path": contract.manifest_path,
        "manifest_sha256": contract.manifest_sha256,
        "commit": commit,
        "working_tree_status": dirty,
        "selection": selected,
        "drivers": [asdict(result) for result in results],
        "input_sha256": {asset.path: asset.sha256 for asset in contract.assets},
        "scenarios": observations,
    }
    (directory / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    return all(result.passed for result in results)
