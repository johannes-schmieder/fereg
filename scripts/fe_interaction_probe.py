#!/usr/bin/env python3
"""Run the pinned reghdfe fixed-effect interaction probe on licensed Stata.

The probe reuses the OLS reference harness's clean-checkout preflight,
archive-only staging, isolated sysdirs, and resolver checks. Raw Stata logs and
the machine capture are temporary CI artifacts; only a redacted sealed summary
is suitable for committing to the repository.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from ols_reference import (
    STATA_HEX,
    _run_fresh_process,
    _write_wrapper,
    load_manifest,
    load_reference_lock,
    preflight_reference_root,
    seal_payload,
    sha256_file,
    stage_references,
)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_STATA = Path("/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp")
MANIFEST = ROOT / "benchmarks/manifests/ols_v1.json"
LOCK = ROOT / "references.lock.toml"
BASELINE = ROOT / "benchmarks/stata/baseline_reghdfe.do"
PROBE = ROOT / "benchmarks/stata/probe_reghdfe_fe_interaction.do"
TARGET = ROOT / "tests/fixtures/ols/ols_core.csv"
PREFIX = "STFIXEST_FE_INTERACTION_PROBE"
PROBE_SENTINEL = "END_FIXTURE|reghdfe_fe_interaction_probe|rc=0"
BASELINE_SENTINEL = "END_FIXTURE|baseline_reghdfe|rc=0"
HEX_COMMIT = re.compile(r"^[0-9a-f]{40}$")
ABSOLUTE_PATH = re.compile(r"(?:^|\s)/(?:Users|home|private|tmp|var|Volumes)/\S*|[A-Za-z]:[\\/]\S*")
EXPECTED_CASES = (
    "target_default_firm_year",
    "target_keepsingletons",
    "tuple_second_fe_cascade",
    "fweight_frequency_cascade",
    "missing_interaction_components",
    "repeated_identical_interaction",
    "interaction_order_year_firm",
    "dof_none",
    "dof_firstpair",
    "nested_simple_cluster",
    "identical_interaction_cluster",
    "unrelated_interaction_cluster",
    "final_sample_default_factor_rebase",
    "final_sample_default_interaction_rebase",
)


class ProbeError(RuntimeError):
    """The source-bound reference probe failed closed."""


def sanitize_message(message: str) -> str:
    redacted = ABSOLUTE_PATH.sub(" <redacted-path>", message.replace("\n", " "))
    return redacted[:1000]


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def _new_case(case_id: str) -> dict[str, Any]:
    return {
        "id": case_id,
        "return_code": None,
        "scalars": {},
        "macros": {},
        "matrix_dimensions": {},
        "matrices": [],
        "observation_count": None,
        "observations": [],
        "absent": [],
    }


def _canonical_hex(value: str, where: str) -> str:
    if not STATA_HEX.fullmatch(value):
        raise ProbeError(f"{where} is not canonical Stata hexadecimal data")
    return value


def parse_probe_capture(text: str, expected_manifest_sha256: str) -> dict[str, Any]:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    sentinels = [line for line in lines if line.startswith("END_FIXTURE|")]
    if sentinels != [PROBE_SENTINEL] or not lines or lines[-1] != PROBE_SENTINEL:
        raise ProbeError("probe capture lacks its unique terminal success sentinel")

    records = [line.split("|") for line in lines[:-1]]
    if len(records) < 2 or records[0] != [PREFIX, "schema", "1"]:
        raise ProbeError("probe capture schema header is absent or malformed")
    if records[1] != [PREFIX, "manifest_sha256", expected_manifest_sha256]:
        raise ProbeError("probe capture is bound to a different manifest")

    environment: dict[str, str] = {}
    cases: dict[str, dict[str, Any]] = {}
    order: list[str] = []
    active: str | None = None
    for parts in records[2:]:
        if len(parts) < 3 or parts[0] != PREFIX:
            raise ProbeError("probe capture contains an unknown record prefix")
        kind = parts[1]
        if kind == "environment" and len(parts) == 4 and active is None:
            if parts[2] in environment or not parts[3]:
                raise ProbeError("probe environment field is duplicated or empty")
            environment[parts[2]] = parts[3]
            continue
        if kind == "case_begin" and len(parts) == 3 and active is None:
            case_id = parts[2]
            if case_id in cases:
                raise ProbeError("probe case is duplicated")
            active = case_id
            order.append(case_id)
            cases[case_id] = _new_case(case_id)
            continue
        if active is None or parts[2] != active:
            raise ProbeError("probe record appears outside its active case")
        case = cases[active]
        if kind == "return_code" and len(parts) == 4:
            if case["return_code"] is not None:
                raise ProbeError("probe return code is duplicated")
            try:
                case["return_code"] = int(parts[3])
            except ValueError as exc:
                raise ProbeError("probe return code is not an integer") from exc
        elif kind in {"scalar", "macro"} and len(parts) == 5:
            container = case[kind + "s"]
            if parts[3] in container:
                raise ProbeError(f"probe {kind} is duplicated")
            container[parts[3]] = (
                _canonical_hex(parts[4], f"probe scalar {parts[3]}")
                if kind == "scalar"
                else parts[4]
            )
        elif kind == "absent" and len(parts) == 5:
            case["absent"].append({"kind": parts[3], "name": parts[4]})
        elif kind == "matrix_dimensions" and len(parts) == 6:
            name = parts[3]
            if name in case["matrix_dimensions"]:
                raise ProbeError("probe matrix dimensions are duplicated")
            try:
                rows, columns = int(parts[4]), int(parts[5])
            except ValueError as exc:
                raise ProbeError("probe matrix dimensions are not integers") from exc
            if rows < 0 or columns < 0:
                raise ProbeError("probe matrix dimensions are negative")
            case["matrix_dimensions"][name] = {"rows": rows, "columns": columns}
        elif kind == "matrix" and len(parts) == 9:
            try:
                row, column = int(parts[4]), int(parts[5])
            except ValueError as exc:
                raise ProbeError("probe matrix coordinates are not integers") from exc
            case["matrices"].append(
                {
                    "name": parts[3],
                    "row": row,
                    "column": column,
                    "row_name": parts[6],
                    "column_name": parts[7],
                    "value_hex": _canonical_hex(parts[8], "probe matrix value"),
                }
            )
        elif kind == "observation_count" and len(parts) == 4:
            if case["observation_count"] is not None:
                raise ProbeError("probe observation count is duplicated")
            try:
                case["observation_count"] = int(parts[3])
            except ValueError as exc:
                raise ProbeError("probe observation count is not an integer") from exc
        elif kind == "observation" and len(parts) == 6:
            case["observations"].append(
                {
                    "obs_id_hex": _canonical_hex(parts[3], "probe observation id"),
                    "field": parts[4],
                    "value_hex": _canonical_hex(parts[5], "probe observation value"),
                }
            )
        elif kind == "case_end" and len(parts) == 3:
            if case["return_code"] is None:
                raise ProbeError("probe case ended without a return code")
            active = None
        else:
            raise ProbeError(f"probe record kind {kind!r} is malformed or unknown")

    if active is not None or tuple(order) != EXPECTED_CASES:
        raise ProbeError("probe case inventory is incomplete or out of order")
    expected_environment = {
        "stata_version",
        "stata_flavor",
        "stata_processors",
        "stata_processors_limit",
        "os",
        "machine_type",
    }
    if set(environment) != expected_environment:
        raise ProbeError("probe Stata environment inventory is incomplete")

    for case_id in order:
        case = cases[case_id]
        if case["return_code"] != 0:
            raise ProbeError(f"reference probe case {case_id} returned nonzero")
        seen: set[tuple[str, int, int]] = set()
        by_name: dict[str, int] = {}
        for item in case["matrices"]:
            coordinate = (item["name"], item["row"], item["column"])
            if coordinate in seen:
                raise ProbeError("probe matrix coordinate is duplicated")
            seen.add(coordinate)
            by_name[item["name"]] = by_name.get(item["name"], 0) + 1
        for name, dimensions in case["matrix_dimensions"].items():
            expected = dimensions["rows"] * dimensions["columns"]
            if by_name.get(name, 0) != expected:
                raise ProbeError(f"probe matrix {name} does not fill its declared dimensions")
        observation_count = case["observation_count"]
        if observation_count is not None and observation_count < 0:
            raise ProbeError("probe observation count is negative")

    return {"environment": environment, "cases": [cases[name] for name in order]}


def _derived_case_summary(case: dict[str, Any]) -> dict[str, Any]:
    b_entries = sorted(
        (item for item in case["matrices"] if item["name"] == "b" and item["row"] == 1),
        key=lambda item: item["column"],
    )
    return {
        "id": case["id"],
        "return_code": case["return_code"],
        "coefficient_names": [item["column_name"] for item in b_entries],
        "coefficient_values_hex": [item["value_hex"] for item in b_entries],
        "matrix_dimensions": case["matrix_dimensions"],
        "absent": case["absent"],
    }


def execute_probe(
    *,
    stata: Path,
    reference_root: Path,
    output_dir: Path,
    source_sha: str,
) -> dict[str, Any]:
    manifest = load_manifest(MANIFEST, ROOT)
    lock = load_reference_lock(LOCK)
    manifest_sha256 = sha256_file(MANIFEST)
    input_hashes_before = {
        "manifest": manifest_sha256,
        "baseline": sha256_file(BASELINE),
        "probe": sha256_file(PROBE),
        "target": sha256_file(TARGET),
    }
    identities = preflight_reference_root(reference_root, lock, ROOT)

    with tempfile.TemporaryDirectory(prefix="stfixest-fe-interaction-probe-") as temporary:
        run_dir = Path(temporary)
        reference_stage = run_dir / "references"
        reference_stage.mkdir()
        staged_references = stage_references(identities, lock, reference_stage)
        shutil.copyfile(TARGET, run_dir / "target.csv")
        capture_file = run_dir / "capture.machine"
        wrapper = run_dir / "run.do"
        _write_wrapper(
            wrapper,
            BASELINE,
            PROBE,
            capture_file,
            staged_references,
            manifest_sha256,
            run_dir / "sysdirs",
            "preflight",
        )
        completed, rss = _run_fresh_process(
            [str(stata), "-q", "-b", "do", str(wrapper)],
            run_dir,
        )
        log_file = run_dir / "run.log"
        log_text = (
            log_file.read_text(encoding="utf-8", errors="replace")
            if log_file.is_file()
            else ""
        )
        log_sentinels = [
            line.strip()
            for line in log_text.splitlines()
            if line.strip().startswith("END_FIXTURE|")
        ]
        if log_file.is_file():
            shutil.copy2(log_file, output_dir / "run.log")
        if capture_file.is_file():
            shutil.copy2(capture_file, output_dir / "capture.machine")
        if completed.returncode != 0 or log_sentinels != [BASELINE_SENTINEL]:
            raise ProbeError(
                "licensed reference probe failed: "
                f"process_rc={completed.returncode}; "
                f"baseline_sentinel_count={len(log_sentinels)}"
            )
        if not capture_file.is_file():
            raise ProbeError("licensed reference probe wrote no machine capture")
        capture_text = capture_file.read_text(encoding="utf-8", errors="strict")
        parsed = parse_probe_capture(capture_text, manifest_sha256)

    identities_after = preflight_reference_root(reference_root, lock, ROOT)
    if {
        name: identity.receipt_value() for name, identity in identities.items()
    } != {
        name: identity.receipt_value() for name, identity in identities_after.items()
    }:
        raise ProbeError("reference checkout identity changed during the probe")
    input_hashes_after = {
        "manifest": sha256_file(MANIFEST),
        "baseline": sha256_file(BASELINE),
        "probe": sha256_file(PROBE),
        "target": sha256_file(TARGET),
    }
    if input_hashes_after != input_hashes_before:
        raise ProbeError("a source-bound probe input changed during execution")

    capture_sha256 = sha256_file(output_dir / "capture.machine")
    payload = {
        "schema_version": 1,
        "receipt_type": "stfixest-reghdfe-fe-interaction-probe",
        "status": "success",
        "source_sha": source_sha,
        "inputs": input_hashes_before,
        "capture_sha256": capture_sha256,
        "raw_capture_committed": False,
        "references": {
            name: identity.receipt_value()
            for name, identity in sorted(identities.items())
        },
        "environment": parsed["environment"],
        "host": {
            "system": platform.system(),
            "machine": platform.machine(),
            "cpu_count": os.cpu_count(),
        },
        "peak_rss": {**rss, "scope": "licensed_reference_probe"},
        "case_summary": [_derived_case_summary(case) for case in parsed["cases"]],
        "cases": parsed["cases"],
    }
    return seal_payload(payload)


def failure_document(source_sha: str, exc: BaseException) -> dict[str, Any]:
    payload = {
        "schema_version": 1,
        "receipt_type": "stfixest-reghdfe-fe-interaction-probe",
        "status": "failure",
        "source_sha": source_sha,
        "stage": "probe_execution",
        "failure_kind": type(exc).__name__,
        "failure_detail": sanitize_message(str(exc)),
        "raw_capture_committed": False,
    }
    run_id = os.environ.get("GITHUB_RUN_ID")
    run_attempt = os.environ.get("GITHUB_RUN_ATTEMPT")
    if run_id and run_id.isdigit():
        payload["workflow_run_id"] = int(run_id)
    if run_attempt and run_attempt.isdigit():
        payload["workflow_run_attempt"] = int(run_attempt)
    return seal_payload(payload)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stata", type=Path, default=DEFAULT_STATA)
    parser.add_argument("--reference-root", type=Path)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    args = parser.parse_args()

    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    if not HEX_COMMIT.fullmatch(args.source_sha):
        write_json(
            output_dir / "summary.json",
            failure_document(args.source_sha, ProbeError("source SHA is malformed")),
        )
        return 1
    raw_reference_root = args.reference_root or (
        Path(os.environ["STFIXEST_REFERENCE_ROOT"])
        if os.environ.get("STFIXEST_REFERENCE_ROOT")
        else None
    )
    if raw_reference_root is None:
        write_json(
            output_dir / "summary.json",
            failure_document(
                args.source_sha,
                ProbeError("STFIXEST_REFERENCE_ROOT is not configured on the trusted runner"),
            ),
        )
        print("FE_INTERACTION_PROBE status=failure kind=reference_root_unconfigured")
        return 1

    try:
        document = execute_probe(
            stata=args.stata.resolve(),
            reference_root=raw_reference_root.expanduser().resolve(),
            output_dir=output_dir,
            source_sha=args.source_sha,
        )
    except BaseException as exc:  # Fail closed while still publishing redacted evidence.
        document = failure_document(args.source_sha, exc)
        write_json(output_dir / "summary.json", document)
        print(f"FE_INTERACTION_PROBE status=failure kind={type(exc).__name__}")
        return 1
    write_json(output_dir / "summary.json", document)
    print(f"FE_INTERACTION_PROBE status=success source_sha={args.source_sha}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
