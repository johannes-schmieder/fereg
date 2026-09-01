#!/usr/bin/env python3
"""Validate and evaluate the frozen PPML memory/performance evidence contract.

This module does not run Stata.  Platform runners must capture the closed JSON
schema defined here.  Evaluation deliberately completes every scientific gate
before it reads timing or memory measurements.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import statistics
import sys
from typing import Any, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json"
HEX_SHA1 = re.compile(r"^[0-9a-f]{40}$")
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")

TOP_LEVEL_FIELDS = {
    "schema_version",
    "manifest_id",
    "status",
    "qualification_eligible",
    "contracts",
    "dependencies",
    "generator_contract",
    "capture_contract",
    "dimensions",
    "cases",
    "science_gates",
    "performance_gates",
    "memory_gates",
    "instrumentation",
}
CASE_FIELDS = {
    "id",
    "profile",
    "scale",
    "design",
    "controls",
    "outcome",
    "finite_design",
    "weight",
    "vce",
    "threads",
    "memory",
    "output",
    "performance_role",
    "oracle_required",
    "scaling_group",
    "scaling_thread",
    "equivalence_group",
    "paired_fit_case",
}
EXPECTED_CASE_IDS = (
    "dev_n100k_no_fe_k0",
    "dev_n100k_one_fe_offset_fw_c1",
    "dev_n100k_lowmob_exposure_pw_c2",
    "dev_n100k_three_fe_separated_c3",
    "dev_n100k_slopes_collinear_c4",
    "dev_n100k_disconnected_illcond_c5",
    "scale_n100k_two_fe_fast",
    "scale_n1m_two_fe_fast",
    "route_n1m_two_fe_low",
    "route_n1m_two_fe_auto",
    "gov_no_fe_easy_k0",
    "gov_one_fe_offset_k10_fw_c1",
    "gov_two_fe_standard_k10",
    "gov_two_fe_lowmob_long_k50_pw_c2",
    "gov_disconnected_separated_k10_c3",
    "gov_three_fe_exposure_k10_fw_c4",
    "gov_slopes_illcond_k50_pw_c5",
    "gov_collinear_two_fe_k50",
    "save_prediction_two_fe",
    "save_total_fe_two_fe",
    "save_individual_fe_three_fe",
)
SCIENCE_RESULT_FIELDS = {
    "rc",
    "status",
    "sample_identity_sha256",
    "separation_ledger_sha256",
    "coefficient_names",
    "coefficient_status",
    "coefficients",
    "vce_shape",
    "vce",
    "dof",
    "objective",
    "outputs",
    "certificates",
}
ORACLE_BUNDLE_FIELDS = {
    "schema_version",
    "check_type",
    "cell_id",
    "fixture",
    "candidate",
    "comparator",
    "independent",
    "complete_sentinel",
}
ORACLE_FIXTURE_FIELDS = {
    "fixture_id",
    "data_sha256",
    "canonical_export_sha256",
    "model_sha256",
    "generator_sha256",
    "row_count",
}
OUTPUT_FIELDS = {
    "prediction_probe",
    "total_absorbed_effect_probe",
    "individual_fe_probe",
}
CERTIFICATE_FIELDS = {
    "converged",
    "two_consecutive_iterations",
    "strict_final_solve",
    "rank_frozen",
    "score_linf",
    "fe_score_linf",
    "vce_symmetry_linf",
    "reconstruction_linf",
}
TIMING_FIELDS = {"cold_seconds", "warm_seconds"}
EXECUTION_FIELDS = {
    "scheduler_job_id",
    "node_id",
    "plugin_sha256",
    "comparator_bundle_sha256",
    "os",
    "cpu",
    "stata_version",
    "stata_flavor",
    "rustc_version",
    "candidate_process_ids",
    "comparator_process_ids",
    "arm_order",
    "requested_threads",
    "candidate_effective_threads",
    "comparator_stata_processors",
    "comparator_processor_limit",
}
DIAGNOSTIC_FIELDS = {
    "requested_threads",
    "effective_threads",
    "requested_memory",
    "effective_route",
    "linear_route",
    "fallbacks",
    "outer_iterations",
    "inner_iterations",
    "step_halvings",
    "workspace_allocation_free",
    "whole_iteration_allocation_free",
    "phase_seconds",
}
MEMORY_FIELDS = {
    "modeled_peak_bytes",
    "native_peak_bytes",
    "phase_modeled_peak_bytes",
    "phase_observed_peak_bytes",
    "process_peak_rss_bytes",
    "repeated_call_retained_bytes",
    "measured_steady_state_iterations",
    "allocator_event_count_start",
    "allocator_event_count_end",
    "allocator_current_bytes_start",
    "allocator_current_bytes_end",
    "allocator_peak_bytes_start",
    "allocator_peak_bytes_end",
    "workspace_capacity_start",
    "workspace_capacity_end",
    "scratch_bytes_start",
    "scratch_bytes_end",
    "scratch_files_after",
    "linear_session_allocation_free",
    "fresh_warm_equivalent",
    "median_steady_iteration_seconds",
}
IDENTITY_FIELDS = {
    "source_commit",
    "source_tree",
    "cargo_lock_sha256",
    "references_lock_sha256",
    "manifest_sha256",
    "generator_sha256",
    "reference_commits",
}
CELL_EXECUTION_IDENTITY_FIELDS = {
    "scheduler_job_id",
    "node_id",
    "plugin_sha256",
    "comparator_bundle_sha256",
    "os",
    "cpu",
    "stata_version",
    "stata_flavor",
    "rustc_version",
}
EXPECTED_PHASES = (
    "preparation",
    "separation_fe",
    "separation_simplex",
    "separation_ir",
    "separation_mu",
    "weight_response_refresh",
    "fe_projection",
    "finite_solve",
    "objective_line_search",
    "strict_final_solve",
    "inference",
    "fe_recovery",
    "publication",
)
EXPECTED_COUNTERS = (
    "requested_threads",
    "effective_threads",
    "requested_memory",
    "effective_route",
    "linear_route",
    "fallbacks",
    "outer_iterations",
    "inner_iterations",
    "step_halvings",
    "workspace_allocation_free",
    "whole_iteration_allocation_free",
    "modeled_peak_bytes",
    "native_peak_bytes",
    "phase_modeled_peak_bytes",
    "phase_observed_peak_bytes",
    "measured_steady_state_iterations",
    "allocator_event_count_start",
    "allocator_event_count_end",
    "allocator_current_bytes_start",
    "allocator_current_bytes_end",
    "allocator_peak_bytes_start",
    "allocator_peak_bytes_end",
    "workspace_capacity_start",
    "workspace_capacity_end",
    "scratch_bytes_start",
    "scratch_bytes_end",
    "linear_session_allocation_free",
    "fresh_warm_equivalent",
    "process_peak_rss_bytes",
)


class ContractError(ValueError):
    """The manifest or captured benchmark evidence fails closed."""


def canonical_json_bytes(value: Any) -> bytes:
    try:
        encoded = json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
    except (TypeError, ValueError) as error:
        raise ContractError(f"value is not canonical finite JSON: {error}") from error
    return (encoded + "\n").encode("ascii")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1 << 20), b""):
                digest.update(block)
    except OSError as error:
        raise ContractError(f"unable to hash {path}: {error}") from error
    return digest.hexdigest()


def _mapping(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ContractError(f"{where} must be an object")
    return value


def _expect_keys(value: Mapping[str, Any], expected: set[str], where: str) -> None:
    if set(value) != expected:
        raise ContractError(
            f"{where} fields are not closed; "
            f"missing={sorted(expected - set(value))}, extra={sorted(set(value) - expected)}"
        )


def _integer(value: Any, where: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        raise ContractError(f"{where} must be an integer >= {minimum}")
    return value


def _number(value: Any, where: str, *, positive: bool = False) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{where} must be numeric")
    output = float(value)
    if not math.isfinite(output) or (positive and output <= 0.0):
        raise ContractError(f"{where} must be {'positive ' if positive else ''}finite")
    return output


def _sha1(value: Any, where: str) -> str:
    if not isinstance(value, str) or HEX_SHA1.fullmatch(value) is None:
        raise ContractError(f"{where} must be a lowercase 40-character git id")
    return value


def _sha256(value: Any, where: str) -> str:
    if not isinstance(value, str) or HEX_SHA256.fullmatch(value) is None:
        raise ContractError(f"{where} must be a lowercase SHA-256")
    return value


def _index(values: Any, where: str) -> dict[str, Mapping[str, Any]]:
    if not isinstance(values, list):
        raise ContractError(f"{where} inventory must be a list")
    output: dict[str, Mapping[str, Any]] = {}
    for index, raw in enumerate(values):
        value = _mapping(raw, f"{where}[{index}]")
        identifier = value.get("id")
        if not isinstance(identifier, str) or not identifier:
            raise ContractError(f"{where}[{index}].id must be nonempty")
        if identifier in output:
            raise ContractError(f"duplicate {where} id: {identifier}")
        output[identifier] = value
    return output


def load_manifest(path: Path = DEFAULT_MANIFEST) -> dict[str, Any]:
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"unable to read PPML performance manifest: {error}") from error
    validate_manifest(manifest)
    return manifest


def validate_manifest(manifest: Mapping[str, Any]) -> None:
    _expect_keys(manifest, TOP_LEVEL_FIELDS, "manifest")
    if manifest["schema_version"] != 1 or manifest["manifest_id"] != "ppml_memory_performance_v1":
        raise ContractError("PPML performance manifest identity is invalid")
    if manifest["status"] not in {"development_frozen", "qualification_frozen"}:
        raise ContractError("manifest status must be development_frozen or qualification_frozen")
    if not isinstance(manifest["qualification_eligible"], bool):
        raise ContractError("qualification_eligible must be boolean")
    if (manifest["status"] == "development_frozen") == manifest["qualification_eligible"]:
        raise ContractError("development evidence cannot be qualification eligible")

    dependencies = _mapping(manifest["dependencies"], "dependencies")
    _expect_keys(
        dependencies,
        {"ppmlhdfe", "reghdfe", "ftools", "require", "independent_oracle"},
        "dependencies",
    )
    expected_pins = {
        "ppmlhdfe": "b85665d8f674e93c1e07446a9a2b29be7f797910",
        "reghdfe": "29b2b203f534413369d675d0c0f674d61291f536",
        "ftools": "d6e24d66fd02ba1b37325c82c5040444c5b4c636",
        "require": "f4760b86e55cf73b82041ce59cf20e8e8224afb0",
    }
    for name, commit in expected_pins.items():
        record = _mapping(dependencies[name], f"dependencies.{name}")
        _expect_keys(record, {"commit"}, f"dependencies.{name}")
        if record["commit"] != commit:
            raise ContractError(f"{name} comparator pin is not frozen")

    capture = _mapping(manifest["capture_contract"], "capture_contract")
    if capture.get("cold_repetitions") != 1 or capture.get("minimum_warm_repetitions") != 5:
        raise ContractError("capture cadence must be one cold and at least five warm repetitions")
    if (
        capture.get("cold_pair_order") != "AB"
        or capture.get("warm_pair_order") != "alternating_AB_BA_starting_AB"
    ):
        raise ContractError(
            "capture pair order must be one cold AB pair followed by warm AB/BA alternation"
        )
    if not all(
        capture.get(field) is True
        for field in (
            "fresh_process_per_arm_repetition",
            "same_job_and_node_per_cell",
            "balanced_alternating_arm_order",
            "requested_application_threads_when_controllable",
            "science_before_timing",
        )
    ):
        raise ContractError("capture isolation and science-first rules must remain enabled")
    identity_inventory = capture.get("required_identity_fields")
    if (
        not isinstance(identity_inventory, list)
        or not all(isinstance(field, str) for field in identity_inventory)
        or len(identity_inventory) != len(set(identity_inventory))
        or set(identity_inventory) != IDENTITY_FIELDS - {"reference_commits"}
    ):
        raise ContractError("capture identity field inventory is incomplete")
    cell_identity_inventory = capture.get("required_cell_execution_identity_fields")
    if (
        not isinstance(cell_identity_inventory, list)
        or not all(isinstance(field, str) for field in cell_identity_inventory)
        or len(cell_identity_inventory) != len(set(cell_identity_inventory))
        or set(cell_identity_inventory) != CELL_EXECUTION_IDENTITY_FIELDS
    ):
        raise ContractError("capture cell execution identity field inventory is incomplete")

    dimensions = _mapping(manifest["dimensions"], "dimensions")
    _expect_keys(
        dimensions,
        {
            "scales",
            "control_counts",
            "designs",
            "outcomes",
            "finite_designs",
            "weights",
            "vces",
            "threads",
            "memory_modes",
            "outputs",
        },
        "dimensions",
    )
    scales = _index(dimensions["scales"], "scale")
    designs = _index(dimensions["designs"], "design")
    outcomes = _index(dimensions["outcomes"], "outcome")
    if {key: value.get("n") for key, value in scales.items()} != {
        "n100k": 100_000,
        "n1m": 1_000_000,
        "n10m": 10_000_000,
    }:
        raise ContractError("scale inventory must freeze 100k, 1m, and 10m")
    if {value.get("fe_dimensions") for value in designs.values()} != {0, 1, 2, 3}:
        raise ContractError("designs must cover zero through three FE dimensions")
    if not any(value.get("slope_channels", 0) > 0 for value in designs.values()):
        raise ContractError("designs must include absorbed slopes")
    if {value.get("connectivity") for value in designs.values()} != {
        "none", "standard", "low_mobility", "disconnected"
    }:
        raise ContractError("designs must cover mobility and disconnected components")
    if {value.get("adjustment") for value in outcomes.values()} != {"none", "offset", "exposure"}:
        raise ContractError("outcomes must cover offset and exposure separately")
    if {value.get("irls_path") for value in outcomes.values()} != {"easy", "long"}:
        raise ContractError("outcomes must cover easy and long IRLS paths")
    if not any(value.get("separated_share", 0.0) > 0.0 for value in outcomes.values()):
        raise ContractError("outcomes must cover separation")
    frozen_dimensions = {
        "control_counts": [0, 10, 50],
        "finite_designs": ["full_rank", "collinear", "ill_conditioned"],
        "weights": ["none", "fweight", "pweight"],
        "vces": ["robust", "cluster1", "cluster2", "cluster3", "cluster4", "cluster5"],
        "threads": [1, 2, 4, 8, 16],
        "memory_modes": ["fast", "low", "auto"],
        "outputs": ["fit", "prediction", "total_absorbed_effect", "individual_fe"],
    }
    for field, expected in frozen_dimensions.items():
        if dimensions[field] != expected:
            raise ContractError(f"{field} inventory is not frozen")

    cases = _index(manifest["cases"], "case")
    if tuple(cases) != EXPECTED_CASE_IDS:
        raise ContractError("PPML benchmark case inventory or order changed")
    controls = set(dimensions["control_counts"])
    finite_designs = set(dimensions["finite_designs"])
    weights = set(dimensions["weights"])
    vces = set(dimensions["vces"])
    threads = set(dimensions["threads"])
    memories = set(dimensions["memory_modes"])
    outputs = set(dimensions["outputs"])
    for identifier, case in cases.items():
        _expect_keys(case, CASE_FIELDS, f"case {identifier}")
        if case["scale"] not in scales or case["design"] not in designs or case["outcome"] not in outcomes:
            raise ContractError(f"case {identifier} references an unknown catalog value")
        for field, allowed in (
            ("controls", controls),
            ("finite_design", finite_designs),
            ("weight", weights),
            ("vce", vces),
            ("memory", memories),
            ("output", outputs),
        ):
            if case[field] not in allowed:
                raise ContractError(f"case {identifier} has unknown {field}")
        if (
            not isinstance(case["threads"], list)
            or not case["threads"]
            or len(case["threads"]) != len(set(case["threads"]))
            or not set(case["threads"]) <= threads
        ):
            raise ContractError(f"case {identifier} has invalid threads")
        if case["profile"] not in {"development", "scc_governing"}:
            raise ContractError(f"case {identifier} has invalid profile")
        if case["performance_role"] not in {"diagnostic", "primary", "saved_output"}:
            raise ContractError(f"case {identifier} has invalid performance role")
        if not isinstance(case["oracle_required"], bool):
            raise ContractError(f"case {identifier}.oracle_required must be boolean")
        if case["performance_role"] == "primary" and (
            case["profile"] != "scc_governing"
            or case["scale"] != "n10m"
            or case["output"] != "fit"
        ):
            raise ContractError("primary performance cells must be governing 10m fit cells")
        if (case["scaling_group"] is None) != (case["scaling_thread"] is None):
            raise ContractError(f"case {identifier} has incomplete scaling registration")
        if case["scaling_thread"] is not None and case["scaling_thread"] not in case["threads"]:
            raise ContractError(f"case {identifier} scaling thread is not measured")
        pair = case["paired_fit_case"]
        if case["performance_role"] == "saved_output":
            if not isinstance(pair, str) or pair not in cases or case["output"] == "fit":
                raise ContractError(f"case {identifier} has invalid saved-output pair")
        elif pair is not None:
            raise ContractError(f"case {identifier} unexpectedly names a fit pair")

    expanded = expand_cells(manifest)
    if len(expanded) != 33:
        raise ContractError("frozen PPML matrix must expand to 33 cells")
    for field, allowed in (
        ("scale", set(scales)),
        ("design", set(designs)),
        ("controls", controls),
        ("outcome", set(outcomes)),
        ("finite_design", finite_designs),
        ("weight", weights),
        ("vce", vces),
        ("requested_threads", threads),
        ("memory", memories),
        ("output", outputs),
    ):
        observed = {cell[field] for cell in expanded}
        if observed != allowed:
            raise ContractError(f"expanded matrix does not cover every {field}: {sorted(allowed - observed)}")

    primary = [cell for cell in expanded if cell["performance_role"] == "primary"]
    if len(primary) != 20 or {cell["requested_threads"] for cell in primary} != threads:
        raise ContractError("governing population must contain 20 cells and all thread counts")
    small_oracles = [cell for cell in expanded if cell["oracle_required"]]
    if not small_oracles or any(cell["scale"] != "n100k" for cell in small_oracles):
        raise ContractError("independent oracles must be registered on bounded 100k cells")

    performance = _mapping(manifest["performance_gates"], "performance_gates")
    required_performance = {
        "governing_profile": "scc_governing",
        "governing_scale": "n10m",
        "maximum_equal_cell_geometric_mean_ratio": 0.40,
        "maximum_twice_as_fast_ratio": 0.50,
        "minimum_twice_as_fast_share": 0.80,
        "maximum_registered_single_thread_ratio": 2.0 / 3.0,
        "maximum_unexplained_cell_ratio": 1.25,
        "saved_output_maximum_seconds": 0.05,
        "saved_output_maximum_fit_fraction": 0.05,
        "maximum_normalized_iteration_cost_growth": 1.25,
    }
    if dict(performance) != required_performance:
        raise ContractError("ADR 0018 performance gates are not frozen")
    memory = _mapping(manifest["memory_gates"], "memory_gates")
    _expect_keys(
        memory,
        {
            "maximum_modeled_allocation_relative_error",
            "maximum_repeated_call_retained_bytes",
            "whole_iteration_allocation_free_qualified_linear_routes",
            "minimum_measured_steady_state_iterations",
            "maximum_allocator_event_growth",
            "maximum_allocator_current_growth_bytes",
            "maximum_allocator_peak_growth_bytes",
            "require_stable_workspace_capacity",
            "require_stable_scratch_bytes",
            "require_linear_session_allocation_free",
            "require_fresh_warm_equivalent",
            "require_zero_scratch_files_after",
            "constrained_preflight_maximum_allocation_bytes",
            "fixtures",
        },
        "memory_gates",
    )
    if (
        memory.get("maximum_modeled_allocation_relative_error") != 0.25
        or memory.get("maximum_repeated_call_retained_bytes") != 0
        or memory.get("whole_iteration_allocation_free_qualified_linear_routes") != ["map"]
        or memory.get("minimum_measured_steady_state_iterations") != 16
        or memory.get("maximum_allocator_event_growth") != 0
        or memory.get("maximum_allocator_current_growth_bytes") != 0
        or memory.get("maximum_allocator_peak_growth_bytes") != 0
        or memory.get("require_stable_workspace_capacity") is not True
        or memory.get("require_stable_scratch_bytes") is not True
        or memory.get("require_linear_session_allocation_free") is not True
        or memory.get("require_fresh_warm_equivalent") is not True
        or memory.get("require_zero_scratch_files_after") is not True
    ):
        raise ContractError("ADR 0018 memory gates are not frozen")
    fixtures = _index(memory.get("fixtures"), "memory fixture")
    if set(fixtures) != {"low_succeeds_fast_fails", "no_route_fits"}:
        raise ContractError("constrained-memory fixture inventory is incomplete")
    instrumentation = _mapping(manifest["instrumentation"], "instrumentation")
    _expect_keys(instrumentation, {"phases", "required_counters"}, "instrumentation")
    if instrumentation["phases"] != list(EXPECTED_PHASES):
        raise ContractError("PPML instrumentation phase order is not frozen")
    if instrumentation["required_counters"] != list(EXPECTED_COUNTERS):
        raise ContractError("PPML instrumentation counter order is not frozen")


def expand_cells(manifest: Mapping[str, Any], profile: str | None = None) -> list[dict[str, Any]]:
    output: list[dict[str, Any]] = []
    scales = {item["id"]: item for item in manifest["dimensions"]["scales"]}
    designs = {item["id"]: item for item in manifest["dimensions"]["designs"]}
    outcomes = {item["id"]: item for item in manifest["dimensions"]["outcomes"]}
    for case in manifest["cases"]:
        if profile is not None and case["profile"] != profile:
            continue
        for requested_threads in case["threads"]:
            output.append(
                {
                    **case,
                    "cell_id": f"{case['id']}_t{requested_threads}",
                    "requested_threads": requested_threads,
                    "scale_spec": dict(scales[case["scale"]]),
                    "design_spec": dict(designs[case["design"]]),
                    "outcome_spec": dict(outcomes[case["outcome"]]),
                }
            )
    return output


def _validate_identity(
    manifest: Mapping[str, Any], evidence: Mapping[str, Any], manifest_sha256: str
) -> None:
    identity = _mapping(evidence["identity"], "identity")
    _expect_keys(identity, IDENTITY_FIELDS, "identity")
    _sha1(identity["source_commit"], "identity.source_commit")
    _sha1(identity["source_tree"], "identity.source_tree")
    for field in (
        "cargo_lock_sha256",
        "references_lock_sha256",
        "manifest_sha256",
        "generator_sha256",
    ):
        _sha256(identity[field], f"identity.{field}")
    if identity["manifest_sha256"] != manifest_sha256:
        raise ContractError("evidence identity names a different manifest digest")
    pins = {
        name: record["commit"]
        for name, record in manifest["dependencies"].items()
        if isinstance(record, Mapping)
    }
    if identity["reference_commits"] != pins:
        raise ContractError("captured comparator closure does not match frozen commits")


def _validate_science_result(value: Any, where: str) -> Mapping[str, Any]:
    result = _mapping(value, where)
    _expect_keys(result, SCIENCE_RESULT_FIELDS, where)
    _integer(result["rc"], f"{where}.rc")
    if result["status"] not in {"success", "failure"}:
        raise ContractError(f"{where}.status is invalid")
    _sha256(result["sample_identity_sha256"], f"{where}.sample_identity_sha256")
    _sha256(result["separation_ledger_sha256"], f"{where}.separation_ledger_sha256")
    names = result["coefficient_names"]
    statuses = result["coefficient_status"]
    coefficients = result["coefficients"]
    if not isinstance(names, list) or not all(isinstance(item, str) and item for item in names):
        raise ContractError(f"{where}.coefficient_names is invalid")
    if not isinstance(statuses, list) or not all(isinstance(item, str) and item for item in statuses):
        raise ContractError(f"{where}.coefficient_status is invalid")
    if not isinstance(coefficients, list) or not (len(names) == len(statuses) == len(coefficients)):
        raise ContractError(f"{where} coefficient arrays are inconsistent")
    for index, value in enumerate(coefficients):
        _number(value, f"{where}.coefficients[{index}]")
    shape = result["vce_shape"]
    if shape != [len(names), len(names)] or len(result["vce"]) != len(names) ** 2:
        raise ContractError(f"{where}.vce shape is inconsistent")
    for index, value in enumerate(result["vce"]):
        _number(value, f"{where}.vce[{index}]")
    dof = _mapping(result["dof"], f"{where}.dof")
    _expect_keys(dof, {"N_physical", "N_effective", "finite_rank", "absorbed_rank", "df", "cluster_counts"}, f"{where}.dof")
    for field in ("N_physical", "N_effective", "finite_rank", "absorbed_rank", "df"):
        _number(dof[field], f"{where}.dof.{field}")
    if not isinstance(dof["cluster_counts"], list):
        raise ContractError(f"{where}.dof.cluster_counts must be a list")
    for index, value in enumerate(dof["cluster_counts"]):
        _integer(value, f"{where}.dof.cluster_counts[{index}]", minimum=2)
    objective = _mapping(result["objective"], f"{where}.objective")
    _expect_keys(objective, {"deviance", "log_likelihood", "null_log_likelihood", "pseudo_r2"}, f"{where}.objective")
    for field, value in objective.items():
        _number(value, f"{where}.objective.{field}")
    outputs = _mapping(result["outputs"], f"{where}.outputs")
    _expect_keys(outputs, OUTPUT_FIELDS, f"{where}.outputs")
    for field, values in outputs.items():
        if values is not None:
            if not isinstance(values, list) or not values:
                raise ContractError(f"{where}.outputs.{field} must be null or a nonempty list")
            for index, value in enumerate(values):
                _number(value, f"{where}.outputs.{field}[{index}]")
    certificates = _mapping(result["certificates"], f"{where}.certificates")
    _expect_keys(certificates, CERTIFICATE_FIELDS, f"{where}.certificates")
    for field in ("converged", "two_consecutive_iterations", "strict_final_solve", "rank_frozen"):
        if not isinstance(certificates[field], bool):
            raise ContractError(f"{where}.certificates.{field} must be boolean")
    for field in CERTIFICATE_FIELDS - {"converged", "two_consecutive_iterations", "strict_final_solve", "rank_frozen"}:
        _number(certificates[field], f"{where}.certificates.{field}")
    return result


def _compare_registered_oracle(
    manifest: Mapping[str, Any],
    plan: Mapping[str, Any],
    full_candidate: Mapping[str, Any],
    value: Any,
    where: str,
) -> list[str]:
    """Validate either the legacy same-cell oracle or a closed small fixture bundle."""
    oracle = _mapping(value, where)
    if set(oracle) == SCIENCE_RESULT_FIELDS:
        legacy = _validate_science_result(oracle, where)
        return [f"oracle:{failure}" for failure in compare_science(manifest, full_candidate, legacy)]

    _expect_keys(oracle, ORACLE_BUNDLE_FIELDS, where)
    if (
        oracle["schema_version"] != 1
        or oracle["check_type"] != "ppml-explicit-dummy-oracle-v1"
        or oracle["cell_id"] != plan["cell_id"]
        or oracle["complete_sentinel"] != "FEPPML_ORACLE_CHECK_COMPLETE"
    ):
        raise ContractError(f"{where} bundle identity is invalid")
    fixture = _mapping(oracle["fixture"], f"{where}.fixture")
    _expect_keys(fixture, ORACLE_FIXTURE_FIELDS, f"{where}.fixture")
    if not isinstance(fixture["fixture_id"], str) or not fixture["fixture_id"]:
        raise ContractError(f"{where}.fixture.fixture_id must be nonempty")
    for field in (
        "data_sha256",
        "canonical_export_sha256",
        "model_sha256",
        "generator_sha256",
    ):
        _sha256(fixture[field], f"{where}.fixture.{field}")
    _integer(fixture["row_count"], f"{where}.fixture.row_count", minimum=1)
    bundle_candidate = _validate_science_result(
        oracle["candidate"], f"{where}.candidate"
    )
    bundle_comparator = _validate_science_result(
        oracle["comparator"], f"{where}.comparator"
    )
    independent = _validate_science_result(
        oracle["independent"], f"{where}.independent"
    )
    failures = [
        f"oracle_bundle:comparator:{failure}"
        for failure in compare_science(manifest, bundle_candidate, bundle_comparator)
    ]
    failures.extend(
        f"oracle_bundle:independent:{failure}"
        for failure in compare_science(manifest, bundle_candidate, independent)
    )
    failures.extend(
        f"oracle_bundle:{failure}"
        for failure in _candidate_certificate_failures(manifest, bundle_candidate)
    )
    expected_physical = int(fixture["row_count"])
    expected_effective = expected_physical
    if plan["weight"] == "fweight":
        cycles, remainder = divmod(expected_physical, 3)
        expected_effective = cycles * 6 + sum(
            1 + observation % 3 for observation in range(1, remainder + 1)
        )
    for arm, result in (
        ("candidate", bundle_candidate),
        ("comparator", bundle_comparator),
        ("independent", independent),
    ):
        if result["dof"]["N_physical"] != expected_physical:
            failures.append(f"oracle_bundle:{arm}:dof.N_physical")
        if result["dof"]["N_effective"] != expected_effective:
            failures.append(f"oracle_bundle:{arm}:dof.N_effective")
    return failures


def _close(left: float, right: float, atol: float, rtol: float) -> bool:
    return abs(left - right) <= atol + rtol * max(abs(left), abs(right))


def _compare_vector(
    left: Sequence[Any], right: Sequence[Any], atol: float, rtol: float
) -> bool:
    return len(left) == len(right) and all(
        _close(float(a), float(b), atol, rtol) for a, b in zip(left, right)
    )


def compare_science(
    manifest: Mapping[str, Any],
    candidate: Mapping[str, Any],
    reference: Mapping[str, Any],
    *,
    compare_outputs: bool = True,
) -> list[str]:
    failures: list[str] = []
    for field in (
        "rc",
        "status",
        "sample_identity_sha256",
        "separation_ledger_sha256",
        "coefficient_names",
        "coefficient_status",
        "vce_shape",
        "dof",
    ):
        if candidate[field] != reference[field]:
            failures.append(field)
    gates = manifest["science_gates"]
    if not _compare_vector(
        candidate["coefficients"], reference["coefficients"],
        gates["coefficient_atol"], gates["coefficient_rtol"],
    ):
        failures.append("coefficients")
    if not _compare_vector(
        candidate["vce"], reference["vce"], gates["vce_atol"], gates["vce_rtol"]
    ):
        failures.append("vce")
    for field in ("deviance", "log_likelihood", "null_log_likelihood", "pseudo_r2"):
        if not _close(
            float(candidate["objective"][field]),
            float(reference["objective"][field]),
            gates["objective_atol"],
            gates["objective_rtol"],
        ):
            failures.append(f"objective.{field}")
    if compare_outputs:
        for field in OUTPUT_FIELDS:
            left = candidate["outputs"][field]
            right = reference["outputs"][field]
            if (left is None) != (right is None) or (
                left is not None
                and not _compare_vector(
                    left, right, gates["output_probe_atol"], gates["output_probe_rtol"]
                )
            ):
                failures.append(f"outputs.{field}")
    return failures


def _expected_sample_sizes(cell: Mapping[str, Any]) -> tuple[int, int]:
    n = int(cell["scale_spec"]["n"])
    physical = n
    if cell["weight"] == "fweight":
        cycles, remainder = divmod(n, 3)
        effective = cycles * 6 + sum(
            1 + observation % 3 for observation in range(1, remainder + 1)
        )
    else:
        effective = physical
    return physical, effective


def _sample_size_failures(cell: Mapping[str, Any], result: Mapping[str, Any]) -> list[str]:
    physical, effective = _expected_sample_sizes(cell)
    failures = []
    if result["dof"]["N_physical"] != physical:
        failures.append("dof.N_physical")
    if result["dof"]["N_effective"] != effective:
        failures.append("dof.N_effective")
    return failures


def _candidate_certificate_failures(
    manifest: Mapping[str, Any], result: Mapping[str, Any]
) -> list[str]:
    cert = result["certificates"]
    gates = manifest["science_gates"]
    failures = []
    for field in ("converged", "two_consecutive_iterations", "strict_final_solve", "rank_frozen"):
        if cert[field] is not True:
            failures.append(f"certificate.{field}")
    for field, gate in (
        ("score_linf", "maximum_score_linf"),
        ("fe_score_linf", "maximum_fe_score_linf"),
        ("vce_symmetry_linf", "maximum_vce_symmetry_linf"),
        ("reconstruction_linf", "maximum_reconstruction_linf"),
    ):
        if cert[field] > gates[gate]:
            failures.append(f"certificate.{field}")
    return failures


def _validate_timing(value: Any, where: str, minimum_warm: int) -> Mapping[str, Any]:
    timing = _mapping(value, where)
    _expect_keys(timing, TIMING_FIELDS, where)
    _number(timing["cold_seconds"], f"{where}.cold_seconds", positive=True)
    warm = timing["warm_seconds"]
    if not isinstance(warm, list) or len(warm) < minimum_warm:
        raise ContractError(f"{where} must contain at least {minimum_warm} warm repetitions")
    for index, value in enumerate(warm):
        _number(value, f"{where}.warm_seconds[{index}]", positive=True)
    return timing


def _validate_execution(
    value: Any,
    where: str,
    requested_threads: int,
    candidate_repetitions: int,
    comparator_repetitions: int,
    capture_contract: Mapping[str, Any],
) -> None:
    execution = _mapping(value, where)
    _expect_keys(execution, EXECUTION_FIELDS, where)
    if execution["requested_threads"] != requested_threads or execution["candidate_effective_threads"] != requested_threads:
        raise ContractError(f"{where} did not honor requested native threads")
    for field in ("scheduler_job_id", "node_id"):
        if not isinstance(execution[field], str) or not execution[field]:
            raise ContractError(f"{where}.{field} must be nonempty")
    for field in ("plugin_sha256", "comparator_bundle_sha256"):
        _sha256(execution[field], f"{where}.{field}")
    for field in ("os", "cpu", "stata_version", "stata_flavor", "rustc_version"):
        if not isinstance(execution[field], str) or not execution[field]:
            raise ContractError(f"{where}.{field} must be nonempty")
    candidate_pids = execution["candidate_process_ids"]
    comparator_pids = execution["comparator_process_ids"]
    if (
        not isinstance(candidate_pids, list)
        or not isinstance(comparator_pids, list)
        or len(candidate_pids) != candidate_repetitions
        or len(comparator_pids) != comparator_repetitions
        or len(set(candidate_pids)) != len(candidate_pids)
        or len(set(comparator_pids)) != len(comparator_pids)
        or set(candidate_pids) & set(comparator_pids)
    ):
        raise ContractError(f"{where} does not use fresh isolated processes")
    order = execution["arm_order"]
    if (
        capture_contract.get("cold_pair_order") != "AB"
        or capture_contract.get("warm_pair_order")
        != "alternating_AB_BA_starting_AB"
    ):
        raise ContractError(f"{where} received an unsupported manifest pair cadence")
    expected_pairs = [["candidate", "comparator"]]
    expected_pairs.extend(
        ["candidate", "comparator"]
        if warm_index % 2 == 0
        else ["comparator", "candidate"]
        for warm_index in range(candidate_repetitions - 1)
    )
    if (
        not isinstance(order, list)
        or candidate_repetitions != comparator_repetitions
        or order != expected_pairs
    ):
        raise ContractError(
            f"{where} arm order must be cold AB then ordered warm AB/BA pairs"
        )
    limit = _integer(execution["comparator_processor_limit"], f"{where}.comparator_processor_limit", minimum=1)
    observed = _integer(execution["comparator_stata_processors"], f"{where}.comparator_stata_processors", minimum=1)
    if observed != min(requested_threads, limit):
        raise ContractError(f"{where} comparator processor control is inconsistent")


def _validate_diagnostics(
    manifest: Mapping[str, Any], value: Any, cell: Mapping[str, Any], where: str
) -> Mapping[str, Any]:
    diagnostics = _mapping(value, where)
    _expect_keys(diagnostics, DIAGNOSTIC_FIELDS, where)
    if diagnostics["requested_threads"] != cell["requested_threads"] or diagnostics["effective_threads"] != cell["requested_threads"]:
        raise ContractError(f"{where} thread diagnostics contradict the cell")
    if diagnostics["requested_memory"] != cell["memory"]:
        raise ContractError(f"{where} memory diagnostics contradict the cell")
    if diagnostics["effective_route"] not in {"fast", "low"}:
        raise ContractError(f"{where}.effective_route is invalid")
    if cell["memory"] in {"fast", "low"} and diagnostics["effective_route"] != cell["memory"]:
        raise ContractError(f"{where} explicit memory route was not honored")
    if not isinstance(diagnostics["fallbacks"], list) or not all(
        isinstance(value, str) and value for value in diagnostics["fallbacks"]
    ):
        raise ContractError(f"{where}.fallbacks must be a string list")
    if diagnostics["linear_route"] not in {
        "map",
        "lsmr",
        "cmg",
        "cg",
        "irons_tuck",
        "squarem",
    }:
        raise ContractError(f"{where}.linear_route is invalid")
    for field in ("workspace_allocation_free", "whole_iteration_allocation_free"):
        if not isinstance(diagnostics[field], bool):
            raise ContractError(f"{where}.{field} must be boolean")
    for field in ("outer_iterations", "inner_iterations", "step_halvings"):
        _integer(diagnostics[field], f"{where}.{field}")
    phases = _mapping(diagnostics["phase_seconds"], f"{where}.phase_seconds")
    expected_phases = set(EXPECTED_PHASES)
    _expect_keys(phases, expected_phases, f"{where}.phase_seconds")
    for field, value in phases.items():
        _number(value, f"{where}.phase_seconds.{field}")
    return diagnostics


def _validate_memory(value: Any, where: str) -> Mapping[str, Any]:
    memory = _mapping(value, where)
    _expect_keys(memory, MEMORY_FIELDS, where)
    integer_fields = MEMORY_FIELDS - {
        "median_steady_iteration_seconds",
        "phase_modeled_peak_bytes",
        "phase_observed_peak_bytes",
        "linear_session_allocation_free",
        "fresh_warm_equivalent",
    }
    for field in integer_fields:
        _integer(memory[field], f"{where}.{field}")
    for field in ("linear_session_allocation_free", "fresh_warm_equivalent"):
        if not isinstance(memory[field], bool):
            raise ContractError(f"{where}.{field} must be boolean")
    modeled = _mapping(memory["phase_modeled_peak_bytes"], f"{where}.phase_modeled_peak_bytes")
    observed = _mapping(memory["phase_observed_peak_bytes"], f"{where}.phase_observed_peak_bytes")
    _expect_keys(modeled, set(EXPECTED_PHASES), f"{where}.phase_modeled_peak_bytes")
    _expect_keys(observed, set(EXPECTED_PHASES), f"{where}.phase_observed_peak_bytes")
    for phase in EXPECTED_PHASES:
        _integer(modeled[phase], f"{where}.phase_modeled_peak_bytes.{phase}")
        if observed[phase] is not None:
            _integer(observed[phase], f"{where}.phase_observed_peak_bytes.{phase}")
    _number(memory["median_steady_iteration_seconds"], f"{where}.median_steady_iteration_seconds", positive=True)
    return memory


def _validate_explanation(value: Any, where: str) -> bool:
    if value is None:
        return False
    explanation = _mapping(value, where)
    _expect_keys(explanation, {"ticket", "classification", "rationale", "evidence_sha256", "accepted_by"}, where)
    for field in ("ticket", "classification", "rationale", "accepted_by"):
        if not isinstance(explanation[field], str) or not explanation[field]:
            raise ContractError(f"{where}.{field} must be nonempty")
    _sha256(explanation["evidence_sha256"], f"{where}.evidence_sha256")
    return True


def _validate_constrained(
    manifest: Mapping[str, Any], records: Any, *, embedded_git_commit: str
) -> list[str]:
    indexed = _index(records, "constrained-memory evidence")
    expected = {item["id"]: item for item in manifest["memory_gates"]["fixtures"]}
    if set(indexed) != set(expected):
        raise ContractError("constrained-memory evidence inventory is incomplete")
    reasons: list[str] = []
    ceiling = manifest["memory_gates"]["constrained_preflight_maximum_allocation_bytes"]
    for fixture_id, fixture in expected.items():
        record = indexed[fixture_id]
        _expect_keys(record, {"id", "attempts"}, f"constrained-memory evidence {fixture_id}")
        attempts = _index(record["attempts"], f"{fixture_id} attempt")
        if set(attempts) != set(fixture["required_attempts"]):
            raise ContractError(f"{fixture_id} attempt inventory is incomplete")
        for mode, attempt in attempts.items():
            _expect_keys(
                attempt,
                {
                    "id", "status", "rc", "preflight_receipt_sha256",
                    "preflight_payload_sha256", "preflight",
                },
                f"{fixture_id}.{mode}",
            )
            _integer(attempt["rc"], f"{fixture_id}.{mode}.rc")
            if attempt["status"] not in {"success", "failure"} or (
                attempt["status"] == "success"
            ) != (attempt["rc"] == 0):
                raise ContractError(f"{fixture_id}.{mode} command status is invalid")
            _sha256(
                attempt["preflight_receipt_sha256"],
                f"{fixture_id}.{mode}.preflight_receipt_sha256",
            )
            payload_sha = _sha256(
                attempt["preflight_payload_sha256"],
                f"{fixture_id}.{mode}.preflight_payload_sha256",
            )
            preflight = _mapping(
                attempt["preflight"], f"{fixture_id}.{mode}.preflight"
            )
            _expect_keys(
                preflight,
                {
                    "schema_version", "receipt_type", "requested", "decision",
                    "selected_route", "fast_peak_bytes", "low_peak_bytes",
                    "available_bytes", "embedding_resident_bytes",
                    "safety_headroom_bytes", "usable_bytes",
                    "planner_allocator_event_growth",
                    "planner_allocator_current_growth_bytes",
                    "planner_allocator_peak_growth_bytes", "allocation_bytes",
                    "workspace_allocation_started", "embedded_git_commit",
                    "complete_sentinel",
                },
                f"{fixture_id}.{mode}.preflight",
            )
            if payload_sha != hashlib.sha256(canonical_json_bytes(preflight)).hexdigest():
                raise ContractError(
                    f"{fixture_id}.{mode} preflight payload seal mismatch"
                )
            if (
                preflight["schema_version"] != 1
                or preflight["receipt_type"] != "feppml-memory-preflight-v1"
                or preflight["requested"] != mode
                or preflight["decision"] not in {"selected", "rejected"}
                or preflight["selected_route"] not in {None, "fast", "low"}
                or preflight["workspace_allocation_started"] is not False
                or preflight["embedded_git_commit"] != embedded_git_commit
                or preflight["complete_sentinel"]
                    != "FEPPML_MEMORY_PREFLIGHT_COMPLETE"
            ):
                raise ContractError(f"{fixture_id}.{mode} preflight identity is invalid")
            for field in (
                "fast_peak_bytes", "low_peak_bytes", "available_bytes",
                "embedding_resident_bytes", "safety_headroom_bytes", "usable_bytes",
                "planner_allocator_event_growth",
                "planner_allocator_peak_growth_bytes", "allocation_bytes",
            ):
                _integer(preflight[field], f"{fixture_id}.{mode}.preflight.{field}")
            current_growth = preflight["planner_allocator_current_growth_bytes"]
            if isinstance(current_growth, bool) or not isinstance(current_growth, int):
                raise ContractError(
                    f"{fixture_id}.{mode} planner current growth is invalid"
                )
            if (
                preflight["fast_peak_bytes"] <= 0
                or preflight["low_peak_bytes"] <= 0
                or preflight["usable_bytes"]
                    != preflight["available_bytes"]
                    - preflight["safety_headroom_bytes"]
                or preflight["safety_headroom_bytes"]
                    != max(512 * 1024 * 1024, preflight["available_bytes"] // 5)
                or preflight["allocation_bytes"]
                    != max(
                        abs(current_growth),
                        preflight["planner_allocator_peak_growth_bytes"],
                    )
                or (preflight["decision"] == "selected")
                    != (preflight["selected_route"] is not None)
            ):
                raise ContractError(
                    f"{fixture_id}.{mode} preflight decision arithmetic is invalid"
                )
        if fixture_id == "low_succeeds_fast_fails":
            low, fast = attempts["low"], attempts["fast"]
            low_preflight = low["preflight"]
            fast_preflight = fast["preflight"]
            if (
                (low["status"], low["rc"]) != ("success", 0)
                or (low_preflight["decision"], low_preflight["selected_route"])
                    != ("selected", "low")
                or low_preflight["low_peak_bytes"] > low_preflight["usable_bytes"]
            ):
                reasons.append("constrained_memory:low_did_not_succeed")
            if (
                (fast["status"], fast["rc"]) != ("failure", 909)
                or (fast_preflight["decision"], fast_preflight["selected_route"])
                    != ("rejected", None)
                or fast_preflight["available_bytes"] != low_preflight["available_bytes"]
                or fast_preflight["fast_peak_bytes"] <= fast_preflight["usable_bytes"]
                or fast_preflight["allocation_bytes"] > ceiling
            ):
                reasons.append("constrained_memory:fast_did_not_fail_before_large_allocation")
        else:
            auto = attempts["auto"]
            auto_preflight = auto["preflight"]
            if (
                (auto["status"], auto["rc"]) != ("failure", 909)
                or (auto_preflight["decision"], auto_preflight["selected_route"])
                    != ("rejected", None)
                or auto_preflight["fast_peak_bytes"] <= auto_preflight["usable_bytes"]
                or auto_preflight["low_peak_bytes"] <= auto_preflight["usable_bytes"]
                or auto_preflight["allocation_bytes"] > ceiling
            ):
                reasons.append("constrained_memory:no_route_did_not_fail_before_large_allocation")
    return reasons


def evaluate_science_evidence(
    manifest: Mapping[str, Any],
    evidence: Mapping[str, Any],
    *,
    manifest_sha256: str | None = None,
) -> list[str]:
    """Validate closed scientific/memory evidence without any timing cadence gate.

    This entry point is for qualification preflight and shard assembly. It
    intentionally never interprets timing values, process IDs, scheduler
    placement, arm order, or constrained-memory cadence; the full evaluator
    remains the sole performance authority.
    """
    validate_manifest(manifest)
    manifest_sha256 = manifest_sha256 or hashlib.sha256(
        canonical_json_bytes(manifest)
    ).hexdigest()
    evidence = _mapping(evidence, "evidence")
    required_top = {"schema_version", "evidence_type", "manifest", "identity", "cells"}
    allowed_top = required_top | {"constrained_memory"}
    if not required_top.issubset(evidence) or not set(evidence).issubset(allowed_top):
        raise ContractError(
            "science evidence fields are not closed; "
            f"missing={sorted(required_top - set(evidence))}, "
            f"extra={sorted(set(evidence) - allowed_top)}"
        )
    if (
        evidence["schema_version"] != 1
        or evidence["evidence_type"] != "ppml-memory-performance-v1-evidence"
    ):
        raise ContractError("PPML science evidence identity is invalid")
    manifest_record = _mapping(evidence["manifest"], "evidence.manifest")
    _expect_keys(manifest_record, {"id", "sha256"}, "evidence.manifest")
    if manifest_record != {"id": manifest["manifest_id"], "sha256": manifest_sha256}:
        raise ContractError("science evidence names a different manifest")
    _validate_identity(manifest, evidence, manifest_sha256)

    expected_cells = expand_cells(manifest)
    expected_ids = [cell["cell_id"] for cell in expected_cells]
    raw_cells = evidence["cells"]
    if not isinstance(raw_cells, list):
        raise ContractError("evidence.cells must be a list")
    observed_ids = [
        value.get("cell_id") if isinstance(value, Mapping) else None for value in raw_cells
    ]
    if observed_ids != expected_ids:
        raise ContractError("science evidence cell inventory or order does not match the frozen plan")

    required_cell = {"cell_id", "data_sha256", "scientific"}
    allowed_cell = required_cell | {
        "timing",
        "execution",
        "diagnostics",
        "memory",
        "slowdown_explanation",
    }
    reasons: list[str] = []
    records: dict[str, Mapping[str, Any]] = {}
    candidates: dict[str, Mapping[str, Any]] = {}
    for plan, raw in zip(expected_cells, raw_cells):
        cell_id = plan["cell_id"]
        record = _mapping(raw, f"cell {cell_id}")
        if not required_cell.issubset(record) or not set(record).issubset(allowed_cell):
            raise ContractError(
                f"cell {cell_id} science fields are not closed; "
                f"missing={sorted(required_cell - set(record))}, "
                f"extra={sorted(set(record) - allowed_cell)}"
            )
        _sha256(record["data_sha256"], f"cell {cell_id}.data_sha256")
        scientific = _mapping(record["scientific"], f"cell {cell_id}.scientific")
        _expect_keys(scientific, {"candidate", "comparator", "oracle"}, f"cell {cell_id}.scientific")
        candidate = _validate_science_result(
            scientific["candidate"], f"cell {cell_id}.candidate"
        )
        comparator = _validate_science_result(
            scientific["comparator"], f"cell {cell_id}.comparator"
        )
        candidates[cell_id] = candidate
        records[cell_id] = record
        expected_output = {
            "fit": None,
            "prediction": "prediction_probe",
            "total_absorbed_effect": "total_absorbed_effect_probe",
            "individual_fe": "individual_fe_probe",
        }[plan["output"]]
        reasons.extend(f"{cell_id}:{failure}" for failure in _sample_size_failures(plan, candidate))
        reasons.extend(f"{cell_id}:comparator:{failure}" for failure in _sample_size_failures(plan, comparator))
        for output_field in OUTPUT_FIELDS:
            if (candidate["outputs"][output_field] is not None) != (
                output_field == expected_output
            ):
                reasons.append(f"{cell_id}:requested_output:{output_field}")
        reasons.extend(
            f"{cell_id}:comparator:{failure}"
            for failure in compare_science(manifest, candidate, comparator)
        )
        reasons.extend(
            f"{cell_id}:{failure}"
            for failure in _candidate_certificate_failures(manifest, candidate)
        )
        if plan["oracle_required"]:
            reasons.extend(
                f"{cell_id}:{failure}"
                for failure in _compare_registered_oracle(
                    manifest,
                    plan,
                    candidate,
                    scientific["oracle"],
                    f"cell {cell_id}.oracle",
                )
            )
        elif scientific["oracle"] is not None:
            raise ContractError(f"cell {cell_id} supplied an unregistered oracle result")

        if "diagnostics" in record:
            _validate_diagnostics(
                manifest, record["diagnostics"], plan, f"cell {cell_id}.diagnostics"
            )
        if "memory" in record:
            memory = _validate_memory(record["memory"], f"cell {cell_id}.memory")
            for phase in EXPECTED_PHASES:
                observed = memory["phase_observed_peak_bytes"][phase]
                if observed is None:
                    reasons.append(f"{cell_id}:phase_allocation_unobserved:{phase}")
                elif observed > memory["phase_modeled_peak_bytes"][phase]:
                    reasons.append(f"{cell_id}:phase_allocation_exceeds_model:{phase}")

    equivalence: dict[str, list[Mapping[str, Any]]] = {}
    for plan in expected_cells:
        group = plan["equivalence_group"]
        if group is not None:
            equivalence.setdefault(group, []).append(candidates[plan["cell_id"]])
    for group, results in equivalence.items():
        if len(results) != 3:
            raise ContractError(f"memory equivalence group {group} must contain fast, low, and auto")
        for result in results[1:]:
            reasons.extend(
                f"memory_equivalence:{group}:{failure}"
                for failure in compare_science(manifest, results[0], result)
            )
        group_ids = [
            plan["cell_id"] for plan in expected_cells if plan["equivalence_group"] == group
        ]
        if len({records[cell_id]["data_sha256"] for cell_id in group_ids}) != 1:
            reasons.append(f"memory_equivalence:{group}:data_identity")

    for plan in expected_cells:
        if plan["performance_role"] != "saved_output":
            continue
        cell_id = plan["cell_id"]
        fit_id = f"{plan['paired_fit_case']}_t{plan['requested_threads']}"
        if fit_id not in candidates:
            raise ContractError(f"saved-output pair {cell_id} has no measured fit cell")
        reasons.extend(
            f"{cell_id}:fit_invariance:{failure}"
            for failure in compare_science(
                manifest, candidates[cell_id], candidates[fit_id], compare_outputs=False
            )
        )
        if records[cell_id]["data_sha256"] != records[fit_id]["data_sha256"]:
            reasons.append(f"{cell_id}:fit_invariance:data_identity")
    return reasons


def evaluate_evidence(
    manifest: Mapping[str, Any],
    evidence: Mapping[str, Any],
    *,
    manifest_sha256: str | None = None,
) -> dict[str, Any]:
    validate_manifest(manifest)
    manifest_sha256 = manifest_sha256 or hashlib.sha256(canonical_json_bytes(manifest)).hexdigest()
    evidence = _mapping(evidence, "evidence")
    _expect_keys(
        evidence,
        {"schema_version", "evidence_type", "manifest", "identity", "cells", "constrained_memory"},
        "evidence",
    )
    if evidence["schema_version"] != 1 or evidence["evidence_type"] != "ppml-memory-performance-v1-evidence":
        raise ContractError("PPML performance evidence identity is invalid")
    manifest_record = _mapping(evidence["manifest"], "evidence.manifest")
    _expect_keys(manifest_record, {"id", "sha256"}, "evidence.manifest")
    if manifest_record != {"id": manifest["manifest_id"], "sha256": manifest_sha256}:
        raise ContractError("evidence names a different manifest")
    _validate_identity(manifest, evidence, manifest_sha256)

    expected_cells = expand_cells(manifest)
    expected_ids = [cell["cell_id"] for cell in expected_cells]
    if not isinstance(evidence["cells"], list):
        raise ContractError("evidence.cells must be a list")
    observed_ids = [
        value.get("cell_id") if isinstance(value, Mapping) else None
        for value in evidence["cells"]
    ]
    if observed_ids != expected_ids:
        raise ContractError("evidence cell inventory or order does not match the frozen plan")
    cell_records: dict[str, Mapping[str, Any]] = {}
    science_reasons: list[str] = []
    candidate_science: dict[str, Mapping[str, Any]] = {}
    for plan, raw in zip(expected_cells, evidence["cells"]):
        record = _mapping(raw, f"cell {plan['cell_id']}")
        _expect_keys(
            record,
            {"cell_id", "data_sha256", "scientific", "timing", "execution", "diagnostics", "memory", "slowdown_explanation"},
            f"cell {plan['cell_id']}",
        )
        _sha256(record["data_sha256"], f"cell {plan['cell_id']}.data_sha256")
        scientific = _mapping(record["scientific"], f"cell {plan['cell_id']}.scientific")
        _expect_keys(scientific, {"candidate", "comparator", "oracle"}, f"cell {plan['cell_id']}.scientific")
        candidate = _validate_science_result(scientific["candidate"], f"cell {plan['cell_id']}.candidate")
        comparator = _validate_science_result(scientific["comparator"], f"cell {plan['cell_id']}.comparator")
        candidate_science[plan["cell_id"]] = candidate
        science_reasons.extend(
            f"{plan['cell_id']}:{failure}" for failure in _sample_size_failures(plan, candidate)
        )
        science_reasons.extend(
            f"{plan['cell_id']}:comparator:{failure}" for failure in _sample_size_failures(plan, comparator)
        )
        expected_output = {
            "fit": None,
            "prediction": "prediction_probe",
            "total_absorbed_effect": "total_absorbed_effect_probe",
            "individual_fe": "individual_fe_probe",
        }[plan["output"]]
        for output_field in OUTPUT_FIELDS:
            should_exist = output_field == expected_output
            if (candidate["outputs"][output_field] is not None) != should_exist:
                science_reasons.append(
                    f"{plan['cell_id']}:requested_output:{output_field}"
                )
        for failure in compare_science(manifest, candidate, comparator):
            science_reasons.append(f"{plan['cell_id']}:comparator:{failure}")
        for failure in _candidate_certificate_failures(manifest, candidate):
            science_reasons.append(f"{plan['cell_id']}:{failure}")
        if plan["oracle_required"]:
            for failure in _compare_registered_oracle(
                manifest,
                plan,
                candidate,
                scientific["oracle"],
                f"cell {plan['cell_id']}.oracle",
            ):
                science_reasons.append(f"{plan['cell_id']}:{failure}")
        elif scientific["oracle"] is not None:
            raise ContractError(f"cell {plan['cell_id']} supplied an unregistered oracle result")
        cell_records[plan["cell_id"]] = record

    equivalence: dict[str, list[Mapping[str, Any]]] = {}
    for plan in expected_cells:
        if plan["equivalence_group"] is not None:
            equivalence.setdefault(plan["equivalence_group"], []).append(candidate_science[plan["cell_id"]])
    for group, results in equivalence.items():
        if len(results) != 3:
            raise ContractError(f"memory equivalence group {group} must contain fast, low, and auto")
        for result in results[1:]:
            for failure in compare_science(manifest, results[0], result):
                science_reasons.append(f"memory_equivalence:{group}:{failure}")
        group_cell_ids = [
            plan["cell_id"]
            for plan in expected_cells
            if plan["equivalence_group"] == group
        ]
        if len({cell_records[cell_id]["data_sha256"] for cell_id in group_cell_ids}) != 1:
            science_reasons.append(f"memory_equivalence:{group}:data_identity")

    plans_by_case = {case["id"]: case for case in manifest["cases"]}
    for plan in expected_cells:
        if plan["performance_role"] != "saved_output":
            continue
        fit_id = f"{plan['paired_fit_case']}_t{plan['requested_threads']}"
        if fit_id not in candidate_science:
            raise ContractError(f"saved-output pair {plan['cell_id']} has no measured fit cell")
        for failure in compare_science(
            manifest,
            candidate_science[plan["cell_id"]],
            candidate_science[fit_id],
            compare_outputs=False,
        ):
            science_reasons.append(f"{plan['cell_id']}:fit_invariance:{failure}")
        if cell_records[plan["cell_id"]]["data_sha256"] != cell_records[fit_id]["data_sha256"]:
            science_reasons.append(f"{plan['cell_id']}:fit_invariance:data_identity")

    if science_reasons:
        return {
            "schema_version": 1,
            "kind": "ppml_memory_performance_evaluation",
            "status": "fail",
            "stage": "scientific",
            "performance_evaluated": False,
            "reasons": science_reasons,
            "manifest_id": manifest["manifest_id"],
            "manifest_sha256": manifest_sha256,
            "cells": [],
        }

    performance_reasons: list[str] = []
    summaries: list[dict[str, Any]] = []
    summary_by_id: dict[str, dict[str, Any]] = {}
    minimum_warm = manifest["capture_contract"]["minimum_warm_repetitions"]
    memory_gates = manifest["memory_gates"]
    for plan in expected_cells:
        record = cell_records[plan["cell_id"]]
        timing = _mapping(record["timing"], f"cell {plan['cell_id']}.timing")
        _expect_keys(timing, {"candidate", "comparator"}, f"cell {plan['cell_id']}.timing")
        candidate_timing = _validate_timing(timing["candidate"], f"cell {plan['cell_id']}.timing.candidate", minimum_warm)
        comparator_timing = _validate_timing(timing["comparator"], f"cell {plan['cell_id']}.timing.comparator", minimum_warm)
        candidate_repetitions = 1 + len(candidate_timing["warm_seconds"])
        comparator_repetitions = 1 + len(comparator_timing["warm_seconds"])
        _validate_execution(
            record["execution"], f"cell {plan['cell_id']}.execution",
            plan["requested_threads"], candidate_repetitions, comparator_repetitions,
            manifest["capture_contract"],
        )
        diagnostics = _validate_diagnostics(
            manifest, record["diagnostics"], plan, f"cell {plan['cell_id']}.diagnostics"
        )
        memory = _validate_memory(record["memory"], f"cell {plan['cell_id']}.memory")
        has_explanation = _validate_explanation(
            record["slowdown_explanation"], f"cell {plan['cell_id']}.slowdown_explanation"
        )
        candidate_median = statistics.median(candidate_timing["warm_seconds"])
        comparator_median = statistics.median(comparator_timing["warm_seconds"])
        ratio = candidate_median / comparator_median
        modeled = memory["modeled_peak_bytes"]
        native = memory["native_peak_bytes"]
        relative_error = (
            abs(native - modeled) / modeled
            if modeled
            else (0.0 if native == 0 else math.inf)
        )
        if relative_error > memory_gates["maximum_modeled_allocation_relative_error"]:
            performance_reasons.append(f"{plan['cell_id']}:allocation_model_error")
        phase_relative_errors: dict[str, float | None] = {}
        for phase in EXPECTED_PHASES:
            phase_modeled = memory["phase_modeled_peak_bytes"][phase]
            phase_observed = memory["phase_observed_peak_bytes"][phase]
            if phase_observed is None:
                phase_relative_errors[phase] = None
                performance_reasons.append(
                    f"{plan['cell_id']}:phase_allocation_unobserved:{phase}"
                )
                continue
            phase_error = (
                abs(phase_observed - phase_modeled) / phase_modeled
                if phase_modeled
                else (0.0 if phase_observed == 0 else math.inf)
            )
            phase_relative_errors[phase] = phase_error
            if phase_error > memory_gates["maximum_modeled_allocation_relative_error"]:
                performance_reasons.append(
                    f"{plan['cell_id']}:phase_allocation_model_error:{phase}"
                )
        if memory["repeated_call_retained_bytes"] > memory_gates["maximum_repeated_call_retained_bytes"]:
            performance_reasons.append(f"{plan['cell_id']}:repeated_call_leak")
        if memory["allocator_event_count_end"] < memory["allocator_event_count_start"]:
            raise ContractError(f"cell {plan['cell_id']} allocator event counter moved backwards")
        if memory["allocator_peak_bytes_end"] < memory["allocator_peak_bytes_start"]:
            raise ContractError(f"cell {plan['cell_id']} allocator peak counter moved backwards")
        current_growth = (
            memory["allocator_current_bytes_end"] - memory["allocator_current_bytes_start"]
        )
        if abs(current_growth) > memory_gates["maximum_allocator_current_growth_bytes"]:
            performance_reasons.append(f"{plan['cell_id']}:allocator_current_growth")
        if memory["scratch_bytes_start"] != memory["scratch_bytes_end"]:
            performance_reasons.append(f"{plan['cell_id']}:scratch_bytes_growth")
        if memory["scratch_files_after"] != 0:
            performance_reasons.append(f"{plan['cell_id']}:scratch_leak")
        if not memory["fresh_warm_equivalent"]:
            performance_reasons.append(f"{plan['cell_id']}:fresh_warm_mismatch")

        qualified_routes = set(
            memory_gates["whole_iteration_allocation_free_qualified_linear_routes"]
        )
        linear_route = diagnostics["linear_route"]
        route_allocation_failures: list[str] = []
        if linear_route in qualified_routes:
            if (
                memory["measured_steady_state_iterations"]
                < memory_gates["minimum_measured_steady_state_iterations"]
            ):
                route_allocation_failures.append("insufficient_allocation_measurement_iterations")
            if (
                memory["allocator_event_count_end"]
                - memory["allocator_event_count_start"]
                > memory_gates["maximum_allocator_event_growth"]
            ):
                route_allocation_failures.append("allocator_event_growth")
            if (
                memory["allocator_peak_bytes_end"]
                - memory["allocator_peak_bytes_start"]
                > memory_gates["maximum_allocator_peak_growth_bytes"]
            ):
                route_allocation_failures.append("allocator_peak_growth")
            if abs(current_growth) > memory_gates["maximum_allocator_current_growth_bytes"]:
                route_allocation_failures.append("allocator_current_growth")
            if memory["workspace_capacity_start"] != memory["workspace_capacity_end"]:
                route_allocation_failures.append("workspace_capacity_growth")
            if memory["scratch_bytes_start"] != memory["scratch_bytes_end"]:
                route_allocation_failures.append("scratch_bytes_growth")
            if not memory["linear_session_allocation_free"]:
                route_allocation_failures.append("linear_session_allocation")
            if not memory["fresh_warm_equivalent"]:
                route_allocation_failures.append("fresh_warm_mismatch")
            if not diagnostics["workspace_allocation_free"]:
                route_allocation_failures.append("workspace_allocation")
            if not diagnostics["whole_iteration_allocation_free"]:
                route_allocation_failures.append("whole_iteration_allocation")
        elif diagnostics["whole_iteration_allocation_free"]:
            route_allocation_failures.append(
                f"whole_iteration_allocation_claim_unqualified_route:{linear_route}"
            )
        performance_reasons.extend(
            f"{plan['cell_id']}:{failure}" for failure in route_allocation_failures
        )
        if plan["performance_role"] == "primary" and ratio > manifest["performance_gates"]["maximum_unexplained_cell_ratio"] and not has_explanation:
            performance_reasons.append(f"{plan['cell_id']}:unexplained_slowdown")
        summary = {
            "cell_id": plan["cell_id"],
            "case_id": plan["id"],
            "profile": plan["profile"],
            "scale": plan["scale"],
            "n": plan["scale_spec"]["n"],
            "requested_threads": plan["requested_threads"],
            "memory": plan["memory"],
            "output": plan["output"],
            "performance_role": plan["performance_role"],
            "candidate_warm_median_seconds": candidate_median,
            "comparator_warm_median_seconds": comparator_median,
            "candidate_over_comparator": ratio,
            "outer_iterations": diagnostics["outer_iterations"],
            "linear_route": linear_route,
            "whole_iteration_allocation_qualified": (
                linear_route in qualified_routes and not route_allocation_failures
            ),
            "median_steady_iteration_seconds": memory["median_steady_iteration_seconds"],
            "allocation_relative_error": relative_error,
            "phase_allocation_relative_errors": phase_relative_errors,
        }
        summaries.append(summary)
        summary_by_id[plan["cell_id"]] = summary

    primary = [item for item in summaries if item["performance_role"] == "primary"]
    primary_ratios = [item["candidate_over_comparator"] for item in primary]
    geometric_mean = math.exp(statistics.fmean(math.log(value) for value in primary_ratios))
    twice_share = sum(value <= manifest["performance_gates"]["maximum_twice_as_fast_ratio"] for value in primary_ratios) / len(primary_ratios)
    if geometric_mean > manifest["performance_gates"]["maximum_equal_cell_geometric_mean_ratio"]:
        performance_reasons.append("governing_geometric_mean_above_0.40")
    if twice_share < manifest["performance_gates"]["minimum_twice_as_fast_share"]:
        performance_reasons.append("governing_twice_as_fast_share_below_0.80")
    single_thread = [item for item in primary if item["requested_threads"] == 1]
    for item in single_thread:
        if item["candidate_over_comparator"] > manifest["performance_gates"]["maximum_registered_single_thread_ratio"]:
            performance_reasons.append(f"{item['cell_id']}:single_thread_ratio_above_two_thirds")

    for plan in expected_cells:
        if plan["performance_role"] != "saved_output":
            continue
        save = summary_by_id[plan["cell_id"]]
        fit_id = f"{plan['paired_fit_case']}_t{plan['requested_threads']}"
        if fit_id not in summary_by_id:
            raise ContractError(f"saved-output pair {plan['cell_id']} has no measured fit cell")
        fit = summary_by_id[fit_id]
        pair_case = plans_by_case[plan["paired_fit_case"]]
        for field in ("scale", "design", "controls", "outcome", "finite_design", "weight", "vce", "memory"):
            if plan[field] != pair_case[field]:
                raise ContractError(f"saved-output pair {plan['cell_id']} changes estimation field {field}")
        overhead = save["candidate_warm_median_seconds"] - fit["candidate_warm_median_seconds"]
        allowed = max(
            manifest["performance_gates"]["saved_output_maximum_seconds"],
            manifest["performance_gates"]["saved_output_maximum_fit_fraction"] * fit["candidate_warm_median_seconds"],
        )
        save["saved_output_incremental_seconds"] = overhead
        save["saved_output_allowed_seconds"] = allowed
        if overhead > allowed:
            performance_reasons.append(f"{plan['cell_id']}:saved_output_overhead")

    scaling: dict[str, list[tuple[int, Mapping[str, Any]]]] = {}
    for plan in expected_cells:
        if plan["scaling_group"] is not None and plan["requested_threads"] == plan["scaling_thread"]:
            scaling.setdefault(plan["scaling_group"], []).append(
                (plan["scale_spec"]["n"], summary_by_id[plan["cell_id"]])
            )
    for group, points in scaling.items():
        if [n for n, _ in sorted(points)] != [100_000, 1_000_000, 10_000_000]:
            raise ContractError(f"scaling group {group} does not contain all three scales")
        normalized = [
            (n, item["median_steady_iteration_seconds"] / n)
            for n, item in sorted(points)
        ]
        for (lower_n, lower), (upper_n, upper) in zip(normalized, normalized[1:]):
            if upper / lower > manifest["performance_gates"]["maximum_normalized_iteration_cost_growth"]:
                performance_reasons.append(
                    f"scaling:{group}:{lower_n}_to_{upper_n}:superlinear_iteration_cost"
                )

    performance_reasons.extend(
        _validate_constrained(
            manifest,
            evidence["constrained_memory"],
            embedded_git_commit=evidence["identity"]["source_commit"][:12],
        )
    )
    if not manifest["qualification_eligible"]:
        performance_reasons.append("development_manifest_not_qualification_eligible")
    status = "pass" if not performance_reasons else (
        "blocked" if performance_reasons == ["development_manifest_not_qualification_eligible"] else "fail"
    )
    return {
        "schema_version": 1,
        "kind": "ppml_memory_performance_evaluation",
        "status": status,
        "stage": "complete",
        "performance_evaluated": True,
        "reasons": performance_reasons,
        "manifest_id": manifest["manifest_id"],
        "manifest_sha256": manifest_sha256,
        "governing_cells": len(primary),
        "governing_equal_cell_geometric_mean_ratio": geometric_mean,
        "governing_twice_as_fast_share": twice_share,
        "registered_single_thread_cells": len(single_thread),
        "cells": summaries,
    }


def render_markdown(result: Mapping[str, Any]) -> str:
    lines = [
        "# PPML memory and performance evaluation",
        "",
        f"Status: **{result['status']}**",
        f"Stage: `{result['stage']}`",
        "",
    ]
    if result["performance_evaluated"]:
        lines.extend(
            [
                f"- Governing cells: {result['governing_cells']}",
                f"- Equal-cell geometric-mean ratio: {result['governing_equal_cell_geometric_mean_ratio']:.3f}",
                f"- Share at or below 0.50: {result['governing_twice_as_fast_share']:.1%}",
                f"- Registered single-thread cells: {result['registered_single_thread_cells']}",
                "",
            ]
        )
    lines.append("## Reasons")
    lines.append("")
    if result["reasons"]:
        lines.extend(f"- `{reason}`" for reason in result["reasons"])
    else:
        lines.append("- None")
    lines.append("")
    return "\n".join(lines)


def _write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    actions = parser.add_subparsers(dest="action", required=True)
    actions.add_parser("validate-manifest")
    plan = actions.add_parser("plan")
    plan.add_argument("--profile", choices=("development", "scc_governing"))
    plan.add_argument("--output", type=Path)
    evaluate = actions.add_parser("evaluate")
    evaluate.add_argument("--evidence", type=Path, required=True)
    evaluate.add_argument("--output", type=Path, required=True)
    evaluate.add_argument("--markdown", type=Path)
    evaluate_science = actions.add_parser("evaluate-science")
    evaluate_science.add_argument("--evidence", type=Path, required=True)
    evaluate_science.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        manifest = load_manifest(args.manifest)
        if args.action == "validate-manifest":
            print(sha256_file(args.manifest))
            return 0
        if args.action == "plan":
            document = expand_cells(manifest, args.profile)
            if args.output:
                _write_json(args.output, document)
            else:
                print(json.dumps(document, indent=2, sort_keys=True))
            return 0
        evidence = json.loads(args.evidence.read_text(encoding="utf-8"))
        if args.action == "evaluate-science":
            reasons = evaluate_science_evidence(
                manifest,
                evidence,
                manifest_sha256=sha256_file(args.manifest),
            )
            result = {
                "schema_version": 1,
                "kind": "ppml_memory_performance_science_preflight",
                "status": "pass" if not reasons else "fail",
                "stage": "scientific",
                "performance_evaluated": False,
                "reasons": reasons,
                "manifest_id": manifest["manifest_id"],
                "manifest_sha256": sha256_file(args.manifest),
            }
            _write_json(args.output, result)
            return 0 if not reasons else 1
        result = evaluate_evidence(
            manifest,
            evidence,
            manifest_sha256=sha256_file(args.manifest),
        )
        _write_json(args.output, result)
        if args.markdown:
            args.markdown.write_text(render_markdown(result), encoding="utf-8")
        return 0 if result["status"] in {"pass", "blocked"} else 1
    except (ContractError, OSError, json.JSONDecodeError) as error:
        print(f"ppml_memory_performance: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
