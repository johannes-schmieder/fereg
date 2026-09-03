#!/usr/bin/env python3
"""Exact-source SCC orchestration for the fereg F0 regression gate.

This module owns the Linux/SCC mechanics only.  The platform-independent
scientific and performance decision is delegated to
``scripts/fereg_f0_regression.py evaluate`` after all array shards have
produced the closed evidence schema frozen by the manifest.
"""

from __future__ import annotations

import argparse
import copy
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import math
import os
import platform
import re
import shlex
import shutil
import socket
import stat
import statistics
import subprocess
import sys
import tarfile
import tempfile
import time
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence
from zoneinfo import ZoneInfo


PROJECT_ROOT = Path("/projectnb/welfgr/fereg")
MANIFEST_ID = "fereg_f0_regression_v1"
EVIDENCE_TYPE = "fereg-f0-regression-v1-evidence"
RECEIPT_SCHEMA = 1
PAIR_COUNT = 9
PAIR_ORDERS = ("AB", "BA", "AB", "BA", "AB", "BA", "AB", "BA", "AB")
T95_DF8 = 2.306004135
SCC_STATA_PROCESSOR_CAP = 4
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
RUN_ID_RE = re.compile(r"^[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$")
CELL_ID_RE = re.compile(r"^[a-z0-9][a-z0-9_]{0,95}$")
JOB_ID_RE = re.compile(r"^[1-9][0-9]*$")
EXPECTED_RUSTC = "rustc 1.85.1 (4eb161250 2025-03-15)"
EXPECTED_CARGO = "cargo 1.85.1 (d73d2caf9 2024-12-31)"
EXPECTED_PYTHON = "Python 3.12.4"
EXPECTED_CARGO_HOME = Path("/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo")
EXPECTED_RUSTUP_HOME = Path("/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup")
EXPECTED_RUSTUP_TOOLCHAIN = "1.85.1-x86_64-unknown-linux-gnu"
EXPECTED_QSUB = Path("/usr/local/bin/qsub")
EXPECTED_QSUB_SHA256 = "fdd46c981d8f3107a44e6985ca9e689e4969b7c6281c8e56a78420c084fd7a6e"
EXPECTED_QSUB_RESOLVED = "/usr/local/ogs-ge2011.11.p1/sge_root/bin/linux-x64/qsub"
EXPECTED_QSUB_SIZE = 1_732_888
EXPECTED_QACCT = Path("/usr/local/bin/qacct")
EXPECTED_QACCT_SHA256 = "aa8575f51ad1f07673ef862d6dfbe06381ebc53bdb88bb3a0256573ededc37e0"
EXPECTED_QACCT_RESOLVED = "/usr/local/ogs-ge2011.11.p1/sge_root/bin/linux-x64/qacct"
EXPECTED_QACCT_SIZE = 1_753_600
EXPECTED_QSTAT = Path("/usr/local/bin/qstat")
EXPECTED_QSTAT_SHA256 = "0fb1c3334e9885f7df3873986c71465ef8b5528674a938441dbaefb6214c9941"
EXPECTED_QSTAT_RESOLVED = "/usr/local/ogs-ge2011.11.p1/sge_root/bin/linux-x64/qstat"
EXPECTED_QSTAT_SIZE = 2_043_136
EXPECTED_SGE_ROOT = "/usr/local/sge/sge_root"
DARWIN_GIT = Path("/usr/bin/git")
DARWIN_GIT_SHA256 = "b8763cf250e607a778bb4603cecb5b90338814d0a3dfcba0d57b1de242f610e9"
DARWIN_GIT_SIZE = 118_640
SCC_STATA = Path("/share/pkg.8/stata-mp/19/install/bin/stata-mp")
SCC_STATA_SHA256 = "fd3b9ed7c1e6a5d9f4c9c8bf4053452dc78a85c310fc00f3333eeec0b7467ec1"
SCC_STATA_SIZE = 82_832_984
BUILD_TOOL_ROLES = (
    "bash", "cargo", "rustc", "rustup", "python3", "cc", "linker", "ar",
    "ranlib", "strip", "objcopy", "sha256sum", "readelf", "nm", "ldd", "uname", "dirname",
    "basename", "mkdir", "rm",
)

HARNESS_PATHS = (
    "benchmarks/scc/fereg_f0_scc.py",
    "benchmarks/scc/fereg_f0_setup_job.sh",
    "benchmarks/scc/fereg_f0_job.sh",
    "benchmarks/scc/fereg_f0_collector_job.sh",
    "benchmarks/scc/fereg_f0_deploy.sh",
    "benchmarks/scc/fereg_f0_submit.sh",
    "benchmarks/scc/fereg_f0_close.sh",
    "benchmarks/scc/fereg_f0_formal_build.sh",
    "scripts/fereg_f0_regression.py",
    "scripts/formal_rust_build_closure.py",
)

SUPPORTED_DESIGNS = {
    "dense_two_fe", "sparse_two_fe", "absorbed_slope", "clustered_vce",
    "moderate_k", "constrained_memory", "disconnected_two_fe", "saved_slope",
}


class SccContractError(RuntimeError):
    """A closed SCC input, shard, receipt, or accounting contract failed."""


def _expect_keys(value: Any, expected: set[str], where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping) or set(value) != expected:
        missing = sorted(expected - set(value)) if isinstance(value, Mapping) else sorted(expected)
        extra = sorted(set(value) - expected) if isinstance(value, Mapping) else []
        raise SccContractError(
            f"{where} fields are not closed; missing={missing}, extra={extra}"
        )
    return value


def _receipt_sentinel(path: Path, label: str) -> str:
    return f"{label}|{sha256_file(path)}\n"


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise SccContractError(f"cannot read JSON {path.name}: {exc}") from exc


def _write_exclusive(path: Path, data: bytes, mode: int = 0o444) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError as exc:
        raise SccContractError(f"refusing to overwrite immutable output: {path}") from exc
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        path.chmod(mode)
    except BaseException:
        try:
            path.unlink()
        except OSError:
            pass
        raise


def write_json_exclusive(path: Path, value: Any, mode: int = 0o444) -> None:
    _write_exclusive(path, json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True).encode("ascii") + b"\n", mode)


def sealed_document(payload: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "payload": payload,
        "seal": {
            "algorithm": "sha256",
            "canonicalization": "json-sort-keys-compact-ascii-v1",
            "payload_sha256": sha256_bytes(canonical_json_bytes(payload)),
        },
    }


def write_sealed(path: Path, payload: Mapping[str, Any]) -> str:
    document = sealed_document(payload)
    write_json_exclusive(path, document)
    return str(document["seal"]["payload_sha256"])


def read_sealed(path: Path, receipt_type: str | None = None) -> Mapping[str, Any]:
    document = _json(path)
    if not isinstance(document, Mapping) or set(document) != {"payload", "seal"}:
        raise SccContractError(f"invalid receipt envelope: {path.name}")
    payload, seal = document["payload"], document["seal"]
    if not isinstance(payload, Mapping) or not isinstance(seal, Mapping):
        raise SccContractError(f"invalid receipt fields: {path.name}")
    if seal != sealed_document(payload)["seal"]:
        raise SccContractError(f"receipt seal mismatch: {path.name}")
    if receipt_type is not None and payload.get("receipt_type") != receipt_type:
        raise SccContractError(f"unexpected receipt type: {path.name}")
    return payload


def read_evaluator_receipt(path: Path) -> Mapping[str, Any]:
    """Read the generic evaluator's newline-terminated canonical seal."""
    document = _json(path)
    if not isinstance(document, Mapping) or set(document) != {"payload", "seal"}:
        raise SccContractError(f"invalid evaluator receipt envelope: {path.name}")
    payload = document["payload"]
    seal = document["seal"]
    if not isinstance(payload, Mapping) or not isinstance(seal, Mapping):
        raise SccContractError(f"invalid evaluator receipt fields: {path.name}")
    expected = {
        "algorithm": "sha256",
        "canonicalization": "json-sort-keys-compact-ascii-v1",
        "payload_sha256": sha256_bytes(canonical_json_bytes(payload) + b"\n"),
    }
    if seal != expected or payload.get("receipt_type") != "fereg-f0-regression-v1":
        raise SccContractError(f"evaluator receipt seal or type mismatch: {path.name}")
    return payload


def _run(command: Sequence[str], *, cwd: Path | None = None, env: Mapping[str, str] | None = None) -> subprocess.CompletedProcess[bytes]:
    completed = subprocess.run(command, cwd=cwd, env=env, check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if completed.returncode:
        label = " ".join(command[:3])
        raise SccContractError(f"command failed ({completed.returncode}): {label}")
    return completed


def _git(repo: Path, arguments: Sequence[str]) -> bytes:
    if (
        platform.system() != "Darwin"
        or DARWIN_GIT.is_symlink()
        or not DARWIN_GIT.is_file()
        or sha256_file(DARWIN_GIT) != DARWIN_GIT_SHA256
        or DARWIN_GIT.stat().st_size != DARWIN_GIT_SIZE
    ):
        raise SccContractError("exact macOS Git controller changed")
    environment = {
        "HOME": "/private/tmp/fereg-f0-git-home", "PATH": "/usr/bin:/bin",
        "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
        "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_NO_REPLACE_OBJECTS": "1",
    }
    return _run(
        [str(DARWIN_GIT), "--no-replace-objects", "-C", str(repo), *arguments], env=environment
    ).stdout


def resolve_commit(repo: Path, value: str) -> str:
    if not COMMIT_RE.fullmatch(value):
        raise SccContractError("source commit must be a full lowercase SHA")
    observed = _git(repo, ["rev-parse", f"{value}^{{commit}}"] ).decode("ascii").strip()
    if observed != value:
        raise SccContractError("source commit did not resolve exactly")
    return observed


def git_blob(repo: Path, commit: str, relative: str) -> bytes:
    if Path(relative).is_absolute() or ".." in Path(relative).parts:
        raise SccContractError("unsafe committed path")
    return _git(repo, ["show", f"{commit}:{relative}"])


def _git_archive_embedded_commit(path: Path) -> str:
    try:
        with path.open("rb") as handle:
            completed = subprocess.run(
                [str(DARWIN_GIT), "get-tar-commit-id"],
                env={
                    "HOME": "/private/tmp/fereg-f0-git-home",
                    "PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C",
                    "TZ": "UTC", "GIT_CONFIG_NOSYSTEM": "1",
                    "GIT_CONFIG_GLOBAL": "/dev/null",
                },
                stdin=handle,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
    except OSError as error:
        raise SccContractError(f"cannot inspect source archive {path.name}: {error}") from error
    observed = completed.stdout.decode("ascii", errors="replace").strip()
    if completed.returncode != 0 or COMMIT_RE.fullmatch(observed) is None:
        raise SccContractError(f"source archive omits an exact git commit: {path.name}")
    return observed


def _archive_member_bytes(path: Path, relative: str) -> bytes:
    try:
        with tarfile.open(path, mode="r:") as archive:
            member = archive.getmember(relative)
            if not member.isfile():
                raise SccContractError(
                    f"source archive member is not a file: {relative}"
                )
            extracted = archive.extractfile(member)
            if extracted is None:
                raise SccContractError(
                    f"source archive member cannot be read: {relative}"
                )
            return extracted.read()
    except (OSError, KeyError, tarfile.TarError) as error:
        raise SccContractError(
            f"source archive omits required member {relative}"
        ) from error


def source_identity(repo: Path, commit: str, archive: bytes) -> dict[str, Any]:
    tree = _git(repo, ["rev-parse", f"{commit}^{{tree}}"] ).decode("ascii").strip()
    timestamp = _git(repo, ["show", "-s", "--format=%cs", commit]).decode("ascii").strip()
    return {
        "commit": commit,
        "tree": tree,
        "cargo_lock_sha256": sha256_bytes(git_blob(repo, commit, "Cargo.lock")),
        "references_lock_sha256": sha256_bytes(git_blob(repo, commit, "references.lock.toml")),
        "archive_sha256": sha256_bytes(archive),
        "archive_size_bytes": len(archive),
        "build_date": timestamp,
    }


def load_regression_manifest(path: Path) -> Mapping[str, Any]:
    manifest = _json(path)
    if not isinstance(manifest, Mapping) or manifest.get("schema_version") != 1 or manifest.get("manifest_id") != MANIFEST_ID:
        raise SccContractError("unsupported F0 regression manifest")
    cells = manifest.get("cells")
    if not isinstance(cells, list) or not cells:
        raise SccContractError("F0 manifest has no closed cell inventory")
    ids: list[str] = []
    required = {
        "id", "profile", "design", "n", "controls", "threads", "memory",
        "outputs", "vce", "weight", "cluster_dimensions", "solver",
        "preconditioner", "primary",
    }
    for cell in cells:
        if not isinstance(cell, Mapping) or set(cell) != required:
            raise SccContractError("F0 manifest cell schema is not closed")
        cell_id = cell["id"]
        if not isinstance(cell_id, str) or not CELL_ID_RE.fullmatch(cell_id) or cell_id in ids:
            raise SccContractError("F0 manifest cell IDs are malformed or duplicated")
        if cell["profile"] not in {"local_screen", "scc_governing"} or cell["design"] not in SUPPORTED_DESIGNS:
            raise SccContractError(f"unsupported F0 cell {cell_id}")
        if cell["profile"] == "scc_governing" and cell["n"] != 10_000_000:
            raise SccContractError(f"SCC cell scale changed: {cell_id}")
        if cell["profile"] == "local_screen" and cell["n"] not in {100_000, 1_000_000}:
            raise SccContractError(f"local cell scale changed: {cell_id}")
        if cell["threads"] not in {1, 8}:
            raise SccContractError(f"cell threads changed: {cell_id}")
        if cell["memory"] not in {"fast", "low"} or cell["outputs"] not in {"fit", "save_fe"}:
            raise SccContractError(f"SCC cell mode changed: {cell_id}")
        if cell["vce"] not in {"unadjusted", "robust", "cluster"}:
            raise SccContractError(f"SCC cell VCE changed: {cell_id}")
        if cell["weight"] not in {"unweighted", "fweight", "pweight"}:
            raise SccContractError(f"SCC cell weight changed: {cell_id}")
        dimensions = cell["cluster_dimensions"]
        if (
            isinstance(dimensions, bool)
            or not isinstance(dimensions, int)
            or dimensions < 0
            or dimensions > 5
            or (cell["vce"] == "cluster") != (dimensions > 0)
        ):
            raise SccContractError(f"SCC cell cluster dimension changed: {cell_id}")
        if cell["solver"] not in {
            "auto", "map", "cg", "it", "squarem", "lsmr", "cmg",
        } or cell["preconditioner"] not in {
            "auto", "off", "diagonal", "schwarz",
        }:
            raise SccContractError(f"SCC cell solver route changed: {cell_id}")
        ids.append(cell_id)
    performance = manifest.get("performance_rules")
    if (
        not isinstance(performance, Mapping)
        or performance.get("warm_pairs") != PAIR_COUNT
        or tuple(performance.get("pair_order", ())) != PAIR_ORDERS
        or performance.get("individual_geometric_mean_ratio_confirmation_threshold") != 1.05
        or performance.get("student_t_two_sided_95_df8") != T95_DF8
        or performance.get("reject_reproducible_slowdown_when_log_ci_lower_exceeds_zero") is not True
    ):
        raise SccContractError("SCC pair cadence differs from the frozen contract")
    return manifest


def scc_cells(manifest: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    cells = [cell for cell in manifest["cells"] if cell["profile"] == "scc_governing"]
    if not cells or len({cell["id"] for cell in cells}) != len(cells):
        raise SccContractError("SCC governing cell inventory is empty or duplicated")
    return cells


def _manifest_lifecycle(manifest: Mapping[str, Any]) -> str:
    status = manifest.get("status")
    if status not in {"candidate_unsealed", "artifacts_sealed", "frozen"}:
        raise SccContractError("F0 manifest lifecycle state is invalid")
    return str(status)


def _submission_job_names(run_id: str) -> dict[str, str]:
    """Return short, run-unique SGE names used by the submission script."""

    _validate_run_id(run_id)
    tag = hashlib.sha256(run_id.encode("ascii")).hexdigest()[:12]
    return {
        "setup": f"f0s_{tag}",
        "array": f"f0a_{tag}",
        "collector": f"f0c_{tag}",
    }


def _submitted_script_records(run_root: Path) -> dict[str, dict[str, Any]]:
    relative_by_stage = {
        "setup": "input/harness/fereg_f0_setup_job.sh",
        "array": "input/harness/fereg_f0_job.sh",
        "collector": "input/harness/fereg_f0_collector_job.sh",
    }
    records: dict[str, dict[str, Any]] = {}
    for stage, relative in relative_by_stage.items():
        path = run_root / relative
        if path.is_symlink() or not path.is_file():
            raise SccContractError(f"submitted {stage} script is absent or unsafe")
        records[stage] = {
            "path": relative,
            "sha256": sha256_file(path),
            "size_bytes": path.stat().st_size,
        }
    return records


def _expected_scheduler_client(role: str) -> dict[str, Any]:
    values = {
        "qsub": (EXPECTED_QSUB, EXPECTED_QSUB_RESOLVED, EXPECTED_QSUB_SHA256, EXPECTED_QSUB_SIZE),
        "qacct": (EXPECTED_QACCT, EXPECTED_QACCT_RESOLVED, EXPECTED_QACCT_SHA256, EXPECTED_QACCT_SIZE),
        "qstat": (EXPECTED_QSTAT, EXPECTED_QSTAT_RESOLVED, EXPECTED_QSTAT_SHA256, EXPECTED_QSTAT_SIZE),
    }
    if role not in values:
        raise SccContractError("unknown formal scheduler client role")
    path, resolved, digest, size = values[role]
    return {
        "path": str(path), "resolved_path": resolved, "sha256": digest,
        "size_bytes": size,
    }


def _verify_scheduler_client(role: str) -> dict[str, Any]:
    expected = _expected_scheduler_client(role)
    path = Path(expected["path"])
    if (
        not path.exists()
        or str(path.resolve()) != expected["resolved_path"]
        or not path.resolve().is_file()
        or sha256_file(path.resolve()) != expected["sha256"]
        or path.resolve().stat().st_size != expected["size_bytes"]
    ):
        raise SccContractError(f"exact SCC {role} executable changed")
    return expected


def _qsub_output_jobs(run_root: Path) -> tuple[dict[str, str], dict[str, dict[str, Any]]]:
    jobs: dict[str, str] = {}
    records: dict[str, dict[str, Any]] = {}
    for stage in ("setup", "array", "collector"):
        relative = f"receipts/qsub/{stage}.txt"
        path = run_root / relative
        if path.is_symlink() or not path.is_file():
            raise SccContractError(f"raw {stage} qsub output is absent or unsafe")
        try:
            raw = path.read_text(encoding="ascii")
        except UnicodeDecodeError as error:
            raise SccContractError(f"raw {stage} qsub output is not ASCII") from error
        if re.fullmatch(r"[1-9][0-9]*(?:\.[0-9.:-]+)?\n", raw) is None:
            raise SccContractError(f"raw {stage} qsub output is malformed")
        jobs[stage] = raw.split(".", 1)[0].strip()
        records[stage] = {
            "path": relative,
            "sha256": sha256_file(path),
            "size_bytes": path.stat().st_size,
        }
    if len(set(jobs.values())) != len(jobs):
        raise SccContractError("qsub returned duplicate scheduler job IDs")
    return jobs, records


def _qstat_submission_records(
    run_root: Path, jobs: Mapping[str, str], *, submitted_root: Path
) -> dict[str, dict[str, Any]]:
    names = _submission_job_names(run_root.name)
    scripts = _submitted_script_records(run_root)
    records: dict[str, dict[str, Any]] = {}
    for stage in ("setup", "array", "collector"):
        relative = f"receipts/qsub/{stage}.qstat.txt"
        path = run_root / relative
        if path.is_symlink() or not path.is_file():
            raise SccContractError(f"queue-time {stage} qstat attestation is absent")
        text = path.read_text(encoding="utf-8")
        fields: dict[str, str] = {}
        for line in text.splitlines():
            if ":" not in line or line.startswith("="):
                continue
            key, value = line.split(":", 1)
            normalized = re.sub(r"[^a-z0-9]+", "_", key.strip().lower()).strip("_")
            if normalized:
                fields[normalized] = value.strip()
        required = {
            "job_number", "submission_time", "owner", "cwd", "merge",
            "hard_resource_list", "job_name", "stdout_path_list", "env_list",
            "script_file", "project", "shell_list",
        }
        if not required <= set(fields):
            raise SccContractError(f"queue-time {stage} qstat contract is incomplete")
        expected_script = submitted_root / scripts[stage]["path"]
        if (
            fields["job_number"] != jobs[stage]
            or fields["owner"] != "johannes"
            or fields["cwd"] != str(submitted_root)
            or fields["merge"] != "y"
            or fields["job_name"] != names[stage]
            or fields["script_file"] != str(expected_script)
            or fields["project"] != "welfgr"
            or fields["env_list"] != f"RUN_ROOT={submitted_root}"
            or fields["stdout_path_list"] != f"NONE:NONE:{submitted_root / 'logs'}"
            or "mem_per_core=4G" not in fields["hard_resource_list"]
            or fields["shell_list"] != "NONE:/bin/bash"
        ):
            raise SccContractError(f"queue-time {stage} qstat contract differs from submission")
        parallel = fields.get("parallel_environment", "")
        expected_h_rt = {
            "setup": "h_rt=14400", "array": "h_rt=42300",
            "collector": "h_rt=1800",
        }[stage]
        predecessors = {
            value for key, value in fields.items()
            if key in {"jid_predecessor_list", "jid_predecessor_list_req"} and value
        }
        expected_hold = {
            "setup": None, "array": jobs["setup"], "collector": jobs["array"],
        }[stage]
        array = fields.get("job_array_tasks")
        count = len(scc_cells(load_regression_manifest(run_root / "input/manifest.json")))
        if (
            set(fields["hard_resource_list"].split(","))
            != {expected_h_rt, "mem_per_core=4G"}
            or (stage != "collector" and re.search(r"\bomp\b.*\b16\b", parallel) is None)
            or (stage == "collector" and parallel not in ("", "NONE"))
            or (stage == "setup" and array not in (None, ""))
            or (stage == "array" and array != f"1-{count}:1")
            or (expected_hold is None and predecessors)
            or (expected_hold is not None and predecessors != {expected_hold})
        ):
            raise SccContractError(f"queue-time {stage} resource/dependency contract differs")
        records[stage] = {
            "path": relative, "sha256": sha256_file(path),
            "size_bytes": path.stat().st_size,
            "fields": dict(sorted(fields.items())),
        }
    return records


def _canonical_absolute_path(value: str | Path, where: str) -> Path:
    raw = str(value)
    path = Path(raw)
    if (
        not raw
        or not path.is_absolute()
        or path.as_posix() != raw
        or any(part in {".", ".."} for part in path.parts)
    ):
        raise SccContractError(f"{where} is not a canonical absolute path")
    return path


def _submitted_root(value: str | Path, run_id: str) -> Path:
    path = _canonical_absolute_path(value, "submission root")
    if path.name != run_id:
        raise SccContractError("submission root does not end in the exact run ID")
    return path


def _validate_frozen_harness_candidate_binding(
    lifecycle: str,
    harness: Mapping[str, Any],
    candidate: Mapping[str, Any],
) -> None:
    if lifecycle != "frozen":
        return
    for field in (
        "commit", "tree", "cargo_lock_sha256", "references_lock_sha256",
        "archive_sha256", "archive_size_bytes", "build_date",
    ):
        if harness.get(field) != candidate.get(field):
            raise SccContractError(
                f"frozen F0 harness differs from candidate source: {field}"
            )


def _submission_argv(
    run_root: Path,
    jobs: Mapping[str, str],
    *,
    submitted_root: Path | None = None,
) -> dict[str, list[str]]:
    names = _submission_job_names(run_root.name)
    scripts = _submitted_script_records(run_root)
    origin = (
        _submitted_root(run_root, run_root.name)
        if submitted_root is None
        else _submitted_root(submitted_root, run_root.name)
    )
    task_count = load_regression_manifest(
        run_root / "input/manifest.json"
    )
    count = len(scc_cells(task_count))
    # `-clear` is part of the sealed request contract: neither global nor
    # per-user SGE request files may inject -V, a different shell, or resources.
    common = [
        str(EXPECTED_QSUB), "-clear", "-terse", "-P", "welfgr",
        "-S", "/bin/bash", "-j", "y", "-m", "a",
    ]

    def command(stage: str, tail: Sequence[str]) -> list[str]:
        return [
            *common,
            "-N",
            names[stage],
            "-wd",
            str(origin),
            "-o",
            str(origin / "logs"),
            *tail,
            "-v",
            f"RUN_ROOT={origin}",
            str(origin / scripts[stage]["path"]),
        ]

    return {
        "setup": command(
            "setup", ["-pe", "omp", "16", "-l", "h_rt=04:00:00", "-l", "mem_per_core=4G"]
        ),
        "array": command(
            "array", ["-pe", "omp", "16", "-l", "h_rt=11:45:00", "-l", "mem_per_core=4G", "-hold_jid", jobs["setup"], "-t", f"1-{count}"],
        ),
        "collector": command(
            "collector", ["-l", "h_rt=00:30:00", "-l", "mem_per_core=4G", "-hold_jid", jobs["array"]]
        ),
    }


def _submission_timestamp(
    run_root: Path, name: str
) -> tuple[str, dict[str, Any]]:
    relative = f"receipts/qsub/{name}.txt"
    path = run_root / relative
    if path.is_symlink() or not path.is_file():
        raise SccContractError(f"submission {name} timestamp is absent or unsafe")
    try:
        raw = path.read_text(encoding="ascii")
    except UnicodeDecodeError as error:
        raise SccContractError(f"submission {name} timestamp is not ASCII") from error
    value = raw.removesuffix("\n")
    if raw != value + "\n":
        raise SccContractError(f"submission {name} timestamp is not canonical")
    _parse_utc_timestamp(value, f"submission {name} timestamp")
    return value, {
        "path": relative,
        "sha256": sha256_file(path),
        "size_bytes": path.stat().st_size,
    }


def _submission_ledger(
    run_root: Path,
    expected_jobs: Mapping[str, str] | None = None,
    *,
    submitted_root: Path | None = None,
) -> tuple[dict[str, Any], dict[str, str]]:
    jobs, raw_qsub = _qsub_output_jobs(run_root)
    if expected_jobs is not None and dict(expected_jobs) != jobs:
        raise SccContractError("parsed raw qsub job IDs differ from submission")
    started, started_file = _submission_timestamp(run_root, "started-at-utc")
    completed, completed_file = _submission_timestamp(run_root, "completed-at-utc")
    if _parse_utc_timestamp(started, "submission start") > _parse_utc_timestamp(
        completed, "submission completion"
    ):
        raise SccContractError("submission timestamps are out of order")
    origin = (
        _submitted_root(run_root, run_root.name)
        if submitted_root is None
        else _submitted_root(submitted_root, run_root.name)
    )
    ledger = {
        "record_type": "fereg-f0-sge-submission-ledger-v1",
        "submit_cwd": str(origin),
        "started_at_utc": started,
        "completed_at_utc": completed,
        "timestamp_files": {
            "started": started_file,
            "completed": completed_file,
        },
        "raw_qsub": raw_qsub,
        "argv": _submission_argv(run_root, jobs, submitted_root=origin),
        "qsub_client": _expected_scheduler_client("qsub"),
        "qstat_client": _expected_scheduler_client("qstat"),
        "qstat": _qstat_submission_records(run_root, jobs, submitted_root=origin),
    }
    return ledger, jobs


def _parse_utc_timestamp(value: Any, where: str) -> datetime:
    if not isinstance(value, str) or not value.endswith("Z"):
        raise SccContractError(f"{where} is not a UTC timestamp")
    try:
        parsed = datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError as error:
        raise SccContractError(f"{where} is not a UTC timestamp") from error
    if parsed.tzinfo != timezone.utc:
        raise SccContractError(f"{where} is not a UTC timestamp")
    return parsed


def _seal_source_file(root: Path, relative: str, where: str) -> Path:
    if not isinstance(relative, str) or not relative or Path(relative).is_absolute():
        raise SccContractError(f"{where} path must be relative to the seal root")
    resolved_root = root.resolve()
    source = root / relative
    try:
        resolved = source.resolve(strict=True)
        resolved.relative_to(resolved_root)
    except (OSError, ValueError) as error:
        raise SccContractError(f"{where} escapes or is absent from the seal root") from error
    current = root
    for component in Path(relative).parts:
        current = current / component
        if current.is_symlink():
            raise SccContractError(f"{where} contains a symlink")
    if not resolved.is_file():
        raise SccContractError(f"{where} is not a regular file")
    return resolved


def _materialize_seal_evidence(
    manifest: Mapping[str, Any],
    source_root: Path,
    destination_root: Path,
) -> Mapping[str, Any]:
    """Copy only the external bytes named by the manifest's seal closure."""

    records: dict[str, dict[str, Any]] = {}

    def copy_file(relative: str, where: str, binding: Mapping[str, Any] | None = None) -> None:
        source = _seal_source_file(source_root, relative, where)
        content = source.read_bytes()
        observed = {
            "path": relative,
            "sha256": sha256_bytes(content),
            "size_bytes": len(content),
        }
        if binding is not None and (
            binding.get("path") != relative
            or binding.get("sha256") != observed["sha256"]
            or binding.get("size_bytes") != observed["size_bytes"]
        ):
            raise SccContractError(f"{where} differs from its manifest binding")
        previous = records.get(relative)
        if previous is not None:
            if previous != observed:
                raise SccContractError(f"{where} has conflicting byte identities")
            return
        _write_exclusive(destination_root / relative, content)
        records[relative] = observed

    def copy_binding(value: Any, where: str) -> Mapping[str, Any]:
        binding = _expect_keys(
            value,
            ({"path", "sha256", "size_bytes", "payload_sha256"}
             if isinstance(value, Mapping) and "payload_sha256" in value
             else {"path", "sha256", "size_bytes"}),
            where,
        )
        copy_file(str(binding["path"]), where, binding)
        return binding

    def copy_directory(relative: Any, where: str) -> None:
        if not isinstance(relative, str) or not relative or Path(relative).is_absolute():
            raise SccContractError(f"{where} path must be relative to the seal root")
        source = source_root / relative
        try:
            resolved = source.resolve(strict=True)
            resolved.relative_to(source_root.resolve())
        except (OSError, ValueError) as error:
            raise SccContractError(f"{where} escapes or is absent from the seal root") from error
        if source.is_symlink() or not resolved.is_dir():
            raise SccContractError(f"{where} is not a regular directory")
        files = sorted(path for path in resolved.rglob("*") if path.is_file())
        if not files:
            raise SccContractError(f"{where} is empty")
        for path in resolved.rglob("*"):
            if path.is_symlink():
                raise SccContractError(f"{where} contains a symlink")
        for path in files:
            child = (Path(relative) / path.relative_to(resolved)).as_posix()
            copy_file(child, where)

    sealing = _expect_keys(
        manifest.get("sealing"),
        {
            "state", "observed_local_evidence", "missing_required_evidence",
            "artifact_seal", "qualification_seal", "controller_source",
            "freeze_rule",
        },
        "F0 sealing manifest",
    )
    observed = sealing["observed_local_evidence"]
    if not isinstance(observed, list) or len(observed) != 1:
        raise SccContractError("F0 observed local evidence inventory changed")
    local_relative = str(observed[0].get("receipt_path"))
    copy_file(local_relative, "observed local F0 receipt")
    if records[local_relative]["sha256"] != observed[0].get("receipt_sha256"):
        raise SccContractError("observed local F0 receipt differs from manifest")

    artifact_binding = copy_binding(sealing["artifact_seal"], "F0 artifact seal")
    artifact_document = _json(
        _seal_source_file(source_root, str(artifact_binding["path"]), "F0 artifact seal")
    )
    artifact_envelope = _expect_keys(
        artifact_document, {"payload", "seal"}, "F0 artifact seal envelope"
    )
    artifact_payload = artifact_envelope["payload"]
    if not isinstance(artifact_payload, Mapping):
        raise SccContractError("F0 artifact seal payload is invalid")
    evidence = _expect_keys(
        artifact_payload.get("evidence"),
        {"local_full_receipt", "direct_behavior", "macos", "linux", "windows"},
        "F0 artifact seal evidence",
    )
    copy_binding(evidence["local_full_receipt"], "sealed local F0 receipt")
    behavior = _expect_keys(
        evidence["direct_behavior"],
        {"observation", "bundle", "verified_observation"},
        "sealed direct behavior evidence",
    )
    copy_binding(behavior["observation"], "sealed behavior observation")
    copy_binding(behavior["bundle"], "sealed behavior bundle")
    for platform_name in ("macos", "windows"):
        portable = evidence[platform_name]
        if not isinstance(portable, Mapping):
            raise SccContractError(f"sealed {platform_name} evidence is invalid")
        for field, value in portable.items():
            if field != "verified_receipt":
                copy_binding(value, f"sealed {platform_name} {field}")
        if platform_name == "macos":
            attestation = portable.get("native_attestation")
            if not isinstance(attestation, Mapping):
                raise SccContractError("sealed macOS attestation is absent")
            relative = str(attestation.get("path"))
            for suffix in (".status", ".stata.log", ".wrapper.do", ".invocation.json"):
                copy_file(relative + suffix, f"sealed macOS attestation sidecar {suffix}")
    linux = _expect_keys(
        evidence["linux"], {"run_root", "verified_close"}, "sealed Linux evidence"
    )
    copy_directory(linux["run_root"], "sealed Linux platform run")

    if manifest.get("status") == "frozen":
        qualification_binding = copy_binding(
            sealing["qualification_seal"], "F0 qualification seal"
        )
        qualification_document = _json(
            _seal_source_file(
                source_root,
                str(qualification_binding["path"]),
                "F0 qualification seal",
            )
        )
        qualification_envelope = _expect_keys(
            qualification_document,
            {"payload", "seal"},
            "F0 qualification seal envelope",
        )
        qualification_payload = qualification_envelope["payload"]
        if not isinstance(qualification_payload, Mapping):
            raise SccContractError("F0 qualification seal payload is invalid")
        copy_binding(
            qualification_payload.get("diagnostic_manifest"),
            "artifact-sealed diagnostic manifest",
        )
        copy_directory(
            qualification_payload.get("diagnostic_run_root"),
            "F0 diagnostic run",
        )
    elif sealing["qualification_seal"] is not None:
        raise SccContractError("artifact-sealed manifest has a qualification seal")

    return {
        "root": "seal-evidence",
        "files": [records[path] for path in sorted(records)],
    }


def _verify_copied_seal_evidence(
    manifest_path: Path,
    harness_archive: Path,
    seal_root: Path,
) -> None:
    """Run the authoritative manifest validator from the exact harness archive."""

    with tempfile.TemporaryDirectory(prefix="fereg-f0-seal-verify-") as directory:
        exact_root = Path(directory) / "source"
        _safe_extract_tar(harness_archive, exact_root)
        evaluator = exact_root / "scripts/fereg_f0_regression.py"
        if not evaluator.is_file() or evaluator.is_symlink():
            raise SccContractError("exact harness omits the F0 seal validator")
        spec = importlib.util.spec_from_file_location(
            f"fereg_f0_seal_validator_{sha256_file(evaluator)[:12]}",
            evaluator,
        )
        if spec is None or spec.loader is None:
            raise SccContractError("exact F0 seal validator cannot be loaded")
        module = importlib.util.module_from_spec(spec)
        root_text = str(exact_root)
        sys.path.insert(0, root_text)
        try:
            spec.loader.exec_module(module)
            module.load_manifest(
                manifest_path,
                verify_git_object=False,
                root=exact_root,
                evidence_root=seal_root,
            )
        except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
            raise SccContractError(
                f"copied F0 seal evidence fails the exact harness validator: {error}"
            ) from error
        finally:
            sys.path.remove(root_text)


def validate_submission_binding(
    run_root: Path,
    preparation: Mapping[str, Any],
) -> Mapping[str, Any]:
    submission = read_sealed(
        run_root / "receipts/submission.json", "fereg-f0-scc-submission-v1"
    )
    _expect_keys(
        submission,
        {
            "schema_version",
            "receipt_type",
            "run_id",
            "owner",
            "project",
            "manifest",
            "preparation_receipt_sha256",
            "jobs",
            "job_names",
            "scripts",
            "array_tasks",
            "resources",
            "submission_ledger",
        },
        "submission receipt",
    )
    if (
        submission.get("schema_version") != RECEIPT_SCHEMA
        or submission.get("owner") != "johannes"
        or submission.get("project") != "welfgr"
        or submission.get("run_id") != run_root.name
        or submission.get("manifest") != preparation.get("manifest")
        or submission.get("preparation_receipt_sha256")
        != sha256_file(run_root / "input/preparation.json")
        or submission.get("array_tasks") != preparation.get("tasks", {}).get("count")
    ):
        raise SccContractError("submission receipt is not bound to preparation and manifest")
    jobs = submission.get("jobs")
    if not isinstance(jobs, Mapping) or set(jobs) != {"setup", "array", "collector"}:
        raise SccContractError("submission receipt job inventory is not closed")
    for value in jobs.values():
        if not isinstance(value, str) or not JOB_ID_RE.fullmatch(value):
            raise SccContractError("submission receipt contains a malformed job ID")
    if len(set(jobs.values())) != len(jobs):
        raise SccContractError("submission stages do not have distinct scheduler jobs")
    if submission.get("job_names") != _submission_job_names(run_root.name):
        raise SccContractError("submission job names are not unique to the exact run")
    if submission.get("scripts") != _submitted_script_records(run_root):
        raise SccContractError("submission script identities differ from deployed bytes")
    recorded_ledger = _expect_keys(
        submission.get("submission_ledger"),
        {
            "record_type", "submit_cwd", "started_at_utc", "completed_at_utc",
            "timestamp_files", "raw_qsub", "argv",
            "qsub_client",
            "qstat_client", "qstat",
        },
        "submission ledger",
    )
    origin = _submitted_root(recorded_ledger["submit_cwd"], run_root.name)
    ledger, ledger_jobs = _submission_ledger(
        run_root, jobs, submitted_root=origin
    )
    if submission.get("submission_ledger") != ledger or ledger_jobs != jobs:
        raise SccContractError(
            "submission ledger differs from raw qsub output or exact argv"
        )
    expected_resources = {
        "setup": {"slots": 16, "mem_per_core": "4G", "h_rt": "04:00:00"},
        "array": {"slots": 16, "mem_per_core": "4G", "h_rt": "11:45:00"},
        "collector": {"slots": 1, "mem_per_core": "4G", "h_rt": "00:30:00"},
    }
    if submission.get("resources") != expected_resources:
        raise SccContractError("submission resources differ from the frozen SCC contract")
    return submission


def _validate_run_id(run_id: str) -> None:
    if not RUN_ID_RE.fullmatch(run_id):
        raise SccContractError("run ID must be a sortable UTC ID followed by a safe slug")


def prepare_run(
    *,
    run_root: Path,
    baseline_repo: Path,
    baseline_commit: str,
    candidate_repo: Path,
    candidate_commit: str,
    harness_repo: Path,
    harness_commit: str,
    manifest_path: Path,
    seal_root: Path | None = None,
    behavior_baseline: Path | None = None,
    behavior_baseline_bundle: Path | None = None,
    behavior_candidate: Path | None = None,
    behavior_candidate_bundle: Path | None = None,
    behavior_comparison: Path | None = None,
) -> None:
    _validate_run_id(run_root.name)
    if run_root.exists():
        raise SccContractError("run root already exists; retries require a new run ID")
    baseline_commit = resolve_commit(baseline_repo, baseline_commit)
    candidate_commit = resolve_commit(candidate_repo, candidate_commit)
    harness_commit = resolve_commit(harness_repo, harness_commit)
    manifest_relative = manifest_path.resolve().relative_to(harness_repo.resolve()).as_posix()
    manifest_bytes = git_blob(harness_repo, harness_commit, manifest_relative)
    if manifest_path.read_bytes() != manifest_bytes:
        raise SccContractError("manifest bytes do not match the exact harness commit")
    manifest = load_regression_manifest(manifest_path)
    lifecycle = _manifest_lifecycle(manifest)
    baseline_declared = manifest.get("baseline_candidate", {}).get("source")
    if not isinstance(baseline_declared, Mapping):
        raise SccContractError("manifest lacks the F0 source identity")

    directories = ("input/archives", "input/harness", "logs", "work", "output/cells", "receipts/tasks", "receipts/qacct")
    for relative in directories:
        (run_root / relative).mkdir(parents=True, exist_ok=False if relative == "input/archives" else True)
    _write_exclusive(run_root / "input/manifest.json", manifest_bytes)
    manifest_sha = sha256_bytes(manifest_bytes)

    arms: dict[str, Any] = {}
    for arm, repo, commit in (
        ("baseline", baseline_repo, baseline_commit),
        ("candidate", candidate_repo, candidate_commit),
    ):
        archive = _git(repo, ["archive", "--format=tar", commit])
        archive_path = run_root / f"input/archives/{arm}.tar"
        _write_exclusive(archive_path, archive)
        arms[arm] = source_identity(repo, commit, archive)
        arms[arm]["archive"] = f"archives/{arm}.tar"
    for key in ("commit", "tree", "cargo_lock_sha256", "references_lock_sha256"):
        if baseline_declared.get(key) != arms["baseline"][key]:
            raise SccContractError(f"prepared F0 identity contradicts manifest field {key}")

    harness_archive = _git(
        harness_repo, ["archive", "--format=tar", harness_commit]
    )
    harness_archive_path = run_root / "input/archives/harness.tar"
    _write_exclusive(harness_archive_path, harness_archive)
    harness_source = source_identity(
        harness_repo, harness_commit, harness_archive
    )
    harness_source["archive"] = "archives/harness.tar"

    seal_evidence: Mapping[str, Any] | None = None
    if lifecycle != "candidate_unsealed":
        if seal_root is None:
            raise SccContractError(
                "sealed F0 preparation requires an explicit external seal root"
            )
        seal_evidence = _materialize_seal_evidence(
            manifest,
            seal_root.resolve(),
            run_root / "input/seal-evidence",
        )
    elif seal_root is not None:
        raise SccContractError(
            "candidate_unsealed preparation must not claim a seal evidence root"
        )

    harness: dict[str, Any] = {}
    for relative in HARNESS_PATHS:
        content = git_blob(harness_repo, harness_commit, relative)
        destination = run_root / "input/harness" / Path(relative).name
        mode = 0o555 if destination.suffix in {".py", ".sh"} else 0o444
        _write_exclusive(destination, content, mode)
        harness[destination.name] = {"sha256": sha256_bytes(content), "size_bytes": len(content)}

    if seal_evidence is not None:
        _verify_copied_seal_evidence(
            run_root / "input/manifest.json",
            harness_archive_path,
            run_root / "input/seal-evidence",
        )

    behavior_arguments = {
        "baseline_observation": (
            behavior_baseline,
            "behavior/baseline/observation.json",
        ),
        "baseline_bundle": (
            behavior_baseline_bundle,
            None,
        ),
        "candidate_observation": (
            behavior_candidate,
            "behavior/candidate/observation.json",
        ),
        "candidate_bundle": (
            behavior_candidate_bundle,
            None,
        ),
        "comparison": (
            behavior_comparison,
            "behavior/comparison.json",
        ),
    }
    provided_behavior = [item[0] is not None for item in behavior_arguments.values()]
    if any(provided_behavior) and not all(provided_behavior):
        raise SccContractError("behavior evidence inputs must be supplied together")
    if lifecycle == "frozen" and not all(provided_behavior):
        raise SccContractError(
            "final frozen SCC runs require both behavior receipts/bundles and comparison"
        )
    if lifecycle != "frozen" and any(provided_behavior):
        raise SccContractError(
            "only the final frozen run may carry candidate behavior evidence"
        )
    behavior: dict[str, Any] | None = None
    if all(provided_behavior):
        behavior = {}
        for name, (source_value, fixed_destination) in behavior_arguments.items():
            assert source_value is not None
            source = source_value.resolve()
            if not source.is_file() or source.is_symlink():
                raise SccContractError(f"behavior evidence is not a regular file: {name}")
            content = source.read_bytes()
            if name not in {"baseline_bundle", "candidate_bundle"}:
                try:
                    document = json.loads(content)
                except json.JSONDecodeError as error:
                    raise SccContractError(f"behavior evidence is not JSON: {name}") from error
                if not isinstance(document, Mapping) or set(document) != {"payload", "seal"}:
                    raise SccContractError(f"behavior evidence envelope is invalid: {name}")
            if fixed_destination is None:
                arm = name.removesuffix("_bundle")
                relative = f"behavior/{arm}/{source.name}"
            else:
                relative = fixed_destination
            destination = run_root / "input" / relative
            _write_exclusive(destination, content)
            behavior[name] = {
                "path": relative,
                "sha256": sha256_bytes(content),
                "size_bytes": len(content),
            }

    task_lines = ["task_id\tcell_id"]
    governing_cells = scc_cells(manifest)
    for index, cell in enumerate(governing_cells, 1):
        task_lines.append(f"{index}\t{cell['id']}")
    tasks = ("\n".join(task_lines) + "\n").encode("ascii")
    _write_exclusive(run_root / "input/tasks.tsv", tasks)
    payload = {
        "schema_version": RECEIPT_SCHEMA,
        "receipt_type": "fereg-f0-scc-preparation-v1",
        "run_id": run_root.name,
        "project": "welfgr",
        "manifest": {"id": manifest["manifest_id"], "sha256": manifest_sha},
        "harness_manifest_path": manifest_relative,
        "tasks": {"count": len(governing_cells), "sha256": sha256_bytes(tasks)},
        "arms": arms,
        "harness_source": harness_source,
        "harness": dict(sorted(harness.items())),
        "seal_evidence": seal_evidence,
        "behavior": behavior,
    }
    write_sealed(run_root / "input/preparation.json", payload)
    _write_exclusive(run_root / "input/PREPARED", f"FEREG_F0_PREPARED manifest_sha256={manifest_sha}\n".encode("ascii"))


def verify_input(run_root: Path) -> None:
    preparation_path = run_root / "input/preparation.json"
    preparation = read_sealed(preparation_path, "fereg-f0-scc-preparation-v1")
    _expect_keys(
        preparation,
        {
            "schema_version",
            "receipt_type",
            "run_id",
            "project",
            "manifest",
            "harness_manifest_path",
            "tasks",
            "arms",
            "harness_source",
            "harness",
            "seal_evidence",
            "behavior",
        },
        "preparation receipt",
    )
    if (
        preparation.get("schema_version") != RECEIPT_SCHEMA
        or preparation.get("project") != "welfgr"
        or preparation.get("run_id") != run_root.name
    ):
        raise SccContractError("prepared run ID does not match its directory")
    manifest_path = run_root / "input/manifest.json"
    tasks_path = run_root / "input/tasks.tsv"
    manifest = load_regression_manifest(manifest_path)
    lifecycle = _manifest_lifecycle(manifest)
    harness_manifest_path = preparation["harness_manifest_path"]
    if (
        not isinstance(harness_manifest_path, str)
        or not harness_manifest_path.startswith("benchmarks/manifests/")
        or Path(harness_manifest_path).is_absolute()
        or ".." in Path(harness_manifest_path).parts
        or Path(harness_manifest_path).suffix != ".json"
    ):
        raise SccContractError("prepared harness manifest path is invalid")
    if preparation.get("manifest") != {
        "id": manifest["manifest_id"],
        "sha256": sha256_file(manifest_path),
    }:
        raise SccContractError("prepared manifest identity is stale")
    if sha256_file(manifest_path) != preparation["manifest"]["sha256"]:
        raise SccContractError("prepared manifest hash mismatch")
    if sha256_file(tasks_path) != preparation["tasks"]["sha256"]:
        raise SccContractError("prepared task manifest hash mismatch")
    _cell_for_task(manifest, tasks_path, 1)
    for arm in ("baseline", "candidate"):
        arms = _expect_keys(
            preparation.get("arms"), {"baseline", "candidate"}, "prepared arms"
        )
        identity = _expect_keys(
            arms[arm],
            {
                "commit",
                "tree",
                "cargo_lock_sha256",
                "references_lock_sha256",
                "archive_sha256",
                "archive_size_bytes",
                "build_date",
                "archive",
            },
            f"prepared {arm} source identity",
        )
        if (
            COMMIT_RE.fullmatch(str(identity["commit"])) is None
            or COMMIT_RE.fullmatch(str(identity["tree"])) is None
            or SHA256_RE.fullmatch(str(identity["cargo_lock_sha256"])) is None
            or SHA256_RE.fullmatch(str(identity["references_lock_sha256"])) is None
            or SHA256_RE.fullmatch(str(identity["archive_sha256"])) is None
            or not isinstance(identity["archive_size_bytes"], int)
            or identity["archive_size_bytes"] < 1
            or identity["archive"] != f"archives/{arm}.tar"
        ):
            raise SccContractError(f"prepared {arm} source identity is malformed")
        archive = run_root / "input" / identity["archive"]
        if (
            archive.is_symlink() or not archive.is_file()
            or sha256_file(archive) != identity["archive_sha256"]
            or archive.stat().st_size != identity["archive_size_bytes"]
        ):
            raise SccContractError(f"prepared {arm} archive hash mismatch")
    harness_source = _expect_keys(
        preparation.get("harness_source"),
        {
            "commit", "tree", "cargo_lock_sha256", "references_lock_sha256",
            "archive_sha256", "archive_size_bytes", "build_date", "archive",
        },
        "prepared harness source identity",
    )
    if (
        COMMIT_RE.fullmatch(str(harness_source["commit"])) is None
        or COMMIT_RE.fullmatch(str(harness_source["tree"])) is None
        or SHA256_RE.fullmatch(str(harness_source["cargo_lock_sha256"])) is None
        or SHA256_RE.fullmatch(str(harness_source["references_lock_sha256"])) is None
        or SHA256_RE.fullmatch(str(harness_source["archive_sha256"])) is None
        or not isinstance(harness_source["archive_size_bytes"], int)
        or harness_source["archive_size_bytes"] < 1
        or harness_source["archive"] != "archives/harness.tar"
    ):
        raise SccContractError("prepared harness source identity is malformed")
    _validate_frozen_harness_candidate_binding(
        lifecycle, harness_source, preparation["arms"]["candidate"]
    )
    harness_archive = run_root / "input/archives/harness.tar"
    if (
        harness_archive.is_symlink()
        or not harness_archive.is_file()
        or sha256_file(harness_archive) != harness_source["archive_sha256"]
        or harness_archive.stat().st_size != harness_source["archive_size_bytes"]
    ):
        raise SccContractError("prepared harness archive hash mismatch")
    archive_entries = list((run_root / "input/archives").iterdir())
    if any(path.is_symlink() or not path.is_file() for path in archive_entries):
        raise SccContractError("prepared source archive inventory contains an unsafe entry")
    actual_archives = {path.name for path in archive_entries}
    if actual_archives != {"baseline.tar", "candidate.tar", "harness.tar"}:
        raise SccContractError("prepared source archive inventory is not closed")
    harness_dir = run_root / "input/harness"
    expected_harness = set(preparation["harness"])
    actual_harness = {path.name for path in harness_dir.iterdir() if path.is_file()}
    if actual_harness != expected_harness:
        raise SccContractError("prepared harness inventory is not closed")
    for name, identity in preparation["harness"].items():
        path = harness_dir / name
        if sha256_file(path) != identity["sha256"] or path.stat().st_size != identity["size_bytes"]:
            raise SccContractError(f"prepared harness file changed: {name}")
    seal_evidence = preparation["seal_evidence"]
    seal_root = run_root / "input/seal-evidence"
    if lifecycle == "candidate_unsealed":
        if seal_evidence is not None or seal_root.exists():
            raise SccContractError(
                "candidate_unsealed input contains a seal evidence closure"
            )
    else:
        seal_evidence = _expect_keys(
            seal_evidence, {"root", "files"}, "prepared seal evidence"
        )
        if seal_evidence["root"] != "seal-evidence":
            raise SccContractError("prepared seal evidence root changed")
        files = seal_evidence["files"]
        if not isinstance(files, list) or not files:
            raise SccContractError("prepared seal evidence inventory is empty")
        expected_paths: set[str] = set()
        for index, record in enumerate(files):
            record = _expect_keys(
                record,
                {"path", "sha256", "size_bytes"},
                f"prepared seal evidence file {index}",
            )
            relative = record["path"]
            if (
                not isinstance(relative, str)
                or not relative
                or relative in expected_paths
                or Path(relative).is_absolute()
            ):
                raise SccContractError("prepared seal evidence path is invalid")
            expected_paths.add(relative)
            path = _seal_source_file(
                seal_root, relative, "prepared seal evidence file"
            )
            if (
                sha256_file(path) != record["sha256"]
                or path.stat().st_size != record["size_bytes"]
            ):
                raise SccContractError("prepared seal evidence bytes changed")
        actual_paths = {
            path.relative_to(seal_root).as_posix()
            for path in seal_root.rglob("*")
            if path.is_file()
        }
        if actual_paths != expected_paths:
            raise SccContractError("prepared seal evidence inventory is not closed")
    behavior = preparation["behavior"]
    if behavior is None:
        if lifecycle == "frozen":
            raise SccContractError("final frozen input omits behavior evidence")
    else:
        if lifecycle != "frozen":
            raise SccContractError(
                "nonfinal input contains candidate behavior evidence"
            )
        behavior = _expect_keys(
            behavior,
            {
                "baseline_observation", "baseline_bundle",
                "candidate_observation", "candidate_bundle", "comparison",
            },
            "prepared behavior evidence",
        )
        expected_behavior_files = {
            str(record["path"]).removeprefix("behavior/")
            for record in behavior.values()
        }
        behavior_root = run_root / "input/behavior"
        actual_behavior_files = {
            path.relative_to(behavior_root).as_posix()
            for path in behavior_root.rglob("*")
            if path.is_file()
        }
        if actual_behavior_files != expected_behavior_files:
            raise SccContractError("prepared behavior evidence inventory is not closed")
        for name, raw_record in behavior.items():
            record = _expect_keys(
                raw_record,
                {"path", "sha256", "size_bytes"},
                f"prepared behavior evidence {name}",
            )
            expected_prefix = (
                "behavior/baseline/"
                if name.startswith("baseline_")
                else "behavior/candidate/"
                if name.startswith("candidate_")
                else "behavior/"
            )
            if (
                not isinstance(record["path"], str)
                or not record["path"].startswith(expected_prefix)
                or Path(record["path"]).is_absolute()
                or ".." in Path(record["path"]).parts
            ):
                raise SccContractError("prepared behavior evidence path is stale")
            path = run_root / "input" / record["path"]
            if (
                path.is_symlink()
                or not path.is_file()
                or path.stat().st_size != record["size_bytes"]
                or sha256_file(path) != record["sha256"]
            ):
                raise SccContractError(f"prepared behavior evidence changed: {name}")
    prepared = run_root / "input/PREPARED"
    expected = f"FEREG_F0_PREPARED manifest_sha256={preparation['manifest']['sha256']}\n"
    if not prepared.is_file() or prepared.read_text(encoding="ascii") != expected:
        raise SccContractError("prepared terminal sentinel is missing or malformed")


def _safe_extract_tar(path: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=False)
    with tarfile.open(path, mode="r:") as archive:
        members = archive.getmembers()
        for member in members:
            relative = Path(member.name)
            if relative.is_absolute() or ".." in relative.parts or not (member.isfile() or member.isdir()):
                raise SccContractError("source archive contains an unsafe entry")
        archive.extractall(destination)


def _verify_extracted_archive_files(
    archive_path: Path,
    extracted_root: Path,
    *,
    allow_formal_dist: bool = False,
    containment_root: Path | None = None,
    read_only: bool = False,
) -> None:
    if extracted_root.is_symlink() or not extracted_root.is_dir():
        raise SccContractError("extracted source root is not a retained real directory")
    if containment_root is not None:
        boundary = containment_root.resolve()
        current = extracted_root
        while True:
            if current.is_symlink():
                raise SccContractError("extracted source parent contains a symlink")
            if current == boundary:
                break
            if current.parent == current or boundary not in current.parents:
                raise SccContractError("extracted source root escapes the retained run")
            current = current.parent
    expected: dict[str, tuple[str, int]] = {}
    with tarfile.open(archive_path, mode="r:") as archive:
        for member in archive.getmembers():
            relative = Path(member.name)
            if relative.is_absolute() or ".." in relative.parts or member.issym() or member.islnk():
                raise SccContractError("source archive file inventory is unsafe")
            if member.isfile():
                handle = archive.extractfile(member)
                if handle is None:
                    raise SccContractError("source archive member cannot be read")
                mode = member.mode & 0o777
                if read_only:
                    mode = 0o555 if mode & 0o111 else 0o444
                expected[relative.as_posix()] = (sha256_bytes(handle.read()), mode)
    actual: dict[str, tuple[str, int]] = {}
    directories: set[str] = set()
    generated: dict[str, tuple[str, int]] = {}
    for path in sorted(extracted_root.rglob("*")):
        relative = path.relative_to(extracted_root).as_posix()
        if path.is_symlink():
            raise SccContractError("extracted source tree contains a symlink")
        if path.is_dir():
            directories.add(relative)
        elif not path.is_file():
            raise SccContractError("extracted source tree contains an unsafe file type")
        elif relative.startswith("dist/"):
            generated[relative] = (sha256_file(path), path.stat().st_mode & 0o777)
        else:
            actual[relative] = (sha256_file(path), path.stat().st_mode & 0o777)
    if actual != expected:
        raise SccContractError("extracted source file inventory differs from its exact archive")
    archive_directories = {
        parent.as_posix()
        for name in expected
        for parent in Path(name).parents
        if parent.as_posix() != "."
    }
    allowed_directories = archive_directories | ({"dist"} if allow_formal_dist else set())
    if directories != allowed_directories:
        raise SccContractError("extracted source directory inventory changed")
    if allow_formal_dist:
        if set(generated) != {"dist/fereg_linux.plugin", "dist/fereg_linux.plugin.sha256"}:
            raise SccContractError("formal dist inventory changed")
        plugin = extracted_root / "dist/fereg_linux.plugin"
        digest = extracted_root / "dist/fereg_linux.plugin.sha256"
        expected_plugin_mode = 0o555 if read_only else 0o755
        expected_digest_mode = 0o444 if read_only else 0o644
        if generated["dist/fereg_linux.plugin"][1] != expected_plugin_mode or generated["dist/fereg_linux.plugin.sha256"][1] != expected_digest_mode:
            raise SccContractError("formal dist file modes changed")
        line = digest.read_text(encoding="ascii", errors="strict")
        if (
            not line.endswith("/dist/fereg_linux.plugin\n")
            or line.split(None, 1)[0] != sha256_file(plugin)
        ):
            raise SccContractError("formal dist digest file changed")
    elif generated:
        raise SccContractError("source archive invented a dist tree")


def _tool_version(command: Sequence[str]) -> str:
    completed = _run(command)
    text = (completed.stdout or completed.stderr).decode("utf-8", errors="replace").splitlines()
    if not text:
        raise SccContractError(f"tool did not report a version: {command[0]}")
    return text[0].strip()


def _stata_executable() -> str:
    if (
        not SCC_STATA.is_file() or SCC_STATA.is_symlink()
        or sha256_file(SCC_STATA) != SCC_STATA_SHA256
        or SCC_STATA.stat().st_size != SCC_STATA_SIZE
    ):
        raise SccContractError("exact SCC Stata/MP executable changed")
    return str(SCC_STATA)


def _stata_environment(run_dir: Path, *, create: bool = True) -> dict[str, str]:
    home = run_dir / "controller-home"
    temporary = run_dir / "controller-tmp"
    if create:
        home.mkdir(exist_ok=True)
        temporary.mkdir(exist_ok=True)
    elif not home.is_dir() or not temporary.is_dir():
        raise SccContractError("retained Stata isolation directories are missing")
    return {
        "HOME": str(home.resolve()), "TMPDIR": str(temporary.resolve()),
        "PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
    }


def _closed_stata_runner(environment: Mapping[str, str]):
    def run(command: Sequence[str], *, cwd: Path, **_: Any) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryFile(mode="w+b") as output:
            process = subprocess.Popen(
                list(command), cwd=cwd, env=dict(environment), stdout=output,
                stderr=subprocess.STDOUT,
            )
            if not hasattr(os, "wait4"):
                process.kill()
                process.wait()
                raise SccContractError("SCC Stata RSS supervisor requires wait4")
            _, status, usage = os.wait4(process.pid, 0)
            process.returncode = os.waitstatus_to_exitcode(status)
            output.seek(0)
            stdout = output.read().decode("utf-8", errors="replace")
        completed = subprocess.CompletedProcess(
            list(command), process.returncode, stdout=stdout, stderr=None
        )
        completed.fereg_peak_rss = {
            "value": int(usage.ru_maxrss) * 1024,
            "unit": "bytes", "method": "os_wait4_ru_maxrss",
        }
        return completed
    return run


def _render_dataset_wrapper(
    generator: Path,
    arguments: Sequence[Any],
    destination: Path,
    dataset_id: str,
    variant: str,
) -> str:
    quoted_generator = str(generator).replace("\\", "/")
    quoted_destination = str(destination).replace("\\", "/")
    args = " ".join(str(value) for value in arguments)
    post_generation: list[str] = []
    if variant == "qualified_dense":
        post_generation = [
            "generate byte __f0_fweight = 1 + mod(obs_id, 5)",
            "generate byte __f0_cluster1 = 1 + mod(worker, 17)",
            "generate byte __f0_cluster2 = 1 + mod(firm, 13)",
            "generate byte __f0_cluster3 = 1 + mod(year - 2000, 10)",
            "generate byte __f0_cluster4 = 1 + mod(worker + firm, 7)",
            "generate byte __f0_cluster5 = 1 + mod(3*worker + 5*firm + year, 5)",
        ]
    elif variant == "disconnected":
        post_generation = [
            "generate byte __f0_component = mod(obs_id, 2)",
            "recast long worker firm",
            "replace worker = 2*worker + __f0_component",
            "replace firm = 2*firm + __f0_component",
            "drop __f0_component",
        ]
    elif variant not in {"dense", "sparse"}:
        raise SccContractError(f"unknown SCC dataset variant: {variant}")
    post_generation.extend(("compress", "isid obs_id"))
    return "\n".join([
        "version 19.0", "clear all", "set more off",
        f'capture noisily do "{quoted_generator}" {args} "{quoted_destination}"',
        "local rc = _rc", "if `rc' != 0 exit `rc'",
        f'capture confirm file "{quoted_destination}"', "if _rc exit _rc",
        f'quietly use "{quoted_destination}", clear',
        "capture confirm variable obs_id", "if !_rc exit 110",
        "generate long obs_id = _n", "isid obs_id", "order obs_id",
        *post_generation, f'quietly save "{quoted_destination}", replace',
        f'display as result "FEREG_F0_DATASET_SUCCESS|{dataset_id}"',
        "exit 0", "",
    ])


def _generate_dataset(
    stata: str,
    generator: Path,
    arguments: Sequence[Any],
    destination: Path,
    log_dir: Path,
    dataset_id: str,
    *,
    variant: str,
) -> Mapping[str, Any]:
    run_dir = log_dir / f"generate-{dataset_id}"
    run_dir.mkdir(parents=True, exist_ok=False)
    wrapper = run_dir / "generate.do"
    wrapper.write_text(_render_dataset_wrapper(generator, arguments, destination, dataset_id, variant), encoding="utf-8")
    environment = _stata_environment(run_dir)
    argv = [stata, "-b", "do", str(wrapper)]
    completed = subprocess.run(argv, cwd=run_dir, env=environment, check=False, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    (run_dir / "runner.stdout").write_text(completed.stdout, encoding="utf-8")
    log = run_dir / "generate.log"
    log_text = log.read_text(encoding="utf-8", errors="replace") if log.is_file() else ""
    sentinel = f"FEREG_F0_DATASET_SUCCESS|{dataset_id}"
    observed = [
        line.strip()
        for line in log_text.splitlines()
        if line.strip().startswith("FEREG_F0_DATASET_SUCCESS|")
    ]
    if completed.returncode != 0 or observed != [sentinel] or not destination.is_file():
        raise SccContractError(f"dataset generation failed: {dataset_id}")
    invocation = run_dir / "invocation.json"
    status = run_dir / "status.json"
    write_json_exclusive(invocation, {
        "schema_version": 1,
        "stata": {"path": stata, "sha256": SCC_STATA_SHA256, "size_bytes": SCC_STATA_SIZE},
        "argv": argv, "cwd": str(run_dir), "environment": environment,
        "stream": "merged-stdout-stderr-v1",
    })
    write_json_exclusive(status, {
        "schema_version": 1, "process_returncode": completed.returncode,
        "terminal_sentinels": observed,
    })
    run_root = log_dir.parent
    return {
        "receipt_type": "fereg-f0-dataset-action-v1",
        "wrapper": _relative_file_record(run_root, wrapper),
        "invocation": _relative_file_record(run_root, invocation),
        "stream": _relative_file_record(run_root, run_dir / "runner.stdout"),
        "log": _relative_file_record(run_root, log),
        "status": _relative_file_record(run_root, status),
        "output": _relative_file_record(run_root, destination),
        "sentinel": sentinel,
    }


def _verify_dataset_action(
    run_root: Path,
    origin_root: Path,
    dataset_id: str,
    dataset_record: Mapping[str, Any],
    generator: Path,
    arguments: Sequence[Any],
    variant: str,
) -> None:
    action = _expect_keys(
        dataset_record.get("action"),
        {"receipt_type", "wrapper", "invocation", "stream", "log", "status", "output", "sentinel"},
        f"dataset {dataset_id} action",
    )
    if action["receipt_type"] != "fereg-f0-dataset-action-v1":
        raise SccContractError(f"dataset {dataset_id} action type changed")
    for name in ("wrapper", "invocation", "stream", "log", "status", "output"):
        _verify_relative_file(run_root, action[name], f"dataset {dataset_id} {name}")
    local_run_dir = run_root / f"logs/generate-{dataset_id}"
    origin_run_dir = origin_root / f"logs/generate-{dataset_id}"
    local_destination = run_root / f"work/datasets/{dataset_id}.dta"
    origin_destination = origin_root / f"work/datasets/{dataset_id}.dta"
    local_generator = generator
    origin_generator = origin_root / generator.relative_to(run_root)
    if not (local_run_dir / "controller-home").is_dir() or not (local_run_dir / "controller-tmp").is_dir():
        raise SccContractError(f"dataset {dataset_id} isolation directories are missing")
    expected_wrapper = _render_dataset_wrapper(
        origin_generator, arguments, origin_destination, dataset_id, variant
    ).encode("utf-8")
    if (local_run_dir / "generate.do").read_bytes() != expected_wrapper:
        raise SccContractError(f"dataset {dataset_id} wrapper changed")
    invocation = _expect_keys(
        _json(local_run_dir / "invocation.json"),
        {"schema_version", "stata", "argv", "cwd", "environment", "stream"},
        f"dataset {dataset_id} invocation",
    )
    expected_environment = {
        "HOME": str(origin_run_dir / "controller-home"),
        "TMPDIR": str(origin_run_dir / "controller-tmp"),
        "PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
    }
    if invocation != {
        "schema_version": 1,
        "stata": {"path": str(SCC_STATA), "sha256": SCC_STATA_SHA256, "size_bytes": SCC_STATA_SIZE},
        "argv": [str(SCC_STATA), "-b", "do", str(origin_run_dir / "generate.do")],
        "cwd": str(origin_run_dir), "environment": expected_environment,
        "stream": "merged-stdout-stderr-v1",
    }:
        raise SccContractError(f"dataset {dataset_id} invocation changed")
    status = _expect_keys(
        _json(local_run_dir / "status.json"),
        {"schema_version", "process_returncode", "terminal_sentinels"},
        f"dataset {dataset_id} status",
    )
    sentinel = f"FEREG_F0_DATASET_SUCCESS|{dataset_id}"
    log_sentinels = [
        line.strip() for line in (local_run_dir / "generate.log").read_text(
            encoding="utf-8", errors="strict"
        ).splitlines() if line.strip().startswith("FEREG_F0_DATASET_SUCCESS|")
    ]
    (local_run_dir / "runner.stdout").read_text(encoding="utf-8", errors="strict")
    if (
        action["sentinel"] != sentinel
        or status != {"schema_version": 1, "process_returncode": 0, "terminal_sentinels": [sentinel]}
        or log_sentinels != [sentinel]
        or action["output"] != _relative_file_record(run_root, local_destination)
        or dataset_record.get("sha256") != sha256_file(local_destination)
        or dataset_record.get("size_bytes") != local_destination.stat().st_size
    ):
        raise SccContractError(f"dataset {dataset_id} action did not reproduce its output")


def _bundle_digest(bundle: Path) -> tuple[str, int, list[dict[str, Any]]]:
    inventory: list[dict[str, Any]] = []
    total = 0
    for path in sorted(bundle.rglob("*")):
        if path.is_symlink():
            raise SccContractError("staged bundle may not contain symlinks")
        if path.is_file():
            size = path.stat().st_size
            total += size
            inventory.append({"path": path.relative_to(bundle).as_posix(), "sha256": sha256_file(path), "size_bytes": size})
    if not inventory:
        raise SccContractError("staged bundle is empty")
    return sha256_bytes(canonical_json_bytes(inventory)), total, inventory


def _archive_stata_inventory(archive_path: Path) -> list[dict[str, Any]]:
    inventory: list[dict[str, Any]] = []
    try:
        with tarfile.open(archive_path, mode="r:") as archive:
            for member in archive.getmembers():
                relative = Path(member.name)
                if (
                    relative.is_absolute()
                    or ".." in relative.parts
                    or member.issym()
                    or member.islnk()
                    or member.isdev()
                ):
                    raise SccContractError(
                        "source archive contains an unsafe Stata bundle member"
                    )
                if not member.isfile() or not relative.parts or relative.parts[0] != "stata":
                    continue
                if relative.as_posix() == "stata/fereg_linux.plugin":
                    continue
                extracted = archive.extractfile(member)
                if extracted is None:
                    raise SccContractError("source archive Stata member is unreadable")
                content = extracted.read()
                inventory.append(
                    {
                        "path": relative.as_posix(),
                        "sha256": sha256_bytes(content),
                        "size_bytes": len(content),
                    }
                )
    except (OSError, tarfile.TarError) as error:
        raise SccContractError("source archive cannot prove staged Stata bytes") from error
    if not inventory:
        raise SccContractError("source archive has no Stata package inventory")
    return sorted(inventory, key=lambda item: item["path"])


def _verify_staged_bundle(
    bundle: Path,
    arm_receipt: Mapping[str, Any],
    source_archive: Path,
) -> None:
    bundle_sha, bundle_size, inventory = _bundle_digest(bundle)
    observed = {
        "sha256": bundle_sha,
        "size_bytes": bundle_size,
        "inventory": inventory,
    }
    if observed != arm_receipt.get("bundle"):
        raise SccContractError(f"staged bundle changed after setup: {bundle.name}")
    plugin = bundle / "stata/fereg_linux.plugin"
    artifact = arm_receipt.get("artifact")
    if not isinstance(artifact, Mapping):
        raise SccContractError(f"setup receipt omits artifact identity: {bundle.name}")
    if (
        not plugin.is_file()
        or plugin.is_symlink()
        or sha256_file(plugin) != artifact.get("sha256")
        or plugin.stat().st_size != artifact.get("size_bytes")
    ):
        raise SccContractError(f"staged plugin changed after setup: {bundle.name}")
    expected_inventory = _archive_stata_inventory(source_archive)
    expected_inventory.append(
        {
            "path": "stata/fereg_linux.plugin",
            "sha256": artifact["sha256"],
            "size_bytes": artifact["size_bytes"],
        }
    )
    expected_inventory.sort(key=lambda item: item["path"])
    if inventory != expected_inventory:
        raise SccContractError(
            f"staged non-plugin package differs from exact source archive: {bundle.name}"
        )


def _make_tree_read_only(root: Path) -> None:
    for path in sorted(root.rglob("*"), reverse=True):
        if path.is_symlink():
            raise SccContractError(f"immutable work tree contains a symlink: {root.name}")
        if path.is_dir():
            path.chmod(0o555)
        else:
            path.chmod(0o555 if path.stat().st_mode & 0o111 else 0o444)
    root.chmod(0o555)


def _build_tool_paths() -> dict[str, Path]:
    cargo_bin = EXPECTED_CARGO_HOME / "bin"
    return {
        "bash": Path("/bin/bash"),
        "cargo": cargo_bin / "cargo",
        "rustc": cargo_bin / "rustc",
        "rustup": cargo_bin / "rustup",
        "python3": Path(sys.executable).resolve(),
        "cc": Path("/usr/bin/cc"),
        "linker": Path("/usr/bin/cc"),
        "ar": Path("/usr/bin/ar"),
        "ranlib": Path("/usr/bin/ranlib"),
        "strip": Path("/usr/bin/strip"),
        "objcopy": Path("/usr/bin/objcopy"),
        "sha256sum": Path("/usr/bin/sha256sum"),
        "readelf": Path("/usr/bin/readelf"),
        "nm": Path("/usr/bin/nm"),
        "ldd": Path("/usr/bin/ldd"),
        "uname": Path("/usr/bin/uname"),
        "dirname": Path("/usr/bin/dirname"),
        "basename": Path("/usr/bin/basename"),
        "mkdir": Path("/usr/bin/mkdir"),
        "rm": Path("/usr/bin/rm"),
    }


def _build_path(paths: Mapping[str, Path]) -> str:
    directories: list[str] = []
    # The build audit invokes bare `python3`; its exact recorded module
    # interpreter must precede /bin and /usr/bin. Cargo/Rust follow for the
    # same reason, with every remaining directory still derived from a sealed
    # tool record rather than ambient PATH.
    priority = ("python3", "cargo", "rustc", "rustup") + tuple(
        role for role in BUILD_TOOL_ROLES
        if role not in {"python3", "cargo", "rustc", "rustup"}
    )
    for role in priority:
        parent = str(paths[role].parent)
        if parent not in directories:
            directories.append(parent)
    return ":".join(directories)


def _closed_tool_environment(run_root: Path, paths: Mapping[str, Path]) -> dict[str, str]:
    return {
        "HOME": str(run_root / "work/build/tool-home"),
        "TMPDIR": str(run_root / "work/build/tool-tmp"),
        "PATH": _build_path(paths),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "CARGO_HOME": str(EXPECTED_CARGO_HOME),
        "RUSTUP_HOME": str(EXPECTED_RUSTUP_HOME),
        "RUSTUP_TOOLCHAIN": EXPECTED_RUSTUP_TOOLCHAIN,
    }


def _tool_first_line(path: Path, environment: Mapping[str, str]) -> str:
    completed = subprocess.run(
        [str(path), "--version"], env=dict(environment), check=False,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )
    lines = completed.stdout.splitlines()
    if completed.returncode != 0 or not lines:
        raise SccContractError(f"formal build tool did not report a version: {path}")
    return lines[0].strip()


def _tool_record(path: Path, version: str) -> dict[str, Any]:
    resolved = path.resolve()
    if not path.is_file() or not os.access(path, os.X_OK) or not resolved.is_file():
        raise SccContractError(f"formal build tool is absent or unsafe: {path}")
    return {
        "path": str(path),
        "resolved_path": str(resolved),
        "sha256": sha256_file(resolved),
        "size_bytes": resolved.stat().st_size,
        "version": version,
    }


def _tree_identity(root: Path, *, allow_symlinks: bool) -> dict[str, Any]:
    if root.is_symlink() or not root.is_dir():
        raise SccContractError(f"formal build tree is absent or unsafe: {root}")
    inventory: list[dict[str, Any]] = []
    total = 0
    for path in sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            if not allow_symlinks:
                raise SccContractError(f"formal build tree contains a symlink: {relative}")
            target = os.readlink(path)
            inventory.append({"path": relative, "kind": "symlink", "target": target})
        elif path.is_file():
            size = path.stat().st_size
            total += size
            inventory.append({
                "path": relative, "kind": "file", "sha256": sha256_file(path),
                "size_bytes": size,
            })
        elif not path.is_dir():
            raise SccContractError(f"formal build tree contains an unsafe entry: {relative}")
    if not inventory:
        raise SccContractError("formal build tree inventory is empty")
    return {
        "sha256": sha256_bytes(canonical_json_bytes(inventory)),
        "entries": len(inventory),
        "size_bytes": total,
    }


def _command_output(
    command: Sequence[str], environment: Mapping[str, str], where: str
) -> str:
    completed = subprocess.run(
        list(command), env=dict(environment), check=False,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )
    value = completed.stdout.strip()
    if completed.returncode != 0 or not value:
        raise SccContractError(f"{where} could not be resolved")
    return value


def _build_tool_records(
    run_root: Path, paths: Mapping[str, Path]
) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    if set(paths) != set(BUILD_TOOL_ROLES):
        raise SccContractError("formal build tool role inventory is not closed")
    environment = _closed_tool_environment(run_root, paths)
    tools = {
        role: _tool_record(paths[role], _tool_first_line(paths[role], environment))
        for role in BUILD_TOOL_ROLES
    }
    if (
        tools["cargo"]["version"] != EXPECTED_CARGO
        or tools["rustc"]["version"] != EXPECTED_RUSTC
        or tools["python3"]["version"] != EXPECTED_PYTHON
    ):
        raise SccContractError("formal build cargo/rustc/Python versions differ")
    actual_rustc = Path(_command_output(
        [str(paths["rustup"]), "which", "rustc", "--toolchain", EXPECTED_RUSTUP_TOOLCHAIN],
        environment, "actual rustc",
    ))
    actual_cargo = Path(_command_output(
        [str(paths["rustup"]), "which", "cargo", "--toolchain", EXPECTED_RUSTUP_TOOLCHAIN],
        environment, "actual cargo",
    ))
    sysroot = Path(_command_output(
        [str(actual_rustc), "--print", "sysroot"], environment, "Rust sysroot",
    ))
    rustc_verbose = _command_output(
        [str(actual_rustc), "-vV"], environment, "actual rustc metadata"
    )
    toolchain = {
        "toolchain": EXPECTED_RUSTUP_TOOLCHAIN,
        "actual_rustc": _tool_record(actual_rustc, rustc_verbose),
        "actual_cargo": _tool_record(
            actual_cargo, _tool_first_line(actual_cargo, environment)
        ),
        "sysroot": {
            "path": str(sysroot),
            **_tree_identity(sysroot, allow_symlinks=True),
        },
    }
    return tools, toolchain


def _formal_build_environment(
    run_root: Path,
    identity: Mapping[str, Any],
    tools: Mapping[str, Mapping[str, Any]],
    *,
    arm: str,
    slots: int,
) -> dict[str, str]:
    if slots != 16 or arm not in {"baseline", "candidate"} or set(tools) != set(BUILD_TOOL_ROLES):
        raise SccContractError("formal F0 build requires its exact tool/slot contract")
    base = run_root / f"work/build/{arm}"
    return {
        "HOME": str(base / "home"),
        "TMPDIR": str(base / "tmp"),
        "PATH": _build_path({role: Path(str(tools[role]["path"])) for role in BUILD_TOOL_ROLES}),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "CARGO_HOME": str(base / "cargo-home"),
        "RUSTUP_HOME": str(EXPECTED_RUSTUP_HOME),
        "RUSTUP_TOOLCHAIN": EXPECTED_RUSTUP_TOOLCHAIN,
        "CARGO_NET_OFFLINE": "true",
        "CARGO_INCREMENTAL": "0",
        "CARGO_BUILD_JOBS": "16",
        "CC": str(tools["cc"]["path"]),
        "AR": str(tools["ar"]["path"]),
        "RANLIB": str(tools["ranlib"]["path"]),
        "RUSTC": str(tools["rustc"]["path"]),
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER": str(tools["linker"]["path"]),
        "FEREG_GIT_COMMIT": str(identity["commit"])[:12],
        "FEREG_BUILD_DATE": str(identity["build_date"]),
    }


def _vendor_environment(
    run_root: Path,
    tools: Mapping[str, Mapping[str, Any]],
    *,
    arm: str,
) -> dict[str, str]:
    base = run_root / f"work/build/{arm}"
    return {
        "HOME": str(base / "vendor-home"),
        "TMPDIR": str(base / "tmp"),
        "PATH": _build_path({role: Path(str(tools[role]["path"])) for role in BUILD_TOOL_ROLES}),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "CARGO_HOME": str(EXPECTED_CARGO_HOME),
        "RUSTUP_HOME": str(EXPECTED_RUSTUP_HOME),
        "RUSTUP_TOOLCHAIN": EXPECTED_RUSTUP_TOOLCHAIN,
        "CARGO_NET_OFFLINE": "true",
    }


def _relative_file_record(run_root: Path, path: Path) -> dict[str, Any]:
    relative = path.resolve().relative_to(run_root.resolve()).as_posix()
    return {"path": relative, "sha256": sha256_file(path), "size_bytes": path.stat().st_size}


def _prepare_dependency_closure(
    run_root: Path,
    source: Path,
    tools: Mapping[str, Mapping[str, Any]],
    *,
    arm: str,
) -> dict[str, Any]:
    lock_bytes = (source / "Cargo.lock").read_bytes()
    if b'\nsource = "git+' in lock_bytes:
        # A shared Cargo git checkout is not an independent source of truth:
        # `cargo vendor --offline` would faithfully seal attacker-selected bytes.
        # Keep the formal path unavailable until the common qualification/F0
        # closure supplies canonical archives for every pinned git revision.
        raise SccContractError(
            "formal F0 build requires independently sealed canonical archives "
            "for every Cargo.lock git dependency"
        )
    base = run_root / f"work/build/{arm}"
    for directory in (base / "home", base / "tmp", base / "vendor-home", base / "cargo-home"):
        directory.mkdir(parents=True, exist_ok=False)
    vendor = base / "vendor"
    environment = _vendor_environment(run_root, tools, arm=arm)
    command = [
        str(tools["cargo"]["path"]), "vendor", "--locked", "--offline",
        "--versioned-dirs", str(vendor),
    ]
    completed = subprocess.run(
        command, cwd=source, env=environment, check=False,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    vendor_log = base / "vendor.log"
    _write_exclusive(vendor_log, completed.stderr)
    config = base / "cargo-home/config.toml"
    if completed.returncode != 0 or not completed.stdout or not vendor.is_dir():
        raise SccContractError(f"{arm} dependency vendoring failed")
    _write_exclusive(config, completed.stdout)
    if str(vendor).encode("utf-8") not in completed.stdout:
        raise SccContractError(f"{arm} Cargo vendor config does not bind its run-local closure")
    return {
        "command": command,
        "cwd": str(source),
        "environment": environment,
        "config": _relative_file_record(run_root, config),
        "vendor_log": _relative_file_record(run_root, vendor_log),
        "vendor": {"path": vendor.relative_to(run_root).as_posix(), **_tree_identity(vendor, allow_symlinks=False)},
    }


def _verify_relative_file(run_root: Path, value: Any, where: str) -> None:
    record = _expect_keys(value, {"path", "sha256", "size_bytes"}, where)
    path = run_root / str(record["path"])
    if (
        Path(str(record["path"])).is_absolute()
        or ".." in Path(str(record["path"])).parts
        or path.is_symlink()
        or not path.is_file()
        or sha256_file(path) != record["sha256"]
        or path.stat().st_size != record["size_bytes"]
    ):
        raise SccContractError(f"{where} bytes changed")


def _verify_build_receipt(
    run_root: Path,
    preparation: Mapping[str, Any],
    arm: str,
    receipt: Any,
    *,
    portable: bool,
) -> None:
    record = _expect_keys(
        receipt,
        {
            "schema_version", "receipt_type", "arm", "source_commit", "source_tree",
            "build_date", "command", "cwd", "environment", "tools", "rust_toolchain",
            "dependencies", "log", "artifact", "complete_sentinel",
        },
        f"{arm} formal build receipt",
    )
    identity = preparation["arms"][arm]
    if (
        record["schema_version"] != 1
        or record["receipt_type"] != "fereg-f0-formal-linux-build-v1"
        or record["arm"] != arm
        or record["source_commit"] != identity["commit"]
        or record["source_tree"] != identity["tree"]
        or record["build_date"] != identity["build_date"]
        or record["complete_sentinel"] != "FEREG_F0_FORMAL_LINUX_BUILD_COMPLETE"
    ):
        raise SccContractError(f"{arm} formal build identity is invalid")
    tools = _expect_keys(record["tools"], set(BUILD_TOOL_ROLES), f"{arm} build tools")
    for role in BUILD_TOOL_ROLES:
        _expect_keys(
            tools[role], {"path", "resolved_path", "sha256", "size_bytes", "version"},
            f"{arm} build tool {role}",
        )
    if tools["cargo"]["version"] != EXPECTED_CARGO or tools["rustc"]["version"] != EXPECTED_RUSTC or tools["python3"]["version"] != EXPECTED_PYTHON:
        raise SccContractError(f"{arm} formal tool versions changed")
    rust_toolchain = _expect_keys(
        record["rust_toolchain"], {"toolchain", "actual_rustc", "actual_cargo", "sysroot"},
        f"{arm} actual Rust toolchain",
    )
    if rust_toolchain["toolchain"] != EXPECTED_RUSTUP_TOOLCHAIN:
        raise SccContractError(f"{arm} Rust toolchain selection changed")
    for role in ("actual_rustc", "actual_cargo"):
        _expect_keys(
            rust_toolchain[role], {"path", "resolved_path", "sha256", "size_bytes", "version"},
            f"{arm} {role}",
        )
    _expect_keys(rust_toolchain["sysroot"], {"path", "sha256", "entries", "size_bytes"}, f"{arm} Rust sysroot")
    recorded_root = Path(str(record["cwd"])).parents[3]
    if recorded_root.name != run_root.name or record["cwd"] != str(recorded_root / f"work/setup/{arm}/source"):
        raise SccContractError(f"{arm} formal build cwd changed")
    expected_environment = _formal_build_environment(recorded_root, identity, tools, arm=arm, slots=16)
    if record["environment"] != expected_environment or record["command"] != [str(tools["bash"]["path"]), "scripts/build_linux.sh"]:
        raise SccContractError(f"{arm} formal build command/environment changed")
    dependencies = _expect_keys(
        record["dependencies"], {"command", "cwd", "environment", "config", "vendor_log", "vendor"},
        f"{arm} dependency closure",
    )
    expected_vendor = recorded_root / f"work/build/{arm}/vendor"
    expected_vendor_command = [
        str(tools["cargo"]["path"]), "vendor", "--locked", "--offline",
        "--versioned-dirs", str(expected_vendor),
    ]
    if (
        dependencies["command"] != expected_vendor_command
        or dependencies["cwd"] != record["cwd"]
        or dependencies["environment"] != _vendor_environment(recorded_root, tools, arm=arm)
    ):
        raise SccContractError(f"{arm} dependency closure command/environment changed")
    _verify_relative_file(run_root, dependencies["config"], f"{arm} Cargo config")
    _verify_relative_file(run_root, dependencies["vendor_log"], f"{arm} vendor log")
    vendor_record = _expect_keys(dependencies["vendor"], {"path", "sha256", "entries", "size_bytes"}, f"{arm} vendor tree")
    local_vendor = run_root / str(vendor_record["path"])
    if vendor_record != {"path": str(vendor_record["path"]), **_tree_identity(local_vendor, allow_symlinks=False)}:
        raise SccContractError(f"{arm} vendored dependency closure changed")
    for field in ("log", "artifact"):
        _verify_relative_file(run_root, record[field], f"{arm} formal build {field}")
    if not portable:
        for role in BUILD_TOOL_ROLES:
            tool = tools[role]
            path = Path(str(tool["path"]))
            if (
                not path.is_file()
                or str(path.resolve()) != tool["resolved_path"]
                or sha256_file(path.resolve()) != tool["sha256"]
                or path.resolve().stat().st_size != tool["size_bytes"]
                or _tool_first_line(path, record["environment"]) != tool["version"]
            ):
                raise SccContractError(f"{arm} formal build tool changed: {role}")
        actual_rustc = Path(str(rust_toolchain["actual_rustc"]["path"]))
        actual_cargo = Path(str(rust_toolchain["actual_cargo"]["path"]))
        for path, role in ((actual_rustc, "actual_rustc"), (actual_cargo, "actual_cargo")):
            tool = rust_toolchain[role]
            if sha256_file(path.resolve()) != tool["sha256"] or path.resolve().stat().st_size != tool["size_bytes"]:
                raise SccContractError(f"{arm} actual Rust tool changed: {role}")
        sysroot = Path(str(rust_toolchain["sysroot"]["path"]))
        if rust_toolchain["sysroot"] != {"path": str(sysroot), **_tree_identity(sysroot, allow_symlinks=True)}:
            raise SccContractError(f"{arm} Rust sysroot changed")


def _verify_shared_build_receipt(
    run_root: Path,
    preparation: Mapping[str, Any],
    arm: str,
    receipt: Any,
    *,
    portable: bool,
    expected_origin_root: Path | None = None,
) -> None:
    record = _expect_keys(
        receipt,
        {
            "schema_version", "receipt_type", "arm", "source_commit",
            "source_tree", "build_date", "command", "cwd", "environment",
            "formal_closure", "runtime_closure", "log", "artifact",
            "complete_sentinel",
        },
        f"{arm} formal build receipt",
    )
    identity = preparation["arms"][arm]
    if (
        record["schema_version"] != 1
        or record["receipt_type"] != "fereg-f0-formal-linux-build-v1"
        or record["arm"] != arm
        or record["source_commit"] != identity["commit"]
        or record["source_tree"] != identity["tree"]
        or record["build_date"] != identity["build_date"]
        or record["complete_sentinel"] != "FEREG_F0_FORMAL_LINUX_BUILD_COMPLETE"
    ):
        raise SccContractError(f"{arm} formal build identity is invalid")
    source = run_root / f"work/setup/{arm}/source"
    recorded_source_suffix = f"/work/setup/{arm}/source"
    if (
        (not portable and record["cwd"] != str(source))
        or (portable and not str(record["cwd"]).endswith(recorded_source_suffix))
    ):
        raise SccContractError(f"{arm} formal build cwd changed")
    formal_path = run_root / "work/setup/harness/source/scripts/formal_rust_build_closure.py"
    formal = _load_python_module(
        formal_path, f"fereg_f0_formal_verify_{sha256_file(formal_path)[:12]}"
    )
    try:
        cache_inputs = None
        if not portable:
            cargo_source_home = EXPECTED_CARGO_HOME
            cache_inputs = formal.SourceCacheInputs(
                cargo_source_home=cargo_source_home,
                registry_archive_roots=(cargo_source_home / "registry/cache/index.crates.io-1949cf8c6b5b557f",),
                git_repositories={
                    "git+https://github.com/johannes-schmieder/CMG?rev=ce126358a82cc1a461cfe7b9f9318b4c3c1c1316": cargo_source_home / "git/checkouts/cmg-2012f6025e587b49/ce12635",
                    "git+https://github.com/py-econometrics/within?rev=b7779cbab7a3116be56aae4389fde1f6e6a99a9f": cargo_source_home / "git/checkouts/within-937ec8f21002c884/b7779cb",
                },
                git_executable=Path("/usr/bin/git"),
            )
        verified = formal.verify_formal_rust_build_closure(
            run_root, record["formal_closure"], portable=portable,
            cache_inputs=cache_inputs,
        )
        qualified = formal.verify_qualified_formal_rust_build(
            run_root, record["formal_closure"], record["runtime_closure"],
            portable=portable, cache_inputs=cache_inputs,
        )
    except (OSError, ValueError) as error:
        raise SccContractError(f"{arm} shared formal Rust closure failed: {error}") from error
    if record["environment"] != verified["environment"]:
        raise SccContractError(f"{arm} build environment differs from shared closure")
    if qualified["artifact"] != record["artifact"]:
        raise SccContractError(f"{arm} runtime closure binds a different artifact")
    inner_tools = verified["toolchain"]["build_tools"]
    if portable:
        recorded_root = str(record["cwd"])[: -len(recorded_source_suffix)]
        if expected_origin_root is not None and Path(recorded_root) != expected_origin_root:
            raise SccContractError(f"{arm} formal build origin differs from submission")
        expected_script = Path(recorded_root) / "work/setup/harness/source/benchmarks/scc/fereg_f0_formal_build.sh"
        command_source = Path(record["cwd"])
    else:
        expected_script = run_root / "work/setup/harness/source/benchmarks/scc/fereg_f0_formal_build.sh"
        command_source = source
    if record["command"] != [str(inner_tools["bash"]["path"]), str(expected_script), str(command_source)]:
        raise SccContractError(f"{arm} formal build command changed")
    for field in ("log", "artifact"):
        _verify_relative_file(run_root, record[field], f"{arm} formal build {field}")


# All current callers use the authoritative shared receipt verifier.  Keep the
# older inline prototype above only until its callers and tests have migrated;
# it cannot publish or verify a receipt because this binding replaces it.
_verify_build_receipt = _verify_shared_build_receipt


def _run_setup_smoke(
    run_root: Path,
    source: Path,
    artifact: Mapping[str, Any],
    *,
    arm: str,
) -> dict[str, Any]:
    """Run one small OLS fixture before any governing matrix tasks."""

    smoke_root = run_root / f"logs/setup-smoke/{arm}"
    smoke_root.mkdir(parents=True, exist_ok=False)
    summary_path = smoke_root / "summary.json"
    output_path = smoke_root / "runner.stdout"
    fixture = source / "ci/fereg_smoke.do"
    plugin = source / "dist/fereg_linux.plugin"
    command = [
        sys.executable,
        str(source / "scripts/run_stata_tests.py"),
        "--stata",
        str(SCC_STATA),
        "--plugin",
        str(plugin),
        "--plugin-sha256",
        str(artifact["sha256"]),
        "--log-dir",
        str(smoke_root / "fixture-logs"),
        "--summary-json",
        str(summary_path),
        str(fixture),
    ]
    completed = subprocess.run(
        command,
        cwd=source,
        env=_stata_environment(smoke_root),
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    _write_exclusive(output_path, completed.stdout.encode("utf-8"))
    if not summary_path.is_file():
        raise SccContractError(f"{arm} setup smoke wrote no summary")
    summary = _json(summary_path)
    if not isinstance(summary, Mapping):
        raise SccContractError(f"{arm} setup smoke summary is malformed")
    results = summary.get("results")
    if (
        completed.returncode != 0
        or summary.get("status") != "success"
        or summary.get("completed") is not True
        or summary.get("stata_rc") != 0
        or summary.get("tests_passed") != 1
        or summary.get("tests_failed") != 0
        or not isinstance(results, list)
        or len(results) != 1
        or not isinstance(results[0], Mapping)
        or results[0].get("fixture") != "ci/fereg_smoke.do"
        or results[0].get("status") != "success"
        or results[0].get("process_rc") != 0
        or results[0].get("stata_rc") != 0
    ):
        raise SccContractError(f"{arm} setup smoke failed")
    evidence_sha, evidence_size, inventory = _bundle_digest(smoke_root)
    return {
        "status": "pass",
        "fixture": _relative_file_record(run_root, fixture),
        "plugin": _relative_file_record(run_root, plugin),
        "summary": _relative_file_record(run_root, summary_path),
        "evidence": {
            "sha256": evidence_sha,
            "size_bytes": evidence_size,
            "inventory": inventory,
        },
        "terminal_sentinel": "END_FIXTURE|fereg_smoke|rc=0",
    }


def _verify_setup_smoke(
    run_root: Path,
    setup: Mapping[str, Any],
) -> None:
    smoke = _expect_keys(
        setup.get("smoke"), {"baseline", "candidate"}, "setup smoke"
    )
    for arm in ("baseline", "candidate"):
        record = _expect_keys(
            smoke[arm],
            {"status", "fixture", "plugin", "summary", "evidence", "terminal_sentinel"},
            f"setup {arm} smoke",
        )
        source = run_root / f"work/setup/{arm}/source"
        fixture = source / "ci/fereg_smoke.do"
        plugin = source / "dist/fereg_linux.plugin"
        fixture_record = _expect_keys(
            record["fixture"], {"path", "sha256", "size_bytes"},
            f"setup {arm} smoke fixture",
        )
        plugin_record = _expect_keys(
            record["plugin"], {"path", "sha256", "size_bytes"},
            f"setup {arm} smoke plugin",
        )
        summary_record = _expect_keys(
            record["summary"], {"path", "sha256", "size_bytes"},
            f"setup {arm} smoke summary",
        )
        summary_path = run_root / str(summary_record["path"])
        if (
            record["status"] != "pass"
            or record["terminal_sentinel"] != "END_FIXTURE|fereg_smoke|rc=0"
            or fixture_record != _relative_file_record(run_root, fixture)
            or plugin_record != _relative_file_record(run_root, plugin)
            or plugin_record["sha256"] != setup["arms"][arm]["artifact"]["sha256"]
        ):
            raise SccContractError(f"setup {arm} smoke binding changed")
        _verify_relative_file(run_root, summary_record, f"setup {arm} smoke summary")
        summary = _json(summary_path)
        if not isinstance(summary, Mapping):
            raise SccContractError(f"setup {arm} smoke summary is malformed")
        results = summary.get("results")
        if (
            summary.get("status") != "success"
            or summary.get("completed") is not True
            or summary.get("stata_rc") != 0
            or summary.get("tests_passed") != 1
            or summary.get("tests_failed") != 0
            or not isinstance(results, list)
            or len(results) != 1
            or not isinstance(results[0], Mapping)
            or results[0].get("fixture") != "ci/fereg_smoke.do"
            or results[0].get("status") != "success"
            or results[0].get("process_rc") != 0
            or results[0].get("stata_rc") != 0
        ):
            raise SccContractError(f"setup {arm} smoke summary changed")
        smoke_root = run_root / f"logs/setup-smoke/{arm}"
        evidence_sha, evidence_size, inventory = _bundle_digest(smoke_root)
        if record["evidence"] != {
            "sha256": evidence_sha,
            "size_bytes": evidence_size,
            "inventory": inventory,
        }:
            raise SccContractError(f"setup {arm} smoke evidence changed")
        log = smoke_root / "fixture-logs/fereg_smoke.log"
        sentinels = [
            line.strip()
            for line in log.read_text(encoding="utf-8", errors="strict").splitlines()
            if line.strip().startswith("END_FIXTURE|")
        ]
        if sentinels != [record["terminal_sentinel"]]:
            raise SccContractError(f"setup {arm} smoke sentinel changed")


def setup_run(run_root: Path) -> None:
    verify_input(run_root)
    preparation = read_sealed(run_root / "input/preparation.json", "fereg-f0-scc-preparation-v1")
    manifest_path = run_root / "input/manifest.json"
    manifest = load_regression_manifest(manifest_path)
    if sha256_file(manifest_path) != preparation["manifest"]["sha256"]:
        raise SccContractError("run manifest changed after preparation")
    setup_root = run_root / "work/setup"
    setup_root.mkdir(parents=True, exist_ok=False)
    bundles_root = run_root / "work/bundles"
    bundles_root.mkdir(parents=True, exist_ok=False)
    logs = run_root / "logs/setup"
    logs.mkdir(parents=True, exist_ok=False)

    harness_identity = preparation["harness_source"]
    harness_archive = run_root / "input" / harness_identity["archive"]
    harness_source = setup_root / "harness/source"
    harness_source.parent.mkdir(parents=True, exist_ok=False)
    _safe_extract_tar(harness_archive, harness_source)
    _verify_extracted_archive_files(harness_archive, harness_source, containment_root=run_root)
    if (
        sha256_file(harness_source / "Cargo.lock")
        != harness_identity["cargo_lock_sha256"]
        or sha256_file(harness_source / "references.lock.toml")
        != harness_identity["references_lock_sha256"]
        or (harness_source / preparation["harness_manifest_path"]).read_bytes()
        != manifest_path.read_bytes()
    ):
        raise SccContractError("extracted harness source closure mismatch")
    for relative in HARNESS_PATHS:
        if (
            (harness_source / relative).read_bytes()
            != (run_root / "input/harness" / Path(relative).name).read_bytes()
        ):
            raise SccContractError(f"run-scoped harness differs from source: {relative}")

    try:
        build_slots = int(os.environ.get("NSLOTS", ""))
    except ValueError as error:
        raise SccContractError("formal F0 build has no exact NSLOTS value") from error
    if build_slots != 16:
        raise SccContractError("formal F0 build requires exactly 16 SCC slots")
    for directory in (run_root / "work/build/tool-home", run_root / "work/build/tool-tmp"):
        directory.mkdir(parents=True, exist_ok=False)
    build_paths = _build_tool_paths()
    build_tools, rust_toolchain = _build_tool_records(run_root, build_paths)
    formal = _load_python_module(
        harness_source / "scripts/formal_rust_build_closure.py",
        f"fereg_f0_formal_rust_{harness_identity['commit'][:12]}",
    )
    cargo_source_home = EXPECTED_CARGO_HOME
    cache_inputs = formal.SourceCacheInputs(
        cargo_source_home=cargo_source_home,
        registry_archive_roots=(
            cargo_source_home / "registry/cache/index.crates.io-1949cf8c6b5b557f",
        ),
        git_repositories={
            "git+https://github.com/johannes-schmieder/CMG?rev=ce126358a82cc1a461cfe7b9f9318b4c3c1c1316": cargo_source_home / "git/checkouts/cmg-2012f6025e587b49/ce12635",
            "git+https://github.com/py-econometrics/within?rev=b7779cbab7a3116be56aae4389fde1f6e6a99a9f": cargo_source_home / "git/checkouts/within-937ec8f21002c884/b7779cb",
        },
        git_executable=Path("/usr/bin/git"),
    )
    toolchain = {
        "cargo": build_tools["cargo"]["version"],
        "rustc": build_tools["rustc"]["version"],
        "cc": build_tools["cc"]["version"],
    }
    arm_receipts: dict[str, Any] = {}
    for arm in ("baseline", "candidate"):
        identity = preparation["arms"][arm]
        archive = run_root / "input" / identity["archive"]
        if sha256_file(archive) != identity["archive_sha256"] or archive.stat().st_size != identity["archive_size_bytes"]:
            raise SccContractError(f"{arm} source archive identity mismatch")
        source = setup_root / arm / "source"
        source.parent.mkdir(parents=True, exist_ok=False)
        _safe_extract_tar(archive, source)
        _verify_extracted_archive_files(archive, source, containment_root=run_root)
        if sha256_file(source / "Cargo.lock") != identity["cargo_lock_sha256"] or sha256_file(source / "references.lock.toml") != identity["references_lock_sha256"]:
            raise SccContractError(f"{arm} extracted source closure mismatch")
        closure_receipt = formal.prepare_formal_rust_build_closure(
            run_root,
            source,
            run_root / f"work/build/{arm}/closure",
            run_root / f"work/build/{arm}/target",
            cache_inputs,
            cargo=Path(rust_toolchain["actual_cargo"]["path"]),
            rustc=Path(rust_toolchain["actual_rustc"]["path"]),
            helpers={
                role: build_paths[role]
                for role in ("cc", "linker", "ar", "ranlib", "nm", "strip", "objcopy")
            },
            build_tools={
                role: build_paths[role]
                for role in formal.REQUIRED_BUILD_TOOL_ROLES
            },
            slots=build_slots,
            build_variables={
                "FEREG_GIT_COMMIT": str(identity["commit"])[:12],
                "FEREG_BUILD_DATE": str(identity["build_date"]),
            },
        )
        env = dict(closure_receipt["environment"])
        formal_build_script = harness_source / "benchmarks/scc/fereg_f0_formal_build.sh"
        command = [str(build_tools["bash"]["path"]), str(formal_build_script), str(source)]
        completed = subprocess.run(
            command, cwd=source, env=env, check=False,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        )
        build_log = logs / f"{arm}-build.log"
        _write_exclusive(build_log, completed.stdout.encode("utf-8"))
        plugin = source / "dist/fereg_linux.plugin"
        if completed.returncode != 0 or not plugin.is_file():
            raise SccContractError(f"{arm} Linux build/audit/loader failed")
        runtime_closure = formal.capture_linux_runtime_closure(
            run_root, plugin, closure_receipt["toolchain"]
        )
        embedded_commit = str(identity["commit"])[:12]
        if embedded_commit.encode("ascii") not in plugin.read_bytes():
            raise SccContractError(f"{arm} Linux plugin omits its exact source commit")
        bundle = bundles_root / arm
        shutil.copytree(source / "stata", bundle / "stata")
        shutil.copy2(plugin, bundle / "stata/fereg_linux.plugin")
        bundle_sha, bundle_size, inventory = _bundle_digest(bundle)
        build_receipt = {
            "schema_version": 1,
            "receipt_type": "fereg-f0-formal-linux-build-v1",
            "arm": arm,
            "source_commit": identity["commit"],
            "source_tree": identity["tree"],
            "build_date": identity["build_date"],
            "command": command,
            "cwd": str(source),
            "environment": env,
            "formal_closure": closure_receipt,
            "runtime_closure": runtime_closure,
            "log": _relative_file_record(run_root, build_log),
            "artifact": _relative_file_record(run_root, plugin),
            "complete_sentinel": "FEREG_F0_FORMAL_LINUX_BUILD_COMPLETE",
        }
        arm_receipts[arm] = {
            "source": {key: identity[key] for key in ("commit", "tree", "cargo_lock_sha256", "references_lock_sha256")},
            "artifact": {
                "platform": "linux-x86_64",
                "sha256": sha256_file(plugin),
                "size_bytes": plugin.stat().st_size,
                "embedded_git_commit": embedded_commit,
            },
            "bundle": {"sha256": bundle_sha, "size_bytes": bundle_size, "inventory": inventory},
            "build": build_receipt,
        }
        _verify_build_receipt(
            run_root, preparation, arm, build_receipt, portable=False
        )
        _verify_extracted_archive_files(
            archive, source, allow_formal_dist=True, containment_root=run_root
        )
        _verify_staged_bundle(bundle, arm_receipts[arm], archive)

    registered_linux = next(
        (
            item
            for item in manifest["baseline_candidate"]["artifacts"]
            if item["platform"] == "linux-x86_64"
        ),
        None,
    )
    if not isinstance(registered_linux, Mapping):
        raise SccContractError("manifest omits the Linux F0 artifact slot")
    if registered_linux.get("status") != "pending":
        registered_identity = {
            key: registered_linux[key]
            for key in ("platform", "sha256", "size_bytes", "embedded_git_commit")
        }
        if arm_receipts["baseline"]["artifact"] != registered_identity:
            raise SccContractError("exact F0 Linux build differs from the registered artifact")

    setup_smoke = {
        arm: _run_setup_smoke(
            run_root,
            setup_root / arm / "source",
            arm_receipts[arm]["artifact"],
            arm=arm,
        )
        for arm in ("baseline", "candidate")
    }

    baseline_source = setup_root / "baseline/source"
    ols_manifest = _json(baseline_source / "benchmarks/manifests/ols_v1.json")
    datasets_by_id = {item["id"]: item for item in ols_manifest["datasets"]}
    dataset_root = run_root / "work/datasets"
    dataset_root.mkdir(parents=True, exist_ok=False)
    stata = _stata_executable()
    datasets: dict[str, Any] = {}
    dataset_plans = (
        ("generated_dense", "generated_dense", "dense"),
        ("generated_sparse", "generated_sparse", "sparse"),
        ("generated_qualified_dense", "generated_dense", "qualified_dense"),
        ("generated_disconnected", "generated_sparse", "disconnected"),
    )
    for dataset_id, source_id, variant in dataset_plans:
        spec = datasets_by_id.get(source_id)
        if not isinstance(spec, Mapping) or spec.get("kind") != "generated_dta":
            raise SccContractError(f"baseline OLS manifest lacks {source_id}")
        generator = baseline_source / spec["source"]["path"]
        if sha256_file(generator) != spec["source"]["sha256"]:
            raise SccContractError(f"baseline generator hash mismatch: {source_id}")
        destination = dataset_root / f"{dataset_id}.dta"
        action = _generate_dataset(
            stata,
            generator,
            spec["arguments"],
            destination,
            logs,
            dataset_id,
            variant=variant,
        )
        datasets[dataset_id] = {
            "sha256": sha256_file(destination),
            "size_bytes": destination.stat().st_size,
            "source_sha256": spec["source"]["sha256"],
            "source_dataset": source_id,
            "arguments": list(spec["arguments"]),
            "variant": variant,
            "action": action,
        }

    for arm in ("baseline", "candidate"):
        _make_tree_read_only(bundles_root / arm)
        _make_tree_read_only(setup_root / arm / "source")
    _make_tree_read_only(harness_source)
    _make_tree_read_only(dataset_root)

    payload = {
        "schema_version": RECEIPT_SCHEMA,
        "receipt_type": "fereg-f0-scc-setup-v1",
        "run_id": run_root.name,
        "manifest": dict(preparation["manifest"]),
        "preparation_receipt_sha256": sha256_file(run_root / "input/preparation.json"),
        "harness_source": dict(harness_identity),
        "toolchain": toolchain,
        "stata_controller": {
            "path": str(SCC_STATA), "sha256": SCC_STATA_SHA256,
            "size_bytes": SCC_STATA_SIZE,
            "argv_contract": [str(SCC_STATA), "-b", "do", "<retained-wrapper.do>"],
            "environment_contract": "closed-private-home-tmp-v1",
        },
        "arms": arm_receipts,
        "smoke": setup_smoke,
        "datasets": datasets,
        "sge": {
            "job_id": os.environ.get("JOB_ID", ""),
            "host_id": socket.gethostname(),
            "slots": int(os.environ.get("NSLOTS", "1")),
        },
        "terminal_sentinel": "FEREG_F0_SETUP_SUCCESS",
    }
    setup_path = run_root / "receipts/setup.json"
    write_sealed(setup_path, payload)
    _write_exclusive(
        run_root / "receipts/setup.SUCCESS",
        _receipt_sentinel(setup_path, "FEREG_F0_SETUP_SUCCESS").encode("ascii"),
    )
    print("FEREG_F0_SETUP_SUCCESS", flush=True)


def _load_python_module(path: Path, name: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise SccContractError(f"cannot load exact harness module {path.name}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _support_modules(harness_source: Path) -> tuple[Any, Any, Any, Any]:
    root_text = str(harness_source)
    if root_text not in sys.path:
        sys.path.insert(0, root_text)
    reference = _load_python_module(harness_source / "scripts/ols_reference.py", "fereg_f0_exact_ols_reference")
    paired = _load_python_module(harness_source / "scripts/ols_paired.py", "fereg_f0_exact_ols_paired")
    evaluator = _load_python_module(harness_source / "scripts/fereg_f0_regression.py", "fereg_f0_exact_evaluator")
    routing = _load_python_module(
        harness_source / "scripts/run_fereg_f0_local.py",
        "fereg_f0_exact_routing",
    )
    return reference, paired, evaluator, routing


def _cell_for_task(manifest: Mapping[str, Any], tasks_path: Path, task_id: int) -> Mapping[str, Any]:
    lines = tasks_path.read_text(encoding="ascii").splitlines()
    governing_cells = scc_cells(manifest)
    if not lines or lines[0] != "task_id\tcell_id" or len(lines) != len(governing_cells) + 1:
        raise SccContractError("task manifest is incomplete")
    rows: dict[int, str] = {}
    for raw in lines[1:]:
        parts = raw.split("\t")
        if len(parts) != 2 or not parts[0].isdigit() or not CELL_ID_RE.fullmatch(parts[1]):
            raise SccContractError("task manifest row is malformed")
        index = int(parts[0])
        if index in rows:
            raise SccContractError("task manifest contains duplicate IDs")
        rows[index] = parts[1]
    expected = {index: cell["id"] for index, cell in enumerate(governing_cells, 1)}
    if rows != expected or task_id not in rows:
        raise SccContractError("task manifest does not close over the registered cells")
    return next(cell for cell in governing_cells if cell["id"] == rows[task_id])


def _replace_option(command: str, option: str, value: str) -> str:
    pattern = re.compile(rf"\b{re.escape(option)}\([^)]*\)")
    replacement = f"{option}({value})"
    return pattern.sub(replacement, command) if pattern.search(command) else f"{command} {replacement}"


def _cell_case(
    ols_manifest: Mapping[str, Any],
    cell: Mapping[str, Any],
    routing: Any,
) -> tuple[dict[str, Any], Mapping[str, Any], list[str]]:
    base_id = routing.base_case_id(cell)
    base = next((item for item in ols_manifest["benchmark_cases"] if item["id"] == base_id), None)
    if base is None:
        raise SccContractError(f"OLS baseline manifest lacks mapped case {base_id}")
    case = copy.deepcopy(base)
    case["id"] = cell["id"]
    try:
        command, saved = routing.build_cell_command(ols_manifest, cell)
        variant = routing.dataset_variant(cell)
    except (ValueError, KeyError, TypeError) as error:
        raise SccContractError(
            f"shared F0 cell routing rejected {cell['id']}: {error}"
        ) from error
    saved_variables = list(saved)
    case["arms"]["candidate"] = command
    # The SCC Stata 19 license exposes four Stata processors. Native fereg
    # threads remain the registered one/eight value in the command itself.
    case["thread_count"] = min(cell["threads"], SCC_STATA_PROCESSOR_CAP)
    dataset_id = {
        "dense": "generated_dense",
        "sparse": "generated_sparse",
        "qualified_dense": "generated_qualified_dense",
        "disconnected": "generated_disconnected",
    }[variant]
    base_dataset_id = (
        "generated_sparse"
        if variant in {"sparse", "disconnected"}
        else "generated_dense"
    )
    dataset = copy.deepcopy(
        next(
            item for item in ols_manifest["datasets"]
            if item["id"] == base_dataset_id
        )
    )
    dataset["id"] = dataset_id
    dataset["variant"] = variant
    case["dataset"] = dataset_id
    arguments = dataset.get("arguments")
    if not isinstance(arguments, list) or not arguments or arguments[0] != cell["n"]:
        raise SccContractError(f"mapped OLS dataset scale differs from cell {cell['id']}")
    return case, dataset, saved_variables


def _stata_quote(value: Path | str) -> str:
    text = str(value).replace("\\", "/")
    if any(token in text for token in ('"', "\n", "\r")):
        raise SccContractError("unsupported character in Stata path")
    return f'"{text}"'


def _inject_output_signature(source: str, signature_path: Path, variables: Sequence[str]) -> str:
    if not variables:
        return source
    marker = 'file write __frg_capture "END_FIXTURE|ols_v1_reference_capture|rc=0" _n'
    if source.count(marker) != 1:
        raise SccContractError("capture renderer terminal marker changed")
    names = " ".join(variables)
    lines = [
        f"capture confirm variable {names}",
        "if _rc exit _rc",
        f"quietly _datasignature obs_id {names}",
        "local __frg_f0_output_signature `\"`r(datasignature)'\"'",
        f"file open __frg_f0_output using {_stata_quote(signature_path)}, write text replace",
        f'file write __frg_f0_output "variables={names}" _n',
        "file write __frg_f0_output \"observations=`=_N'\" _n",
        "file write __frg_f0_output `\"signature=`__frg_f0_output_signature'\"' _n",
        "file close __frg_f0_output",
    ]
    return source.replace(marker, "\n".join(lines + [marker]))


def _inject_ereturn_inventory(source: str, signature_path: Path) -> str:
    marker = 'file write __frg_capture "END_FIXTURE|ols_v1_reference_capture|rc=0" _n'
    if source.count(marker) != 1:
        raise SccContractError("capture renderer terminal marker changed")
    lines = [
        "local __frg_f0_e_scalars : e(scalars)",
        "local __frg_f0_e_macros : e(macros)",
        "local __frg_f0_e_matrices : e(matrices)",
        f"file open __frg_f0_inventory using {_stata_quote(signature_path)}, write text replace",
        "file write __frg_f0_inventory `\"scalars=`__frg_f0_e_scalars'\"' _n",
        "file write __frg_f0_inventory `\"macros=`__frg_f0_e_macros'\"' _n",
        "file write __frg_f0_inventory `\"matrices=`__frg_f0_e_matrices'\"' _n",
        "file close __frg_f0_inventory",
    ]
    return source.replace(marker, "\n".join(lines + [marker]))


def _execute_fereg_run(
    *,
    reference: Any,
    paired: Any,
    stata: str,
    manifest: Mapping[str, Any],
    manifest_sha: str,
    run_case: Mapping[str, Any],
    dataset: Mapping[str, Any],
    data_path: Path,
    package_dir: Path,
    run_dir: Path,
    sysdirs: Path,
    output_variables: Sequence[str] = (),
) -> tuple[Mapping[str, Any], Mapping[str, Any], Mapping[str, str], str | None, str]:
    run_dir.mkdir(parents=True, exist_ok=False)
    unit_manifest = copy.deepcopy(manifest)
    unit_manifest["datasets"] = [{**dataset, "kind": "prepared_dta"}]
    unit_manifest["semantic_cases"] = []
    unit_manifest["benchmark_cases"] = [run_case]
    case_do = run_dir / "case.do"
    signature_path = run_dir / "output.signature"
    inventory_path = run_dir / "ereturn.inventory"
    rendered = reference.render_capture_do(unit_manifest, {dataset["id"]: data_path}, run_dir, manifest_sha)
    rendered = _inject_ereturn_inventory(rendered, inventory_path)
    case_do.write_text(_inject_output_signature(rendered, signature_path, output_variables), encoding="utf-8")
    capture_file = run_dir / "capture.machine"
    wrapper = run_dir / "run.do"
    paired._write_candidate_wrapper(wrapper, case_do, capture_file, package_dir, manifest_sha, sysdirs)
    completed, rss = reference._run_fresh_process(
        [stata, "-b", "do", str(wrapper)], run_dir,
        runner=_closed_stata_runner(_stata_environment(run_dir)),
    )
    _write_exclusive(
        run_dir / "process.stdout-stderr",
        str(completed.stdout or "").encode("utf-8"),
    )
    write_json_exclusive(
        run_dir / "invocation.json",
        {
            "schema_version": 1,
            "stata": {
                "path": stata, "sha256": sha256_file(Path(stata)),
                "size_bytes": Path(stata).stat().st_size,
            },
            "argv": [stata, "-b", "do", str(wrapper)],
            "cwd": str(run_dir),
            "environment": _stata_environment(run_dir),
            "stream": "merged-stdout-stderr-v1",
        },
    )
    log_path = run_dir / "run.log"
    log_text = log_path.read_text(encoding="utf-8", errors="replace") if log_path.is_file() else ""
    sentinels = [line.strip() for line in log_text.splitlines() if line.strip().startswith("END_FIXTURE|")]
    write_json_exclusive(
        run_dir / "process.json",
        {
            "schema_version": 1,
            "process_returncode": completed.returncode,
            "peak_rss": dict(rss),
            "terminal_sentinels": sentinels,
        },
    )
    if completed.returncode != 0 or sentinels != [paired.CANDIDATE_LOG_SENTINEL] or not capture_file.is_file():
        raise SccContractError(f"licensed Stata fereg child failed: {run_dir.name}")
    parsed = reference.parse_capture(capture_file.read_text(encoding="utf-8"), unit_manifest, manifest_sha)
    captured = parsed.cases[0]
    reference._validate_case_evidence(run_case, captured)
    output_sha = sha256_file(signature_path) if output_variables else None
    if not inventory_path.is_file():
        raise SccContractError("licensed Stata child omitted the ereturn inventory")
    return captured, rss, parsed.environment, output_sha, sha256_file(inventory_path)


def _matrix_digest(
    reference: Any,
    captured: Mapping[str, Any],
    name: str,
    *,
    excluded_columns: Sequence[int] = (),
) -> str:
    entries = reference.canonical_json_bytes(sorted(
        [
            entry
            for entry in captured["matrices"]
            if entry["name"] == name and entry["column"] not in excluded_columns
        ],
        key=lambda entry: (entry["row"], entry["column"]),
    ))
    return sha256_bytes(entries)


def _scientific_record(
    reference: Any,
    paired: Any,
    captured: Mapping[str, Any],
    outputs: Mapping[str, Any],
    ereturn_inventory_sha256: str,
) -> dict[str, Any]:
    basic = paired.scientific_evidence(captured, "candidate")
    certificates = basic["certificates"]
    reconstruction = max(
        abs(paired._matrix_value(captured, "fereg_certificate", row, 2))
        for row in range(9, 13)
    )
    ordered_matrix_hex = lambda name: [
        str(entry["value_hex"])
        for entry in sorted(
            [entry for entry in captured["matrices"] if entry["name"] == name],
            key=lambda entry: (entry["row"], entry["column"]),
        )
    ]
    exact_hex = {
        "coefficients": ordered_matrix_hex("b"),
        "vce": ordered_matrix_hex("V"),
        "fit": ordered_matrix_hex("fereg_fit"),
        "certificates": ordered_matrix_hex("fereg_certificate"),
    }
    fit = dict(basic["fit"])
    fit["F"] = paired.stata_hex_number(str(captured["scalars"]["F"]))
    return {
        "stata_rc": int(captured["return_code"]),
        "sample_identity_sha256": captured["identities"]["sample"]["sha256"],
        "singleton_identity_sha256": captured["identities"]["singletons"]["sha256"],
        "coefficient_names": basic["coefficient_names"],
        "coefficient_status": basic["coefficient_status"],
        "coefficients": basic["coefficients"],
        "vce_shape": [basic["vce"]["rows"], basic["vce"]["columns"]],
        "vce": basic["vce"]["values"],
        "numeric_hex_sha256": sha256_bytes(reference.canonical_json_bytes(exact_hex)),
        "dof": basic["dof"],
        "fit": fit,
        "outputs": dict(outputs),
        "solver": {
            # fereg_solver columns 9 and 10 are inherently environmental
            # timings. The exact scientific contract binds every other cell.
            "solver_sha256": _matrix_digest(
                reference,
                captured,
                "fereg_solver",
                excluded_columns=(9, 10),
            ),
            "status_sha256": _matrix_digest(reference, captured, "fereg_status"),
            "dof_sha256": _matrix_digest(reference, captured, "fereg_dof"),
            "requested_threads": round(paired._matrix_value(captured, "fereg_runtime", 28, 2)),
            "effective_threads": round(paired._matrix_value(captured, "fereg_runtime", 29, 2)),
            "requested_memory_code": round(paired._matrix_value(captured, "fereg_runtime", 1, 2)),
            "selected_memory_code": round(paired._matrix_value(captured, "fereg_runtime", 2, 2)),
            "ereturn_inventory_sha256": ereturn_inventory_sha256,
            "protocols": {
                name: captured["macros"][name]
                for name in ("fereg_action_protocol", "fereg_model_wire", "fereg_result_wire", "fereg_gate")
            },
        },
        "certificates": {
            "converged": bool(certificates["converged"]),
            "values": {
                "normal_equation_linf": abs(float(certificates["normal_equation_inf_norm"])),
                "fe_projection_linf": abs(float(certificates["fe_orthogonality_inf_norm"])),
                "vce_symmetry_linf": abs(float(certificates["vce_symmetry_inf_norm"])),
                "reconstruction_linf": float(reconstruction),
            },
            "digests": {
                "certificate_sha256": _matrix_digest(reference, captured, "fereg_certificate"),
                "receipt_sha256": _matrix_digest(reference, captured, "fereg_receipt"),
            },
        },
    }


def _allocation_classes(
    paired: Any,
    captured: Mapping[str, Any],
    scientific: Mapping[str, Any],
    observations: int,
) -> dict[str, int]:
    def matrix_sum(name: str, column: int) -> int:
        values = [entry for entry in captured["matrices"] if entry["name"] == name and entry["column"] == column]
        return sum(round(paired.stata_hex_number(str(entry["value_hex"]))) for entry in values)

    if isinstance(observations, bool) or not isinstance(observations, int) or observations < 1:
        raise SccContractError("physical dataset observation count is invalid")
    return {
        # e(N) is a weighted population for fweights.  The memory model is
        # intentionally bound to physical dataset rows instead.
        "observations": observations,
        "observation_columns": len(scientific["coefficient_names"]),
        "fe_levels": matrix_sum("fereg_fe", 4),
        "clusters": matrix_sum("fereg_cluster", 3),
    }


def _memory_record(
    paired: Any,
    captured: Mapping[str, Any],
    scientific: Mapping[str, Any],
    observations: int,
    retained: int,
    constrained: Mapping[str, Any] | None,
) -> dict[str, Any]:
    return {
        "native_allocation_bytes": round(paired._matrix_value(captured, "fereg_runtime", 13, 2)),
        "allocation_classes": _allocation_classes(
            paired, captured, scientific, observations
        ),
        "repeated_call_retained_bytes": retained,
        "constrained_memory": constrained,
    }


def _run_case_for_kind(paired: Any, case: Mapping[str, Any], kind: str) -> dict[str, Any]:
    run_case = paired._arm_case(case, "candidate", kind)
    for name in ("fereg_status", "fereg_fit", "fereg_dof", "fereg_fe", "fereg_cluster", "fereg_receipt"):
        if name not in run_case["capture"]["matrices"]:
            run_case["capture"]["matrices"].append(name)
    for name in ("fereg_action_protocol", "fereg_model_wire", "fereg_result_wire", "fereg_gate"):
        if name not in run_case["capture"]["macros"]:
            run_case["capture"]["macros"].append(name)
    return run_case


def paired_summary(pairs: Sequence[Mapping[str, Any]]) -> dict[str, float]:
    if len(pairs) != PAIR_COUNT:
        raise SccContractError("paired summary requires exactly nine pairs")
    logs = [
        math.log(float(item["candidate"]["seconds"]))
        - math.log(float(item["baseline"]["seconds"]))
        for item in pairs
    ]
    if any(not math.isfinite(value) for value in logs):
        raise SccContractError("paired timings are nonfinite")
    mean = statistics.fmean(logs)
    half_width = T95_DF8 * statistics.stdev(logs) / math.sqrt(len(logs))
    return {"ratio": math.exp(mean), "log_ci_lower": mean - half_width, "log_ci_upper": mean + half_width}


def confirmation_required(pairs: Sequence[Mapping[str, Any]]) -> bool:
    summary = paired_summary(pairs)
    return summary["ratio"] > 1.05 or summary["log_ci_lower"] > 0.0


def _physical_memory_bytes() -> int:
    meminfo = Path("/proc/meminfo")
    if meminfo.is_file():
        match = re.search(r"^MemTotal:\s+([0-9]+)\s+kB$", meminfo.read_text(encoding="ascii"), re.MULTILINE)
        if match:
            return int(match.group(1)) * 1024
    raise SccContractError("physical memory identity is unavailable")


def _cpu_identity() -> str:
    cpuinfo = Path("/proc/cpuinfo")
    if cpuinfo.is_file():
        match = re.search(r"^model name\s*:\s*(.+)$", cpuinfo.read_text(encoding="utf-8", errors="replace"), re.MULTILINE)
        if match:
            return match.group(1).strip()
    value = platform.processor().strip()
    if value:
        return value
    raise SccContractError("CPU identity is unavailable")


def _common_constrained_limit(paired: Any, captures: Mapping[str, Mapping[str, Any]]) -> int:
    low = max(round(paired._matrix_value(value, "fereg_runtime", 5, 2)) for value in captures.values())
    fast = min(round(paired._matrix_value(value, "fereg_runtime", 4, 2)) for value in captures.values())
    return paired.imposed_limit_between(low, fast)


def _constrained_arm(
    *,
    reference: Any,
    paired: Any,
    stata: str,
    manifest: Mapping[str, Any],
    manifest_sha: str,
    case: Mapping[str, Any],
    dataset: Mapping[str, Any],
    data_path: Path,
    package_dir: Path,
    work: Path,
    sysdirs_root: Path,
    limit: int,
) -> dict[str, Any]:
    work.mkdir(parents=True, exist_ok=False)
    write_json_exclusive(
        work / "limit.json", {"schema_version": 1, "limit_bytes": limit}
    )
    prior = os.environ.get("FEREG_MEMORY_LIMIT_BYTES")
    os.environ["FEREG_MEMORY_LIMIT_BYTES"] = str(limit)
    try:
        outcomes: dict[str, int] = {}
        for mode in ("low", "fast"):
            selected = copy.deepcopy(case)
            selected["arms"]["candidate"] = _replace_option(selected["arms"]["candidate"], "memory", mode)
            run_case = _run_case_for_kind(paired, selected, "cold")
            if mode == "fast":
                run_case["expected"] = "must_error"
            captured, _, _, _, _ = _execute_fereg_run(
                reference=reference,
                paired=paired,
                stata=stata,
                manifest=manifest,
                manifest_sha=manifest_sha,
                run_case=run_case,
                dataset=dataset,
                data_path=data_path,
                package_dir=package_dir,
                run_dir=work / mode,
                sysdirs=sysdirs_root / mode,
            )
            outcomes[mode] = int(captured["return_code"])
        if outcomes != {"low": 0, "fast": 909}:
            raise SccContractError("constrained memory proof returned unexpected codes")
        return {"limit_bytes": limit, "low": {"status": "success", "rc": 0}, "fast": {"status": "preflight_failure", "rc": 909}}
    finally:
        if prior is None:
            os.environ.pop("FEREG_MEMORY_LIMIT_BYTES", None)
        else:
            os.environ["FEREG_MEMORY_LIMIT_BYTES"] = prior


def _raw_inventory(root: Path) -> list[dict[str, Any]]:
    inventory: list[dict[str, Any]] = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise SccContractError("task raw evidence contains a symlink")
        if path.is_file():
            inventory.append(
                {
                    "path": path.relative_to(root).as_posix(),
                    "sha256": sha256_file(path),
                    "size_bytes": path.stat().st_size,
                }
            )
    if not inventory:
        raise SccContractError("task raw evidence inventory is empty")
    return inventory


def _verify_raw_inventory(
    task_root: Path, value: Any, cell_id: str
) -> list[dict[str, Any]]:
    if not isinstance(value, list) or not value:
        raise SccContractError(f"task {cell_id} raw evidence inventory is empty")
    expected = _raw_inventory(task_root)
    if value != expected:
        raise SccContractError(
            f"task {cell_id} raw evidence bytes or inventory changed"
        )
    return expected


def _reconstruct_raw_run(
    *,
    reference: Any,
    paired: Any,
    ols_manifest: Mapping[str, Any],
    manifest_sha: str,
    run_case: Mapping[str, Any],
    dataset: Mapping[str, Any],
    data_path: Path,
    run_dir: Path,
    package_dir: Path,
    sysdirs: Path,
    origin_root: Path,
    portable: bool,
    output_variables: Sequence[str],
) -> tuple[Mapping[str, Any], Mapping[str, Any], Mapping[str, str], str | None, str]:
    invocation = _expect_keys(
        _json(run_dir / "invocation.json"),
        {"schema_version", "stata", "argv", "cwd", "environment", "stream"},
        f"raw invocation {run_dir.name}",
    )
    expected_stata = str(SCC_STATA) if portable else _stata_executable()
    local_root = next(
        parent.parent for parent in run_dir.parents if parent.name == "work"
    )
    logical_suffix = run_dir.relative_to(local_root).as_posix()
    recorded_dir = Path(str(invocation["cwd"]))
    if not str(recorded_dir).endswith("/" + logical_suffix):
        raise SccContractError(f"raw Stata cwd suffix changed: {run_dir.name}")
    recorded_root = Path(str(recorded_dir)[: -(len(logical_suffix) + 1)])
    if recorded_root != origin_root:
        raise SccContractError(f"raw Stata origin differs from submission: {run_dir.name}")
    expected_argv = [expected_stata, "-b", "do", str(recorded_dir / "run.do")]
    local_environment = _stata_environment(run_dir, create=False)
    expected_environment = {
        key: value.replace(str(local_root), str(recorded_root))
        for key, value in local_environment.items()
    }
    if (
        invocation["schema_version"] != 1
        or invocation["stata"] != {
            "path": expected_stata, "sha256": SCC_STATA_SHA256,
            "size_bytes": SCC_STATA_SIZE,
        }
        or invocation["argv"] != expected_argv
        or invocation["cwd"] != str(recorded_dir)
        or invocation["environment"] != expected_environment
        or invocation["stream"] != "merged-stdout-stderr-v1"
    ):
        raise SccContractError(f"raw Stata invocation changed: {run_dir.name}")
    stream = (run_dir / "process.stdout-stderr").read_bytes()
    try:
        stream.decode("utf-8")
    except UnicodeDecodeError as error:
        raise SccContractError(f"raw Stata process stream is not UTF-8: {run_dir.name}") from error
    status = _json(run_dir / "process.json")
    _expect_keys(
        status,
        {
            "schema_version", "process_returncode", "peak_rss",
            "terminal_sentinels",
        },
        f"raw process status {run_dir.name}",
    )
    rss = _expect_keys(
        status["peak_rss"], {"value", "unit", "method"}, "raw peak RSS"
    )
    if (
        status["schema_version"] != 1
        or status["process_returncode"] != 0
        or status["terminal_sentinels"] != [paired.CANDIDATE_LOG_SENTINEL]
        or not isinstance(rss["value"], int)
        or rss["value"] <= 0
        or rss["unit"] != "bytes"
        or rss["method"] != "os_wait4_ru_maxrss"
    ):
        raise SccContractError(f"raw process status did not pass: {run_dir.name}")
    log_text = (run_dir / "run.log").read_text(
        encoding="utf-8", errors="replace"
    )
    sentinels = [
        line.strip()
        for line in log_text.splitlines()
        if line.strip().startswith("END_FIXTURE|")
    ]
    if sentinels != status["terminal_sentinels"]:
        raise SccContractError(f"raw log sentinel changed: {run_dir.name}")
    unit_manifest = copy.deepcopy(ols_manifest)
    unit_manifest["datasets"] = [{**dataset, "kind": "prepared_dta"}]
    unit_manifest["semantic_cases"] = []
    unit_manifest["benchmark_cases"] = [run_case]
    expected_case = reference.render_capture_do(
        unit_manifest, {dataset["id"]: data_path}, run_dir, manifest_sha
    )
    expected_case = _inject_ereturn_inventory(
        expected_case, run_dir / "ereturn.inventory"
    )
    expected_case = _inject_output_signature(
        expected_case, run_dir / "output.signature", output_variables
    ).replace(str(local_root), str(recorded_root)).encode("utf-8")
    if (run_dir / "case.do").read_bytes() != expected_case:
        raise SccContractError(f"raw case wrapper changed: {run_dir.name}")
    with tempfile.TemporaryDirectory() as temporary:
        expected_wrapper = Path(temporary) / "run.do"
        paired._write_candidate_wrapper(
            expected_wrapper, run_dir / "case.do", run_dir / "capture.machine",
            package_dir, manifest_sha, sysdirs,
        )
        expected_wrapper_bytes = expected_wrapper.read_bytes().replace(
            str(local_root).encode("utf-8"), str(recorded_root).encode("utf-8")
        )
        if (run_dir / "run.do").read_bytes() != expected_wrapper_bytes:
            raise SccContractError(f"raw candidate wrapper changed: {run_dir.name}")
    capture_path = run_dir / "capture.machine"
    parsed = reference.parse_capture(
        capture_path.read_text(encoding="utf-8"), unit_manifest, manifest_sha
    )
    if len(parsed.cases) != 1:
        raise SccContractError(f"raw capture case inventory changed: {run_dir.name}")
    captured = parsed.cases[0]
    reference._validate_case_evidence(run_case, captured)
    output_path = run_dir / "output.signature"
    if output_variables:
        if not output_path.is_file():
            raise SccContractError(f"raw saved-output signature is absent: {run_dir.name}")
        output_sha = sha256_file(output_path)
    else:
        if output_path.exists():
            raise SccContractError(f"raw fit-only run invented saved outputs: {run_dir.name}")
        output_sha = None
    inventory_path = run_dir / "ereturn.inventory"
    if not inventory_path.is_file():
        raise SccContractError(f"raw ereturn inventory is absent: {run_dir.name}")
    for required in ("case.do", "run.do", "capture.machine", "run.log"):
        if not (run_dir / required).is_file():
            raise SccContractError(f"raw run omits {required}: {run_dir.name}")
    return (
        captured,
        rss,
        parsed.environment,
        output_sha,
        sha256_file(inventory_path),
    )


def _reconstruct_task_cell(
    run_root: Path,
    manifest: Mapping[str, Any],
    setup: Mapping[str, Any],
    cell: Mapping[str, Any],
    task_id: int,
    origin_root: Path,
    portable: bool,
) -> dict[str, Any]:
    cell_id = str(cell["id"])
    task_root = run_root / "work/tasks" / f"task-{task_id:03d}-{cell_id}"
    harness_source = run_root / "work/setup/harness/source"
    baseline_source = run_root / "work/setup/baseline/source"
    reference, paired, evaluator, routing = _support_modules(harness_source)
    ols_manifest = _json(baseline_source / "benchmarks/manifests/ols_v1.json")
    case, dataset, saved_variables = _cell_case(ols_manifest, cell, routing)
    observations = dataset.get("arguments", [None])[0]
    if isinstance(observations, bool) or not isinstance(observations, int) or observations < 1:
        raise SccContractError("sealed dataset omits its physical observation count")
    data_path = run_root / "work/datasets" / f"{dataset['id']}.dta"
    dataset_receipt = setup["datasets"].get(dataset["id"])
    if (
        not isinstance(dataset_receipt, Mapping)
        or sha256_file(data_path) != dataset_receipt.get("sha256")
        or data_path.stat().st_size != dataset_receipt.get("size_bytes")
    ):
        raise SccContractError("raw task dataset differs from sealed setup")
    manifest_sha = sha256_file(run_root / "input/manifest.json")

    scientific: dict[str, Any] = {}
    preflight: dict[str, Mapping[str, Any]] = {}
    environments: dict[str, Mapping[str, str]] = {}
    for arm in ("baseline", "candidate"):
        run_case = _run_case_for_kind(paired, case, "cold")
        captured, _, environment, output_sha, inventory_sha = _reconstruct_raw_run(
            reference=reference,
            paired=paired,
            ols_manifest=ols_manifest,
            manifest_sha=manifest_sha,
            run_case=run_case,
            dataset=dataset,
            data_path=data_path,
            run_dir=task_root / "preflight" / arm,
            package_dir=run_root / f"work/bundles/{arm}/stata",
            sysdirs=task_root / "sysdirs/preflight" / arm,
            origin_root=origin_root,
            portable=portable,
            output_variables=saved_variables,
        )
        outputs = (
            {}
            if not saved_variables
            else {
                "saved_effects_sha256": output_sha,
                "saved_variables": saved_variables,
            }
        )
        scientific[arm] = _scientific_record(
            reference, paired, captured, outputs, inventory_sha
        )
        preflight[arm] = captured
        environments[arm] = environment
    if environments["baseline"] != environments["candidate"]:
        raise SccContractError("raw preflight environments differ")
    if evaluator.compare_scientific(
        manifest, scientific["baseline"], scientific["candidate"]
    ):
        raise SccContractError("raw preflight science differs")

    retained: dict[str, int] = {"baseline": 0, "candidate": 0}

    def reconstruct_pairs(label: str) -> list[dict[str, Any]]:
        pairs: list[dict[str, Any]] = []
        for index, order in enumerate(PAIR_ORDERS, 1):
            measurements: dict[str, Any] = {}
            arms = (
                ("baseline", "candidate")
                if order == "AB"
                else ("candidate", "baseline")
            )
            for arm in arms:
                run_case = _run_case_for_kind(paired, case, "warm")
                captured, rss, environment, output_sha, inventory_sha = (
                    _reconstruct_raw_run(
                        reference=reference,
                        paired=paired,
                        ols_manifest=ols_manifest,
                        manifest_sha=manifest_sha,
                        run_case=run_case,
                        dataset=dataset,
                        data_path=data_path,
                        run_dir=task_root / label / f"pair-{index:02d}-{arm}",
                        package_dir=run_root / f"work/bundles/{arm}/stata",
                        sysdirs=task_root / "sysdirs" / label / f"pair-{index:02d}-{arm}",
                        origin_root=origin_root,
                        portable=portable,
                        output_variables=saved_variables,
                    )
                )
                if environment != environments[arm]:
                    raise SccContractError("raw paired environment changed")
                outputs = (
                    {}
                    if not saved_variables
                    else {
                        "saved_effects_sha256": output_sha,
                        "saved_variables": saved_variables,
                    }
                )
                current = _scientific_record(
                    reference, paired, captured, outputs, inventory_sha
                )
                if evaluator.compare_scientific(manifest, scientific[arm], current):
                    raise SccContractError("raw paired science changed")
                previous = round(
                    paired._matrix_value(captured, "fereg_runtime", 10, 2)
                )
                current_baseline = round(
                    paired._matrix_value(captured, "fereg_runtime", 11, 2)
                )
                retained[arm] = max(
                    retained[arm], abs(current_baseline - previous)
                )
                seconds = paired.stata_hex_number(
                    str(captured["timings"][0]["seconds_hex"])
                )
                measurements[arm] = {
                    "seconds": seconds,
                    "peak_rss_bytes": int(rss["value"]),
                }
            pairs.append({"index": index, "order": order, **measurements})
        return pairs

    pairs = reconstruct_pairs("pairs")
    confirmation_root = task_root / "confirmation"
    required_confirmation = confirmation_required(pairs)
    if confirmation_root.exists() != required_confirmation:
        raise SccContractError("raw confirmation inventory contradicts threshold")
    confirmation = (
        reconstruct_pairs("confirmation") if required_confirmation else None
    )

    constrained: dict[str, Any] = {"baseline": None, "candidate": None}
    if cell_id == manifest["memory_rules"]["constrained_cell"]:
        for arm in ("baseline", "candidate"):
            limit_record = _json(task_root / "constrained" / arm / "limit.json")
            _expect_keys(
                limit_record,
                {"schema_version", "limit_bytes"},
                "raw constrained limit",
            )
            limit = limit_record["limit_bytes"]
            outcomes: dict[str, int] = {}
            for mode in ("low", "fast"):
                selected = copy.deepcopy(case)
                selected["arms"]["candidate"] = _replace_option(
                    selected["arms"]["candidate"], "memory", mode
                )
                run_case = _run_case_for_kind(paired, selected, "cold")
                if mode == "fast":
                    run_case["expected"] = "must_error"
                captured, _, _, _, _ = _reconstruct_raw_run(
                    reference=reference,
                    paired=paired,
                    ols_manifest=ols_manifest,
                    manifest_sha=manifest_sha,
                    run_case=run_case,
                    dataset=dataset,
                    data_path=data_path,
                    run_dir=task_root / "constrained" / arm / mode,
                    package_dir=run_root / f"work/bundles/{arm}/stata",
                    sysdirs=task_root / "sysdirs/constrained" / arm / mode,
                    origin_root=origin_root,
                    portable=portable,
                    output_variables=(),
                )
                outcomes[mode] = int(captured["return_code"])
            if outcomes != {"low": 0, "fast": 909}:
                raise SccContractError("raw constrained-memory outcomes changed")
            constrained[arm] = {
                "limit_bytes": limit,
                "low": {"status": "success", "rc": 0},
                "fast": {"status": "preflight_failure", "rc": 909},
            }

    environment = _json(task_root / "environment.json")
    _expect_keys(
        environment,
        set(manifest["evidence_contract"]["environment_fields"]),
        "raw task environment",
    )
    if (
        environment["stata_version"] != environments["baseline"]["stata_version"]
        or environment["stata_flavor"] != environments["baseline"]["stata_flavor"]
    ):
        raise SccContractError("raw task environment differs from Stata capture")
    return {
        "cell_id": cell_id,
        "environment": environment,
        "scientific": scientific,
        "pairs": pairs,
        "confirmation_pairs": confirmation,
        "memory": {
            arm: _memory_record(
                paired,
                preflight[arm],
                scientific[arm],
                observations,
                retained[arm],
                constrained[arm],
            )
            for arm in ("baseline", "candidate")
        },
    }


def task_run(run_root: Path, task_id: int) -> None:
    verify_input(run_root)
    preparation = read_sealed(run_root / "input/preparation.json", "fereg-f0-scc-preparation-v1")
    setup = read_sealed(run_root / "receipts/setup.json", "fereg-f0-scc-setup-v1")
    setup_path = run_root / "receipts/setup.json"
    setup_success = run_root / "receipts/setup.SUCCESS"
    if (
        not setup_success.is_file()
        or setup_success.read_text(encoding="ascii")
        != _receipt_sentinel(setup_path, "FEREG_F0_SETUP_SUCCESS")
        or setup["preparation_receipt_sha256"]
        != sha256_file(run_root / "input/preparation.json")
    ):
        raise SccContractError("setup did not publish a matching success receipt")
    _verify_setup_smoke(run_root, setup)
    manifest_path = run_root / "input/manifest.json"
    manifest = load_regression_manifest(manifest_path)
    if _manifest_lifecycle(manifest) == "candidate_unsealed":
        raise SccContractError(
            "candidate_unsealed permits setup artifact discovery only; seal artifacts before tasks"
        )
    manifest_sha = sha256_file(manifest_path)
    if manifest_sha != preparation["manifest"]["sha256"] or manifest_sha != setup["manifest"]["sha256"]:
        raise SccContractError("task manifest identity mismatch")
    submission = validate_submission_binding(run_root, preparation)
    cell = _cell_for_task(manifest, run_root / "input/tasks.tsv", task_id)
    if int(os.environ.get("NSLOTS", "1")) < int(cell["threads"]):
        raise SccContractError("SGE allocation is smaller than the registered native thread count")
    cell_id = str(cell["id"])
    expected_job = submission["jobs"]["array"]
    if os.environ.get("JOB_ID", "") != expected_job:
        raise SccContractError("task scheduler job ID differs from the sealed submission")
    task_root = run_root / "work/tasks" / f"task-{task_id:03d}-{cell_id}"
    task_root.mkdir(parents=True, exist_ok=False)

    baseline_source = run_root / "work/setup/baseline/source"
    harness_source = run_root / "work/setup/harness/source"
    reference, paired, evaluator, routing = _support_modules(harness_source)
    ols_manifest = _json(baseline_source / "benchmarks/manifests/ols_v1.json")
    case, dataset, saved_variables = _cell_case(ols_manifest, cell, routing)
    observations = dataset.get("arguments", [None])[0]
    if isinstance(observations, bool) or not isinstance(observations, int) or observations < 1:
        raise SccContractError("sealed dataset omits its physical observation count")
    data_path = run_root / "work/datasets" / f"{dataset['id']}.dta"
    dataset_receipt = setup["datasets"][dataset["id"]]
    if sha256_file(data_path) != dataset_receipt["sha256"]:
        raise SccContractError("prepared benchmark dataset changed")
    stata = _stata_executable()
    bundle_roots = {arm: run_root / f"work/bundles/{arm}" for arm in ("baseline", "candidate")}
    for arm, bundle in bundle_roots.items():
        _verify_staged_bundle(
            bundle,
            setup["arms"][arm],
            run_root / "input" / preparation["arms"][arm]["archive"],
        )
    packages = {arm: bundle / "stata" for arm, bundle in bundle_roots.items()}
    scientific: dict[str, Any] = {}
    preflight_captures: dict[str, Mapping[str, Any]] = {}
    environments: dict[str, Mapping[str, str]] = {}
    for arm in ("baseline", "candidate"):
        run_case = _run_case_for_kind(paired, case, "cold")
        captured, _, environment, output_sha, inventory_sha = _execute_fereg_run(
            reference=reference,
            paired=paired,
            stata=stata,
            manifest=ols_manifest,
            manifest_sha=manifest_sha,
            run_case=run_case,
            dataset=dataset,
            data_path=data_path,
            package_dir=packages[arm],
            run_dir=task_root / "preflight" / arm,
            sysdirs=task_root / "sysdirs/preflight" / arm,
            output_variables=saved_variables,
        )
        if saved_variables and output_sha is None:
            raise SccContractError("saved-output preflight omitted its identity")
        outputs = {} if not saved_variables else {"saved_effects_sha256": output_sha, "saved_variables": saved_variables}
        scientific[arm] = _scientific_record(reference, paired, captured, outputs, inventory_sha)
        preflight_captures[arm] = captured
        environments[arm] = environment
    if environments["baseline"] != environments["candidate"]:
        raise SccContractError("preflight arms used different Stata environments")
    mismatches = evaluator.compare_scientific(manifest, scientific["baseline"], scientific["candidate"])
    if mismatches:
        failure = {"cell_id": cell_id, "stage": "scientific_preflight", "mismatches": mismatches}
        write_json_exclusive(run_root / f"output/cells/{cell_id}.scientific_failure.json", failure)
        raise SccContractError("scientific preflight failed before timing: " + ",".join(mismatches))

    retained: dict[str, int] = {"baseline": 0, "candidate": 0}

    def run_pairs(label: str) -> list[dict[str, Any]]:
        pairs: list[dict[str, Any]] = []
        for index, order in enumerate(PAIR_ORDERS, 1):
            measurements: dict[str, Any] = {}
            for arm in (("baseline", "candidate") if order == "AB" else ("candidate", "baseline")):
                run_case = _run_case_for_kind(paired, case, "warm")
                captured, rss, environment, output_sha, inventory_sha = _execute_fereg_run(
                    reference=reference,
                    paired=paired,
                    stata=stata,
                    manifest=ols_manifest,
                    manifest_sha=manifest_sha,
                    run_case=run_case,
                    dataset=dataset,
                    data_path=data_path,
                    package_dir=packages[arm],
                    run_dir=task_root / label / f"pair-{index:02d}-{arm}",
                    sysdirs=task_root / "sysdirs" / label / f"pair-{index:02d}-{arm}",
                    output_variables=saved_variables,
                )
                if environment != environments[arm]:
                    raise SccContractError("Stata environment changed within a paired cell")
                if saved_variables and output_sha is None:
                    raise SccContractError("timed saved-output call omitted its identity")
                outputs = (
                    {}
                    if not saved_variables
                    else {"saved_effects_sha256": output_sha, "saved_variables": saved_variables}
                )
                current = _scientific_record(
                    reference,
                    paired,
                    captured,
                    outputs,
                    inventory_sha,
                )
                drift = evaluator.compare_scientific(manifest, scientific[arm], current)
                if drift:
                    raise SccContractError(f"{arm} science changed during timing: {','.join(drift)}")
                previous = round(paired._matrix_value(captured, "fereg_runtime", 10, 2))
                current_baseline = round(paired._matrix_value(captured, "fereg_runtime", 11, 2))
                retained[arm] = max(retained[arm], abs(current_baseline - previous))
                seconds = paired.stata_hex_number(str(captured["timings"][0]["seconds_hex"]))
                if seconds <= 0 or int(rss["value"]) <= 0:
                    raise SccContractError("paired measurement is nonpositive")
                measurements[arm] = {"seconds": seconds, "peak_rss_bytes": int(rss["value"])}
            pairs.append({"index": index, "order": order, **measurements})
        return pairs

    pairs = run_pairs("pairs")
    confirmation = run_pairs("confirmation") if confirmation_required(pairs) else None

    constrained: dict[str, Any] = {"baseline": None, "candidate": None}
    if cell_id == manifest["memory_rules"]["constrained_cell"]:
        limit = _common_constrained_limit(paired, preflight_captures)
        for arm in ("baseline", "candidate"):
            constrained[arm] = _constrained_arm(
                reference=reference,
                paired=paired,
                stata=stata,
                manifest=ols_manifest,
                manifest_sha=manifest_sha,
                case=case,
                dataset=dataset,
                data_path=data_path,
                package_dir=packages[arm],
                work=task_root / "constrained" / arm,
                sysdirs_root=task_root / "sysdirs/constrained" / arm,
                limit=limit,
            )

    environment = {
        "host_id": socket.gethostname(),
        "os": platform.platform(),
        "arch": platform.machine(),
        "cpu": _cpu_identity(),
        "stata_version": environments["baseline"]["stata_version"],
        "stata_flavor": environments["baseline"]["stata_flavor"],
        "toolchain": "; ".join(f"{key}={value}" for key, value in sorted(setup["toolchain"].items())),
        "physical_memory_bytes": _physical_memory_bytes(),
    }
    cell_evidence = {
        "cell_id": cell_id,
        "environment": environment,
        "scientific": scientific,
        "pairs": pairs,
        "confirmation_pairs": confirmation,
        "memory": {
            arm: _memory_record(
                paired,
                preflight_captures[arm],
                scientific[arm],
                observations,
                retained[arm],
                constrained[arm],
            )
            for arm in ("baseline", "candidate")
        },
    }
    write_json_exclusive(task_root / "environment.json", environment)
    reconstructed = _reconstruct_task_cell(
        run_root, manifest, setup, cell, task_id,
        _submitted_root(submission["submission_ledger"]["submit_cwd"], run_root.name),
        False,
    )
    if reconstructed != cell_evidence:
        raise SccContractError(
            "live task evidence is not reproduced by retained raw outputs"
        )
    raw_files = _raw_inventory(task_root)
    cell_path = run_root / f"output/cells/{cell_id}.json"
    write_json_exclusive(cell_path, cell_evidence)
    payload = {
        "schema_version": RECEIPT_SCHEMA,
        "receipt_type": "fereg-f0-scc-task-v1",
        "run_id": run_root.name,
        "task_id": task_id,
        "cell_id": cell_id,
        "manifest": dict(preparation["manifest"]),
        "setup_receipt_sha256": sha256_file(run_root / "receipts/setup.json"),
        "dataset": {"id": dataset["id"], **dict(dataset_receipt)},
        "arms": {arm: setup["arms"][arm] for arm in ("baseline", "candidate")},
        "environment": environment,
        "raw_files": raw_files,
        "cell_sha256": sha256_file(cell_path),
        "sge": {"job_id": os.environ.get("JOB_ID", ""), "task_id": os.environ.get("SGE_TASK_ID", str(task_id)), "host_id": socket.gethostname(), "slots": int(os.environ.get("NSLOTS", "1"))},
        "terminal_sentinel": f"FEREG_F0_TASK_SUCCESS|{cell_id}",
    }
    receipt_path = run_root / f"receipts/tasks/task-{task_id:03d}.json"
    write_sealed(receipt_path, payload)
    _write_exclusive(
        run_root / f"receipts/tasks/task-{task_id:03d}.SUCCESS",
        _receipt_sentinel(
            receipt_path, f"FEREG_F0_TASK_SUCCESS|{cell_id}"
        ).encode("ascii"),
    )
    print(f"FEREG_F0_TASK_SUCCESS|{cell_id}", flush=True)


def validate_task_inventory(
    run_root: Path,
    manifest: Mapping[str, Any],
    setup: Mapping[str, Any],
    submission: Mapping[str, Any] | None = None,
) -> list[Mapping[str, Any]]:
    cells: list[Mapping[str, Any]] = []
    if submission is None:
        preparation = read_sealed(
            run_root / "input/preparation.json", "fereg-f0-scc-preparation-v1"
        )
        submission = validate_submission_binding(run_root, preparation)
    expected_ids = [cell["id"] for cell in scc_cells(manifest)]
    for task_id, cell_id in enumerate(expected_ids, 1):
        receipt_path = run_root / f"receipts/tasks/task-{task_id:03d}.json"
        success_path = run_root / f"receipts/tasks/task-{task_id:03d}.SUCCESS"
        payload = read_sealed(receipt_path, "fereg-f0-scc-task-v1")
        _expect_keys(
            payload,
            {
                "schema_version", "receipt_type", "run_id", "task_id",
                "cell_id", "manifest", "setup_receipt_sha256", "dataset",
                "arms", "environment", "raw_files", "cell_sha256", "sge",
                "terminal_sentinel",
            },
            f"task {task_id} receipt",
        )
        if (
            payload.get("run_id") != run_root.name
            or payload.get("task_id") != task_id
            or payload.get("cell_id") != cell_id
        ):
            raise SccContractError("task receipt ID mismatch")
        if payload.get("manifest") != setup["manifest"]:
            raise SccContractError("task receipt binds a different manifest")
        if payload.get("setup_receipt_sha256") != sha256_file(run_root / "receipts/setup.json"):
            raise SccContractError("task receipt binds a different setup")
        expected_job = submission["jobs"]["array"]
        if (
            payload.get("sge", {}).get("job_id") != expected_job
            or str(payload.get("sge", {}).get("task_id")) != str(task_id)
        ):
            raise SccContractError("task receipt scheduler identity differs from submission")
        expected_sentinel = _receipt_sentinel(
            receipt_path, f"FEREG_F0_TASK_SUCCESS|{cell_id}"
        )
        if not success_path.is_file() or success_path.read_text(encoding="ascii") != expected_sentinel:
            raise SccContractError("task terminal sentinel is missing or malformed")
        cell_path = run_root / f"output/cells/{cell_id}.json"
        if sha256_file(cell_path) != payload.get("cell_sha256"):
            raise SccContractError("task cell evidence hash mismatch")
        evidence = _json(cell_path)
        if not isinstance(evidence, Mapping) or evidence.get("cell_id") != cell_id:
            raise SccContractError("task cell evidence identity mismatch")
        if payload.get("environment") != evidence.get("environment"):
            raise SccContractError("task receipt environment differs from its evidence")
        if payload.get("sge", {}).get("host_id") != evidence.get("environment", {}).get("host_id"):
            raise SccContractError("task scheduler host differs from its evidence host")
        for arm in ("baseline", "candidate"):
            if payload["arms"][arm] != setup["arms"][arm]:
                raise SccContractError("task used a different source or artifact")
        dataset_id = payload.get("dataset", {}).get("id")
        if setup.get("datasets") and (
            not isinstance(dataset_id, str)
            or dataset_id not in setup["datasets"]
            or payload.get("dataset")
            != {"id": dataset_id, **dict(setup["datasets"][dataset_id])}
        ):
            raise SccContractError("task receipt binds a different prepared dataset")
        cell = next(item for item in scc_cells(manifest) if item["id"] == cell_id)
        task_root = run_root / "work/tasks" / f"task-{task_id:03d}-{cell_id}"
        _verify_raw_inventory(task_root, payload["raw_files"], cell_id)
        reconstructed = _reconstruct_task_cell(
            run_root, manifest, setup, cell, task_id,
            _submitted_root(submission["submission_ledger"]["submit_cwd"], run_root.name),
            True,
        )
        if reconstructed != evidence:
            raise SccContractError(
                "task cell evidence is not reproduced by retained raw outputs"
            )
        cells.append(evidence)
    actual_receipts = {path.name for path in (run_root / "receipts/tasks").glob("task-*.json")}
    expected_receipts = {f"task-{index:03d}.json" for index in range(1, len(expected_ids) + 1)}
    if actual_receipts != expected_receipts:
        raise SccContractError("task receipt inventory contains missing or unknown shards")
    actual_success = {path.name for path in (run_root / "receipts/tasks").glob("task-*.SUCCESS")}
    expected_success = {f"task-{index:03d}.SUCCESS" for index in range(1, len(expected_ids) + 1)}
    if actual_success != expected_success:
        raise SccContractError("task success inventory contains missing or unknown shards")
    actual_cells = {path.name for path in (run_root / "output/cells").glob("*.json")}
    expected_cells = {f"{cell_id}.json" for cell_id in expected_ids}
    if actual_cells != expected_cells:
        raise SccContractError("cell evidence inventory contains missing or unknown shards")
    return cells


def _validate_behavior_arm_binding(
    observation: Mapping[str, Any], setup_arm: Mapping[str, Any], arm: str
) -> None:
    if observation.get("source") != setup_arm["source"]:
        raise SccContractError(
            f"governing behavior source differs from SCC arm: {arm}"
        )
    artifact = observation.get("artifact")
    if (
        not isinstance(artifact, Mapping)
        or artifact.get("platform") == setup_arm["artifact"].get("platform")
    ):
        raise SccContractError(
            f"governing behavior artifact is not a distinct native-platform identity: {arm}"
        )


def assemble_evidence(run_root: Path) -> dict[str, Any]:
    preparation = read_sealed(run_root / "input/preparation.json", "fereg-f0-scc-preparation-v1")
    setup = read_sealed(run_root / "receipts/setup.json", "fereg-f0-scc-setup-v1")
    manifest_path = run_root / "input/manifest.json"
    manifest = load_regression_manifest(manifest_path)
    lifecycle = _manifest_lifecycle(manifest)
    if lifecycle == "candidate_unsealed":
        raise SccContractError(
            "candidate_unsealed cannot collect a governing matrix; seal artifacts first"
        )
    if sha256_file(manifest_path) != preparation["manifest"]["sha256"] or setup["manifest"] != preparation["manifest"]:
        raise SccContractError("collector manifest identities disagree")
    submission = validate_submission_binding(run_root, preparation)
    cells = validate_task_inventory(run_root, manifest, setup, submission)
    behavior_records = preparation.get("behavior")
    if lifecycle == "artifacts_sealed":
        if behavior_records is not None:
            raise SccContractError(
                "diagnostic F0-versus-F0 run contains synthetic candidate behavior"
            )
        return {
            "schema_version": 1,
            "evidence_type": EVIDENCE_TYPE,
            "profile": "scc_governing",
            "manifest": dict(preparation["manifest"]),
            "baseline": {
                "source": setup["arms"]["baseline"]["source"],
                "artifact": setup["arms"]["baseline"]["artifact"],
            },
            "candidate": {
                "source": setup["arms"]["candidate"]["source"],
                "artifact": setup["arms"]["candidate"]["artifact"],
            },
            "behavior": None,
            "cells": cells,
        }
    if not isinstance(behavior_records, Mapping):
        raise SccContractError("governing evidence omits direct behavior receipts")
    expected_behavior = {
        "baseline_observation", "baseline_bundle", "candidate_observation",
        "candidate_bundle", "comparison",
    }
    if set(behavior_records) != expected_behavior:
        raise SccContractError("governing behavior receipt/bundle inventory is incomplete")
    paths = {
        name: run_root / "input" / str(behavior_records[name]["path"])
        for name in expected_behavior
    }
    harness_source = run_root / "work/setup/harness/source"
    behavior_module = _load_python_module(
        harness_source / "scripts/fereg_f0_behavior.py",
        "fereg_f0_exact_behavior",
    )
    behavior_manifest = (
        harness_source / "benchmarks/manifests/fereg_f0_behavior_v1.json"
    )
    try:
        verified_comparison = behavior_module.verify_comparison(
            paths["comparison"],
            paths["baseline_observation"],
            paths["baseline_bundle"],
            paths["candidate_observation"],
            paths["candidate_bundle"],
            behavior_manifest,
        )
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise SccContractError(
            f"governing behavior evidence failed authoritative verification: {error}"
        ) from error
    expected_controller = preparation["harness_source"]
    for arm, observation_name in (
        ("baseline", "baseline_observation"),
        ("candidate", "candidate_observation"),
    ):
        observation = _json(paths[observation_name]).get("payload", {})
        _validate_behavior_arm_binding(observation, setup["arms"][arm], arm)
        controller = observation.get("controller_source", {})
        for field in (
            "commit", "tree", "cargo_lock_sha256", "references_lock_sha256",
            "archive_sha256", "archive_size_bytes",
        ):
            if controller.get(field) != expected_controller.get(field):
                raise SccContractError(
                    f"governing behavior controller differs from harness: {observation_name} {field}"
                )

    def binding(name: str, *, sealed: bool = False) -> dict[str, Any]:
        path = paths[name]
        record: dict[str, Any] = {
            "path": path.relative_to(run_root).as_posix(),
            "sha256": sha256_file(path),
            "size_bytes": path.stat().st_size,
        }
        if sealed:
            document = _json(path)
            if not isinstance(document, Mapping) or set(document) != {"payload", "seal"}:
                raise SccContractError(f"governing behavior receipt is invalid: {name}")
            seal = document.get("seal")
            if not isinstance(seal, Mapping) or SHA256_RE.fullmatch(
                str(seal.get("payload_sha256"))
            ) is None:
                raise SccContractError(f"governing behavior receipt seal is invalid: {name}")
            record["payload_sha256"] = seal["payload_sha256"]
        return record

    behavior = {
        "baseline_observation": binding("baseline_observation", sealed=True),
        "baseline_bundle": binding("baseline_bundle"),
        "candidate_observation": binding("candidate_observation", sealed=True),
        "candidate_bundle": binding("candidate_bundle"),
        "comparison": binding("comparison", sealed=True),
        "verified_comparison": dict(verified_comparison),
    }
    return {
        "schema_version": 1,
        "evidence_type": EVIDENCE_TYPE,
        "profile": "scc_governing",
        "manifest": dict(preparation["manifest"]),
        "baseline": {"source": setup["arms"]["baseline"]["source"], "artifact": setup["arms"]["baseline"]["artifact"]},
        "candidate": {"source": setup["arms"]["candidate"]["source"], "artifact": setup["arms"]["candidate"]["artifact"]},
        "behavior": behavior,
        "cells": cells,
    }


def collect_run(run_root: Path) -> None:
    evidence = assemble_evidence(run_root)
    preparation = read_sealed(
        run_root / "input/preparation.json", "fereg-f0-scc-preparation-v1"
    )
    submission = validate_submission_binding(run_root, preparation)
    if os.environ.get("JOB_ID", "") != submission["jobs"]["collector"]:
        raise SccContractError("collector scheduler job ID differs from submission")
    evidence_path = run_root / "output/evidence.json"
    write_json_exclusive(evidence_path, evidence)
    harness_source = run_root / "work/setup/harness/source"
    evaluator = harness_source / "scripts/fereg_f0_regression.py"
    committed_manifest = harness_source / preparation["harness_manifest_path"]
    if (
        not committed_manifest.is_file()
        or committed_manifest.read_bytes()
        != (run_root / "input/manifest.json").read_bytes()
    ):
        raise SccContractError(
            "run manifest differs from the exact candidate archive"
        )
    receipt = run_root / "output/qualification.json"
    completed = subprocess.run(
        [
            sys.executable,
            str(evaluator),
            "evaluate",
            "--manifest",
            str(run_root / "input/manifest.json"),
            "--evidence",
            str(evidence_path),
            "--output",
            str(receipt),
            "--exact-archive-root",
            str(harness_source),
            "--evidence-root",
            str(run_root),
            "--seal-root",
            str(run_root / "input/seal-evidence"),
        ],
        cwd=harness_source,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    (run_root / "logs/collector-evaluator.log").write_text(completed.stdout, encoding="utf-8")
    manifest = load_regression_manifest(run_root / "input/manifest.json")
    lifecycle = _manifest_lifecycle(manifest)
    if not receipt.is_file():
        raise SccContractError("platform-independent F0 evaluator omitted its receipt")
    qualification = read_evaluator_receipt(receipt)
    evaluation = qualification.get("evaluation")
    if not isinstance(evaluation, Mapping):
        raise SccContractError("platform-independent F0 evaluator omitted its verdict")
    if lifecycle == "artifacts_sealed":
        expected_rc = 2
        expected_status = "blocked"
        expected_reasons = ["baseline_candidate_unsealed"]
        collector_sentinel = "FEREG_F0_COLLECTOR_BASELINE_EVIDENCE_COMPLETE"
    elif lifecycle == "frozen":
        expected_rc = 0
        expected_status = "pass"
        expected_reasons = []
        collector_sentinel = "FEREG_F0_COLLECTOR_SUCCESS"
    else:
        raise SccContractError("candidate_unsealed cannot publish collector evidence")
    if (
        completed.returncode != expected_rc
        or evaluation.get("status") != expected_status
        or evaluation.get("reasons") != expected_reasons
        or evaluation.get("profile") != "scc_governing"
    ):
        raise SccContractError("platform-independent F0 evaluator returned the wrong lifecycle verdict")
    payload = {
        "schema_version": RECEIPT_SCHEMA,
        "receipt_type": "fereg-f0-scc-collector-v1",
        "run_id": run_root.name,
        "manifest": evidence["manifest"],
        "cell_count": len(evidence["cells"]),
        "evidence_sha256": sha256_file(evidence_path),
        "qualification_sha256": sha256_file(receipt),
        "qualification_status": expected_status,
        "qualification_reasons": expected_reasons,
        "sge": {"job_id": os.environ.get("JOB_ID", ""), "host_id": socket.gethostname(), "slots": int(os.environ.get("NSLOTS", "1"))},
        "terminal_sentinel": collector_sentinel,
    }
    collector_path = run_root / "receipts/collector.json"
    write_sealed(collector_path, payload)
    _write_exclusive(
        run_root / "receipts/collector.COMPLETE",
        _receipt_sentinel(collector_path, collector_sentinel).encode("ascii"),
    )
    if lifecycle == "frozen":
        _write_exclusive(
            run_root / "receipts/collector.SUCCESS",
            _receipt_sentinel(
                collector_path, "FEREG_F0_COLLECTOR_SUCCESS"
            ).encode("ascii"),
        )
    print(collector_sentinel, flush=True)


def record_submission(run_root: Path) -> None:
    preparation = read_sealed(run_root / "input/preparation.json", "fereg-f0-scc-preparation-v1")
    _verify_scheduler_client("qsub")
    _verify_scheduler_client("qstat")
    submission_ledger, jobs = _submission_ledger(run_root)
    payload = {
        "schema_version": RECEIPT_SCHEMA,
        "receipt_type": "fereg-f0-scc-submission-v1",
        "run_id": run_root.name,
        "owner": "johannes",
        "project": "welfgr",
        "manifest": dict(preparation["manifest"]),
        "preparation_receipt_sha256": sha256_file(
            run_root / "input/preparation.json"
        ),
        "jobs": jobs,
        "job_names": _submission_job_names(run_root.name),
        "scripts": _submitted_script_records(run_root),
        "array_tasks": preparation["tasks"]["count"],
        "resources": {
            "setup": {"slots": 16, "mem_per_core": "4G", "h_rt": "04:00:00"},
            "array": {"slots": 16, "mem_per_core": "4G", "h_rt": "11:45:00"},
            "collector": {"slots": 1, "mem_per_core": "4G", "h_rt": "00:30:00"},
        },
        "submission_ledger": submission_ledger,
    }
    write_sealed(run_root / "receipts/submission.json", payload)


def parse_qacct(text: str) -> list[dict[str, str]]:
    records: list[dict[str, str]] = []
    current: dict[str, str] = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line:
            continue
        if set(line) == {"="}:
            if current:
                records.append(current)
                current = {}
            continue
        parts = line.split(None, 1)
        if len(parts) == 2:
            key, value = parts
            if key in current:
                raise SccContractError(f"duplicate qacct field {key}")
            current[key] = value.strip()
    if current:
        records.append(current)
    return [record for record in records if "jobnumber" in record]


def validate_qacct(
    text: str,
    job_id: str,
    task_ids: Sequence[int] | None,
    *,
    expected_slots: int,
    expected_job_name: str,
    expected_script: Path,
    expected_run_root: Path,
    expected_argv: Sequence[str],
    submission_started_at_utc: str,
    submission_completed_at_utc: str,
) -> list[dict[str, str]]:
    records = parse_qacct(text)
    selected = [record for record in records if record.get("jobnumber") == job_id]
    if not selected:
        raise SccContractError(f"qacct has no record for job {job_id}")
    if len(selected) != len(records):
        raise SccContractError(f"qacct for job {job_id} contains unrelated job records")
    if task_ids is None:
        if (
            len(selected) != 1
            or selected[0].get("taskid") not in {None, "undefined"}
        ):
            raise SccContractError(f"scalar job {job_id} has ambiguous qacct records")
    else:
        observed: list[int] = []
        for record in selected:
            raw = record.get("taskid", "")
            if not raw.isdigit():
                raise SccContractError(f"array job {job_id} has malformed task accounting")
            observed.append(int(raw))
        if sorted(observed) != sorted(task_ids) or len(observed) != len(set(observed)):
            raise SccContractError(f"array job {job_id} accounting inventory is incomplete")
    for record in selected:
        if (
            record.get("owner") != "johannes"
            or record.get("project") != "welfgr"
            or record.get("jobname") != expected_job_name
            or record.get("failed") != "0"
            or record.get("exit_status") != "0"
        ):
            raise SccContractError(f"job {job_id} has failed or wrong-project accounting")
        for required in (
            "hostname",
            "qname",
            "slots",
            "qsub_time",
            "start_time",
            "end_time",
            "ru_wallclock",
            "cpu",
            "maxvmem",
        ):
            if not record.get(required):
                raise SccContractError(f"job {job_id} qacct omits {required}")
        if record["slots"] != str(expected_slots):
            raise SccContractError(f"job {job_id} qacct slot count differs from submission")
        try:
            times = [
                datetime.strptime(record[field], "%a %b %d %H:%M:%S %Y")
                for field in ("qsub_time", "start_time", "end_time")
            ]
        except ValueError as error:
            raise SccContractError(f"job {job_id} qacct timestamps are malformed") from error
        if not times[0] <= times[1] <= times[2]:
            raise SccContractError(f"job {job_id} qacct timestamps are out of order")
        qsub_utc = times[0].replace(
            tzinfo=ZoneInfo("America/New_York")
        ).astimezone(timezone.utc)
        submitted_start = _parse_utc_timestamp(
            submission_started_at_utc, "submission start"
        )
        submitted_end = _parse_utc_timestamp(
            submission_completed_at_utc, "submission completion"
        )
        if not submitted_start <= qsub_utc <= submitted_end:
            raise SccContractError(
                f"job {job_id} qacct qsub_time falls outside the sealed submission window"
            )
        submitted_root = _canonical_absolute_path(
            expected_run_root, "expected qacct run root"
        )
        submitted_script = _canonical_absolute_path(
            expected_script, "expected qacct script"
        )
        if "cwd" in record and record["cwd"] != str(submitted_root):
            raise SccContractError(f"job {job_id} qacct cwd differs from its run")
        if "scriptfile" in record and record["scriptfile"] != str(submitted_script):
            raise SccContractError(f"job {job_id} qacct script differs from submission")
        command_fields = [
            field
            for field in (
                "submit_cmd_line", "submit_cmdline", "submission_command"
            )
            if field in record
        ]
        if len(command_fields) > 1:
            raise SccContractError(
                f"job {job_id} qacct exposes duplicate submission commands"
            )
        if command_fields:
            command = record[command_fields[0]]
            try:
                tokens = shlex.split(command)
            except ValueError as error:
                raise SccContractError(f"job {job_id} qacct command is malformed") from error
            if tokens != list(expected_argv):
                raise SccContractError(
                    f"job {job_id} qacct command differs from submission"
                )
        try:
            wallclock = float(record["ru_wallclock"])
            cpu = float(record["cpu"])
        except ValueError as error:
            raise SccContractError(f"job {job_id} qacct resources are malformed") from error
        if (
            not math.isfinite(wallclock)
            or not math.isfinite(cpu)
            or wallclock < 0.0
            or cpu < 0.0
            or re.fullmatch(
                r"[0-9]+(?:\.[0-9]+)?(?:[KMGTPE](?:B)?|B)?",
                record["maxvmem"],
            )
            is None
        ):
            raise SccContractError(f"job {job_id} qacct resources are malformed")
    return selected


def _fetch_qacct(
    job_id: str,
    task_ids: Sequence[int] | None,
    *,
    expected_slots: int,
    expected_job_name: str,
    expected_script: Path,
    expected_run_root: Path,
    expected_argv: Sequence[str],
    submission_started_at_utc: str,
    submission_completed_at_utc: str,
    attempts: int = 30,
    delay_seconds: float = 10.0,
) -> str:
    last_error: SccContractError | None = None
    for attempt in range(attempts):
        _verify_scheduler_client("qacct")
        completed = subprocess.run(
            [str(EXPECTED_QACCT), "-j", job_id],
            env={
                "HOME": str(expected_run_root / "work/scheduler-home"),
                "PATH": "/usr/local/bin:/usr/bin:/bin",
                "LANG": "C",
                "LC_ALL": "C",
                "TZ": "UTC",
                "SGE_ROOT": EXPECTED_SGE_ROOT,
            },
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        if completed.returncode == 0:
            try:
                validate_qacct(
                    completed.stdout,
                    job_id,
                    task_ids,
                    expected_slots=expected_slots,
                    expected_job_name=expected_job_name,
                    expected_script=expected_script,
                    expected_run_root=expected_run_root,
                    expected_argv=expected_argv,
                    submission_started_at_utc=submission_started_at_utc,
                    submission_completed_at_utc=submission_completed_at_utc,
                )
            except SccContractError as exc:
                last_error = exc
            else:
                return completed.stdout
        if attempt + 1 < attempts:
            time.sleep(delay_seconds)
    detail = f": {last_error}" if last_error is not None else ""
    raise SccContractError(f"qacct remained unavailable or incomplete for job {job_id}{detail}")


def close_run(run_root: Path) -> None:
    preparation = read_sealed(
        run_root / "input/preparation.json", "fereg-f0-scc-preparation-v1"
    )
    submission = validate_submission_binding(run_root, preparation)
    collector = read_sealed(run_root / "receipts/collector.json", "fereg-f0-scc-collector-v1")
    setup_path = run_root / "receipts/setup.json"
    collector_path = run_root / "receipts/collector.json"
    setup_success = run_root / "receipts/setup.SUCCESS"
    collector_complete = run_root / "receipts/collector.COMPLETE"
    if (
        not setup_success.is_file()
        or setup_success.read_text(encoding="ascii")
        != _receipt_sentinel(setup_path, "FEREG_F0_SETUP_SUCCESS")
        or not collector_complete.is_file()
        or collector_complete.read_text(encoding="ascii")
        != _receipt_sentinel(
            collector_path, str(collector.get("terminal_sentinel", ""))
        )
    ):
        raise SccContractError("run cannot close without setup and collector completion sentinels")
    manifest = load_regression_manifest(run_root / "input/manifest.json")
    lifecycle = _manifest_lifecycle(manifest)
    if lifecycle == "candidate_unsealed":
        raise SccContractError("candidate_unsealed cannot close a governing matrix")
    setup = read_sealed(run_root / "receipts/setup.json", "fereg-f0-scc-setup-v1")
    if setup.get("sge", {}).get("job_id") != submission["jobs"]["setup"]:
        raise SccContractError("setup receipt job ID differs from submission")
    if collector.get("sge", {}).get("job_id") != submission["jobs"]["collector"]:
        raise SccContractError("collector receipt job ID differs from submission")
    validate_task_inventory(run_root, manifest, setup, submission)
    submission_ledger = submission["submission_ledger"]
    submitted_root = _submitted_root(
        submission_ledger["submit_cwd"], run_root.name
    )
    accounting: dict[str, Any] = {}
    for stage in ("setup", "array", "collector"):
        job_id = str(submission["jobs"][stage])
        if stage == "array":
            expected: Sequence[int] | None = list(
                range(1, len(scc_cells(manifest)) + 1)
            )
        else:
            expected = None
        expected_slots = int(submission["resources"][stage]["slots"])
        expected_script = submitted_root / submission["scripts"][stage]["path"]
        text = _fetch_qacct(
            job_id,
            expected,
            expected_slots=expected_slots,
            expected_job_name=submission["job_names"][stage],
            expected_script=expected_script,
            expected_run_root=submitted_root,
            expected_argv=submission_ledger["argv"][stage],
            submission_started_at_utc=submission_ledger["started_at_utc"],
            submission_completed_at_utc=submission_ledger[
                "completed_at_utc"
            ],
        )
        records = validate_qacct(
            text,
            job_id,
            expected,
            expected_slots=expected_slots,
            expected_job_name=submission["job_names"][stage],
            expected_script=expected_script,
            expected_run_root=submitted_root,
            expected_argv=submission_ledger["argv"][stage],
            submission_started_at_utc=submission_ledger["started_at_utc"],
            submission_completed_at_utc=submission_ledger["completed_at_utc"],
        )
        destination = run_root / f"receipts/qacct/{stage}.txt"
        _write_exclusive(destination, text.encode("utf-8"))
        accounting[stage] = {"job_id": job_id, "records": len(records), "sha256": sha256_file(destination)}
    for task_id in range(1, len(scc_cells(manifest)) + 1):
        task = read_sealed(
            run_root / f"receipts/tasks/task-{task_id:03d}.json",
            "fereg-f0-scc-task-v1",
        )
        if (
            task.get("sge", {}).get("job_id") != submission["jobs"]["array"]
            or str(task.get("sge", {}).get("task_id")) != str(task_id)
        ):
            raise SccContractError("task receipt job ID differs from submission")
    qualification = run_root / "output/qualification.json"
    evidence = run_root / "output/evidence.json"
    if sha256_file(qualification) != collector["qualification_sha256"] or sha256_file(evidence) != collector["evidence_sha256"]:
        raise SccContractError("collector output changed before accounting closure")
    if lifecycle == "artifacts_sealed":
        receipt_type = "fereg-f0-scc-baseline-evidence-v1"
        status = "complete_nonqualifying"
        terminal_sentinel = "FEREG_F0_SCC_BASELINE_EVIDENCE_COMPLETE"
        destination = run_root / "receipts/baseline-evidence.json"
        sentinel_path = run_root / "receipts/BASELINE_EVIDENCE.COMPLETE"
    else:
        receipt_type = "fereg-f0-scc-closed-run-v1"
        status = "success"
        terminal_sentinel = "FEREG_F0_SCC_RUN_SUCCESS"
        destination = run_root / "receipts/run.json"
        sentinel_path = run_root / "receipts/RUN.SUCCESS"
    expected_collector_status = "blocked" if lifecycle == "artifacts_sealed" else "pass"
    if collector.get("qualification_status") != expected_collector_status:
        raise SccContractError("collector verdict differs from manifest lifecycle")
    payload = {
        "schema_version": RECEIPT_SCHEMA,
        "receipt_type": receipt_type,
        "status": status,
        "run_id": run_root.name,
        "manifest": dict(collector["manifest"]),
        "jobs": dict(submission["jobs"]),
        "scheduler_clients": {
            "qsub": submission_ledger["qsub_client"],
            "qacct": _verify_scheduler_client("qacct"),
        },
        "accounting": accounting,
        "cells": [cell["id"] for cell in scc_cells(manifest)],
        "evidence_sha256": collector["evidence_sha256"],
        "qualification_sha256": collector["qualification_sha256"],
        "setup_receipt_sha256": sha256_file(run_root / "receipts/setup.json"),
        "submission_receipt_sha256": sha256_file(
            run_root / "receipts/submission.json"
        ),
        "collector_receipt_sha256": sha256_file(run_root / "receipts/collector.json"),
        "terminal_sentinel": terminal_sentinel,
    }
    write_sealed(destination, payload)
    _write_exclusive(
        sentinel_path,
        _receipt_sentinel(destination, terminal_sentinel).encode("ascii"),
    )
    print(terminal_sentinel, flush=True)


def _verify_closed_run(
    run_root: Path,
    manifest_path: Path,
    *,
    expected_lifecycle: str,
    expected_closure: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Verify a collected final frozen run without querying SCC or writing.

    This is the PPML qualification read boundary. The held collector is not sufficient:
    every source/archive, setup, submitted job, task, scientific cell, evaluator
    receipt, raw qacct record, and final hash-bound sentinel must agree.
    """

    root = run_root.resolve()
    _validate_run_id(root.name)
    verify_input(root)
    run_manifest_path = root / "input/manifest.json"
    external_manifest_path = manifest_path.resolve()
    if (
        not external_manifest_path.is_file()
        or external_manifest_path.read_bytes() != run_manifest_path.read_bytes()
    ):
        raise SccContractError(
            "closed run manifest differs from the committed manifest"
        )
    manifest = load_regression_manifest(run_manifest_path)
    if expected_lifecycle not in {"artifacts_sealed", "frozen"}:
        raise SccContractError("closed-run verifier lifecycle is invalid")
    if _manifest_lifecycle(manifest) != expected_lifecycle:
        raise SccContractError(
            f"closed run is not the required {expected_lifecycle} lifecycle"
        )

    preparation_path = root / "input/preparation.json"
    preparation = read_sealed(
        preparation_path, "fereg-f0-scc-preparation-v1"
    )
    source_repo = external_manifest_path.parents[2]
    source_identities = {
        "baseline": preparation["arms"]["baseline"],
        "candidate": preparation["arms"]["candidate"],
        "harness": preparation["harness_source"],
    }
    repository_objects_available = (source_repo / ".git").exists()
    if not repository_objects_available:
        if expected_closure is None:
            raise SccContractError(
                "git-free diagnostic verification requires its frozen expected closure"
            )
        if (
            expected_closure.get("source_archives")
            != {
                arm: dict(preparation["arms"][arm])
                for arm in ("baseline", "candidate")
            }
            or expected_closure.get("harness_source")
            != preparation["harness_source"]
        ):
            raise SccContractError(
                "git-free diagnostic source archives differ from the frozen closure"
            )
    for label, identity in source_identities.items():
        archive_path = root / "input" / identity["archive"]
        if repository_objects_available:
            exact_archive = _git(
                source_repo, ["archive", "--format=tar", identity["commit"]]
            )
            if exact_archive != archive_path.read_bytes():
                raise SccContractError(
                    f"closed run {label} archive differs from its exact git object"
                )
            observed = source_identity(
                source_repo, identity["commit"], exact_archive
            )
            for field in (
                "commit",
                "tree",
                "cargo_lock_sha256",
                "references_lock_sha256",
                "archive_sha256",
                "archive_size_bytes",
                "build_date",
            ):
                if identity[field] != observed[field]:
                    raise SccContractError(
                        f"closed run {label} source identity differs at {field}"
                    )
        else:
            if (
                _git_archive_embedded_commit(archive_path) != identity["commit"]
                or sha256_file(archive_path) != identity["archive_sha256"]
                or archive_path.stat().st_size != identity["archive_size_bytes"]
                or sha256_bytes(_archive_member_bytes(archive_path, "Cargo.lock"))
                != identity["cargo_lock_sha256"]
                or sha256_bytes(
                    _archive_member_bytes(archive_path, "references.lock.toml")
                )
                != identity["references_lock_sha256"]
            ):
                raise SccContractError(
                    f"git-free closed run {label} archive identity changed"
                )
        extracted = root / (
            "work/setup/harness/source" if label == "harness"
            else f"work/setup/{label}/source"
        )
        _verify_extracted_archive_files(
            archive_path, extracted,
            allow_formal_dist=label != "harness",
            containment_root=root,
            read_only=True,
        )
    if (
        _archive_member_bytes(
            root / "input/archives/harness.tar",
            preparation["harness_manifest_path"],
        )
        != run_manifest_path.read_bytes()
    ):
        raise SccContractError("harness archive contains a different F0 manifest")

    baseline_source = manifest["baseline_candidate"]["source"]
    if any(
        preparation["arms"]["baseline"].get(field) != baseline_source[field]
        for field in (
            "commit",
            "tree",
            "cargo_lock_sha256",
            "references_lock_sha256",
        )
    ):
        raise SccContractError("closed run substitutes the F0 source")

    submission = validate_submission_binding(root, preparation)
    setup_path = root / "receipts/setup.json"
    setup = read_sealed(setup_path, "fereg-f0-scc-setup-v1")
    _expect_keys(
        setup,
        {
            "schema_version",
            "receipt_type",
            "run_id",
            "manifest",
            "preparation_receipt_sha256",
            "harness_source",
            "toolchain",
            "stata_controller",
            "arms",
            "smoke",
            "datasets",
            "sge",
            "terminal_sentinel",
        },
        "setup receipt",
    )
    if (
        setup.get("schema_version") != RECEIPT_SCHEMA
        or setup.get("run_id") != root.name
        or setup.get("manifest") != preparation["manifest"]
        or setup.get("preparation_receipt_sha256")
        != sha256_file(preparation_path)
        or setup.get("harness_source") != preparation.get("harness_source")
        or setup.get("terminal_sentinel") != "FEREG_F0_SETUP_SUCCESS"
        or setup.get("sge")
        != {
            "job_id": submission["jobs"]["setup"],
            "host_id": setup.get("sge", {}).get("host_id"),
            "slots": 16,
        }
    ):
        raise SccContractError("setup receipt is not bound to preparation/submission")
    if not isinstance(setup["sge"]["host_id"], str) or not setup["sge"]["host_id"]:
        raise SccContractError("setup receipt omits its execution host")
    if setup.get("toolchain", {}).get("rustc") != "rustc 1.85.1 (4eb161250 2025-03-15)":
        raise SccContractError("setup receipt records the wrong Rust compiler")
    if setup.get("toolchain", {}).get("cargo") != "cargo 1.85.1 (d73d2caf9 2024-12-31)":
        raise SccContractError("setup receipt records the wrong Cargo version")
    if not isinstance(setup.get("toolchain", {}).get("cc"), str) or not setup["toolchain"]["cc"]:
        raise SccContractError("setup receipt omits its C compiler")
    if setup.get("stata_controller") != {
        "path": str(SCC_STATA), "sha256": SCC_STATA_SHA256,
        "size_bytes": SCC_STATA_SIZE,
        "argv_contract": [str(SCC_STATA), "-b", "do", "<retained-wrapper.do>"],
        "environment_contract": "closed-private-home-tmp-v1",
    }:
        raise SccContractError("setup receipt records the wrong Stata controller")
    submitted_root = _submitted_root(
        submission["submission_ledger"]["submit_cwd"], root.name
    )
    baseline_ols = _json(root / "work/setup/baseline/source/benchmarks/manifests/ols_v1.json")
    baseline_datasets = {item["id"]: item for item in baseline_ols["datasets"]}
    dataset_plans = (
        ("generated_dense", "generated_dense", "dense"),
        ("generated_sparse", "generated_sparse", "sparse"),
        ("generated_qualified_dense", "generated_dense", "qualified_dense"),
        ("generated_disconnected", "generated_sparse", "disconnected"),
    )
    if set(setup["datasets"]) != {item[0] for item in dataset_plans}:
        raise SccContractError("setup dataset inventory changed")
    for dataset_id, source_id, variant in dataset_plans:
        spec = baseline_datasets[source_id]
        record = _expect_keys(
            setup["datasets"][dataset_id],
            {"sha256", "size_bytes", "source_sha256", "source_dataset", "arguments", "variant", "action"},
            f"setup dataset {dataset_id}",
        )
        if (
            record["source_dataset"] != source_id
            or record["source_sha256"] != spec["source"]["sha256"]
            or record["arguments"] != list(spec["arguments"])
            or record["variant"] != variant
        ):
            raise SccContractError(f"setup dataset plan changed: {dataset_id}")
        _verify_dataset_action(
            root, submitted_root, dataset_id, record,
            root / "work/setup/baseline/source" / spec["source"]["path"],
            spec["arguments"], variant,
        )
    if (
        (root / "receipts/setup.SUCCESS").read_text(encoding="ascii")
        != _receipt_sentinel(setup_path, "FEREG_F0_SETUP_SUCCESS")
    ):
        raise SccContractError("setup success sentinel does not bind setup bytes")

    _expect_keys(setup.get("arms"), {"baseline", "candidate"}, "setup arms")
    for arm in ("baseline", "candidate"):
        arm_receipt = _expect_keys(
            setup["arms"][arm],
            {"source", "artifact", "bundle", "build"},
            f"setup {arm} arm",
        )
        expected_source = {
            field: preparation["arms"][arm][field]
            for field in (
                "commit",
                "tree",
                "cargo_lock_sha256",
                "references_lock_sha256",
            )
        }
        if arm_receipt["source"] != expected_source:
            raise SccContractError(f"setup {arm} source differs from preparation")
        _verify_build_receipt(
            root, preparation, arm, arm_receipt["build"], portable=True,
            expected_origin_root=submitted_root,
        )
        artifact = _expect_keys(
            arm_receipt["artifact"],
            {"platform", "sha256", "size_bytes", "embedded_git_commit"},
            f"setup {arm} artifact",
        )
        if (
            artifact["platform"] != "linux-x86_64"
            or SHA256_RE.fullmatch(str(artifact["sha256"])) is None
            or not isinstance(artifact["size_bytes"], int)
            or artifact["size_bytes"] < 1
            or artifact["embedded_git_commit"]
            != preparation["arms"][arm]["commit"][:12]
        ):
            raise SccContractError(f"setup {arm} artifact identity is malformed")
        _verify_staged_bundle(
            root / f"work/bundles/{arm}",
            arm_receipt,
            root / "input" / preparation["arms"][arm]["archive"],
        )

    _verify_setup_smoke(root, setup)

    registered_linux = next(
        item
        for item in manifest["baseline_candidate"]["artifacts"]
        if item["platform"] == "linux-x86_64"
    )
    expected_f0_linux = {
        field: registered_linux[field]
        for field in ("platform", "sha256", "size_bytes", "embedded_git_commit")
    }
    if (
        registered_linux.get("status") != "qualified"
        or setup["arms"]["baseline"]["artifact"] != expected_f0_linux
    ):
        raise SccContractError("closed run does not use the frozen F0 Linux artifact")

    cells = validate_task_inventory(root, manifest, setup, submission)
    assembled = assemble_evidence(root)
    evidence_path = root / "output/evidence.json"
    evidence = _json(evidence_path)
    if evidence != assembled or evidence.get("cells") != cells:
        raise SccContractError("closed run assembled evidence differs from its shards")

    qualification_path = root / "output/qualification.json"
    evaluator_path = source_repo / "scripts/fereg_f0_regression.py"
    evaluator = _load_python_module(
        evaluator_path,
        f"fereg_f0_closed_run_{sha256_file(evaluator_path)[:12]}",
    )
    try:
        qualification_document = evaluator.verify_receipt(
            qualification_path, external_manifest_path
        )
    except (OSError, ValueError) as error:
        raise SccContractError(
            f"closed run evaluator receipt failed verification: {error}"
        ) from error
    qualification_payload = qualification_document["payload"]
    evaluation = qualification_payload.get("evaluation")
    expected_evaluation_status = (
        "blocked" if expected_lifecycle == "artifacts_sealed" else "pass"
    )
    expected_evaluation_reasons = (
        ["baseline_candidate_unsealed"]
        if expected_lifecycle == "artifacts_sealed"
        else []
    )
    if (
        qualification_payload.get("evidence") != evidence
        or not isinstance(evaluation, Mapping)
        or evaluation.get("status") != expected_evaluation_status
        or evaluation.get("reasons") != expected_evaluation_reasons
        or evaluation.get("profile") != "scc_governing"
        or evaluation.get("scientific_cells_passed") != len(scc_cells(manifest))
    ):
        raise SccContractError("closed run evaluator result has the wrong lifecycle verdict")

    collector_path = root / "receipts/collector.json"
    collector = read_sealed(
        collector_path, "fereg-f0-scc-collector-v1"
    )
    _expect_keys(
        collector,
        {
            "schema_version",
            "receipt_type",
            "run_id",
            "manifest",
            "cell_count",
            "evidence_sha256",
            "qualification_sha256",
            "qualification_status",
            "qualification_reasons",
            "sge",
            "terminal_sentinel",
        },
        "collector receipt",
    )
    expected_collector_sentinel = (
        "FEREG_F0_COLLECTOR_BASELINE_EVIDENCE_COMPLETE"
        if expected_lifecycle == "artifacts_sealed"
        else "FEREG_F0_COLLECTOR_SUCCESS"
    )
    if (
        collector.get("schema_version") != RECEIPT_SCHEMA
        or collector.get("run_id") != root.name
        or collector.get("manifest") != preparation["manifest"]
        or collector.get("cell_count") != len(scc_cells(manifest))
        or collector.get("evidence_sha256") != sha256_file(evidence_path)
        or collector.get("qualification_sha256") != sha256_file(qualification_path)
        or collector.get("qualification_status") != expected_evaluation_status
        or collector.get("qualification_reasons") != expected_evaluation_reasons
        or collector.get("terminal_sentinel") != expected_collector_sentinel
        or collector.get("sge")
        != {
            "job_id": submission["jobs"]["collector"],
            "host_id": collector.get("sge", {}).get("host_id"),
            "slots": 1,
        }
    ):
        raise SccContractError("collector receipt is not bound to the exact pass")
    if not isinstance(collector["sge"]["host_id"], str) or not collector["sge"]["host_id"]:
        raise SccContractError("collector receipt omits its execution host")
    collector_sentinels = [
        (root / "receipts/collector.COMPLETE", expected_collector_sentinel)
    ]
    if expected_lifecycle == "frozen":
        collector_sentinels.append(
            (root / "receipts/collector.SUCCESS", "FEREG_F0_COLLECTOR_SUCCESS")
        )
    elif (root / "receipts/collector.SUCCESS").exists():
        raise SccContractError(
            "diagnostic collector may not publish a success sentinel"
        )
    for sentinel, label in collector_sentinels:
        if sentinel.read_text(encoding="ascii") != _receipt_sentinel(
            collector_path, label
        ):
            raise SccContractError("collector success sentinel does not bind collector bytes")

    qacct_records: dict[str, list[dict[str, str]]] = {}
    submission_ledger = submission["submission_ledger"]
    submitted_root = _submitted_root(
        submission_ledger["submit_cwd"], root.name
    )
    expected_slots = {"setup": 16, "array": 16, "collector": 1}
    for stage in ("setup", "array", "collector"):
        task_ids: Sequence[int] | None
        if stage == "array":
            task_ids = list(range(1, len(scc_cells(manifest)) + 1))
        else:
            task_ids = None
        qacct_path = root / f"receipts/qacct/{stage}.txt"
        qacct_records[stage] = validate_qacct(
            qacct_path.read_text(encoding="utf-8"),
            submission["jobs"][stage],
            task_ids,
            expected_slots=expected_slots[stage],
            expected_job_name=submission["job_names"][stage],
            expected_script=submitted_root / submission["scripts"][stage]["path"],
            expected_run_root=submitted_root,
            expected_argv=submission_ledger["argv"][stage],
            submission_started_at_utc=submission_ledger["started_at_utc"],
            submission_completed_at_utc=submission_ledger["completed_at_utc"],
        )
    actual_qacct = {
        path.name for path in (root / "receipts/qacct").iterdir() if path.is_file()
    }
    if actual_qacct != {"setup.txt", "array.txt", "collector.txt"}:
        raise SccContractError("closed run qacct inventory is not exact")
    if qacct_records["setup"][0]["hostname"] != setup["sge"]["host_id"]:
        raise SccContractError("setup receipt host differs from qacct")
    if qacct_records["collector"][0]["hostname"] != collector["sge"]["host_id"]:
        raise SccContractError("collector receipt host differs from qacct")
    for task_id in range(1, len(scc_cells(manifest)) + 1):
        task = read_sealed(
            root / f"receipts/tasks/task-{task_id:03d}.json",
            "fereg-f0-scc-task-v1",
        )
        records = [
            record
            for record in qacct_records["array"]
            if record.get("taskid") == str(task_id)
        ]
        if (
            len(records) != 1
            or records[0]["hostname"] != task.get("sge", {}).get("host_id")
            or records[0]["slots"] != str(task.get("sge", {}).get("slots"))
        ):
            raise SccContractError(
                f"task {task_id} receipt does not match its qacct record"
            )

    if expected_lifecycle == "artifacts_sealed":
        final_path = root / "receipts/baseline-evidence.json"
        final_receipt_type = "fereg-f0-scc-baseline-evidence-v1"
        final_status = "complete_nonqualifying"
        final_terminal = "FEREG_F0_SCC_BASELINE_EVIDENCE_COMPLETE"
        final_sentinel_path = root / "receipts/BASELINE_EVIDENCE.COMPLETE"
        forbidden_final = root / "receipts/RUN.SUCCESS"
    else:
        final_path = root / "receipts/run.json"
        final_receipt_type = "fereg-f0-scc-closed-run-v1"
        final_status = "success"
        final_terminal = "FEREG_F0_SCC_RUN_SUCCESS"
        final_sentinel_path = root / "receipts/RUN.SUCCESS"
        forbidden_final = root / "receipts/BASELINE_EVIDENCE.COMPLETE"
    if forbidden_final.exists():
        raise SccContractError("closed run publishes a sentinel for the wrong lifecycle")
    final = read_sealed(final_path, final_receipt_type)
    _expect_keys(
        final,
        {
            "schema_version",
            "receipt_type",
            "status",
            "run_id",
            "manifest",
            "jobs",
            "scheduler_clients",
            "accounting",
            "cells",
            "evidence_sha256",
            "qualification_sha256",
            "setup_receipt_sha256",
            "submission_receipt_sha256",
            "collector_receipt_sha256",
            "terminal_sentinel",
        },
        "closed-run receipt",
    )
    if (
        final.get("schema_version") != RECEIPT_SCHEMA
        or final.get("status") != final_status
        or final.get("run_id") != root.name
        or final.get("manifest") != preparation["manifest"]
        or final.get("jobs") != submission["jobs"]
        or final.get("scheduler_clients")
        != {
            "qsub": submission["submission_ledger"]["qsub_client"],
            "qacct": _expected_scheduler_client("qacct"),
        }
        or final.get("cells") != [cell["id"] for cell in scc_cells(manifest)]
        or final.get("evidence_sha256") != sha256_file(evidence_path)
        or final.get("qualification_sha256") != sha256_file(qualification_path)
        or final.get("setup_receipt_sha256") != sha256_file(setup_path)
        or final.get("submission_receipt_sha256")
        != sha256_file(root / "receipts/submission.json")
        or final.get("collector_receipt_sha256") != sha256_file(collector_path)
        or final.get("terminal_sentinel") != final_terminal
    ):
        raise SccContractError("closed-run receipt differs from exact run evidence")
    accounting = _expect_keys(
        final.get("accounting"),
        {"setup", "array", "collector"},
        "closed-run accounting",
    )
    for stage, records in qacct_records.items():
        qacct_path = root / f"receipts/qacct/{stage}.txt"
        if accounting[stage] != {
            "job_id": submission["jobs"][stage],
            "records": len(records),
            "sha256": sha256_file(qacct_path),
        }:
            raise SccContractError(
                f"closed-run {stage} accounting differs from raw qacct"
            )
    if final_sentinel_path.read_text(encoding="ascii") != _receipt_sentinel(
        final_path, final_terminal
    ):
        raise SccContractError("final success sentinel does not bind closed-run bytes")

    result = {
        "schema_version": 1,
        "status": final_status,
        "run_id": root.name,
        "manifest": dict(preparation["manifest"]),
        "baseline": dict(assembled["baseline"]),
        "candidate": dict(assembled["candidate"]),
        "source_archives": {
            arm: dict(preparation["arms"][arm])
            for arm in ("baseline", "candidate")
        },
        "harness_source": dict(preparation["harness_source"]),
        "jobs": dict(submission["jobs"]),
        "qualification_sha256": sha256_file(qualification_path),
        "closed_run_sha256": sha256_file(final_path),
        "terminal_sentinel": (
            "FEREG_F0_DIAGNOSTIC_RUN_VERIFIED"
            if expected_lifecycle == "artifacts_sealed"
            else "FEREG_F0_CLOSED_RUN_VERIFIED"
        ),
    }
    if expected_closure is not None and result != expected_closure:
        raise SccContractError(
            "diagnostic run differs from its frozen expected closure"
        )
    return result


def verify_diagnostic_run(
    run_root: Path,
    manifest_path: Path,
    *,
    expected_closure: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Read-only verification of the nonqualifying artifact-sealed run."""

    return _verify_closed_run(
        run_root,
        manifest_path,
        expected_lifecycle="artifacts_sealed",
        expected_closure=expected_closure,
    )


def verify_closed_run(
    run_root: Path,
    manifest_path: Path,
) -> dict[str, Any]:
    """Read-only verification of the final exact frozen-manifest run."""

    return _verify_closed_run(
        run_root,
        manifest_path,
        expected_lifecycle="frozen",
    )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    prepare = commands.add_parser("prepare", help="create immutable exact-source run inputs locally")
    prepare.add_argument("--run-root", type=Path, required=True)
    prepare.add_argument("--baseline-repo", type=Path, required=True)
    prepare.add_argument("--baseline-commit", required=True)
    prepare.add_argument("--candidate-repo", type=Path, required=True)
    prepare.add_argument("--candidate-commit", required=True)
    prepare.add_argument("--harness-repo", type=Path, required=True)
    prepare.add_argument("--harness-commit", required=True)
    prepare.add_argument("--manifest", type=Path, required=True)
    prepare.add_argument(
        "--seal-root",
        type=Path,
        help="external root containing the manifest's byte-bound seal closure",
    )
    prepare.add_argument("--behavior-baseline", type=Path)
    prepare.add_argument("--behavior-baseline-bundle", type=Path)
    prepare.add_argument("--behavior-candidate", type=Path)
    prepare.add_argument("--behavior-candidate-bundle", type=Path)
    prepare.add_argument("--behavior-comparison", type=Path)

    setup = commands.add_parser("setup", help="build both Linux bundles and generate shared data")
    setup.add_argument("--run-root", type=Path, required=True)

    verify = commands.add_parser("verify-input", help="verify a transferred run input closure")
    verify.add_argument("--run-root", type=Path, required=True)

    task = commands.add_parser("task", help="execute one same-node paired array cell")
    task.add_argument("--run-root", type=Path, required=True)
    task.add_argument("--task-id", type=int, required=True)

    collect = commands.add_parser("collect", help="assemble closed shards and invoke the generic evaluator")
    collect.add_argument("--run-root", type=Path, required=True)

    submission = commands.add_parser("record-submission", help="bind accepted numeric qsub IDs")
    submission.add_argument("--run-root", type=Path, required=True)

    close = commands.add_parser("close", help="validate qacct and publish the final success receipt")
    close.add_argument("--run-root", type=Path, required=True)

    closed = commands.add_parser(
        "verify-closed-run",
        help="read-only verification of a collected frozen run and raw qacct",
    )
    closed.add_argument("--run-root", type=Path, required=True)
    closed.add_argument("--manifest", type=Path, required=True)
    diagnostic = commands.add_parser(
        "verify-diagnostic-run",
        help="read-only verification of a nonqualifying artifacts-sealed run",
    )
    diagnostic.add_argument("--run-root", type=Path, required=True)
    diagnostic.add_argument("--manifest", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    if not (sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode):
        raise SystemExit("fereg F0 SCC CLI requires Python -I -B -S")
    forbidden = ("PYTHONPATH", "PYTHONHOME", "PYTHONSTARTUP", "LD_PRELOAD", "DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH")
    present = [name for name in forbidden if os.environ.get(name)]
    if present:
        raise SystemExit("fereg F0 SCC CLI rejects ambient loader/startup variables: " + ",".join(present))
    args = _parser().parse_args(argv)
    try:
        if args.command == "prepare":
            prepare_run(
                run_root=args.run_root,
                baseline_repo=args.baseline_repo,
                baseline_commit=args.baseline_commit,
                candidate_repo=args.candidate_repo,
                candidate_commit=args.candidate_commit,
                harness_repo=args.harness_repo,
                harness_commit=args.harness_commit,
                manifest_path=args.manifest,
                seal_root=args.seal_root,
                behavior_baseline=args.behavior_baseline,
                behavior_baseline_bundle=args.behavior_baseline_bundle,
                behavior_candidate=args.behavior_candidate,
                behavior_candidate_bundle=args.behavior_candidate_bundle,
                behavior_comparison=args.behavior_comparison,
            )
        elif args.command == "setup":
            setup_run(args.run_root)
        elif args.command == "verify-input":
            verify_input(args.run_root)
            print("FEREG_F0_INPUT_VERIFIED", flush=True)
        elif args.command == "task":
            if args.task_id <= 0:
                raise SccContractError("task ID must be positive")
            task_run(args.run_root, args.task_id)
        elif args.command == "collect":
            collect_run(args.run_root)
        elif args.command == "record-submission":
            record_submission(args.run_root)
        elif args.command == "close":
            close_run(args.run_root)
        elif args.command == "verify-closed-run":
            result = verify_closed_run(args.run_root, args.manifest)
            print(json.dumps(result, sort_keys=True), flush=True)
        elif args.command == "verify-diagnostic-run":
            result = verify_diagnostic_run(args.run_root, args.manifest)
            print(json.dumps(result, sort_keys=True), flush=True)
        else:  # pragma: no cover
            raise AssertionError(args.command)
    except (SccContractError, OSError, ValueError, KeyError, TypeError) as exc:
        print(f"fereg F0 SCC contract failed: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
