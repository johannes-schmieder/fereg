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
import hashlib
import importlib.util
import json
import math
import os
import platform
import re
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

HARNESS_PATHS = (
    "benchmarks/scc/fereg_f0_scc.py",
    "benchmarks/scc/fereg_f0_setup_job.sh",
    "benchmarks/scc/fereg_f0_job.sh",
    "benchmarks/scc/fereg_f0_collector_job.sh",
    "benchmarks/scc/fereg_f0_deploy.sh",
    "benchmarks/scc/fereg_f0_submit.sh",
    "benchmarks/scc/fereg_f0_close.sh",
    "scripts/fereg_f0_regression.py",
)

BASE_CASE_BY_DESIGN = {
    "dense_two_fe": "bench_dense_two_fe_t{threads}",
    "sparse_two_fe": "bench_sparse_two_fe_t{threads}",
    "absorbed_slope": "bench_absorbed_slope_t{threads}",
    "clustered_vce": "bench_clustered_vce_t{threads}",
    "moderate_k": "bench_moderate_k_t{threads}",
    "constrained_memory": "bench_dense_two_fe_t{threads}",
}


class SccContractError(RuntimeError):
    """A closed SCC input, shard, receipt, or accounting contract failed."""


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


def _run(command: Sequence[str], *, cwd: Path | None = None, env: Mapping[str, str] | None = None) -> subprocess.CompletedProcess[bytes]:
    completed = subprocess.run(command, cwd=cwd, env=env, check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if completed.returncode:
        label = " ".join(command[:3])
        raise SccContractError(f"command failed ({completed.returncode}): {label}")
    return completed


def _git(repo: Path, arguments: Sequence[str]) -> bytes:
    return _run(["git", "-C", str(repo), *arguments]).stdout


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
    required = {"id", "profile", "design", "n", "controls", "threads", "memory", "outputs", "vce", "primary"}
    for cell in cells:
        if not isinstance(cell, Mapping) or set(cell) != required:
            raise SccContractError("F0 manifest cell schema is not closed")
        cell_id = cell["id"]
        if not isinstance(cell_id, str) or not CELL_ID_RE.fullmatch(cell_id) or cell_id in ids:
            raise SccContractError("F0 manifest cell IDs are malformed or duplicated")
        if cell["profile"] not in {"local_screen", "scc_governing"} or cell["design"] not in BASE_CASE_BY_DESIGN:
            raise SccContractError(f"unsupported F0 cell {cell_id}")
        if cell["profile"] == "scc_governing" and cell["n"] != 10_000_000:
            raise SccContractError(f"SCC cell scale changed: {cell_id}")
        if cell["profile"] == "local_screen" and cell["n"] not in {100_000, 1_000_000}:
            raise SccContractError(f"local cell scale changed: {cell_id}")
        if cell["threads"] not in {1, 8}:
            raise SccContractError(f"cell threads changed: {cell_id}")
        if cell["memory"] not in {"fast", "low"} or cell["outputs"] not in {"fit", "save_fe"}:
            raise SccContractError(f"SCC cell mode changed: {cell_id}")
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
    manifest_path: Path,
) -> None:
    _validate_run_id(run_root.name)
    if run_root.exists():
        raise SccContractError("run root already exists; retries require a new run ID")
    baseline_commit = resolve_commit(baseline_repo, baseline_commit)
    candidate_commit = resolve_commit(candidate_repo, candidate_commit)
    manifest_relative = manifest_path.resolve().relative_to(candidate_repo.resolve()).as_posix()
    manifest_bytes = git_blob(candidate_repo, candidate_commit, manifest_relative)
    if manifest_path.read_bytes() != manifest_bytes:
        raise SccContractError("manifest bytes do not match the exact candidate commit")
    manifest = load_regression_manifest(manifest_path)
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

    harness: dict[str, Any] = {}
    for relative in HARNESS_PATHS:
        content = git_blob(candidate_repo, candidate_commit, relative)
        destination = run_root / "input/harness" / Path(relative).name
        mode = 0o555 if destination.suffix in {".py", ".sh"} else 0o444
        _write_exclusive(destination, content, mode)
        harness[destination.name] = {"sha256": sha256_bytes(content), "size_bytes": len(content)}

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
        "tasks": {"count": len(governing_cells), "sha256": sha256_bytes(tasks)},
        "arms": arms,
        "harness": dict(sorted(harness.items())),
    }
    write_sealed(run_root / "input/preparation.json", payload)
    _write_exclusive(run_root / "input/PREPARED", f"FEREG_F0_PREPARED manifest_sha256={manifest_sha}\n".encode("ascii"))


def verify_input(run_root: Path) -> None:
    preparation_path = run_root / "input/preparation.json"
    preparation = read_sealed(preparation_path, "fereg-f0-scc-preparation-v1")
    if preparation.get("run_id") != run_root.name:
        raise SccContractError("prepared run ID does not match its directory")
    manifest_path = run_root / "input/manifest.json"
    tasks_path = run_root / "input/tasks.tsv"
    manifest = load_regression_manifest(manifest_path)
    if sha256_file(manifest_path) != preparation["manifest"]["sha256"]:
        raise SccContractError("prepared manifest hash mismatch")
    if sha256_file(tasks_path) != preparation["tasks"]["sha256"]:
        raise SccContractError("prepared task manifest hash mismatch")
    _cell_for_task(manifest, tasks_path, 1)
    for arm in ("baseline", "candidate"):
        identity = preparation["arms"][arm]
        archive = run_root / "input" / identity["archive"]
        if sha256_file(archive) != identity["archive_sha256"] or archive.stat().st_size != identity["archive_size_bytes"]:
            raise SccContractError(f"prepared {arm} archive hash mismatch")
    harness_dir = run_root / "input/harness"
    expected_harness = set(preparation["harness"])
    actual_harness = {path.name for path in harness_dir.iterdir() if path.is_file()}
    if actual_harness != expected_harness:
        raise SccContractError("prepared harness inventory is not closed")
    for name, identity in preparation["harness"].items():
        path = harness_dir / name
        if sha256_file(path) != identity["sha256"] or path.stat().st_size != identity["size_bytes"]:
            raise SccContractError(f"prepared harness file changed: {name}")
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


def _tool_version(command: Sequence[str]) -> str:
    completed = _run(command)
    text = (completed.stdout or completed.stderr).decode("utf-8", errors="replace").splitlines()
    if not text:
        raise SccContractError(f"tool did not report a version: {command[0]}")
    return text[0].strip()


def _stata_executable() -> str:
    for name in ("stata-mp", "stata-se", "stata"):
        value = shutil.which(name)
        if value:
            return value
    raise SccContractError("Stata executable is unavailable after loading stata-mp/19")


def _generate_dataset(stata: str, generator: Path, arguments: Sequence[Any], destination: Path, log_dir: Path, dataset_id: str) -> None:
    run_dir = log_dir / f"generate-{dataset_id}"
    run_dir.mkdir(parents=True, exist_ok=False)
    wrapper = run_dir / "generate.do"
    quoted_generator = str(generator).replace("\\", "/")
    quoted_destination = str(destination).replace("\\", "/")
    args = " ".join(str(value) for value in arguments)
    wrapper.write_text(
        "\n".join(
            [
                "version 19.0",
                "clear all",
                "set more off",
                f'capture noisily do "{quoted_generator}" {args} "{quoted_destination}"',
                "local rc = _rc",
                "if `rc' != 0 exit `rc'",
                f'capture confirm file "{quoted_destination}"',
                "if _rc exit _rc",
                f'display as result "FEREG_F0_DATASET_SUCCESS|{dataset_id}"',
                "exit 0",
                "",
            ]
        ),
        encoding="utf-8",
    )
    completed = subprocess.run([stata, "-b", "do", str(wrapper)], cwd=run_dir, check=False, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    (run_dir / "runner.stdout").write_text(completed.stdout, encoding="utf-8")
    log = run_dir / "generate.log"
    log_text = log.read_text(encoding="utf-8", errors="replace") if log.is_file() else ""
    sentinel = f"FEREG_F0_DATASET_SUCCESS|{dataset_id}"
    if completed.returncode != 0 or sentinel not in log_text or not destination.is_file():
        raise SccContractError(f"dataset generation failed: {dataset_id}")


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


def _verify_staged_bundle(bundle: Path, arm_receipt: Mapping[str, Any]) -> None:
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


def _make_tree_read_only(root: Path) -> None:
    for path in sorted(root.rglob("*"), reverse=True):
        if path.is_symlink():
            raise SccContractError(f"immutable work tree contains a symlink: {root.name}")
        path.chmod(0o555 if path.is_dir() else 0o444)
    root.chmod(0o555)


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

    toolchain = {
        "cargo": _tool_version(["cargo", "--version"]),
        "rustc": _tool_version(["rustc", "--version"]),
        "cc": _tool_version([os.environ.get("CC", "cc"), "--version"]),
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
        if sha256_file(source / "Cargo.lock") != identity["cargo_lock_sha256"] or sha256_file(source / "references.lock.toml") != identity["references_lock_sha256"]:
            raise SccContractError(f"{arm} extracted source closure mismatch")
        env = dict(os.environ)
        env["FEREG_GIT_COMMIT"] = str(identity["commit"])[:12]
        env["FEREG_BUILD_DATE"] = str(identity["build_date"])
        completed = subprocess.run(["bash", "scripts/build_linux.sh"], cwd=source, env=env, check=False, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        (logs / f"{arm}-build.log").write_text(completed.stdout, encoding="utf-8")
        plugin = source / "dist/fereg_linux.plugin"
        if completed.returncode != 0 or not plugin.is_file():
            raise SccContractError(f"{arm} Linux build/audit/loader failed")
        embedded_commit = str(identity["commit"])[:12]
        if embedded_commit.encode("ascii") not in plugin.read_bytes():
            raise SccContractError(f"{arm} Linux plugin omits its exact source commit")
        bundle = bundles_root / arm
        shutil.copytree(source / "stata", bundle / "stata")
        shutil.copy2(plugin, bundle / "stata/fereg_linux.plugin")
        bundle_sha, bundle_size, inventory = _bundle_digest(bundle)
        arm_receipts[arm] = {
            "source": {key: identity[key] for key in ("commit", "tree", "cargo_lock_sha256", "references_lock_sha256")},
            "artifact": {
                "platform": "linux-x86_64",
                "sha256": sha256_file(plugin),
                "size_bytes": plugin.stat().st_size,
                "embedded_git_commit": embedded_commit,
            },
            "bundle": {"sha256": bundle_sha, "size_bytes": bundle_size, "inventory": inventory},
        }

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

    baseline_source = setup_root / "baseline/source"
    ols_manifest = _json(baseline_source / "benchmarks/manifests/ols_v1.json")
    datasets_by_id = {item["id"]: item for item in ols_manifest["datasets"]}
    dataset_root = run_root / "work/datasets"
    dataset_root.mkdir(parents=True, exist_ok=False)
    stata = _stata_executable()
    datasets: dict[str, Any] = {}
    for dataset_id in ("generated_dense", "generated_sparse"):
        spec = datasets_by_id.get(dataset_id)
        if not isinstance(spec, Mapping) or spec.get("kind") != "generated_dta":
            raise SccContractError(f"baseline OLS manifest lacks {dataset_id}")
        generator = baseline_source / spec["source"]["path"]
        if sha256_file(generator) != spec["source"]["sha256"]:
            raise SccContractError(f"baseline generator hash mismatch: {dataset_id}")
        destination = dataset_root / f"{dataset_id}.dta"
        _generate_dataset(stata, generator, spec["arguments"], destination, logs, dataset_id)
        datasets[dataset_id] = {"sha256": sha256_file(destination), "size_bytes": destination.stat().st_size, "source_sha256": spec["source"]["sha256"]}

    for arm in ("baseline", "candidate"):
        _make_tree_read_only(bundles_root / arm)
        _make_tree_read_only(setup_root / arm / "source")
    _make_tree_read_only(dataset_root)

    payload = {
        "schema_version": RECEIPT_SCHEMA,
        "receipt_type": "fereg-f0-scc-setup-v1",
        "run_id": run_root.name,
        "manifest": dict(preparation["manifest"]),
        "preparation_receipt_sha256": sha256_file(run_root / "input/preparation.json"),
        "toolchain": toolchain,
        "arms": arm_receipts,
        "datasets": datasets,
        "sge": {
            "job_id": os.environ.get("JOB_ID", ""),
            "host_id": socket.gethostname(),
            "slots": int(os.environ.get("NSLOTS", "1")),
        },
        "terminal_sentinel": "FEREG_F0_SETUP_SUCCESS",
    }
    write_sealed(run_root / "receipts/setup.json", payload)
    _write_exclusive(run_root / "receipts/setup.SUCCESS", b"FEREG_F0_SETUP_SUCCESS\n")
    print("FEREG_F0_SETUP_SUCCESS", flush=True)


def _load_python_module(path: Path, name: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise SccContractError(f"cannot load exact harness module {path.name}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _support_modules(candidate_source: Path) -> tuple[Any, Any, Any]:
    root_text = str(candidate_source)
    if root_text not in sys.path:
        sys.path.insert(0, root_text)
    reference = _load_python_module(candidate_source / "scripts/ols_reference.py", "fereg_f0_exact_ols_reference")
    paired = _load_python_module(candidate_source / "scripts/ols_paired.py", "fereg_f0_exact_ols_paired")
    evaluator = _load_python_module(candidate_source / "scripts/fereg_f0_regression.py", "fereg_f0_exact_evaluator")
    return reference, paired, evaluator


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


def _cell_case(ols_manifest: Mapping[str, Any], cell: Mapping[str, Any]) -> tuple[dict[str, Any], Mapping[str, Any], list[str]]:
    base_id = BASE_CASE_BY_DESIGN[cell["design"]].format(threads=cell["threads"])
    base = next((item for item in ols_manifest["benchmark_cases"] if item["id"] == base_id), None)
    if base is None:
        raise SccContractError(f"OLS baseline manifest lacks mapped case {base_id}")
    case = copy.deepcopy(base)
    case["id"] = cell["id"]
    command = str(case["arms"]["candidate"])
    command = _replace_option(command, "threads", str(cell["threads"]))
    command = _replace_option(command, "memory", str(cell["memory"]))
    saved_variables: list[str] = []
    if cell["outputs"] == "save_fe":
        before = "absorb(worker firm)"
        after = "absorb(__frg_f0_worker=worker __frg_f0_firm=firm)"
        if before not in command:
            raise SccContractError("saved-FE cell no longer has the registered absorb syntax")
        command = command.replace(before, after)
        saved_variables = ["__frg_f0_worker", "__frg_f0_firm"]
    case["arms"]["candidate"] = command
    # The SCC Stata 19 license exposes four Stata processors. Native fereg
    # threads remain the registered one/eight value in the command itself.
    case["thread_count"] = min(cell["threads"], SCC_STATA_PROCESSOR_CAP)
    dataset = next(item for item in ols_manifest["datasets"] if item["id"] == case["dataset"])
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
    completed, rss = reference._run_fresh_process([stata, "-b", "do", str(wrapper)], run_dir)
    log_path = run_dir / "run.log"
    log_text = log_path.read_text(encoding="utf-8", errors="replace") if log_path.is_file() else ""
    sentinels = [line.strip() for line in log_text.splitlines() if line.strip().startswith("END_FIXTURE|")]
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


def _allocation_classes(paired: Any, captured: Mapping[str, Any], scientific: Mapping[str, Any]) -> dict[str, int]:
    def matrix_sum(name: str, column: int) -> int:
        values = [entry for entry in captured["matrices"] if entry["name"] == name and entry["column"] == column]
        return sum(round(paired.stata_hex_number(str(entry["value_hex"]))) for entry in values)

    n = round(paired.stata_hex_number(str(captured["scalars"]["N"])))
    return {
        "observations": n,
        "observation_columns": len(scientific["coefficient_names"]),
        "fe_levels": matrix_sum("fereg_fe", 4),
        "clusters": matrix_sum("fereg_cluster", 3),
    }


def _memory_record(paired: Any, captured: Mapping[str, Any], scientific: Mapping[str, Any], retained: int, constrained: Mapping[str, Any] | None) -> dict[str, Any]:
    return {
        "native_allocation_bytes": round(paired._matrix_value(captured, "fereg_runtime", 13, 2)),
        "allocation_classes": _allocation_classes(paired, captured, scientific),
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


def task_run(run_root: Path, task_id: int) -> None:
    verify_input(run_root)
    preparation = read_sealed(run_root / "input/preparation.json", "fereg-f0-scc-preparation-v1")
    setup = read_sealed(run_root / "receipts/setup.json", "fereg-f0-scc-setup-v1")
    if not (run_root / "receipts/setup.SUCCESS").is_file() or setup["preparation_receipt_sha256"] != sha256_file(run_root / "input/preparation.json"):
        raise SccContractError("setup did not publish a matching success receipt")
    manifest_path = run_root / "input/manifest.json"
    manifest = load_regression_manifest(manifest_path)
    manifest_sha = sha256_file(manifest_path)
    if manifest_sha != preparation["manifest"]["sha256"] or manifest_sha != setup["manifest"]["sha256"]:
        raise SccContractError("task manifest identity mismatch")
    cell = _cell_for_task(manifest, run_root / "input/tasks.tsv", task_id)
    if int(os.environ.get("NSLOTS", "1")) < int(cell["threads"]):
        raise SccContractError("SGE allocation is smaller than the registered native thread count")
    cell_id = str(cell["id"])
    task_root = run_root / "work/tasks" / f"task-{task_id:03d}-{cell_id}"
    task_root.mkdir(parents=True, exist_ok=False)

    baseline_source = run_root / "work/setup/baseline/source"
    candidate_source = run_root / "work/setup/candidate/source"
    reference, paired, evaluator = _support_modules(candidate_source)
    ols_manifest = _json(baseline_source / "benchmarks/manifests/ols_v1.json")
    case, dataset, saved_variables = _cell_case(ols_manifest, cell)
    data_path = run_root / "work/datasets" / f"{dataset['id']}.dta"
    dataset_receipt = setup["datasets"][dataset["id"]]
    if sha256_file(data_path) != dataset_receipt["sha256"]:
        raise SccContractError("prepared benchmark dataset changed")
    stata = _stata_executable()
    bundle_roots = {arm: run_root / f"work/bundles/{arm}" for arm in ("baseline", "candidate")}
    for arm, bundle in bundle_roots.items():
        _verify_staged_bundle(bundle, setup["arms"][arm])
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
            arm: _memory_record(paired, preflight_captures[arm], scientific[arm], retained[arm], constrained[arm])
            for arm in ("baseline", "candidate")
        },
    }
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
        "cell_sha256": sha256_file(cell_path),
        "sge": {"job_id": os.environ.get("JOB_ID", ""), "task_id": os.environ.get("SGE_TASK_ID", str(task_id)), "host_id": socket.gethostname(), "slots": int(os.environ.get("NSLOTS", "1"))},
        "terminal_sentinel": f"FEREG_F0_TASK_SUCCESS|{cell_id}",
    }
    receipt_path = run_root / f"receipts/tasks/task-{task_id:03d}.json"
    write_sealed(receipt_path, payload)
    _write_exclusive(run_root / f"receipts/tasks/task-{task_id:03d}.SUCCESS", f"FEREG_F0_TASK_SUCCESS|{cell_id}\n".encode("ascii"))
    print(f"FEREG_F0_TASK_SUCCESS|{cell_id}", flush=True)


def validate_task_inventory(run_root: Path, manifest: Mapping[str, Any], setup: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    cells: list[Mapping[str, Any]] = []
    expected_ids = [cell["id"] for cell in scc_cells(manifest)]
    for task_id, cell_id in enumerate(expected_ids, 1):
        receipt_path = run_root / f"receipts/tasks/task-{task_id:03d}.json"
        success_path = run_root / f"receipts/tasks/task-{task_id:03d}.SUCCESS"
        payload = read_sealed(receipt_path, "fereg-f0-scc-task-v1")
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
        expected_sentinel = f"FEREG_F0_TASK_SUCCESS|{cell_id}\n"
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


def assemble_evidence(run_root: Path) -> dict[str, Any]:
    preparation = read_sealed(run_root / "input/preparation.json", "fereg-f0-scc-preparation-v1")
    setup = read_sealed(run_root / "receipts/setup.json", "fereg-f0-scc-setup-v1")
    manifest_path = run_root / "input/manifest.json"
    manifest = load_regression_manifest(manifest_path)
    if sha256_file(manifest_path) != preparation["manifest"]["sha256"] or setup["manifest"] != preparation["manifest"]:
        raise SccContractError("collector manifest identities disagree")
    cells = validate_task_inventory(run_root, manifest, setup)
    return {
        "schema_version": 1,
        "evidence_type": EVIDENCE_TYPE,
        "profile": "scc_governing",
        "manifest": dict(preparation["manifest"]),
        "baseline": {"source": setup["arms"]["baseline"]["source"], "artifact": setup["arms"]["baseline"]["artifact"]},
        "candidate": {"source": setup["arms"]["candidate"]["source"], "artifact": setup["arms"]["candidate"]["artifact"]},
        "cells": cells,
    }


def collect_run(run_root: Path) -> None:
    evidence = assemble_evidence(run_root)
    evidence_path = run_root / "output/evidence.json"
    write_json_exclusive(evidence_path, evidence)
    candidate_source = run_root / "work/setup/candidate/source"
    evaluator = candidate_source / "scripts/fereg_f0_regression.py"
    receipt = run_root / "output/qualification.json"
    completed = subprocess.run(
        [sys.executable, str(evaluator), "evaluate", "--manifest", str(run_root / "input/manifest.json"), "--evidence", str(evidence_path), "--output", str(receipt)],
        cwd=candidate_source,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    (run_root / "logs/collector-evaluator.log").write_text(completed.stdout, encoding="utf-8")
    if completed.returncode != 0 or not receipt.is_file():
        raise SccContractError("platform-independent F0 evaluator rejected SCC evidence")
    payload = {
        "schema_version": RECEIPT_SCHEMA,
        "receipt_type": "fereg-f0-scc-collector-v1",
        "run_id": run_root.name,
        "manifest": evidence["manifest"],
        "cell_count": len(evidence["cells"]),
        "evidence_sha256": sha256_file(evidence_path),
        "qualification_sha256": sha256_file(receipt),
        "sge": {"job_id": os.environ.get("JOB_ID", ""), "host_id": socket.gethostname(), "slots": int(os.environ.get("NSLOTS", "1"))},
        "terminal_sentinel": "FEREG_F0_COLLECTOR_SUCCESS",
    }
    write_sealed(run_root / "receipts/collector.json", payload)
    _write_exclusive(run_root / "receipts/collector.SUCCESS", b"FEREG_F0_COLLECTOR_SUCCESS\n")
    print("FEREG_F0_COLLECTOR_SUCCESS", flush=True)


def record_submission(run_root: Path, setup_job: str, smoke_job: str, array_job: str, collector_job: str) -> None:
    for value in (setup_job, smoke_job, array_job, collector_job):
        if not JOB_ID_RE.fullmatch(value):
            raise SccContractError("qsub did not return a numeric job ID")
    preparation = read_sealed(run_root / "input/preparation.json", "fereg-f0-scc-preparation-v1")
    payload = {
        "schema_version": RECEIPT_SCHEMA,
        "receipt_type": "fereg-f0-scc-submission-v1",
        "run_id": run_root.name,
        "project": "welfgr",
        "jobs": {"setup": setup_job, "smoke": smoke_job, "array": array_job, "collector": collector_job},
        "array_tasks": preparation["tasks"]["count"],
        "resources": {
            "setup": {"slots": 16, "mem_per_core": "4G", "h_rt": "04:00:00"},
            "smoke": {"slots": 16, "mem_per_core": "4G", "h_rt": "11:45:00", "tasks": [1]},
            "array": {"slots": 16, "mem_per_core": "4G", "h_rt": "11:45:00"},
            "collector": {"slots": 1, "mem_per_core": "4G", "h_rt": "00:30:00"},
        },
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


def validate_qacct(text: str, job_id: str, task_ids: Sequence[int] | None) -> list[dict[str, str]]:
    records = parse_qacct(text)
    selected = [record for record in records if record.get("jobnumber") == job_id]
    if not selected:
        raise SccContractError(f"qacct has no record for job {job_id}")
    if task_ids is None:
        if len(selected) != 1:
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
        if record.get("project") != "welfgr" or record.get("failed") != "0" or record.get("exit_status") != "0":
            raise SccContractError(f"job {job_id} has failed or wrong-project accounting")
        for required in ("hostname", "qname", "slots", "ru_wallclock", "cpu", "maxvmem"):
            if not record.get(required):
                raise SccContractError(f"job {job_id} qacct omits {required}")
    return selected


def _fetch_qacct(
    job_id: str,
    task_ids: Sequence[int] | None,
    attempts: int = 30,
    delay_seconds: float = 10.0,
) -> str:
    last_error: SccContractError | None = None
    for attempt in range(attempts):
        completed = subprocess.run(["qacct", "-j", job_id], check=False, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        if completed.returncode == 0:
            try:
                validate_qacct(completed.stdout, job_id, task_ids)
            except SccContractError as exc:
                last_error = exc
            else:
                return completed.stdout
        if attempt + 1 < attempts:
            time.sleep(delay_seconds)
    detail = f": {last_error}" if last_error is not None else ""
    raise SccContractError(f"qacct remained unavailable or incomplete for job {job_id}{detail}")


def close_run(run_root: Path, qacct_dir: Path | None = None) -> None:
    submission = read_sealed(run_root / "receipts/submission.json", "fereg-f0-scc-submission-v1")
    collector = read_sealed(run_root / "receipts/collector.json", "fereg-f0-scc-collector-v1")
    if not (run_root / "receipts/setup.SUCCESS").is_file() or not (run_root / "receipts/collector.SUCCESS").is_file():
        raise SccContractError("run cannot close without setup and collector sentinels")
    manifest = load_regression_manifest(run_root / "input/manifest.json")
    validate_task_inventory(run_root, manifest, read_sealed(run_root / "receipts/setup.json", "fereg-f0-scc-setup-v1"))
    accounting: dict[str, Any] = {}
    for stage in ("setup", "smoke", "array", "collector"):
        job_id = str(submission["jobs"][stage])
        if stage == "smoke":
            expected: Sequence[int] | None = [1]
        elif stage == "array":
            expected = list(range(2, len(scc_cells(manifest)) + 1))
        else:
            expected = None
        if qacct_dir is None:
            text = _fetch_qacct(job_id, expected)
        else:
            path = qacct_dir / f"{stage}.txt"
            text = path.read_text(encoding="utf-8")
        records = validate_qacct(text, job_id, expected)
        destination = run_root / f"receipts/qacct/{stage}.txt"
        _write_exclusive(destination, text.encode("utf-8"))
        accounting[stage] = {"job_id": job_id, "records": len(records), "sha256": sha256_file(destination)}
    qualification = run_root / "output/qualification.json"
    evidence = run_root / "output/evidence.json"
    if sha256_file(qualification) != collector["qualification_sha256"] or sha256_file(evidence) != collector["evidence_sha256"]:
        raise SccContractError("collector output changed before accounting closure")
    payload = {
        "schema_version": RECEIPT_SCHEMA,
        "receipt_type": "fereg-f0-scc-closed-run-v1",
        "status": "success",
        "run_id": run_root.name,
        "manifest": dict(collector["manifest"]),
        "jobs": dict(submission["jobs"]),
        "accounting": accounting,
        "cells": [cell["id"] for cell in scc_cells(manifest)],
        "evidence_sha256": collector["evidence_sha256"],
        "qualification_sha256": collector["qualification_sha256"],
        "setup_receipt_sha256": sha256_file(run_root / "receipts/setup.json"),
        "collector_receipt_sha256": sha256_file(run_root / "receipts/collector.json"),
        "terminal_sentinel": "FEREG_F0_SCC_RUN_SUCCESS",
    }
    write_sealed(run_root / "receipts/run.json", payload)
    _write_exclusive(run_root / "receipts/RUN.SUCCESS", b"FEREG_F0_SCC_RUN_SUCCESS\n")
    print("FEREG_F0_SCC_RUN_SUCCESS", flush=True)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    prepare = commands.add_parser("prepare", help="create immutable exact-source run inputs locally")
    prepare.add_argument("--run-root", type=Path, required=True)
    prepare.add_argument("--baseline-repo", type=Path, required=True)
    prepare.add_argument("--baseline-commit", required=True)
    prepare.add_argument("--candidate-repo", type=Path, required=True)
    prepare.add_argument("--candidate-commit", required=True)
    prepare.add_argument("--manifest", type=Path, required=True)

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
    submission.add_argument("--setup-job", required=True)
    submission.add_argument("--smoke-job", required=True)
    submission.add_argument("--array-job", required=True)
    submission.add_argument("--collector-job", required=True)

    close = commands.add_parser("close", help="validate qacct and publish the final success receipt")
    close.add_argument("--run-root", type=Path, required=True)
    close.add_argument("--qacct-dir", type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "prepare":
            prepare_run(
                run_root=args.run_root,
                baseline_repo=args.baseline_repo,
                baseline_commit=args.baseline_commit,
                candidate_repo=args.candidate_repo,
                candidate_commit=args.candidate_commit,
                manifest_path=args.manifest,
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
            record_submission(args.run_root, args.setup_job, args.smoke_job, args.array_job, args.collector_job)
        elif args.command == "close":
            close_run(args.run_root, args.qacct_dir)
        else:  # pragma: no cover
            raise AssertionError(args.command)
    except (SccContractError, OSError, ValueError, KeyError, TypeError) as exc:
        print(f"fereg F0 SCC contract failed: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
