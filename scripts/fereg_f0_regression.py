#!/usr/bin/env python3
"""Evaluate direct fereg candidate-versus-F0 regression evidence.

The module deliberately does not run Stata. Platform runners produce the
closed evidence schema frozen in ``fereg_f0_regression_v1.json``; this module
then validates identities, applies the scientific gate before inspecting any
timing, evaluates paired performance and memory, and writes a sealed receipt.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import statistics
import subprocess
import sys
from typing import Any, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "benchmarks/manifests/fereg_f0_regression_v1.json"
HEX_SHA1 = re.compile(r"^[0-9a-f]{40}$")
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
LOCAL_SCREEN_CELL_IDS = (
    "local_n100k_dense_two_fe_t1_fast_fit",
    "local_n100k_dense_two_fe_t8_fast_fit",
    "local_n100k_sparse_two_fe_t1_fast_fit",
    "local_n100k_sparse_two_fe_t8_fast_fit",
    "local_n100k_absorbed_slope_t1_fast_fit",
    "local_n100k_clustered_vce_t1_fast_fit",
    "local_n100k_moderate_k_t1_fast_fit",
    "local_n100k_dense_two_fe_t1_low_fit",
    "local_n100k_dense_two_fe_t8_fast_save_fe",
    "local_n1m_dense_two_fe_t1_fast_fit",
    "local_n1m_dense_two_fe_t8_fast_fit",
    "local_n1m_sparse_two_fe_t1_fast_fit",
    "local_n1m_sparse_two_fe_t8_fast_fit",
    "local_n1m_absorbed_slope_t8_fast_fit",
    "local_n1m_clustered_vce_t8_fast_fit",
    "local_n1m_moderate_k_t8_fast_fit",
    "local_n1m_dense_two_fe_t1_low_fit",
    "local_n1m_dense_two_fe_t8_fast_save_fe",
)
SCC_GOVERNING_CELL_IDS = (
    "dense_two_fe_t1_fast_fit",
    "dense_two_fe_t8_fast_fit",
    "sparse_two_fe_t1_fast_fit",
    "sparse_two_fe_t8_fast_fit",
    "absorbed_slope_t1_fast_fit",
    "absorbed_slope_t8_fast_fit",
    "clustered_vce_t1_fast_fit",
    "clustered_vce_t8_fast_fit",
    "moderate_k_t1_fast_fit",
    "moderate_k_t8_fast_fit",
    "dense_two_fe_t1_low_fit",
    "sparse_two_fe_t8_low_fit",
    "dense_two_fe_t8_fast_save_fe",
    "constrained_memory_t1_low",
)
EXPECTED_CELLS_BY_PROFILE = {
    "local_screen": LOCAL_SCREEN_CELL_IDS,
    "scc_governing": SCC_GOVERNING_CELL_IDS,
}
EXPECTED_CELL_IDS = LOCAL_SCREEN_CELL_IDS + SCC_GOVERNING_CELL_IDS
SOURCE_FIELDS = {
    "commit",
    "tree",
    "cargo_lock_sha256",
    "references_lock_sha256",
}
ARTIFACT_FIELDS = {"platform", "sha256", "size_bytes", "embedded_git_commit"}


class ContractError(ValueError):
    """The manifest, evidence, source identity, or receipt fails closed."""


def canonical_json_bytes(value: Any) -> bytes:
    """Return the one canonical encoding used by evidence and receipt seals."""
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


def _expect_keys(
    value: Mapping[str, Any], required: set[str], where: str
) -> None:
    if set(value) != required:
        missing = sorted(required - set(value))
        extra = sorted(set(value) - required)
        raise ContractError(f"{where} fields are not closed; missing={missing}, extra={extra}")


def _mapping(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ContractError(f"{where} must be an object")
    return value


def _integer(value: Any, where: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        raise ContractError(f"{where} must be an integer >= {minimum}")
    return value


def _number(value: Any, where: str, *, positive: bool = False) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ContractError(f"{where} must be numeric")
    result = float(value)
    if not math.isfinite(result) or (positive and result <= 0.0):
        qualifier = "positive and " if positive else ""
        raise ContractError(f"{where} must be {qualifier}finite")
    return result


def _sha256(value: Any, where: str) -> str:
    if not isinstance(value, str) or HEX_SHA256.fullmatch(value) is None:
        raise ContractError(f"{where} must be a lowercase SHA-256")
    return value


def _sha1(value: Any, where: str) -> str:
    if not isinstance(value, str) or HEX_SHA1.fullmatch(value) is None:
        raise ContractError(f"{where} must be a lowercase 40-character git id")
    return value


def _json_tree(value: Any, where: str) -> None:
    """Validate arbitrary exact-comparison metadata without accepting NaN."""
    if value is None or isinstance(value, (str, bool)):
        return
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        _number(value, where)
        return
    if isinstance(value, list):
        for index, item in enumerate(value):
            _json_tree(item, f"{where}[{index}]")
        return
    if isinstance(value, Mapping):
        for key, item in value.items():
            if not isinstance(key, str) or not key:
                raise ContractError(f"{where} contains an empty or non-string key")
            _json_tree(item, f"{where}.{key}")
        return
    raise ContractError(f"{where} contains a non-JSON value")


def _git(repository: Path, arguments: Sequence[str]) -> bytes:
    try:
        completed = subprocess.run(
            ["git", "-C", str(repository), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", b"").decode("utf-8", errors="replace").strip()
        raise ContractError(f"git identity command failed: {detail or error}") from error
    return completed.stdout


def source_identity(
    repository: Path = ROOT,
    revision: str = "HEAD",
    *,
    require_clean: bool = True,
) -> dict[str, str]:
    """Capture an exact commit/tree/lockfile identity from a local git object."""
    repository = repository.resolve()
    commit = _git(repository, ["rev-parse", "--verify", f"{revision}^{{commit}}"])
    commit_text = commit.decode("ascii").strip()
    _sha1(commit_text, "source commit")
    tree_text = _git(repository, ["show", "-s", "--format=%T", commit_text]).decode(
        "ascii"
    ).strip()
    _sha1(tree_text, "source tree")
    if require_clean:
        dirty = _git(repository, ["status", "--porcelain", "--untracked-files=no"])
        if dirty.strip():
            raise ContractError("source worktree has tracked modifications")
    cargo_lock = _git(repository, ["show", f"{commit_text}:Cargo.lock"])
    references_lock = _git(repository, ["show", f"{commit_text}:references.lock.toml"])
    return {
        "commit": commit_text,
        "tree": tree_text,
        "cargo_lock_sha256": hashlib.sha256(cargo_lock).hexdigest(),
        "references_lock_sha256": hashlib.sha256(references_lock).hexdigest(),
    }


def verify_source_identity(
    identity: Mapping[str, Any],
    repository: Path = ROOT,
) -> dict[str, str]:
    """Reject stale, dirty, or partially substituted source identities."""
    _validate_source(identity, "source identity")
    observed = source_identity(repository, str(identity["commit"]), require_clean=True)
    if dict(identity) != observed:
        raise ContractError(
            f"source identity mismatch; expected={dict(identity)}, observed={observed}"
        )
    head = _git(repository.resolve(), ["rev-parse", "--verify", "HEAD^{commit}"])
    if head.decode("ascii").strip() != identity["commit"]:
        raise ContractError("source identity does not name the checked-out HEAD")
    return observed


def _validate_source(value: Mapping[str, Any], where: str) -> None:
    _expect_keys(value, SOURCE_FIELDS, where)
    _sha1(value["commit"], f"{where}.commit")
    _sha1(value["tree"], f"{where}.tree")
    _sha256(value["cargo_lock_sha256"], f"{where}.cargo_lock_sha256")
    _sha256(value["references_lock_sha256"], f"{where}.references_lock_sha256")


def _validate_artifact(value: Mapping[str, Any], where: str) -> None:
    _expect_keys(value, ARTIFACT_FIELDS, where)
    if not isinstance(value["platform"], str) or not value["platform"]:
        raise ContractError(f"{where}.platform must be nonempty")
    _sha256(value["sha256"], f"{where}.sha256")
    _integer(value["size_bytes"], f"{where}.size_bytes", minimum=1)
    embedded = value["embedded_git_commit"]
    if not isinstance(embedded, str) or not re.fullmatch(r"[0-9a-f]{12}", embedded):
        raise ContractError(f"{where}.embedded_git_commit must be a 12-character git id")


def load_manifest(
    path: Path = DEFAULT_MANIFEST,
    *,
    verify_git_object: bool = True,
    root: Path = ROOT,
) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"unable to read F0 manifest: {error}") from error
    validate_manifest(value)
    if verify_git_object:
        expected = value["baseline_candidate"]["source"]
        observed = source_identity(root, expected["commit"], require_clean=False)
        if expected != observed:
            raise ContractError(
                f"F0 source object no longer matches manifest; expected={expected}, observed={observed}"
            )
        for record in value["sealing"]["observed_local_evidence"]:
            receipt_path = root / record["receipt_path"]
            if not receipt_path.is_file() or sha256_file(receipt_path) != record["receipt_sha256"]:
                raise ContractError("observed exact-SHA local receipt is missing or stale")
            receipt = _mapping(
                json.loads(receipt_path.read_text(encoding="utf-8")),
                "observed exact-SHA local receipt",
            )
            expected_receipt_fields = {
                "tested_sha": record["tested_commit"],
                "plugin_sha256": record["plugin_sha256"],
                "stata_bundle_version": record["stata_version"],
                "stata_edition": record["stata_flavor"],
                "stata_processors": record["stata_processors"],
                "tests_passed": record["tests_passed"],
                "rust_status": record["rust_status"],
                "stata_status": record["stata_status"],
            }
            for field, expected_field in expected_receipt_fields.items():
                if receipt.get(field) != expected_field:
                    raise ContractError(f"observed exact-SHA local receipt contradicts {field}")
    return value


def validate_manifest(manifest: Mapping[str, Any]) -> None:
    _expect_keys(
        manifest,
        {
            "schema_version",
            "manifest_id",
            "status",
            "baseline_candidate",
            "evidence_contract",
            "profiles",
            "scientific_rules",
            "performance_rules",
            "memory_rules",
            "cells",
            "sealing",
        },
        "manifest",
    )
    if manifest["schema_version"] != 1 or manifest["manifest_id"] != "fereg_f0_regression_v1":
        raise ContractError("manifest identity must be fereg_f0_regression_v1 schema 1")
    if manifest["status"] not in {"candidate_unsealed", "frozen"}:
        raise ContractError("manifest status must be candidate_unsealed or frozen")

    baseline = _mapping(manifest["baseline_candidate"], "baseline_candidate")
    _expect_keys(baseline, {"label", "source", "versions", "artifacts"}, "baseline_candidate")
    if baseline["label"] != "F0":
        raise ContractError("baseline candidate label must be F0")
    source = _mapping(baseline["source"], "baseline_candidate.source")
    _validate_source(source, "baseline_candidate.source")
    if source != {
        "commit": "2dee5d8c4cd99b51bbaca600348f66f325b58ab4",
        "tree": "cc10b50077f162cab8be10702a9627b99ee5eea2",
        "cargo_lock_sha256": "4dd5db4d5b7f889cc664b522b84ac46dd10b8ca83f15af371cd6acf1239b4e42",
        "references_lock_sha256": "48aec6f93ea758987734f702dde87853da56342b228bae927b7bbebcf5b3f801",
    }:
        raise ContractError("baseline candidate source identity is not the audited F0 object")
    versions = _mapping(baseline["versions"], "baseline_candidate.versions")
    if versions != {
        "ado": "0.5.0",
        "plugin": "0.5.0",
        "action_protocol": "1.5",
        "model_wire": "2.1",
        "ols_result_wire": "1.4",
    }:
        raise ContractError("baseline candidate protocol versions are not frozen")
    artifacts = baseline["artifacts"]
    if not isinstance(artifacts, list) or len(artifacts) != 3:
        raise ContractError("baseline candidate must enumerate exactly three platform artifacts")
    artifact_platforms: set[str] = set()
    for index, artifact_value in enumerate(artifacts):
        artifact = _mapping(artifact_value, f"baseline artifact {index}")
        _expect_keys(
            artifact,
            ARTIFACT_FIELDS | {"status"},
            f"baseline artifact {index}",
        )
        platform = artifact["platform"]
        if platform in artifact_platforms:
            raise ContractError("baseline artifact platform is duplicated")
        artifact_platforms.add(platform)
        if artifact["status"] not in {"pending", "captured_unsealed", "qualified"}:
            raise ContractError("baseline artifact status is invalid")
        if artifact["status"] == "pending":
            if any(artifact[field] is not None for field in ARTIFACT_FIELDS - {"platform"}):
                raise ContractError("pending baseline artifact must not invent an identity")
        else:
            _validate_artifact(
                {field: artifact[field] for field in ARTIFACT_FIELDS},
                f"baseline artifact {index}",
            )
            if artifact["embedded_git_commit"] != source["commit"][:12]:
                raise ContractError("baseline artifact is not bound to the F0 commit")
    if artifact_platforms != {"macos-universal", "linux-x86_64", "windows-x86_64"}:
        raise ContractError("baseline artifact platform inventory is not closed")

    evidence_contract = _mapping(manifest["evidence_contract"], "evidence_contract")
    _expect_keys(
        evidence_contract,
        {
            "schema_version",
            "evidence_type",
            "environment_fields",
            "cell_fields",
            "scientific_fields",
            "pair_fields",
            "measurement_fields",
            "memory_fields",
            "allocation_class_fields",
        },
        "evidence_contract",
    )
    if evidence_contract["schema_version"] != 1 or evidence_contract["evidence_type"] != "fereg-f0-regression-v1-evidence":
        raise ContractError("evidence contract identity is invalid")
    expected_lists = {
        "environment_fields": [
            "host_id", "os", "arch", "cpu", "stata_version", "stata_flavor",
            "toolchain", "physical_memory_bytes",
        ],
        "cell_fields": [
            "cell_id", "environment", "scientific", "pairs",
            "confirmation_pairs", "memory",
        ],
        "scientific_fields": [
            "stata_rc", "sample_identity_sha256", "singleton_identity_sha256",
            "coefficient_names", "coefficient_status", "numeric_hex_sha256",
            "coefficients", "vce_shape", "vce", "dof", "fit", "outputs",
            "solver", "certificates",
        ],
        "pair_fields": ["index", "order", "baseline", "candidate"],
        "measurement_fields": ["seconds", "peak_rss_bytes"],
        "memory_fields": [
            "native_allocation_bytes", "allocation_classes",
            "repeated_call_retained_bytes", "constrained_memory",
        ],
        "allocation_class_fields": [
            "observations", "observation_columns", "fe_levels", "clusters",
        ],
    }
    for field, expected in expected_lists.items():
        if evidence_contract[field] != expected:
            raise ContractError(f"evidence contract {field} inventory is not frozen")

    profiles = manifest["profiles"]
    if not isinstance(profiles, list) or len(profiles) != 2:
        raise ContractError("manifest must contain exactly two regression profiles")
    expected_profile_fields = {"id", "role", "platforms", "governing"}
    observed_profiles: list[str] = []
    for index, profile_value in enumerate(profiles):
        profile = _mapping(profile_value, f"profile {index}")
        _expect_keys(profile, expected_profile_fields, f"profile {index}")
        observed_profiles.append(profile["id"])
        if not isinstance(profile["role"], str) or not profile["role"]:
            raise ContractError("profile role must be nonempty")
        if not isinstance(profile["platforms"], list) or len(profile["platforms"]) != 1:
            raise ContractError("each F0 regression profile has exactly one platform")
        if not isinstance(profile["governing"], bool):
            raise ContractError("profile governing flag must be boolean")
    if observed_profiles != ["local_screen", "scc_governing"]:
        raise ContractError("regression profile inventory or order is not frozen")
    if profiles[0]["platforms"] != ["macos-universal"] or profiles[0]["governing"]:
        raise ContractError("local screening profile contract is invalid")
    if profiles[1]["platforms"] != ["linux-x86_64"] or not profiles[1]["governing"]:
        raise ContractError("SCC governing profile contract is invalid")

    scientific = _mapping(manifest["scientific_rules"], "scientific_rules")
    _expect_keys(
        scientific,
        {
            "active_policy", "registered_tolerance_activation", "policies",
            "certificate_maxima",
        },
        "scientific_rules",
    )
    if scientific["active_policy"] not in {"extraction_exact", "registered_tolerance"}:
        raise ContractError("scientific comparison policy is unknown")
    activation = _mapping(
        scientific["registered_tolerance_activation"],
        "registered_tolerance_activation",
    )
    _expect_keys(activation, {"authorized", "adr"}, "registered_tolerance_activation")
    if not isinstance(activation["authorized"], bool):
        raise ContractError("registered-tolerance authorization must be boolean")
    if scientific["active_policy"] == "registered_tolerance":
        if not activation["authorized"] or not isinstance(activation["adr"], str) or not activation["adr"]:
            raise ContractError("registered tolerances require an explicit accepted ADR")
    elif activation != {"authorized": False, "adr": None}:
        raise ContractError("inactive registered tolerances must remain unauthorized")
    policies = _mapping(scientific["policies"], "scientific policies")
    _expect_keys(policies, {"extraction_exact", "registered_tolerance"}, "scientific policies")
    exact_policy = _mapping(policies["extraction_exact"], "extraction_exact policy")
    registered_policy = _mapping(
        policies["registered_tolerance"], "registered_tolerance policy"
    )
    for name, policy in (
        ("extraction_exact", exact_policy),
        ("registered_tolerance", registered_policy),
    ):
        _expect_keys(policy, {"exact_fields", "tolerant_fields"}, f"{name} policy")
    if exact_policy["exact_fields"] != [
        "stata_rc", "sample_identity_sha256", "singleton_identity_sha256",
        "coefficient_names", "coefficient_status", "numeric_hex_sha256",
        "coefficients", "vce_shape", "vce", "dof", "fit", "outputs", "solver",
        "certificates.converged",
        "certificates.values", "certificates.digests",
    ]:
        raise ContractError("extraction exact-field inventory is not frozen")
    if exact_policy["tolerant_fields"] != {}:
        raise ContractError("pure extraction policy cannot contain numeric tolerances")
    if registered_policy["exact_fields"] != [
        "stata_rc", "sample_identity_sha256", "singleton_identity_sha256",
        "coefficient_names", "coefficient_status", "vce_shape", "dof", "outputs",
        "solver", "certificates.converged", "certificates.digests",
    ]:
        raise ContractError("registered-tolerance exact-field inventory is not frozen")
    if set(registered_policy["tolerant_fields"]) != {
        "coefficients", "vce", "fit", "certificates.values",
    }:
        raise ContractError("registered tolerant-field inventory is not frozen")
    expected_tolerances = {
        "coefficients": {"rtol": 1e-7, "atol": 1e-10},
        "vce": {"rtol": 1e-6, "atol": 1e-10},
        "fit": {"rtol": 1e-7, "atol": 1e-9},
        "certificates.values": {"rtol": 1e-7, "atol": 1e-10},
    }
    if registered_policy["tolerant_fields"] != expected_tolerances:
        raise ContractError("registered tolerances differ from the audited OLS gates")
    for name, tolerance_value in registered_policy["tolerant_fields"].items():
        tolerance = _mapping(tolerance_value, f"scientific tolerance {name}")
        _expect_keys(tolerance, {"rtol", "atol"}, f"scientific tolerance {name}")
        if _number(tolerance["rtol"], f"{name}.rtol") < 0 or _number(tolerance["atol"], f"{name}.atol") < 0:
            raise ContractError("scientific tolerances must be nonnegative")
    certificate_maxima = _mapping(scientific["certificate_maxima"], "certificate_maxima")
    if set(certificate_maxima) != {
        "normal_equation_linf", "fe_projection_linf", "vce_symmetry_linf",
        "reconstruction_linf",
    }:
        raise ContractError("certificate maximum inventory is not frozen")
    for name, maximum in certificate_maxima.items():
        if _number(maximum, f"certificate maximum {name}", positive=True) <= 0:
            raise ContractError("certificate maximum must be positive")

    performance = _mapping(manifest["performance_rules"], "performance_rules")
    _expect_keys(
        performance,
        {
            "warm_pairs", "pair_order", "paired_ratio",
            "aggregate_geometric_mean_ratio_max",
            "individual_geometric_mean_ratio_confirmation_threshold",
            "student_t_two_sided_95_df8",
            "reject_reproducible_slowdown_when_log_ci_lower_exceeds_zero",
            "scientific_preflight_required_before_timing",
            "same_node_and_environment_required",
        },
        "performance_rules",
    )
    if performance != {
        "warm_pairs": 9,
        "pair_order": ["AB", "BA", "AB", "BA", "AB", "BA", "AB", "BA", "AB"],
        "paired_ratio": "candidate_seconds_over_baseline_seconds",
        "aggregate_geometric_mean_ratio_max": 1.02,
        "individual_geometric_mean_ratio_confirmation_threshold": 1.05,
        "student_t_two_sided_95_df8": 2.306004135,
        "reject_reproducible_slowdown_when_log_ci_lower_exceeds_zero": True,
        "scientific_preflight_required_before_timing": True,
        "same_node_and_environment_required": True,
    }:
        raise ContractError("paired performance rules are not frozen")

    memory = _mapping(manifest["memory_rules"], "memory_rules")
    _expect_keys(
        memory,
        {
            "peak_rss_relative_delta_max", "peak_rss_absolute_delta_bytes_max",
            "native_allocation_fixed_delta_bytes_max",
            "data_sized_allocation_classes_exact",
            "repeated_call_retained_bytes_max", "constrained_cell",
            "constrained_contract",
        },
        "memory_rules",
    )
    if memory != {
        "peak_rss_relative_delta_max": 0.02,
        "peak_rss_absolute_delta_bytes_max": 67108864,
        "native_allocation_fixed_delta_bytes_max": 1048576,
        "data_sized_allocation_classes_exact": True,
        "repeated_call_retained_bytes_max": 0,
        "constrained_cell": "constrained_memory_t1_low",
        "constrained_contract": {
            "low": {"status": "success", "rc": 0},
            "fast": {"status": "preflight_failure", "rc": 909},
        },
    }:
        raise ContractError("memory rules are not frozen")

    cells = manifest["cells"]
    if not isinstance(cells, list):
        raise ContractError("manifest cells must be a list")
    cell_fields = {
        "id", "profile", "design", "n", "controls", "threads", "memory",
        "outputs", "vce", "primary",
    }
    observed_ids: list[str] = []
    for index, cell_value in enumerate(cells):
        cell = _mapping(cell_value, f"manifest cell {index}")
        _expect_keys(cell, cell_fields, f"manifest cell {index}")
        if not isinstance(cell["id"], str):
            raise ContractError("manifest cell id must be a string")
        observed_ids.append(cell["id"])
        if cell["profile"] not in EXPECTED_CELLS_BY_PROFILE:
            raise ContractError(f"manifest cell {cell['id']} has an unknown profile")
        if cell["profile"] == "local_screen" and cell["n"] not in {100_000, 1_000_000}:
            raise ContractError(f"manifest cell {cell['id']} has an unknown local scale")
        if cell["profile"] == "scc_governing" and cell["n"] != 10_000_000:
            raise ContractError(f"manifest cell {cell['id']} has an unknown SCC scale")
        if cell["controls"] not in {2, 30} or cell["threads"] not in {1, 8}:
            raise ContractError(f"manifest cell {cell['id']} has unknown controls or threads")
        if cell["memory"] not in {"fast", "low"} or cell["outputs"] not in {"fit", "save_fe"}:
            raise ContractError(f"manifest cell {cell['id']} has unknown memory or output mode")
        if cell["vce"] not in {"unadjusted", "cluster"} or not isinstance(cell["primary"], bool):
            raise ContractError(f"manifest cell {cell['id']} has unknown VCE or primary status")
    if tuple(observed_ids) != EXPECTED_CELL_IDS:
        raise ContractError("manifest cell inventory or order is not frozen")

    sealing = _mapping(manifest["sealing"], "sealing")
    _expect_keys(
        sealing,
        {"state", "observed_local_evidence", "missing_required_evidence", "freeze_rule"},
        "sealing",
    )
    local_evidence = sealing["observed_local_evidence"]
    if not isinstance(local_evidence, list) or len(local_evidence) != 1:
        raise ContractError("observed local evidence inventory is malformed")
    local_record = _mapping(local_evidence[0], "observed local evidence")
    _expect_keys(
        local_record,
        {
            "id", "receipt_path", "receipt_sha256", "raw_receipt_sha256",
            "source_archive_sha256",
            "tested_commit", "plugin_sha256", "stata_version", "stata_flavor",
            "stata_processors", "tests_passed", "includes_break", "rust_status",
            "stata_status",
        },
        "observed local evidence",
    )
    if local_record != {
        "id": "exact_sha_local_full",
        "receipt_path": "build_evidence/local/f0-2dee5d8/receipt.json",
        "receipt_sha256": "5813d66caa6f6b1d1a4369e56c9d90a435f6973f2b62fcfc169b3e91ba5a4b71",
        "raw_receipt_sha256": "4c18f1b7561aed1ac5e5488c37c4d6a6c1d981990dadbe4bc1fb68eb37d39da5",
        "source_archive_sha256": "41dab4eff5cd2242b2e02877fddcd81fb8f011478c80db8119b7dd34b383ee18",
        "tested_commit": "2dee5d8c4cd99b51bbaca600348f66f325b58ab4",
        "plugin_sha256": "633fcd1a0b91617ea5c1aff10f17facf0990bd6ca72893cb81f0c100dfeb2298",
        "stata_version": "19.0.115",
        "stata_flavor": "MP",
        "stata_processors": 8,
        "tests_passed": 35,
        "includes_break": True,
        "rust_status": "success",
        "stata_status": "success",
    }:
        raise ContractError("observed local evidence identity is not the audited F0 receipt")
    if not isinstance(sealing["missing_required_evidence"], list) or not all(
        isinstance(item, str) and item for item in sealing["missing_required_evidence"]
    ):
        raise ContractError("missing evidence inventory is malformed")
    if not isinstance(sealing["freeze_rule"], str) or not sealing["freeze_rule"]:
        raise ContractError("manifest freeze rule is missing")
    if manifest["status"] == "candidate_unsealed":
        if sealing["state"] != "not_sealed" or not sealing["missing_required_evidence"]:
            raise ContractError("an unsealed baseline must name missing required evidence")
    elif sealing["state"] != "sealed" or sealing["missing_required_evidence"]:
        raise ContractError("a frozen baseline must be sealed with no missing evidence")
    elif any(artifact["status"] != "qualified" for artifact in artifacts):
        raise ContractError("a frozen baseline requires every platform artifact to be qualified")


def _get_path(value: Mapping[str, Any], dotted: str, where: str) -> Any:
    current: Any = value
    for component in dotted.split("."):
        if not isinstance(current, Mapping) or component not in current:
            raise ContractError(f"{where} is missing {dotted}")
        current = current[component]
    return current


def _validate_numeric_tree(value: Any, where: str) -> None:
    if isinstance(value, Mapping):
        if not value:
            raise ContractError(f"{where} must not be empty")
        for key, item in value.items():
            if not isinstance(key, str) or not key:
                raise ContractError(f"{where} contains an invalid key")
            _validate_numeric_tree(item, f"{where}.{key}")
        return
    if isinstance(value, list):
        for index, item in enumerate(value):
            _validate_numeric_tree(item, f"{where}[{index}]")
        return
    _number(value, where)


def _validate_scientific(
    manifest: Mapping[str, Any],
    value: Any,
    where: str,
) -> Mapping[str, Any]:
    scientific = _mapping(value, where)
    expected = set(manifest["evidence_contract"]["scientific_fields"])
    _expect_keys(scientific, expected, where)
    _integer(scientific["stata_rc"], f"{where}.stata_rc")
    _sha256(scientific["sample_identity_sha256"], f"{where}.sample_identity_sha256")
    _sha256(
        scientific["singleton_identity_sha256"],
        f"{where}.singleton_identity_sha256",
    )
    names = scientific["coefficient_names"]
    if not isinstance(names, list) or not all(isinstance(name, str) and name for name in names):
        raise ContractError(f"{where}.coefficient_names is malformed")
    if len(set(names)) != len(names):
        raise ContractError(f"{where}.coefficient_names contains duplicates")
    status = scientific["coefficient_status"]
    coefficients = scientific["coefficients"]
    if not isinstance(status, list) or len(status) != len(names):
        raise ContractError(f"{where}.coefficient_status is not coefficient aligned")
    for index, item in enumerate(status):
        if isinstance(item, bool) or not isinstance(item, (str, int)):
            raise ContractError(f"{where}.coefficient_status[{index}] is malformed")
    _sha256(scientific["numeric_hex_sha256"], f"{where}.numeric_hex_sha256")
    if not isinstance(coefficients, list) or len(coefficients) != len(names):
        raise ContractError(f"{where}.coefficients is not coefficient aligned")
    _validate_numeric_tree(coefficients, f"{where}.coefficients")
    shape = scientific["vce_shape"]
    if not isinstance(shape, list) or len(shape) != 2:
        raise ContractError(f"{where}.vce_shape must have two dimensions")
    rows = _integer(shape[0], f"{where}.vce_shape[0]")
    columns = _integer(shape[1], f"{where}.vce_shape[1]")
    if rows != len(names) or columns != len(names):
        raise ContractError(f"{where}.vce_shape is not coefficient aligned")
    vce = scientific["vce"]
    if not isinstance(vce, list) or len(vce) != rows * columns:
        raise ContractError(f"{where}.vce has the wrong flattened dimensions")
    _validate_numeric_tree(vce, f"{where}.vce")
    for field in ("dof", "fit"):
        _validate_numeric_tree(scientific[field], f"{where}.{field}")
    outputs = _mapping(scientific["outputs"], f"{where}.outputs")
    _json_tree(outputs, f"{where}.outputs")
    solver = _mapping(scientific["solver"], f"{where}.solver")
    if not solver:
        raise ContractError(f"{where}.solver must not be empty")
    _json_tree(solver, f"{where}.solver")
    certificates = _mapping(scientific["certificates"], f"{where}.certificates")
    _expect_keys(certificates, {"converged", "values", "digests"}, f"{where}.certificates")
    if not isinstance(certificates["converged"], bool):
        raise ContractError(f"{where}.certificates.converged must be boolean")
    values = _mapping(certificates["values"], f"{where}.certificates.values")
    if set(values) != set(manifest["scientific_rules"]["certificate_maxima"]):
        raise ContractError(f"{where}.certificates.values inventory is not frozen")
    for name, item in values.items():
        number = _number(item, f"{where}.certificates.values.{name}")
        if number < 0:
            raise ContractError(f"{where}.certificates.values.{name} must be nonnegative")
    digests = _mapping(certificates["digests"], f"{where}.certificates.digests")
    for name, digest in digests.items():
        if not isinstance(name, str) or not name:
            raise ContractError(f"{where}.certificates.digests contains an invalid key")
        _sha256(digest, f"{where}.certificates.digests.{name}")
    return scientific


def _compare_numeric_tree(
    left: Any,
    right: Any,
    *,
    rtol: float,
    atol: float,
    path: str,
    reasons: list[str],
) -> None:
    if isinstance(left, Mapping) and isinstance(right, Mapping):
        if set(left) != set(right):
            reasons.append(f"{path}:inventory")
            return
        for key in sorted(left):
            _compare_numeric_tree(
                left[key], right[key], rtol=rtol, atol=atol,
                path=f"{path}.{key}", reasons=reasons,
            )
        return
    if isinstance(left, list) and isinstance(right, list):
        if len(left) != len(right):
            reasons.append(f"{path}:length")
            return
        for index, (left_item, right_item) in enumerate(zip(left, right)):
            _compare_numeric_tree(
                left_item, right_item, rtol=rtol, atol=atol,
                path=f"{path}[{index}]", reasons=reasons,
            )
        return
    try:
        left_number = _number(left, path)
        right_number = _number(right, path)
    except ContractError:
        reasons.append(f"{path}:shape")
        return
    if not math.isclose(left_number, right_number, rel_tol=rtol, abs_tol=atol):
        reasons.append(path)


def compare_scientific(
    manifest: Mapping[str, Any],
    baseline: Mapping[str, Any],
    candidate: Mapping[str, Any],
) -> list[str]:
    """Return stable scientific mismatch reasons; an empty list means pass."""
    _validate_scientific(manifest, baseline, "baseline scientific record")
    _validate_scientific(manifest, candidate, "candidate scientific record")
    reasons: list[str] = []
    rules = manifest["scientific_rules"]
    policy = rules["policies"][rules["active_policy"]]
    for path in policy["exact_fields"]:
        if _get_path(baseline, path, "baseline scientific record") != _get_path(
            candidate, path, "candidate scientific record"
        ):
            reasons.append(path)
    for path, tolerance in policy["tolerant_fields"].items():
        _compare_numeric_tree(
            _get_path(baseline, path, "baseline scientific record"),
            _get_path(candidate, path, "candidate scientific record"),
            rtol=float(tolerance["rtol"]),
            atol=float(tolerance["atol"]),
            path=path,
            reasons=reasons,
        )
    for arm_name, record in (("baseline", baseline), ("candidate", candidate)):
        if record["stata_rc"] != 0:
            reasons.append(f"{arm_name}.stata_rc_nonzero")
        if record["certificates"]["converged"] is not True:
            reasons.append(f"{arm_name}.certificates.not_converged")
        for name, maximum in rules["certificate_maxima"].items():
            if record["certificates"]["values"][name] > maximum:
                reasons.append(f"{arm_name}.certificates.{name}_above_maximum")
    return sorted(set(reasons))


def _validate_environment(manifest: Mapping[str, Any], value: Any, where: str) -> None:
    environment = _mapping(value, where)
    _expect_keys(
        environment,
        set(manifest["evidence_contract"]["environment_fields"]),
        where,
    )
    for field in (
        "host_id", "os", "arch", "cpu", "stata_version", "stata_flavor", "toolchain",
    ):
        if not isinstance(environment[field], str) or not environment[field]:
            raise ContractError(f"{where}.{field} must be a nonempty string")
    _integer(environment["physical_memory_bytes"], f"{where}.physical_memory_bytes", minimum=1)


def _manifest_sha256(manifest: Mapping[str, Any]) -> str:
    return hashlib.sha256(canonical_json_bytes(manifest)).hexdigest()


def _validate_evidence_header(
    manifest: Mapping[str, Any],
    evidence: Mapping[str, Any],
    manifest_sha256: str,
) -> tuple[str, tuple[str, ...], dict[str, Mapping[str, Any]]]:
    _expect_keys(
        evidence,
        {
            "schema_version", "evidence_type", "profile", "manifest", "baseline",
            "candidate", "cells",
        },
        "evidence",
    )
    if evidence["schema_version"] != 1 or evidence["evidence_type"] != manifest["evidence_contract"]["evidence_type"]:
        raise ContractError("evidence identity is invalid")
    manifest_binding = _mapping(evidence["manifest"], "evidence.manifest")
    _expect_keys(manifest_binding, {"id", "sha256"}, "evidence.manifest")
    if manifest_binding != {"id": manifest["manifest_id"], "sha256": manifest_sha256}:
        raise ContractError("evidence manifest identity is stale or substituted")
    profile = evidence["profile"]
    if profile not in EXPECTED_CELLS_BY_PROFILE:
        raise ContractError("evidence profile is unknown")
    expected_ids = EXPECTED_CELLS_BY_PROFILE[profile]

    identities: dict[str, Mapping[str, Any]] = {}
    for arm in ("baseline", "candidate"):
        identity = _mapping(evidence[arm], f"evidence.{arm}")
        _expect_keys(identity, {"source", "artifact"}, f"evidence.{arm}")
        source = _mapping(identity["source"], f"evidence.{arm}.source")
        artifact = _mapping(identity["artifact"], f"evidence.{arm}.artifact")
        _validate_source(source, f"evidence.{arm}.source")
        _validate_artifact(artifact, f"evidence.{arm}.artifact")
        if artifact["embedded_git_commit"] != source["commit"][:12]:
            raise ContractError(f"{arm} artifact is not bound to its source commit")
        identities[arm] = identity
    if evidence["baseline"]["source"] != manifest["baseline_candidate"]["source"]:
        raise ContractError("evidence baseline source does not equal F0")
    profile_spec = next(item for item in manifest["profiles"] if item["id"] == profile)
    if evidence["baseline"]["artifact"]["platform"] not in profile_spec["platforms"]:
        raise ContractError("evidence baseline artifact platform contradicts its profile")
    if evidence["candidate"]["artifact"]["platform"] != evidence["baseline"]["artifact"]["platform"]:
        raise ContractError("candidate and F0 artifacts must target the same platform")
    matching_artifacts = [
        artifact
        for artifact in manifest["baseline_candidate"]["artifacts"]
        if artifact["platform"] == evidence["baseline"]["artifact"]["platform"]
    ]
    if len(matching_artifacts) != 1 or matching_artifacts[0]["status"] == "pending":
        raise ContractError("evidence selects an unavailable F0 platform artifact")
    expected_artifact = {
        field: matching_artifacts[0][field] for field in ARTIFACT_FIELDS
    }
    if evidence["baseline"]["artifact"] != expected_artifact:
        raise ContractError("evidence F0 artifact identity is stale or substituted")

    cells_value = evidence["cells"]
    if not isinstance(cells_value, list):
        raise ContractError("evidence cells must be a list")
    cells: dict[str, Mapping[str, Any]] = {}
    expected_cell_fields = set(manifest["evidence_contract"]["cell_fields"])
    for index, cell_value in enumerate(cells_value):
        cell = _mapping(cell_value, f"evidence cell {index}")
        _expect_keys(cell, expected_cell_fields, f"evidence cell {index}")
        cell_id = cell["cell_id"]
        if not isinstance(cell_id, str) or cell_id not in expected_ids or cell_id in cells:
            raise ContractError("evidence cell inventory contains an unknown or duplicate id")
        _validate_environment(manifest, cell["environment"], f"evidence cell {cell_id}.environment")
        scientific = _mapping(cell["scientific"], f"evidence cell {cell_id}.scientific")
        _expect_keys(scientific, {"baseline", "candidate"}, f"evidence cell {cell_id}.scientific")
        _validate_scientific(manifest, scientific["baseline"], f"{cell_id} baseline scientific")
        _validate_scientific(manifest, scientific["candidate"], f"{cell_id} candidate scientific")
        cells[cell_id] = cell
    if tuple(cells) != expected_ids:
        raise ContractError("evidence cell inventory or order is incomplete")
    return profile, expected_ids, cells


def _validate_pairs(
    manifest: Mapping[str, Any], value: Any, where: str
) -> list[Mapping[str, Any]]:
    if not isinstance(value, list) or len(value) != manifest["performance_rules"]["warm_pairs"]:
        raise ContractError(f"{where} must contain exactly nine warm pairs")
    expected_orders = manifest["performance_rules"]["pair_order"]
    pair_fields = set(manifest["evidence_contract"]["pair_fields"])
    measurement_fields = set(manifest["evidence_contract"]["measurement_fields"])
    output: list[Mapping[str, Any]] = []
    for offset, pair_value in enumerate(value):
        pair = _mapping(pair_value, f"{where}[{offset}]")
        _expect_keys(pair, pair_fields, f"{where}[{offset}]")
        if pair["index"] != offset + 1 or pair["order"] != expected_orders[offset]:
            raise ContractError(f"{where} does not alternate the frozen AB/BA cadence")
        for arm in ("baseline", "candidate"):
            measurement = _mapping(pair[arm], f"{where}[{offset}].{arm}")
            _expect_keys(measurement, measurement_fields, f"{where}[{offset}].{arm}")
            _number(measurement["seconds"], f"{where}[{offset}].{arm}.seconds", positive=True)
            _integer(
                measurement["peak_rss_bytes"],
                f"{where}[{offset}].{arm}.peak_rss_bytes",
                minimum=1,
            )
        output.append(pair)
    return output


def _validate_constrained(value: Any, where: str) -> Mapping[str, Any] | None:
    if value is None:
        return None
    constrained = _mapping(value, where)
    _expect_keys(constrained, {"limit_bytes", "low", "fast"}, where)
    _integer(constrained["limit_bytes"], f"{where}.limit_bytes", minimum=1)
    for mode in ("low", "fast"):
        result = _mapping(constrained[mode], f"{where}.{mode}")
        _expect_keys(result, {"status", "rc"}, f"{where}.{mode}")
        if not isinstance(result["status"], str) or not result["status"]:
            raise ContractError(f"{where}.{mode}.status must be nonempty")
        _integer(result["rc"], f"{where}.{mode}.rc")
    return constrained


def _validate_memory_record(
    manifest: Mapping[str, Any], value: Any, where: str
) -> Mapping[str, Any]:
    memory = _mapping(value, where)
    _expect_keys(memory, set(manifest["evidence_contract"]["memory_fields"]), where)
    _integer(memory["native_allocation_bytes"], f"{where}.native_allocation_bytes")
    _integer(
        memory["repeated_call_retained_bytes"],
        f"{where}.repeated_call_retained_bytes",
    )
    classes = _mapping(memory["allocation_classes"], f"{where}.allocation_classes")
    _expect_keys(
        classes,
        set(manifest["evidence_contract"]["allocation_class_fields"]),
        f"{where}.allocation_classes",
    )
    for name, count in classes.items():
        _integer(count, f"{where}.allocation_classes.{name}")
    _validate_constrained(memory["constrained_memory"], f"{where}.constrained_memory")
    return memory


def _paired_metrics(
    pairs: Sequence[Mapping[str, Any]], t_critical: float
) -> dict[str, Any]:
    log_ratios = [
        math.log(float(pair["candidate"]["seconds"]))
        - math.log(float(pair["baseline"]["seconds"]))
        for pair in pairs
    ]
    mean_log = statistics.fmean(log_ratios)
    standard_deviation = statistics.stdev(log_ratios)
    standard_error = standard_deviation / math.sqrt(len(log_ratios))
    lower_log = mean_log - t_critical * standard_error
    upper_log = mean_log + t_critical * standard_error
    return {
        "pairs": len(log_ratios),
        "log_ratios": log_ratios,
        "mean_log_ratio": mean_log,
        "sample_standard_deviation_log_ratio": standard_deviation,
        "standard_error_log_ratio": standard_error,
        "confidence_interval_log_ratio": [lower_log, upper_log],
        "geometric_mean_ratio": math.exp(mean_log),
        "confidence_interval_ratio": [math.exp(lower_log), math.exp(upper_log)],
    }


def evaluate_evidence(
    manifest: Mapping[str, Any],
    evidence: Mapping[str, Any],
    *,
    manifest_sha256: str | None = None,
) -> dict[str, Any]:
    """Evaluate the closed evidence, applying science before performance."""
    validate_manifest(manifest)
    expected_manifest_sha = manifest_sha256 or _manifest_sha256(manifest)
    _sha256(expected_manifest_sha, "manifest sha256")
    profile, expected_ids, cells = _validate_evidence_header(
        manifest, evidence, expected_manifest_sha
    )

    scientific_failures: list[str] = []
    for cell_id in expected_ids:
        scientific = cells[cell_id]["scientific"]
        for reason in compare_scientific(
            manifest, scientific["baseline"], scientific["candidate"]
        ):
            scientific_failures.append(f"scientific:{cell_id}:{reason}")
    if scientific_failures:
        return {
            "status": "fail",
            "stage": "scientific",
            "reasons": scientific_failures,
            "profile": profile,
            "scientific_policy": manifest["scientific_rules"]["active_policy"],
            "scientific_cells_passed": len(expected_ids) - len({
                reason.split(":", 2)[1] for reason in scientific_failures
            }),
            "performance_evaluated": False,
            "cells": [],
        }

    performance_rules = manifest["performance_rules"]
    memory_rules = manifest["memory_rules"]
    failures: list[str] = []
    blockers: list[str] = []
    metrics: list[dict[str, Any]] = []
    primary_mean_logs: list[float] = []
    cell_specs = {cell["id"]: cell for cell in manifest["cells"]}
    for cell_id in expected_ids:
        cell = cells[cell_id]
        pairs = _validate_pairs(manifest, cell["pairs"], f"{cell_id}.pairs")
        initial = _paired_metrics(
            pairs, float(performance_rules["student_t_two_sided_95_df8"])
        )
        confirmation_value = cell["confirmation_pairs"]
        threshold = float(
            performance_rules["individual_geometric_mean_ratio_confirmation_threshold"]
        )
        confirmation: dict[str, Any] | None = None
        selected_pairs = pairs
        selected = initial
        confirmation_triggered = (
            initial["geometric_mean_ratio"] > threshold
            or initial["confidence_interval_log_ratio"][0] > 0.0
        )
        if confirmation_triggered:
            if confirmation_value is None:
                blockers.append(f"confirmation_required:{cell_id}")
            else:
                confirmation_pairs = _validate_pairs(
                    manifest,
                    confirmation_value,
                    f"{cell_id}.confirmation_pairs",
                )
                confirmation = _paired_metrics(
                    confirmation_pairs,
                    float(performance_rules["student_t_two_sided_95_df8"]),
                )
                selected_pairs = confirmation_pairs
                selected = confirmation
                if confirmation["geometric_mean_ratio"] > threshold:
                    failures.append(f"confirmed_individual_slowdown:{cell_id}")
                if confirmation["confidence_interval_log_ratio"][0] > 0.0:
                    failures.append(f"confirmed_reproducible_paired_slowdown:{cell_id}")
        elif confirmation_value is not None:
            raise ContractError(f"{cell_id} contains an unrequested confirmation rerun")

        memory_container = _mapping(cell["memory"], f"{cell_id}.memory")
        _expect_keys(memory_container, {"baseline", "candidate"}, f"{cell_id}.memory")
        baseline_memory = _validate_memory_record(
            manifest, memory_container["baseline"], f"{cell_id}.memory.baseline"
        )
        candidate_memory = _validate_memory_record(
            manifest, memory_container["candidate"], f"{cell_id}.memory.candidate"
        )
        if baseline_memory["allocation_classes"] != candidate_memory["allocation_classes"]:
            failures.append(f"data_sized_allocation_class_regression:{cell_id}")
        allocation_delta = (
            candidate_memory["native_allocation_bytes"]
            - baseline_memory["native_allocation_bytes"]
        )
        if allocation_delta > memory_rules["native_allocation_fixed_delta_bytes_max"]:
            failures.append(f"native_allocation_regression:{cell_id}")
        if candidate_memory["repeated_call_retained_bytes"] > memory_rules["repeated_call_retained_bytes_max"]:
            failures.append(f"repeated_call_leak:{cell_id}")

        baseline_constrained = baseline_memory["constrained_memory"]
        candidate_constrained = candidate_memory["constrained_memory"]
        if cell_id == memory_rules["constrained_cell"]:
            if baseline_constrained is None or candidate_constrained is None:
                failures.append(f"constrained_memory_evidence_missing:{cell_id}")
            else:
                expected_contract = memory_rules["constrained_contract"]
                if (
                    baseline_constrained["low"] != expected_contract["low"]
                    or baseline_constrained["fast"] != expected_contract["fast"]
                    or candidate_constrained["low"] != expected_contract["low"]
                    or candidate_constrained["fast"] != expected_contract["fast"]
                    or baseline_constrained["limit_bytes"]
                    != candidate_constrained["limit_bytes"]
                ):
                    failures.append(f"constrained_memory_contract:{cell_id}")
        elif baseline_constrained is not None or candidate_constrained is not None:
            raise ContractError(f"unexpected constrained-memory evidence in {cell_id}")

        baseline_peak_rss = max(pair["baseline"]["peak_rss_bytes"] for pair in selected_pairs)
        candidate_peak_rss = max(pair["candidate"]["peak_rss_bytes"] for pair in selected_pairs)
        rss_allowance = max(
            baseline_peak_rss * memory_rules["peak_rss_relative_delta_max"],
            memory_rules["peak_rss_absolute_delta_bytes_max"],
        )
        rss_delta = candidate_peak_rss - baseline_peak_rss
        if rss_delta > rss_allowance:
            failures.append(f"peak_rss_regression:{cell_id}")

        if cell_specs[cell_id]["primary"]:
            primary_mean_logs.append(selected["mean_log_ratio"])
        metrics.append(
            {
                "cell_id": cell_id,
                "environment": cell["environment"],
                "initial": initial,
                "confirmation": confirmation,
                "selected_geometric_mean_ratio": selected["geometric_mean_ratio"],
                "baseline_peak_rss_bytes": baseline_peak_rss,
                "candidate_peak_rss_bytes": candidate_peak_rss,
                "peak_rss_delta_bytes": rss_delta,
                "peak_rss_allowed_delta_bytes": rss_allowance,
                "native_allocation_delta_bytes": allocation_delta,
            }
        )

    aggregate = math.exp(statistics.fmean(primary_mean_logs))
    if aggregate > performance_rules["aggregate_geometric_mean_ratio_max"]:
        failures.append("aggregate_geometric_mean_ratio_above_1.02")
    failures = sorted(set(failures))
    blockers = sorted(set(blockers))
    if failures:
        status = "fail"
        stage = "performance_memory"
        reasons = failures
    else:
        if manifest["status"] != "frozen":
            blockers.append("baseline_candidate_unsealed")
        blockers = sorted(set(blockers))
        status = "blocked" if blockers else "pass"
        stage = "sealing" if blockers == ["baseline_candidate_unsealed"] else "performance_memory"
        reasons = blockers
    return {
        "status": status,
        "stage": stage,
        "reasons": reasons,
        "profile": profile,
        "scientific_policy": manifest["scientific_rules"]["active_policy"],
        "scientific_cells_passed": len(expected_ids),
        "performance_evaluated": True,
        "aggregate_primary_geometric_mean_ratio": aggregate,
        "aggregate_primary_cell_count": len(primary_mean_logs),
        "cells": metrics,
    }


def seal_payload(payload: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "payload": dict(payload),
        "seal": {
            "algorithm": "sha256",
            "canonicalization": "json-sort-keys-compact-ascii-v1",
            "payload_sha256": hashlib.sha256(canonical_json_bytes(payload)).hexdigest(),
        },
    }


def build_receipt(
    manifest: Mapping[str, Any],
    evidence: Mapping[str, Any],
    *,
    manifest_sha256: str,
) -> dict[str, Any]:
    evaluation = evaluate_evidence(
        manifest, evidence, manifest_sha256=manifest_sha256
    )
    payload = {
        "schema_version": 1,
        "receipt_type": "fereg-f0-regression-v1",
        "manifest": {"id": manifest["manifest_id"], "sha256": manifest_sha256},
        "evidence_sha256": hashlib.sha256(canonical_json_bytes(evidence)).hexdigest(),
        "evidence": dict(evidence),
        "evaluation": evaluation,
    }
    return seal_payload(payload)


def receipt_identity(document: Mapping[str, Any], encoded: bytes | None = None) -> dict[str, Any]:
    if encoded is None:
        encoded = canonical_json_bytes(document)
    seal = _mapping(document.get("seal"), "receipt seal")
    return {
        "schema_version": 1,
        "identity": "fereg-f0-regression-v1-receipt",
        "file_sha256": hashlib.sha256(encoded).hexdigest(),
        "payload_sha256": seal.get("payload_sha256"),
    }


def write_receipt(
    path: Path,
    document: Mapping[str, Any],
    *,
    identity_path: Path | None = None,
) -> dict[str, Any]:
    encoded = canonical_json_bytes(document)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded)
    identity = receipt_identity(document, encoded)
    if identity_path is not None:
        identity_path.parent.mkdir(parents=True, exist_ok=True)
        identity_path.write_bytes(canonical_json_bytes(identity))
    return identity


def verify_receipt(
    path: Path,
    manifest_path: Path = DEFAULT_MANIFEST,
    *,
    identity_path: Path | None = None,
) -> dict[str, Any]:
    try:
        encoded = path.read_bytes()
        document = json.loads(encoded)
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"unable to read F0 receipt: {error}") from error
    if not isinstance(document, Mapping):
        raise ContractError("receipt must be an object")
    _expect_keys(document, {"payload", "seal"}, "receipt")
    if encoded != canonical_json_bytes(document):
        raise ContractError("receipt is not in canonical JSON encoding")
    payload = _mapping(document["payload"], "receipt payload")
    seal = _mapping(document["seal"], "receipt seal")
    expected_seal = {
        "algorithm": "sha256",
        "canonicalization": "json-sort-keys-compact-ascii-v1",
        "payload_sha256": hashlib.sha256(canonical_json_bytes(payload)).hexdigest(),
    }
    if seal != expected_seal:
        raise ContractError("receipt payload seal mismatch")
    _expect_keys(
        payload,
        {"schema_version", "receipt_type", "manifest", "evidence_sha256", "evidence", "evaluation"},
        "receipt payload",
    )
    if payload["schema_version"] != 1 or payload["receipt_type"] != "fereg-f0-regression-v1":
        raise ContractError("receipt identity is invalid")
    manifest = load_manifest(manifest_path)
    manifest_hash = sha256_file(manifest_path)
    if payload["manifest"] != {"id": manifest["manifest_id"], "sha256": manifest_hash}:
        raise ContractError("receipt manifest binding is stale or substituted")
    if payload["evidence_sha256"] != hashlib.sha256(canonical_json_bytes(payload["evidence"])).hexdigest():
        raise ContractError("receipt evidence hash mismatch")
    expected_evaluation = evaluate_evidence(
        manifest, payload["evidence"], manifest_sha256=manifest_hash
    )
    if payload["evaluation"] != expected_evaluation:
        raise ContractError("receipt evaluation was modified or produced by another contract")
    if identity_path is not None:
        try:
            external = json.loads(identity_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise ContractError(f"unable to read receipt identity: {error}") from error
        expected_identity = receipt_identity(document, encoded)
        if external != expected_identity:
            raise ContractError("external receipt identity mismatch")
    return dict(document)


def _load_json(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"unable to read {where}: {error}") from error
    return _mapping(value, where)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    validate = subparsers.add_parser("validate-manifest")
    validate.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)

    identity = subparsers.add_parser("source-identity")
    identity.add_argument("--repository", type=Path, default=ROOT)
    identity.add_argument("--revision", default="HEAD")
    identity.add_argument("--allow-tracked-dirty", action="store_true")

    evaluate = subparsers.add_parser("evaluate")
    evaluate.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    evaluate.add_argument("--evidence", type=Path, required=True)
    evaluate.add_argument("--output", type=Path, required=True)
    evaluate.add_argument("--identity-output", type=Path)

    verify = subparsers.add_parser("verify-receipt")
    verify.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    verify.add_argument("--receipt", type=Path, required=True)
    verify.add_argument("--identity", type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "validate-manifest":
            manifest = load_manifest(args.manifest)
            print(
                json.dumps(
                    {
                        "manifest_id": manifest["manifest_id"],
                        "status": manifest["status"],
                        "sha256": sha256_file(args.manifest),
                    },
                    sort_keys=True,
                )
            )
            return 0
        if args.command == "source-identity":
            identity = source_identity(
                args.repository,
                args.revision,
                require_clean=not args.allow_tracked_dirty,
            )
            print(json.dumps(identity, sort_keys=True))
            return 0
        if args.command == "evaluate":
            manifest = load_manifest(args.manifest)
            evidence = _load_json(args.evidence, "F0 evidence")
            manifest_hash = sha256_file(args.manifest)
            document = build_receipt(
                manifest, evidence, manifest_sha256=manifest_hash
            )
            identity = write_receipt(
                args.output, document, identity_path=args.identity_output
            )
            status = document["payload"]["evaluation"]["status"]
            print(json.dumps({"status": status, "receipt_identity": identity}, sort_keys=True))
            return 0 if status == "pass" else 2
        if args.command == "verify-receipt":
            document = verify_receipt(
                args.receipt, args.manifest, identity_path=args.identity
            )
            print(json.dumps(document["payload"]["evaluation"], sort_keys=True))
            return 0
    except ContractError as error:
        print(f"fereg F0 regression contract error: {error}", file=sys.stderr)
        return 2
    raise AssertionError("unreachable command")


if __name__ == "__main__":
    raise SystemExit(main())
