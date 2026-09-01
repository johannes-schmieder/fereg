#!/usr/bin/env python3
"""Evaluate and seal the exact-SHA Phase 12 ``feppml`` qualification.

The lower-level PPML memory/performance evaluator owns the mathematical
candidate/comparator/oracle comparisons and operational gates.  This module
adds the release boundary that development evidence intentionally lacks:

* exact candidate source/package/plugin and comparator-closure identities;
* a source/call-graph audit of the live ``estimate-ppml`` transaction;
* immutable independent-oracle identities;
* one cold and nine matched fresh-process AB/BA pairs for every cell;
* cross-platform build/load receipts; and
* an immutable SHA-256 receipt envelope.

Scientific evaluation always finishes before this module reads any timing
measurement.  A scientific failure therefore cannot accidentally become a
performance result.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import io
import json
import math
import os
from pathlib import Path
import re
import statistics
import subprocess
import sys
import tarfile
from typing import Any, Callable, Mapping, Sequence

try:
    from scripts.ppml_legacy_boundary import audit_documents as audit_ppml_legacy_documents
except ImportError:
    from ppml_legacy_boundary import audit_documents as audit_ppml_legacy_documents


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "benchmarks/manifests/feppml_qualification_v1.json"
DEFAULT_DEVELOPMENT_MANIFEST = ROOT / "benchmarks/manifests/ppml_memory_performance_v1.json"
HEX_SHA1 = re.compile(r"^[0-9a-f]{40}$")
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
EXPECTED_CELL_IDS = [
    "dev_n100k_no_fe_k0_t1",
    "dev_n100k_one_fe_offset_fw_c1_t2",
    "dev_n100k_lowmob_exposure_pw_c2_t4",
    "dev_n100k_three_fe_separated_c3_t8",
    "dev_n100k_slopes_collinear_c4_t16",
    "dev_n100k_disconnected_illcond_c5_t1",
    "scale_n100k_two_fe_fast_t8",
    "scale_n1m_two_fe_fast_t8",
    "route_n1m_two_fe_low_t8",
    "route_n1m_two_fe_auto_t8",
    "gov_no_fe_easy_k0_t1",
    "gov_no_fe_easy_k0_t8",
    "gov_one_fe_offset_k10_fw_c1_t1",
    "gov_one_fe_offset_k10_fw_c1_t8",
    "gov_two_fe_standard_k10_t1",
    "gov_two_fe_standard_k10_t2",
    "gov_two_fe_standard_k10_t4",
    "gov_two_fe_standard_k10_t8",
    "gov_two_fe_standard_k10_t16",
    "gov_two_fe_lowmob_long_k50_pw_c2_t1",
    "gov_two_fe_lowmob_long_k50_pw_c2_t8",
    "gov_two_fe_lowmob_long_k50_pw_c2_t16",
    "gov_disconnected_separated_k10_c3_t1",
    "gov_disconnected_separated_k10_c3_t8",
    "gov_three_fe_exposure_k10_fw_c4_t1",
    "gov_three_fe_exposure_k10_fw_c4_t8",
    "gov_slopes_illcond_k50_pw_c5_t1",
    "gov_slopes_illcond_k50_pw_c5_t8",
    "gov_collinear_two_fe_k50_t1",
    "gov_collinear_two_fe_k50_t8",
    "save_prediction_two_fe_t8",
    "save_total_fe_two_fe_t8",
    "save_individual_fe_three_fe_t8",
]

EXPECTED_RECIPE_PROFILE_SHA256 = (
    "709f119450b3df485c6c0b15f9fd53ab034529bf698095b57b2d13a0fb0952b6"
)
EXPECTED_RECIPE_COMPILERS = {
    "main": "scripts/generate_feppml_qualification_recipes.py",
    "oracle": "scripts/generate_feppml_oracle_fixtures.py",
    "constrained": "scripts/generate_feppml_constrained_recipes.py",
}
EXPECTED_ORACLE_CELL_IDS = EXPECTED_CELL_IDS[:7]


class QualificationError(ValueError):
    """A frozen input, evidence document, or receipt fails closed."""


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
        raise QualificationError(f"value is not canonical finite JSON: {error}") from error
    return (encoded + "\n").encode("ascii")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1 << 20), b""):
                digest.update(block)
    except OSError as error:
        raise QualificationError(f"unable to hash {path}: {error}") from error
    return digest.hexdigest()


def _mapping(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise QualificationError(f"{where} must be an object")
    return value


def _expect_keys(value: Mapping[str, Any], expected: set[str], where: str) -> None:
    if set(value) != expected:
        raise QualificationError(
            f"{where} fields are not closed; "
            f"missing={sorted(expected - set(value))}, extra={sorted(set(value) - expected)}"
        )


def _integer(value: Any, where: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        raise QualificationError(f"{where} must be an integer >= {minimum}")
    return value


def _number(value: Any, where: str, *, positive: bool = False) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise QualificationError(f"{where} must be numeric")
    output = float(value)
    if not math.isfinite(output) or (positive and output <= 0.0):
        raise QualificationError(f"{where} must be {'positive ' if positive else ''}finite")
    return output


def _sha1(value: Any, where: str) -> str:
    if not isinstance(value, str) or HEX_SHA1.fullmatch(value) is None:
        raise QualificationError(f"{where} must be a full lowercase git SHA")
    return value


def _sha256(value: Any, where: str) -> str:
    if not isinstance(value, str) or HEX_SHA256.fullmatch(value) is None:
        raise QualificationError(f"{where} must be a lowercase SHA-256")
    return value


def _read_json(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise QualificationError(f"unable to read {where}: {error}") from error
    return _mapping(value, where)


def load_manifest(
    path: Path = DEFAULT_MANIFEST,
    *,
    verify_files: bool = True,
    root: Path = ROOT,
) -> dict[str, Any]:
    manifest = dict(_read_json(path, "qualification manifest"))
    validate_manifest(manifest, root=root if verify_files else None)
    return manifest


def _expected_reference_commits() -> dict[str, str]:
    return {
        "ppmlhdfe": "b85665d8f674e93c1e07446a9a2b29be7f797910",
        "reghdfe": "29b2b203f534413369d675d0c0f674d61291f536",
        "ftools": "d6e24d66fd02ba1b37325c82c5040444c5b4c636",
        "require": "f4760b86e55cf73b82041ce59cf20e8e8224afb0",
    }


def _recipe_contract_module(path: Path):
    name = "feppml_qualification_recipe_contract_frozen"
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise QualificationError("unable to load frozen recipe contract")
    module = importlib.util.module_from_spec(spec)
    previous = sys.modules.get(name)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException as error:
        if previous is None:
            sys.modules.pop(name, None)
        else:
            sys.modules[name] = previous
        raise QualificationError(f"frozen recipe contract failed: {error}") from error
    return module


def validate_manifest(manifest: Mapping[str, Any], *, root: Path | None = ROOT) -> None:
    _expect_keys(
        manifest,
        {
            "schema_version",
            "manifest_id",
            "status",
            "qualification_eligible",
            "contracts",
            "reference_closure",
            "oracle_program",
            "independent_oracles",
            "recipe_contract",
            "source_gate",
            "capture_contract",
            "matrix_contract",
            "scientific_gates",
            "performance_gates",
            "memory_gates",
            "platform_gates",
            "evidence_contract",
        },
        "manifest",
    )
    if (
        manifest["schema_version"] != 1
        or manifest["manifest_id"] != "feppml_qualification_v1"
        or manifest["status"] != "qualification_frozen"
        or manifest["qualification_eligible"] is not True
    ):
        raise QualificationError("qualification manifest identity is not frozen")

    contracts = _mapping(manifest["contracts"], "contracts")
    if set(contracts) != {
        "scientific", "separation", "memory_performance", "fereg_nonregression"
    }:
        raise QualificationError("contract inventory is incomplete")
    contract_paths = {
        "scientific": "docs/adr/0016-ppml-scientific-contract.md",
        "separation": "docs/adr/0017-certified-ppml-separation-ledger.md",
        "memory_performance": "docs/adr/0018-ppml-memory-routing-performance.md",
        "fereg_nonregression": (
            "docs/adr/0019-additive-ppml-transaction-and-fereg-regression-boundary.md"
        ),
    }
    for name, raw in contracts.items():
        record = _mapping(raw, f"contracts.{name}")
        _expect_keys(record, {"path", "sha256"}, f"contracts.{name}")
        if record["path"] != contract_paths[name]:
            raise QualificationError(f"contract path changed: {name}")
        _sha256(record["sha256"], f"contracts.{name}.sha256")
        if root is not None and sha256_file(root / record["path"]) != record["sha256"]:
            raise QualificationError(f"contract file changed after manifest freeze: {name}")

    references = _mapping(manifest["reference_closure"], "reference_closure")
    _expect_keys(
        references,
        {"references_lock_sha256", "ppmlhdfe", "reghdfe", "ftools", "require", "policy"},
        "reference_closure",
    )
    _sha256(references["references_lock_sha256"], "reference_closure.references_lock_sha256")
    if root is not None and sha256_file(root / "references.lock.toml") != references["references_lock_sha256"]:
        raise QualificationError("references.lock.toml changed after qualification freeze")
    reference_repositories = {
        "ppmlhdfe": "https://github.com/sergiocorreia/ppmlhdfe",
        "reghdfe": "https://github.com/sergiocorreia/reghdfe",
        "ftools": "https://github.com/sergiocorreia/ftools",
        "require": "https://github.com/sergiocorreia/stata-require",
    }
    if references["policy"] != (
        "offline clean exact-commit direct-child checkouts; no install or fetch during capture"
    ):
        raise QualificationError("reference closure policy changed")
    for name, commit in _expected_reference_commits().items():
        record = _mapping(references[name], f"reference_closure.{name}")
        _expect_keys(record, {"repository", "commit"}, f"reference_closure.{name}")
        if (
            record["commit"] != commit
            or record["repository"] != reference_repositories[name]
        ):
            raise QualificationError(f"{name} reference commit changed")

    oracle_program = _mapping(manifest["oracle_program"], "oracle_program")
    _expect_keys(oracle_program, {"path", "sha256"}, "oracle_program")
    if oracle_program["path"] != "scripts/feppml_explicit_dummy_oracle.py":
        raise QualificationError("independent oracle driver path changed")
    _sha256(oracle_program["sha256"], "oracle_program.sha256")
    if root is not None and sha256_file(root / oracle_program["path"]) != oracle_program["sha256"]:
        raise QualificationError("independent oracle driver changed")

    oracles = manifest["independent_oracles"]
    if not isinstance(oracles, list) or [item.get("id") for item in oracles if isinstance(item, Mapping)] != [
        "explicit_dummy", "multiway_cluster"
    ]:
        raise QualificationError("independent oracle inventory or order changed")
    oracle_metadata = {
        "explicit_dummy": (
            "tests/python/ppml_explicit_dummy_oracle.py",
            "coefficients, objective, residuals, weights, rank, and one-way sandwich",
        ),
        "multiway_cluster": (
            "tests/python/ppml_multiway_cluster_oracle.py",
            "independent one-through-five-way CGM inclusion-exclusion and common correction",
        ),
    }
    for index, raw in enumerate(oracles):
        record = _mapping(raw, f"independent_oracles[{index}]")
        _expect_keys(record, {"id", "path", "sha256", "role"}, f"independent_oracles[{index}]")
        expected_path, expected_role = oracle_metadata[record["id"]]
        if record["path"] != expected_path or record["role"] != expected_role:
            raise QualificationError(f"independent oracle metadata changed: {record['id']}")
        _sha256(record["sha256"], f"independent_oracles[{index}].sha256")
        if root is not None and sha256_file(root / record["path"]) != record["sha256"]:
            raise QualificationError(f"independent oracle changed: {record['id']}")

    recipe_contract = _mapping(manifest["recipe_contract"], "recipe_contract")
    _expect_keys(
        recipe_contract,
        {"path", "sha256", "profile_sha256", "compilers"},
        "recipe_contract",
    )
    if (
        recipe_contract["path"]
            != "scripts/feppml_qualification_recipe_contract.py"
        or recipe_contract["profile_sha256"]
            != EXPECTED_RECIPE_PROFILE_SHA256
    ):
        raise QualificationError("independent recipe profile identity changed")
    _sha256(recipe_contract["sha256"], "recipe_contract.sha256")
    compilers = recipe_contract["compilers"]
    if (
        not isinstance(compilers, list)
        or [item.get("id") for item in compilers if isinstance(item, Mapping)]
            != list(EXPECTED_RECIPE_COMPILERS)
    ):
        raise QualificationError("qualification compiler inventory/order changed")
    for index, raw in enumerate(compilers):
        record = _mapping(raw, f"recipe_contract.compilers[{index}]")
        _expect_keys(
            record,
            {"id", "path", "sha256"},
            f"recipe_contract.compilers[{index}]",
        )
        if record["path"] != EXPECTED_RECIPE_COMPILERS[record["id"]]:
            raise QualificationError("qualification compiler path changed")
        _sha256(record["sha256"], f"recipe_contract.compilers[{index}].sha256")
    if root is not None:
        contract_path = root / recipe_contract["path"]
        if sha256_file(contract_path) != recipe_contract["sha256"]:
            raise QualificationError("independent recipe contract source changed")
        for record in compilers:
            if sha256_file(root / record["path"]) != record["sha256"]:
                raise QualificationError(
                    f"qualification compiler changed: {record['id']}"
                )
        contract_module = _recipe_contract_module(contract_path)
        profile = contract_module.canonical_profile(
            root / "benchmarks/manifests/ppml_memory_performance_v1.json"
        )
        if (
            contract_module.profile_digest(profile)
            != recipe_contract["profile_sha256"]
        ):
            raise QualificationError("independent recipe profile digest changed")

    source_gate = _mapping(manifest["source_gate"], "source_gate")
    _expect_keys(
        source_gate,
        {
            "production_path", "forbidden_route_symbols", "exact_full_source_sha_required",
            "clean_archive_required", "rule",
        },
        "source_gate",
    )
    if (
        source_gate["production_path"] != "crates/fereg-plugin/src/ppml_estimate.rs"
        or source_gate["forbidden_route_symbols"] != ["fit_ppml_separated", "profile_irls_step"]
        or source_gate["exact_full_source_sha_required"] is not True
        or source_gate["clean_archive_required"] is not True
    ):
        raise QualificationError("production source gate was weakened")

    capture = _mapping(manifest["capture_contract"], "capture_contract")
    _expect_keys(
        capture,
        {
            "scientific_preflight_before_timing",
            "fresh_process_per_arm_measurement",
            "same_node_and_stata_per_pair",
            "immutable_generated_data",
            "generated_outside_timing",
            "cold_pairs",
            "warm_pairs",
            "warm_pair_order",
            "candidate_arm",
            "comparator_arm",
            "raw_measurements_required",
            "peak_rss_required",
            "terminal_sentinel_required",
            "sealed_receipts_required",
            "task_shards",
            "pairs_per_shard",
            "arms_per_pair",
            "comparator_processor_limit",
            "raw_file_inventory_required",
            "collector_raw_reparse_required",
            "pure_replay_required",
            "sealed_child_environment_required",
            "inherited_child_environment_forbidden",
            "sealed_capture_imports_required",
            "isolated_oracle_environment_required",
            "isolated_oracle_argv",
            "sealed_oracle_imports_required",
            "core_preflight_receipt",
        },
        "capture_contract",
    )
    expected_order = ["AB", "BA", "AB", "BA", "AB", "BA", "AB", "BA", "AB"]
    core_preflight = _mapping(
        capture["core_preflight_receipt"],
        "capture_contract.core_preflight_receipt",
    )
    _expect_keys(
        core_preflight,
        {
            "receipt_type",
            "complete_sentinel",
            "workspace_allocation_started",
            "exact_file_inventory_required",
        },
        "capture_contract.core_preflight_receipt",
    )
    if (
        capture.get("cold_pairs") != 1
        or capture.get("warm_pairs") != 9
        or capture.get("warm_pair_order") != expected_order
        or capture.get("candidate_arm") != "feppml"
        or capture.get("comparator_arm") != "ppmlhdfe"
        or capture.get("task_shards") != 33
        or capture.get("pairs_per_shard") != 10
        or capture.get("arms_per_pair") != 2
        or capture.get("comparator_processor_limit") != 16
        or not all(
            capture.get(field) is True
            for field in (
                "scientific_preflight_before_timing",
                "fresh_process_per_arm_measurement",
                "same_node_and_stata_per_pair",
                "immutable_generated_data",
                "generated_outside_timing",
                "raw_measurements_required",
                "peak_rss_required",
                "terminal_sentinel_required",
                "sealed_receipts_required",
                "raw_file_inventory_required",
                "collector_raw_reparse_required",
                "pure_replay_required",
                "sealed_child_environment_required",
                "inherited_child_environment_forbidden",
                "sealed_capture_imports_required",
                "isolated_oracle_environment_required",
                "sealed_oracle_imports_required",
            )
        )
        or capture.get("isolated_oracle_argv") != ["-I", "-B", "-S"]
        or core_preflight
        != {
            "receipt_type": "feppml-memory-preflight-v1",
            "complete_sentinel": "FEPPML_MEMORY_PREFLIGHT_COMPLETE",
            "workspace_allocation_started": False,
            "exact_file_inventory_required": True,
        }
    ):
        raise QualificationError("capture cadence or isolation contract was weakened")

    matrix = _mapping(manifest["matrix_contract"], "matrix_contract")
    required_matrix_fields = {
        "development_schema", "development_manifest", "expected_cell_ids",
        "local_cell_count", "scc_cell_count", "oracle_required_cell_ids",
        "required_dimensions",
    }
    _expect_keys(matrix, required_matrix_fields, "matrix_contract")
    development_manifest = _mapping(
        matrix["development_manifest"], "matrix_contract.development_manifest"
    )
    _expect_keys(
        development_manifest,
        {"path", "sha256"},
        "matrix_contract.development_manifest",
    )
    if development_manifest["path"] != (
        "benchmarks/manifests/ppml_memory_performance_v1.json"
    ):
        raise QualificationError("development manifest path is not frozen")
    _sha256(
        development_manifest["sha256"],
        "matrix_contract.development_manifest.sha256",
    )
    if root is not None and sha256_file(
        root / development_manifest["path"]
    ) != development_manifest["sha256"]:
        raise QualificationError(
            "development manifest changed after qualification freeze"
        )
    ids = matrix["expected_cell_ids"]
    required_dimensions = {
        "scales": [100_000, 1_000_000, 10_000_000],
        "fe_dimensions": [0, 1, 2, 3],
        "finite_designs": ["full_rank", "collinear", "ill_conditioned"],
        "weights": ["none", "fweight", "pweight"],
        "vces": ["robust", "cluster1", "cluster2", "cluster3", "cluster4", "cluster5"],
        "threads": [1, 2, 4, 8, 16],
        "memory_modes": ["fast", "low", "auto"],
        "outputs": ["fit", "prediction", "total_absorbed_effect", "individual_fe"],
    }
    if (
        matrix["development_schema"] != "ppml-memory-performance-v1-evidence"
        or ids != EXPECTED_CELL_IDS
        or matrix["local_cell_count"] != 10
        or matrix["scc_cell_count"] != 23
        or matrix["oracle_required_cell_ids"] != EXPECTED_ORACLE_CELL_IDS
        or matrix["required_dimensions"] != required_dimensions
    ):
        raise QualificationError("qualification cell inventory is not closed")

    scientific = _mapping(manifest["scientific_gates"], "scientific_gates")
    expected_scientific = {
        "discrete_fields_exact": True,
        "coefficient_atol": 1e-9,
        "coefficient_rtol": 1e-7,
        "vce_atol": 1e-9,
        "vce_rtol": 1e-6,
        "objective_atol": 1e-8,
        "objective_rtol": 1e-8,
        "output_probe_atol": 1e-8,
        "output_probe_rtol": 1e-7,
        "maximum_score_linf": 1e-8,
        "maximum_fe_score_linf": 1e-8,
        "maximum_vce_symmetry_linf": 1e-10,
        "maximum_reconstruction_linf": 1e-8,
        "require_two_consecutive_iterations": True,
        "require_strict_final_solve": True,
        "require_frozen_rank": True,
    }
    if scientific != expected_scientific:
        raise QualificationError("scientific qualification gate changed")

    performance = _mapping(manifest["performance_gates"], "performance_gates")
    expected_performance = {
        "governing_scale": 10_000_000,
        "maximum_equal_cell_geometric_mean_ratio": 0.40,
        "maximum_twice_as_fast_ratio": 0.50,
        "minimum_twice_as_fast_share": 0.80,
        "maximum_registered_single_thread_ratio": 2.0 / 3.0,
        "maximum_unexplained_cell_ratio": 1.25,
        "saved_output_maximum_seconds": 0.05,
        "saved_output_maximum_fit_fraction": 0.05,
        "maximum_normalized_iteration_cost_growth": 1.25,
    }
    if performance != expected_performance:
        raise QualificationError("ADR 0018 performance gate was weakened")
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
            "required_fixtures",
        },
        "memory_gates",
    )
    if (
        memory.get("maximum_modeled_allocation_relative_error") != 0.25
        or memory.get("maximum_repeated_call_retained_bytes") != 0
        or memory.get("whole_iteration_allocation_free_qualified_linear_routes")
            != ["map"]
        or memory.get("minimum_measured_steady_state_iterations") != 16
        or memory.get("maximum_allocator_event_growth") != 0
        or memory.get("maximum_allocator_current_growth_bytes") != 0
        or memory.get("maximum_allocator_peak_growth_bytes") != 0
        or memory.get("require_stable_workspace_capacity") is not True
        or memory.get("require_stable_scratch_bytes") is not True
        or memory.get("require_linear_session_allocation_free") is not True
        or memory.get("require_fresh_warm_equivalent") is not True
        or memory.get("require_zero_scratch_files_after") is not True
        or memory.get("constrained_preflight_maximum_allocation_bytes")
            != 1_048_576
        or memory.get("required_fixtures") != ["low_succeeds_fast_fails", "no_route_fits"]
    ):
        raise QualificationError("ADR 0018 memory gate was weakened")
    platforms = _mapping(manifest["platform_gates"], "platform_gates")
    _expect_keys(
        platforms,
        {
            "required_artifacts",
            "required_load_status",
            "licensed_stata_load_required",
            "mock_only_receipts_qualification_eligible",
            "platform_receipt_scientific_claim",
            "platform_receipt_scope",
            "performance_authority",
            "windows_scope",
            "capture_program",
        },
        "platform_gates",
    )
    platform_program = _mapping(
        platforms["capture_program"], "platform_gates.capture_program"
    )
    _expect_keys(
        platform_program,
        {"path", "sha256"},
        "platform_gates.capture_program",
    )
    _sha256(platform_program["sha256"], "platform_gates.capture_program.sha256")
    if platforms.get("required_artifacts") != [
        "macos-universal", "linux-x86_64", "windows-x86_64"
    ] or platforms.get("required_load_status") != "success" or (
        platforms.get("licensed_stata_load_required") is not True
        or platforms.get("mock_only_receipts_qualification_eligible") is not False
        or platforms.get("platform_receipt_scientific_claim") is not False
        or platforms.get("platform_receipt_scope")
        != "artifact_and_licensed_stata_load_only"
        or platforms.get("performance_authority")
            != "linux-x86_64 licensed Stata SCC"
        or platforms.get("windows_scope")
            != "build, binary audit, mock SPI, and licensed load when available"
        or platform_program.get("path")
        != "tests/stata/installation/feppml_platform_load_capture.do"
        or (
            root is not None
            and sha256_file(root / platform_program["path"])
                != platform_program["sha256"]
        )
    ):
        raise QualificationError("cross-platform artifact gate was weakened")
    evidence_contract = _mapping(
        manifest["evidence_contract"], "evidence_contract"
    )
    if evidence_contract != {
        "schema_version": 1,
        "evidence_type": "feppml-qualification-v1-evidence",
        "receipt_type": "feppml-qualification-v1-receipt",
        "complete_sentinel": "FEPPML_QUALIFICATION_COMPLETE",
    }:
        raise QualificationError("qualification evidence contract changed")


def _git_bytes(source_root: Path, arguments: Sequence[str]) -> bytes:
    try:
        completed = subprocess.run(
            ["git", "-C", str(source_root), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", b"").decode("utf-8", errors="replace").strip()
        raise QualificationError(f"unable to resolve exact source: {detail}") from error
    return completed.stdout


def _legacy_route_violations(
    source_root: Path,
    commit: str,
    *,
    git_reader: Callable[[Path, Sequence[str]], bytes] = _git_bytes,
) -> list[str]:
    listed = git_reader(
        source_root,
        [
            "ls-tree", "-r", "--name-only", commit, "--",
            "crates/fereg-core/src", "crates/fereg-plugin/src",
        ],
    ).decode("utf-8", errors="strict")
    paths = [path for path in listed.splitlines() if path.endswith(".rs")]
    documents = {
        path: git_reader(source_root, ["show", f"{commit}:{path}"]).decode(
            "utf-8", errors="strict"
        )
        for path in paths
    }
    return [str(violation) for violation in audit_ppml_legacy_documents(documents)]


def audit_source(
    source_root: Path,
    commit: str,
    manifest: Mapping[str, Any],
    *,
    git_reader: Callable[[Path, Sequence[str]], bytes] = _git_bytes,
) -> dict[str, Any]:
    """Audit the complete committed production Rust boundary, never the working tree."""

    commit = _sha1(commit, "source commit")
    observed = git_reader(source_root, ["rev-parse", f"{commit}^{{commit}}"]).decode("ascii").strip()
    if observed != commit:
        raise QualificationError("source commit did not resolve exactly")
    gate = manifest["source_gate"]
    path = gate["production_path"]
    content = git_reader(source_root, ["show", f"{commit}:{path}"])
    content.decode("utf-8", errors="strict")
    found = _legacy_route_violations(
        source_root, commit, git_reader=git_reader
    )
    return {
        "source_commit": commit,
        "path": path,
        "sha256": sha256_bytes(content),
        "forbidden_symbols_absent": not found,
        "forbidden_symbols_found": found,
    }


def _validate_candidate(value: Any) -> Mapping[str, Any]:
    candidate = _mapping(value, "candidate")
    fields = {
        "source_commit", "source_tree", "cargo_lock_sha256", "references_lock_sha256",
        "source_archive_sha256", "source_archive_size_bytes", "package_sha256",
        "performance_plugin_sha256", "performance_plugin_size_bytes", "embedded_git_commit",
        "build_date_utc",
    }
    _expect_keys(candidate, fields, "candidate")
    commit = _sha1(candidate["source_commit"], "candidate.source_commit")
    _sha1(candidate["source_tree"], "candidate.source_tree")
    for field in (
        "cargo_lock_sha256", "references_lock_sha256", "source_archive_sha256",
        "package_sha256", "performance_plugin_sha256",
    ):
        _sha256(candidate[field], f"candidate.{field}")
    _integer(candidate["source_archive_size_bytes"], "candidate.source_archive_size_bytes", minimum=1)
    _integer(candidate["performance_plugin_size_bytes"], "candidate.performance_plugin_size_bytes", minimum=1)
    if candidate["embedded_git_commit"] != commit[:12]:
        raise QualificationError("performance plugin is not bound to candidate source")
    if re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", str(candidate["build_date_utc"])) is None:
        raise QualificationError("candidate deterministic UTC build date is invalid")
    return candidate


def _validate_comparator(manifest: Mapping[str, Any], value: Any) -> Mapping[str, Any]:
    comparator = _mapping(value, "comparator")
    _expect_keys(comparator, {"bundle_sha256", "bundle_size_bytes", "references"}, "comparator")
    _sha256(comparator["bundle_sha256"], "comparator.bundle_sha256")
    _integer(comparator["bundle_size_bytes"], "comparator.bundle_size_bytes", minimum=1)
    references = _mapping(comparator["references"], "comparator.references")
    if set(references) != set(_expected_reference_commits()):
        raise QualificationError("comparator reference inventory is incomplete")
    for name, expected_commit in _expected_reference_commits().items():
        record = _mapping(references[name], f"comparator.references.{name}")
        _expect_keys(
            record,
            {"commit", "tree", "archive_sha256", "archive_size_bytes"},
            f"comparator.references.{name}",
        )
        if record["commit"] != expected_commit:
            raise QualificationError(f"comparator {name} does not match the pinned commit")
        _sha1(record["tree"], f"comparator.references.{name}.tree")
        _sha256(record["archive_sha256"], f"comparator.references.{name}.archive_sha256")
        _integer(record["archive_size_bytes"], f"comparator.references.{name}.archive_size_bytes", minimum=1)
    return comparator


def oracle_evidence_records(
    manifest: Mapping[str, Any], benchmark: Mapping[str, Any]
) -> list[dict[str, Any]]:
    """Bind top-level oracle receipts to the actual registered cell bundles.

    The receipt digest is derived from the embedded, independently computed
    oracle bundles.  It is therefore impossible to satisfy the qualification
    header with a syntactically valid but unrelated SHA-256 string.
    """

    cells = {
        item.get("cell_id"): item
        for item in benchmark.get("cells", [])
        if isinstance(item, Mapping) and isinstance(item.get("cell_id"), str)
    }
    required = manifest["matrix_contract"]["oracle_required_cell_ids"]
    bundles: list[dict[str, Any]] = []
    multiway: list[dict[str, Any]] = []
    dimensions: list[int] = []
    for cell_id in required:
        cell = _mapping(cells.get(cell_id), f"oracle benchmark cell {cell_id}")
        scientific = _mapping(cell.get("scientific"), f"{cell_id}.scientific")
        oracle = _mapping(scientific.get("oracle"), f"{cell_id}.scientific.oracle")
        if (
            oracle.get("schema_version") != 1
            or oracle.get("check_type") != "ppml-explicit-dummy-oracle-v1"
            or oracle.get("cell_id") != cell_id
            or oracle.get("complete_sentinel") != "FEPPML_ORACLE_CHECK_COMPLETE"
        ):
            raise QualificationError(f"registered oracle bundle identity is invalid: {cell_id}")
        record = {"cell_id": cell_id, "oracle": oracle}
        bundles.append(record)
        independent = _mapping(oracle.get("independent"), f"{cell_id}.oracle.independent")
        dof = _mapping(independent.get("dof"), f"{cell_id}.oracle.independent.dof")
        clusters = dof.get("cluster_counts")
        if not isinstance(clusters, list):
            raise QualificationError(f"registered oracle cluster inventory is invalid: {cell_id}")
        if clusters:
            dimensions.append(len(clusters))
            multiway.append(record)
    if dimensions != [1, 2, 3, 4, 5]:
        raise QualificationError(
            "registered oracle bundles do not cover ordered one-through-five-way clustering"
        )
    planned = {record["id"]: record for record in manifest["independent_oracles"]}
    if set(planned) != {"explicit_dummy", "multiway_cluster"}:
        raise QualificationError("independent oracle manifest inventory is not closed")
    cases = {
        "explicit_dummy": bundles,
        "multiway_cluster": multiway,
    }
    result = []
    for oracle_id in ("explicit_dummy", "multiway_cluster"):
        source = planned[oracle_id]
        records = cases[oracle_id]
        result.append(
            {
                "id": oracle_id,
                "path": source["path"],
                "sha256": source["sha256"],
                "program_path": manifest["oracle_program"]["path"],
                "program_sha256": manifest["oracle_program"]["sha256"],
                "evidence_sha256": sha256_bytes(canonical_json_bytes(records)),
                "case_ids": [record["cell_id"] for record in records],
            }
        )
    return result


def _validate_oracles(
    manifest: Mapping[str, Any], value: Any, benchmark: Mapping[str, Any]
) -> None:
    if not isinstance(value, list):
        raise QualificationError("oracle evidence must be a list")
    observed = []
    for index, raw in enumerate(value):
        record = dict(_mapping(raw, f"oracle[{index}]"))
        _expect_keys(
            record,
            {
                "id", "path", "sha256", "program_path", "program_sha256",
                "evidence_sha256", "case_ids",
            },
            f"oracle[{index}]",
        )
        _sha256(record["sha256"], f"oracle[{index}].sha256")
        _sha256(record["program_sha256"], f"oracle[{index}].program_sha256")
        _sha256(record["evidence_sha256"], f"oracle[{index}].evidence_sha256")
        observed.append(record)
    if observed != oracle_evidence_records(manifest, benchmark):
        raise QualificationError(
            "oracle evidence records are not bound to the registered cell bundles"
        )


def _validate_source_audit(manifest: Mapping[str, Any], candidate: Mapping[str, Any], value: Any) -> None:
    audit = _mapping(value, "source_audit")
    _expect_keys(
        audit,
        {"source_commit", "path", "sha256", "forbidden_symbols_absent", "forbidden_symbols_found"},
        "source_audit",
    )
    if audit["source_commit"] != candidate["source_commit"] or audit["path"] != manifest["source_gate"]["production_path"]:
        raise QualificationError("source audit is not bound to the candidate transaction")
    _sha256(audit["sha256"], "source_audit.sha256")
    if audit["forbidden_symbols_absent"] is not True or audit["forbidden_symbols_found"] != []:
        raise QualificationError("production Rust still exposes a forbidden stale PPML route")


def _validate_platforms(
    manifest: Mapping[str, Any],
    value: Any,
    candidate: Mapping[str, Any] | None = None,
    *,
    verifier: Callable[[Mapping[str, Any]], Mapping[str, Any]] | None = None,
) -> None:
    try:
        from scripts import feppml_platform_load_receipt as platform_receipts
    except ImportError:
        import feppml_platform_load_receipt as platform_receipts  # type: ignore[no-redef]
    verify_record = platform_receipts.verify_evidence if verifier is None else verifier
    if not isinstance(value, list):
        raise QualificationError("platform_artifacts must be a list")
    expected = manifest["platform_gates"]["required_artifacts"]
    if [item.get("platform") for item in value if isinstance(item, Mapping)] != expected:
        raise QualificationError("platform artifact inventory or order is incomplete")
    for raw in value:
        record = _mapping(raw, "platform artifact")
        try:
            payload = verify_record(record)
        except platform_receipts.PlatformReceiptError as error:
            raise QualificationError(
                f"authoritative platform receipt failed: {record.get('platform')}: {error}"
            ) from error
        platform = payload["platform"]
        if platform != record.get("platform"):
            raise QualificationError("platform verifier returned another platform")
        artifact = _mapping(record.get("artifact"), f"platform {platform}.artifact")
        program = _mapping(
            record.get("capture_program"), f"platform {platform}.capture_program"
        )
        if (
            payload.get("qualification_profile") != "candidate"
            or record.get("qualification_profile") != "candidate"
            or payload["artifact_sha256"] != artifact.get("sha256")
            or payload["artifact_size_bytes"] != artifact.get("size_bytes")
            or payload["capture_program_sha256"] != program.get("sha256")
            or payload["capture_program_sha256"]
                != manifest["platform_gates"]["capture_program"]["sha256"]
            or payload["licensed_stata_load"] is not True
            or payload["mock_only"] is not False
            or payload["scientific_claim"] is not False
            or payload["receipt_scope"] != "artifact_and_licensed_stata_load_only"
            or payload["complete_sentinel"]
                != "FEPPML_PLATFORM_LOAD_RECEIPT_COMPLETE"
        ):
            raise QualificationError(
                f"platform verifier payload is not qualification eligible: {platform}"
            )
        if candidate is not None:
            expected_source = {
                "source_commit": candidate["source_commit"],
                "source_tree": candidate["source_tree"],
                "source_archive_sha256": candidate["source_archive_sha256"],
                "source_archive_size_bytes": candidate["source_archive_size_bytes"],
                "embedded_git_commit": candidate["embedded_git_commit"],
                "build_date_utc": candidate["build_date_utc"],
            }
            if any(payload[field] != expected_value for field, expected_value in expected_source.items()):
                raise QualificationError(
                    f"platform artifact is stale relative to candidate: {platform}"
                )
            if platform != "windows-x86_64" and (
                payload["controller_commit"] != candidate["source_commit"]
                or payload["controller_tree"] != candidate["source_tree"]
            ):
                raise QualificationError(
                    f"platform controller is stale relative to candidate: {platform}"
                )
            provenance = payload["platform_provenance"]
            if isinstance(provenance, Mapping):
                for commit_field, tree_field in (
                    ("harness_commit", "harness_tree"),
                    ("controller_commit", "controller_tree"),
                ):
                    if commit_field in provenance and (
                        provenance.get(commit_field) != candidate["source_commit"]
                        or provenance.get(tree_field) != candidate["source_tree"]
                    ):
                        raise QualificationError(
                            f"platform provenance uses a stale harness: {platform}"
                        )
            if platform == "linux-x86_64" and (
                payload["artifact_sha256"] != candidate["performance_plugin_sha256"]
                or payload["artifact_size_bytes"]
                    != candidate["performance_plugin_size_bytes"]
            ):
                raise QualificationError(
                    "Linux platform artifact differs from the performance plugin"
                )


MEASUREMENT_FIELDS = {
    "seconds", "peak_rss_bytes", "process_id", "node_id", "stata_version",
    "artifact_sha256", "fresh_process",
}


def _measurement(value: Any, where: str, artifact_sha256: str) -> Mapping[str, Any]:
    record = _mapping(value, where)
    _expect_keys(record, MEASUREMENT_FIELDS, where)
    _number(record["seconds"], f"{where}.seconds", positive=True)
    _integer(record["peak_rss_bytes"], f"{where}.peak_rss_bytes", minimum=1)
    _integer(record["process_id"], f"{where}.process_id", minimum=1)
    for field in ("node_id", "stata_version"):
        if not isinstance(record[field], str) or not record[field]:
            raise QualificationError(f"{where}.{field} must be nonempty")
    if record["artifact_sha256"] != artifact_sha256:
        raise QualificationError(f"{where} used the wrong arm artifact")
    if record["fresh_process"] is not True:
        raise QualificationError(f"{where} was not a fresh process")
    return record


def _pair(
    value: Any,
    where: str,
    *,
    expected_index: int,
    expected_order: str,
    candidate_artifact: str,
    comparator_artifact: str,
) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    pair = _mapping(value, where)
    _expect_keys(pair, {"index", "order", "candidate", "comparator"}, where)
    if pair["index"] != expected_index or pair["order"] != expected_order:
        raise QualificationError(f"{where} index/order differs from the frozen cadence")
    candidate = _measurement(pair["candidate"], f"{where}.candidate", candidate_artifact)
    comparator = _measurement(pair["comparator"], f"{where}.comparator", comparator_artifact)
    if candidate["node_id"] != comparator["node_id"] or candidate["stata_version"] != comparator["stata_version"]:
        raise QualificationError(f"{where} arms were not measured on the same node/Stata")
    return candidate, comparator


def _paired_metrics(pairs: Sequence[tuple[Mapping[str, Any], Mapping[str, Any]]]) -> dict[str, float]:
    logs = [math.log(float(candidate["seconds"]) / float(comparator["seconds"])) for candidate, comparator in pairs]
    mean = statistics.fmean(logs)
    standard_error = statistics.stdev(logs) / math.sqrt(len(logs)) if len(logs) > 1 else 0.0
    t95 = 2.306004135 if len(logs) == 9 else 0.0
    return {
        "geometric_mean_ratio": math.exp(mean),
        "log_ratio_mean": mean,
        "log_ratio_standard_error": standard_error,
        "ratio_ci95_lower": math.exp(mean - t95 * standard_error),
        "ratio_ci95_upper": math.exp(mean + t95 * standard_error),
    }


def _validate_pairs(
    manifest: Mapping[str, Any],
    candidate: Mapping[str, Any],
    comparator: Mapping[str, Any],
    value: Any,
    benchmark_evidence: Mapping[str, Any],
) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        raise QualificationError("paired_cells must be a list")
    expected_ids = manifest["matrix_contract"]["expected_cell_ids"]
    if [item.get("cell_id") for item in value if isinstance(item, Mapping)] != expected_ids:
        raise QualificationError("paired cell inventory or order differs from the frozen matrix")
    benchmark_cells = {
        item["cell_id"]: item
        for item in benchmark_evidence.get("cells", [])
        if isinstance(item, Mapping) and isinstance(item.get("cell_id"), str)
    }
    if set(benchmark_cells) != set(expected_ids):
        raise QualificationError("paired timing cannot be bound to the benchmark cell inventory")

    summaries: list[dict[str, Any]] = []
    candidate_artifact = candidate["performance_plugin_sha256"]
    comparator_artifact = comparator["bundle_sha256"]
    order = manifest["capture_contract"]["warm_pair_order"]
    for raw in value:
        record = _mapping(raw, "paired cell")
        _expect_keys(record, {"cell_id", "cold_pair", "warm_pairs"}, f"paired cell {record.get('cell_id')}")
        cell_id = record["cell_id"]
        cold = _pair(
            record["cold_pair"], f"{cell_id}.cold_pair",
            expected_index=0, expected_order="AB",
            candidate_artifact=candidate_artifact, comparator_artifact=comparator_artifact,
        )
        warm_values = record["warm_pairs"]
        if not isinstance(warm_values, list) or len(warm_values) != 9:
            raise QualificationError(f"{cell_id} must contain exactly nine warm pairs")
        warm = [
            _pair(
                pair, f"{cell_id}.warm_pairs[{index - 1}]",
                expected_index=index, expected_order=order[index - 1],
                candidate_artifact=candidate_artifact, comparator_artifact=comparator_artifact,
            )
            for index, pair in enumerate(warm_values, 1)
        ]
        processes = [item["process_id"] for pair in (cold, *warm) for item in pair]
        if len(processes) != len(set(processes)):
            raise QualificationError(f"{cell_id} reused a process across arm measurements")

        benchmark_cell = benchmark_cells[cell_id]
        timing = _mapping(benchmark_cell.get("timing"), f"benchmark {cell_id}.timing")
        candidate_timing = _mapping(timing.get("candidate"), f"benchmark {cell_id}.timing.candidate")
        comparator_timing = _mapping(timing.get("comparator"), f"benchmark {cell_id}.timing.comparator")
        if (
            candidate_timing.get("cold_seconds") != cold[0]["seconds"]
            or comparator_timing.get("cold_seconds") != cold[1]["seconds"]
            or candidate_timing.get("warm_seconds") != [pair[0]["seconds"] for pair in warm]
            or comparator_timing.get("warm_seconds") != [pair[1]["seconds"] for pair in warm]
        ):
            raise QualificationError(f"{cell_id} paired raw timing differs from benchmark evidence")
        execution = _mapping(
            benchmark_cell.get("execution"), f"benchmark {cell_id}.execution"
        )
        expected_candidate_pids = [cold[0]["process_id"], *[pair[0]["process_id"] for pair in warm]]
        expected_comparator_pids = [cold[1]["process_id"], *[pair[1]["process_id"] for pair in warm]]
        expected_arm_order: list[list[str]] = []
        for pair_record in (record["cold_pair"], *record["warm_pairs"]):
            expected_arm_order.append(
                ["candidate", "comparator"]
                if pair_record["order"] == "AB"
                else ["comparator", "candidate"]
            )
        if (
            execution.get("candidate_process_ids") != expected_candidate_pids
            or execution.get("comparator_process_ids") != expected_comparator_pids
            or execution.get("arm_order") != expected_arm_order
        ):
            raise QualificationError(
                f"{cell_id} benchmark execution differs from paired raw process/order evidence"
            )
        memory = _mapping(benchmark_cell.get("memory"), f"benchmark {cell_id}.memory")
        observed_candidate_rss = max(
            item["peak_rss_bytes"] for item in (cold[0], *[pair[0] for pair in warm])
        )
        if memory.get("process_peak_rss_bytes") != observed_candidate_rss:
            raise QualificationError(
                f"{cell_id} benchmark process RSS differs from paired raw evidence"
            )
        summaries.append({"cell_id": cell_id, **_paired_metrics(warm)})
    return summaries


def _validate_benchmark_identity(
    candidate: Mapping[str, Any],
    comparator: Mapping[str, Any],
    benchmark: Mapping[str, Any],
    development_manifest_sha256: str,
) -> None:
    identity = _mapping(benchmark.get("identity"), "benchmark_evidence.identity")
    _expect_keys(
        identity,
        {
            "source_commit", "source_tree", "cargo_lock_sha256",
            "references_lock_sha256", "manifest_sha256", "generator_sha256",
            "reference_commits",
        },
        "benchmark_evidence.identity",
    )
    bindings = {
        "source_commit": candidate["source_commit"],
        "source_tree": candidate["source_tree"],
        "cargo_lock_sha256": candidate["cargo_lock_sha256"],
        "references_lock_sha256": candidate["references_lock_sha256"],
        "manifest_sha256": development_manifest_sha256,
        "reference_commits": {
            name: record["commit"] for name, record in comparator["references"].items()
        },
    }
    for field, expected in bindings.items():
        if identity.get(field) != expected:
            raise QualificationError(
                f"benchmark identity is not bound to qualification {field}"
            )
    _sha256(identity["generator_sha256"], "benchmark identity.generator_sha256")


def _validate_pair_environment(
    benchmark: Mapping[str, Any],
    paired_cells: Any,
    candidate: Mapping[str, Any],
    comparator: Mapping[str, Any],
) -> None:
    if not isinstance(paired_cells, list):
        raise QualificationError("paired_cells must be a list")
    benchmark_cells = {
        item.get("cell_id"): item
        for item in benchmark.get("cells", [])
        if isinstance(item, Mapping)
    }
    for cell in paired_cells:
        record = _mapping(cell, "paired cell")
        measurements: list[Mapping[str, Any]] = []
        cold = _mapping(record.get("cold_pair"), "cold pair")
        measurements.extend(
            [_mapping(cold.get("candidate"), "cold candidate"), _mapping(cold.get("comparator"), "cold comparator")]
        )
        warm = record.get("warm_pairs")
        if not isinstance(warm, list):
            raise QualificationError("warm_pairs must be a list")
        for pair in warm:
            item = _mapping(pair, "warm pair")
            measurements.extend(
                [_mapping(item.get("candidate"), "warm candidate"), _mapping(item.get("comparator"), "warm comparator")]
            )
        cell_id = record.get("cell_id")
        benchmark_cell = _mapping(
            benchmark_cells.get(cell_id), f"benchmark cell {cell_id}"
        )
        execution = _mapping(
            benchmark_cell.get("execution"), f"benchmark cell {cell_id}.execution"
        )
        observed_nodes = {item.get("node_id") for item in measurements}
        observed_stata = {item.get("stata_version") for item in measurements}
        if (
            observed_nodes != {execution.get("node_id")}
            or observed_stata != {execution.get("stata_version")}
            or execution.get("plugin_sha256")
                != candidate["performance_plugin_sha256"]
            or execution.get("comparator_bundle_sha256")
                != comparator["bundle_sha256"]
        ):
            raise QualificationError(
                f"{cell_id} paired environment/artifacts differ from cell execution identity"
            )


def _load_development(
    qualification_manifest: Mapping[str, Any], *, root: Path = ROOT
) -> tuple[Any, Mapping[str, Any], str]:
    try:
        from scripts import ppml_memory_performance as development
    except ImportError:
        import ppml_memory_performance as development  # type: ignore[no-redef]
    frozen = _mapping(
        qualification_manifest["matrix_contract"]["development_manifest"],
        "matrix_contract.development_manifest",
    )
    path = root / frozen["path"]
    digest = sha256_file(path)
    if digest != frozen["sha256"]:
        raise QualificationError(
            "development manifest differs from the frozen qualification input"
        )
    manifest = development.load_manifest(path)
    return development, manifest, digest


def _validate_matrix_against_development(manifest: Mapping[str, Any], development: Any, dev_manifest: Mapping[str, Any]) -> None:
    cells = development.expand_cells(dev_manifest)
    ids = [cell["cell_id"] for cell in cells]
    if ids != manifest["matrix_contract"]["expected_cell_ids"]:
        raise QualificationError("development capture matrix differs from the frozen qualification matrix")
    oracle_ids = [cell["cell_id"] for cell in cells if cell["oracle_required"]]
    if oracle_ids != manifest["matrix_contract"]["oracle_required_cell_ids"]:
        raise QualificationError("development oracle inventory differs from qualification")


def evaluate_evidence(
    manifest: Mapping[str, Any],
    evidence: Mapping[str, Any],
    *,
    manifest_sha256: str | None = None,
    development_root: Path = ROOT,
    platform_verifier: Callable[
        [Mapping[str, Any]], Mapping[str, Any]
    ] | None = None,
) -> dict[str, Any]:
    validate_manifest(manifest)
    manifest_sha256 = manifest_sha256 or sha256_bytes(canonical_json_bytes(manifest))
    evidence = _mapping(evidence, "evidence")
    _expect_keys(
        evidence,
        {
            "schema_version", "evidence_type", "qualification_manifest", "candidate",
            "comparator", "oracles", "source_audit", "platform_artifacts",
            "paired_cells", "benchmark_evidence", "complete_sentinel",
        },
        "evidence",
    )
    if (
        evidence["schema_version"] != 1
        or evidence["evidence_type"] != "feppml-qualification-v1-evidence"
        or evidence["complete_sentinel"] != "FEPPML_QUALIFICATION_COMPLETE"
    ):
        raise QualificationError("qualification evidence identity/sentinel is invalid")
    binding = _mapping(evidence["qualification_manifest"], "qualification_manifest")
    _expect_keys(binding, {"id", "sha256"}, "qualification_manifest")
    if binding != {"id": manifest["manifest_id"], "sha256": manifest_sha256}:
        raise QualificationError("evidence is bound to a different qualification manifest")

    candidate = _validate_candidate(evidence["candidate"])
    if candidate["references_lock_sha256"] != manifest["reference_closure"]["references_lock_sha256"]:
        raise QualificationError("candidate references.lock.toml differs from the frozen closure")
    comparator = _validate_comparator(manifest, evidence["comparator"])
    _validate_source_audit(manifest, candidate, evidence["source_audit"])
    _validate_platforms(
        manifest,
        evidence["platform_artifacts"],
        candidate,
        verifier=platform_verifier,
    )

    development, dev_manifest, dev_manifest_sha = _load_development(
        manifest, root=development_root
    )
    _validate_matrix_against_development(manifest, development, dev_manifest)

    # The development evaluator intentionally reports otherwise-passing
    # evidence as blocked solely because its own manifest is non-qualifying.
    # Crucially, it finishes every scientific comparison before it validates
    # timing.  Preserve that stage boundary here.
    benchmark = _mapping(evidence["benchmark_evidence"], "benchmark_evidence")
    evaluation = development.evaluate_evidence(
        dev_manifest, benchmark, manifest_sha256=dev_manifest_sha
    )
    if evaluation["stage"] == "scientific":
        return {
            "schema_version": 1,
            "kind": "feppml_qualification_evaluation",
            "status": "fail",
            "stage": "scientific",
            "performance_evaluated": False,
            "reasons": list(evaluation["reasons"]),
            "manifest_id": manifest["manifest_id"],
            "manifest_sha256": manifest_sha256,
            "candidate_source_commit": candidate["source_commit"],
            "cells": [],
        }

    _validate_oracles(manifest, evidence["oracles"], benchmark)

    allowed_block = evaluation["status"] == "blocked" and evaluation["reasons"] == [
        "development_manifest_not_qualification_eligible"
    ]
    if evaluation["status"] not in {"pass", "blocked"} or (
        evaluation["status"] == "blocked" and not allowed_block
    ):
        return {
            "schema_version": 1,
            "kind": "feppml_qualification_evaluation",
            "status": "fail",
            "stage": "performance",
            "performance_evaluated": True,
            "reasons": list(evaluation["reasons"]),
            "manifest_id": manifest["manifest_id"],
            "manifest_sha256": manifest_sha256,
            "candidate_source_commit": candidate["source_commit"],
            "cells": [],
        }

    _validate_benchmark_identity(
        candidate, comparator, benchmark, dev_manifest_sha
    )
    _validate_pair_environment(
        benchmark, evidence["paired_cells"], candidate, comparator
    )
    pair_summaries = _validate_pairs(
        manifest, candidate, comparator, evidence["paired_cells"], benchmark
    )
    development_by_id = {item["cell_id"]: item for item in evaluation["cells"]}
    primary = [
        item for item in pair_summaries
        if development_by_id[item["cell_id"]]["performance_role"] == "primary"
    ]
    primary_ratios = [item["geometric_mean_ratio"] for item in primary]
    geometric_mean = math.exp(statistics.fmean(math.log(value) for value in primary_ratios))
    twice_share = sum(
        value <= manifest["performance_gates"]["maximum_twice_as_fast_ratio"]
        for value in primary_ratios
    ) / len(primary_ratios)
    reasons: list[str] = []
    if geometric_mean > manifest["performance_gates"]["maximum_equal_cell_geometric_mean_ratio"]:
        reasons.append("paired_governing_geometric_mean_above_0.40")
    if twice_share < manifest["performance_gates"]["minimum_twice_as_fast_share"]:
        reasons.append("paired_governing_twice_as_fast_share_below_0.80")
    for item in primary:
        if (
            development_by_id[item["cell_id"]]["requested_threads"] == 1
            and item["geometric_mean_ratio"]
            > manifest["performance_gates"]["maximum_registered_single_thread_ratio"]
        ):
            reasons.append(f"{item['cell_id']}:paired_single_thread_ratio_above_two_thirds")

    return {
        "schema_version": 1,
        "kind": "feppml_qualification_evaluation",
        "status": "pass" if not reasons else "fail",
        "stage": "complete",
        "performance_evaluated": True,
        "reasons": reasons,
        "manifest_id": manifest["manifest_id"],
        "manifest_sha256": manifest_sha256,
        "candidate_source_commit": candidate["source_commit"],
        "governing_cells": len(primary),
        "paired_governing_equal_cell_geometric_mean_ratio": geometric_mean,
        "paired_governing_twice_as_fast_share": twice_share,
        "cells": pair_summaries,
    }


def seal_payload(payload: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "payload": payload,
        "seal": {
            "algorithm": "sha256",
            "canonicalization": "json-sort-keys-compact-ascii-newline-v1",
            "payload_sha256": sha256_bytes(canonical_json_bytes(payload)),
        },
    }


def write_receipt(path: Path, payload: Mapping[str, Any]) -> None:
    document = seal_payload(payload)
    data = json.dumps(document, sort_keys=True, indent=2, ensure_ascii=True).encode("ascii") + b"\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise QualificationError(f"refusing to overwrite receipt: {path}") from error
    try:
        with os.fdopen(descriptor, "wb") as target:
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
    except BaseException:
        try:
            path.unlink()
        except OSError:
            pass
        raise


def verify_receipt(path: Path) -> Mapping[str, Any]:
    """Verify only the immutable JSON envelope.

    This helper is intentionally insufficient for activation.  Use
    :func:`verify_qualification_receipt` to recompute the complete evaluation
    from its committed manifest and evidence.
    """

    document = _read_json(path, "qualification receipt")
    _expect_keys(document, {"payload", "seal"}, "qualification receipt")
    payload = _mapping(document["payload"], "qualification receipt payload")
    if document["seal"] != seal_payload(payload)["seal"]:
        raise QualificationError("qualification receipt seal mismatch")
    if payload.get("kind") != "feppml_qualification_evaluation":
        raise QualificationError("qualification receipt has the wrong result kind")
    return payload


def _verify_committed_candidate(
    source_root: Path,
    manifest: Mapping[str, Any],
    evidence: Mapping[str, Any],
    *,
    manifest_path: Path | None = None,
    source_archive_path: Path | None = None,
) -> None:
    """Reconstruct the candidate identity from committed bytes.

    The ordinary evaluator validates the closed evidence schema.  Activation
    additionally needs to know that its source/tree/archive and source audit
    were not merely internally consistent strings in that schema.
    """

    candidate = _validate_candidate(evidence.get("candidate"))
    commit = candidate["source_commit"]
    if source_archive_path is not None or not (source_root / ".git").exists():
        if source_archive_path is None:
            inferred = source_root.parent.parent / "input/archives/candidate.tar"
            source_archive_path = inferred
        _verify_candidate_archive_tree(
            source_root,
            source_archive_path,
            manifest,
            evidence,
            manifest_path=manifest_path,
        )
        return
    observed_commit = _git_bytes(
        source_root, ["rev-parse", f"{commit}^{{commit}}"]
    ).decode("ascii").strip()
    observed_tree = _git_bytes(
        source_root, ["rev-parse", f"{commit}^{{tree}}"]
    ).decode("ascii").strip()
    if observed_commit != commit or observed_tree != candidate["source_tree"]:
        raise QualificationError("candidate commit/tree differs from committed source")
    committed_bindings = {
        "cargo_lock_sha256": sha256_bytes(
            _git_bytes(source_root, ["show", f"{commit}:Cargo.lock"])
        ),
        "references_lock_sha256": sha256_bytes(
            _git_bytes(source_root, ["show", f"{commit}:references.lock.toml"])
        ),
    }
    for field, expected in committed_bindings.items():
        if candidate[field] != expected:
            raise QualificationError(
                f"candidate {field} differs from committed source"
            )
    development = _mapping(
        manifest["matrix_contract"]["development_manifest"],
        "matrix_contract.development_manifest",
    )
    committed_development = _git_bytes(
        source_root, ["show", f"{commit}:{development['path']}"]
    )
    if sha256_bytes(committed_development) != development["sha256"]:
        raise QualificationError(
            "frozen development manifest differs from committed candidate bytes"
        )
    if manifest_path is not None:
        committed_qualification = _git_bytes(
            source_root,
            ["show", f"{commit}:benchmarks/manifests/feppml_qualification_v1.json"],
        )
        try:
            supplied_qualification = manifest_path.read_bytes()
        except OSError as error:
            raise QualificationError(
                f"unable to read committed qualification manifest: {error}"
            ) from error
        if supplied_qualification != committed_qualification:
            raise QualificationError(
                "qualification manifest differs from committed candidate bytes"
            )
    archive = _git_bytes(source_root, ["archive", "--format=tar", commit])
    if (
        sha256_bytes(archive) != candidate["source_archive_sha256"]
        or len(archive) != candidate["source_archive_size_bytes"]
    ):
        raise QualificationError(
            "candidate source archive identity differs from exact git archive"
        )
    observed_audit = audit_source(source_root, commit, manifest)
    if evidence.get("source_audit") != observed_audit:
        raise QualificationError(
            "source audit differs from a fresh audit of the committed candidate"
        )


def _git_object_id(kind: str, data: bytes) -> bytes:
    return hashlib.sha1(
        f"{kind} {len(data)}\0".encode("ascii") + data
    ).digest()


def _archive_tree_and_files(
    archive: bytes,
) -> tuple[str, str, dict[str, tuple[bytes, bool]]]:
    files: dict[str, tuple[bytes, bool]] = {}
    commit = ""
    try:
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as source:
            commit = str(source.pax_headers.get("comment", ""))
            if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
                raise QualificationError(
                    "candidate archive omits its exact Git commit PAX header"
                )
            for member in source.getmembers():
                relative = Path(member.name.rstrip("/"))
                if relative.is_absolute() or ".." in relative.parts:
                    raise QualificationError("candidate archive contains an unsafe path")
                if member.isdir():
                    continue
                if member.issym() or member.islnk():
                    raise QualificationError(
                        "candidate archive symlinks/hardlinks are forbidden"
                    )
                if member.isfile():
                    opened = source.extractfile(member)
                    if opened is None:
                        raise QualificationError("candidate archive file is unreadable")
                    data = opened.read()
                    executable = bool(member.mode & 0o111)
                    mode = "100755" if executable else "100644"
                else:
                    raise QualificationError("candidate archive contains a special entry")
                name = relative.as_posix()
                if not name or name in files:
                    raise QualificationError("candidate archive has duplicate paths")
                files[name] = (data, executable)
                files[name + "\0mode"] = (mode.encode("ascii"), False)
    except tarfile.TarError as error:
        raise QualificationError("candidate source archive is invalid") from error

    root: dict[str, Any] = {}
    for name, (data, _executable) in files.items():
        if name.endswith("\0mode"):
            continue
        pieces = name.split("/")
        node = root
        for piece in pieces[:-1]:
            child = node.setdefault(piece, {})
            if not isinstance(child, dict):
                raise QualificationError("candidate archive tree has a path collision")
            node = child
        mode = files[name + "\0mode"][0].decode("ascii")
        node[pieces[-1]] = (mode, _git_object_id("blob", data))

    def tree_id(node: Mapping[str, Any]) -> bytes:
        entries: list[tuple[str, str, bytes, bool]] = []
        for name, value in node.items():
            if isinstance(value, dict):
                entries.append((name, "40000", tree_id(value), True))
            else:
                mode, object_id = value
                entries.append((name, mode, object_id, False))
        body = b"".join(
            mode.encode("ascii") + b" " + name.encode("utf-8") + b"\0" + object_id
            for name, mode, object_id, is_tree in sorted(
                entries, key=lambda item: (item[0] + ("/" if item[3] else "")).encode("utf-8")
            )
        )
        return _git_object_id("tree", body)

    clean_files = {
        name: value for name, value in files.items() if not name.endswith("\0mode")
    }
    return commit, tree_id(root).hex(), clean_files


def _verify_extracted_archive_inventory(
    source_root: Path, files: Mapping[str, tuple[bytes, bool]]
) -> None:
    """Require the extracted source tree to be exactly the archive, no extras."""

    expected_files = set(files)
    expected_directories: set[str] = set()
    for relative in expected_files:
        parent = Path(relative).parent
        while parent != Path("."):
            expected_directories.add(parent.as_posix())
            parent = parent.parent
    observed_files: set[str] = set()
    observed_directories: set[str] = set()
    if source_root.is_symlink() or not source_root.is_dir():
        raise QualificationError("extracted candidate source root is unsafe")
    for path in source_root.rglob("*"):
        relative = path.relative_to(source_root).as_posix()
        if path.is_symlink():
            raise QualificationError(
                f"extracted candidate source contains a symlink: {relative}"
            )
        if path.is_file():
            observed_files.add(relative)
        elif path.is_dir():
            observed_directories.add(relative)
        else:
            raise QualificationError(
                f"extracted candidate source contains a special entry: {relative}"
            )
    if observed_files != expected_files or observed_directories != expected_directories:
        extra = sorted(
            (observed_files - expected_files)
            | (observed_directories - expected_directories)
        )
        missing = sorted(
            (expected_files - observed_files)
            | (expected_directories - observed_directories)
        )
        raise QualificationError(
            "extracted candidate inventory differs from archive: "
            f"extra={extra[:5]} missing={missing[:5]}"
        )


def _verify_candidate_archive_tree(
    source_root: Path,
    source_archive_path: Path,
    manifest: Mapping[str, Any],
    evidence: Mapping[str, Any],
    *,
    manifest_path: Path | None,
) -> None:
    candidate = _validate_candidate(evidence.get("candidate"))
    try:
        archive = source_archive_path.read_bytes()
    except OSError as error:
        raise QualificationError("candidate source archive is unavailable") from error
    if (
        sha256_bytes(archive) != candidate["source_archive_sha256"]
        or len(archive) != candidate["source_archive_size_bytes"]
    ):
        raise QualificationError("candidate source archive bytes differ from evidence")
    commit, tree, files = _archive_tree_and_files(archive)
    if commit != candidate["source_commit"]:
        raise QualificationError("candidate archive commit identity is invalid")
    if tree != candidate["source_tree"]:
        raise QualificationError("candidate archive tree differs from evidence")
    _verify_extracted_archive_inventory(source_root, files)
    for relative, (data, executable) in files.items():
        path = source_root / relative
        if path.is_symlink() or not path.is_file() or path.read_bytes() != data:
            raise QualificationError(f"extracted candidate source changed: {relative}")
        if bool(path.stat().st_mode & 0o111) != executable:
            raise QualificationError(f"extracted candidate mode changed: {relative}")
    committed = {
        "cargo_lock_sha256": sha256_bytes(files["Cargo.lock"][0]),
        "references_lock_sha256": sha256_bytes(files["references.lock.toml"][0]),
    }
    if any(candidate[field] != expected for field, expected in committed.items()):
        raise QualificationError("candidate lock digest differs from archive")
    development = _mapping(
        manifest["matrix_contract"]["development_manifest"],
        "matrix_contract.development_manifest",
    )
    if sha256_bytes(files[development["path"]][0]) != development["sha256"]:
        raise QualificationError("development manifest differs from candidate archive")
    if manifest_path is not None and manifest_path.read_bytes() != files[
        "benchmarks/manifests/feppml_qualification_v1.json"
    ][0]:
        raise QualificationError("qualification manifest differs from candidate archive")
    gate_path = manifest["source_gate"]["production_path"]
    production = files[gate_path][0]
    documents = {
        path: data.decode("utf-8", errors="strict")
        for path, (data, _mode) in files.items()
        if path.endswith(".rs")
        and path.startswith(("crates/fereg-core/src/", "crates/fereg-plugin/src/"))
    }
    found = [str(value) for value in audit_ppml_legacy_documents(documents)]
    observed_audit = {
        "source_commit": candidate["source_commit"],
        "path": gate_path,
        "sha256": sha256_bytes(production),
        "forbidden_symbols_absent": not found,
        "forbidden_symbols_found": found,
    }
    if evidence.get("source_audit") != observed_audit:
        raise QualificationError("source audit differs from candidate archive")


def verify_qualification_receipt(
    manifest_path: Path,
    evidence_path: Path,
    receipt_path: Path,
    *,
    source_root: Path = ROOT,
    source_archive_path: Path | None = None,
    platform_verifier: Callable[
        [Mapping[str, Any]], Mapping[str, Any]
    ] | None = None,
) -> Mapping[str, Any]:
    """Authoritatively recompute and verify one passing qualification receipt.

    This validates real platform prerequisite bytes and reconstructs the
    candidate git tree/archive.  The outer SCC lifecycle verifier remains
    responsible for proving that the evidence came from the sealed 33-task
    run and its successful qacct records.
    """

    manifest_path = manifest_path.resolve()
    evidence_path = evidence_path.resolve()
    receipt_path = receipt_path.resolve()
    source_root = source_root.resolve()
    manifest = load_manifest(manifest_path, root=source_root)
    evidence = _read_json(evidence_path, "qualification evidence")
    recomputed = evaluate_evidence(
        manifest,
        evidence,
        manifest_sha256=sha256_file(manifest_path),
        development_root=source_root,
        platform_verifier=platform_verifier,
    )
    stored = verify_receipt(receipt_path)
    if stored != recomputed:
        raise QualificationError(
            "qualification receipt differs from a fresh evidence evaluation"
        )
    if (
        recomputed.get("status") != "pass"
        or recomputed.get("stage") != "complete"
        or recomputed.get("performance_evaluated") is not True
        or recomputed.get("reasons") != []
    ):
        raise QualificationError(
            "qualification receipt is not a passing complete evaluation"
        )
    _verify_committed_candidate(
        source_root,
        manifest,
        evidence,
        manifest_path=manifest_path,
        source_archive_path=source_archive_path,
    )
    return recomputed


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("validate-manifest")
    audit = subparsers.add_parser("audit-source")
    audit.add_argument("--source-root", type=Path, required=True)
    audit.add_argument("--commit", required=True)
    evaluate = subparsers.add_parser("evaluate")
    evaluate.add_argument("--evidence", type=Path, required=True)
    evaluate.add_argument("--receipt", type=Path, required=True)
    verify = subparsers.add_parser("verify-receipt")
    verify.add_argument("--receipt", type=Path, required=True)
    authoritative = subparsers.add_parser("verify-qualification")
    authoritative.add_argument("--evidence", type=Path, required=True)
    authoritative.add_argument("--receipt", type=Path, required=True)
    authoritative.add_argument("--source-root", type=Path, default=ROOT)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    manifest = load_manifest(args.manifest)
    if args.command == "validate-manifest":
        print(f"FEPPML_QUALIFICATION_MANIFEST_OK|{manifest['manifest_id']}")
        return 0
    if args.command == "audit-source":
        result = audit_source(args.source_root, args.commit, manifest)
        print(json.dumps(result, sort_keys=True))
        return 0 if result["forbidden_symbols_absent"] else 1
    if args.command == "evaluate":
        evidence = _read_json(args.evidence, "qualification evidence")
        result = evaluate_evidence(manifest, evidence, manifest_sha256=sha256_file(args.manifest))
        write_receipt(args.receipt, result)
        print(f"FEPPML_QUALIFICATION_{result['status'].upper()}|{result['stage']}")
        return 0 if result["status"] == "pass" else 1
    if args.command == "verify-receipt":
        result = verify_receipt(args.receipt)
        print(f"FEPPML_QUALIFICATION_RECEIPT_ENVELOPE_OK|{result['status']}")
        return 0
    result = verify_qualification_receipt(
        args.manifest,
        args.evidence,
        args.receipt,
        source_root=args.source_root,
    )
    print(f"FEPPML_QUALIFICATION_RECEIPT_OK|{result['status']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
