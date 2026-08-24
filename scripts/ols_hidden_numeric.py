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
DEFAULT_CANDIDATE_RECEIPT_IDENTITY = (
    ROOT / "benchmarks/manifests/ols_hidden_numeric_candidate_receipt_v1.json"
)
RECEIPT_TYPE = "stfixest-ols-hidden-numeric-v1-candidate"
TERMINAL_SENTINEL = "END_FIXTURE|ols_v1_reference_capture|rc=0"
FORBIDDEN_WARNING = "warning: variance matrix is nonsymmetric or highly singular."
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
SAFE_ID = re.compile(r"^[a-z][a-z0-9_]{1,63}$")
GIT_OBJECT_ID = re.compile(r"^[0-9a-f]{40}(?:[0-9a-f]{24})?$")
FORBIDDEN_WARNING_BYTES = FORBIDDEN_WARNING.encode("utf-8")

EXECUTABLE_CASE_IDS = (
    "unweighted_no_fe",
    "one_fe_robust",
    "multi_fe_interactions",
    "intercept_and_slope",
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


def _git_output(repository: Path, *arguments: str) -> str:
    completed = subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise ContractError(f"git {' '.join(arguments)} failed: {detail}")
    return completed.stdout.strip()


def _candidate_source_identity(repository: Path = ROOT) -> dict[str, Any]:
    if subprocess.run(
        ["git", "-C", str(repository), "diff", "--quiet", "--no-ext-diff"],
        check=False,
    ).returncode != 0 or subprocess.run(
        ["git", "-C", str(repository), "diff", "--cached", "--quiet", "--no-ext-diff"],
        check=False,
    ).returncode != 0:
        raise ContractError(
            "candidate capture requires a clean tracked worktree and index"
        )
    object_format = _git_output(repository, "rev-parse", "--show-object-format")
    if object_format not in {"sha1", "sha256"}:
        raise ContractError("candidate repository has an unsupported Git object format")
    commit = _git_output(repository, "rev-parse", "--verify", "HEAD^{commit}")
    tree = _git_output(repository, "rev-parse", "--verify", "HEAD^{tree}")
    identity = {
        "object_format": object_format,
        "commit": commit,
        "tree": tree,
        "tracked_worktree_clean": True,
        "scope": "complete-committed-repository-tree-v1",
    }
    _verify_candidate_source_identity(identity, repository)
    return identity


def _verify_candidate_source_identity(
    identity: Mapping[str, Any], repository: Path = ROOT
) -> None:
    if not isinstance(identity, Mapping):
        raise ContractError("candidate source identity is malformed")
    _require_keys(
        identity,
        {"object_format", "commit", "tree", "tracked_worktree_clean", "scope"},
        "candidate source identity",
    )
    if (
        identity["object_format"] not in {"sha1", "sha256"}
        or identity["tracked_worktree_clean"] is not True
        or identity["scope"] != "complete-committed-repository-tree-v1"
        or not isinstance(identity["commit"], str)
        or not GIT_OBJECT_ID.fullmatch(identity["commit"])
        or not isinstance(identity["tree"], str)
        or not GIT_OBJECT_ID.fullmatch(identity["tree"])
    ):
        raise ContractError("candidate source identity is malformed")
    if _git_output(repository, "rev-parse", "--show-object-format") != identity["object_format"]:
        raise ContractError("candidate source Git object format is stale")
    try:
        observed_commit = _git_output(
            repository, "rev-parse", "--verify", f"{identity['commit']}^{{commit}}"
        )
        observed_tree = _git_output(
            repository, "rev-parse", "--verify", f"{identity['commit']}^{{tree}}"
        )
    except ContractError as exc:
        raise ContractError("candidate source commit is unavailable or stale") from exc
    if observed_commit != identity["commit"] or observed_tree != identity["tree"]:
        raise ContractError("candidate source commit/tree identity is stale")
    if subprocess.run(
        [
            "git",
            "-C",
            str(repository),
            "merge-base",
            "--is-ancestor",
            identity["commit"],
            "HEAD",
        ],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ).returncode != 0:
        raise ContractError("candidate source commit is not an ancestor of HEAD")


def _assert_committed_git_blob(path: Path) -> dict[str, Any]:
    resolved = path.resolve()
    try:
        repository = Path(_git_output(resolved.parent, "rev-parse", "--show-toplevel"))
        relative = resolved.relative_to(repository.resolve()).as_posix()
    except (ContractError, ValueError) as exc:
        raise ContractError("candidate receipt is not inside a Git worktree") from exc
    try:
        head_blob = _git_output(repository, "rev-parse", "--verify", f"HEAD:{relative}")
    except ContractError as exc:
        raise ContractError("candidate receipt is not committed in HEAD") from exc
    observed_blob = _git_output(repository, "hash-object", "--", relative)
    if observed_blob != head_blob:
        raise ContractError("candidate receipt bytes differ from the committed Git blob")
    tree_entry = _git_output(repository, "ls-tree", "HEAD", "--", relative).split()
    if len(tree_entry) < 3 or tree_entry[0] != "100644" or tree_entry[1] != "blob":
        raise ContractError("candidate receipt has an unsupported committed Git mode")
    return {
        "repository": repository.resolve(),
        "path": relative,
        "object_format": _git_output(repository, "rev-parse", "--show-object-format"),
        "blob": head_blob,
        "mode": tree_entry[0],
    }


def load_candidate_receipt_identity(
    path: Path = DEFAULT_CANDIDATE_RECEIPT_IDENTITY,
) -> dict[str, Any]:
    committed = _assert_committed_git_blob(path)
    try:
        identity = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ContractError(f"cannot read candidate receipt identity: {exc}") from exc
    if not isinstance(identity, Mapping):
        raise ContractError("candidate receipt identity must be an object")
    _require_keys(
        identity,
        {
            "schema_version",
            "identity",
            "receipt_path",
            "git_object_format",
            "git_blob",
            "git_mode",
            "file_sha256",
            "payload_sha256",
        },
        "candidate receipt identity",
    )
    if (
        identity["schema_version"] != 1
        or identity["identity"] != "stfixest-ols-hidden-numeric-v1-canonical-receipt"
        or not isinstance(identity["receipt_path"], str)
        or Path(identity["receipt_path"]).is_absolute()
        or ".." in Path(identity["receipt_path"]).parts
        or identity["git_object_format"] != committed["object_format"]
        or not isinstance(identity["git_blob"], str)
        or not GIT_OBJECT_ID.fullmatch(identity["git_blob"])
        or identity["git_mode"] != "100644"
        or not isinstance(identity["file_sha256"], str)
        or not HEX_SHA256.fullmatch(identity["file_sha256"])
        or not isinstance(identity["payload_sha256"], str)
        or not HEX_SHA256.fullmatch(identity["payload_sha256"])
    ):
        raise ContractError("candidate receipt identity is malformed")
    return dict(identity)


def _assert_canonical_candidate_receipt(
    path: Path,
    identity_path: Path = DEFAULT_CANDIDATE_RECEIPT_IDENTITY,
) -> dict[str, Any]:
    identity_commit = _assert_committed_git_blob(identity_path)
    identity = load_candidate_receipt_identity(identity_path)
    repository = identity_commit["repository"]
    expected_path = (repository / identity["receipt_path"]).resolve()
    if path.resolve() != expected_path:
        raise ContractError("candidate receipt path is not the canonical sealed path")
    committed = _assert_committed_git_blob(path)
    if committed["repository"] != repository:
        raise ContractError("candidate receipt and identity are in different repositories")
    if (
        committed["object_format"] != identity["git_object_format"]
        or committed["blob"] != identity["git_blob"]
        or committed["mode"] != identity["git_mode"]
        or sha256_file(path) != identity["file_sha256"]
    ):
        raise ContractError("candidate receipt differs from its external canonical identity")
    return identity


def _write_exclusive_receipt(path: Path, encoded: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open("x", encoding="utf-8", newline="\n") as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
    except FileExistsError as exc:
        raise ContractError(f"refusing to overwrite immutable receipt: {path}") from exc


def _verify_reference_receipt_portable(
    path: Path, manifest: Path
) -> dict[str, Any]:
    """Verify frozen comparator bytes without relying on checkout mode bits."""
    with tempfile.TemporaryDirectory(prefix="stfixest-reference-receipt-") as raw:
        copy_path = Path(raw) / "receipt.json"
        shutil.copyfile(path, copy_path)
        copy_path.chmod(0o444)
        return verify_reference_receipt(
            copy_path,
            manifest,
            ROOT / "references.lock.toml",
            ROOT,
        )


def _expected_plugin_filename() -> str:
    system = platform.system()
    if system == "Darwin":
        return "stfixest_macos.plugin"
    if system == "Linux":
        return "stfixest_linux.plugin"
    if system == "Windows":
        return "stfixest_windows.plugin"
    raise ContractError(f"unsupported candidate-capture platform {system!r}")


def _supplied_plugin_identity(
    plugin: Path, expected_filename: str | None = None
) -> tuple[Path, dict[str, Any]]:
    supplied = plugin.resolve()
    expected = expected_filename or _expected_plugin_filename()
    if not supplied.is_file() or supplied.name != expected:
        raise ContractError(f"candidate plugin must be an existing {expected} file")
    size = supplied.stat().st_size
    if size <= 0:
        raise ContractError("candidate plugin is empty")
    return supplied, {
        "filename": expected,
        "sha256": sha256_file(supplied),
        "size_bytes": size,
        "loader_policy": "isolated-single-candidate-copy-v1",
    }


def _validate_plugin_identity(plugin: Any) -> None:
    if (
        not isinstance(plugin, Mapping)
        or set(plugin)
        != {
            "filename",
            "sha256",
            "size_bytes",
            "loader_policy",
            "embedded_git_commit",
        }
        or plugin["filename"] not in {
            "stfixest_macos.plugin",
            "stfixest_linux.plugin",
            "stfixest_windows.plugin",
        }
        or not isinstance(plugin["sha256"], str)
        or not HEX_SHA256.fullmatch(plugin["sha256"])
        or not isinstance(plugin["size_bytes"], int)
        or plugin["size_bytes"] <= 0
        or plugin["loader_policy"] != "isolated-single-candidate-copy-v1"
        or not isinstance(plugin["embedded_git_commit"], str)
        or not re.fullmatch(r"[0-9a-f]{12}", plugin["embedded_git_commit"])
    ):
        raise ContractError("candidate receipt plugin identity is malformed")


def _warning_audit(channels: Sequence[bytes]) -> dict[str, Any]:
    digest = hashlib.sha256()
    total_bytes = 0
    matches = 0
    for index, channel in enumerate(channels):
        digest.update(index.to_bytes(4, "big"))
        digest.update(len(channel).to_bytes(8, "big"))
        digest.update(channel)
        total_bytes += len(channel)
        matches += channel.lower().count(FORBIDDEN_WARNING_BYTES)
    return {
        "audit": "framed-complete-stata-stdout-and-logs-v1",
        "channel_count": len(channels),
        "byte_count": total_bytes,
        "transcript_sha256": digest.hexdigest(),
        "forbidden_pattern_sha256": hashlib.sha256(FORBIDDEN_WARNING_BYTES).hexdigest(),
        "forbidden_match_count": matches,
    }


def _validate_warning_audit(warning_audit: Any) -> None:
    if (
        not isinstance(warning_audit, Mapping)
        or set(warning_audit)
        != {
            "audit",
            "channel_count",
            "byte_count",
            "transcript_sha256",
            "forbidden_pattern_sha256",
            "forbidden_match_count",
        }
        or warning_audit["audit"] != "framed-complete-stata-stdout-and-logs-v1"
        or not isinstance(warning_audit["channel_count"], int)
        or warning_audit["channel_count"] < 1
        or not isinstance(warning_audit["byte_count"], int)
        or warning_audit["byte_count"] <= 0
        or not isinstance(warning_audit["transcript_sha256"], str)
        or not HEX_SHA256.fullmatch(warning_audit["transcript_sha256"])
        or warning_audit["forbidden_pattern_sha256"]
        != hashlib.sha256(FORBIDDEN_WARNING_BYTES).hexdigest()
        or warning_audit["forbidden_match_count"] != 0
    ):
        raise ContractError("candidate receipt warning audit is malformed or failed")


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
    for row in (1, 2, 3, 24, 32):
        if (
            stata_hex_to_float(certificate[(row, 1)]["value_hex"]) != 1.0
            or stata_hex_to_float(certificate[(row, 2)]["value_hex"]) != 1.0
        ):
            raise ContractError(f"case {case['id']}: mandatory certificate row {row} failed")
    if (
        stata_hex_to_float(certificate[(31, 1)]["value_hex"]) != 1.0
        or not math.isfinite(stata_hex_to_float(certificate[(31, 2)]["value_hex"]))
    ):
        raise ContractError(
            f"case {case['id']}: native covariance determinant metadata is invalid"
        )
    posted_determinant = case["scalars"].get("stfixest_v_determinant")
    if (
        posted_determinant is not None
        and posted_determinant != certificate[(31, 2)]["value_hex"]
    ):
        raise ContractError(
            f"case {case['id']}: posted covariance determinant is not native-bound"
        )
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
        determinant = certificate[(31, 2)]["value_hex"]
        if determinant != anchor["determinant"]:
            raise ContractError(f"case {case['id']}: exact native determinant anchor changed")


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


def _run_stata_capture(
    stata: Path,
    plugin: Path,
    projection_path: Path,
    source_identity: Mapping[str, Any],
) -> tuple[
    dict[str, str],
    list[dict[str, Any]],
    str,
    dict[str, Any],
    dict[str, Any],
]:
    projection = load_projection(projection_path)
    projected = projected_manifest(projection)
    projection_hash = sha256_file(projection_path)
    expected_plugin_name = _expected_plugin_filename()
    staged_plugin, plugin_identity = _supplied_plugin_identity(
        plugin, expected_plugin_name
    )
    plugin_sha256 = plugin_identity["sha256"]
    plugin_size = plugin_identity["size_bytes"]
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
                f"adopath ++ {_stata_quote(staged_plugin.parent)}",
                f"capture program __stfixest_candidate_provenance, plugin using({_stata_quote(staged_plugin)})",
                "local provenance_load_rc = _rc",
                "if `provenance_load_rc' != 0 & `provenance_load_rc' != 110 exit `provenance_load_rc'",
                "plugin call __stfixest_candidate_provenance, version",
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
        log_paths = sorted(run_dir.glob("*.log"), key=lambda path: path.name)
        log_bytes = [path.read_bytes() for path in log_paths]
        stdout_bytes = completed.stdout.encode("utf-8", errors="replace")
        warning_audit = _warning_audit([stdout_bytes, *log_bytes])
        logs = "\n".join(
            value.decode("utf-8", errors="replace") for value in log_bytes
        )
        if completed.returncode != 0:
            raise ContractError(f"licensed Stata process returned {completed.returncode}")
        if warning_audit["forbidden_match_count"] != 0:
            raise ContractError("licensed Stata emitted the forbidden covariance warning")
        embedded_commits = re.findall(
            r"^stfixest\.git_commit=([0-9a-f]{12})\s*$", logs, flags=re.MULTILINE
        )
        expected_embedded_commit = str(source_identity["commit"])[:12]
        if embedded_commits != [expected_embedded_commit]:
            raise ContractError(
                "loaded candidate plugin does not embed the captured source commit"
            )
        if (
            sha256_file(staged_plugin) != plugin_sha256
            or staged_plugin.stat().st_size != plugin_size
        ):
            raise ContractError("candidate plugin bytes changed during licensed Stata capture")
        plugin_identity["embedded_git_commit"] = expected_embedded_commit
        if not capture_file.is_file():
            raise ContractError("licensed Stata did not create a candidate capture")
        capture_text = capture_file.read_text(encoding="utf-8", errors="strict")
        try:
            parsed = parse_capture(capture_text, projected, projection_hash)
        except ContractError as exc:
            log_tail = "\n".join(logs.splitlines()[-300:])
            raise ContractError(f"{exc}\nlicensed Stata log tail:\n{log_tail}") from exc
        return (
            parsed.environment,
            parsed.cases,
            hashlib.sha256(capture_text.encode("utf-8")).hexdigest(),
            plugin_identity,
            warning_audit,
        )


def _cargo_binary(explicit: Path | None) -> Path:
    if explicit is not None:
        candidate = explicit
    else:
        discovered = shutil.which("cargo")
        candidate = Path(discovered) if discovered else Path.home() / ".rustup/toolchains/1.85.1-aarch64-apple-darwin/bin/cargo"
    if not candidate.is_file():
        raise ContractError("cargo is required to revalidate the fake-SPI failure sweep")
    return candidate.resolve()


def _supporting_gate_evidence(plugin: Mapping[str, Any]) -> dict[str, Any]:
    binding = {
        "sha256": plugin["sha256"],
        "size_bytes": plugin["size_bytes"],
        "embedded_git_commit": plugin["embedded_git_commit"],
        "loader_policy": "one-shared-isolated-staged-candidate-v1",
    }
    return {
        "candidate_plugin_binding": binding,
        "fake_spi_store_failures": {
            "status": "pass",
            "ordinals": 222,
            "test_sha256": sha256_file(ROOT / "crates/stfixest-plugin/src/estimate.rs"),
        },
        "actual_plugin_loader": {
            "status": "pass",
            "ordinals": 219,
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


def _run_supporting_gates(
    stata: Path,
    plugin: Path,
    plugin_identity: Mapping[str, Any],
    cargo: Path | None,
) -> dict[str, Any]:
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
                "--plugin",
                str(plugin.resolve()),
                "--plugin-sha256",
                plugin_identity["sha256"],
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
                "--plugin",
                str(plugin.resolve()),
                "--plugin-sha256",
                plugin_identity["sha256"],
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
                "--plugin",
                str(plugin.resolve()),
                "--plugin-sha256",
                plugin_identity["sha256"],
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
        if (
            sha256_file(plugin) != plugin_identity["sha256"]
            or plugin.stat().st_size != plugin_identity["size_bytes"]
        ):
            raise ContractError(f"supporting gate {name} changed the staged candidate plugin")
    return _supporting_gate_evidence(plugin_identity)


def capture_receipt(
    stata: Path,
    plugin: Path,
    projection_path: Path,
    output: Path,
    cargo: Path | None = None,
) -> str:
    projection = load_projection(projection_path)
    source_identity = _candidate_source_identity(ROOT)
    comparator_path = _repository_path(projection["comparator_receipt"]["path"], "comparator_receipt")
    comparator_document = _verify_reference_receipt_portable(
        comparator_path,
        _repository_path(projection["parent_manifest"]["path"], "parent_manifest"),
    )
    if comparator_document["seal"]["payload_sha256"] != projection["comparator_receipt"]["payload_sha256"]:
        raise ContractError("comparator payload seal differs from projection")
    supplied_plugin, supplied_identity = _supplied_plugin_identity(plugin)
    with tempfile.TemporaryDirectory(prefix="stfixest-candidate-transaction-") as raw:
        plugin_dir = Path(raw) / "candidate-plugin"
        plugin_dir.mkdir()
        staged_plugin = plugin_dir / supplied_identity["filename"]
        shutil.copyfile(supplied_plugin, staged_plugin)
        staged_plugin.chmod(supplied_plugin.stat().st_mode & 0o777)
        if (
            sha256_file(staged_plugin) != supplied_identity["sha256"]
            or staged_plugin.stat().st_size != supplied_identity["size_bytes"]
        ):
            raise ContractError("staged candidate plugin differs from the supplied plugin")
        environment, cases, capture_sha, plugin_identity, warning_audit = _run_stata_capture(
            stata, staged_plugin, projection_path, source_identity
        )
        if {
            key: plugin_identity[key]
            for key in ("filename", "sha256", "size_bytes", "loader_policy")
        } != supplied_identity:
            raise ContractError("captured candidate plugin identity changed after staging")
        comparisons = compare_cases(cases, comparator_document["payload"]["cases"], projection)
        supporting_gates = _run_supporting_gates(
            stata, staged_plugin, plugin_identity, cargo
        )
        if (
            sha256_file(supplied_plugin) != supplied_identity["sha256"]
            or supplied_plugin.stat().st_size != supplied_identity["size_bytes"]
            or sha256_file(staged_plugin) != supplied_identity["sha256"]
            or staged_plugin.stat().st_size != supplied_identity["size_bytes"]
        ):
            raise ContractError("candidate plugin bytes changed during the gate transaction")
    if _candidate_source_identity(ROOT) != source_identity:
        raise ContractError("candidate tracked source state changed during capture")
    payload = {
        "schema_version": 2,
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
            "source": source_identity,
            "plugin": plugin_identity,
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
        "warning_audit": warning_audit,
    }
    document = seal_payload(payload)
    encoded = json.dumps(document, sort_keys=True, indent=2, ensure_ascii=True) + "\n"
    _write_exclusive_receipt(output, encoded)
    return document["seal"]["payload_sha256"]


def verify_candidate_receipt(
    path: Path,
    projection_path: Path = DEFAULT_PROJECTION,
    identity_path: Path = DEFAULT_CANDIDATE_RECEIPT_IDENTITY,
) -> dict[str, Any]:
    projection = load_projection(projection_path)
    canonical_identity = _assert_canonical_candidate_receipt(path, identity_path)
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
    if document["seal"]["payload_sha256"] != canonical_identity["payload_sha256"]:
        raise ContractError("candidate receipt payload seal differs from its canonical identity")
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
            "warning_audit",
        },
        "candidate receipt payload",
    )
    _assert_redacted(payload)
    if (
        payload.get("schema_version") != 2
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
    ):
        raise ContractError("candidate receipt identity/boundary is stale")
    candidate = payload["candidate"]
    if not isinstance(candidate, Mapping):
        raise ContractError("candidate receipt candidate identity is malformed")
    _require_keys(
        candidate,
        {"command", "intended_public_command", "source", "plugin", "capture_sha256"},
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
    _verify_candidate_source_identity(candidate["source"], ROOT)
    plugin = candidate["plugin"]
    _validate_plugin_identity(plugin)
    if plugin["embedded_git_commit"] != candidate["source"]["commit"][:12]:
        raise ContractError("candidate plugin/source commit provenance is stale")
    warning_audit = payload.get("warning_audit")
    _validate_warning_audit(warning_audit)
    if payload.get("supporting_gates") != _supporting_gate_evidence(plugin):
        raise ContractError("candidate receipt supporting-gate binding is stale")
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
    comparator_document = _verify_reference_receipt_portable(
        _repository_path(
            projection["comparator_receipt"]["path"], "comparator_receipt"
        ),
        _repository_path(projection["parent_manifest"]["path"], "parent_manifest"),
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
    capture_parser.add_argument("--plugin", type=Path, required=True)
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
