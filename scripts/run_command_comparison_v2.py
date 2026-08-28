#!/usr/bin/env python3
"""Run the expanded Stata 19 command matrix as resumable scope shards."""
from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path
import subprocess
import sys
from typing import Any, Mapping, Sequence

if __package__:
    from scripts import command_comparison_v2 as comparison
else:
    import command_comparison_v2 as comparison


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_STATA = Path("/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp")
HARNESS = ROOT / "benchmarks/stata/command_comparison_stata19_v2.do"


class RunError(RuntimeError):
    """The expanded benchmark could not be run or resumed safely."""


def scope_name(scope: Mapping[str, Any]) -> str:
    return (
        f"{scope['scale']['id']}-{scope['design']['id']}-"
        f"k{scope['controls']}-t{scope['requested_threads']}"
    )


def dataset_path(output_root: Path, scope: Mapping[str, Any]) -> Path:
    return (
        output_root
        / "datasets"
        / f"{scope['scale']['id']}-{scope['design']['dataset']}.dta"
    )


def shard_path(output_root: Path, scope: Mapping[str, Any]) -> Path:
    return output_root / "shards" / scope_name(scope)


def _read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8-sig") as source:
        return list(csv.DictReader(source))


def validate_shard(scope: Mapping[str, Any], directory: Path) -> None:
    required = {
        "SUCCESS.txt",
        "environment.txt",
        comparison.RUNS_NAME,
        comparison.PREFLIGHTS_NAME,
    }
    if not required <= {item.name for item in directory.iterdir()}:
        raise RunError(f"completed shard is incomplete: {directory}")
    if "status=success" not in (directory / "SUCCESS.txt").read_text(
        encoding="utf-8"
    ):
        raise RunError(f"completed shard has no success receipt: {directory}")
    rows = _read_csv(directory / comparison.RUNS_NAME)
    preflights = _read_csv(directory / comparison.PREFLIGHTS_NAME)
    expected = {
        (mode, phase, rep, arm)
        for mode in ("fit", "save_fe")
        for phase, repetitions in (("cold", {0}), ("warm", {1, 2, 3, 4, 5}))
        for rep in repetitions
        for arm in ("fereg", "greg", "reghdfe")
    }
    observed = {
        (row["mode"], row["phase"], int(row["rep"]), row["arm"])
        for row in rows
    }
    if len(rows) != len(expected) or observed != expected or len(preflights) != 1:
        raise RunError(f"completed shard has an invalid cadence: {directory}")
    case_id = scope["case_id"]
    requested = scope["requested_threads"]
    if any(
        row["case_id"] != case_id
        or int(row["requested_threads"]) != requested
        or int(row["rc"]) != 0
        or (
            row["arm"] == "fereg"
            and int(float(row["fereg_effective_threads"])) != requested
        )
        for row in rows
    ):
        raise RunError(f"completed shard failed its runtime contract: {directory}")
    preflight = preflights[0]
    if (
        preflight["case_id"] != case_id
        or int(preflight["requested_threads"]) != requested
        or int(preflight["science_fail"]) != 0
    ):
        raise RunError(f"completed shard failed its science gate: {directory}")


def _stata_command(
    *,
    stata: Path,
    repo_root: Path,
    output_dir: Path,
    comparator_root: str,
    requested_threads: int,
    scale: str,
    design: str,
    controls: int,
    warm_reps: int,
    preflight: str,
    arm: str,
    mode: str,
    cold: str,
    data_path: Path,
) -> list[str]:
    return [
        str(stata),
        "-b",
        "do",
        str(HARNESS),
        str(repo_root),
        str(output_dir),
        str(requested_threads),
        scale,
        str(warm_reps),
        comparator_root or "system",
        design,
        str(controls),
        "0",
        preflight,
        arm,
        mode,
        cold,
        str(data_path),
    ]


def _run_stata(command: Sequence[str], directory: Path, timeout: int) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    stdout_path = directory / "runner.stdout.txt"
    with stdout_path.open("w", encoding="utf-8") as output:
        try:
            result = subprocess.run(
                command,
                cwd=directory,
                stdout=output,
                stderr=subprocess.STDOUT,
                timeout=timeout,
                check=False,
            )
        except subprocess.TimeoutExpired as error:
            raise RunError(f"Stata scope timed out after {timeout}s: {directory}") from error
    if result.returncode != 0:
        raise RunError(
            f"Stata exited with {result.returncode}; inspect {stdout_path} and the .log"
        )


def prepare_datasets(
    manifest: Mapping[str, Any], output_root: Path, *, stata: Path,
    comparator_root: str, timeout: int,
) -> None:
    representatives: dict[tuple[str, str], Mapping[str, Any]] = {}
    for scope in comparison.expand_scopes(manifest):
        key = (scope["scale"]["id"], scope["design"]["dataset"])
        representatives.setdefault(key, scope)
    for key, scope in sorted(representatives.items()):
        data = dataset_path(output_root, scope)
        if data.is_file():
            continue
        build_dir = output_root / "data-build" / f"{key[0]}-{key[1]}"
        data.parent.mkdir(parents=True, exist_ok=True)
        command = _stata_command(
            stata=stata,
            repo_root=ROOT,
            output_dir=build_dir,
            comparator_root=comparator_root,
            requested_threads=1,
            scale=scope["scale"]["id"],
            design=scope["design"]["id"],
            controls=0,
            warm_reps=0,
            preflight="off",
            arm="none",
            mode="none",
            cold="off",
            data_path=data,
        )
        _run_stata(command, build_dir, timeout)
        if not data.is_file() or not (build_dir / "SUCCESS.txt").is_file():
            raise RunError(f"dataset build failed closed: {data}")


def run_scope(
    scope: Mapping[str, Any], output_root: Path, *, stata: Path,
    comparator_root: str, timeout: int,
) -> str:
    directory = shard_path(output_root, scope)
    success = directory / "SUCCESS.txt"
    if success.is_file():
        validate_shard(scope, directory)
        return "skipped"
    data = dataset_path(output_root, scope)
    if not data.is_file():
        raise RunError(f"scope dataset is absent: {data}")
    command = _stata_command(
        stata=stata,
        repo_root=ROOT,
        output_dir=directory,
        comparator_root=comparator_root,
        requested_threads=scope["requested_threads"],
        scale=scope["scale"]["id"],
        design=scope["design"]["id"],
        controls=scope["controls"],
        warm_reps=5,
        preflight="on",
        arm="all",
        mode="both",
        cold="on",
        data_path=data,
    )
    _run_stata(command, directory, timeout)
    validate_shard(scope, directory)
    return "completed"


def merge_and_analyze(
    manifest: Mapping[str, Any], output_root: Path, manifest_path: Path,
) -> dict[str, int]:
    scopes = comparison.expand_scopes(manifest)
    shards = [shard_path(output_root, scope) for scope in scopes]
    for scope, shard in zip(scopes, shards, strict=True):
        validate_shard(scope, shard)
    merged = output_root / "merged"
    counts = comparison.merge_shards(manifest, shards, merged)
    evidence = comparison.analyze(
        manifest,
        merged / comparison.RUNS_NAME,
        merged / comparison.PREFLIGHTS_NAME,
        merged / "environment.txt",
        manifest_path,
    )
    (merged / "evidence.json").write_text(
        json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (merged / "report.md").write_text(
        comparison.render_markdown(evidence), encoding="utf-8"
    )
    (merged / "SUCCESS.txt").write_text(
        "status=success\nscopes=116\n", encoding="utf-8"
    )
    return counts


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=comparison.DEFAULT_MANIFEST)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--stata", type=Path, default=DEFAULT_STATA)
    parser.add_argument("--comparator-root", default="")
    parser.add_argument("--timeout", type=int, default=7200)
    actions = parser.add_subparsers(dest="action", required=True)
    actions.add_parser("prepare-data")
    scope_parser = actions.add_parser("run-scope")
    scope_parser.add_argument("--index", type=int, required=True)
    actions.add_parser("run-all")
    actions.add_parser("merge")
    args = parser.parse_args(argv)
    try:
        if not args.stata.is_file():
            raise RunError(f"Stata executable is absent: {args.stata}")
        if args.timeout <= 0:
            raise RunError("timeout must be positive")
        manifest = comparison.load_manifest(args.manifest)
        scopes = comparison.expand_scopes(manifest)
        args.output_root.mkdir(parents=True, exist_ok=True)
        if args.action in {"prepare-data", "run-all"}:
            prepare_datasets(
                manifest,
                args.output_root,
                stata=args.stata,
                comparator_root=args.comparator_root,
                timeout=args.timeout,
            )
        if args.action == "run-scope":
            if args.index < 1 or args.index > len(scopes):
                raise RunError(f"scope index must be between 1 and {len(scopes)}")
            state = run_scope(
                scopes[args.index - 1],
                args.output_root,
                stata=args.stata,
                comparator_root=args.comparator_root,
                timeout=args.timeout,
            )
            print(f"scope {args.index}/{len(scopes)} {state}: {scope_name(scopes[args.index - 1])}")
        elif args.action == "run-all":
            for index, scope in enumerate(scopes, start=1):
                state = run_scope(
                    scope,
                    args.output_root,
                    stata=args.stata,
                    comparator_root=args.comparator_root,
                    timeout=args.timeout,
                )
                print(f"scope {index}/{len(scopes)} {state}: {scope_name(scope)}", flush=True)
            print(json.dumps(merge_and_analyze(manifest, args.output_root, args.manifest)))
        elif args.action == "merge":
            print(json.dumps(merge_and_analyze(manifest, args.output_root, args.manifest)))
        return 0
    except (RunError, comparison.ContractError, OSError, ValueError) as error:
        print(f"run_command_comparison_v2: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
