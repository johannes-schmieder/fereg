#!/usr/bin/env python3
"""Evaluate direct fereg candidate-versus-F0 regression evidence.

The module deliberately does not run Stata. Platform runners produce the
closed evidence schema frozen in ``fereg_f0_regression_v1.json``; this module
then validates identities, applies the scientific gate before inspecting any
timing, evaluates paired performance and memory, and writes a sealed receipt.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import statistics
import subprocess
import sys
from typing import Any, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "benchmarks/manifests/fereg_f0_regression_v1.json"
DARWIN_GIT = Path("/usr/bin/git")
DARWIN_GIT_SHA256 = "b8763cf250e607a778bb4603cecb5b90338814d0a3dfcba0d57b1de242f610e9"
DARWIN_GIT_SIZE = 118_640
HEX_SHA1 = re.compile(r"^[0-9a-f]{40}$")
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
LOCAL_SCREEN_CELL_IDS = (
    'local_n100k_dense_two_fe_t1_fast_fit',
    'local_n100k_dense_two_fe_t8_fast_fit',
    'local_n100k_sparse_two_fe_t1_fast_fit',
    'local_n100k_sparse_two_fe_t8_fast_fit',
    'local_n100k_absorbed_slope_t1_fast_fit',
    'local_n100k_clustered_vce_t1_fast_fit',
    'local_n100k_moderate_k_t1_fast_fit',
    'local_n100k_dense_two_fe_t1_low_fit',
    'local_n100k_dense_two_fe_t8_fast_save_fe',
    'local_n1m_dense_two_fe_t1_fast_fit',
    'local_n1m_dense_two_fe_t8_fast_fit',
    'local_n1m_sparse_two_fe_t1_fast_fit',
    'local_n1m_sparse_two_fe_t8_fast_fit',
    'local_n1m_absorbed_slope_t8_fast_fit',
    'local_n1m_clustered_vce_t8_fast_fit',
    'local_n1m_moderate_k_t8_fast_fit',
    'local_n1m_dense_two_fe_t1_low_fit',
    'local_n1m_dense_two_fe_t8_fast_save_fe',
    'local_n100k_robust_t1_fast_fit',
    'local_n100k_robust_t8_fast_fit',
    'local_n100k_robust_t1_low_fit',
    'local_n100k_fweight_unadjusted_t1_fast_fit',
    'local_n100k_fweight_robust_t1_fast_fit',
    'local_n100k_pweight_robust_t1_fast_fit',
    'local_n100k_pweight_cluster2_t1_fast_fit',
    'local_n100k_cluster1_t1_fast_fit',
    'local_n100k_cluster2_t1_fast_fit',
    'local_n100k_cluster3_t1_fast_fit',
    'local_n100k_cluster4_t1_fast_fit',
    'local_n100k_cluster5_t1_fast_fit',
    'local_n100k_cluster1_t1_low_fit',
    'local_n100k_cluster2_t1_low_fit',
    'local_n100k_cluster3_t1_low_fit',
    'local_n100k_cluster4_t1_low_fit',
    'local_n100k_cluster5_t1_low_fit',
    'local_n100k_moderate_k_cluster2_t8_fast_fit',
    'local_n1m_disconnected_two_fe_t1_fast_save_fe',
    'local_n1m_saved_slope_t1_fast_save_fe',
    'local_n100k_solver_map_t1_fast_fit',
    'local_n100k_solver_cg_t1_fast_fit',
    'local_n100k_solver_it_t1_fast_fit',
    'local_n100k_solver_squarem_t1_fast_fit',
    'local_n100k_solver_lsmr_off_t1_fast_fit',
    'local_n100k_solver_lsmr_diagonal_t1_fast_fit',
    'local_n100k_solver_lsmr_schwarz_t1_fast_fit',
    'local_n100k_solver_cmg_t1_fast_fit',
)
SCC_GOVERNING_CELL_IDS = (
    'dense_two_fe_t1_fast_fit',
    'dense_two_fe_t8_fast_fit',
    'sparse_two_fe_t1_fast_fit',
    'sparse_two_fe_t8_fast_fit',
    'absorbed_slope_t1_fast_fit',
    'absorbed_slope_t8_fast_fit',
    'clustered_vce_t1_fast_fit',
    'clustered_vce_t8_fast_fit',
    'moderate_k_t1_fast_fit',
    'moderate_k_t8_fast_fit',
    'dense_two_fe_t1_low_fit',
    'sparse_two_fe_t8_low_fit',
    'dense_two_fe_t8_fast_save_fe',
    'constrained_memory_t1_low',
    'robust_t1_fast_fit',
    'robust_t8_fast_fit',
    'robust_t1_low_fit',
    'fweight_unadjusted_t1_fast_fit',
    'fweight_robust_t1_fast_fit',
    'pweight_robust_t1_fast_fit',
    'pweight_cluster2_t1_fast_fit',
    'cluster1_t1_fast_fit',
    'cluster2_t1_fast_fit',
    'cluster3_t1_fast_fit',
    'cluster4_t1_fast_fit',
    'cluster5_t1_fast_fit',
    'cluster1_t1_low_fit',
    'cluster2_t1_low_fit',
    'cluster3_t1_low_fit',
    'cluster4_t1_low_fit',
    'cluster5_t1_low_fit',
    'moderate_k_cluster2_t8_fast_fit',
    'disconnected_two_fe_t8_fast_save_fe',
    'saved_slope_t8_fast_save_fe',
    'solver_map_t1_fast_fit',
    'solver_cg_t1_fast_fit',
    'solver_it_t1_fast_fit',
    'solver_squarem_t1_fast_fit',
    'solver_lsmr_off_t1_fast_fit',
    'solver_lsmr_diagonal_t1_fast_fit',
    'solver_lsmr_schwarz_t1_fast_fit',
    'solver_cmg_t1_fast_fit',
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
CONTROLLER_SOURCE_FIELDS = {"commit", "tree"}
SOURCE_ARCHIVE_FIELDS = {"sha256", "size_bytes"}
FILE_BINDING_FIELDS = {"path", "sha256", "size_bytes"}
SEALED_FILE_BINDING_FIELDS = FILE_BINDING_FIELDS | {"payload_sha256"}
ARTIFACT_SEAL_TYPE = "fereg-f0-artifact-seal-v1"
QUALIFICATION_SEAL_TYPE = "fereg-f0-qualification-seal-v1"
DIRECT_BEHAVIOR_OBSERVATION_TYPE = "fereg_f0_behavior_observation_v1"
UNSEALED_MISSING_EVIDENCE = [
    "direct_f0_behavioral_package_contract",
    "macos_universal_licensed_stata_load",
    "linux_x86_64_scc_setup_build_loader_qacct",
    "scc_governing_scientific_performance_memory_qacct",
    "windows_x86_64_build_audit_mock_or_licensed_load",
]
ARTIFACT_SEALED_MISSING_EVIDENCE = [
    "scc_governing_scientific_performance_memory_qacct",
]


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
    if (
        sys.platform != "darwin"
        or DARWIN_GIT.is_symlink()
        or not DARWIN_GIT.is_file()
        or sha256_file(DARWIN_GIT) != DARWIN_GIT_SHA256
        or DARWIN_GIT.stat().st_size != DARWIN_GIT_SIZE
    ):
        raise ContractError("exact macOS Git controller changed")
    try:
        completed = subprocess.run(
            [str(DARWIN_GIT), "--no-replace-objects", "-C", str(repository), *arguments],
            env={
                "HOME": "/private/tmp/fereg-f0-git-home",
                "PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C",
                "TZ": "UTC", "GIT_CONFIG_NOSYSTEM": "1",
                "GIT_CONFIG_GLOBAL": "/dev/null",
                "GIT_NO_REPLACE_OBJECTS": "1",
            },
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
        dirty = _git(repository, ["status", "--porcelain", "--untracked-files=all"])
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


def _validate_controller_source(value: Mapping[str, Any], where: str) -> None:
    _expect_keys(value, CONTROLLER_SOURCE_FIELDS, where)
    _sha1(value["commit"], f"{where}.commit")
    _sha1(value["tree"], f"{where}.tree")


def _validate_source_archive(value: Mapping[str, Any], where: str) -> None:
    _expect_keys(value, SOURCE_ARCHIVE_FIELDS, where)
    _sha256(value["sha256"], f"{where}.sha256")
    _integer(value["size_bytes"], f"{where}.size_bytes", minimum=1)


def _bound_path(root: Path, value: Any, where: str) -> Path:
    if not isinstance(value, str) or not value or "\\" in value:
        raise ContractError(f"{where}.path must be a nonempty repository-relative POSIX path")
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts or relative.as_posix() != value:
        raise ContractError(f"{where}.path escapes the evidence root")
    root = root.resolve()
    current = root
    for component in relative.parts:
        current = current / component
        if current.is_symlink():
            raise ContractError(f"{where}.path traverses a symlink")
    resolved = current.resolve()
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise ContractError(f"{where}.path escapes the evidence root") from error
    if not resolved.is_file():
        raise ContractError(f"{where} bytes are missing")
    return resolved


def _validate_file_binding(
    value: Any,
    root: Path,
    where: str,
    *,
    sealed: bool = False,
) -> tuple[Path, Mapping[str, Any]]:
    record = _mapping(value, where)
    _expect_keys(
        record,
        SEALED_FILE_BINDING_FIELDS if sealed else FILE_BINDING_FIELDS,
        where,
    )
    path = _bound_path(root, record["path"], where)
    _sha256(record["sha256"], f"{where}.sha256")
    _integer(record["size_bytes"], f"{where}.size_bytes", minimum=1)
    if path.stat().st_size != record["size_bytes"] or sha256_file(path) != record["sha256"]:
        raise ContractError(f"{where} bytes are missing or changed")
    if sealed:
        _sha256(record["payload_sha256"], f"{where}.payload_sha256")
    return path, record


def file_binding(path: Path, root: Path = ROOT, *, sealed: bool = False) -> dict[str, Any]:
    """Return the exact repository-relative identity used by seal records."""
    root = root.resolve()
    resolved = path.resolve()
    try:
        relative = resolved.relative_to(root).as_posix()
    except ValueError as error:
        raise ContractError("bound evidence must live below the evidence root") from error
    if path.is_symlink() or not resolved.is_file():
        raise ContractError("bound evidence must be a regular non-symlink file")
    record: dict[str, Any] = {
        "path": relative,
        "sha256": sha256_file(resolved),
        "size_bytes": resolved.stat().st_size,
    }
    if sealed:
        document = _load_json(resolved, "sealed evidence")
        seal = _mapping(document.get("seal"), "sealed evidence seal")
        record["payload_sha256"] = _sha256(
            seal.get("payload_sha256"), "sealed evidence payload_sha256"
        )
    return record


def _read_bound_json(
    value: Any,
    root: Path,
    where: str,
    *,
    sealed: bool = False,
    scc_seal: bool = False,
    behavior_seal: bool = False,
    qualification_seal: bool = False,
) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    path, record = _validate_file_binding(value, root, where, sealed=sealed)
    document = _load_json(path, where)
    if not sealed:
        return document, record
    _expect_keys(document, {"payload", "seal"}, f"{where} envelope")
    payload = _mapping(document["payload"], f"{where} payload")
    seal = _mapping(document["seal"], f"{where} seal")
    seal_fields = (
        {"algorithm", "payload_sha256"}
        if behavior_seal
        else {"algorithm", "canonicalization", "payload_sha256"}
    )
    _expect_keys(seal, seal_fields, f"{where} seal")
    encoded = json.dumps(
        payload,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")
    if not scc_seal and not behavior_seal:
        encoded += b"\n"
    expected: dict[str, Any] = {
        "algorithm": "sha256",
        "payload_sha256": hashlib.sha256(encoded).hexdigest(),
    }
    if not behavior_seal:
        expected["canonicalization"] = (
            "json-sort-keys-compact-ascii-newline-v1"
            if qualification_seal
            else "json-sort-keys-compact-ascii-v1"
        )
    if seal != expected or record["payload_sha256"] != expected["payload_sha256"]:
        raise ContractError(f"{where} payload seal mismatch")
    return payload, record


def _artifact_identity(artifact: Mapping[str, Any]) -> dict[str, Any]:
    return {field: artifact[field] for field in ARTIFACT_FIELDS}


def _validate_local_full_receipt(
    receipt: Mapping[str, Any],
    source: Mapping[str, Any],
    artifact: Mapping[str, Any],
    where: str,
) -> None:
    expected = {
        "tested_sha": source["commit"],
        "plugin_sha256": artifact["sha256"],
        "stata_bundle_version": "19.0.115",
        "stata_edition": "MP",
        "stata_processors": 8,
        "tests_passed": 35,
        "tests_failed": 0,
        "rust_status": "success",
        "stata_status": "success",
        "status": "success",
        "process_rc": 0,
        "rust_rc": 0,
        "stata_rc": 0,
    }
    for field, expected_value in expected.items():
        if receipt.get(field) != expected_value:
            raise ContractError(f"{where} contradicts required field {field}")


def _verify_baseline_behavior_evidence(
    value: Any,
    source_root: Path,
    evidence_root: Path,
    source: Mapping[str, Any],
    artifact: Mapping[str, Any],
) -> Mapping[str, Any]:
    """Reopen the baseline observation's immutable raw-evidence bundle."""

    evidence = _mapping(value, "direct F0 behavior evidence")
    _expect_keys(
        evidence,
        {"observation", "bundle", "verified_observation"},
        "direct F0 behavior evidence",
    )
    observation_path, _ = _validate_file_binding(
        evidence["observation"],
        evidence_root,
        "direct F0 behavior observation",
        sealed=True,
    )
    bundle_path, _ = _validate_file_binding(
        evidence["bundle"], evidence_root, "direct F0 behavior bundle"
    )
    module, _, _ = _load_behavior_contract(source_root)
    behavior_manifest = (
        source_root / "benchmarks/manifests/fereg_f0_behavior_v1.json"
    )
    try:
        verified = module.verify_observation(
            observation_path,
            bundle_path,
            behavior_manifest,
            expected_role="baseline",
            expected_phase="baseline",
        )
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise ContractError(
            f"direct F0 behavior evidence fails authoritative verification: {error}"
        ) from error
    verified = _mapping(verified, "verified direct F0 behavior observation")
    if evidence["verified_observation"] != verified:
        raise ContractError(
            "direct F0 behavior record differs from authoritative verification"
        )
    if (
        verified.get("status") != "pass"
        or verified.get("source") != source
        or verified.get("artifact") != artifact
    ):
        raise ContractError("direct F0 behavior substitutes source or artifact bytes")
    return verified


def _load_behavior_contract(root: Path) -> tuple[Any, Mapping[str, Any], str]:
    behavior_script = root / "scripts/fereg_f0_behavior.py"
    behavior_manifest_path = (
        root / "benchmarks/manifests/fereg_f0_behavior_v1.json"
    )
    spec = importlib.util.spec_from_file_location(
        f"fereg_f0_behavior_evidence_{sha256_file(behavior_script)[:12]}",
        behavior_script,
    )
    if spec is None or spec.loader is None:
        raise ContractError("direct F0 behavior validator is unavailable")
    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
        manifest = _load_json(
            behavior_manifest_path, "direct F0 behavior contract"
        )
        module.validate_manifest(manifest, root=root)
    except (OSError, ValueError) as error:
        raise ContractError(
            f"direct F0 behavior contract is unavailable or stale: {error}"
        ) from error
    return module, manifest, sha256_file(behavior_manifest_path)


def _behavior_document(
    value: Any,
    module: Any,
    where: str,
) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    document = _mapping(value, where)
    _expect_keys(document, {"payload", "seal"}, where)
    payload = _mapping(document["payload"], f"{where} payload")
    seal = _mapping(document["seal"], f"{where} seal")
    if seal != module.seal_payload(payload)["seal"]:
        raise ContractError(f"{where} payload seal mismatch")
    return payload, document


def _validate_candidate_behavior_evidence(
    value: Any,
    root: Path,
    evidence_root: Path,
    baseline_source: Mapping[str, Any],
    candidate_source: Mapping[str, Any],
    baseline_artifacts: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Reopen both immutable bundles and reproduce the comparison exactly.

    Direct behavior is a target-native licensed-host comparison.  It therefore
    shares source identity with the governing Linux SCC comparison but carries
    its own platform artifact identity.  The baseline native artifact must be
    one of the exact frozen F0 artifacts; the candidate native artifact is
    exposed for the aggregate gate to bind to that platform's independently
    verified build/load receipt.
    """

    evidence = _mapping(value, "candidate direct behavior evidence")
    _expect_keys(
        evidence,
        {
            "baseline_observation", "baseline_bundle",
            "candidate_observation", "candidate_bundle", "comparison",
            "verified_comparison",
        },
        "candidate direct behavior evidence",
    )
    paths: dict[str, Path] = {}
    for field in (
        "baseline_observation", "candidate_observation", "comparison",
    ):
        paths[field], _ = _validate_file_binding(
            evidence[field],
            evidence_root,
            f"candidate behavior {field.replace('_', ' ')}",
            sealed=True,
        )
    for field in ("baseline_bundle", "candidate_bundle"):
        paths[field], _ = _validate_file_binding(
            evidence[field],
            evidence_root,
            f"candidate behavior {field.replace('_', ' ')}",
        )
    module, _, _ = _load_behavior_contract(root)
    behavior_manifest = root / "benchmarks/manifests/fereg_f0_behavior_v1.json"
    try:
        comparison = module.verify_comparison(
            paths["comparison"],
            paths["baseline_observation"],
            paths["baseline_bundle"],
            paths["candidate_observation"],
            paths["candidate_bundle"],
            behavior_manifest,
        )
        baseline = module.verify_observation(
            paths["baseline_observation"],
            paths["baseline_bundle"],
            behavior_manifest,
            expected_role="baseline",
            expected_phase="baseline",
        )
        candidate = module.verify_observation(
            paths["candidate_observation"],
            paths["candidate_bundle"],
            behavior_manifest,
            expected_role="candidate",
        )
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise ContractError(
            f"candidate direct behavior evidence fails its authoritative contract: {error}"
        ) from error
    if evidence["verified_comparison"] != comparison:
        raise ContractError(
            "direct behavior comparison differs from authoritative verification"
        )
    baseline_behavior_artifact = _mapping(
        baseline.get("artifact"), "direct behavior baseline artifact"
    )
    candidate_behavior_artifact = _mapping(
        candidate.get("artifact"), "direct behavior candidate artifact"
    )
    _validate_artifact(
        baseline_behavior_artifact, "direct behavior baseline artifact"
    )
    _validate_artifact(
        candidate_behavior_artifact, "direct behavior candidate artifact"
    )
    if baseline.get("source") != baseline_source:
        raise ContractError("direct behavior baseline substitutes frozen F0 source")
    if candidate.get("source") != candidate_source:
        raise ContractError("direct behavior candidate substitutes tested source")
    behavior_platform = baseline_behavior_artifact["platform"]
    if candidate_behavior_artifact["platform"] != behavior_platform:
        raise ContractError("direct behavior artifacts target different platforms")
    matching_baselines = [
        artifact
        for artifact in baseline_artifacts
        if isinstance(artifact, Mapping)
        and artifact.get("platform") == behavior_platform
    ]
    if (
        len(matching_baselines) != 1
        or matching_baselines[0].get("status") != "qualified"
        or _artifact_identity(matching_baselines[0])
        != baseline_behavior_artifact
    ):
        raise ContractError(
            "direct behavior baseline substitutes the frozen native F0 artifact"
        )
    return {
        "phase": candidate["phase"],
        "platform": behavior_platform,
        "baseline_source": dict(baseline["source"]),
        "candidate_source": dict(candidate["source"]),
        "baseline_artifact": dict(baseline_behavior_artifact),
        "candidate_artifact": dict(candidate_behavior_artifact),
        "stata_environment": dict(candidate["stata_environment"]),
        "comparison_payload_sha256": evidence["comparison"]["payload_sha256"],
        "baseline_bundle_sha256": evidence["baseline_bundle"]["sha256"],
        "candidate_bundle_sha256": evidence["candidate_bundle"]["sha256"],
    }


def _bound_directory(root: Path, value: Any, where: str) -> Path:
    if not isinstance(value, str) or not value or "\\" in value:
        raise ContractError(f"{where} must be a repository-relative POSIX directory")
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts or relative.as_posix() != value:
        raise ContractError(f"{where} escapes the evidence root")
    root = root.resolve()
    current = root
    for component in relative.parts:
        current = current / component
        if current.is_symlink():
            raise ContractError(f"{where} traverses a symlink")
    resolved = current.resolve()
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise ContractError(f"{where} escapes the evidence root") from error
    if not resolved.is_dir() or resolved.is_symlink():
        raise ContractError(f"{where} is not an existing regular directory")
    return resolved


def _load_repository_module(root: Path, relative: str, label: str) -> Any:
    path = (root / relative).resolve()
    try:
        path.relative_to(root.resolve())
    except ValueError as error:
        raise ContractError(f"{label} escapes the evidence root") from error
    if not path.is_file() or path.is_symlink():
        raise ContractError(f"{label} is unavailable")
    spec = importlib.util.spec_from_file_location(
        f"{label.replace('-', '_')}_{sha256_file(path)[:12]}", path
    )
    if spec is None or spec.loader is None:
        raise ContractError(f"{label} cannot be loaded")
    module = importlib.util.module_from_spec(spec)
    root_text = str(root.resolve())
    inserted = root_text not in sys.path
    if inserted:
        sys.path.insert(0, root_text)
    try:
        spec.loader.exec_module(module)
    except (ImportError, OSError, ValueError) as error:
        raise ContractError(f"{label} cannot be loaded: {error}") from error
    finally:
        if inserted:
            sys.path.remove(root_text)
    return module


def _platform_paths(
    value: Any,
    root: Path,
    platform: str,
) -> tuple[Mapping[str, Any], dict[str, Path]]:
    evidence = _mapping(value, f"{platform} platform evidence")
    common = {
        "artifact", "capture", "receipt", "record", "source_archive",
        "capture_program", "verified_receipt",
    }
    platform_specific = (
        {"native_attestation", "native_producer"}
        if platform == "macos-universal"
        else {
            "windows_controller_receipt", "windows_controller_output",
            "windows_controller_archive", "windows_results_archive",
            "windows_source_manifest",
        }
    )
    _expect_keys(
        evidence,
        common | platform_specific,
        f"{platform} platform evidence",
    )
    paths: dict[str, Path] = {}
    for field in sorted((common - {"verified_receipt"}) | platform_specific):
        paths[field], _ = _validate_file_binding(
            evidence[field], root, f"{platform} {field.replace('_', ' ')}"
        )
    return evidence, paths


def _platform_controller_matches(
    platform: str,
    verified: Mapping[str, Any],
    controller_source: Mapping[str, Any],
) -> bool:
    provenance = _mapping(
        verified.get("platform_provenance"),
        f"{platform} verified platform provenance",
    )
    if platform == "windows-x86_64":
        return (
            provenance.get("harness_commit") == controller_source["commit"]
            and provenance.get("harness_tree") == controller_source["tree"]
        )
    return (
        verified.get("controller_commit") == controller_source["commit"]
        and verified.get("controller_tree") == controller_source["tree"]
    )


def _verify_portable_platform_evidence(
    value: Any,
    source_root: Path,
    evidence_root: Path,
    platform: str,
    source: Mapping[str, Any],
    controller_source: Mapping[str, Any],
    artifact: Mapping[str, Any],
    target_source_archive: Mapping[str, Any],
) -> Mapping[str, Any]:
    """Delegate all platform receipt schemas to the shared verifier."""

    evidence, paths = _platform_paths(value, evidence_root, platform)
    verifier = _load_repository_module(
        source_root,
        "scripts/feppml_platform_load_receipt.py",
        "feppml_platform_load_receipt",
    )
    arguments: dict[str, Any] = {
        "receipt": paths["receipt"],
        "record": paths["record"],
        "artifact": paths["artifact"],
        "capture_path": paths["capture"],
        "source_archive": paths["source_archive"],
        "capture_program": paths["capture_program"],
    }
    if platform == "macos-universal":
        arguments["native_attestation"] = paths["native_attestation"]
        arguments["native_producer"] = paths["native_producer"]
    else:
        arguments.update(
            {
                "windows_controller_receipt": paths[
                    "windows_controller_receipt"
                ],
                "windows_controller_output": paths["windows_controller_output"],
                "windows_controller_archive": paths[
                    "windows_controller_archive"
                ],
                "windows_results_archive": paths["windows_results_archive"],
                "windows_source_manifest": paths["windows_source_manifest"],
            }
        )
    try:
        verified = verifier.verify_portable(**arguments)
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise ContractError(
            f"{platform} evidence fails the authoritative verifier: {error}"
        ) from error
    verified = _mapping(verified, f"{platform} verified receipt")
    if evidence["verified_receipt"] != verified:
        raise ContractError(
            f"{platform} recorded receipt differs from authoritative verification"
        )
    expected_artifact = {
        "platform": platform,
        "sha256": verified.get("artifact_sha256"),
        "size_bytes": verified.get("artifact_size_bytes"),
        "embedded_git_commit": verified.get("embedded_git_commit"),
    }
    if (
        verified.get("platform") != platform
        or verified.get("qualification_profile") != "f0-baseline"
        or verified.get("source_commit") != source["commit"]
        or verified.get("source_tree") != source["tree"]
        or verified.get("source_archive_sha256")
        != target_source_archive["sha256"]
        or verified.get("source_archive_size_bytes")
        != target_source_archive["size_bytes"]
        or not _platform_controller_matches(
            platform, verified, controller_source
        )
        or expected_artifact != artifact
        or verified.get("licensed_stata_load") is not True
        or verified.get("mock_only") is not False
        or verified.get("scientific_claim") is not False
    ):
        raise ContractError(f"{platform} evidence substitutes source or artifact bytes")
    return verified


def _verify_linux_platform_evidence(
    value: Any,
    source_root: Path,
    evidence_root: Path,
    source: Mapping[str, Any],
    controller_source: Mapping[str, Any],
    artifact: Mapping[str, Any],
    target_source_archive: Mapping[str, Any],
) -> Mapping[str, Any]:
    """Use the Linux bootstrap's sole closed-run schema authority."""

    evidence = _mapping(value, "Linux platform evidence")
    _expect_keys(
        evidence,
        {"run_root", "verified_close"},
        "Linux platform evidence",
    )
    run_root = _bound_directory(
        evidence_root, evidence["run_root"], "Linux platform run root"
    )
    verifier = _load_repository_module(
        source_root,
        "benchmarks/scc/feppml_linux_platform_scc.py",
        "feppml_linux_platform_scc",
    )
    try:
        verified = verifier.verify_closed_run(
            run_root,
            expected_source_commit=source["commit"],
            expected_harness_commit=controller_source["commit"],
        )
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise ContractError(
            f"Linux evidence fails the authoritative closed-run verifier: {error}"
        ) from error
    verified = _mapping(verified, "Linux authoritative close")
    if evidence["verified_close"] != verified:
        raise ContractError(
            "Linux recorded close differs from authoritative verification"
        )
    linux_source = _mapping(verified.get("source"), "Linux verified target source")
    linux_controller = _mapping(
        verified.get("harness_source"), "Linux verified controller source"
    )
    linux_artifact = _mapping(verified.get("artifact"), "Linux verified artifact")
    if (
        verified.get("record_type") != "feppml-linux-platform-closed-run-v1"
        or linux_source.get("commit") != source["commit"]
        or linux_source.get("tree") != source["tree"]
        or linux_source.get("archive_sha256") != target_source_archive["sha256"]
        or linux_source.get("archive_size_bytes")
        != target_source_archive["size_bytes"]
        or linux_source.get("embedded_git_commit")
        != artifact["embedded_git_commit"]
        or linux_controller.get("commit") != controller_source["commit"]
        or linux_controller.get("tree") != controller_source["tree"]
        or linux_artifact.get("sha256") != artifact["sha256"]
        or linux_artifact.get("size_bytes") != artifact["size_bytes"]
        or verified.get("licensed_stata_load") is not True
        or verified.get("mock_only") is not False
        or verified.get("scientific_claim") is not False
    ):
        raise ContractError("Linux closed run substitutes source or artifact bytes")
    return verified


def _validate_artifact_seal(
    manifest: Mapping[str, Any],
    value: Any,
    source_root: Path,
    evidence_root: Path,
) -> None:
    payload, _ = _read_bound_json(
        value,
        evidence_root,
        "F0 artifact seal",
        sealed=True,
    )
    _expect_keys(
        payload,
        {
            "schema_version", "record_type", "baseline_candidate",
            "controller_source", "target_source_archive", "evidence",
            "terminal_sentinel",
        },
        "F0 artifact seal payload",
    )
    controller_source = _mapping(
        payload["controller_source"], "F0 artifact seal controller source"
    )
    target_source_archive = _mapping(
        payload["target_source_archive"], "F0 artifact seal target source archive"
    )
    _validate_controller_source(controller_source, "F0 artifact seal controller source")
    _validate_source_archive(
        target_source_archive, "F0 artifact seal target source archive"
    )
    if (
        payload["schema_version"] != 1
        or payload["record_type"] != ARTIFACT_SEAL_TYPE
        or payload["baseline_candidate"] != manifest["baseline_candidate"]
        or controller_source != manifest["sealing"]["controller_source"]
        or payload["terminal_sentinel"] != "FEREG_F0_ARTIFACT_SEAL_COMPLETE"
    ):
        raise ContractError("F0 artifact seal identity is stale or substituted")
    evidence = _mapping(payload["evidence"], "F0 artifact seal evidence")
    _expect_keys(
        evidence,
        {
            "local_full_receipt", "direct_behavior", "macos",
            "linux", "windows",
        },
        "F0 artifact seal evidence",
    )
    source = manifest["baseline_candidate"]["source"]
    artifacts = {
        item["platform"]: _artifact_identity(item)
        for item in manifest["baseline_candidate"]["artifacts"]
    }
    local, local_record = _read_bound_json(
        evidence["local_full_receipt"],
        evidence_root,
        "exact-SHA local F0 receipt",
    )
    registered = manifest["sealing"]["observed_local_evidence"][0]
    if (
        local_record["path"] != registered["receipt_path"]
        or local_record["sha256"] != registered["receipt_sha256"]
    ):
        raise ContractError("artifact seal substitutes the observed local F0 receipt")
    _validate_local_full_receipt(local, source, artifacts["macos-universal"], "local F0 receipt")
    _verify_baseline_behavior_evidence(
        evidence["direct_behavior"],
        source_root,
        evidence_root,
        source,
        artifacts["macos-universal"],
    )
    _verify_portable_platform_evidence(
        evidence["macos"],
        source_root,
        evidence_root,
        "macos-universal",
        source,
        controller_source,
        artifacts["macos-universal"],
        target_source_archive,
    )
    _verify_linux_platform_evidence(
        evidence["linux"],
        source_root,
        evidence_root,
        source,
        controller_source,
        artifacts["linux-x86_64"],
        target_source_archive,
    )
    _verify_portable_platform_evidence(
        evidence["windows"],
        source_root,
        evidence_root,
        "windows-x86_64",
        source,
        controller_source,
        artifacts["windows-x86_64"],
        target_source_archive,
    )


def load_manifest(
    path: Path = DEFAULT_MANIFEST,
    *,
    verify_git_object: bool = True,
    root: Path = ROOT,
    evidence_root: Path | None = None,
) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"unable to read F0 manifest: {error}") from error
    validate_manifest(value, root=root, evidence_root=evidence_root)
    if verify_git_object:
        expected = value["baseline_candidate"]["source"]
        observed = source_identity(root, expected["commit"], require_clean=False)
        if expected != observed:
            raise ContractError(
                f"F0 source object no longer matches manifest; expected={expected}, observed={observed}"
            )
    return value


def validate_manifest(
    manifest: Mapping[str, Any],
    *,
    root: Path = ROOT,
    evidence_root: Path | None = None,
) -> None:
    seal_root = (evidence_root or root).resolve()
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
    if manifest["status"] not in {"candidate_unsealed", "artifacts_sealed", "frozen"}:
        raise ContractError(
            "manifest status must be candidate_unsealed, artifacts_sealed, or frozen"
        )

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
        "action_protocol": "1.6",
        "model_wire": "2.1",
        "ols_result_wire": "1.5",
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
        "outputs", "vce", "weight", "cluster_dimensions", "solver",
        "preconditioner", "primary",
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
        if cell["design"] not in {
            "dense_two_fe", "sparse_two_fe", "absorbed_slope",
            "clustered_vce", "moderate_k", "constrained_memory",
            "disconnected_two_fe", "saved_slope",
        }:
            raise ContractError(f"manifest cell {cell['id']} has an unknown design")
        if cell["vce"] not in {"unadjusted", "robust", "cluster"}:
            raise ContractError(f"manifest cell {cell['id']} has an unknown VCE")
        if cell["weight"] not in {"unweighted", "fweight", "pweight"}:
            raise ContractError(f"manifest cell {cell['id']} has an unknown weight")
        if (
            isinstance(cell["cluster_dimensions"], bool)
            or not isinstance(cell["cluster_dimensions"], int)
            or cell["cluster_dimensions"] < 0
            or cell["cluster_dimensions"] > 5
            or (cell["vce"] == "cluster")
            != (cell["cluster_dimensions"] > 0)
        ):
            raise ContractError(
                f"manifest cell {cell['id']} has an invalid cluster dimension"
            )
        if cell["solver"] not in {
            "auto", "map", "cg", "it", "squarem", "lsmr", "cmg",
        }:
            raise ContractError(f"manifest cell {cell['id']} has an unknown solver")
        if cell["preconditioner"] not in {
            "auto", "off", "diagonal", "schwarz",
        } or (
            cell["preconditioner"] in {"diagonal", "schwarz"}
            and cell["solver"] != "lsmr"
        ):
            raise ContractError(
                f"manifest cell {cell['id']} has an invalid preconditioner"
            )
        if not isinstance(cell["primary"], bool):
            raise ContractError(f"manifest cell {cell['id']} has unknown VCE or primary status")
    if tuple(observed_ids) != EXPECTED_CELL_IDS:
        raise ContractError("manifest cell inventory or order is not frozen")

    sealing = _mapping(manifest["sealing"], "sealing")
    _expect_keys(
        sealing,
        {
            "state", "observed_local_evidence", "artifact_seal",
            "qualification_seal", "controller_source",
            "missing_required_evidence", "freeze_rule",
        },
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
    receipt_path = _bound_path(
        seal_root, local_record["receipt_path"], "observed local evidence"
    )
    if sha256_file(receipt_path) != local_record["receipt_sha256"]:
        raise ContractError("observed exact-SHA local receipt is missing or stale")
    local_receipt = _load_json(receipt_path, "observed exact-SHA local receipt")
    macos_artifact = next(
        item for item in artifacts if item["platform"] == "macos-universal"
    )
    _validate_local_full_receipt(
        local_receipt,
        source,
        _artifact_identity(macos_artifact),
        "observed exact-SHA local receipt",
    )
    if not isinstance(sealing["missing_required_evidence"], list) or not all(
        isinstance(item, str) and item for item in sealing["missing_required_evidence"]
    ):
        raise ContractError("missing evidence inventory is malformed")
    if not isinstance(sealing["freeze_rule"], str) or not sealing["freeze_rule"]:
        raise ContractError("manifest freeze rule is missing")
    if manifest["status"] == "candidate_unsealed":
        if (
            sealing["state"] != "not_sealed"
            or sealing["artifact_seal"] is not None
            or sealing["qualification_seal"] is not None
            or sealing["controller_source"] is not None
            or sealing["missing_required_evidence"] != UNSEALED_MISSING_EVIDENCE
        ):
            raise ContractError(
                "an unsealed baseline must retain the exact missing-evidence inventory"
            )
    elif manifest["status"] == "artifacts_sealed":
        controller_source = _mapping(
            sealing["controller_source"], "sealing.controller_source"
        )
        _validate_controller_source(
            controller_source, "sealing.controller_source"
        )
        if (
            sealing["state"] != "artifacts_sealed"
            or sealing["qualification_seal"] is not None
            or sealing["missing_required_evidence"]
            != ARTIFACT_SEALED_MISSING_EVIDENCE
        ):
            raise ContractError(
                "an artifact-sealed baseline must remain nonqualifying until SCC closure"
            )
        if any(artifact["status"] != "qualified" for artifact in artifacts):
            raise ContractError("an artifact-sealed baseline requires every platform artifact")
        _validate_artifact_seal(
            manifest, sealing["artifact_seal"], root, seal_root
        )
    else:
        controller_source = _mapping(
            sealing["controller_source"], "sealing.controller_source"
        )
        _validate_controller_source(
            controller_source, "sealing.controller_source"
        )
        if (
            sealing["state"] != "sealed"
            or sealing["missing_required_evidence"] != []
            or any(artifact["status"] != "qualified" for artifact in artifacts)
        ):
            raise ContractError(
                "a frozen baseline must be sealed with every artifact and no missing evidence"
            )
        _validate_artifact_seal(
            manifest, sealing["artifact_seal"], root, seal_root
        )
        _validate_qualification_seal(
            manifest, sealing["qualification_seal"], root, seal_root
        )


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
    root: Path,
    evidence_root: Path,
) -> tuple[
    str,
    tuple[str, ...],
    dict[str, Mapping[str, Any]],
    dict[str, Any] | None,
]:
    _expect_keys(
        evidence,
        {
            "schema_version", "evidence_type", "profile", "manifest", "baseline",
            "candidate", "behavior", "cells",
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
    if profile == "local_screen":
        if evidence["behavior"] is not None:
            raise ContractError("local screen may not carry governing behavior evidence")
        behavior = None
    elif manifest["status"] == "artifacts_sealed":
        if (
            evidence["candidate"] != evidence["baseline"]
            or evidence["behavior"] is not None
        ):
            raise ContractError(
                "artifact-sealed diagnostic evidence must be exact F0-versus-F0 "
                "without a synthetic candidate behavior comparison"
            )
        behavior = None
    else:
        if evidence["candidate"]["source"] == evidence["baseline"]["source"]:
            raise ContractError(
                "frozen governing evidence requires a distinct candidate source"
            )
        behavior = _validate_candidate_behavior_evidence(
            evidence["behavior"],
            root,
            evidence_root,
            identities["baseline"]["source"],
            identities["candidate"]["source"],
            manifest["baseline_candidate"]["artifacts"],
        )
    return profile, expected_ids, cells, behavior


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
    root: Path = ROOT,
    evidence_root: Path | None = None,
    seal_root: Path | None = None,
) -> dict[str, Any]:
    """Evaluate the closed evidence, applying science before performance."""
    validate_manifest(manifest, root=root, evidence_root=seal_root)
    expected_manifest_sha = manifest_sha256 or _manifest_sha256(manifest)
    _sha256(expected_manifest_sha, "manifest sha256")
    bound_evidence_root = (evidence_root or root).resolve()
    profile, expected_ids, cells, behavior = _validate_evidence_header(
        manifest, evidence, expected_manifest_sha, root, bound_evidence_root
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
            "behavior": behavior,
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
        if status == "pass":
            stage = "complete"
        else:
            stage = (
                "sealing"
                if blockers == ["baseline_candidate_unsealed"]
                else "performance_memory"
            )
        reasons = blockers
    return {
        "status": status,
        "stage": stage,
        "reasons": reasons,
        "profile": profile,
        "scientific_policy": manifest["scientific_rules"]["active_policy"],
        "behavior": behavior,
        "scientific_cells_passed": len(expected_ids),
        "performance_evaluated": True,
        "aggregate_primary_geometric_mean_ratio": aggregate,
        "aggregate_primary_cell_count": len(primary_mean_logs),
        "cells": metrics,
    }


def _validate_qualification_seal(
    manifest: Mapping[str, Any],
    value: Any,
    source_root: Path,
    evidence_root: Path,
) -> None:
    """Reopen the entire diagnostic run through the SCC schema authority."""

    payload, _ = _read_bound_json(
        value,
        evidence_root,
        "F0 qualification seal",
        sealed=True,
    )
    _expect_keys(
        payload,
        {
            "schema_version", "record_type", "baseline_candidate",
            "artifact_seal", "diagnostic_manifest", "diagnostic_run_root",
            "diagnostic_closure", "terminal_sentinel",
        },
        "F0 qualification seal payload",
    )
    if (
        payload["schema_version"] != 1
        or payload["record_type"] != QUALIFICATION_SEAL_TYPE
        or payload["baseline_candidate"] != manifest["baseline_candidate"]
        or payload["artifact_seal"] != manifest["sealing"]["artifact_seal"]
        or payload["terminal_sentinel"]
        != "FEREG_F0_QUALIFICATION_SEAL_COMPLETE"
    ):
        raise ContractError("F0 qualification seal identity is stale or substituted")

    diagnostic, diagnostic_record = _read_bound_json(
        payload["diagnostic_manifest"],
        evidence_root,
        "artifact-sealed diagnostic manifest",
    )
    diagnostic_path = _bound_path(
        evidence_root,
        payload["diagnostic_manifest"]["path"],
        "artifact-sealed diagnostic manifest",
    )
    try:
        relative_manifest = diagnostic_path.relative_to(
            evidence_root.resolve()
        ).as_posix()
    except ValueError as error:  # guarded by _bound_path, retained for clarity
        raise ContractError("diagnostic manifest escapes the evidence root") from error
    if not relative_manifest.startswith("benchmarks/manifests/"):
        raise ContractError("diagnostic manifest must live in benchmarks/manifests")
    expected_diagnostic = copy.deepcopy(dict(manifest))
    expected_diagnostic["status"] = "artifacts_sealed"
    expected_diagnostic["sealing"]["state"] = "artifacts_sealed"
    expected_diagnostic["sealing"]["qualification_seal"] = None
    expected_diagnostic["sealing"]["missing_required_evidence"] = list(
        ARTIFACT_SEALED_MISSING_EVIDENCE
    )
    if diagnostic != expected_diagnostic:
        raise ContractError(
            "diagnostic manifest differs from the frozen manifest outside the sealing transition"
        )
    validate_manifest(
        diagnostic, root=source_root, evidence_root=evidence_root
    )

    run_root = _bound_directory(
        evidence_root,
        payload["diagnostic_run_root"],
        "F0 diagnostic SCC run root",
    )
    verifier = _load_repository_module(
        source_root,
        "benchmarks/scc/fereg_f0_scc.py",
        "fereg_f0_scc",
    )
    try:
        verified = verifier.verify_diagnostic_run(
            run_root,
            diagnostic_path,
            expected_closure=_mapping(
                payload["diagnostic_closure"], "frozen diagnostic closure"
            ),
        )
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise ContractError(
            f"F0 diagnostic run fails authoritative verification: {error}"
        ) from error
    verified = _mapping(verified, "F0 verified diagnostic closure")
    if payload["diagnostic_closure"] != verified:
        raise ContractError(
            "F0 diagnostic closure differs from authoritative verification"
        )
    linux_artifact = next(
        _artifact_identity(item)
        for item in manifest["baseline_candidate"]["artifacts"]
        if item["platform"] == "linux-x86_64"
    )
    expected_arm = {
        "source": manifest["baseline_candidate"]["source"],
        "artifact": linux_artifact,
    }
    if (
        verified.get("schema_version") != 1
        or verified.get("status") != "complete_nonqualifying"
        or verified.get("manifest")
        != {
            "id": diagnostic["manifest_id"],
            "sha256": diagnostic_record["sha256"],
        }
        or verified.get("baseline") != expected_arm
        or verified.get("candidate") != expected_arm
        or verified.get("terminal_sentinel")
        != "FEREG_F0_DIAGNOSTIC_RUN_VERIFIED"
    ):
        raise ContractError(
            "F0 diagnostic run is not an exact nonqualifying F0-versus-F0 closure"
        )


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
    root: Path = ROOT,
    evidence_root: Path | None = None,
    seal_root: Path | None = None,
) -> dict[str, Any]:
    evaluation = evaluate_evidence(
        manifest,
        evidence,
        manifest_sha256=manifest_sha256,
        root=root,
        evidence_root=evidence_root,
        seal_root=seal_root,
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
    scc_run_root = (
        path.resolve().parents[1]
        if payload["evidence"].get("profile") == "scc_governing"
        and path.resolve().parent.name == "output"
        else None
    )
    source_root = manifest_path.resolve().parents[2]
    seal_root = (
        scc_run_root / "input/seal-evidence"
        if scc_run_root is not None
        else source_root
    )
    manifest = load_manifest(
        manifest_path, root=source_root, evidence_root=seal_root
    )
    manifest_hash = sha256_file(manifest_path)
    if payload["manifest"] != {"id": manifest["manifest_id"], "sha256": manifest_hash}:
        raise ContractError("receipt manifest binding is stale or substituted")
    if payload["evidence_sha256"] != hashlib.sha256(canonical_json_bytes(payload["evidence"])).hexdigest():
        raise ContractError("receipt evidence hash mismatch")
    expected_evaluation = evaluate_evidence(
        manifest,
        payload["evidence"],
        manifest_sha256=manifest_hash,
        root=source_root,
        evidence_root=(
            scc_run_root if scc_run_root is not None else source_root
        ),
        seal_root=seal_root,
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


def _output_path(root: Path, path: Path, where: str) -> Path:
    root = root.resolve()
    candidate = path if path.is_absolute() else root / path
    parent = candidate.parent.resolve()
    try:
        parent.relative_to(root)
    except ValueError as error:
        raise ContractError(f"{where} must live below the evidence root") from error
    current = root
    relative_parent = parent.relative_to(root)
    for component in relative_parent.parts:
        current = current / component
        if current.exists() and current.is_symlink():
            raise ContractError(f"{where} parent traverses a symlink")
    parent.mkdir(parents=True, exist_ok=True)
    resolved = parent / candidate.name
    if resolved.exists() or resolved.is_symlink():
        raise ContractError(f"refusing to overwrite immutable {where}: {resolved}")
    return resolved


def _write_immutable_json(path: Path, value: Any, where: str) -> None:
    encoded = canonical_json_bytes(value)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise ContractError(f"refusing to overwrite immutable {where}: {path}") from error
    try:
        with os.fdopen(descriptor, "wb") as target:
            target.write(encoded)
            target.flush()
            os.fsync(target.fileno())
    except OSError as error:
        raise ContractError(f"unable to write immutable {where}: {error}") from error


def _controller_source_identity(repository: Path, commit: str) -> dict[str, str]:
    repository = repository.resolve()
    _sha1(commit, "controller commit")
    observed_commit = _git(
        repository, ["rev-parse", "--verify", f"{commit}^{{commit}}"]
    ).decode("ascii").strip()
    head = _git(repository, ["rev-parse", "--verify", "HEAD^{commit}"]).decode(
        "ascii"
    ).strip()
    tree = _git(repository, ["show", "-s", "--format=%T", commit]).decode(
        "ascii"
    ).strip()
    if observed_commit != commit or head != commit:
        raise ContractError(
            "controller commit must be the explicit full commit checked out at HEAD"
        )
    _sha1(tree, "controller tree")
    return {"commit": commit, "tree": tree}


def _spec_path(root: Path, value: Any, where: str) -> Path:
    return _bound_path(root, value, where)


def _build_portable_platform_evidence(
    spec_value: Any,
    root: Path,
    platform: str,
) -> tuple[dict[str, Any], Mapping[str, Any]]:
    spec = _mapping(spec_value, f"{platform} platform input spec")
    common = {
        "artifact", "capture", "receipt", "record", "source_archive",
        "capture_program",
    }
    specific = (
        {"native_attestation", "native_producer"}
        if platform == "macos-universal"
        else {
            "windows_controller_receipt", "windows_controller_output",
            "windows_controller_archive", "windows_results_archive",
            "windows_source_manifest",
        }
    )
    _expect_keys(spec, common | specific, f"{platform} platform input spec")
    paths = {
        field: _spec_path(root, spec[field], f"{platform} {field}")
        for field in common | specific
    }
    verifier = _load_repository_module(
        root,
        "scripts/feppml_platform_load_receipt.py",
        "feppml_platform_load_receipt",
    )
    arguments: dict[str, Any] = {
        "receipt": paths["receipt"],
        "record": paths["record"],
        "artifact": paths["artifact"],
        "capture_path": paths["capture"],
        "source_archive": paths["source_archive"],
        "capture_program": paths["capture_program"],
    }
    if platform == "macos-universal":
        arguments["native_attestation"] = paths["native_attestation"]
        arguments["native_producer"] = paths["native_producer"]
    else:
        arguments.update(
            {
                "windows_controller_receipt": paths[
                    "windows_controller_receipt"
                ],
                "windows_controller_output": paths["windows_controller_output"],
                "windows_controller_archive": paths[
                    "windows_controller_archive"
                ],
                "windows_results_archive": paths["windows_results_archive"],
                "windows_source_manifest": paths["windows_source_manifest"],
            }
        )
    try:
        verified = verifier.verify_portable(**arguments)
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise ContractError(
            f"{platform} evidence fails the authoritative verifier: {error}"
        ) from error
    evidence = {
        field: file_binding(paths[field], root)
        for field in sorted(common | specific)
    }
    evidence["verified_receipt"] = dict(verified)
    return evidence, _mapping(verified, f"{platform} verified receipt")


def seal_artifacts(
    *,
    manifest_path: Path,
    evidence_spec_path: Path,
    controller_repository: Path,
    controller_commit: str,
    artifact_seal_output: Path,
    manifest_output: Path,
    root: Path = ROOT,
) -> dict[str, Any]:
    """Create the candidate_unsealed -> artifacts_sealed transition."""

    root = root.resolve()
    manifest = load_manifest(manifest_path, root=root)
    if manifest["status"] != "candidate_unsealed":
        raise ContractError("artifact sealing requires candidate_unsealed input")
    spec = _load_json(evidence_spec_path, "F0 artifact evidence spec")
    _expect_keys(
        spec,
        {
            "local_full_receipt", "direct_behavior_observation",
            "direct_behavior_bundle", "macos", "linux_run_root", "windows",
        },
        "F0 artifact evidence spec",
    )
    controller_source = _controller_source_identity(
        controller_repository, controller_commit
    )
    macos_evidence, macos = _build_portable_platform_evidence(
        spec["macos"], root, "macos-universal"
    )
    windows_evidence, windows = _build_portable_platform_evidence(
        spec["windows"], root, "windows-x86_64"
    )
    linux_run_root = _bound_directory(
        root, spec["linux_run_root"], "Linux platform run root"
    )
    linux_verifier = _load_repository_module(
        root,
        "benchmarks/scc/feppml_linux_platform_scc.py",
        "feppml_linux_platform_scc",
    )
    source = manifest["baseline_candidate"]["source"]
    try:
        linux = linux_verifier.verify_closed_run(
            linux_run_root,
            expected_source_commit=source["commit"],
            expected_harness_commit=controller_source["commit"],
        )
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise ContractError(
            f"Linux evidence fails the authoritative closed-run verifier: {error}"
        ) from error
    linux = _mapping(linux, "Linux verified close")
    target_source_archive = {
        "sha256": macos.get("source_archive_sha256"),
        "size_bytes": macos.get("source_archive_size_bytes"),
    }
    _validate_source_archive(target_source_archive, "F0 target source archive")
    for platform_name, observed in (
        ("macos-universal", macos),
        ("windows-x86_64", windows),
    ):
        if (
            observed.get("qualification_profile") != "f0-baseline"
            or not _platform_controller_matches(
                platform_name, observed, controller_source
            )
            or observed.get("source_commit") != source["commit"]
            or observed.get("source_tree") != source["tree"]
            or observed.get("source_archive_sha256")
            != target_source_archive["sha256"]
            or observed.get("source_archive_size_bytes")
            != target_source_archive["size_bytes"]
        ):
            raise ContractError(
                f"{platform_name} source/controller differs from the allowlisted F0 seal"
            )
    linux_source = _mapping(linux.get("source"), "Linux target source")
    linux_controller = _mapping(linux.get("harness_source"), "Linux controller source")
    if (
        linux.get("qualification_profile") != "f0-baseline"
        or linux_source.get("commit") != source["commit"]
        or linux_source.get("tree") != source["tree"]
        or linux_source.get("archive_sha256") != target_source_archive["sha256"]
        or linux_source.get("archive_size_bytes")
        != target_source_archive["size_bytes"]
        or linux_controller.get("commit") != controller_source["commit"]
        or linux_controller.get("tree") != controller_source["tree"]
    ):
        raise ContractError(
            "Linux target/controller differs from the allowlisted F0 seal"
        )

    artifact_identities = {
        "macos-universal": {
            "platform": "macos-universal",
            "sha256": macos.get("artifact_sha256"),
            "size_bytes": macos.get("artifact_size_bytes"),
            "embedded_git_commit": macos.get("embedded_git_commit"),
        },
        "linux-x86_64": {
            "platform": "linux-x86_64",
            "sha256": linux.get("artifact", {}).get("sha256"),
            "size_bytes": linux.get("artifact", {}).get("size_bytes"),
            "embedded_git_commit": source["commit"][:12],
        },
        "windows-x86_64": {
            "platform": "windows-x86_64",
            "sha256": windows.get("artifact_sha256"),
            "size_bytes": windows.get("artifact_size_bytes"),
            "embedded_git_commit": windows.get("embedded_git_commit"),
        },
    }
    for platform_name, artifact in artifact_identities.items():
        _validate_artifact(artifact, f"sealed {platform_name} artifact")

    transitioned = copy.deepcopy(manifest)
    for artifact in transitioned["baseline_candidate"]["artifacts"]:
        artifact.update(artifact_identities[artifact["platform"]])
        artifact["status"] = "qualified"
    transitioned["status"] = "artifacts_sealed"
    transitioned["sealing"]["state"] = "artifacts_sealed"
    transitioned["sealing"]["controller_source"] = controller_source
    transitioned["sealing"]["qualification_seal"] = None
    transitioned["sealing"]["missing_required_evidence"] = list(
        ARTIFACT_SEALED_MISSING_EVIDENCE
    )
    local_path = _spec_path(
        root, spec["local_full_receipt"], "exact-SHA local F0 receipt"
    )
    behavior_path = _spec_path(
        root, spec["direct_behavior_observation"], "direct F0 behavior receipt"
    )
    behavior_bundle = _spec_path(
        root, spec["direct_behavior_bundle"], "direct F0 behavior bundle"
    )
    behavior_module, _, _ = _load_behavior_contract(root)
    behavior_manifest = root / "benchmarks/manifests/fereg_f0_behavior_v1.json"
    try:
        verified_behavior = behavior_module.verify_observation(
            behavior_path,
            behavior_bundle,
            behavior_manifest,
            expected_role="baseline",
            expected_phase="baseline",
        )
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise ContractError(
            f"direct F0 behavior evidence fails authoritative verification: {error}"
        ) from error
    if (
        verified_behavior.get("status") != "pass"
        or verified_behavior.get("source") != source
        or verified_behavior.get("artifact")
        != artifact_identities["macos-universal"]
    ):
        raise ContractError(
            "direct F0 behavior evidence substitutes source or macOS artifact bytes"
        )
    linux_relative = linux_run_root.relative_to(root).as_posix()
    payload = {
        "schema_version": 1,
        "record_type": ARTIFACT_SEAL_TYPE,
        "baseline_candidate": copy.deepcopy(transitioned["baseline_candidate"]),
        "controller_source": controller_source,
        "target_source_archive": target_source_archive,
        "evidence": {
            "local_full_receipt": file_binding(local_path, root),
            "direct_behavior": {
                "observation": file_binding(behavior_path, root, sealed=True),
                "bundle": file_binding(behavior_bundle, root),
                "verified_observation": dict(verified_behavior),
            },
            "macos": macos_evidence,
            "linux": {
                "run_root": linux_relative,
                "verified_close": dict(linux),
            },
            "windows": windows_evidence,
        },
        "terminal_sentinel": "FEREG_F0_ARTIFACT_SEAL_COMPLETE",
    }
    artifact_seal_path = _output_path(
        root, artifact_seal_output, "F0 artifact seal"
    )
    _write_immutable_json(
        artifact_seal_path, seal_payload(payload), "F0 artifact seal"
    )
    transitioned["sealing"]["artifact_seal"] = file_binding(
        artifact_seal_path, root, sealed=True
    )
    validate_manifest(transitioned, root=root)
    manifest_target = _output_path(
        root, manifest_output, "artifact-sealed F0 manifest"
    )
    _write_immutable_json(
        manifest_target, transitioned, "artifact-sealed F0 manifest"
    )
    return {
        "status": "artifacts_sealed",
        "artifact_seal": file_binding(artifact_seal_path, root, sealed=True),
        "manifest": file_binding(manifest_target, root),
        "controller_source": controller_source,
        "target_source_archive": target_source_archive,
    }


def freeze_baseline(
    *,
    manifest_path: Path,
    diagnostic_run_root: Path,
    qualification_seal_output: Path,
    manifest_output: Path,
    root: Path = ROOT,
) -> dict[str, Any]:
    """Create artifacts_sealed -> frozen only from a verified SCC diagnostic run."""

    root = root.resolve()
    manifest = load_manifest(manifest_path, root=root)
    if manifest["status"] != "artifacts_sealed":
        raise ContractError("baseline freezing requires artifacts_sealed input")
    diagnostic_run = diagnostic_run_root.resolve()
    try:
        diagnostic_relative = diagnostic_run.relative_to(root).as_posix()
    except ValueError as error:
        raise ContractError("diagnostic SCC run must live below the evidence root") from error
    _bound_directory(root, diagnostic_relative, "F0 diagnostic SCC run root")
    verifier = _load_repository_module(
        root,
        "benchmarks/scc/fereg_f0_scc.py",
        "fereg_f0_scc",
    )
    try:
        diagnostic_closure = verifier.verify_diagnostic_run(
            diagnostic_run, manifest_path.resolve()
        )
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise ContractError(
            f"F0 diagnostic run fails authoritative verification: {error}"
        ) from error
    linux_artifact = next(
        _artifact_identity(item)
        for item in manifest["baseline_candidate"]["artifacts"]
        if item["platform"] == "linux-x86_64"
    )
    expected_arm = {
        "source": manifest["baseline_candidate"]["source"],
        "artifact": linux_artifact,
    }
    if (
        diagnostic_closure.get("status") != "complete_nonqualifying"
        or diagnostic_closure.get("baseline") != expected_arm
        or diagnostic_closure.get("candidate") != expected_arm
    ):
        raise ContractError(
            "diagnostic run must be an exact nonqualifying F0-versus-F0 closure"
        )
    payload = {
        "schema_version": 1,
        "record_type": QUALIFICATION_SEAL_TYPE,
        "baseline_candidate": copy.deepcopy(manifest["baseline_candidate"]),
        "artifact_seal": copy.deepcopy(manifest["sealing"]["artifact_seal"]),
        "diagnostic_manifest": file_binding(manifest_path.resolve(), root),
        "diagnostic_run_root": diagnostic_relative,
        "diagnostic_closure": dict(diagnostic_closure),
        "terminal_sentinel": "FEREG_F0_QUALIFICATION_SEAL_COMPLETE",
    }
    qualification_path = _output_path(
        root, qualification_seal_output, "F0 qualification seal"
    )
    _write_immutable_json(
        qualification_path, seal_payload(payload), "F0 qualification seal"
    )
    frozen = copy.deepcopy(manifest)
    frozen["status"] = "frozen"
    frozen["sealing"]["state"] = "sealed"
    frozen["sealing"]["qualification_seal"] = file_binding(
        qualification_path, root, sealed=True
    )
    frozen["sealing"]["missing_required_evidence"] = []
    validate_manifest(frozen, root=root)
    manifest_target = _output_path(root, manifest_output, "frozen F0 manifest")
    _write_immutable_json(manifest_target, frozen, "frozen F0 manifest")
    return {
        "status": "frozen",
        "qualification_seal": file_binding(
            qualification_path, root, sealed=True
        ),
        "manifest": file_binding(manifest_target, root),
        "diagnostic_closure": dict(diagnostic_closure),
    }


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
    validate.add_argument(
        "--evidence-root",
        type=Path,
        help="root containing the byte-bound F0 seal evidence closure",
    )

    identity = subparsers.add_parser("source-identity")
    identity.add_argument("--repository", type=Path, default=ROOT)
    identity.add_argument("--revision", default="HEAD")
    identity.add_argument("--allow-tracked-dirty", action="store_true")

    evaluate = subparsers.add_parser("evaluate")
    evaluate.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    evaluate.add_argument("--evidence", type=Path, required=True)
    evaluate.add_argument("--output", type=Path, required=True)
    evaluate.add_argument("--identity-output", type=Path)
    evaluate.add_argument(
        "--evidence-root",
        type=Path,
        help="root for byte-bound external behavior receipts and bundles",
    )
    evaluate.add_argument(
        "--seal-root",
        type=Path,
        help="root containing copied artifact/qualification seal evidence",
    )
    evaluate.add_argument(
        "--exact-archive-root",
        type=Path,
        help=(
            "root of a source tree already authenticated by the SCC preparation "
            "receipt; skips only the unavailable .git object lookup"
        ),
    )

    verify = subparsers.add_parser("verify-receipt")
    verify.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    verify.add_argument("--receipt", type=Path, required=True)
    verify.add_argument("--identity", type=Path)

    artifacts = subparsers.add_parser(
        "seal-artifacts",
        help="create an immutable candidate_unsealed -> artifacts_sealed transition",
    )
    artifacts.add_argument("--manifest", type=Path, required=True)
    artifacts.add_argument("--evidence-spec", type=Path, required=True)
    artifacts.add_argument("--controller-repository", type=Path, required=True)
    artifacts.add_argument("--controller-commit", required=True)
    artifacts.add_argument("--artifact-seal-output", type=Path, required=True)
    artifacts.add_argument("--manifest-output", type=Path, required=True)

    freeze = subparsers.add_parser(
        "freeze-baseline",
        help="create artifacts_sealed -> frozen from a verified SCC diagnostic run",
    )
    freeze.add_argument("--manifest", type=Path, required=True)
    freeze.add_argument("--diagnostic-run-root", type=Path, required=True)
    freeze.add_argument("--qualification-seal-output", type=Path, required=True)
    freeze.add_argument("--manifest-output", type=Path, required=True)

    diagnostic = subparsers.add_parser(
        "verify-diagnostic-run",
        help="read-only verification of an artifacts_sealed SCC closure",
    )
    diagnostic.add_argument("--manifest", type=Path, required=True)
    diagnostic.add_argument("--run-root", type=Path, required=True)

    closed = subparsers.add_parser(
        "verify-closed-run",
        help="read-only verification of a final exact frozen-manifest SCC run",
    )
    closed.add_argument("--manifest", type=Path, required=True)
    closed.add_argument("--run-root", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    if not (sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode):
        raise SystemExit("fereg F0 regression CLI requires Python -I -B -S")
    forbidden = ("PYTHONPATH", "PYTHONHOME", "PYTHONSTARTUP", "LD_PRELOAD", "DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH")
    present = [name for name in forbidden if os.environ.get(name)]
    if present:
        raise SystemExit("fereg F0 regression CLI rejects ambient loader/startup variables: " + ",".join(present))
    args = _parser().parse_args(argv)
    try:
        if args.command == "validate-manifest":
            manifest = load_manifest(
                args.manifest,
                evidence_root=(
                    args.evidence_root.resolve()
                    if args.evidence_root is not None
                    else None
                ),
            )
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
            evidence_root = (
                args.exact_archive_root.resolve()
                if args.exact_archive_root is not None
                else ROOT
            )
            manifest = load_manifest(
                args.manifest,
                verify_git_object=args.exact_archive_root is None,
                root=evidence_root,
            )
            evidence = _load_json(args.evidence, "F0 evidence")
            manifest_hash = sha256_file(args.manifest)
            document = build_receipt(
                manifest,
                evidence,
                manifest_sha256=manifest_hash,
                root=evidence_root,
                evidence_root=(
                    args.evidence_root.resolve()
                    if args.evidence_root is not None
                    else evidence_root
                ),
                seal_root=(
                    args.seal_root.resolve()
                    if args.seal_root is not None
                    else evidence_root
                ),
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
        if args.command == "seal-artifacts":
            result = seal_artifacts(
                manifest_path=args.manifest,
                evidence_spec_path=args.evidence_spec,
                controller_repository=args.controller_repository,
                controller_commit=args.controller_commit,
                artifact_seal_output=args.artifact_seal_output,
                manifest_output=args.manifest_output,
            )
            print(json.dumps(result, sort_keys=True))
            return 0
        if args.command == "freeze-baseline":
            result = freeze_baseline(
                manifest_path=args.manifest,
                diagnostic_run_root=args.diagnostic_run_root,
                qualification_seal_output=args.qualification_seal_output,
                manifest_output=args.manifest_output,
            )
            print(json.dumps(result, sort_keys=True))
            return 0
        if args.command in {"verify-diagnostic-run", "verify-closed-run"}:
            verifier = _load_repository_module(
                ROOT,
                "benchmarks/scc/fereg_f0_scc.py",
                "fereg_f0_scc",
            )
            operation = (
                verifier.verify_diagnostic_run
                if args.command == "verify-diagnostic-run"
                else verifier.verify_closed_run
            )
            print(
                json.dumps(
                    operation(args.run_root, args.manifest), sort_keys=True
                )
            )
            return 0
    except ContractError as error:
        print(f"fereg F0 regression contract error: {error}", file=sys.stderr)
        return 2
    raise AssertionError("unreachable command")


if __name__ == "__main__":
    raise SystemExit(main())
