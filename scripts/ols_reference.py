#!/usr/bin/env python3
"""Freeze and verify licensed-Stata OLS reference evidence.

The capture path is intentionally reference-only.  It never invokes StFixest,
never searches a user's installed PLUS/PERSONAL trees, and never downloads an
upstream dependency.  A capture starts only from clean local git checkouts at
the exact revisions in ``references.lock.toml``.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import math
import os
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
from statistics import geometric_mean, median
from typing import Any, Callable, Iterable, Mapping, Sequence

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "benchmarks/manifests/ols_v1.json"
DEFAULT_LOCK = ROOT / "references.lock.toml"
CAPTURE_PREFIX = "STFIXEST_OLS_CAPTURE|"
CAPTURE_SENTINEL = "END_FIXTURE|ols_v1_reference_capture|rc=0"
LOG_SENTINEL = "END_FIXTURE|baseline_reghdfe|rc=0"
RECEIPT_TYPE = "stfixest-ols-v1-reference"
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
HEX_COMMIT = re.compile(r"^[0-9a-f]{40}$")
STATA_HEX = re.compile(r"^(?:\.|[+-][0-9A-Fa-f]+(?:\.[0-9A-Fa-f]+)?X[+-][0-9A-Fa-f]+)$")
SAFE_ID = re.compile(r"^[a-z][a-z0-9_]{1,63}$")
SAFE_FIELD = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*(?:\([0-9]+\))?$")
REFERENCE_NAMES = ("reghdfe", "ftools", "require")
ENVIRONMENT_FIELDS = {
    "stata_version",
    "stata_flavor",
    "stata_processors",
    "stata_processors_limit",
    "os",
    "machine_type",
}
ALLOWED_OUTPUTS = {
    "sample": "__stfx_sample",
    "residual": "__stfx_resid",
    "xb": "__stfx_xb",
    "xbd": "__stfx_xbd",
    "d": "__stfx_d",
    "fe_worker": "__stfx_fe_worker",
    "fe_firm": "__stfx_fe_firm",
}
EXPECTED_POLICIES = {"must_succeed", "must_error", "record"}
REQUIRED_SURFACE = {
    "models": {
        "no_fe",
        "one_fe",
        "multi_fe",
        "categorical_interaction",
        "intercept_slope",
        "slope_only",
        "multiple_slopes",
        "ordinary_factor",
        "ordinary_interaction",
        "time_series_operators",
        "string_fe",
        "disconnected_components",
        "no_regressors",
        "collinearity",
    },
    "sample": {
        "if_in",
        "missing",
        "missing_fe",
        "missing_cluster",
        "singleton_cascade",
        "keepsingletons",
        "empty_sample_error",
        "one_observation_error",
    },
    "weights": {
        "unweighted",
        "aweight",
        "fweight",
        "pweight",
        "iweight_probe",
        "constant_rescaling",
        "zero_weight",
        "negative_weight",
        "fractional_fweight",
        "missing_weight",
        "large_weight",
        "weight_if_in",
        "weighted_singletons",
    },
    "inference": {
        "vce_unadjusted",
        "vce_robust",
        "cluster_oneway",
        "cluster_twoway",
        "cluster_threeway",
        "cluster_interaction",
        "fe_nested_cluster",
        "dof_default",
        "dof_none",
        "dof_firstpair",
        "dof_continuous",
    },
    "outputs": {
        "ereturn_scalars",
        "coefficient_status",
        "e_sample",
        "residual",
        "predict_xb",
        "predict_xbd",
        "absorbed_sum",
        "saved_fe",
        "fit_statistics",
        "test",
        "estat_summarize",
    },
    "failures": {
        "invalid_weight_error",
        "perfect_collinearity",
        "insufficient_observations",
        "all_dropped",
    },
    "benchmarks": {
        "dense_two_fe",
        "sparse_two_fe",
        "absorbed_slope",
        "clustered_vce",
        "moderate_k",
    },
}


class ContractError(ValueError):
    """The frozen manifest, raw capture, or receipt violates its schema."""


class PreflightError(RuntimeError):
    """A local reference or input cannot be proved immutable and current."""


@dataclass(frozen=True)
class ReferenceIdentity:
    name: str
    commit: str
    tree: str
    checkout: Path
    ado_dir: Path

    def receipt_value(self) -> dict[str, str]:
        return {"commit": self.commit, "git_tree": self.tree}


@dataclass(frozen=True)
class ParsedCapture:
    environment: dict[str, str]
    cases: list[dict[str, Any]]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def canonical_json_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode("utf-8")


def _expect_keys(value: Mapping[str, Any], required: set[str], optional: set[str], where: str) -> None:
    keys = set(value)
    missing = required - keys
    extra = keys - required - optional
    if missing or extra:
        pieces = []
        if missing:
            pieces.append("missing " + ", ".join(sorted(missing)))
        if extra:
            pieces.append("unknown " + ", ".join(sorted(extra)))
        raise ContractError(f"{where}: {'; '.join(pieces)}")


def _relative_input(root: Path, raw: str, where: str) -> Path:
    candidate = Path(raw)
    if candidate.is_absolute() or ".." in candidate.parts:
        raise ContractError(f"{where}: input path must be repository-relative")
    resolved = (root / candidate).resolve()
    try:
        resolved.relative_to(root.resolve())
    except ValueError as exc:
        raise ContractError(f"{where}: input escapes the repository") from exc
    return resolved


def _validate_hash_bound_file(root: Path, spec: Mapping[str, Any], where: str) -> Path:
    _expect_keys(spec, {"path", "sha256"}, set(), where)
    if not isinstance(spec["path"], str) or not isinstance(spec["sha256"], str):
        raise ContractError(f"{where}: path and sha256 must be strings")
    if not HEX_SHA256.fullmatch(spec["sha256"]):
        raise ContractError(f"{where}: sha256 is not a lowercase SHA-256")
    path = _relative_input(root, spec["path"], where)
    if not path.is_file():
        raise ContractError(f"{where}: missing input {spec['path']}")
    actual = sha256_file(path)
    if actual != spec["sha256"]:
        raise ContractError(f"{where}: stale or mutable input {spec['path']}; expected {spec['sha256']}, observed {actual}")
    return path


def validate_manifest(manifest: Mapping[str, Any], root: Path = ROOT) -> None:
    """Validate the complete, closed OLS-v1 schema and every bound input."""
    _expect_keys(
        manifest,
        {
            "schema_version",
            "manifest_id",
            "frozen",
            "reference",
            "capture",
            "benchmark_contract",
            "datasets",
            "required_surface",
            "semantic_cases",
            "benchmark_cases",
        },
        set(),
        "manifest",
    )
    if manifest["schema_version"] != 1 or manifest["manifest_id"] != "ols_v1" or manifest["frozen"] is not True:
        raise ContractError("manifest: expected frozen ols_v1 schema version 1")

    reference = manifest["reference"]
    if not isinstance(reference, Mapping):
        raise ContractError("reference must be an object")
    _expect_keys(reference, {"command", "lock_name", "dependencies", "isolation_required"}, set(), "reference")
    if reference != {
        "command": "reghdfe",
        "lock_name": "reghdfe",
        "dependencies": ["ftools", "require"],
        "isolation_required": True,
    }:
        raise ContractError("reference must require isolated pinned reghdfe, ftools, and require")

    capture = manifest["capture"]
    if not isinstance(capture, Mapping):
        raise ContractError("capture must be an object")
    _expect_keys(
        capture,
        {"script", "runner", "generator", "seed", "machine_protocol", "terminal_sentinel", "receipt_schema"},
        set(),
        "capture",
    )
    _validate_hash_bound_file(root, capture["script"], "capture.script")
    _validate_hash_bound_file(root, capture["runner"], "capture.runner")
    _validate_hash_bound_file(root, capture["generator"], "capture.generator")
    if capture["seed"] != 20260821 or capture["machine_protocol"] != 1 or capture["receipt_schema"] != 1:
        raise ContractError("capture: seed and protocol versions are frozen")
    if capture["terminal_sentinel"] != CAPTURE_SENTINEL:
        raise ContractError("capture: terminal sentinel is not the OLS-v1 sentinel")

    benchmark_contract = manifest["benchmark_contract"]
    if not isinstance(benchmark_contract, Mapping):
        raise ContractError("benchmark_contract must be an object")
    _expect_keys(
        benchmark_contract,
        {"candidate_state", "arms", "primary_min_observations", "thread_counts", "repetitions", "required_metadata", "thresholds"},
        set(),
        "benchmark_contract",
    )
    if benchmark_contract["candidate_state"] != "disabled_fail_closed":
        raise ContractError("benchmark_contract: candidate must remain disabled until stfeols is implemented")
    if benchmark_contract["arms"] != {"comparator": "reghdfe", "candidate": "stfeols"}:
        raise ContractError("benchmark_contract: comparator/candidate arms must be reghdfe and stfeols")
    if benchmark_contract["primary_min_observations"] != 10_000_000:
        raise ContractError("benchmark_contract: primary cases require at least 10,000,000 observations")
    if benchmark_contract["thread_counts"] != [1, 8]:
        raise ContractError("benchmark_contract: one- and eight-thread gates are required")
    if benchmark_contract["repetitions"] != {"cold": 1, "warm": 5}:
        raise ContractError("benchmark_contract: exactly one cold and at least five warm runs are frozen")
    if set(benchmark_contract["required_metadata"]) != {
        "sample_identity_sha256",
        "singleton_identity_sha256",
        "peak_rss_bytes",
        "host",
        "stata",
        "thread_count",
    }:
        raise ContractError("benchmark_contract: required evidence metadata is incomplete")
    if benchmark_contract["thresholds"] != {
        "geometric_mean_speedup_min": 1.5,
        "primary_case_time_ratio_max": 1.0,
        "primary_case_rss_ratio_max": 1.0,
    }:
        raise ContractError("benchmark_contract: performance thresholds are not the OLS-v1 gates")

    datasets = manifest["datasets"]
    if not isinstance(datasets, list) or not datasets:
        raise ContractError("datasets must be a nonempty list")
    dataset_ids: set[str] = set()
    dataset_by_id: dict[str, Mapping[str, Any]] = {}
    for index, dataset in enumerate(datasets):
        where = f"datasets[{index}]"
        if not isinstance(dataset, Mapping):
            raise ContractError(f"{where} must be an object")
        _expect_keys(dataset, {"id", "kind", "source"}, {"arguments"}, where)
        dataset_id = dataset["id"]
        if not isinstance(dataset_id, str) or not SAFE_ID.fullmatch(dataset_id) or dataset_id in dataset_ids:
            raise ContractError(f"{where}: invalid or duplicate id")
        dataset_ids.add(dataset_id)
        dataset_by_id[dataset_id] = dataset
        if dataset["kind"] not in {"fixture_csv", "generated_dta"}:
            raise ContractError(f"{where}: unsupported dataset kind")
        _validate_hash_bound_file(root, dataset["source"], f"{where}.source")
        if dataset["kind"] == "fixture_csv" and "arguments" in dataset:
            raise ContractError(f"{where}: fixture_csv cannot have generator arguments")
        if dataset["kind"] == "generated_dta":
            arguments = dataset.get("arguments")
            if not isinstance(arguments, list) or len(arguments) != 5 or not all(isinstance(v, int) and v > 0 for v in arguments):
                raise ContractError(f"{where}: generated_dta requires five positive integer arguments")

    required_surface = manifest["required_surface"]
    if not isinstance(required_surface, Mapping):
        raise ContractError("required_surface must be an object")
    if set(required_surface) != set(REQUIRED_SURFACE):
        raise ContractError("required_surface categories do not match the OLS-v1 contract")
    declared_surface: set[str] = set()
    for category, expected in REQUIRED_SURFACE.items():
        values = required_surface[category]
        if not isinstance(values, list) or set(values) != expected or len(values) != len(expected):
            raise ContractError(f"required_surface.{category} does not match the frozen contract")
        declared_surface.update(f"{category}:{value}" for value in values)

    all_cases: list[Mapping[str, Any]] = []
    case_ids: set[str] = set()
    covered: set[str] = set()
    for group, benchmark in (("semantic_cases", False), ("benchmark_cases", True)):
        cases = manifest[group]
        if not isinstance(cases, list) or not cases:
            raise ContractError(f"{group} must be a nonempty list")
        for index, case in enumerate(cases):
            where = f"{group}[{index}]"
            if not isinstance(case, Mapping):
                raise ContractError(f"{where} must be an object")
            _expect_keys(
                case,
                {"id", "dataset", "command", "expected", "covers", "capture", "repetitions"},
                {"arms", "primary", "thread_count", "setup", "postestimation"},
                where,
            )
            case_id = case["id"]
            if not isinstance(case_id, str) or not SAFE_ID.fullmatch(case_id) or case_id in case_ids:
                raise ContractError(f"{where}: invalid or duplicate case id")
            case_ids.add(case_id)
            if case["dataset"] not in dataset_ids:
                raise ContractError(f"{where}: unknown dataset {case['dataset']!r}")
            command = case["command"]
            if not isinstance(command, str) or not (command.startswith("reghdfe ") or command.startswith("reghdfe,")):
                raise ContractError(f"{where}: command must invoke reghdfe directly")
            if any(token in command for token in ("\n", "\r", ";", "!", " do ", " run ")):
                raise ContractError(f"{where}: command contains control syntax")
            setup = case.get("setup", [])
            if not isinstance(setup, list) or not all(
                isinstance(item, str) and item and not any(token in item for token in ("\n", "\r", ";", "!"))
                for item in setup
            ):
                raise ContractError(f"{where}: setup commands are unsafe")
            if case["expected"] not in EXPECTED_POLICIES:
                raise ContractError(f"{where}: invalid expected policy")
            covers = case["covers"]
            if not isinstance(covers, list) or not covers or not all(isinstance(value, str) for value in covers):
                raise ContractError(f"{where}: covers must be a nonempty string list")
            unknown = set(covers) - declared_surface
            if unknown:
                raise ContractError(f"{where}: unknown coverage token(s): {sorted(unknown)}")
            covered.update(covers)

            capture_spec = case["capture"]
            if not isinstance(capture_spec, Mapping):
                raise ContractError(f"{where}.capture must be an object")
            _expect_keys(capture_spec, {"scalars", "macros", "matrices", "outputs"}, {"identities"}, f"{where}.capture")
            for key in ("scalars", "macros", "matrices", "outputs", "identities"):
                if key == "identities" and key not in capture_spec:
                    continue
                values = capture_spec[key]
                if not isinstance(values, list) or len(values) != len(set(values)) or not all(isinstance(value, str) for value in values):
                    raise ContractError(f"{where}.capture.{key} must contain unique strings")
            if not all(SAFE_FIELD.fullmatch(value) for value in capture_spec["scalars"] + capture_spec["macros"] + capture_spec["matrices"]):
                raise ContractError(f"{where}.capture contains an unsafe e() field")
            if not set(capture_spec["outputs"]).issubset(ALLOWED_OUTPUTS):
                raise ContractError(f"{where}.capture.outputs contains an unsupported output")
            if not set(capture_spec.get("identities", [])).issubset({"sample", "singletons"}):
                raise ContractError(f"{where}.capture.identities contains an unsupported identity")
            if "residual" in capture_spec["outputs"] and "residuals(__stfx_resid)" not in command:
                raise ContractError(f"{where}: residual output requires a named residuals() target")
            for output in ("fe_worker", "fe_firm"):
                if output in capture_spec["outputs"] and ALLOWED_OUTPUTS[output] not in command:
                    raise ContractError(f"{where}: {output} requires an explicitly named absorb target")

            repetitions = case["repetitions"]
            if not isinstance(repetitions, Mapping):
                raise ContractError(f"{where}.repetitions must be an object")
            _expect_keys(repetitions, {"cold", "warm"}, set(), f"{where}.repetitions")
            if repetitions["cold"] != 1 or not isinstance(repetitions["warm"], int):
                raise ContractError(f"{where}: repetitions must have one cold run")
            if benchmark and repetitions["warm"] < 5:
                raise ContractError(f"{where}: benchmark cases require at least five warm repetitions")
            if not benchmark and repetitions["warm"] != 0:
                raise ContractError(f"{where}: semantic cases have no timing-only warm repetitions")
            if benchmark and case["expected"] != "must_succeed":
                raise ContractError(f"{where}: benchmark must succeed")
            if benchmark:
                if case.get("primary") is not True:
                    raise ContractError(f"{where}: every frozen benchmark is a primary gate")
                thread_count = case.get("thread_count")
                if thread_count not in benchmark_contract["thread_counts"]:
                    raise ContractError(f"{where}: benchmark thread count must be one or eight")
                if repetitions != benchmark_contract["repetitions"]:
                    raise ContractError(f"{where}: benchmark repetitions differ from the frozen contract")
                dataset = dataset_by_id[case["dataset"]]
                if dataset["kind"] != "generated_dta" or dataset["arguments"][0] < benchmark_contract["primary_min_observations"]:
                    raise ContractError(f"{where}: primary benchmark dataset is smaller than 10,000,000 observations")
                arms = case.get("arms")
                if not isinstance(arms, Mapping) or set(arms) != {"comparator", "candidate"}:
                    raise ContractError(f"{where}: comparator and candidate arm commands are required")
                if arms.get("comparator") != command:
                    raise ContractError(f"{where}: comparator arm must equal the reference-only capture command")
                candidate = arms.get("candidate")
                if not isinstance(candidate, str) or not (candidate.startswith("stfeols ") or candidate.startswith("stfeols,")):
                    raise ContractError(f"{where}: candidate arm must invoke stfeols directly")
                if any(token in candidate for token in ("\n", "\r", ";", "!", " do ", " run ")):
                    raise ContractError(f"{where}: candidate command contains control syntax")
                if f"threads({thread_count})" not in candidate:
                    raise ContractError(f"{where}: candidate arm does not lock its thread count")
                if set(capture_spec.get("identities", [])) != {"sample", "singletons"}:
                    raise ContractError(f"{where}: sample and singleton identities are required")
                if "num_singletons" not in capture_spec["scalars"]:
                    raise ContractError(f"{where}: num_singletons is required for benchmark parity")
            else:
                if any(name in case for name in ("arms", "primary", "thread_count")):
                    raise ContractError(f"{where}: benchmark-only fields appear on a semantic case")
                post = case.get("postestimation", [])
                if not isinstance(post, list):
                    raise ContractError(f"{where}: postestimation must be a list")
                post_ids: set[str] = set()
                for post_index, item in enumerate(post):
                    post_where = f"{where}.postestimation[{post_index}]"
                    if not isinstance(item, Mapping):
                        raise ContractError(f"{post_where} must be an object")
                    _expect_keys(item, {"id", "command", "scalars", "matrices"}, set(), post_where)
                    if item["id"] not in {"test", "estat_summarize"} or item["id"] in post_ids:
                        raise ContractError(f"{post_where}: invalid or duplicate id")
                    post_ids.add(item["id"])
                    prefix = "test " if item["id"] == "test" else "estat summarize"
                    if not isinstance(item["command"], str) or not item["command"].startswith(prefix):
                        raise ContractError(f"{post_where}: postestimation command does not match its id")
                    for field in ("scalars", "matrices"):
                        values = item[field]
                        if not isinstance(values, list) or len(values) != len(set(values)) or not all(SAFE_FIELD.fullmatch(value) for value in values):
                            raise ContractError(f"{post_where}.{field} must contain unique safe fields")
            all_cases.append(case)

    missing_surface = declared_surface - covered
    if missing_surface:
        raise ContractError("manifest does not cover: " + ", ".join(sorted(missing_surface)))
    benchmark_threads: dict[str, set[int]] = {}
    for case in manifest["benchmark_cases"]:
        token = case["covers"][0]
        benchmark_threads.setdefault(token, set()).add(case["thread_count"])
    if set(benchmark_threads) != {f"benchmarks:{item}" for item in REQUIRED_SURFACE["benchmarks"]} or any(
        counts != {1, 8} for counts in benchmark_threads.values()
    ):
        raise ContractError("each benchmark design requires both one- and eight-thread primary cases")


def load_manifest(path: Path = DEFAULT_MANIFEST, root: Path = ROOT) -> dict[str, Any]:
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ContractError(f"cannot read manifest {path}: {exc}") from exc
    if not isinstance(manifest, dict):
        raise ContractError("manifest root must be an object")
    validate_manifest(manifest, root)
    return manifest


def load_reference_lock(path: Path = DEFAULT_LOCK) -> dict[str, Any]:
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise ContractError(f"cannot read reference lock {path}: {exc}") from exc
    if value.get("schema_version") not in {1, 2}:
        raise ContractError("unsupported references.lock.toml schema")
    return value


def _reference_by_name(lock: Mapping[str, Any], name: str) -> Mapping[str, Any]:
    matches = [entry for entry in lock.get("reference", []) if entry.get("name") == name]
    if len(matches) != 1:
        raise PreflightError(f"reference lock must contain exactly one {name!r} entry")
    return matches[0]


def _git(checkout: Path, arguments: Sequence[str], runner: Callable[..., subprocess.CompletedProcess[str]]) -> str:
    completed = runner(
        ["git", "-C", str(checkout), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.returncode:
        raise PreflightError(f"git {' '.join(arguments)} failed for isolated {checkout.name} checkout")
    return completed.stdout.strip()


def preflight_reference_root(
    reference_root: Path,
    lock: Mapping[str, Any],
    repository_root: Path = ROOT,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> dict[str, ReferenceIdentity]:
    """Require the full exact, clean, hash-bound reference dependency closure."""
    if not reference_root.is_absolute():
        raise PreflightError("--reference-root must be an absolute path")
    if reference_root.is_symlink() or not reference_root.is_dir():
        raise PreflightError("--reference-root must be a real directory, not a symlink")
    resolved_root = reference_root.resolve()
    resolved_repository = repository_root.resolve()
    if resolved_root == resolved_repository or resolved_repository in resolved_root.parents or resolved_root in resolved_repository.parents:
        raise PreflightError("reference checkouts must be outside the StFixest repository")

    entries: dict[str, Mapping[str, Any]] = {}
    for name in REFERENCE_NAMES:
        entry = _reference_by_name(lock, name)
        commit = entry.get("commit")
        if entry.get("pin_status") != "resolved" or not isinstance(commit, str) or not HEX_COMMIT.fullmatch(commit):
            raise PreflightError(f"{name} exact pin is unresolved; update references.lock.toml from adjudicated provenance before capture")
        entries[name] = entry

    identities: dict[str, ReferenceIdentity] = {}
    for name in REFERENCE_NAMES:
        entry = entries[name]
        commit = entry["commit"]
        checkout_name = entry.get("checkout_dir")
        ado_subdir = entry.get("capture_ado_subdir")
        required_files = entry.get("required_capture_files")
        required_hashes = entry.get("required_capture_sha256")
        if not isinstance(checkout_name, str) or not SAFE_ID.fullmatch(checkout_name):
            raise PreflightError(f"{name} checkout_dir is not locked")
        if not isinstance(ado_subdir, str) or Path(ado_subdir).is_absolute() or ".." in Path(ado_subdir).parts:
            raise PreflightError(f"{name} capture_ado_subdir is not safe")
        if not isinstance(required_files, list) or not required_files:
            raise PreflightError(f"{name} required_capture_files are not locked")
        if not isinstance(required_hashes, Mapping) or set(required_hashes) != set(required_files) or not all(
            isinstance(value, str) and HEX_SHA256.fullmatch(value) for value in required_hashes.values()
        ):
            raise PreflightError(f"{name} required_capture_sha256 is not a closed exact file inventory")
        checkout = resolved_root / checkout_name
        if checkout.is_symlink() or not checkout.is_dir() or checkout.resolve() != checkout:
            raise PreflightError(f"{name} must be a non-symlink direct child of --reference-root")
        top = Path(_git(checkout, ["rev-parse", "--show-toplevel"], runner)).resolve()
        if top != checkout:
            raise PreflightError(f"{name} path is not the root of its isolated git checkout")
        observed_commit = _git(checkout, ["rev-parse", "HEAD"], runner)
        if observed_commit != commit:
            raise PreflightError(f"{name} checkout is stale: expected {commit}, observed {observed_commit}")
        if _git(checkout, ["status", "--porcelain=v1", "--untracked-files=all"], runner):
            raise PreflightError(f"{name} checkout is mutable or dirty")
        tree = _git(checkout, ["rev-parse", "HEAD^{tree}"], runner)
        if not HEX_COMMIT.fullmatch(tree):
            raise PreflightError(f"{name} git tree identity is malformed")
        for raw in required_files:
            if not isinstance(raw, str) or Path(raw).is_absolute() or ".." in Path(raw).parts:
                raise PreflightError(f"{name} has an unsafe required file")
            required = checkout / raw
            if required.is_symlink() or not required.is_file():
                raise PreflightError(f"{name} required tracked file is missing: {raw}")
            _git(checkout, ["ls-files", "--error-unmatch", "--", raw], runner)
            observed_sha = sha256_file(required)
            if observed_sha != required_hashes[raw]:
                raise PreflightError(
                    f"{name} required tracked file hash mismatch: {raw}; expected {required_hashes[raw]}, observed {observed_sha}"
                )
        ado_dir = checkout / ado_subdir
        if not ado_dir.is_dir():
            raise PreflightError(f"{name} ado directory is missing: {ado_subdir}")
        identities[name] = ReferenceIdentity(name, commit, tree, checkout, ado_dir)
    return identities


def _safe_extract_git_archive(data: bytes, destination: Path) -> None:
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:") as archive:
        for member in archive.getmembers():
            member_path = Path(member.name)
            if (
                member_path.is_absolute()
                or ".." in member_path.parts
                or not (member.isfile() or member.isdir())
            ):
                raise PreflightError("reference git archive contains an unsafe path or non-regular entry")
        archive.extractall(destination)


def stage_references(
    identities: Mapping[str, ReferenceIdentity],
    lock: Mapping[str, Any],
    destination: Path,
    runner: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run,
) -> dict[str, Path]:
    """Stage only committed reference bytes into the private capture directory."""
    staged: dict[str, Path] = {}
    for name, identity in identities.items():
        target = destination / name
        target.mkdir(parents=True)
        completed = runner(
            ["git", "-C", str(identity.checkout), "archive", "--format=tar", identity.commit],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if completed.returncode:
            raise PreflightError(f"could not stage committed {name} reference bytes")
        _safe_extract_git_archive(completed.stdout, target)
        entry = _reference_by_name(lock, name)
        ado_dir = target / entry["capture_ado_subdir"]
        for raw in entry["required_capture_files"]:
            staged_file = target / raw
            if not staged_file.is_file():
                raise PreflightError(f"staged {name} reference is incomplete: {raw}")
            if sha256_file(staged_file) != entry["required_capture_sha256"][raw]:
                raise PreflightError(f"staged {name} reference hash mismatch: {raw}")
        staged[name] = ado_dir
    return staged


def manifest_bound_inputs(manifest: Mapping[str, Any], root: Path = ROOT) -> dict[str, str]:
    """Return repository-relative input names and their revalidated hashes."""
    specs: list[Mapping[str, str]] = [
        manifest["capture"]["script"],
        manifest["capture"]["runner"],
        manifest["capture"]["generator"],
    ]
    specs.extend(dataset["source"] for dataset in manifest["datasets"])
    result: dict[str, str] = {}
    for spec in specs:
        path = _validate_hash_bound_file(root, spec, f"input {spec['path']}")
        result[path.relative_to(root.resolve()).as_posix()] = spec["sha256"]
    return dict(sorted(result.items()))


def stage_manifest_inputs(manifest: Mapping[str, Any], root: Path, destination: Path) -> dict[str, Path]:
    staged: dict[str, Path] = {}
    for dataset in manifest["datasets"]:
        source = _relative_input(root, dataset["source"]["path"], f"dataset {dataset['id']}")
        target = destination / f"{dataset['id']}{source.suffix}"
        shutil.copyfile(source, target)
        if sha256_file(target) != dataset["source"]["sha256"]:
            raise PreflightError(f"dataset changed while staging: {dataset['id']}")
        target.chmod(stat.S_IRUSR | stat.S_IRGRP | stat.S_IROTH)
        staged[dataset["id"]] = target
    generator = _relative_input(root, manifest["capture"]["generator"]["path"], "capture.generator")
    generator_target = destination / "generate_panel.do"
    shutil.copyfile(generator, generator_target)
    if sha256_file(generator_target) != manifest["capture"]["generator"]["sha256"]:
        raise PreflightError("generator changed while staging")
    generator_target.chmod(stat.S_IRUSR | stat.S_IRGRP | stat.S_IROTH)
    staged["__generator__"] = generator_target
    return staged


def _stata_quote(path: Path | str) -> str:
    value = str(path).replace("\\", "/")
    if '"' in value or "\n" in value or "\r" in value:
        raise ContractError("Stata path contains an unsupported character")
    return f'"{value}"'


def _emit_file_line(parts: Iterable[str]) -> str:
    joined = "|".join(parts)
    return f'file write __stfx_capture "{joined}" _n'


def _render_matrix(case_id: str, name: str, source: str = "e", record: str = "matrix", post_id: str | None = None) -> list[str]:
    prefix = [CAPTURE_PREFIX[:-1], record, case_id]
    if post_id is not None:
        prefix.append(post_id)
    return [
        f"capture matrix __stfx_matrix = {source}({name})",
        "if _rc {",
        "    " + _emit_file_line([CAPTURE_PREFIX[:-1], "absent", case_id, record, name]),
        "}",
        "else {",
        "    local __stfx_rows = rowsof(__stfx_matrix)",
        "    local __stfx_cols = colsof(__stfx_matrix)",
        "    local __stfx_rownames : rowfullnames __stfx_matrix",
        "    local __stfx_colnames : colfullnames __stfx_matrix",
        "    forvalues __stfx_r = 1/`__stfx_rows' {",
        "        local __stfx_rowname : word `__stfx_r' of `__stfx_rownames'",
        "        forvalues __stfx_c = 1/`__stfx_cols' {",
        "            local __stfx_colname : word `__stfx_c' of `__stfx_colnames'",
        "            local __stfx_value : display %21x __stfx_matrix[`__stfx_r',`__stfx_c']",
        f'            file write __stfx_capture "{"|".join(prefix)}|{name}|`__stfx_r\'|`__stfx_c\'|`__stfx_rowname\'|`__stfx_colname\'|`__stfx_value\'" _n',
        "        }",
        "    }",
        "}",
    ]


def _render_case(case: Mapping[str, Any], dataset: Mapping[str, Any], staged: Mapping[str, Path], run_dir: Path) -> list[str]:
    case_id = case["id"]
    lines = ["clear"]
    if dataset["kind"] == "fixture_csv":
        lines.append(f"import delimited using {_stata_quote(staged[dataset['id']])}, clear varnames(1) case(preserve)")
    else:
        output = run_dir / f"{dataset['id']}.dta"
        arguments = " ".join(str(value) for value in dataset["arguments"])
        lines.extend(
            [
                f"do {_stata_quote(staged['__generator__'])} {arguments} {_stata_quote(output)}",
                f"use {_stata_quote(output)}, clear",
            ]
        )
    lines.extend(["capture confirm variable obs_id", "if _rc generate long obs_id = _n"])
    lines.extend(case.get("setup", []))
    if "thread_count" in case:
        lines.append(f"set processors {case['thread_count']}")
    lines.append(_emit_file_line([CAPTURE_PREFIX[:-1], "case_begin", case_id]))
    total = case["repetitions"]["cold"] + case["repetitions"]["warm"]
    for repetition in range(total):
        timing_kind = "cold" if repetition == 0 else "warm"
        lines.extend(
            [
                "timer clear 99",
                "timer on 99",
                f"capture noisily {case['command']}",
                "local __stfx_rc = _rc",
                "timer off 99",
                "quietly timer list 99",
                "local __stfx_seconds : display %21x r(t99)",
                f'file write __stfx_capture "{CAPTURE_PREFIX}timing|{case_id}|{repetition + 1}|{timing_kind}|`__stfx_seconds\'" _n',
            ]
        )
        if repetition + 1 < total:
            lines.extend(
                [
                    "if `__stfx_rc' != 0 {",
                    '    file write __stfx_capture "END_FIXTURE|ols_v1_reference_capture|rc=9" _n',
                    "    file close __stfx_capture",
                    "    exit 9",
                    "}",
                    "estimates clear",
                ]
            )
    lines.append(f'file write __stfx_capture "{CAPTURE_PREFIX}return_code|{case_id}|`__stfx_rc\'" _n')

    capture = case["capture"]
    lines.append("if `__stfx_rc' == 0 {")
    if capture.get("identities") and "sample" not in capture["outputs"]:
        lines.append("    generate byte __stfx_sample = e(sample)")
    for output in capture["outputs"]:
        target = ALLOWED_OUTPUTS[output]
        if output == "sample":
            lines.append(f"    generate byte {target} = e(sample)")
        elif output in {"xb", "xbd", "d"}:
            lines.append(f"    capture noisily predict double {target}, {output}")
            lines.append("    if _rc exit _rc")
    for name in capture["scalars"]:
        lines.extend(
            [
                "    local __stfx_enames : e(scalars)",
                f'    local __stfx_present : list posof "{name}" in __stfx_enames',
                "    if `__stfx_present' == 0 {",
                "        " + _emit_file_line([CAPTURE_PREFIX[:-1], "absent", case_id, "scalar", name]),
                "    }",
                "    else {",
                f"        local __stfx_value : display %21x e({name})",
                f'        file write __stfx_capture "{CAPTURE_PREFIX}scalar|{case_id}|{name}|`__stfx_value\'" _n',
                "    }",
            ]
        )
    for name in capture["macros"]:
        lines.extend(
            [
                "    local __stfx_enames : e(macros)",
                f'    local __stfx_present : list posof "{name}" in __stfx_enames',
                "    if `__stfx_present' == 0 {",
                "        " + _emit_file_line([CAPTURE_PREFIX[:-1], "absent", case_id, "macro", name]),
                "    }",
                "    else {",
                f"        local __stfx_text `\"`e({name})'\"'",
                "        local __stfx_text = subinstr(`\"`__stfx_text'\"', \"%\", \"%25\", .)",
                "        local __stfx_text = subinstr(`\"`__stfx_text'\"', \"|\", \"%7C\", .)",
                f'        file write __stfx_capture "{CAPTURE_PREFIX}macro|{case_id}|{name}|`__stfx_text\'" _n',
                "    }",
            ]
        )
    for name in capture["matrices"]:
        lines.extend("    " + line if line else line for line in _render_matrix(case_id, name))
    if capture["outputs"]:
        lines.extend(
            [
                f'    file write __stfx_capture "{CAPTURE_PREFIX}observation_count|{case_id}|`=_N\'" _n',
                "    forvalues __stfx_obs = 1/`=_N' {",
                "        local __stfx_obs_id : display %21x obs_id[`__stfx_obs']",
            ]
        )
        for output in capture["outputs"]:
            target = ALLOWED_OUTPUTS[output]
            lines.extend(
                [
                    f"        local __stfx_value : display %21x {target}[`__stfx_obs']",
                    f'        file write __stfx_capture "{CAPTURE_PREFIX}observation|{case_id}|`__stfx_obs_id\'|{output}|`__stfx_value\'" _n',
                ]
            )
        lines.append("    }")
    for identity in capture.get("identities", []):
        condition = "__stfx_sample" if identity == "sample" else "!__stfx_sample"
        lines.extend(
            [
                "    preserve",
                f"    quietly keep if {condition}",
                "    if _N == 0 local __stfx_identity empty",
                "    else {",
                "        quietly datasignature obs_id",
                "        local __stfx_identity `\"`r(datasignature)'\"'",
                "    }",
                f'    file write __stfx_capture "{CAPTURE_PREFIX}identity|{case_id}|{identity}|`=_N\'|`__stfx_identity\'" _n',
                "    restore",
            ]
        )
    for post in case.get("postestimation", []):
        post_id = post["id"]
        lines.extend(
            [
                f'    file write __stfx_capture "{CAPTURE_PREFIX}post_begin|{case_id}|{post_id}" _n',
                f"    capture noisily {post['command']}",
                "    local __stfx_post_rc = _rc",
                f'    file write __stfx_capture "{CAPTURE_PREFIX}post_return_code|{case_id}|{post_id}|`__stfx_post_rc\'" _n',
                "    if `__stfx_post_rc' == 0 {",
            ]
        )
        for name in post["scalars"]:
            lines.extend(
                [
                    "        local __stfx_rnames : r(scalars)",
                    f'        local __stfx_present : list posof "{name}" in __stfx_rnames',
                    "        if `__stfx_present' == 0 {",
                    "            " + _emit_file_line([CAPTURE_PREFIX[:-1], "absent", case_id, f"post_scalar:{post_id}", name]),
                    "        }",
                    "        else {",
                    f"            local __stfx_value : display %21x r({name})",
                    f'            file write __stfx_capture "{CAPTURE_PREFIX}post_scalar|{case_id}|{post_id}|{name}|`__stfx_value\'" _n',
                    "        }",
                ]
            )
        for name in post["matrices"]:
            lines.extend("        " + line if line else line for line in _render_matrix(case_id, name, "r", "post_matrix", post_id))
        lines.extend(
            [
                "    }",
                f'    file write __stfx_capture "{CAPTURE_PREFIX}post_end|{case_id}|{post_id}" _n',
            ]
        )
    lines.append("}")
    lines.append(_emit_file_line([CAPTURE_PREFIX[:-1], "case_end", case_id]))
    if case["expected"] == "must_succeed":
        lines.extend(
            [
                "if `__stfx_rc' != 0 {",
                '    file write __stfx_capture "END_FIXTURE|ols_v1_reference_capture|rc=9" _n',
                "    file close __stfx_capture",
                "    exit 9",
                "}",
            ]
        )
    elif case["expected"] == "must_error":
        lines.extend(
            [
                "if `__stfx_rc' == 0 {",
                '    file write __stfx_capture "END_FIXTURE|ols_v1_reference_capture|rc=9" _n',
                "    file close __stfx_capture",
                "    exit 9",
                "}",
            ]
        )
    return lines


def render_capture_do(
    manifest: Mapping[str, Any],
    staged_inputs: Mapping[str, Path],
    run_dir: Path,
    manifest_sha256: str,
) -> str:
    datasets = {dataset["id"]: dataset for dataset in manifest["datasets"]}
    lines = [
        "version 18.0",
        "args capture_file manifest_sha256",
        "set more off",
        f'''if `"`manifest_sha256'"' != "{manifest_sha256}" exit 610''',
        '''file open __stfx_capture using `"`capture_file'"', write text replace''',
        _emit_file_line([CAPTURE_PREFIX[:-1], "schema", "1"]),
        _emit_file_line([CAPTURE_PREFIX[:-1], "manifest_sha256", manifest_sha256]),
        f'file write __stfx_capture "{CAPTURE_PREFIX}environment|stata_version|`c(stata_version)\'" _n',
        f'file write __stfx_capture "{CAPTURE_PREFIX}environment|stata_flavor|`c(flavor)\'" _n',
        f'file write __stfx_capture "{CAPTURE_PREFIX}environment|stata_processors|`c(processors)\'" _n',
        f'file write __stfx_capture "{CAPTURE_PREFIX}environment|stata_processors_limit|`c(processors_lic)\'" _n',
        f'file write __stfx_capture "{CAPTURE_PREFIX}environment|os|`c(os)\'" _n',
        f'file write __stfx_capture "{CAPTURE_PREFIX}environment|machine_type|`c(machine_type)\'" _n',
        "set seed 20260821",
    ]
    for case in [*manifest["semantic_cases"], *manifest["benchmark_cases"]]:
        lines.extend(_render_case(case, datasets[case["dataset"]], staged_inputs, run_dir))
    lines.extend([f'file write __stfx_capture "{CAPTURE_SENTINEL}" _n', "file close __stfx_capture", ""])
    return "\n".join(lines)


def validate_terminal_sentinel(text: str, expected: str = CAPTURE_SENTINEL) -> None:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    sentinels = [line for line in lines if line.startswith("END_FIXTURE|")]
    if not sentinels:
        raise ContractError(f"capture is missing terminal sentinel {expected!r}")
    if sentinels != [expected]:
        raise ContractError(f"capture must contain exactly one successful sentinel; observed {sentinels!r}")
    if lines[-1] != expected:
        raise ContractError("capture sentinel is not the final nonblank record")


def _validate_matrix_entries(entries: Any, requested: Sequence[str], where: str) -> None:
    if not isinstance(entries, list) or not all(isinstance(item, Mapping) for item in entries):
        raise ContractError(f"{where}: matrices must be a list of coordinate records")
    by_name: dict[str, list[Mapping[str, Any]]] = {}
    seen: set[tuple[str, int, int]] = set()
    for item in entries:
        _expect_keys(item, {"name", "row", "column", "row_name", "column_name", "value_hex"}, set(), where)
        name, row, column = item["name"], item["row"], item["column"]
        if name not in requested or not isinstance(row, int) or not isinstance(column, int) or row <= 0 or column <= 0:
            raise ContractError(f"{where}: invalid matrix name or coordinate")
        coordinate = (name, row, column)
        if coordinate in seen:
            raise ContractError(f"{where}: duplicate matrix coordinate {coordinate}")
        seen.add(coordinate)
        if not isinstance(item["row_name"], str) or not item["row_name"] or not isinstance(item["column_name"], str) or not item["column_name"]:
            raise ContractError(f"{where}: matrix names must be nonempty")
        if not isinstance(item["value_hex"], str) or not STATA_HEX.fullmatch(item["value_hex"]):
            raise ContractError(f"{where}: matrix value is not canonical Stata hex")
        by_name.setdefault(name, []).append(item)
    if set(by_name) != set(requested):
        raise ContractError(f"{where}: requested matrix evidence is incomplete")
    for name, values in by_name.items():
        max_row = max(item["row"] for item in values)
        max_column = max(item["column"] for item in values)
        expected_coordinates = {(row, column) for row in range(1, max_row + 1) for column in range(1, max_column + 1)}
        if {(item["row"], item["column"]) for item in values} != expected_coordinates:
            raise ContractError(f"{where}: matrix {name} is not a complete rectangle")
        row_names: dict[int, str] = {}
        column_names: dict[int, str] = {}
        for item in values:
            if item["row"] in row_names and row_names[item["row"]] != item["row_name"]:
                raise ContractError(f"{where}: matrix {name} has contradictory row names")
            if item["column"] in column_names and column_names[item["column"]] != item["column_name"]:
                raise ContractError(f"{where}: matrix {name} has contradictory column names")
            row_names[item["row"]] = item["row_name"]
            column_names[item["column"]] = item["column_name"]


def _validate_case_evidence(expected: Mapping[str, Any], captured: Mapping[str, Any]) -> None:
    case_id = expected["id"]
    _expect_keys(
        captured,
        {"id", "return_code", "timings", "scalars", "macros", "matrices", "observation_count", "observations", "identities", "postestimation", "absent"},
        set(),
        f"case {case_id}",
    )
    if captured["id"] != case_id or not isinstance(captured["return_code"], int):
        raise ContractError(f"case {case_id}: identity or return code is malformed")
    rc = captured["return_code"]
    if expected["expected"] == "must_succeed" and rc != 0:
        raise ContractError(f"case {case_id} unexpectedly returned {rc}")
    if expected["expected"] == "must_error" and rc == 0:
        raise ContractError(f"case {case_id} unexpectedly succeeded")
    timings = captured["timings"]
    total = expected["repetitions"]["cold"] + expected["repetitions"]["warm"]
    if not isinstance(timings, list) or len(timings) != total:
        raise ContractError(f"case {case_id} has incomplete timing repetitions")
    for index, timing in enumerate(timings, 1):
        if not isinstance(timing, Mapping) or timing != {
            "repetition": index,
            "kind": "cold" if index == 1 else "warm",
            "seconds_hex": timing.get("seconds_hex"),
        } or not isinstance(timing.get("seconds_hex"), str) or not STATA_HEX.fullmatch(timing["seconds_hex"]):
            raise ContractError(f"case {case_id}: timing sequence or hex value is malformed")
    if not isinstance(captured["absent"], list) or captured["absent"]:
        raise ContractError(f"case {case_id}: requested evidence cannot be absent")
    if rc != 0:
        if any((captured["scalars"], captured["macros"], captured["matrices"], captured["observations"], captured["identities"], captured["postestimation"])):
            raise ContractError(f"case {case_id}: error result contains contradictory scientific evidence")
        if captured["observation_count"] is not None:
            raise ContractError(f"case {case_id}: error result contains an observation count")
        return

    capture = expected["capture"]
    for kind in ("scalars", "macros"):
        values = captured[kind]
        if not isinstance(values, Mapping) or set(values) != set(capture[kind]):
            raise ContractError(f"case {case_id}: requested {kind} are incomplete or unknown")
        if kind == "scalars" and any(not isinstance(value, str) or not STATA_HEX.fullmatch(value) for value in values.values()):
            raise ContractError(f"case {case_id}: scalar value is not canonical Stata hex")
        if kind == "macros" and any(not isinstance(value, str) for value in values.values()):
            raise ContractError(f"case {case_id}: macro value is malformed")
    _validate_matrix_entries(captured["matrices"], capture["matrices"], f"case {case_id}")

    requested_outputs = capture["outputs"]
    observations = captured["observations"]
    if not isinstance(observations, list) or not all(isinstance(item, Mapping) for item in observations):
        raise ContractError(f"case {case_id}: observation evidence is malformed")
    if requested_outputs:
        count = captured["observation_count"]
        if not isinstance(count, int) or count < 0 or len(observations) != count * len(requested_outputs):
            raise ContractError(f"case {case_id}: observation evidence is incomplete")
        pairs: set[tuple[str, str]] = set()
        ids_by_field: dict[str, set[str]] = {field: set() for field in requested_outputs}
        for item in observations:
            _expect_keys(item, {"obs_id_hex", "field", "value_hex"}, set(), f"case {case_id} observation")
            obs_id, field, value = item["obs_id_hex"], item["field"], item["value_hex"]
            if field not in ids_by_field or not isinstance(obs_id, str) or not STATA_HEX.fullmatch(obs_id) or not isinstance(value, str) or not STATA_HEX.fullmatch(value):
                raise ContractError(f"case {case_id}: observation coordinate or value is malformed")
            if (obs_id, field) in pairs:
                raise ContractError(f"case {case_id}: duplicate observation id/field pair")
            pairs.add((obs_id, field))
            ids_by_field[field].add(obs_id)
        identity_sets = list(ids_by_field.values())
        if any(len(ids) != count for ids in identity_sets) or any(ids != identity_sets[0] for ids in identity_sets[1:]):
            raise ContractError(f"case {case_id}: observation IDs are incomplete, duplicated, or contradictory")
    elif captured["observation_count"] is not None or observations:
        raise ContractError(f"case {case_id}: unrequested observation evidence is present")

    identities = captured["identities"]
    if not isinstance(identities, Mapping) or set(identities) != set(capture.get("identities", [])):
        raise ContractError(f"case {case_id}: sample/singleton identities are incomplete or unknown")
    for name, identity in identities.items():
        if not isinstance(identity, Mapping) or set(identity) != {"observation_count", "sha256"}:
            raise ContractError(f"case {case_id}: identity {name} is malformed")
        if not isinstance(identity["observation_count"], int) or identity["observation_count"] < 0 or not HEX_SHA256.fullmatch(identity["sha256"]):
            raise ContractError(f"case {case_id}: identity {name} is invalid")
    if {"sample", "singletons"}.issubset(identities) and identities["sample"]["observation_count"] + identities["singletons"]["observation_count"] <= 0:
        raise ContractError(f"case {case_id}: sample and singleton identities are contradictory")

    posts = captured["postestimation"]
    requested_posts = expected.get("postestimation", [])
    if not isinstance(posts, list) or [item.get("id") for item in posts if isinstance(item, Mapping)] != [item["id"] for item in requested_posts]:
        raise ContractError(f"case {case_id}: postestimation evidence is incomplete or out of order")
    for requested, post in zip(requested_posts, posts):
        _expect_keys(post, {"id", "return_code", "scalars", "matrices"}, set(), f"case {case_id} post {requested['id']}")
        if post["id"] != requested["id"] or post["return_code"] != 0:
            raise ContractError(f"case {case_id}: postestimation {requested['id']} failed")
        if not isinstance(post["scalars"], Mapping) or set(post["scalars"]) != set(requested["scalars"]):
            raise ContractError(f"case {case_id}: postestimation scalars are incomplete")
        if any(not isinstance(value, str) or not STATA_HEX.fullmatch(value) for value in post["scalars"].values()):
            raise ContractError(f"case {case_id}: postestimation scalar is not canonical Stata hex")
        _validate_matrix_entries(post["matrices"], requested["matrices"], f"case {case_id} post {requested['id']}")


def parse_capture(text: str, manifest: Mapping[str, Any], manifest_sha256: str) -> ParsedCapture:
    validate_terminal_sentinel(text)
    records = [line.strip().split("|") for line in text.splitlines() if line.strip() and line.strip() != CAPTURE_SENTINEL]
    if not records or records[0] != [CAPTURE_PREFIX[:-1], "schema", "1"]:
        raise ContractError("capture schema header is missing or out of order")
    if len(records) < 2 or records[1] != [CAPTURE_PREFIX[:-1], "manifest_sha256", manifest_sha256]:
        raise ContractError("capture was produced from a different manifest")
    expected_cases = [*manifest["semantic_cases"], *manifest["benchmark_cases"]]
    expected_by_id = {case["id"]: case for case in expected_cases}
    environment: dict[str, str] = {}
    built: dict[str, dict[str, Any]] = {}
    active: str | None = None
    active_post: str | None = None
    for parts in records[2:]:
        if not parts or parts[0] != CAPTURE_PREFIX[:-1] or len(parts) < 3:
            raise ContractError("capture contains an unknown or malformed record")
        kind = parts[1]
        if kind == "environment" and len(parts) == 4 and active is None:
            if parts[2] in environment or parts[2] not in ENVIRONMENT_FIELDS or not parts[3]:
                raise ContractError("capture environment is duplicated, unknown, or empty")
            environment[parts[2]] = parts[3]
            continue
        if kind == "case_begin" and len(parts) == 3 and active is None:
            case_id = parts[2]
            if case_id not in expected_by_id or case_id in built:
                raise ContractError("capture begins an unknown or duplicate case")
            active = case_id
            built[case_id] = {"id": case_id, "return_code": None, "timings": [], "scalars": {}, "macros": {}, "matrices": [], "observation_count": None, "observations": [], "identities": {}, "postestimation": [], "absent": []}
            continue
        if active is None or parts[2] != active:
            raise ContractError("capture record appears outside its active case")
        case = built[active]
        if kind == "return_code" and len(parts) == 4:
            if case["return_code"] is not None:
                raise ContractError("duplicate return code")
            try:
                case["return_code"] = int(parts[3])
            except ValueError as exc:
                raise ContractError("return code is not an integer") from exc
        elif kind == "timing" and len(parts) == 6:
            try:
                repetition = int(parts[3])
            except ValueError as exc:
                raise ContractError("timing repetition is not an integer") from exc
            case["timings"].append({"repetition": repetition, "kind": parts[4], "seconds_hex": parts[5]})
        elif kind in {"scalar", "macro"} and len(parts) == 5:
            container = case[kind + "s"]
            if parts[3] in container:
                raise ContractError(f"duplicate {kind} {parts[3]}")
            container[parts[3]] = parts[4]
        elif kind == "matrix" and len(parts) == 9:
            try:
                row, column = int(parts[4]), int(parts[5])
            except ValueError as exc:
                raise ContractError("matrix coordinates are not integers") from exc
            case["matrices"].append({"name": parts[3], "row": row, "column": column, "row_name": parts[6], "column_name": parts[7], "value_hex": parts[8]})
        elif kind == "observation_count" and len(parts) == 4:
            if case["observation_count"] is not None:
                raise ContractError("duplicate observation count")
            try:
                case["observation_count"] = int(parts[3])
            except ValueError as exc:
                raise ContractError("observation count is not an integer") from exc
        elif kind == "observation" and len(parts) == 6:
            case["observations"].append({"obs_id_hex": parts[3], "field": parts[4], "value_hex": parts[5]})
        elif kind == "identity" and len(parts) == 6:
            name = parts[3]
            if name in case["identities"]:
                raise ContractError("duplicate sample/singleton identity")
            try:
                count = int(parts[4])
            except ValueError as exc:
                raise ContractError("identity observation count is not an integer") from exc
            case["identities"][name] = {"observation_count": count, "sha256": hashlib.sha256(f"{count}|{parts[5]}".encode("utf-8")).hexdigest()}
        elif kind == "post_begin" and len(parts) == 4 and active_post is None:
            active_post = parts[3]
            case["postestimation"].append({"id": active_post, "return_code": None, "scalars": {}, "matrices": []})
        elif kind == "post_return_code" and len(parts) == 5 and active_post == parts[3]:
            post = case["postestimation"][-1]
            if post["return_code"] is not None:
                raise ContractError("duplicate postestimation return code")
            try:
                post["return_code"] = int(parts[4])
            except ValueError as exc:
                raise ContractError("postestimation return code is not an integer") from exc
        elif kind == "post_scalar" and len(parts) == 6 and active_post == parts[3]:
            post = case["postestimation"][-1]
            if parts[4] in post["scalars"]:
                raise ContractError("duplicate postestimation scalar")
            post["scalars"][parts[4]] = parts[5]
        elif kind == "post_matrix" and len(parts) == 10 and active_post == parts[3]:
            try:
                row, column = int(parts[5]), int(parts[6])
            except ValueError as exc:
                raise ContractError("postestimation matrix coordinates are not integers") from exc
            case["postestimation"][-1]["matrices"].append({"name": parts[4], "row": row, "column": column, "row_name": parts[7], "column_name": parts[8], "value_hex": parts[9]})
        elif kind == "post_end" and len(parts) == 4 and active_post == parts[3]:
            if case["postestimation"][-1]["return_code"] is None:
                raise ContractError("postestimation ended without a return code")
            active_post = None
        elif kind == "absent" and len(parts) == 5:
            case["absent"].append({"kind": parts[3], "name": parts[4]})
        elif kind == "case_end" and len(parts) == 3 and active_post is None:
            if case["return_code"] is None:
                raise ContractError("case ended without a return code")
            active = None
        else:
            raise ContractError(f"unknown or malformed capture record kind {kind!r}")
    if active is not None or active_post is not None:
        raise ContractError("capture ended inside a case or postestimation command")
    if list(built) != [case["id"] for case in expected_cases]:
        raise ContractError("capture cases are missing or out of manifest order")
    if set(environment) != ENVIRONMENT_FIELDS:
        raise ContractError("capture environment is incomplete")
    for expected in expected_cases:
        _validate_case_evidence(expected, built[expected["id"]])
    return ParsedCapture(environment, list(built.values()))


def _assert_redacted(value: Any, key_path: tuple[str, ...] = ()) -> None:
    forbidden_key_fragments = {"path", "executable", "serial", "license", "username", "hostname", "raw_log"}
    if key_path and any(fragment in key_path[-1].lower() for fragment in forbidden_key_fragments):
        raise ContractError(f"receipt contains forbidden sensitive field {'.'.join(key_path)}")
    if isinstance(value, Mapping):
        for key, item in value.items():
            if not isinstance(key, str):
                raise ContractError("receipt keys must be strings")
            _assert_redacted(item, (*key_path, key))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            _assert_redacted(item, (*key_path, str(index)))
    elif isinstance(value, str):
        if re.search(r"(?:^|\s)/(?:Users|home|private|tmp|var|Volumes)/", value) or re.search(r"[A-Za-z]:[\\/]", value):
            raise ContractError(f"receipt contains an absolute local path at {'.'.join(key_path)}")
        lowered = value.lower()
        if "serial number" in lowered or "licensed to" in lowered:
            raise ContractError(f"receipt contains licensed-user text at {'.'.join(key_path)}")


def seal_payload(payload: Mapping[str, Any]) -> dict[str, Any]:
    _assert_redacted(payload)
    digest = hashlib.sha256(canonical_json_bytes(payload)).hexdigest()
    return {
        "payload": payload,
        "seal": {
            "algorithm": "sha256",
            "canonicalization": "json-sort-keys-compact-ascii-v1",
            "payload_sha256": digest,
        },
    }


def write_sealed_receipt(path: Path, payload: Mapping[str, Any]) -> str:
    document = seal_payload(payload)
    encoded = json.dumps(document, sort_keys=True, indent=2, ensure_ascii=True) + "\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open("x", encoding="utf-8", newline="\n") as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
    except FileExistsError as exc:
        raise ContractError(f"refusing to overwrite immutable receipt: {path}") from exc
    path.chmod(stat.S_IRUSR | stat.S_IRGRP | stat.S_IROTH)
    return document["seal"]["payload_sha256"]


def evaluate_benchmark_gates(manifest: Mapping[str, Any], evidence: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    """Evaluate a complete future two-arm benchmark run; never upgrade a disabled arm."""
    contract = manifest["benchmark_contract"]
    if contract["candidate_state"] != "enabled":
        return {"status": "blocked", "reason": "candidate_stfeols_disabled", "thresholds": contract["thresholds"]}
    cases = manifest["benchmark_cases"]
    expected_inventory = {(case["id"], arm) for case in cases for arm in ("comparator", "candidate")}
    records: dict[tuple[str, str], Mapping[str, Any]] = {}
    required = {
        "case_id",
        "arm",
        "thread_count",
        "cold_seconds",
        "warm_seconds",
        "peak_rss_bytes",
        "sample_identity_sha256",
        "singleton_identity_sha256",
        "host",
        "stata",
    }
    for record in evidence:
        if not isinstance(record, Mapping):
            raise ContractError("benchmark gate evidence record must be an object")
        _expect_keys(record, required, set(), "benchmark gate evidence")
        key = (record["case_id"], record["arm"])
        if key not in expected_inventory or key in records:
            raise ContractError("benchmark gate evidence inventory is unknown or duplicated")
        case = next(item for item in cases if item["id"] == record["case_id"])
        if record["thread_count"] != case["thread_count"]:
            raise ContractError("benchmark gate evidence has contradictory thread metadata")
        if not isinstance(record["cold_seconds"], (int, float)) or isinstance(record["cold_seconds"], bool) or not math.isfinite(record["cold_seconds"]) or record["cold_seconds"] <= 0:
            raise ContractError("benchmark cold timing must be finite and positive")
        warm = record["warm_seconds"]
        if not isinstance(warm, list) or len(warm) < contract["repetitions"]["warm"] or any(
            not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(value) or value <= 0 for value in warm
        ):
            raise ContractError("benchmark warm timings are incomplete or invalid")
        if not isinstance(record["peak_rss_bytes"], int) or record["peak_rss_bytes"] <= 0:
            raise ContractError("benchmark peak RSS must be a positive byte count")
        if not HEX_SHA256.fullmatch(record["sample_identity_sha256"]) or not HEX_SHA256.fullmatch(record["singleton_identity_sha256"]):
            raise ContractError("benchmark sample/singleton identity is malformed")
        for metadata in ("host", "stata"):
            if not isinstance(record[metadata], Mapping) or not record[metadata] or not all(
                isinstance(name, str) and isinstance(value, (str, int)) and value != "" for name, value in record[metadata].items()
            ):
                raise ContractError(f"benchmark {metadata} metadata is incomplete")
        records[key] = record
    if set(records) != expected_inventory:
        raise ContractError("benchmark gate evidence is incomplete; both arms are required for every case")

    speedups: list[float] = []
    per_case: list[dict[str, Any]] = []
    time_limit = contract["thresholds"]["primary_case_time_ratio_max"]
    rss_limit = contract["thresholds"]["primary_case_rss_ratio_max"]
    for case in cases:
        comparator = records[(case["id"], "comparator")]
        candidate = records[(case["id"], "candidate")]
        if comparator["host"] != candidate["host"] or comparator["stata"] != candidate["stata"]:
            raise ContractError(f"benchmark {case['id']} arms were not run on the same host and Stata environment")
        if comparator["sample_identity_sha256"] != candidate["sample_identity_sha256"] or comparator["singleton_identity_sha256"] != candidate["singleton_identity_sha256"]:
            raise ContractError(f"benchmark {case['id']} arms disagree on sample or singleton identity")
        comparator_time = median(comparator["warm_seconds"])
        candidate_time = median(candidate["warm_seconds"])
        time_ratio = candidate_time / comparator_time
        rss_ratio = candidate["peak_rss_bytes"] / comparator["peak_rss_bytes"]
        speedup = comparator_time / candidate_time
        speedups.append(speedup)
        per_case.append({"case_id": case["id"], "thread_count": case["thread_count"], "speedup": speedup, "time_ratio": time_ratio, "rss_ratio": rss_ratio})
        if case["primary"] and time_ratio > time_limit:
            return {"status": "fail", "reason": f"candidate_slower:{case['id']}", "thresholds": contract["thresholds"], "cases": per_case}
        if case["primary"] and rss_ratio > rss_limit:
            return {"status": "fail", "reason": f"candidate_rss_worse:{case['id']}", "thresholds": contract["thresholds"], "cases": per_case}
    observed_geomean = geometric_mean(speedups)
    if observed_geomean < contract["thresholds"]["geometric_mean_speedup_min"]:
        return {"status": "fail", "reason": "geometric_mean_speedup_below_threshold", "thresholds": contract["thresholds"], "geometric_mean_speedup": observed_geomean, "cases": per_case}
    return {"status": "pass", "thresholds": contract["thresholds"], "geometric_mean_speedup": observed_geomean, "cases": per_case}


def verify_receipt(
    path: Path,
    manifest_path: Path = DEFAULT_MANIFEST,
    lock_path: Path = DEFAULT_LOCK,
    root: Path = ROOT,
) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ContractError(f"cannot read receipt: {exc}") from exc
    if not isinstance(document, dict) or set(document) != {"payload", "seal"}:
        raise ContractError("receipt must contain exactly payload and seal")
    seal = document["seal"]
    if seal != {
        "algorithm": "sha256",
        "canonicalization": "json-sort-keys-compact-ascii-v1",
        "payload_sha256": hashlib.sha256(canonical_json_bytes(document["payload"])).hexdigest(),
    }:
        raise ContractError("receipt seal mismatch")
    if path.stat().st_mode & 0o222:
        raise ContractError("receipt is writable; immutable mode bits are required")
    payload = document["payload"]
    if not isinstance(payload, Mapping):
        raise ContractError("receipt payload must be an object")
    _expect_keys(
        payload,
        {
            "schema_version",
            "receipt_type",
            "manifest",
            "inputs",
            "generated_datasets",
            "references",
            "environment",
            "host",
            "benchmark_evidence",
            "benchmark_gate",
            "cases",
        },
        set(),
        "receipt payload",
    )
    _assert_redacted(payload)
    manifest = load_manifest(manifest_path, root)
    manifest_hash = sha256_file(manifest_path)
    if payload.get("receipt_type") != RECEIPT_TYPE or payload.get("schema_version") != 1:
        raise ContractError("receipt type or schema is unsupported")
    if payload.get("manifest") != {"id": manifest["manifest_id"], "sha256": manifest_hash}:
        raise ContractError("receipt is stale relative to the frozen manifest")
    if payload.get("inputs") != manifest_bound_inputs(manifest, root):
        raise ContractError("receipt input hashes are stale")
    expected_generated = {dataset["id"] for dataset in manifest["datasets"] if dataset["kind"] == "generated_dta"}
    generated = payload.get("generated_datasets", {})
    if set(generated) != expected_generated or not all(HEX_SHA256.fullmatch(value) for value in generated.values()):
        raise ContractError("receipt generated-dataset seals are incomplete")
    lock = load_reference_lock(lock_path)
    for name in REFERENCE_NAMES:
        entry = _reference_by_name(lock, name)
        receipt_reference = payload.get("references", {}).get(name, {})
        if entry.get("pin_status") != "resolved" or receipt_reference.get("commit") != entry.get("commit"):
            raise ContractError(f"receipt reference {name} is stale or unresolved")
        if not HEX_COMMIT.fullmatch(receipt_reference.get("git_tree", "")):
            raise ContractError(f"receipt reference {name} lacks a git tree seal")
    if set(payload.get("references", {})) != set(REFERENCE_NAMES):
        raise ContractError("receipt reference inventory is not closed")
    environment = payload.get("environment")
    if not isinstance(environment, Mapping) or set(environment) != ENVIRONMENT_FIELDS:
        raise ContractError("receipt environment is incomplete or unknown")
    if not all(isinstance(value, str) and value for value in environment.values()):
        raise ContractError("receipt environment values must be nonempty strings")

    host = payload.get("host")
    if not isinstance(host, Mapping) or set(host) != {"system", "machine", "cpu_count"}:
        raise ContractError("receipt host metadata is incomplete or unknown")
    if not isinstance(host["system"], str) or not host["system"] or not isinstance(host["machine"], str) or not host["machine"] or not isinstance(host["cpu_count"], int) or host["cpu_count"] <= 0:
        raise ContractError("receipt host metadata is malformed")
    gate = payload.get("benchmark_gate")
    if gate != {
        "status": "blocked",
        "reason": "candidate_stfeols_disabled",
        "thresholds": manifest["benchmark_contract"]["thresholds"],
    }:
        raise ContractError("reference-only receipt must not claim a benchmark gate pass")
    evidence = payload.get("benchmark_evidence")
    if not isinstance(evidence, list) or [item.get("case_id") for item in evidence if isinstance(item, Mapping)] != [
        case["id"] for case in manifest["benchmark_cases"]
    ]:
        raise ContractError("receipt benchmark comparator evidence is incomplete or out of order")
    benchmark_by_id = {case["id"]: case for case in manifest["benchmark_cases"]}
    for item in evidence:
        if not isinstance(item, Mapping):
            raise ContractError("receipt benchmark evidence must be an object")
        _expect_keys(item, {"case_id", "arm", "thread_count", "peak_rss_bytes", "sample_identity_sha256", "singleton_identity_sha256", "host", "stata"}, set(), "benchmark evidence")
        expected = benchmark_by_id[item["case_id"]]
        if item["arm"] != "comparator" or item["thread_count"] != expected["thread_count"]:
            raise ContractError("receipt benchmark arm or thread metadata is stale")
        if not isinstance(item["peak_rss_bytes"], int) or item["peak_rss_bytes"] <= 0:
            raise ContractError("receipt benchmark RSS metadata is invalid")
        if not HEX_SHA256.fullmatch(item["sample_identity_sha256"]) or not HEX_SHA256.fullmatch(item["singleton_identity_sha256"]):
            raise ContractError("receipt benchmark sample/singleton identity is invalid")
        if item["host"] != host:
            raise ContractError("receipt benchmark host metadata is contradictory")
        expected_stata = {"version": environment["stata_version"], "flavor": environment["stata_flavor"], "processors": environment["stata_processors"], "processors_limit": environment["stata_processors_limit"]}
        if item["stata"] != expected_stata:
            raise ContractError("receipt benchmark Stata metadata is contradictory")

    expected_cases = [*manifest["semantic_cases"], *manifest["benchmark_cases"]]
    captured_cases = payload.get("cases")
    if not isinstance(captured_cases, list) or [case.get("id") for case in captured_cases if isinstance(case, Mapping)] != [
        case["id"] for case in expected_cases
    ]:
        raise ContractError("receipt case inventory does not match the manifest")
    for expected, captured in zip(expected_cases, captured_cases):
        if not isinstance(captured, Mapping):
            raise ContractError("receipt case must be an object")
        _validate_case_evidence(expected, captured)
    return document


def _write_wrapper(
    path: Path,
    baseline: Path,
    case_do: Path,
    capture_file: Path,
    staged_references: Mapping[str, Path],
    manifest_sha256: str,
    isolated_sysdirs: Path,
) -> None:
    sysdirs = {}
    for name in ("personal", "plus", "oldplace", "site"):
        target = isolated_sysdirs / name
        target.mkdir(parents=True)
        sysdirs[name] = target
    lines = ["version 18.0", "clear all", "set more off"]
    for name, target in sysdirs.items():
        lines.append(f"sysdir set {name.upper()} {_stata_quote(target)}")
    lines.append(
        "do "
        + " ".join(
            _stata_quote(value)
            for value in (
                baseline,
                case_do,
                capture_file,
                staged_references["reghdfe"],
                staged_references["ftools"],
                staged_references["require"],
                manifest_sha256,
            )
        )
    )
    lines.extend(["exit 0", ""])
    path.write_text("\n".join(lines), encoding="utf-8")


def _host_metadata() -> dict[str, Any]:
    cpu_count = os.cpu_count()
    system = platform.system()
    machine = platform.machine()
    if not isinstance(cpu_count, int) or cpu_count <= 0 or not system or not machine:
        raise ContractError("host identity is incomplete; benchmark capture is fail closed")
    return {"system": system, "machine": machine, "cpu_count": cpu_count}


def _children_peak_rss_bytes() -> int:
    try:
        import resource

        value = int(resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)
    except (ImportError, AttributeError, OSError, ValueError) as exc:
        raise ContractError("peak child RSS is unavailable; benchmark capture is fail closed") from exc
    if value <= 0:
        raise ContractError("peak child RSS is zero; benchmark capture is fail closed")
    return value if sys.platform == "darwin" else value * 1024


def capture_reference(
    stata: str,
    reference_root: Path,
    receipt_path: Path,
    manifest_path: Path = DEFAULT_MANIFEST,
    lock_path: Path = DEFAULT_LOCK,
    root: Path = ROOT,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> str:
    manifest = load_manifest(manifest_path, root)
    lock = load_reference_lock(lock_path)
    manifest_hash = sha256_file(manifest_path)
    inputs_before = manifest_bound_inputs(manifest, root)
    identities = preflight_reference_root(reference_root, lock, root)

    with tempfile.TemporaryDirectory(prefix="stfixest-ols-reference-") as temporary:
        run_dir = Path(temporary)
        reference_stage = run_dir / "references"
        reference_stage.mkdir()
        staged_references = stage_references(identities, lock, reference_stage)
        input_stage = run_dir / "inputs"
        input_stage.mkdir()
        staged_inputs = stage_manifest_inputs(manifest, root, input_stage)
        case_do = run_dir / "ols_v1_cases.do"
        case_do.write_text(render_capture_do(manifest, staged_inputs, run_dir, manifest_hash), encoding="utf-8")
        capture_file = run_dir / "capture.machine"
        wrapper = run_dir / "run.do"
        baseline = _relative_input(root, manifest["capture"]["script"]["path"], "capture.script")
        _write_wrapper(wrapper, baseline, case_do, capture_file, staged_references, manifest_hash, run_dir / "sysdirs")
        completed = runner(
            [stata, "-b", "do", str(wrapper)],
            cwd=run_dir,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        peak_rss_bytes = _children_peak_rss_bytes()
        log_path = run_dir / "run.log"
        log_text = log_path.read_text(encoding="utf-8", errors="replace") if log_path.is_file() else ""
        log_sentinels = [line.strip() for line in log_text.splitlines() if line.strip().startswith("END_FIXTURE|")]
        if completed.returncode != 0 or log_sentinels != [LOG_SENTINEL]:
            raise ContractError("licensed Stata capture failed or lacked its unique success sentinel; raw log was not retained")
        if not capture_file.is_file():
            raise ContractError("licensed Stata capture produced no machine record")
        parsed = parse_capture(capture_file.read_text(encoding="utf-8", errors="strict"), manifest, manifest_hash)
        generated_datasets = {
            dataset["id"]: sha256_file(run_dir / f"{dataset['id']}.dta")
            for dataset in manifest["datasets"]
            if dataset["kind"] == "generated_dta"
        }

    # Detect edits made after preflight or while the licensed process ran.
    if sha256_file(manifest_path) != manifest_hash:
        raise PreflightError("the frozen OLS manifest changed during capture")
    if manifest_bound_inputs(manifest, root) != inputs_before:
        raise PreflightError("a manifest-bound dataset or script changed during capture")
    identities_after = preflight_reference_root(reference_root, lock, root)
    if {name: item.receipt_value() for name, item in identities_after.items()} != {
        name: item.receipt_value() for name, item in identities.items()
    }:
        raise PreflightError("a reference checkout changed during capture")

    host = _host_metadata()
    cases_by_id = {case["id"]: case for case in parsed.cases}
    stata = {
        "version": parsed.environment["stata_version"],
        "flavor": parsed.environment["stata_flavor"],
        "processors": parsed.environment["stata_processors"],
        "processors_limit": parsed.environment["stata_processors_limit"],
    }
    benchmark_evidence = []
    for expected in manifest["benchmark_cases"]:
        captured = cases_by_id[expected["id"]]
        benchmark_evidence.append(
            {
                "case_id": expected["id"],
                "arm": "comparator",
                "thread_count": expected["thread_count"],
                "peak_rss_bytes": peak_rss_bytes,
                "sample_identity_sha256": captured["identities"]["sample"]["sha256"],
                "singleton_identity_sha256": captured["identities"]["singletons"]["sha256"],
                "host": host,
                "stata": stata,
            }
        )
    payload = {
        "schema_version": 1,
        "receipt_type": RECEIPT_TYPE,
        "manifest": {"id": manifest["manifest_id"], "sha256": manifest_hash},
        "inputs": inputs_before,
        "generated_datasets": generated_datasets,
        "references": {name: identity.receipt_value() for name, identity in sorted(identities.items())},
        "environment": parsed.environment,
        "host": host,
        "benchmark_evidence": benchmark_evidence,
        "benchmark_gate": {
            "status": "blocked",
            "reason": "candidate_stfeols_disabled",
            "thresholds": manifest["benchmark_contract"]["thresholds"],
        },
        "cases": parsed.cases,
    }
    return write_sealed_receipt(receipt_path, payload)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--lock", type=Path, default=DEFAULT_LOCK)
    subparsers = parser.add_subparsers(dest="action", required=True)
    subparsers.add_parser("validate")
    preflight = subparsers.add_parser("preflight")
    preflight.add_argument("--reference-root", type=Path, required=True)
    capture = subparsers.add_parser("capture")
    capture.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    capture.add_argument("--reference-root", type=Path, required=True)
    capture.add_argument("--receipt", type=Path, required=True)
    verify = subparsers.add_parser("verify")
    verify.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()

    if args.action == "validate":
        manifest = load_manifest(args.manifest)
        print(f"OLS manifest PASS: {manifest['manifest_id']} {sha256_file(args.manifest)}")
    elif args.action == "preflight":
        identities = preflight_reference_root(args.reference_root, load_reference_lock(args.lock))
        for name, identity in sorted(identities.items()):
            print(f"{name}: commit={identity.commit} tree={identity.tree}")
    elif args.action == "capture":
        if not args.stata:
            raise SystemExit("set STATA_BIN or pass --stata")
        digest = capture_reference(args.stata, args.reference_root, args.receipt, args.manifest, args.lock)
        print(f"sealed OLS reference receipt: {args.receipt} sha256={digest}")
    else:
        document = verify_receipt(args.receipt, args.manifest, args.lock)
        print(f"OLS receipt PASS: {document['seal']['payload_sha256']}")


if __name__ == "__main__":
    main()
