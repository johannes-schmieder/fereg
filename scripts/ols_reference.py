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
import os
import re
import shutil
import stat
import subprocess
import tarfile
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
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
SAFE_ID = re.compile(r"^[a-z][a-z0-9_]{1,63}$")
SAFE_FIELD = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*(?:\([0-9]+\))?$")
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
        "string_fe",
        "disconnected_components",
        "no_regressors",
        "collinearity",
    },
    "sample": {
        "if_in",
        "missing",
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
        "dependencies": ["ftools"],
        "isolation_required": True,
    }:
        raise ContractError("reference must require isolated pinned reghdfe plus ftools")

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

    datasets = manifest["datasets"]
    if not isinstance(datasets, list) or not datasets:
        raise ContractError("datasets must be a nonempty list")
    dataset_ids: set[str] = set()
    for index, dataset in enumerate(datasets):
        where = f"datasets[{index}]"
        if not isinstance(dataset, Mapping):
            raise ContractError(f"{where} must be an object")
        _expect_keys(dataset, {"id", "kind", "source"}, {"arguments"}, where)
        dataset_id = dataset["id"]
        if not isinstance(dataset_id, str) or not SAFE_ID.fullmatch(dataset_id) or dataset_id in dataset_ids:
            raise ContractError(f"{where}: invalid or duplicate id")
        dataset_ids.add(dataset_id)
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
                set(),
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
            _expect_keys(capture_spec, {"scalars", "macros", "matrices", "outputs"}, set(), f"{where}.capture")
            for key in ("scalars", "macros", "matrices", "outputs"):
                values = capture_spec[key]
                if not isinstance(values, list) or len(values) != len(set(values)) or not all(isinstance(value, str) for value in values):
                    raise ContractError(f"{where}.capture.{key} must contain unique strings")
            if not all(SAFE_FIELD.fullmatch(value) for value in capture_spec["scalars"] + capture_spec["macros"] + capture_spec["matrices"]):
                raise ContractError(f"{where}.capture contains an unsafe e() field")
            if not set(capture_spec["outputs"]).issubset(ALLOWED_OUTPUTS):
                raise ContractError(f"{where}.capture.outputs contains an unsupported output")
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
            all_cases.append(case)

    missing_surface = declared_surface - covered
    if missing_surface:
        raise ContractError("manifest does not cover: " + ", ".join(sorted(missing_surface)))


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
    """Require exact, clean, direct-child git checkouts for reghdfe and ftools."""
    if not reference_root.is_absolute():
        raise PreflightError("--reference-root must be an absolute path")
    if reference_root.is_symlink() or not reference_root.is_dir():
        raise PreflightError("--reference-root must be a real directory, not a symlink")
    resolved_root = reference_root.resolve()
    resolved_repository = repository_root.resolve()
    if resolved_root == resolved_repository or resolved_repository in resolved_root.parents or resolved_root in resolved_repository.parents:
        raise PreflightError("reference checkouts must be outside the StFixest repository")

    entries: dict[str, Mapping[str, Any]] = {}
    for name in ("reghdfe", "ftools"):
        entry = _reference_by_name(lock, name)
        commit = entry.get("commit")
        if entry.get("pin_status") != "resolved" or not isinstance(commit, str) or not HEX_COMMIT.fullmatch(commit):
            raise PreflightError(f"{name} exact pin is unresolved; update references.lock.toml from adjudicated provenance before capture")
        entries[name] = entry

    identities: dict[str, ReferenceIdentity] = {}
    for name in ("reghdfe", "ftools"):
        entry = entries[name]
        commit = entry["commit"]
        checkout_name = entry.get("checkout_dir")
        ado_subdir = entry.get("capture_ado_subdir")
        required_files = entry.get("required_capture_files")
        if not isinstance(checkout_name, str) or not SAFE_ID.fullmatch(checkout_name):
            raise PreflightError(f"{name} checkout_dir is not locked")
        if not isinstance(ado_subdir, str) or Path(ado_subdir).is_absolute() or ".." in Path(ado_subdir).parts:
            raise PreflightError(f"{name} capture_ado_subdir is not safe")
        if not isinstance(required_files, list) or not required_files:
            raise PreflightError(f"{name} required_capture_files are not locked")
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
            if not (target / raw).is_file():
                raise PreflightError(f"staged {name} reference is incomplete: {raw}")
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


def _render_matrix(case_id: str, name: str) -> list[str]:
    return [
        f"capture matrix __stfx_matrix = e({name})",
        "if _rc {",
        "    " + _emit_file_line([CAPTURE_PREFIX[:-1], "absent", case_id, "matrix", name]),
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
        f'            file write __stfx_capture "{CAPTURE_PREFIX}matrix|{case_id}|{name}|`__stfx_r\'|`__stfx_c\'|`__stfx_rowname\'|`__stfx_colname\'|`__stfx_value\'" _n',
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
    for parts in records[2:]:
        if not parts or parts[0] != CAPTURE_PREFIX[:-1] or len(parts) < 3:
            raise ContractError("capture contains an unknown or malformed record")
        kind = parts[1]
        if kind == "environment" and len(parts) == 4 and active is None:
            if parts[2] in environment or parts[2] not in {"stata_version", "os", "machine_type"}:
                raise ContractError("capture environment is duplicated or unknown")
            environment[parts[2]] = parts[3]
            continue
        if kind == "case_begin" and len(parts) == 3 and active is None:
            case_id = parts[2]
            if case_id not in expected_by_id or case_id in built:
                raise ContractError("capture begins an unknown or duplicate case")
            active = case_id
            built[case_id] = {
                "id": case_id,
                "return_code": None,
                "timings": [],
                "scalars": {},
                "macros": {},
                "matrices": [],
                "observations": [],
                "absent": [],
            }
            continue
        if active is None:
            raise ContractError("capture record appears outside a case")
        if len(parts) < 3 or parts[2] != active:
            raise ContractError("capture case records are interleaved")
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
            if parts[4] not in {"cold", "warm"}:
                raise ContractError("timing kind is invalid")
            case["timings"].append({"repetition": repetition, "kind": parts[4], "seconds_hex": parts[5]})
        elif kind in {"scalar", "macro"} and len(parts) == 5:
            container = case[kind + "s"]
            if parts[3] in container:
                raise ContractError(f"duplicate {kind} {parts[3]}")
            container[parts[3]] = parts[4]
        elif kind == "matrix" and len(parts) == 9:
            try:
                row = int(parts[4])
                column = int(parts[5])
            except ValueError as exc:
                raise ContractError("matrix coordinates are not integers") from exc
            case["matrices"].append(
                {
                    "name": parts[3],
                    "row": row,
                    "column": column,
                    "row_name": parts[6],
                    "column_name": parts[7],
                    "value_hex": parts[8],
                }
            )
        elif kind == "observation" and len(parts) == 6:
            case["observations"].append({"obs_id_hex": parts[3], "field": parts[4], "value_hex": parts[5]})
        elif kind == "absent" and len(parts) == 5:
            case["absent"].append({"kind": parts[3], "name": parts[4]})
        elif kind == "case_end" and len(parts) == 3:
            if case["return_code"] is None:
                raise ContractError("case ended without a return code")
            active = None
        else:
            raise ContractError(f"unknown or malformed capture record kind {kind!r}")
    if active is not None:
        raise ContractError("capture ended inside a case")
    if list(built) != [case["id"] for case in expected_cases]:
        raise ContractError("capture cases are missing or out of manifest order")
    if set(environment) != {"stata_version", "os", "machine_type"}:
        raise ContractError("capture environment is incomplete")

    for expected in expected_cases:
        captured = built[expected["id"]]
        rc = captured["return_code"]
        if expected["expected"] == "must_succeed" and rc != 0:
            raise ContractError(f"case {expected['id']} unexpectedly returned {rc}")
        if expected["expected"] == "must_error" and rc == 0:
            raise ContractError(f"case {expected['id']} unexpectedly succeeded")
        expected_timing_count = expected["repetitions"]["cold"] + expected["repetitions"]["warm"]
        if len(captured["timings"]) != expected_timing_count:
            raise ContractError(f"case {expected['id']} has incomplete timing repetitions")
        if rc == 0:
            absent = {(item["kind"], item["name"]) for item in captured["absent"]}
            for kind in ("scalars", "macros", "matrices"):
                singular = kind[:-1] if kind != "matrices" else "matrix"
                present = set(captured[kind]) if isinstance(captured[kind], dict) else {item["name"] for item in captured[kind]}
                for name in expected["capture"][kind]:
                    if name not in present and (singular, name) not in absent:
                        raise ContractError(f"case {expected['id']} omitted requested {singular} {name}")
            observed_fields = {item["field"] for item in captured["observations"]}
            if not set(expected["capture"]["outputs"]).issubset(observed_fields):
                raise ContractError(f"case {expected['id']} omitted requested observation outputs")
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
    for name in ("reghdfe", "ftools"):
        entry = _reference_by_name(lock, name)
        receipt_reference = payload.get("references", {}).get(name, {})
        if entry.get("pin_status") != "resolved" or receipt_reference.get("commit") != entry.get("commit"):
            raise ContractError(f"receipt reference {name} is stale or unresolved")
        if not HEX_COMMIT.fullmatch(receipt_reference.get("git_tree", "")):
            raise ContractError(f"receipt reference {name} lacks a git tree seal")
    if set(payload.get("references", {})) != {"reghdfe", "ftools"}:
        raise ContractError("receipt reference inventory is not closed")
    environment = payload.get("environment")
    if not isinstance(environment, Mapping) or set(environment) != {"stata_version", "os", "machine_type"}:
        raise ContractError("receipt environment is incomplete or unknown")
    if not all(isinstance(value, str) and value for value in environment.values()):
        raise ContractError("receipt environment values must be nonempty strings")

    expected_cases = [*manifest["semantic_cases"], *manifest["benchmark_cases"]]
    captured_cases = payload.get("cases")
    if not isinstance(captured_cases, list) or [case.get("id") for case in captured_cases if isinstance(case, Mapping)] != [
        case["id"] for case in expected_cases
    ]:
        raise ContractError("receipt case inventory does not match the manifest")
    for expected, captured in zip(expected_cases, captured_cases):
        if not isinstance(captured, Mapping):
            raise ContractError("receipt case must be an object")
        _expect_keys(
            captured,
            {"id", "return_code", "timings", "scalars", "macros", "matrices", "observations", "absent"},
            set(),
            f"receipt case {expected['id']}",
        )
        return_code = captured["return_code"]
        if not isinstance(return_code, int):
            raise ContractError(f"receipt case {expected['id']} return code is not an integer")
        if expected["expected"] == "must_succeed" and return_code != 0:
            raise ContractError(f"receipt case {expected['id']} violates must_succeed")
        if expected["expected"] == "must_error" and return_code == 0:
            raise ContractError(f"receipt case {expected['id']} violates must_error")
        timings = captured["timings"]
        total = expected["repetitions"]["cold"] + expected["repetitions"]["warm"]
        if not isinstance(timings, list) or len(timings) != total:
            raise ContractError(f"receipt case {expected['id']} has incomplete timings")
        for index, timing in enumerate(timings, 1):
            expected_kind = "cold" if index == 1 else "warm"
            if not isinstance(timing, Mapping) or timing.get("repetition") != index or timing.get("kind") != expected_kind:
                raise ContractError(f"receipt case {expected['id']} timing sequence is malformed")
            if not isinstance(timing.get("seconds_hex"), str) or not timing["seconds_hex"]:
                raise ContractError(f"receipt case {expected['id']} timing value is malformed")
        for key in ("scalars", "macros"):
            if not isinstance(captured[key], Mapping) or not all(
                isinstance(name, str) and isinstance(value, str) for name, value in captured[key].items()
            ):
                raise ContractError(f"receipt case {expected['id']} {key} are malformed")
        for key in ("matrices", "observations", "absent"):
            if not isinstance(captured[key], list) or not all(isinstance(item, Mapping) for item in captured[key]):
                raise ContractError(f"receipt case {expected['id']} {key} are malformed")
        if return_code == 0:
            absent = {(item.get("kind"), item.get("name")) for item in captured["absent"]}
            matrix_names = {item.get("name") for item in captured["matrices"]}
            observed_fields = {item.get("field") for item in captured["observations"]}
            for name in expected["capture"]["scalars"]:
                if name not in captured["scalars"] and ("scalar", name) not in absent:
                    raise ContractError(f"receipt case {expected['id']} omitted scalar {name}")
            for name in expected["capture"]["macros"]:
                if name not in captured["macros"] and ("macro", name) not in absent:
                    raise ContractError(f"receipt case {expected['id']} omitted macro {name}")
            for name in expected["capture"]["matrices"]:
                if name not in matrix_names and ("matrix", name) not in absent:
                    raise ContractError(f"receipt case {expected['id']} omitted matrix {name}")
            if not set(expected["capture"]["outputs"]).issubset(observed_fields):
                raise ContractError(f"receipt case {expected['id']} omitted observation outputs")
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
                manifest_sha256,
            )
        )
    )
    lines.extend(["exit 0", ""])
    path.write_text("\n".join(lines), encoding="utf-8")


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

    payload = {
        "schema_version": 1,
        "receipt_type": RECEIPT_TYPE,
        "manifest": {"id": manifest["manifest_id"], "sha256": manifest_hash},
        "inputs": inputs_before,
        "generated_datasets": generated_datasets,
        "references": {name: identity.receipt_value() for name, identity in sorted(identities.items())},
        "environment": parsed.environment,
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
