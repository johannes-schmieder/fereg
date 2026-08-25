#!/usr/bin/env python3
"""Capture and combine same-node paired fereg/reghdfe benchmark evidence."""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from typing import Any, Mapping, Sequence

try:
    from scripts.ols_reference import (
        DEFAULT_LOCK,
        DEFAULT_MANIFEST,
        ESTIMATOR_RSS_SCOPE,
        ROOT,
        ContractError,
        _host_metadata,
        _prepare_generated_datasets,
        _run_fresh_process,
        _select_persistent_private_ftools_mlib,
        _validate_case_evidence,
        canonical_json_bytes,
        evaluate_benchmark_gates,
        load_manifest,
        load_reference_lock,
        parse_capture,
        preflight_reference_root,
        render_capture_do,
        seal_payload,
        sha256_file,
        stage_manifest_inputs,
        stage_references,
        write_sealed_receipt,
        _execute_capture_unit,
    )
except ModuleNotFoundError:  # Direct ``python scripts/ols_paired.py`` use.
    from ols_reference import (  # type: ignore[no-redef]
        DEFAULT_LOCK,
        DEFAULT_MANIFEST,
        ESTIMATOR_RSS_SCOPE,
        ROOT,
        ContractError,
        _host_metadata,
        _prepare_generated_datasets,
        _run_fresh_process,
        _select_persistent_private_ftools_mlib,
        _validate_case_evidence,
        canonical_json_bytes,
        evaluate_benchmark_gates,
        load_manifest,
        load_reference_lock,
        parse_capture,
        preflight_reference_root,
        render_capture_do,
        seal_payload,
        sha256_file,
        stage_manifest_inputs,
        stage_references,
        write_sealed_receipt,
        _execute_capture_unit,
    )


PAIR_RECEIPT_TYPE = "fereg-ols-v1-paired-case"
QUALIFICATION_RECEIPT_TYPE = "fereg-ols-v1-paired-qualification"
CANDIDATE_LOG_SENTINEL = "END_FIXTURE|baseline_fereg|rc=0"
SOURCE_SHA = re.compile(r"^[0-9a-f]{40}$")
BASE_COMPONENT = re.compile(r"^-?[0-9]+b[n]?\.")
OMITTED_COMPONENT = re.compile(r"^(?:-?[0-9]+o\.|co\.|o\.)")
CANDIDATE_MATRICES = (
    "fereg_certificate",
    "fereg_runtime",
    "fereg_solver",
)
MINIMUM_HEADROOM_BYTES = 512 * 1024 * 1024


def stata_hex_number(value: str) -> float:
    """Decode Stata's canonical ``%21x`` finite numeric representation."""
    if value == ".":
        raise ContractError("required Stata value is missing")
    match = re.fullmatch(
        r"(?P<sign>[+-])(?P<integer>[0-9A-Fa-f]+)"
        r"(?:\.(?P<fraction>[0-9A-Fa-f]+))?X"
        r"(?P<exponent_sign>[+-])(?P<exponent>[0-9A-Fa-f]+)",
        value,
    )
    if match is None:
        raise ContractError(f"invalid canonical Stata hex number: {value!r}")
    fraction = match.group("fraction") or ""
    significand = float(int(match.group("integer"), 16))
    if fraction:
        significand += int(fraction, 16) / float(16 ** len(fraction))
    exponent = int(match.group("exponent"), 16)
    if match.group("exponent_sign") == "-":
        exponent = -exponent
    result = math.ldexp(significand, exponent)
    if match.group("sign") == "-":
        result = -result
    if not math.isfinite(result):
        raise ContractError("decoded Stata value is nonfinite")
    return result


def _matrix_entries(case: Mapping[str, Any], name: str) -> list[Mapping[str, Any]]:
    dimensions = case["matrix_dimensions"].get(name)
    if not isinstance(dimensions, Mapping):
        raise ContractError(f"captured case lacks matrix {name}")
    entries = [item for item in case["matrices"] if item["name"] == name]
    expected = int(dimensions["rows"]) * int(dimensions["columns"])
    if len(entries) != expected:
        raise ContractError(f"captured matrix {name} is incomplete")
    return sorted(entries, key=lambda item: (item["row"], item["column"]))


def _matrix_value(case: Mapping[str, Any], name: str, row: int, column: int) -> float:
    for entry in _matrix_entries(case, name):
        if entry["row"] == row and entry["column"] == column:
            return stata_hex_number(str(entry["value_hex"]))
    raise ContractError(f"captured matrix {name} lacks coordinate ({row}, {column})")


def coefficient_status(name: str) -> str:
    components = name.split("#")
    if any(BASE_COMPONENT.match(component) for component in components):
        return "base"
    if any(OMITTED_COMPONENT.match(component) for component in components):
        return "omitted"
    return "kept"


def scientific_evidence(case: Mapping[str, Any], arm: str) -> dict[str, Any]:
    b = _matrix_entries(case, "b")
    v = _matrix_entries(case, "V")
    names = [str(entry["column_name"]) for entry in b]
    coefficients = [stata_hex_number(str(entry["value_hex"])) for entry in b]
    dimensions = case["matrix_dimensions"]["V"]
    if dimensions != {"rows": len(names), "columns": len(names)}:
        raise ContractError("captured V dimensions contradict e(b)")
    scalars = case["scalars"]
    dof_names = ("df_m", "df_r", "df_a")
    fit_names = ("rss", "tss", "r2", "r2_a", "rmse")
    if not set((*dof_names, *fit_names, "converged")).issubset(scalars):
        raise ContractError("benchmark scalar evidence is incomplete")
    if stata_hex_number(str(scalars["converged"])) != 1.0:
        raise ContractError(f"{arm} benchmark did not report convergence")
    if arm == "candidate":
        passed = all(
            _matrix_value(case, "fereg_certificate", row, 2) == 1.0
            for row in (1, 2, 3, 24, 32)
        )
        normal_equation = max(
            _matrix_value(case, "fereg_solver", row, 8) for row in (1, 2)
        )
        fe_orthogonality = max(
            _matrix_value(case, "fereg_solver", row, 6) for row in (1, 2)
        )
        vce_symmetry = _matrix_value(case, "fereg_certificate", 19, 2)
    else:
        # Pinned reghdfe publishes convergence but no corresponding norm
        # scalars. Its captured b/V/DoF/fit are the exact scientific oracle;
        # candidate-side native norms remain independently binding below.
        passed = True
        normal_equation = 0.0
        fe_orthogonality = 0.0
        vce_symmetry = 0.0
    return {
        "coefficient_names": names,
        "coefficient_status": [coefficient_status(name) for name in names],
        "coefficients": coefficients,
        "vce": {
            "rows": len(names),
            "columns": len(names),
            "values": [
                stata_hex_number(str(entry["value_hex"])) for entry in v
            ],
        },
        "dof": {
            name: stata_hex_number(str(scalars[name])) for name in dof_names
        },
        "fit": {
            name: stata_hex_number(str(scalars[name])) for name in fit_names
        },
        "objective": {
            "name": "rss",
            "value": stata_hex_number(str(scalars["rss"])),
        },
        "certificates": {
            "converged": passed,
            "normal_equation_inf_norm": normal_equation,
            "fe_orthogonality_inf_norm": fe_orthogonality,
            "vce_symmetry_inf_norm": vce_symmetry,
        },
    }


def candidate_memory(cases: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    observed: list[int] = []
    modeled: set[int] = set()
    leaks: list[int] = []
    for case in cases:
        observed.append(round(_matrix_value(case, "fereg_runtime", 13, 2)))
        modeled.add(round(_matrix_value(case, "fereg_runtime", 25, 2)))
        start = round(_matrix_value(case, "fereg_runtime", 10, 2))
        end = round(_matrix_value(case, "fereg_runtime", 11, 2))
        leaks.append(max(0, end - start))
    if any(value <= 0 for value in observed) or len(modeled) != 1:
        raise ContractError("candidate allocation telemetry is absent or unstable")
    model = next(iter(modeled))
    if model <= 0:
        raise ContractError("candidate allocation model is nonpositive")
    return {
        "native_allocation_observed_bytes": max(observed),
        "native_allocation_model_bytes": model,
        "repeated_call_leak_bytes": max(leaks),
        "constrained_memory": None,
    }


def imposed_limit_between(low_bytes: int, fast_bytes: int) -> int:
    """Choose a preflight limit whose usable budget admits low, not fast."""
    if low_bytes <= 0 or fast_bytes <= low_bytes:
        raise ContractError("constrained proof requires low memory below fast memory")
    available = max(
        low_bytes + MINIMUM_HEADROOM_BYTES,
        (low_bytes * 5 + 3) // 4,
    )
    while available - max(MINIMUM_HEADROOM_BYTES, available // 5) < low_bytes:
        available += 1
    usable = available - max(MINIMUM_HEADROOM_BYTES, available // 5)
    if usable >= fast_bytes:
        raise ContractError("no imposed memory limit separates low from fast")
    return available


def _arm_case(case: Mapping[str, Any], arm: str, kind: str) -> dict[str, Any]:
    selected = copy.deepcopy(case)
    selected["command"] = case["arms"][arm]
    selected["repetitions"] = (
        {"cold": 1, "warm": 0} if kind == "cold" else {"cold": 0, "warm": 1}
    )
    if kind == "warm":
        selected["_fresh_warmup"] = True
    selected["capture"]["scalars"].append("converged")
    if arm == "candidate":
        selected["capture"]["matrices"].extend(CANDIDATE_MATRICES)
    return selected


def _write_candidate_wrapper(
    path: Path,
    case_do: Path,
    capture_file: Path,
    candidate_ado: Path,
    manifest_sha256: str,
    isolated_sysdirs: Path,
) -> None:
    sysdirs: dict[str, Path] = {}
    for name in ("personal", "plus", "oldplace", "site"):
        target = isolated_sysdirs / f"candidate-{name}"
        target.mkdir(parents=True, exist_ok=True)
        sysdirs[name] = target
    expected_ado = str(candidate_ado / "fereg.ado").replace("\\", "/")
    expected_plugin = str(candidate_ado / "fereg_linux.plugin").replace("\\", "/")
    quote = lambda value: '"' + str(value).replace("\\", "/") + '"'
    lines = ["version 18.0", "clear all", "set more off"]
    for name, target in sysdirs.items():
        lines.append(f"sysdir set {name.upper()} {quote(target)}")
    lines.extend(
        [
            f"adopath ++ {quote(candidate_ado)}",
            "capture findfile fereg.ado",
            "if _rc exit 111",
            """local __frg_found `"`r(fn)'"'""",
            f"""if `"`__frg_found'"' != "{expected_ado}" exit 603""",
            "capture findfile fereg_linux.plugin",
            "if _rc exit 111",
            """local __frg_plugin `"`r(fn)'"'""",
            f"""if `"`__frg_plugin'"' != "{expected_plugin}" exit 603""",
            (
                f"capture noisily do {quote(case_do)} {quote(capture_file)} "
                f"{quote(manifest_sha256)}"
            ),
            "local __frg_case_rc = _rc",
            "if `__frg_case_rc' != 0 exit `__frg_case_rc'",
            f'di as result "{CANDIDATE_LOG_SENTINEL}"',
            "exit 0",
            "",
        ]
    )
    path.write_text("\n".join(lines), encoding="utf-8")


def _execute_candidate_unit(
    stata: str,
    unit_manifest: Mapping[str, Any],
    staged_inputs: Mapping[str, Path],
    candidate_ado: Path,
    unit_dir: Path,
    isolated_sysdirs: Path,
    manifest_hash: str,
) -> tuple[dict[str, Any], dict[str, Any]]:
    unit_dir.mkdir()
    case_do = unit_dir / "ols_v1_cases.do"
    case_do.write_text(
        render_capture_do(unit_manifest, staged_inputs, unit_dir, manifest_hash),
        encoding="utf-8",
    )
    capture_file = unit_dir / "capture.machine"
    wrapper = unit_dir / "run.do"
    _write_candidate_wrapper(
        wrapper,
        case_do,
        capture_file,
        candidate_ado,
        manifest_hash,
        isolated_sysdirs,
    )
    completed, rss = _run_fresh_process(
        [stata, "-b", "do", str(wrapper)], unit_dir
    )
    log_path = unit_dir / "run.log"
    log_text = (
        log_path.read_text(encoding="utf-8", errors="replace")
        if log_path.is_file()
        else ""
    )
    sentinels = [
        line.strip()
        for line in log_text.splitlines()
        if line.strip().startswith("END_FIXTURE|")
    ]
    if completed.returncode != 0 or sentinels != [CANDIDATE_LOG_SENTINEL]:
        raise ContractError(
            "licensed Stata candidate capture failed or lacked its unique sentinel"
        )
    if not capture_file.is_file():
        raise ContractError("licensed Stata candidate capture produced no machine record")
    parsed = parse_capture(
        capture_file.read_text(encoding="utf-8", errors="strict"),
        unit_manifest,
        manifest_hash,
    )
    return parsed.cases[0], {**rss, "scope": ESTIMATOR_RSS_SCOPE}


def _stage_candidate(candidate_root: Path, plugin: Path, destination: Path) -> Path:
    stata_source = candidate_root / "stata"
    if not stata_source.is_dir() or not plugin.is_file() or plugin.is_symlink():
        raise ContractError("candidate Stata source or audited plugin is absent")
    candidate_ado = destination / "stata"
    shutil.copytree(stata_source, candidate_ado)
    shutil.copy2(plugin, candidate_ado / "fereg_linux.plugin")
    return candidate_ado


def _constrained_memory_proof(
    *,
    stata: str,
    case: Mapping[str, Any],
    manifest: Mapping[str, Any],
    dataset: Mapping[str, Any],
    staged_inputs: Mapping[str, Path],
    candidate_ado: Path,
    work_dir: Path,
    isolated_sysdirs: Path,
    manifest_hash: str,
    fast_bytes: int,
    low_bytes: int,
) -> dict[str, Any]:
    imposed = imposed_limit_between(low_bytes, fast_bytes)
    previous = os.environ.get("FEREG_MEMORY_LIMIT_BYTES")
    os.environ["FEREG_MEMORY_LIMIT_BYTES"] = str(imposed)
    try:
        outcomes: dict[str, Mapping[str, Any]] = {}
        for mode in ("low", "fast"):
            run_case = _arm_case(case, "candidate", "cold")
            run_case["command"] += f" memory({mode})"
            if mode == "fast":
                run_case["expected"] = "must_error"
            unit_manifest = copy.deepcopy(manifest)
            unit_manifest["datasets"] = [{**dataset, "kind": "prepared_dta"}]
            unit_manifest["semantic_cases"] = []
            unit_manifest["benchmark_cases"] = [run_case]
            captured, _ = _execute_candidate_unit(
                stata,
                unit_manifest,
                staged_inputs,
                candidate_ado,
                work_dir / f"constrained-{mode}",
                isolated_sysdirs,
                manifest_hash,
            )
            _validate_case_evidence(run_case, captured)
            outcomes[mode] = captured
    finally:
        if previous is None:
            os.environ.pop("FEREG_MEMORY_LIMIT_BYTES", None)
        else:
            os.environ["FEREG_MEMORY_LIMIT_BYTES"] = previous
    low = outcomes["low"]
    fast = outcomes["fast"]
    if low["return_code"] != 0 or fast["return_code"] != 909:
        raise ContractError("constrained low/fast proof returned unexpected codes")
    if round(_matrix_value(low, "fereg_runtime", 2, 2)) != 2:
        raise ContractError("constrained low run did not select low mode")
    return {
        "method": "FEREG_MEMORY_LIMIT_BYTES_native_preflight",
        "imposed_available_bytes": imposed,
        "modeled_fast_bytes": fast_bytes,
        "modeled_low_bytes": low_bytes,
        "low_return_code": 0,
        "fast_return_code": 909,
        "low_succeeds": True,
        "fast_fails": True,
    }


def capture_pair(
    *,
    stata: str,
    case_id: str,
    reference_root: Path,
    candidate_root: Path,
    candidate_plugin: Path,
    source_sha: str,
    work_dir: Path,
    receipt: Path,
    manifest_path: Path,
    lock_path: Path,
    prove_constrained: bool = False,
) -> str:
    if not SOURCE_SHA.fullmatch(source_sha):
        raise ContractError("candidate source SHA must be a full lowercase commit")
    manifest = load_manifest(manifest_path, candidate_root)
    manifest_hash = sha256_file(manifest_path)
    case = next(
        (item for item in manifest["benchmark_cases"] if item["id"] == case_id),
        None,
    )
    if case is None:
        raise ContractError(f"unknown benchmark case {case_id!r}")
    lock = load_reference_lock(lock_path)
    identities = preflight_reference_root(reference_root, lock, candidate_root)
    work_dir.mkdir(parents=True, exist_ok=False)
    reference_stage = work_dir / "references"
    reference_stage.mkdir()
    staged_references = stage_references(identities, lock, reference_stage)
    inputs = work_dir / "inputs"
    inputs.mkdir()
    staged_inputs = stage_manifest_inputs(manifest, candidate_root, inputs)
    isolated_sysdirs = work_dir / "sysdirs"
    isolated_sysdirs.mkdir()
    baseline = candidate_root / manifest["capture"]["script"]["path"]
    preflight_manifest = copy.deepcopy(manifest)
    preflight_manifest["semantic_cases"] = []
    preflight_manifest["benchmark_cases"] = []
    preflight, _, generated = _execute_capture_unit(
        stata,
        preflight_manifest,
        staged_inputs,
        staged_references,
        baseline,
        work_dir / "reference-preflight",
        isolated_sysdirs,
        manifest_hash,
        subprocess.run,
        "preflight",
    )
    if generated or preflight.cases:
        raise ContractError("reference preflight emitted case or dataset evidence")
    private_mlib = _select_persistent_private_ftools_mlib(
        staged_references, isolated_sysdirs
    )
    private_mlib_hash = sha256_file(private_mlib)
    dataset = next(item for item in manifest["datasets"] if item["id"] == case["dataset"])
    dataset_manifest = copy.deepcopy(manifest)
    dataset_manifest["datasets"] = [dataset]
    prepared_paths, generated_hashes = _prepare_generated_datasets(
        stata,
        dataset_manifest,
        staged_inputs,
        work_dir / "prepared-datasets",
        subprocess.run,
    )
    prepared_inputs = {**staged_inputs, **prepared_paths}
    candidate_ado = _stage_candidate(
        candidate_root, candidate_plugin, work_dir / "candidate"
    )
    environments: dict[str, Mapping[str, str]] = {}
    scientific: dict[str, dict[str, Any]] = {}
    captured_candidate: list[Mapping[str, Any]] = []
    records: list[dict[str, Any]] = []
    total = case["repetitions"]["cold"] + case["repetitions"]["warm"]
    for arm in ("comparator", "candidate"):
        runs: list[dict[str, Any]] = []
        arm_scientific_hash: str | None = None
        arm_scientific: dict[str, Any] | None = None
        for repetition in range(1, total + 1):
            kind = "cold" if repetition <= case["repetitions"]["cold"] else "warm"
            run_case = _arm_case(case, arm, kind)
            unit_manifest = copy.deepcopy(manifest)
            unit_manifest["datasets"] = [{**dataset, "kind": "prepared_dta"}]
            unit_manifest["semantic_cases"] = []
            unit_manifest["benchmark_cases"] = [run_case]
            unit_dir = work_dir / f"{arm}-{repetition}"
            if arm == "comparator":
                parsed, rss, unexpected = _execute_capture_unit(
                    stata,
                    unit_manifest,
                    prepared_inputs,
                    staged_references,
                    baseline,
                    unit_dir,
                    isolated_sysdirs,
                    manifest_hash,
                    subprocess.run,
                    "estimator",
                )
                if unexpected:
                    raise ContractError("measured comparator regenerated benchmark data")
                captured = parsed.cases[0]
                environment = parsed.environment
            else:
                captured, rss = _execute_candidate_unit(
                    stata,
                    unit_manifest,
                    prepared_inputs,
                    candidate_ado,
                    unit_dir,
                    isolated_sysdirs,
                    manifest_hash,
                )
                capture_text = (unit_dir / "capture.machine").read_text(encoding="utf-8")
                environment = parse_capture(
                    capture_text, unit_manifest, manifest_hash
                ).environment
                captured_candidate.append(captured)
            _validate_case_evidence(run_case, captured)
            current_scientific = scientific_evidence(captured, arm)
            current_hash = hashlib.sha256(
                canonical_json_bytes(current_scientific)
            ).hexdigest()
            if arm_scientific_hash is None:
                arm_scientific_hash = current_hash
                arm_scientific = current_scientific
            elif arm_scientific_hash != current_hash:
                raise ContractError(
                    f"{arm} scientific evidence changed across fresh repetitions"
                )
            timing = captured["timings"][0]
            runs.append(
                {
                    "repetition": repetition,
                    "kind": kind,
                    "seconds": stata_hex_number(str(timing["seconds_hex"])),
                    "peak_rss": rss,
                    "fresh_process": True,
                    "stata_processors": captured["runtime_thread_count"],
                }
            )
            if arm in environments and environments[arm] != environment:
                raise ContractError(f"{arm} Stata environment changed between runs")
            environments[arm] = environment
        if arm_scientific is None:
            raise ContractError(f"{arm} produced no scientific evidence")
        scientific[arm] = arm_scientific
        records.append(
            {
                "case_id": case_id,
                "arm": arm,
                "thread_count": case["thread_count"],
                "runs": runs,
                "sample_identity_sha256": captured["identities"]["sample"]["sha256"],
                "singleton_identity_sha256": captured["identities"]["singletons"]["sha256"],
                "host": _host_metadata(),
                "stata": {
                    "version": environment["stata_version"],
                    "flavor": environment["stata_flavor"],
                    "processors_limit": environment["stata_processors_limit"],
                },
                "scientific": arm_scientific,
                "memory": (
                    {
                        "native_allocation_observed_bytes": None,
                        "native_allocation_model_bytes": None,
                        "repeated_call_leak_bytes": None,
                        "constrained_memory": None,
                    }
                    if arm == "comparator"
                    else candidate_memory(captured_candidate)
                ),
            }
        )
    constrained_proof = None
    if prove_constrained:
        first_candidate = captured_candidate[0]
        fast_bytes = round(_matrix_value(first_candidate, "fereg_runtime", 4, 2))
        low_bytes = round(_matrix_value(first_candidate, "fereg_runtime", 5, 2))
        constrained_proof = _constrained_memory_proof(
            stata=stata,
            case=case,
            manifest=manifest,
            dataset=dataset,
            staged_inputs=prepared_inputs,
            candidate_ado=candidate_ado,
            work_dir=work_dir,
            isolated_sysdirs=isolated_sysdirs,
            manifest_hash=manifest_hash,
            fast_bytes=fast_bytes,
            low_bytes=low_bytes,
        )
        candidate_record = next(
            record for record in records if record["arm"] == "candidate"
        )
        candidate_record["memory"]["constrained_memory"] = {
            "low_succeeds": True,
            "fast_fails": True,
        }
    if environments["comparator"] != environments["candidate"]:
        raise ContractError("paired arms used different Stata environments")
    if sha256_file(private_mlib) != private_mlib_hash:
        raise ContractError("private ftools mlib changed during paired runs")
    payload = {
        "schema_version": 1,
        "receipt_type": PAIR_RECEIPT_TYPE,
        "source_sha": source_sha,
        "manifest": {"id": manifest["manifest_id"], "sha256": manifest_hash},
        "case_id": case_id,
        "candidate_plugin_sha256": sha256_file(candidate_plugin),
        "references": {
            name: identity.receipt_value()
            for name, identity in sorted(identities.items())
        },
        "generated_dataset_sha256": generated_hashes[case["dataset"]],
        "constrained_memory_proof": constrained_proof,
        "evidence": records,
    }
    return write_sealed_receipt(receipt, payload)


def _read_sealed(path: Path) -> Mapping[str, Any]:
    document = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(document, Mapping) or set(document) != {"payload", "seal"}:
        raise ContractError(f"invalid paired receipt envelope: {path.name}")
    if document["seal"] != seal_payload(document["payload"])["seal"]:
        raise ContractError(f"paired receipt seal mismatch: {path.name}")
    if path.stat().st_mode & 0o222:
        raise ContractError(f"paired receipt is writable: {path.name}")
    return document["payload"]


def combine_receipts(
    receipt_dir: Path,
    output: Path,
    manifest_path: Path = DEFAULT_MANIFEST,
) -> str:
    manifest = load_manifest(manifest_path)
    expected = {case["id"] for case in manifest["benchmark_cases"]}
    payloads = [_read_sealed(path) for path in sorted(receipt_dir.glob("*.json"))]
    if not payloads or any(
        payload.get("receipt_type") != PAIR_RECEIPT_TYPE for payload in payloads
    ):
        raise ContractError("paired receipt directory contains no closed inventory")
    by_case = {str(payload["case_id"]): payload for payload in payloads}
    if set(by_case) != expected or len(by_case) != len(payloads):
        raise ContractError("paired receipt inventory is incomplete or duplicated")
    binding_fields = (
        "source_sha",
        "manifest",
        "candidate_plugin_sha256",
        "references",
    )
    first = payloads[0]
    if any(
        any(payload[field] != first[field] for field in binding_fields)
        for payload in payloads[1:]
    ):
        raise ContractError("paired receipts disagree on exact source/plugin/reference identity")
    evidence = [
        record
        for case in manifest["benchmark_cases"]
        for record in by_case[case["id"]]["evidence"]
    ]
    gate = evaluate_benchmark_gates(manifest, evidence)
    payload = {
        "schema_version": 1,
        "receipt_type": QUALIFICATION_RECEIPT_TYPE,
        "source_sha": first["source_sha"],
        "manifest": first["manifest"],
        "candidate_plugin_sha256": first["candidate_plugin_sha256"],
        "references": first["references"],
        "dataset_sha256": {
            case_id: by_case[case_id]["generated_dataset_sha256"]
            for case_id in sorted(by_case)
        },
        "benchmark_gate": gate,
        "evidence": evidence,
    }
    return write_sealed_receipt(output, payload)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--lock", type=Path, default=DEFAULT_LOCK)
    subparsers = parser.add_subparsers(dest="action", required=True)
    capture = subparsers.add_parser("capture-case")
    capture.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    capture.add_argument("--case-id", required=True)
    capture.add_argument("--reference-root", type=Path, required=True)
    capture.add_argument("--candidate-root", type=Path, required=True)
    capture.add_argument("--candidate-plugin", type=Path, required=True)
    capture.add_argument("--source-sha", required=True)
    capture.add_argument("--work-dir", type=Path, required=True)
    capture.add_argument("--receipt", type=Path, required=True)
    capture.add_argument("--prove-constrained", action="store_true")
    combine = subparsers.add_parser("combine")
    combine.add_argument("--receipt-dir", type=Path, required=True)
    combine.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.action == "capture-case":
        if not args.stata:
            raise SystemExit("set STATA_BIN or pass --stata")
        digest = capture_pair(
            stata=args.stata,
            case_id=args.case_id,
            reference_root=args.reference_root,
            candidate_root=args.candidate_root.resolve(),
            candidate_plugin=args.candidate_plugin.resolve(),
            source_sha=args.source_sha,
            work_dir=args.work_dir.resolve(),
            receipt=args.receipt.resolve(),
            manifest_path=args.manifest.resolve(),
            lock_path=args.lock.resolve(),
            prove_constrained=args.prove_constrained,
        )
        print(f"paired case receipt PASS: {args.case_id} sha256={digest}")
    else:
        digest = combine_receipts(
            args.receipt_dir.resolve(), args.output.resolve(), args.manifest.resolve()
        )
        print(f"paired qualification receipt written: sha256={digest}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
