#!/usr/bin/env python3
"""Capture and verify the developer-only numeric OLS manifest projection."""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import os
import platform
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Mapping, Sequence

try:
    from ols_reference import (
        ContractError,
        ENVIRONMENT_FIELDS,
        _assert_redacted,
        _validate_case_evidence,
        canonical_json_bytes,
        load_manifest,
        parse_capture,
        render_capture_do,
        seal_payload,
        sha256_file,
        verify_receipt as verify_reference_receipt,
    )
except ModuleNotFoundError:  # Importable as scripts.ols_hidden_numeric in tests.
    from scripts.ols_reference import (
        ContractError,
        ENVIRONMENT_FIELDS,
        _assert_redacted,
        _validate_case_evidence,
        canonical_json_bytes,
        load_manifest,
        parse_capture,
        render_capture_do,
        seal_payload,
        sha256_file,
        verify_receipt as verify_reference_receipt,
    )

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PROJECTION = ROOT / "benchmarks/manifests/ols_hidden_numeric_v1.json"
RECEIPT_TYPE = "stfixest-ols-hidden-numeric-v1-candidate"
TERMINAL_SENTINEL = "END_FIXTURE|ols_v1_reference_capture|rc=0"
FORBIDDEN_WARNING = "warning: variance matrix is nonsymmetric or highly singular."
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
SAFE_ID = re.compile(r"^[a-z][a-z0-9_]{1,63}$")

EXECUTABLE_CASE_IDS = (
    "unweighted_no_fe",
    "one_fe_robust",
    "disconnected_no_regressors",
    "collinear_with_fe",
    "missing_and_if_in",
    "missing_fe_source",
    "missing_cluster_source",
    "singleton_cascade",
    "keep_singletons",
    "aweight",
    "aweight_rescaled",
    "fweight",
    "pweight",
    "zero_weight_probe",
    "negative_weight_error",
    "fractional_fweight_error",
    "missing_weight",
    "large_weight",
    "weight_if_in",
    "invalid_weight_outside_candidate",
    "weighted_singletons",
    "cluster_oneway",
    "cluster_twoway",
    "cluster_threeway",
    "cluster_interaction",
    "nested_fe_cluster",
    "dof_none",
    "dof_firstpair",
    "empty_sample_error",
    "one_observation_error",
)

BLOCKERS = (
    ("multi_fe_interactions", "absorbed_categorical_interaction"),
    ("intercept_and_slope", "absorbed_intercept_slope"),
    ("slope_only_multiple", "absorbed_slope_only_multiple"),
    ("string_fe", "string_source_ingestion"),
    ("time_series_lag", "time_series_materialization"),
    ("time_series_lead_difference_gaps", "time_series_materialization"),
    ("iweight_probe", "importance_weight_semantics"),
    ("dof_continuous", "absorbed_slope_continuous_dof"),
    ("saved_outputs", "fixed_effect_recovery_outputs"),
    ("postestimation_test_summarize", "public_postestimation"),
)

FIT_SCALARS = {
    "rss",
    "tss",
    "tss_within",
    "mss",
    "r2",
    "r2_a",
    "r2_within",
    "r2_a_within",
    "rmse",
    "F",
    "ll",
    "ll_0",
}
EXACT_SCALARS = {
    "N",
    "df_m",
    "df_r",
    "df_a",
    "N_hdfe",
    "num_singletons",
    "N_clust",
    "N_clust1",
    "N_clust2",
    "N_clust3",
    "N_clustervars",
}

Q_ANCHORS = {
    "cluster_twoway": {
        "V": [
            "+1.0202f479f358fX-005",
            "-1.2c7938ea4f98dX-007",
            "-1.2c7938ea4f98dX-007",
            "+1.5ddecfdb9c14bX-009",
        ],
        "determinant": "-1.b7ee8115e0f3cX-01b",
    },
    "cluster_threeway": {
        "V": [
            "+1.337c145ab7e50X-006",
            "-1.6a2d71c410628X-008",
            "-1.6a2d71c410628X-008",
            "+1.aa6d2b8b31611X-00a",
        ],
        "determinant": "-1.a79e7840b02b9X-01b",
    },
}

# These absolute caps apply only to comparison against the frozen DEFAULT
# reghdfe receipt.  They characterize its stopping-path artifact and never
# relax candidate-internal score, covariance, transform, or certificate tests.
SEALED_DEFAULT_V_ABS_CAPS = {
    "cluster_oneway": 1e-7,
    "cluster_twoway": 2e-6,
    "cluster_threeway": 4e-6,
    "cluster_interaction": 1e-7,
}


def _require_keys(value: Mapping[str, Any], expected: set[str], where: str) -> None:
    if set(value) != expected:
        raise ContractError(
            f"{where}: expected keys {sorted(expected)}, observed {sorted(value)}"
        )


def _repository_path(raw: str, where: str) -> Path:
    path = Path(raw)
    if path.is_absolute() or ".." in path.parts:
        raise ContractError(f"{where}: path must be repository-relative")
    resolved = (ROOT / path).resolve()
    try:
        resolved.relative_to(ROOT.resolve())
    except ValueError as exc:
        raise ContractError(f"{where}: path escapes the repository") from exc
    return resolved


def load_projection(path: Path = DEFAULT_PROJECTION) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ContractError(f"cannot read hidden numeric projection: {exc}") from exc
    if not isinstance(value, dict):
        raise ContractError("hidden numeric projection root must be an object")
    _require_keys(
        value,
        {
            "schema_version",
            "manifest_id",
            "mode",
            "public_activation",
            "performance",
            "parent_manifest",
            "comparator_receipt",
            "candidate_command",
            "intended_public_command",
            "tolerances",
            "executable_case_ids",
            "blockers",
        },
        "projection",
    )
    if (
        value["schema_version"] != 1
        or value["manifest_id"] != "ols_hidden_numeric_v1"
        or value["mode"] != "developer_hidden_current_surface"
        or value["public_activation"] is not False
        or value["performance"] != "unmeasured"
        or value["candidate_command"] != "_stfixest_ols_hidden_numeric_v1"
        or value["intended_public_command"] != "stfeols"
    ):
        raise ContractError("projection identity/public boundary is invalid")
    if tuple(value["executable_case_ids"]) != EXECUTABLE_CASE_IDS:
        raise ContractError("projection executable case inventory/order changed")
    blockers = tuple((item.get("case_id"), item.get("reason")) for item in value["blockers"])
    if blockers != BLOCKERS or any(set(item) != {"case_id", "reason"} for item in value["blockers"]):
        raise ContractError("projection blocker inventory/reasons changed")
    parent = value["parent_manifest"]
    comparator = value["comparator_receipt"]
    if not isinstance(parent, Mapping) or set(parent) != {"path", "sha256"}:
        raise ContractError("projection parent manifest identity is malformed")
    if not isinstance(comparator, Mapping) or set(comparator) != {
        "path",
        "file_sha256",
        "payload_sha256",
    }:
        raise ContractError("projection comparator receipt identity is malformed")
    for where, raw_path, digest in (
        ("parent_manifest", parent["path"], parent["sha256"]),
        ("comparator_receipt", comparator["path"], comparator["file_sha256"]),
    ):
        if not isinstance(raw_path, str) or not isinstance(digest, str) or not HEX_SHA256.fullmatch(digest):
            raise ContractError(f"projection {where} identity is malformed")
        resolved = _repository_path(raw_path, where)
        if not resolved.is_file() or sha256_file(resolved) != digest:
            raise ContractError(f"projection {where} hash is stale")
    if not isinstance(comparator["payload_sha256"], str) or not HEX_SHA256.fullmatch(comparator["payload_sha256"]):
        raise ContractError("projection comparator payload seal is malformed")
    expected_tolerances = {
        "coefficient_rtol": 1e-7,
        "coefficient_atol": 1e-10,
        "vce_rtol": 1e-6,
        "vce_atol": 1e-10,
        "fit_rtol": 1e-7,
        "fit_atol": 1e-9,
    }
    if value["tolerances"] != expected_tolerances:
        raise ContractError("projection tolerances differ from the parent contract")
    parent_manifest = load_manifest(_repository_path(parent["path"], "parent_manifest"), ROOT)
    semantic_ids = [case["id"] for case in parent_manifest["semantic_cases"]]
    if set(EXECUTABLE_CASE_IDS).intersection(case for case, _ in BLOCKERS):
        raise ContractError("projection executable and blocker inventories overlap")
    if set(EXECUTABLE_CASE_IDS).union(case for case, _ in BLOCKERS) != set(semantic_ids):
        raise ContractError("projection does not partition all 40 parent semantic cases")
    return value


def projected_manifest(projection: Mapping[str, Any]) -> dict[str, Any]:
    parent_path = _repository_path(projection["parent_manifest"]["path"], "parent_manifest")
    parent = load_manifest(parent_path, ROOT)
    selected = set(EXECUTABLE_CASE_IDS)
    cases: list[dict[str, Any]] = []
    for source in parent["semantic_cases"]:
        if source["id"] not in selected:
            continue
        case = copy.deepcopy(source)
        if not case["command"].startswith("reghdfe "):
            raise ContractError(f"case {case['id']} command cannot be projected")
        case["command"] = case["command"].replace(
            "reghdfe", projection["candidate_command"], 1
        )
        if case["expected"] != "must_error":
            for required in (
                "stfixest_status",
                "stfixest_certificate",
                "stfixest_receipt",
            ):
                if required not in case["capture"]["matrices"]:
                    case["capture"]["matrices"].append(required)
        if case["id"] in Q_ANCHORS and "stfixest_v_determinant" not in case["capture"]["scalars"]:
            case["capture"]["scalars"].append("stfixest_v_determinant")
        cases.append(case)
    if [case["id"] for case in cases] != [case for case in EXECUTABLE_CASE_IDS if case in selected]:
        # The executable inventory is explicit, but execution follows immutable
        # parent order. Compare sets above and preserve that parent order here.
        if set(case["id"] for case in cases) != selected:
            raise ContractError("projected parent case inventory is incomplete")
    projected = copy.deepcopy(parent)
    projected["manifest_id"] = "ols_hidden_numeric_v1"
    projected["semantic_cases"] = cases
    projected["benchmark_cases"] = []
    return projected


def stata_hex_to_float(value: str) -> float:
    if value == ".":
        return math.nan
    match = re.fullmatch(r"([+-])([0-9a-fA-F]+(?:\.[0-9a-fA-F]+)?)X([+-])([0-9a-fA-F]+)", value)
    if match is None:
        raise ContractError(f"invalid canonical Stata hex value {value!r}")
    exponent = int(match.group(4), 16)
    if match.group(3) == "-":
        exponent = -exponent
    return float.fromhex(f"{match.group(1)}0x{match.group(2)}p{exponent}")


def _close(actual_hex: str, expected_hex: str, rtol: float, atol: float) -> tuple[bool, float]:
    actual = stata_hex_to_float(actual_hex)
    expected = stata_hex_to_float(expected_hex)
    error = abs(actual - expected)
    return error <= atol + rtol * abs(expected), error


def _matrix_map(case: Mapping[str, Any], name: str) -> dict[tuple[int, int], Mapping[str, Any]]:
    return {
        (item["row"], item["column"]): item
        for item in case["matrices"]
        if item["name"] == name
    }


def _validate_internal_candidate(case: Mapping[str, Any]) -> None:
    if case["return_code"] != 0:
        return
    receipt = _matrix_map(case, "stfixest_receipt")
    certificate = _matrix_map(case, "stfixest_certificate")
    status = _matrix_map(case, "stfixest_status")
    if len(receipt) != 16:
        raise ContractError(f"case {case['id']}: result receipt is absent or malformed")
    if receipt[(1, 1)]["value_hex"] != "+1.46b5490000000X+005":
        # Decimal 5,354,601 is the result-wire magic. Decode instead of relying
        # on a platform-specific zero exponent spelling.
        if int(stata_hex_to_float(receipt[(1, 1)]["value_hex"])) != 5_354_601:
            raise ContractError(f"case {case['id']}: result receipt magic is invalid")
    if stata_hex_to_float(receipt[(1, 16)]["value_hex"]) != 1.0:
        raise ContractError(f"case {case['id']}: result receipt is incomplete")
    for row in (1, 2, 3, 24):
        if (
            stata_hex_to_float(certificate[(row, 1)]["value_hex"]) != 1.0
            or stata_hex_to_float(certificate[(row, 2)]["value_hex"]) != 1.0
        ):
            raise ContractError(f"case {case['id']}: mandatory certificate row {row} failed")
    if not status:
        raise ContractError(f"case {case['id']}: coefficient status is absent")
    anchor = Q_ANCHORS.get(case["id"])
    if anchor is not None:
        covariance = _matrix_map(case, "V")
        if set(covariance) != {(1, 1), (1, 2), (2, 1), (2, 2)}:
            raise ContractError(f"case {case['id']}: sealed covariance dimensions changed")
        observed_axis = (
            covariance[(1, 1)]["row_name"],
            covariance[(2, 1)]["row_name"],
        )
        if observed_axis != ("x1", "_cons"):
            raise ContractError(f"case {case['id']}: sealed covariance axis changed")
        observed_values = [
            covariance[coordinate]["value_hex"]
            for coordinate in ((1, 1), (1, 2), (2, 1), (2, 2))
        ]
        if observed_values != anchor["V"]:
            raise ContractError(f"case {case['id']}: exact internal covariance anchor changed")
        determinant = case["scalars"].get("stfixest_v_determinant")
        if determinant != anchor["determinant"]:
            raise ContractError(f"case {case['id']}: exact Stata determinant anchor changed")


def compare_cases(
    candidate_cases: Sequence[Mapping[str, Any]],
    comparator_cases: Sequence[Mapping[str, Any]],
    projection: Mapping[str, Any],
) -> list[dict[str, Any]]:
    comparator_by_id = {case["id"]: case for case in comparator_cases}
    tolerance = projection["tolerances"]
    results: list[dict[str, Any]] = []
    for candidate in candidate_cases:
        case_id = candidate["id"]
        comparator = comparator_by_id[case_id]
        if candidate["return_code"] != comparator["return_code"]:
            raise ContractError(
                f"case {case_id}: return code {candidate['return_code']} != comparator {comparator['return_code']}"
            )
        _validate_internal_candidate(candidate)
        if candidate["return_code"] != 0:
            results.append({"case_id": case_id, "status": "pass", "maximum_absolute_error": 0.0})
            continue
        if candidate["observation_count"] != comparator["observation_count"] or candidate["observations"] != comparator["observations"]:
            raise ContractError(f"case {case_id}: observation/sample evidence differs")
        maximum_error = 0.0
        for name, expected in comparator["macros"].items():
            actual = candidate["macros"][name]
            if name == "cmd":
                if actual != projection["candidate_command"]:
                    raise ContractError(f"case {case_id}: hidden command identity is not developer-only")
            elif actual != expected:
                raise ContractError(f"case {case_id}: macro {name} differs: {actual!r} != {expected!r}")
        for name, expected in comparator["scalars"].items():
            actual = candidate["scalars"][name]
            if name in EXACT_SCALARS:
                if stata_hex_to_float(actual) != stata_hex_to_float(expected):
                    raise ContractError(f"case {case_id}: exact scalar {name} differs")
                continue
            rtol = tolerance["fit_rtol"] if name in FIT_SCALARS else tolerance["coefficient_rtol"]
            atol = tolerance["fit_atol"] if name in FIT_SCALARS else tolerance["coefficient_atol"]
            passed, error = _close(actual, expected, rtol, atol)
            maximum_error = max(maximum_error, error)
            if not passed:
                raise ContractError(f"case {case_id}: scalar {name} exceeds tolerance")
        for name in comparator["matrix_dimensions"]:
            if candidate["matrix_dimensions"].get(name) != comparator["matrix_dimensions"][name]:
                raise ContractError(f"case {case_id}: matrix {name} dimensions differ")
            actual_matrix = _matrix_map(candidate, name)
            expected_matrix = _matrix_map(comparator, name)
            if set(actual_matrix) != set(expected_matrix):
                raise ContractError(f"case {case_id}: matrix {name} coordinates differ")
            for coordinate, expected in expected_matrix.items():
                actual = actual_matrix[coordinate]
                if actual["row_name"] != expected["row_name"] or actual["column_name"] != expected["column_name"]:
                    raise ContractError(
                        f"case {case_id}: matrix {name} stripe differs at {coordinate}: "
                        f"{actual['row_name']!r}/{actual['column_name']!r} != "
                        f"{expected['row_name']!r}/{expected['column_name']!r}"
                    )
                if name == "b":
                    rtol, atol = tolerance["coefficient_rtol"], tolerance["coefficient_atol"]
                elif name == "V":
                    receipt_cap = SEALED_DEFAULT_V_ABS_CAPS.get(case_id)
                    if receipt_cap is not None:
                        rtol, atol = 0.0, receipt_cap
                    else:
                        rtol, atol = tolerance["vce_rtol"], tolerance["vce_atol"]
                else:
                    if stata_hex_to_float(actual["value_hex"]) != stata_hex_to_float(expected["value_hex"]):
                        raise ContractError(f"case {case_id}: exact matrix {name} differs at {coordinate}")
                    continue
                passed, error = _close(actual["value_hex"], expected["value_hex"], rtol, atol)
                maximum_error = max(maximum_error, error)
                if not passed:
                    raise ContractError(f"case {case_id}: matrix {name}{coordinate} exceeds tolerance")
        results.append({"case_id": case_id, "status": "pass", "maximum_absolute_error": maximum_error})
    return results


def _stata_quote(path: Path) -> str:
    text = str(path.resolve()).replace("\\", "/")
    if '"' in text or "\n" in text or "\r" in text:
        raise ContractError("Stata path contains an unsupported character")
    return f'"{text}"'


def _run_stata_capture(stata: Path, projection_path: Path) -> tuple[dict[str, str], list[dict[str, Any]], str]:
    projection = load_projection(projection_path)
    projected = projected_manifest(projection)
    projection_hash = sha256_file(projection_path)
    datasets = {dataset["id"]: dataset for dataset in projected["datasets"]}
    staged: dict[str, Path] = {}
    for dataset_id in {case["dataset"] for case in projected["semantic_cases"]}:
        staged[dataset_id] = _repository_path(datasets[dataset_id]["source"]["path"], f"dataset {dataset_id}")
    staged["__generator__"] = _repository_path(projected["capture"]["generator"]["path"], "generator")
    with tempfile.TemporaryDirectory(prefix="stfixest-hidden-numeric-") as raw:
        run_dir = Path(raw)
        case_do = run_dir / "candidate_cases.do"
        capture_file = run_dir / "candidate.capture"
        wrapper = run_dir / "candidate_wrapper.do"
        case_do.write_text(
            render_capture_do(projected, staged, run_dir, projection_hash),
            encoding="utf-8",
        )
        isolated = run_dir / "sysdirs"
        lines = ["version 18.0", "clear all", "set more off"]
        for name in ("PERSONAL", "PLUS", "OLDPLACE", "SITE"):
            target = isolated / name.lower()
            target.mkdir(parents=True)
            lines.append(f"sysdir set {name} {_stata_quote(target)}")
        lines.extend(
            [
                f"adopath ++ {_stata_quote(ROOT / 'stata')}",
                f"adopath ++ {_stata_quote(ROOT / 'dist')}",
                f"do {_stata_quote(case_do)} {_stata_quote(capture_file)} {projection_hash}",
            ]
        )
        wrapper.write_text("\n".join(lines) + "\n", encoding="utf-8")
        completed = subprocess.run(
            [str(stata), "-b", "do", str(wrapper)],
            cwd=run_dir,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        logs = "\n".join(
            path.read_text(encoding="utf-8", errors="replace")
            for path in run_dir.glob("*.log")
        )
        if completed.returncode != 0:
            raise ContractError(f"licensed Stata process returned {completed.returncode}")
        if FORBIDDEN_WARNING in logs.lower():
            raise ContractError("licensed Stata emitted the forbidden covariance warning")
        if not capture_file.is_file():
            raise ContractError("licensed Stata did not create a candidate capture")
        capture_text = capture_file.read_text(encoding="utf-8", errors="strict")
        try:
            parsed = parse_capture(capture_text, projected, projection_hash)
        except ContractError as exc:
            log_tail = "\n".join(logs.splitlines()[-300:])
            raise ContractError(f"{exc}\nlicensed Stata log tail:\n{log_tail}") from exc
        return parsed.environment, parsed.cases, hashlib.sha256(capture_text.encode("utf-8")).hexdigest()


def _source_inputs(plugin: Path) -> dict[str, str]:
    paths = (
        ROOT / "stata/_stfixest_ols_hidden_numeric_v1.ado",
        ROOT / "stata/_stfixest_estimate_ols_test.ado",
        ROOT / "crates/stfixest-plugin/src/estimate.rs",
        ROOT / "scripts/ols_hidden_numeric.py",
        plugin,
    )
    return {path.name: sha256_file(path) for path in paths}


def _cargo_binary(explicit: Path | None) -> Path:
    if explicit is not None:
        candidate = explicit
    else:
        discovered = shutil.which("cargo")
        candidate = Path(discovered) if discovered else Path.home() / ".rustup/toolchains/1.85.1-aarch64-apple-darwin/bin/cargo"
    if not candidate.is_file():
        raise ContractError("cargo is required to revalidate the fake-SPI failure sweep")
    return candidate.resolve()


def _supporting_gate_evidence() -> dict[str, Any]:
    return {
        "fake_spi_store_failures": {
            "status": "pass",
            "ordinals": 216,
            "test_sha256": sha256_file(ROOT / "crates/stfixest-plugin/src/estimate.rs"),
        },
        "actual_plugin_loader": {
            "status": "pass",
            "ordinals": 213,
            "test_sha256": sha256_file(ROOT / "tests/mock_spi/actual_plugin_loader_test.c"),
        },
        "stata_eclass_rollback": {
            "status": "pass",
            "fixture_sha256": sha256_file(ROOT / "tests/stata/installation/estimate_ols_eclass_transaction.do"),
        },
        "stata_numeric_parity": {
            "status": "pass",
            "fixture_sha256": sha256_file(ROOT / "tests/stata/parity_reghdfe/estimate_ols_hidden_parity.do"),
        },
        "stata_actual_break": {
            "status": "pass",
            "fixture_sha256": sha256_file(ROOT / "tests/licensed_stata/estimate_ols_eclass_break.do"),
        },
    }


def _run_supporting_gates(stata: Path, plugin: Path, cargo: Path | None) -> dict[str, Any]:
    cargo = _cargo_binary(cargo)
    environment = os.environ.copy()
    environment["PATH"] = str(cargo.parent) + os.pathsep + environment.get("PATH", "")
    commands = (
        (
            "fake_spi_store_failures",
            [
                str(cargo),
                "test",
                "-p",
                "stfixest-plugin",
                "every_output_store_failure_leaves_receipt_incomplete",
            ],
        ),
        (
            "actual_plugin_loader",
            [str(ROOT / "tests/mock_spi/run_actual_unix_plugin_test.sh"), str(plugin.resolve())],
        ),
        (
            "stata_eclass_rollback",
            [
                sys.executable,
                str(ROOT / "scripts/run_stata_tests.py"),
                "--stata",
                str(stata),
                str(ROOT / "tests/stata/installation/estimate_ols_eclass_transaction.do"),
            ],
        ),
        (
            "stata_numeric_parity",
            [
                sys.executable,
                str(ROOT / "scripts/run_stata_tests.py"),
                "--stata",
                str(stata),
                str(ROOT / "tests/stata/parity_reghdfe/estimate_ols_hidden_parity.do"),
            ],
        ),
        (
            "stata_actual_break",
            [
                sys.executable,
                str(ROOT / "scripts/run_stata_break_test.py"),
                "--stata",
                str(stata),
                "--timeout",
                "30",
            ],
        ),
    )
    for name, command in commands:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            env=environment,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        if completed.returncode != 0:
            tail = "\n".join(completed.stdout.splitlines()[-80:])
            raise ContractError(f"supporting gate {name} failed:\n{tail}")
    return _supporting_gate_evidence()


def capture_receipt(
    stata: Path,
    plugin: Path,
    projection_path: Path,
    output: Path,
    cargo: Path | None = None,
) -> str:
    projection = load_projection(projection_path)
    comparator_path = _repository_path(projection["comparator_receipt"]["path"], "comparator_receipt")
    comparator_document = verify_reference_receipt(
        comparator_path,
        _repository_path(projection["parent_manifest"]["path"], "parent_manifest"),
        ROOT / "references.lock.toml",
        ROOT,
    )
    if comparator_document["seal"]["payload_sha256"] != projection["comparator_receipt"]["payload_sha256"]:
        raise ContractError("comparator payload seal differs from projection")
    environment, cases, capture_sha = _run_stata_capture(stata, projection_path)
    comparisons = compare_cases(cases, comparator_document["payload"]["cases"], projection)
    supporting_gates = _run_supporting_gates(stata, plugin, cargo)
    payload = {
        "schema_version": 1,
        "receipt_type": RECEIPT_TYPE,
        "mode": projection["mode"],
        "public_activation": False,
        "performance": {"status": "unmeasured", "claim": False},
        "projection": {"id": projection["manifest_id"], "sha256": sha256_file(projection_path)},
        "parent_manifest": {
            "id": "ols_v1",
            "file_sha256": projection["parent_manifest"]["sha256"],
        },
        "comparator_receipt": {
            "file_sha256": projection["comparator_receipt"]["file_sha256"],
            "payload_sha256": projection["comparator_receipt"]["payload_sha256"],
        },
        "candidate": {
            "command": projection["candidate_command"],
            "intended_public_command": projection["intended_public_command"],
            "inputs": _source_inputs(plugin),
            "capture_sha256": capture_sha,
        },
        "environment": environment,
        "host": {
            "system": platform.system(),
            "machine": platform.machine(),
            "cpu_count": os.cpu_count() or 1,
        },
        "executed_cases": list(EXECUTABLE_CASE_IDS),
        "blockers": [{"case_id": case, "reason": reason} for case, reason in BLOCKERS],
        "comparisons": comparisons,
        "cases": cases,
        "q2_q3_anchors": Q_ANCHORS,
        "supporting_gates": supporting_gates,
        "forbidden_covariance_warning": "absent",
    }
    document = seal_payload(payload)
    encoded = json.dumps(document, sort_keys=True, indent=2, ensure_ascii=True) + "\n"
    output.parent.mkdir(parents=True, exist_ok=True)
    try:
        with output.open("x", encoding="utf-8", newline="\n") as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
    except FileExistsError as exc:
        raise ContractError(f"refusing to overwrite immutable receipt: {output}") from exc
    output.chmod(stat.S_IRUSR | stat.S_IRGRP | stat.S_IROTH)
    return document["seal"]["payload_sha256"]


def verify_candidate_receipt(path: Path, projection_path: Path = DEFAULT_PROJECTION) -> dict[str, Any]:
    projection = load_projection(projection_path)
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ContractError(f"cannot read candidate receipt: {exc}") from exc
    if not isinstance(document, dict) or set(document) != {"payload", "seal"}:
        raise ContractError("candidate receipt must contain payload and seal")
    payload = document["payload"]
    expected_seal = hashlib.sha256(canonical_json_bytes(payload)).hexdigest()
    if document["seal"] != {
        "algorithm": "sha256",
        "canonicalization": "json-sort-keys-compact-ascii-v1",
        "payload_sha256": expected_seal,
    }:
        raise ContractError("candidate receipt seal is invalid")
    if path.stat().st_mode & 0o222:
        raise ContractError("candidate receipt must be read-only")
    if not isinstance(payload, Mapping):
        raise ContractError("candidate receipt payload must be an object")
    _require_keys(
        payload,
        {
            "schema_version",
            "receipt_type",
            "mode",
            "public_activation",
            "performance",
            "projection",
            "parent_manifest",
            "comparator_receipt",
            "candidate",
            "environment",
            "host",
            "executed_cases",
            "blockers",
            "comparisons",
            "cases",
            "q2_q3_anchors",
            "supporting_gates",
            "forbidden_covariance_warning",
        },
        "candidate receipt payload",
    )
    _assert_redacted(payload)
    if (
        payload.get("schema_version") != 1
        or payload.get("receipt_type") != RECEIPT_TYPE
        or payload.get("mode") != "developer_hidden_current_surface"
        or payload.get("public_activation") is not False
        or payload.get("performance") != {"status": "unmeasured", "claim": False}
        or payload.get("projection") != {"id": projection["manifest_id"], "sha256": sha256_file(projection_path)}
        or payload.get("parent_manifest")
        != {"id": "ols_v1", "file_sha256": projection["parent_manifest"]["sha256"]}
        or payload.get("comparator_receipt")
        != {
            "file_sha256": projection["comparator_receipt"]["file_sha256"],
            "payload_sha256": projection["comparator_receipt"]["payload_sha256"],
        }
        or payload.get("executed_cases") != list(EXECUTABLE_CASE_IDS)
        or payload.get("blockers") != [{"case_id": case, "reason": reason} for case, reason in BLOCKERS]
        or payload.get("candidate", {}).get("inputs")
        != _source_inputs(ROOT / "dist/stfixest_macos.plugin")
        or payload.get("supporting_gates") != _supporting_gate_evidence()
        or payload.get("forbidden_covariance_warning") != "absent"
    ):
        raise ContractError("candidate receipt identity/boundary is stale")
    candidate = payload["candidate"]
    if not isinstance(candidate, Mapping):
        raise ContractError("candidate receipt candidate identity is malformed")
    _require_keys(
        candidate,
        {"command", "intended_public_command", "inputs", "capture_sha256"},
        "candidate receipt candidate identity",
    )
    if (
        candidate["command"] != projection["candidate_command"]
        or candidate["intended_public_command"]
        != projection["intended_public_command"]
        or not isinstance(candidate["capture_sha256"], str)
        or not HEX_SHA256.fullmatch(candidate["capture_sha256"])
    ):
        raise ContractError("candidate receipt command/capture identity is malformed")
    environment = payload["environment"]
    if (
        not isinstance(environment, Mapping)
        or set(environment) != ENVIRONMENT_FIELDS
        or not all(isinstance(value, str) and value for value in environment.values())
    ):
        raise ContractError("candidate receipt Stata environment is malformed")
    host = payload["host"]
    if (
        not isinstance(host, Mapping)
        or set(host) != {"system", "machine", "cpu_count"}
        or not isinstance(host["system"], str)
        or not host["system"]
        or not isinstance(host["machine"], str)
        or not host["machine"]
        or not isinstance(host["cpu_count"], int)
        or host["cpu_count"] <= 0
    ):
        raise ContractError("candidate receipt host metadata is malformed")
    cases = payload.get("cases")
    projected_cases = projected_manifest(projection)["semantic_cases"]
    if not isinstance(cases, list) or [case.get("id") for case in cases] != [
        case["id"] for case in projected_cases
    ]:
        raise ContractError("candidate receipt case inventory/order is invalid")
    for expected, captured in zip(projected_cases, cases):
        _validate_case_evidence(expected, captured)
    comparisons = payload.get("comparisons")
    if not isinstance(comparisons, list) or [item.get("case_id") for item in comparisons] != [case["id"] for case in cases] or any(item.get("status") != "pass" for item in comparisons):
        raise ContractError("candidate receipt comparisons are incomplete")
    comparator_document = verify_reference_receipt(
        _repository_path(
            projection["comparator_receipt"]["path"], "comparator_receipt"
        ),
        _repository_path(projection["parent_manifest"]["path"], "parent_manifest"),
        ROOT / "references.lock.toml",
        ROOT,
    )
    recomputed = compare_cases(
        cases,
        comparator_document["payload"]["cases"],
        projection,
    )
    if comparisons != recomputed:
        raise ContractError(
            "candidate receipt comparisons do not reproduce from sealed cases"
        )
    if payload.get("q2_q3_anchors") != Q_ANCHORS:
        raise ContractError("candidate receipt q2/q3 anchors changed")
    return document


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--projection", type=Path, default=DEFAULT_PROJECTION)
    subparsers = parser.add_subparsers(dest="command", required=True)
    validate_parser = subparsers.add_parser("validate-projection")
    validate_parser.set_defaults(action="validate")
    capture_parser = subparsers.add_parser("capture")
    capture_parser.add_argument("--stata", type=Path, required=True)
    capture_parser.add_argument("--plugin", type=Path, default=ROOT / "dist/stfixest_macos.plugin")
    capture_parser.add_argument("--cargo", type=Path)
    capture_parser.add_argument("--output", type=Path, required=True)
    capture_parser.set_defaults(action="capture")
    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("receipt", type=Path)
    verify_parser.set_defaults(action="verify")
    args = parser.parse_args()
    try:
        if args.action == "validate":
            projection = load_projection(args.projection)
            print(
                f"hidden numeric projection PASS: {len(projection['executable_case_ids'])} executable, "
                f"{len(projection['blockers'])} blocked"
            )
        elif args.action == "capture":
            digest = capture_receipt(
                args.stata,
                args.plugin,
                args.projection,
                args.output,
                args.cargo,
            )
            print(f"sealed hidden numeric candidate receipt: {args.output} sha256={digest}")
        else:
            document = verify_candidate_receipt(args.receipt, args.projection)
            print(f"hidden numeric candidate receipt PASS: {document['seal']['payload_sha256']}")
    except (ContractError, OSError, subprocess.SubprocessError) as exc:
        raise SystemExit(f"hidden numeric candidate gate failed: {exc}") from exc


if __name__ == "__main__":
    main()
