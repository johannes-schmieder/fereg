#!/usr/bin/env python3
"""Run and analyze the non-qualifying explicit PPML FE-solver experiment."""
from __future__ import annotations

import argparse
import csv
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys
import tomllib
from typing import Iterable
import importlib.util


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "benchmarks/manifests/feppml_solver_development_v1.json"
DRIVER = ROOT / "tests/stata/development/feppml_solver_development.do"
TRACE_PREFIX = "FEREG_PPML_SOLVER_TRACE|schema=1"
REFERENCE_ARMS = ("ppmlhdfe", "ppmlhdfe_lsmr")
CANDIDATE_ARMS = (
    "cg",
    "lsmr",
    "cmg",
    "cmg_fused",
    "lsmr_legacy_diagonal",
    "lsmr_fused_diagonal",
    "lsmr_fused_reuse",
    "lsmr_fused_rebuild",
    "lsmr_joint_diagonal",
    "lsmr_joint_reuse",
    "lsmr_joint_rebuild",
)
ARMS = ("map", *CANDIDATE_ARMS, *REFERENCE_ARMS)
IMPLEMENTATION_FILES = (
    "Cargo.lock",
    "benchmarks/manifests/feppml_solver_development_v1.json",
    "benchmarks/scc/feppml_solver_dev_job.sh",
    "crates/fereg-core/Cargo.toml",
    "crates/fereg-core/src/fe/cmg.rs",
    "crates/fereg-core/src/fe/map.rs",
    "crates/fereg-core/src/ols/linear_session.rs",
    "crates/fereg-core/src/ppml/mod.rs",
    "crates/fereg-core/src/ppml/categorical_fwl_lsmr.rs",
    "crates/fereg-core/src/ppml/categorical_fwl_cmg.rs",
    "crates/fereg-core/src/ppml/categorical_lsmr.rs",
    "crates/fereg-core/src/ppml/prepared_estimator.rs",
    "crates/fereg-core/src/ppml/prepared_irls.rs",
    "crates/fereg-core/src/protocol.rs",
    "crates/fereg-plugin/src/ppml_estimate.rs",
    "docs/adr/0022-private-ppml-solver-development-boundary.md",
    "docs/adr/0023-private-ppml-fused-cmg.md",
    "docs/adr/0024-private-ppml-prepared-cmg.md",
    "references.lock.toml",
    "scripts/ppml_solver_development.py",
    "stata/_fereg_estimate_ppml.ado",
    "tests/python/test_ppml_solver_development.py",
    "tests/python/test_ppml_stata_transaction_contract.py",
    "tests/stata/development/feppml_solver_development.do",
)


class DevelopmentError(RuntimeError):
    pass


def canonical_bytes(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n"
    ).encode("utf-8")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def load_manifest() -> dict[str, object]:
    value = json.loads(MANIFEST.read_text(encoding="utf-8"))
    if value.get("qualification_eligible") is not False:
        raise DevelopmentError("solver-development manifest must remain non-qualifying")
    if value.get("request_wire") != "1.4" or value.get("result_wire") != "1.1":
        raise DevelopmentError("solver-development manifest has stale PPML wire identities")
    return value


def matrix_cells(manifest: dict[str, object], profile: str) -> list[dict[str, object]]:
    sections = ["science_matrix", "screening", "scale_thread"] if profile == "all" else [profile]
    cells: list[dict[str, object]] = []
    for section_name in sections:
        section = manifest[section_name]
        assert isinstance(section, dict)
        for topology in section["topologies"]:
            for controls in section["controls"]:
                for difficulty in section["difficulty"]:
                    for threads in section["threads"]:
                        cells.append(
                            {
                                "profile": section_name,
                                "n": int(section["observations"]),
                                "topology": str(topology),
                                "controls": int(controls),
                                "difficulty": str(difficulty),
                                "threads": int(threads),
                            }
                        )
    for section_name in sections:
        section = manifest[section_name]
        assert isinstance(section, dict)
        observed = sum(cell["profile"] == section_name for cell in cells)
        if observed != int(section["expected_cells"]):
            raise DevelopmentError(
                f"{section_name} cell inventory is {observed}, expected {section['expected_cells']}"
            )
    return cells


def cell_id(cell: dict[str, object]) -> str:
    return (
        f"{cell['profile']}-n{cell['n']}-{cell['topology']}-k{cell['controls']}-"
        f"{cell['difficulty']}-t{cell['threads']}"
    )


def select_cells(
    cells: list[dict[str, object]],
    *,
    topology: str | None = None,
    controls: int | None = None,
    difficulty: str | None = None,
    threads: int | None = None,
) -> list[dict[str, object]]:
    selected = [
        cell
        for cell in cells
        if (topology is None or cell["topology"] == topology)
        and (controls is None or cell["controls"] == controls)
        and (difficulty is None or cell["difficulty"] == difficulty)
        and (threads is None or cell["threads"] == threads)
    ]
    if not selected:
        raise DevelopmentError("solver-development cell filters selected no cells")
    return selected


def arm_applicable(arm: str, cell: dict[str, object]) -> bool:
    if arm != "cmg_fused" or not str(cell.get("topology", "")).endswith("_3fe"):
        return True
    observations = int(cell.get("n", 0))
    if observations <= 1_000:
        third_effect_levels = 8
    elif cell.get("difficulty") == "long":
        third_effect_levels = min(2_500, observations // 5)
    else:
        third_effect_levels = 40
    return max(0, third_effect_levels - 1) <= 64


def source_identity() -> dict[str, object]:
    def git(*arguments: str) -> str:
        return subprocess.check_output(
            ["git", "-C", str(ROOT), *arguments], text=True, stderr=subprocess.STDOUT
        ).strip()

    diff = subprocess.check_output(
        ["git", "-C", str(ROOT), "diff", "--binary", "--", ":(exclude)benchmarks/reports/development"],
    )
    implementation_files: dict[str, str] = {}
    for relative in IMPLEMENTATION_FILES:
        path = ROOT / relative
        if not path.is_file():
            raise DevelopmentError(f"missing solver-development source input: {relative}")
        implementation_files[relative] = sha256_file(path)
    return {
        "commit": git("rev-parse", "HEAD^{commit}"),
        "tree": git("rev-parse", "HEAD^{tree}"),
        "working_diff_sha256": hashlib.sha256(diff).hexdigest(),
        "dirty": bool(git("status", "--short")),
        "implementation_files": implementation_files,
    }


def reference_identity(reference_root: Path) -> dict[str, object]:
    lock = tomllib.loads((ROOT / "references.lock.toml").read_text(encoding="utf-8"))
    records = {record["name"]: record for record in lock["reference"]}
    output: dict[str, object] = {}
    for name in ("require", "ftools", "reghdfe", "ppmlhdfe"):
        record = records[name]
        base = reference_root / name
        observed: dict[str, str] = {}
        for relative, expected in record["required_capture_sha256"].items():
            path = base / relative
            if not path.is_file():
                raise DevelopmentError(f"missing pinned comparator file: {path}")
            digest = sha256_file(path)
            if digest != expected:
                raise DevelopmentError(f"pinned comparator digest mismatch: {path}")
            observed[relative] = digest
        output[name] = {
            "commit": record["commit"],
            "required_files": observed,
        }
        if "source_archive_url" in record:
            archive = reference_root / f"{name}.tar.gz"
            if not archive.is_file():
                raise DevelopmentError(f"missing pinned comparator archive: {archive}")
            archive_digest = sha256_file(archive)
            if archive_digest != record["source_archive_sha256"]:
                raise DevelopmentError(
                    f"pinned comparator archive digest mismatch: {archive}"
                )
            if archive.stat().st_size != int(record["source_archive_size_bytes"]):
                raise DevelopmentError(
                    f"pinned comparator archive size mismatch: {archive}"
                )
            output[name]["source_archive"] = {
                "url": record["source_archive_url"],
                "size_bytes": archive.stat().st_size,
                "sha256": archive_digest,
            }
    generated_mlibs = sorted(reference_root.rglob("lftools.mlib"))
    if generated_mlibs:
        raise DevelopmentError(
            "private comparator root must not contain a preexisting lftools.mlib"
        )
    return output


def candidate_artifact_identity() -> dict[str, object]:
    system = platform.system()
    plugin_name = {
        "Darwin": "fereg_macos.plugin",
        "Linux": "fereg_linux.plugin",
        "Windows": "fereg_windows.plugin",
    }.get(system)
    if plugin_name is None:
        raise DevelopmentError(f"unsupported solver-development host: {system}")
    artifact = ROOT / "dist" / plugin_name
    if not artifact.is_file():
        raise DevelopmentError(f"dist/{plugin_name} is missing; build before benchmarking")
    return {
        "name": artifact.name,
        "size_bytes": artifact.stat().st_size,
        "sha256": sha256_file(artifact),
    }


def exclusive_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    with os.fdopen(descriptor, "wb") as target:
        target.write(canonical_bytes(value))
        target.flush()
        os.fsync(target.fileno())


def run_stata(stata: Path, work: Path, environment: dict[str, str], log: Path) -> int:
    process_environment = os.environ.copy()
    process_environment["FEREG_PPML_SOLVER_STATA_LOG"] = str(
        log.with_name(f"{log.stem}.stata.log")
    )
    process_environment.update(environment)
    with log.open("xb") as output:
        completed = subprocess.run(
            [str(stata), "-b", "do", str(DRIVER)],
            cwd=work,
            env=process_environment,
            stdin=subprocess.DEVNULL,
            stdout=output,
            stderr=subprocess.STDOUT,
            check=False,
        )
    return completed.returncode


def base_environment(
    *, cell: dict[str, object], dataset: Path, result: Path, reference_root: Path
) -> dict[str, str]:
    return {
        "FEREG_PPML_SOLVER_REPOSITORY": str(ROOT),
        "FEREG_PPML_SOLVER_DATASET": str(dataset),
        "FEREG_PPML_SOLVER_RESULT": str(result),
        "FEREG_PPML_SOLVER_REFERENCE_ROOT": str(reference_root),
        "FEREG_PPML_SOLVER_N": str(cell["n"]),
        "FEREG_PPML_SOLVER_K": str(cell["controls"]),
        "FEREG_PPML_SOLVER_TOPOLOGY": str(cell["topology"]),
        "FEREG_PPML_SOLVER_DIFFICULTY": str(cell["difficulty"]),
        "FEREG_PPML_SOLVER_THREADS": str(cell["threads"]),
        "FEREG_PPML_SOLVER_SEED": str(cell.get("seed", 20260901)),
        "FEREG_PPML_EXPERIMENTAL_SOLVERS": "1",
    }


def explicit_dummy_oracle(data_csv: Path, controls: int) -> dict[str, object]:
    oracle_path = ROOT / "tests/python/ppml_explicit_dummy_oracle.py"
    specification = importlib.util.spec_from_file_location("fereg_ppml_dummy_oracle", oracle_path)
    if specification is None or specification.loader is None:
        raise DevelopmentError("unable to load independent explicit-dummy PPML oracle")
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    with data_csv.open(encoding="utf-8", newline="") as source:
        rows = list(csv.DictReader(source))
    outcome = [float(row["y"]) for row in rows]
    design: list[list[float]] = [
        [float(row[f"x{index}"]) for index in range(1, controls + 1)]
        for row in rows
    ]
    group_names = [name for name in ("g1", "g2", "g3") if name in rows[0]]
    for group_name in group_names:
        values = [int(float(row[group_name])) for row in rows]
        dummies, _levels = module.dummy_columns(values)
        for target, additions in zip(design, dummies):
            target.extend(additions)
    for row in design:
        row.append(1.0)
    fitted = module.fit_explicit_ppml(
        design,
        outcome,
        vce="robust",
        tolerance=1e-11,
        max_iterations=500,
    )
    return {
        "sample_n": float(len(rows)),
        "deviance": float(fitted.deviance),
        "control_names": [f"x{index}" for index in range(1, controls + 1)],
        "control_values": [float(value) for value in fitted.coefficients[:controls]],
        "iterations": fitted.iterations,
    }


def parse_table(path: Path, cell: dict[str, object]) -> list[dict[str, object]]:
    with path.open(encoding="utf-8", newline="") as source:
        rows = list(csv.DictReader(source, delimiter="\t"))
    output: list[dict[str, object]] = []
    for row in rows:
        values = [float(value) for value in row["b_values"].split()]
        covariance = [float(value) for value in row["v_values"].split()]
        output.append(
            {
                **cell,
                "cell_id": cell_id(cell),
                "arm": row["arm"],
                "repetition": int(row["repetition"]),
                "rc": int(row["rc"]),
                "seconds": float(row["seconds"]),
                "sample_n": float(row["N"]),
                "ll": float(row["ll"]),
                "deviance": float(row["deviance"]),
                "converged": float(row["converged"]),
                "rank": float(row["rank"]),
                "df_a": float(row["df_a"]),
                "df_resid": stata_optional_float(row["df_resid"]),
                "b_names": row["b_names"].split(),
                "b_values": values,
                "v_names": row["v_names"].split(),
                "v_values": covariance,
            }
        )
    return output


def stata_optional_float(value: str) -> float | None:
    stripped = value.strip()
    if stripped == "." or (
        len(stripped) == 2 and stripped.startswith(".") and stripped[1].isalpha()
    ):
        return None
    return float(stripped)


def parse_trace(path: Path) -> list[dict[str, str]]:
    events: list[dict[str, str]] = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        marker = line.find(TRACE_PREFIX)
        if marker < 0:
            continue
        fields: dict[str, str] = {}
        for part in line[marker:].split("|")[2:]:
            if "=" in part:
                key, value = part.split("=", 1)
                fields[key] = value
        events.append(fields)
    return events


def parse_trace_logs(console: Path) -> list[dict[str, str]]:
    paths = [console, console.with_name(f"{console.stem}.stata.log")]
    events: list[dict[str, str]] = []
    for path in paths:
        if path.is_file():
            events.extend(parse_trace(path))
    return events


def process_log_text(console: Path) -> str:
    paths = [console, console.with_name(f"{console.stem}.stata.log")]
    return "\n".join(
        path.read_text(encoding="utf-8", errors="replace")
        for path in paths
        if path.is_file()
    )


def stata_stage_status(
    console: Path, process_rc: int, success_marker: str
) -> tuple[int, str]:
    text = process_log_text(console)
    failure = re.search(
        r"^FEREG_PPML_SOLVER_RUN_FAIL\|arm=[^|]+\|rc=(\d+)\s*$",
        text,
        flags=re.MULTILINE,
    )
    if process_rc != 0:
        return process_rc, "process-exit"
    if failure is not None:
        return int(failure.group(1)), "failure-sentinel"
    if success_marker not in text:
        return 999, "missing-success-sentinel"
    return 0, "success-sentinel"


def run_profile(args: argparse.Namespace) -> Path:
    manifest = load_manifest()
    cells = select_cells(
        matrix_cells(manifest, args.profile),
        topology=args.topology,
        controls=args.controls,
        difficulty=args.difficulty,
        threads=args.threads,
    )
    if args.n_override is not None:
        for cell in cells:
            cell["n"] = args.n_override
    if args.limit is not None:
        cells = cells[: args.limit]
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    output = args.output_dir.resolve() / stamp
    output.mkdir(parents=True, exist_ok=False)
    reference = reference_identity(args.reference_root.resolve())
    artifact = candidate_artifact_identity()
    records: list[dict[str, object]] = []
    process_records: list[dict[str, object]] = []
    repetitions = args.repetitions
    for index, cell in enumerate(cells):
        identity = cell_id(cell)
        cell_dir = output / identity
        cell_dir.mkdir()
        dataset = cell_dir / "data.dta"
        empty_result = cell_dir / "unused.tsv"
        environment = base_environment(
            cell=cell,
            dataset=dataset,
            result=empty_result,
            reference_root=args.reference_root.resolve(),
        )
        environment["FEREG_PPML_SOLVER_MODE"] = "generate"
        data_csv = cell_dir / "data.csv"
        environment["FEREG_PPML_SOLVER_DATA_CSV"] = str(data_csv)
        generation_log = cell_dir / "generate.log"
        process_rc = run_stata(args.stata.resolve(), cell_dir, environment, generation_log)
        rc, terminal = stata_stage_status(
            generation_log, process_rc, "FEREG_PPML_SOLVER_DATA_PASS|"
        )
        process_records.append(
            {
                "cell_id": identity,
                "stage": "generate",
                "rc": rc,
                "process_rc": process_rc,
                "terminal": terminal,
            }
        )
        if rc != 0 or not dataset.is_file():
            raise DevelopmentError(f"data generation failed for {identity}; see {generation_log}")
        if cell["profile"] == "science_matrix":
            records.append(
                {
                    **cell,
                    "cell_id": identity,
                    "arm": "independent_explicit_dummy",
                    "rc": 0,
                    **explicit_dummy_oracle(data_csv, int(cell["controls"])),
                }
            )
        for arm in args.arms:
            if not arm_applicable(arm, cell):
                process_records.append(
                    {
                        "cell_id": identity,
                        "stage": arm,
                        "rc": 0,
                        "process_rc": 0,
                        "terminal": "inapplicable-nuisance-width",
                    }
                )
                continue
            result = cell_dir / f"{arm}.tsv"
            arm_environment = base_environment(
                cell=cell,
                dataset=dataset,
                result=result,
                reference_root=args.reference_root.resolve(),
            )
            arm_environment.update(
                {
                    "FEREG_PPML_SOLVER_MODE": "run",
                    "FEREG_PPML_SOLVER_ARM": arm,
                    "FEREG_PPML_SOLVER_TRACE": "0",
                    "FEREG_PPML_SOLVER_REPETITIONS": str(repetitions),
                }
            )
            log = cell_dir / f"{arm}.log"
            process_rc = run_stata(args.stata.resolve(), cell_dir, arm_environment, log)
            rc, terminal = stata_stage_status(
                log,
                process_rc,
                f"FEREG_PPML_SOLVER_RUN_PASS|arm={arm}|repetitions={repetitions}",
            )
            if arm in REFERENCE_ARMS and rc == 0 and (
                "FEREG_PPML_SOLVER_REFERENCE_PASS|ppmlhdfe=pinned|lftools=private"
                not in process_log_text(log)
            ):
                rc = 999
                terminal = "missing-reference-sentinel"
            expected_acceleration = (
                "lsmr" if arm == "ppmlhdfe_lsmr" else "conjugate_gradient"
            )
            if arm in REFERENCE_ARMS and rc == 0 and (
                f"FEREG_PPML_SOLVER_REFERENCE_ROUTE_PASS|arm={arm}|"
                f"acceleration={expected_acceleration}" not in process_log_text(log)
            ):
                rc = 999
                terminal = "missing-reference-route-sentinel"
            route_signature = (
                "## Computing LSMR"
                if arm == "ppmlhdfe_lsmr"
                else "Running solver (acceleration=conjugate_gradient"
            )
            if arm in REFERENCE_ARMS and rc == 0 and (
                route_signature not in process_log_text(log)
            ):
                rc = 999
                terminal = "missing-reference-route-signature"
            process_records.append(
                {
                    "cell_id": identity,
                    "stage": arm,
                    "rc": rc,
                    "process_rc": process_rc,
                    "terminal": terminal,
                }
            )
            parsed = []
            if result.is_file():
                parsed = parse_table(result, cell)
                records.extend(parsed)
            if rc != 0 and not any(record["arm"] == arm for record in parsed):
                records.append({**cell, "cell_id": identity, "arm": arm, "rc": rc})
            if arm not in REFERENCE_ARMS:
                trace_environment = dict(arm_environment)
                trace_environment["FEREG_PPML_SOLVER_TRACE"] = "1"
                trace_environment["FEREG_PPML_SOLVER_REPETITIONS"] = "1"
                trace_log = cell_dir / f"{arm}.trace.log"
                trace_process_rc = run_stata(
                    args.stata.resolve(), cell_dir, trace_environment, trace_log
                )
                events = parse_trace_logs(trace_log)
                trace_rc, trace_terminal = stata_stage_status(
                    trace_log,
                    trace_process_rc,
                    f"FEREG_PPML_SOLVER_TRACE_PASS|arm={arm}",
                )
                if trace_rc == 0 and not events:
                    trace_rc = 999
                    trace_terminal = "missing-trace-events"
                trace_events_total = len(events)
                if trace_rc != 0 and trace_events_total > 24:
                    events = events[:12] + events[-12:]
                process_records.append(
                    {
                        "cell_id": identity,
                        "stage": f"{arm}-trace",
                        "rc": trace_rc,
                        "process_rc": trace_process_rc,
                        "terminal": trace_terminal,
                        "trace_events": events,
                        "trace_events_total": trace_events_total,
                    }
                )
    generated_mlib = args.reference_root.resolve() / "ppmlhdfe/src/l/lftools.mlib"
    if any(arm in REFERENCE_ARMS for arm in args.arms):
        if not generated_mlib.is_file():
            raise DevelopmentError("pinned comparator did not create its private lftools.mlib")
        reference["ftools"]["generated_mlib"] = {
            "size_bytes": generated_mlib.stat().st_size,
            "sha256": sha256_file(generated_mlib),
        }
    summary = {
        "schema_version": 1,
        "result_type": "feppml-solver-development-run-v1",
        "qualification_eligible": False,
        "manifest": {"path": str(MANIFEST.relative_to(ROOT)), "sha256": sha256_file(MANIFEST)},
        "source": source_identity(),
        "candidate_artifact": artifact,
        "references": reference,
        "host": {"platform": platform.platform(), "python": sys.version},
        "profile": args.profile,
        "arms": list(args.arms),
        "repetitions": repetitions,
        "records": records,
        "processes": process_records,
    }
    exclusive_json(output / "run.json", summary)
    print(output)
    return output


def close(left: float, right: float, *, rtol: float, atol: float) -> bool:
    return abs(left - right) <= atol + rtol * max(abs(left), abs(right))


def geometric_mean(values: Iterable[float]) -> float | None:
    materialized = list(values)
    if not materialized or any(value <= 0 or not math.isfinite(value) for value in materialized):
        return None
    return math.exp(sum(math.log(value) for value in materialized) / len(materialized))


def analyze_document(document: dict[str, object]) -> dict[str, object]:
    manifest = load_manifest()
    science = manifest["science"]
    gate = manifest["projection_potential_gate"]
    assert isinstance(science, dict) and isinstance(gate, dict)
    records = document["records"]
    processes = document["processes"]
    assert isinstance(records, list) and isinstance(processes, list)
    grouped: dict[tuple[str, str], list[dict[str, object]]] = {}
    failed: dict[tuple[str, str], list[dict[str, object]]] = {}
    for record in records:
        target = grouped if record.get("rc") == 0 else failed
        target.setdefault((str(record["cell_id"]), str(record["arm"])), []).append(record)
    measured_arms = set(
        str(arm)
        for arm in document.get(
            "arms",
            sorted({str(record["arm"]) for record in records if record.get("arm") in ARMS}),
        )
    )
    common_failures: list[str] = []
    dof_convention_notes: set[str] = set()
    candidate_failures: dict[str, list[str]] = {arm: [] for arm in CANDIDATE_ARMS}
    timing: dict[str, dict[str, float]] = {}
    cell_ids = sorted({str(record["cell_id"]) for record in records})
    science_cells = matrix_cells(manifest, "science_matrix")
    expected_science_ids = {cell_id(cell) for cell in science_cells}
    science_inventory_complete = (
        document.get("profile") == "science_matrix"
        and set(cell_ids) == expected_science_ids
    )
    if not science_inventory_complete:
        common_failures.append(
            f"science_matrix:incomplete:{len(set(cell_ids) & expected_science_ids)}"
            f"/{len(expected_science_ids)}"
        )
    trace_by_cell_arm: dict[tuple[str, str], list[dict[str, str]]] = {}
    for process in processes:
        stage = str(process.get("stage", ""))
        if stage.endswith("-trace") and process.get("rc") == 0:
            trace_by_cell_arm[(str(process["cell_id"]), stage.removesuffix("-trace"))] = list(
                process.get("trace_events", [])
            )

    def failure_label(identity: str, arm: str) -> str:
        rows = failed.get((identity, arm), [])
        return f"rc{rows[-1]['rc']}" if rows else "missing"

    def compare_to_ppmlhdfe(
        identity: str,
        arm: str,
        candidate: dict[str, object],
        reference: dict[str, object],
        controls: int,
        failures: list[str],
    ) -> None:
        if candidate["sample_n"] != reference["sample_n"]:
            failures.append(f"{identity}:{arm}:sample")
        if candidate["b_names"][:controls] != reference["b_names"][:controls]:
            failures.append(f"{identity}:{arm}:control_names")
        elif any(
            not close(
                float(left),
                float(right),
                rtol=float(science["coefficient_rtol"]),
                atol=float(science["coefficient_atol"]),
            )
            for left, right in zip(
                candidate["b_values"][:controls], reference["b_values"][:controls]
            )
        ):
            failures.append(f"{identity}:{arm}:control_coefficients")
        if float(candidate["converged"]) != 1.0:
            failures.append(f"{identity}:{arm}:convergence")
        candidate_model_df = float(candidate["rank"]) + float(candidate["df_a"])
        reference_model_df = float(reference["rank"]) + float(reference["df_a"])
        if candidate_model_df != reference_model_df:
            failures.append(f"{identity}:{arm}:model_df")
        candidate_residual_df = candidate["df_resid"]
        expected_candidate_residual_df = float(candidate["sample_n"]) - candidate_model_df
        if candidate_residual_df is None or float(candidate_residual_df) != expected_candidate_residual_df:
            failures.append(f"{identity}:{arm}:df_resid")
        reference_residual_df = reference["df_resid"]
        expected_reference_residual_df = float(reference["sample_n"]) - reference_model_df
        if reference_residual_df is not None and float(reference_residual_df) != expected_reference_residual_df:
            failures.append(f"{identity}:{arm}:reference_df_resid")
        if (
            float(candidate["rank"]) != float(reference["rank"])
            or float(candidate["df_a"]) != float(reference["df_a"])
        ):
            dof_convention_notes.add(
                "fereg and ppmlhdfe partition rank versus absorbed DoF differently; "
                "the gate compares their equal total model DoF"
            )
        if reference_residual_df is None:
            dof_convention_notes.add(
                "ppmlhdfe does not publish residual DoF for robust VCE; the gate "
                "derives it from N minus total model DoF"
            )
        candidate_dimension = len(candidate["v_names"])
        reference_dimension = len(reference["v_names"])
        if (
            candidate["v_names"][:controls] != reference["v_names"][:controls]
            or len(candidate["v_values"]) != candidate_dimension * candidate_dimension
            or len(reference["v_values"]) != reference_dimension * reference_dimension
        ):
            failures.append(f"{identity}:{arm}:control_vce_shape")
        elif any(
            not close(
                float(candidate["v_values"][left * candidate_dimension + right]),
                float(reference["v_values"][left * reference_dimension + right]),
                rtol=float(science["vce_rtol"]),
                atol=float(science["vce_atol"]),
            )
            for left in range(controls)
            for right in range(controls)
        ):
            failures.append(f"{identity}:{arm}:control_vce")
        for field in ("ll", "deviance"):
            if not close(
                float(candidate[field]),
                float(reference[field]),
                rtol=float(science["likelihood_rtol"]),
                atol=float(science["likelihood_atol"]),
            ):
                failures.append(f"{identity}:{arm}:{field}")

    def validate_trace(
        identity: str, arm: str, events: list[dict[str, str]], failures: list[str]
    ) -> None:
        solve_events = [event for event in events if event.get("event") == "solve"]
        if not events or events[0].get("event") != "fit_begin" or not solve_events:
            failures.append(f"{identity}:{arm}:trace_missing")
            return
        required = {"fresh", "strict", "final_hessian", "fit_end"}
        if arm == "cmg_fused":
            required.add("fused_fwl_cmg")
        elif arm.startswith("lsmr_joint_"):
            required.add("joint_incremental")
        elif arm.startswith("lsmr_fused_"):
            required.add("fused_fwl")
        else:
            required.add("frozen")
        observed = {event.get("kind") for event in solve_events}
        if not required.issubset(observed):
            failures.append(f"{identity}:{arm}:trace_kinds")
        failed_cmg_events = [
            event
            for event in solve_events
            if arm == "cmg_fused"
            and event.get("kind") == "fused_fwl_cmg"
            and event.get("fallback") == "1"
            and event.get("certified") == "0"
            and event.get("used_solver") == "map"
        ]
        experimental_kinds = {"fused_fwl_cmg", "fused_fwl", "joint_incremental"}
        mixed_checkpoint_route = arm == "cmg_fused" or arm.startswith(
            ("lsmr_fused_", "lsmr_joint_")
        )
        if any(
            event.get("certified") != "1" and event not in failed_cmg_events
            for event in solve_events
        ):
            failures.append(f"{identity}:{arm}:trace_certificate")
        if mixed_checkpoint_route:
            if any(
                event not in failed_cmg_events
                and event.get("used_solver")
                != (
                    "cmg"
                    if event.get("kind") == "fused_fwl_cmg"
                    else "lsmr"
                    if event.get("kind") in experimental_kinds
                    else "map"
                )
                for event in solve_events
            ):
                failures.append(f"{identity}:{arm}:trace_solver_boundary")
            if failed_cmg_events and not any(
                event.get("kind") == "fresh_route_fallback"
                and event.get("used_solver") == "map"
                and event.get("certified") == "1"
                for event in solve_events
            ):
                failures.append(f"{identity}:{arm}:trace_fallback_continuation")
        else:
            expected_solver = "lsmr" if arm.startswith("lsmr_") else arm
            if any(event.get("used_solver") != expected_solver for event in solve_events):
                failures.append(f"{identity}:{arm}:trace_solver_boundary")
        experimental_events = [
            event
            for event in solve_events
            if event.get("kind") in experimental_kinds
        ]
        if arm == "lsmr" and any(
            event.get("used_preconditioner") != "schwarz" for event in solve_events
        ):
            failures.append(f"{identity}:{arm}:trace_preconditioner")
        elif arm.endswith("_diagonal") and any(
            event.get("used_preconditioner") != "diagonal" for event in solve_events
        ):
            failures.append(f"{identity}:{arm}:trace_preconditioner")
        elif (arm.endswith("_reuse") or arm.endswith("_rebuild")) and any(
            event.get("requested_preconditioner") != "schwarz"
            for event in experimental_events
        ):
            failures.append(f"{identity}:{arm}:trace_preconditioner_request")

    for identity in cell_ids:
        map_rows = grouped.get((identity, "map"), [])
        comparator_rows = grouped.get((identity, "ppmlhdfe"), [])
        if not map_rows or not comparator_rows:
            common_failures.append(f"{identity}:missing_oracle")
            continue
        map_row = map_rows[-1]
        controls = int(map_row["controls"])
        reference = comparator_rows[-1]
        for arm in measured_arms:
            rows = grouped.get((identity, arm), [])
            if not rows:
                continue
            timing.setdefault(identity, {})[arm] = statistics.median(
                float(row["seconds"]) for row in rows
            )
        compare_to_ppmlhdfe(identity, "map", map_row, reference, controls, common_failures)
        validate_trace(
            identity,
            "map",
            trace_by_cell_arm.get((identity, "map"), []),
            common_failures,
        )
        for arm in CANDIDATE_ARMS:
            if arm not in measured_arms:
                continue
            if not arm_applicable(arm, map_row):
                continue
            rows = grouped.get((identity, arm), [])
            if not rows:
                candidate_failures[arm].append(
                    f"{identity}:{arm}:{failure_label(identity, arm)}"
                )
                continue
            candidate = rows[-1]
            compare_to_ppmlhdfe(
                identity, arm, candidate, reference, controls, candidate_failures[arm]
            )
            if candidate["b_names"] != map_row["b_names"]:
                candidate_failures[arm].append(
                    f"{identity}:{arm}:native_coefficient_names"
                )
            elif any(
                not close(
                    float(left),
                    float(right),
                    rtol=float(science["coefficient_rtol"]),
                    atol=float(science["coefficient_atol"]),
                )
                for left, right in zip(candidate["b_values"], map_row["b_values"])
            ):
                candidate_failures[arm].append(f"{identity}:{arm}:native_coefficients")
            if candidate["v_names"] != map_row["v_names"]:
                candidate_failures[arm].append(f"{identity}:{arm}:native_vce_names")
            elif len(candidate["v_values"]) != len(map_row["v_values"]) or any(
                not close(
                    float(left),
                    float(right),
                    rtol=float(science["vce_rtol"]),
                    atol=float(science["vce_atol"]),
                )
                for left, right in zip(candidate["v_values"], map_row["v_values"])
            ):
                candidate_failures[arm].append(f"{identity}:{arm}:native_vce")
            for field in ("rank", "df_a", "df_resid"):
                if candidate[field] != map_row[field]:
                    candidate_failures[arm].append(
                        f"{identity}:{arm}:native_{field}"
                    )
            validate_trace(
                identity,
                arm,
                trace_by_cell_arm.get((identity, arm), []),
                candidate_failures[arm],
            )
        if str(map_row.get("profile")) == "science_matrix":
            oracle_rows = grouped.get((identity, "independent_explicit_dummy"), [])
            if not oracle_rows:
                common_failures.append(f"{identity}:independent_oracle_missing")
            else:
                oracle = oracle_rows[-1]
                if map_row["b_names"][:controls] != oracle["control_names"]:
                    common_failures.append(f"{identity}:independent_oracle_names")
                elif any(
                    not close(
                        float(left),
                        float(right),
                        rtol=float(science["coefficient_rtol"]),
                        atol=float(science["coefficient_atol"]),
                    )
                    for left, right in zip(
                        map_row["b_values"][:controls], oracle["control_values"]
                    )
                ):
                    common_failures.append(f"{identity}:independent_oracle_coefficients")
                if not close(
                    float(map_row["deviance"]),
                    float(oracle["deviance"]),
                    rtol=float(science["likelihood_rtol"]),
                    atol=float(science["likelihood_atol"]),
                ):
                    common_failures.append(f"{identity}:independent_oracle_deviance")
    candidates: dict[str, object] = {}
    for arm in CANDIDATE_ARMS:
        expected_arm_ids = {
            cell_id(cell) for cell in science_cells if arm_applicable(arm, cell)
        }
        ratios: list[float] = []
        faster = 0
        individual_ok = True
        for identity in cell_ids:
            map_rows = grouped.get((identity, "map"), [])
            if map_rows and not arm_applicable(arm, map_rows[-1]):
                continue
            candidate_events = trace_by_cell_arm.get((identity, arm), [])
            map_events = trace_by_cell_arm.get((identity, "map"), [])
            candidate_solve = sum(
                float(event.get("solve_seconds", 0.0))
                + float(event.get("finite_seconds", 0.0))
                for event in candidate_events
                if event.get("event") == "solve"
                and event.get("kind") not in ("fresh", "fit_end")
            )
            map_solve = sum(
                float(event.get("solve_seconds", 0.0))
                + float(event.get("finite_seconds", 0.0))
                for event in map_events
                if event.get("event") == "solve"
                and event.get("kind") not in ("fresh", "fit_end")
            )
            if candidate_solve > 0 and map_solve > 0:
                ratio = candidate_solve / map_solve
                ratios.append(ratio)
                faster += int(ratio < 1.0)
                individual_ok &= ratio <= float(gate["individual_cell_ratio_max"])
        projection_ratio = geometric_mean(ratios)
        faster_share = faster / len(ratios) if ratios else 0.0
        end_to_end_ratios = [
            values[arm] / values["map"]
            for values in timing.values()
            if arm in values and "map" in values and values["map"] > 0
        ]
        end_to_end_ratio = geometric_mean(end_to_end_ratios)
        complete = science_inventory_complete and len(ratios) == len(expected_arm_ids)
        passes = (
            arm in measured_arms
            and not common_failures
            and not candidate_failures[arm]
            and complete
            and projection_ratio is not None
            and projection_ratio <= float(gate["post_setup_projection_geometric_mean_ratio_max"])
            and faster_share >= float(gate["faster_cell_share_min"])
            and individual_ok
        )
        label = (
            "explicit-route"
            if passes
            and end_to_end_ratio is not None
            and end_to_end_ratio <= float(gate["end_to_end_explicit_route_ratio_max"])
            else "reuse-engineering" if passes else "reject"
        )
        candidates[arm] = {
            "projection_geometric_mean_ratio": projection_ratio,
            "faster_cell_share": faster_share,
            "individual_cell_cap_passed": individual_ok,
            "end_to_end_geometric_mean_ratio": end_to_end_ratio,
            "projection_potential_passed": passes,
            "label": label,
            "measured_cells": len(ratios),
            "expected_cells": len(expected_arm_ids),
            "science_failures": sorted(set(candidate_failures[arm])),
        }
    science_failures = list(common_failures)
    for arm in CANDIDATE_ARMS:
        if arm in measured_arms:
            science_failures.extend(candidate_failures[arm])
    return {
        "schema_version": 1,
        "result_type": "feppml-solver-development-analysis-v1",
        "qualification_eligible": False,
        "science_failures": sorted(set(science_failures)),
        "dof_convention_notes": sorted(dof_convention_notes),
        "candidates": candidates,
        "route_recommendation": {
            "default": "map",
            "regions": [],
            "note": "Regions remain empty until a candidate passes science and the complete projection-potential gate, including holdout and SCC direction checks.",
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    cells_parser = subparsers.add_parser("cells")
    cells_parser.add_argument("--profile", choices=("science_matrix", "screening", "scale_thread", "all"), default="all")
    run_parser = subparsers.add_parser("run-local")
    run_parser.add_argument("--profile", choices=("science_matrix", "screening", "scale_thread", "all"), default="all")
    run_parser.add_argument("--stata", type=Path, required=True)
    run_parser.add_argument("--reference-root", type=Path, required=True)
    run_parser.add_argument(
        "--output-dir", type=Path, default=ROOT / "benchmarks/reports/development/feppml-solvers"
    )
    run_parser.add_argument("--repetitions", type=int, default=3)
    run_parser.add_argument("--limit", type=int)
    run_parser.add_argument("--n-override", type=int)
    run_parser.add_argument("--topology")
    run_parser.add_argument("--controls", type=int)
    run_parser.add_argument("--difficulty", choices=("easy", "long"))
    run_parser.add_argument("--threads", type=int)
    run_parser.add_argument("--arms", nargs="+", choices=ARMS, default=list(ARMS))
    analyze_parser = subparsers.add_parser("analyze")
    analyze_parser.add_argument("run", type=Path)
    analyze_parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "cells":
        print(json.dumps(matrix_cells(load_manifest(), args.profile), indent=2, sort_keys=True))
        return 0
    if args.command == "run-local":
        if args.repetitions <= 0:
            raise DevelopmentError("repetitions must be positive")
        run_profile(args)
        return 0
    if args.command == "analyze":
        document = json.loads(args.run.read_text(encoding="utf-8"))
        exclusive_json(args.output.resolve(), analyze_document(document))
        print(args.output.resolve())
        return 0
    raise AssertionError(args.command)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except DevelopmentError as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(2)
