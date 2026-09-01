#!/usr/bin/env python3
"""Closed SCC bootstrap for one exact Linux licensed-Stata load artifact.

This lifecycle exists before the 33-cell PPML qualification prepare step.  It
archives one committed candidate, builds/audits/loader-tests it in one SGE
job, captures the exact plugin load in licensed Stata, and closes the result
locally only after qacct and all immutable output identities agree.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
from datetime import datetime, timezone
from typing import Any, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[2]
PLATFORM_RECEIPT_PATH = ROOT / "scripts/feppml_platform_load_receipt.py"

# The formal controller is launched with ``python -I -B -S`` from arbitrary
# local and SCC working directories.  In that mode Python intentionally omits
# both the caller's cwd and user site paths, so make the reviewed repository
# roots explicit before loading the sibling receipt module.
for import_root in (ROOT / "scripts", ROOT):
    import_root_text = str(import_root)
    if import_root_text not in sys.path:
        sys.path.insert(0, import_root_text)


def _module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


platform_receipts = _module(
    "feppml_linux_platform_receipts", PLATFORM_RECEIPT_PATH
)
qualification = platform_receipts.qualification

PROJECT_ROOT = Path("/projectnb/welfgr/fereg")
RUN_ID = re.compile(r"^[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
JOB_ID = re.compile(r"^[1-9][0-9]*$")
EXPECTED_RUSTC = "rustc 1.85.1 (4eb161250 2025-03-15)"
EXPECTED_CARGO = "cargo 1.85.1 (d73d2caf9 2024-12-31)"
SGE_OWNER = "johannes"
SGE_PROJECT = "welfgr"
SGE_JOB_ROLE = "licensed-stata-platform-bootstrap"
SGE_PARALLEL_ENVIRONMENT = "omp"
SGE_SLOTS = 4
SGE_H_RT = "01:00:00"
SGE_MEM_PER_CORE = "4G"
HARNESS_PATHS = (
    "benchmarks/scc/feppml_linux_platform_scc.py",
    "benchmarks/scc/feppml_linux_platform_deploy.sh",
    "benchmarks/scc/feppml_linux_platform_job.sh",
    "benchmarks/scc/feppml_linux_platform_submit.sh",
    "benchmarks/scc/feppml_linux_platform_collect.sh",
    "scripts/capture_feppml_platform_load.py",
    "scripts/feppml_platform_load_receipt.py",
    "scripts/feppml_qualification.py",
    "scripts/ppml_legacy_boundary.py",
    "scripts/prepare_windows_feppml_target.py",
    "tests/stata/installation/feppml_platform_load_capture.do",
)
SOURCE_IDENTITY_PATHS = (
    "Cargo.lock",
    "scripts/build_linux.sh",
    "scripts/audit_binary.py",
    "tests/mock_spi/run_actual_unix_plugin_test.sh",
    "tests/mock_spi/actual_plugin_loader_test.c",
)


class BootstrapError(RuntimeError):
    """A lifecycle identity, scheduler record, or output failed closed."""


def canonical(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii") + b"\n"


def digest_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def digest_file(path: Path) -> str:
    return qualification.sha256_file(path)


def _write(path: Path, data: bytes, mode: int = 0o444) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError as error:
        raise BootstrapError(f"refusing to overwrite immutable output: {path}") from error
    try:
        with os.fdopen(descriptor, "wb") as target:
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
        path.chmod(mode)
    except BaseException:
        try:
            path.unlink()
        except OSError:
            pass
        raise


def write_json(path: Path, value: Mapping[str, Any]) -> None:
    _write(path, json.dumps(value, sort_keys=True, indent=2).encode("ascii") + b"\n")


def write_sealed(path: Path, payload: Mapping[str, Any]) -> None:
    write_json(path, qualification.seal_payload(payload))


def _read(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise BootstrapError(f"cannot read {where}: {error}") from error
    if not isinstance(value, Mapping):
        raise BootstrapError(f"{where} must be an object")
    return value


def _read_record(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise BootstrapError(f"{where} must be an object")
    return value


def read_sealed(path: Path, kind: str) -> Mapping[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise BootstrapError("sealed lifecycle receipt is absent or a symlink")
    document = _read(path, "sealed lifecycle receipt")
    if set(document) != {"payload", "seal"}:
        raise BootstrapError("sealed lifecycle receipt is not closed")
    payload = document["payload"]
    if (
        not isinstance(payload, Mapping)
        or document["seal"] != qualification.seal_payload(payload)["seal"]
        or payload.get("kind") != kind
    ):
        raise BootstrapError("sealed lifecycle receipt identity/digest is invalid")
    return payload


def _run(
    command: Sequence[str],
    *,
    cwd: Path | None = None,
    environment: Mapping[str, str] | None = None,
) -> bytes:
    completed = subprocess.run(
        list(command),
        cwd=cwd,
        env=None if environment is None else dict(environment),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise BootstrapError(
            f"command failed rc={completed.returncode}: {detail[:500]}"
        )
    return completed.stdout


def _git(repo: Path, *arguments: str) -> bytes:
    return _run(["git", "-C", str(repo), *arguments])


def _run_root(path: Path, *, remote: bool | None = None) -> Path:
    resolved = path.resolve()
    if RUN_ID.fullmatch(resolved.name) is None:
        raise BootstrapError("run root basename must be a sortable UTC run ID")
    if remote is True and resolved.parent != PROJECT_ROOT / "runs":
        raise BootstrapError("SCC run root is outside the fereg project run directory")
    return resolved


def _file_record(path: Path, relative_to: Path) -> dict[str, Any]:
    source = Path(path)
    if source.is_symlink():
        raise BootstrapError("evidence file is a symlink")
    resolved = source.resolve()
    root = relative_to.resolve()
    if not resolved.is_file() or resolved.is_symlink():
        raise BootstrapError("evidence file is not a regular file")
    try:
        relative = resolved.relative_to(root).as_posix()
    except ValueError as error:
        raise BootstrapError("evidence file is outside the exact run root") from error
    return {
        "path": relative,
        "sha256": digest_file(resolved),
        "size_bytes": resolved.stat().st_size,
    }


def _run_file(root: Path, relative: Any, where: str) -> Path:
    if not isinstance(relative, str):
        raise BootstrapError(f"{where} path is not a string")
    candidate = Path(relative)
    if candidate.is_absolute() or ".." in candidate.parts:
        raise BootstrapError(f"{where} path escapes the exact run root")
    source = root / candidate
    if source.is_symlink():
        raise BootstrapError(f"{where} is a symlink")
    path = source.resolve()
    try:
        path.relative_to(root.resolve())
    except ValueError as error:
        raise BootstrapError(f"{where} path escapes the exact run root") from error
    return path


def _verify_file_record(root: Path, value: Any, where: str) -> Path:
    record = _read_record(value, where)
    if set(record) != {"path", "sha256", "size_bytes"}:
        raise BootstrapError(f"{where} file record is not closed")
    path = _run_file(root, record["path"], where)
    if (
        not path.is_file()
        or path.is_symlink()
        or isinstance(record["size_bytes"], bool)
        or not isinstance(record["size_bytes"], int)
        or record["size_bytes"] < 1
        or path.stat().st_size != record["size_bytes"]
        or not isinstance(record["sha256"], str)
        or re.fullmatch(r"[0-9a-f]{64}", record["sha256"]) is None
        or digest_file(path) != record["sha256"]
    ):
        raise BootstrapError(f"{where} bytes are absent or changed")
    return path


def _git_bundle(repo: Path, commit: str, branch: str) -> bytes:
    """Return a self-contained bundle in which ``commit`` resolves exactly."""

    with tempfile.TemporaryDirectory(prefix=f"feppml-{branch}-bundle-") as temporary:
        bundle_repo = Path(temporary) / "repo.git"
        _run(["git", "init", "--bare", "-q", str(bundle_repo)])
        _run(
            [
                "git", "-C", str(bundle_repo), "fetch", "--no-tags",
                str(repo), f"{commit}:refs/heads/{branch}",
            ]
        )
        return _run(
            [
                "git", "-C", str(bundle_repo), "bundle", "create", "-",
                f"refs/heads/{branch}",
            ]
        )


def _commit_build_date_utc(repo: Path, commit: str) -> str:
    raw = _git(repo, "show", "-s", "--format=%ct", commit).decode("ascii").strip()
    try:
        epoch = int(raw)
        value = datetime.fromtimestamp(epoch, timezone.utc)
    except ValueError as error:
        raise BootstrapError("target commit timestamp is not an exact epoch") from error
    return value.strftime("%Y-%m-%d")


def _formal_build_environment(
    source: Mapping[str, Any], *, slots: str | int
) -> dict[str, str]:
    environment = dict(os.environ)
    environment["FEREG_GIT_COMMIT"] = str(source["embedded_git_commit"])
    environment["FEREG_BUILD_DATE"] = str(source["build_date_utc"])
    environment["CARGO_BUILD_JOBS"] = str(int(slots))
    return environment


def _remote_run_root(run_id: str) -> Path:
    return PROJECT_ROOT / "runs" / run_id


def _job_name(run_id: str) -> str:
    name = f"feppml_platform_{run_id}"
    if len(name) > 127 or re.fullmatch(r"[A-Za-z0-9_-]+", name) is None:
        raise BootstrapError("run ID cannot form an exact SGE job name")
    return name


def _submission_ledger(root: Path) -> dict[str, Any]:
    run_id = root.name
    remote = _remote_run_root(run_id)
    script_relative = "input/harness/benchmarks/scc/feppml_linux_platform_job.sh"
    script = root / script_relative
    job_name = _job_name(run_id)
    scriptfile = remote / script_relative
    stdout_log = remote / "logs/job.log"
    argv = [
        "qsub", "-terse", "-P", SGE_PROJECT, "-N", job_name, "-cwd",
        "-pe", SGE_PARALLEL_ENVIRONMENT, str(SGE_SLOTS),
        "-l", f"h_rt={SGE_H_RT}", "-l", f"mem_per_core={SGE_MEM_PER_CORE}",
        "-o", str(stdout_log), str(scriptfile), str(remote),
    ]
    return {
        "record_type": "feppml-sge-submission-ledger-v1",
        "owner": SGE_OWNER,
        "project": SGE_PROJECT,
        "job_role": SGE_JOB_ROLE,
        "job_name": job_name,
        "remote_run_root": str(remote),
        "submit_cwd": str(remote),
        "stdout_log": str(stdout_log),
        "scriptfile": str(scriptfile),
        "submitted_script": _file_record(script, root),
        "argv": argv,
    }


def prepare(args: argparse.Namespace) -> None:
    run_root = _run_root(args.run_root)
    if run_root.exists():
        raise BootstrapError("run root already exists; retries require a new run ID")
    repo = args.candidate_repo.resolve()
    commit = args.candidate_commit
    if COMMIT.fullmatch(commit) is None:
        raise BootstrapError("candidate commit must be a full lowercase SHA")
    if _git(repo, "rev-parse", f"{commit}^{{commit}}").decode().strip() != commit:
        raise BootstrapError("candidate commit did not resolve exactly")
    tree = _git(repo, "rev-parse", f"{commit}^{{tree}}").decode().strip()
    archive = _git(repo, "archive", "--format=tar", commit)
    source_bundle = _git_bundle(repo, commit, "target")
    source = {
        "commit": commit,
        "tree": tree,
        "archive_sha256": digest_bytes(archive),
        "archive_size_bytes": len(archive),
        "bundle_sha256": digest_bytes(source_bundle),
        "bundle_size_bytes": len(source_bundle),
        "embedded_git_commit": commit[:12],
        "build_date_utc": _commit_build_date_utc(repo, commit),
    }
    harness_repo = args.harness_repo.resolve()
    harness_commit = args.harness_commit
    if COMMIT.fullmatch(harness_commit) is None:
        raise BootstrapError("harness commit must be a full lowercase SHA")
    if (
        _git(harness_repo, "rev-parse", f"{harness_commit}^{{commit}}")
        .decode()
        .strip()
        != harness_commit
    ):
        raise BootstrapError("harness commit did not resolve exactly")
    harness_source = {
        "commit": harness_commit,
        "tree": _git(
            harness_repo, "rev-parse", f"{harness_commit}^{{tree}}"
        ).decode().strip(),
    }
    controller_archive = _git(
        harness_repo, "archive", "--format=tar", harness_commit
    )
    controller_bundle = _git_bundle(harness_repo, harness_commit, "controller")
    harness_source.update(
        {
            "archive_sha256": digest_bytes(controller_archive),
            "archive_size_bytes": len(controller_archive),
            "bundle_sha256": digest_bytes(controller_bundle),
            "bundle_size_bytes": len(controller_bundle),
        }
    )
    input_root = run_root / "input"
    _write(input_root / "source.tar", archive)
    _write(input_root / "source.bundle", source_bundle)
    _write(input_root / "controller.tar", controller_archive)
    _write(input_root / "controller.bundle", controller_bundle)
    harness: dict[str, Any] = {}
    for relative in HARNESS_PATHS:
        content = _git(
            harness_repo, "show", f"{harness_commit}:{relative}"
        )
        destination = input_root / "harness" / relative
        _write(destination, content, mode=0o555 if relative.endswith((".py", ".sh")) else 0o444)
        harness[relative] = {
            "sha256": digest_bytes(content),
            "size_bytes": len(content),
        }
    source_files = {}
    for relative in SOURCE_IDENTITY_PATHS:
        content = _git(repo, "show", f"{commit}:{relative}")
        source_files[relative] = {
            "sha256": digest_bytes(content),
            "size_bytes": len(content),
        }
    for directory in (run_root / "logs", run_root / "work", run_root / "output", run_root / "receipts"):
        directory.mkdir(parents=True, exist_ok=True)
    files = {}
    for path in sorted(input_root.rglob("*")):
        if path.is_file():
            files[path.relative_to(run_root).as_posix()] = {
                "sha256": digest_file(path),
                "size_bytes": path.stat().st_size,
            }
    payload = {
        "schema_version": 1,
        "kind": "feppml_linux_platform_bootstrap_preparation",
        "run_id": run_root.name,
        "platform": "linux-x86_64",
        "source": source,
        "harness_source": harness_source,
        "source_files": source_files,
        "harness": harness,
        "toolchain": {
            "rustc": EXPECTED_RUSTC,
            "cargo": EXPECTED_CARGO,
            "rustup_toolchain": "1.85.1-x86_64-unknown-linux-gnu",
            "stata_module": "stata-mp/19",
            "python_module": "python3/3.12.4",
        },
        "files": files,
        "complete_sentinel": "FEPPML_LINUX_PLATFORM_PREPARED",
    }
    preparation = input_root / "preparation.json"
    write_sealed(preparation, payload)
    _write(
        input_root / "PREPARED",
        f"FEPPML_LINUX_PLATFORM_PREPARED|{digest_file(preparation)}\n".encode("ascii"),
    )


def verify_input(run_root: Path) -> Mapping[str, Any]:
    root = _run_root(run_root)
    path = root / "input/preparation.json"
    payload = read_sealed(path, "feppml_linux_platform_bootstrap_preparation")
    prepared_marker = root / "input/PREPARED"
    if (
        payload.get("schema_version") != 1
        or payload.get("run_id") != root.name
        or payload.get("platform") != "linux-x86_64"
        or payload.get("complete_sentinel") != "FEPPML_LINUX_PLATFORM_PREPARED"
        or prepared_marker.is_symlink()
        or not prepared_marker.is_file()
        or prepared_marker.read_text(encoding="ascii")
        != f"FEPPML_LINUX_PLATFORM_PREPARED|{digest_file(path)}\n"
    ):
        raise BootstrapError("prepared input identity/sentinel is invalid")
    files = _read_record(payload.get("files"), "prepared input inventory")
    for relative, expected in files.items():
        try:
            expected_record = _read_record(expected, str(relative))
            if set(expected_record) != {"sha256", "size_bytes"}:
                raise BootstrapError("prepared input file record is not closed")
            _verify_file_record(
                root,
                {"path": relative, **dict(expected_record)},
                f"prepared input {relative}",
            )
        except BootstrapError as error:
            raise BootstrapError(f"prepared input changed: {relative}") from error
    return payload


def record_deployment(args: argparse.Namespace) -> None:
    root = _run_root(args.run_root, remote=True)
    preparation = verify_input(root)
    preparation_sha = digest_file(root / "input/preparation.json")
    if args.preparation_sha256 != preparation_sha:
        raise BootstrapError("deployed input differs from local preparation")
    write_sealed(
        root / "receipts/deployment.json",
        {
            "schema_version": 1,
            "kind": "feppml_linux_platform_bootstrap_deployment",
            "run_id": root.name,
            "source_commit": preparation["source"]["commit"],
            "preparation_sha256": preparation_sha,
            "complete_sentinel": "FEPPML_LINUX_PLATFORM_DEPLOYED",
        },
    )


def verify_deployment(run_root: Path) -> Mapping[str, Any]:
    root = _run_root(run_root)
    preparation = verify_input(root)
    receipt = read_sealed(
        root / "receipts/deployment.json",
        "feppml_linux_platform_bootstrap_deployment",
    )
    if (
        receipt.get("schema_version") != 1
        or receipt.get("run_id") != root.name
        or receipt.get("source_commit") != preparation["source"]["commit"]
        or receipt.get("preparation_sha256")
        != digest_file(root / "input/preparation.json")
        or receipt.get("complete_sentinel") != "FEPPML_LINUX_PLATFORM_DEPLOYED"
    ):
        raise BootstrapError("deployment receipt differs from prepared input")
    return receipt


def record_submission(args: argparse.Namespace) -> None:
    root = _run_root(args.run_root)
    preparation = verify_input(root)
    verify_deployment(root)
    if JOB_ID.fullmatch(args.job_id) is None:
        raise BootstrapError("SGE job ID is malformed")
    ledger = _submission_ledger(root)
    write_sealed(
        root / "receipts/submission.json",
        {
            "schema_version": 1,
            "kind": "feppml_linux_platform_bootstrap_submission",
            "run_id": root.name,
            "source_commit": preparation["source"]["commit"],
            "harness_commit": preparation["harness_source"]["commit"],
            "job_id": args.job_id,
            "slots": SGE_SLOTS,
            "h_rt": SGE_H_RT,
            "mem_per_core": SGE_MEM_PER_CORE,
            "ledger": ledger,
            "complete_sentinel": "FEPPML_LINUX_PLATFORM_SUBMITTED",
        },
    )


def verify_submission(run_root: Path) -> Mapping[str, Any]:
    root = _run_root(run_root)
    preparation = verify_input(root)
    verify_deployment(root)
    receipt = read_sealed(
        root / "receipts/submission.json",
        "feppml_linux_platform_bootstrap_submission",
    )
    if (
        receipt.get("schema_version") != 1
        or receipt.get("run_id") != root.name
        or receipt.get("source_commit") != preparation["source"]["commit"]
        or receipt.get("harness_commit")
        != preparation["harness_source"]["commit"]
        or JOB_ID.fullmatch(str(receipt.get("job_id"))) is None
        or receipt.get("slots") != SGE_SLOTS
        or receipt.get("h_rt") != SGE_H_RT
        or receipt.get("mem_per_core") != SGE_MEM_PER_CORE
        or receipt.get("ledger") != _submission_ledger(root)
        or receipt.get("complete_sentinel") != "FEPPML_LINUX_PLATFORM_SUBMITTED"
    ):
        raise BootstrapError("submission receipt is invalid")
    return receipt


def _extract(archive: Path, destination: Path) -> None:
    if destination.exists():
        raise BootstrapError("source workspace already exists")
    destination.mkdir(parents=True)
    with tarfile.open(archive, "r:") as source:
        for member in source.getmembers():
            relative = Path(member.name)
            if (
                relative.is_absolute()
                or ".." in relative.parts
                or member.issym()
                or member.islnk()
                or member.isdev()
            ):
                raise BootstrapError(f"unsafe source archive member: {member.name}")
        source.extractall(destination)


def _platform_source_identity(source: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "source_commit": source["commit"],
        "source_tree": source["tree"],
        "source_archive_sha256": source["archive_sha256"],
        "source_archive_size_bytes": source["archive_size_bytes"],
        "embedded_git_commit": source["embedded_git_commit"],
        "build_date_utc": source["build_date_utc"],
    }


def _platform_controller_identity(controller: Mapping[str, Any]) -> dict[str, str]:
    return {
        "controller_commit": str(controller["commit"]),
        "controller_tree": str(controller["tree"]),
    }


def _native_capture_record(
    *,
    root: Path,
    source: Mapping[str, Any],
    controller: Mapping[str, Any],
    artifact: Path,
    capture_path: Path,
    capture: Mapping[str, Any],
    capture_program: Path,
    attestation: Path,
) -> dict[str, Any]:
    """Validate native evidence and rewrite its file records run-relative."""

    sidecars = platform_receipts.native_sidecar_paths(attestation)
    producer = root / "input/harness/scripts/capture_feppml_platform_load.py"
    observed = platform_receipts._native_provenance(
        attestation=attestation,
        stata_executable=None,
        status_file=sidecars["status"],
        stata_log=sidecars["stata_log"],
        wrapper=sidecars["wrapper"],
        invocation=sidecars["invocation"],
        platform="linux-x86_64",
        source=_platform_source_identity(source),
        controller=_platform_controller_identity(controller),
        artifact=artifact,
        capture_path=capture_path,
        capture=capture,
        capture_program=capture_program,
        producer=producer,
    )
    result: dict[str, Any] = {
        "record_type": "feppml-native-capture-controller-v1",
        "outer_authority": "scc-closed-run",
    }
    files = {"producer": producer, "attestation": attestation, **sidecars}
    for field, path in files.items():
        relative = _file_record(path, root)
        if (
            observed.get(field, {}).get("sha256") != relative["sha256"]
            or observed.get(field, {}).get("size_bytes") != relative["size_bytes"]
        ):
            raise BootstrapError(
                f"native capture {field} differs from its execution attestation"
            )
        result[field] = relative
    return result


def _native_provenance_identity(native: Mapping[str, Any]) -> dict[str, Any]:
    if (
        native.get("record_type") != "feppml-native-capture-controller-v1"
        or native.get("outer_authority") != "scc-closed-run"
    ):
        raise BootstrapError("native capture authority is invalid")
    result: dict[str, Any] = {
        "record_type": "feppml-native-capture-controller-v1",
        "outer_authority": "scc-closed-run",
    }
    for field, normalized in (
        ("producer", "producer"),
        ("attestation", "attestation"),
        ("status", "status"),
        ("stata_log", "raw_log"),
        ("wrapper", "wrapper"),
        ("invocation", "invocation"),
    ):
        record = _read_record(native.get(field), f"native capture {field}")
        result[f"{normalized}_sha256"] = record.get("sha256")
        result[f"{normalized}_size_bytes"] = record.get("size_bytes")
    return result


def run_job(run_root: Path) -> None:
    root = _run_root(run_root, remote=True)
    preparation = verify_input(root)
    verify_deployment(root)
    source_record = preparation["source"]
    source = root / "work/source"
    _run(
        [
            "git", "clone", "-q", "--no-checkout",
            str(root / "input/source.bundle"), str(source),
        ]
    )
    _run(["git", "checkout", "-q", source_record["commit"]], cwd=source)
    observed_source_commit = _run(
        ["git", "rev-parse", "HEAD^{commit}"], cwd=source
    ).decode().strip()
    observed_source_tree = _run(
        ["git", "rev-parse", "HEAD^{tree}"], cwd=source
    ).decode().strip()
    if (
        observed_source_commit != source_record["commit"]
        or observed_source_tree != source_record["tree"]
    ):
        raise BootstrapError("bundled target source differs from the exact commit")
    controller_record = preparation["harness_source"]
    controller = root / "work/controller"
    _run(
        [
            "git", "clone", "-q", "--no-checkout",
            str(root / "input/controller.bundle"), str(controller),
        ]
    )
    _run(["git", "checkout", "-q", controller_record["commit"]], cwd=controller)
    observed_controller_commit = _run(
        ["git", "rev-parse", "HEAD^{commit}"], cwd=controller
    ).decode().strip()
    observed_controller_tree = _run(
        ["git", "rev-parse", "HEAD^{tree}"], cwd=controller
    ).decode().strip()
    if (
        observed_controller_commit != controller_record["commit"]
        or observed_controller_tree != controller_record["tree"]
    ):
        raise BootstrapError("extracted controller tree differs from the exact harness")
    for relative, expected in preparation["source_files"].items():
        path = source / relative
        if (
            not path.is_file()
            or path.stat().st_size != expected["size_bytes"]
            or digest_file(path) != expected["sha256"]
        ):
            raise BootstrapError(f"exact source file changed: {relative}")
    rustc = _run(["rustc", "--version"]).decode().strip()
    cargo = _run(["cargo", "--version"]).decode().strip()
    if rustc != EXPECTED_RUSTC or cargo != EXPECTED_CARGO:
        raise BootstrapError("SCC Rust toolchain differs from the frozen bootstrap")
    environment = _formal_build_environment(
        source_record, slots=os.environ.get("NSLOTS", "1")
    )
    build = subprocess.run(
        ["bash", "scripts/build_linux.sh"],
        cwd=source,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    _write(root / "logs/build.log", build.stdout)
    plugin = source / "dist/fereg_linux.plugin"
    if build.returncode or not plugin.is_file():
        raise BootstrapError(f"exact Linux build/audit/loader returned {build.returncode}")
    plugin_bytes = plugin.read_bytes()
    if (
        source_record["embedded_git_commit"].encode("ascii") not in plugin_bytes
        or source_record["build_date_utc"].encode("ascii") not in plugin_bytes
    ):
        raise BootstrapError(
            "Linux plugin omits its exact source commit or commit-derived build date"
        )
    output_plugin = root / "output/fereg_linux.plugin"
    _write(output_plugin, plugin.read_bytes())
    stata = shutil.which("stata-mp") or shutil.which("stata")
    if stata is None:
        raise BootstrapError("licensed Stata is unavailable after module load")
    capture_path = root / "output/capture.json"
    attestation_path = root / "output/native-attestation.json"
    capture_command = [
        sys.executable,
        str(
            controller / "scripts/capture_feppml_platform_load.py"
        ),
        "--platform",
        "linux-x86_64",
        "--artifact",
        str(output_plugin),
        "--stata",
        stata,
        "--output",
        str(capture_path),
        "--source-repo",
        str(source),
        "--source-commit",
        source_record["commit"],
        "--source-tree",
        source_record["tree"],
        "--controller-repo",
        str(controller),
        "--controller-commit",
        controller_record["commit"],
        "--controller-tree",
        controller_record["tree"],
        "--source-archive",
        str(root / "input/source.tar"),
        "--attestation",
        str(attestation_path),
    ]
    completed = subprocess.run(
        capture_command,
        cwd=source,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        text=True,
    )
    if (
        completed.returncode
        or completed.stdout.strip() != "FEPPML_LICENSED_STATA_LOAD_CAPTURE_COMPLETE"
        or completed.stderr
    ):
        raise BootstrapError("licensed Linux Stata load capture failed")
    capture = platform_receipts.validate_capture(
        _read(capture_path, "licensed Linux capture"),
        output_plugin,
        "linux-x86_64",
    )
    scheduler_job_id = os.environ.get("JOB_ID", "")
    scheduler_job_name = os.environ.get("JOB_NAME", "")
    if JOB_ID.fullmatch(scheduler_job_id) is None:
        raise BootstrapError("SGE did not provide a scalar job identity")
    if scheduler_job_name != _job_name(root.name):
        raise BootstrapError("SGE job name differs from the exact run and role")
    native_capture = _native_capture_record(
        root=root,
        source=source_record,
        controller=controller_record,
        artifact=output_plugin,
        capture_path=capture_path,
        capture=capture,
        capture_program=(
            root
            / "input/harness/tests/stata/installation/feppml_platform_load_capture.do"
        ),
        attestation=attestation_path,
    )
    payload = {
        "schema_version": 1,
        "kind": "feppml_linux_platform_bootstrap_job",
        "run_id": root.name,
        "job_id": scheduler_job_id,
        "job_name": scheduler_job_name,
        "platform": "linux-x86_64",
        "source": dict(source_record),
        "harness_source": dict(preparation["harness_source"]),
        "build_date_utc": source_record["build_date_utc"],
        "rustc": rustc,
        "cargo": cargo,
        "artifact": _file_record(output_plugin, root),
        "build_log": _file_record(root / "logs/build.log", root),
        "capture": _file_record(capture_path, root),
        "native_capture": native_capture,
        "capture_program_sha256": capture["capture_program_sha256"],
        "stata_version": capture["stata_version"],
        "stata_flavor": capture["stata_flavor"],
        "licensed_stata_load": True,
        "mock_only": False,
        "scientific_claim": False,
        "build_binary_audit_actual_loader": "success",
        "complete_sentinel": "FEPPML_LINUX_PLATFORM_JOB_COMPLETE",
    }
    receipt = root / "receipts/job.json"
    write_sealed(receipt, payload)
    _write(
        root / "JOB.SUCCESS",
        f"FEPPML_LINUX_PLATFORM_JOB_COMPLETE|{digest_file(receipt)}\n".encode("ascii"),
    )


def verify_job(run_root: Path) -> Mapping[str, Any]:
    root = _run_root(run_root)
    preparation = verify_input(root)
    verify_deployment(root)
    receipt_path = root / "receipts/job.json"
    receipt = read_sealed(
        receipt_path, "feppml_linux_platform_bootstrap_job"
    )
    if (
        receipt.get("schema_version") != 1
        or receipt.get("run_id") != root.name
        or JOB_ID.fullmatch(str(receipt.get("job_id"))) is None
        or receipt.get("job_name") != _job_name(root.name)
        or receipt.get("platform") != "linux-x86_64"
        or receipt.get("source") != preparation["source"]
        or receipt.get("harness_source") != preparation["harness_source"]
        or receipt.get("build_date_utc") != preparation["source"]["build_date_utc"]
        or receipt.get("rustc") != EXPECTED_RUSTC
        or receipt.get("cargo") != EXPECTED_CARGO
        or receipt.get("licensed_stata_load") is not True
        or receipt.get("mock_only") is not False
        or receipt.get("scientific_claim") is not False
        or receipt.get("build_binary_audit_actual_loader") != "success"
        or receipt.get("complete_sentinel") != "FEPPML_LINUX_PLATFORM_JOB_COMPLETE"
        or (root / "JOB.SUCCESS").is_symlink()
        or not (root / "JOB.SUCCESS").is_file()
        or (root / "JOB.SUCCESS").read_text(encoding="ascii")
        != f"FEPPML_LINUX_PLATFORM_JOB_COMPLETE|{digest_file(receipt_path)}\n"
    ):
        raise BootstrapError("Linux platform job receipt/sentinel is invalid")
    job_paths = {
        field: _verify_file_record(
            root, receipt[field], f"Linux platform job {field}"
        )
        for field in ("artifact", "build_log", "capture")
    }
    capture = platform_receipts.validate_capture(
        _read(job_paths["capture"], "licensed Linux capture"),
        job_paths["artifact"],
        "linux-x86_64",
    )
    attestation_record = _read_record(
        receipt.get("native_capture"), "Linux native capture"
    )
    if set(attestation_record) != {
        "record_type", "outer_authority", "producer", "attestation", "status",
        "stata_log", "wrapper", "invocation",
    }:
        raise BootstrapError("Linux native capture record is not closed")
    native_paths = {
        field: _verify_file_record(
            root, attestation_record[field], f"Linux native capture {field}"
        )
        for field in (
            "producer", "attestation", "status", "stata_log", "wrapper",
            "invocation",
        )
    }
    attestation_path = native_paths["attestation"]
    observed_native = _native_capture_record(
        root=root,
        source=preparation["source"],
        controller=preparation["harness_source"],
        artifact=job_paths["artifact"],
        capture_path=job_paths["capture"],
        capture=capture,
        capture_program=(
            root
            / "input/harness/tests/stata/installation/feppml_platform_load_capture.do"
        ),
        attestation=attestation_path,
    )
    if attestation_record != observed_native:
        raise BootstrapError("Linux native capture evidence changed")
    return receipt


def _qacct_time(value: str, field: str) -> datetime:
    try:
        return datetime.strptime(value, "%a %b %d %H:%M:%S %Y")
    except ValueError as error:
        raise BootstrapError(f"qacct {field} is malformed") from error


def parse_qacct(
    path: Path,
    submission: Mapping[str, Any],
    run_root: Path,
) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise BootstrapError("qacct evidence is absent or a symlink")
    text = path.read_text(encoding="utf-8", errors="strict")
    records: list[dict[str, str]] = []
    current: dict[str, str] = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line:
            continue
        if line.startswith("===="):
            if current:
                records.append(current)
                current = {}
            continue
        key, separator, value = line.partition(" ")
        if not separator:
            continue
        value = value.strip()
        if key in current:
            raise BootstrapError("qacct record contains a duplicate field")
        current[key] = value
    if current:
        records.append(current)
    if len(records) != 1:
        raise BootstrapError("qacct must contain exactly one scalar job record")
    record = records[0]
    expected_job = str(submission.get("job_id"))
    ledger = _read_record(submission.get("ledger"), "submission ledger")
    if ledger != _submission_ledger(_run_root(run_root)):
        raise BootstrapError("submission ledger differs from exact run inputs")
    for field in (
        "owner", "project", "jobname", "jobnumber", "taskid", "qsub_time",
        "start_time", "end_time", "granted_pe", "qname", "hostname", "slots",
        "failed", "exit_status", "ru_wallclock", "cpu", "maxvmem",
    ):
        if not record.get(field):
            raise BootstrapError(f"qacct omits {field}")
    if (
        record["jobnumber"] != expected_job
        or record["owner"] != SGE_OWNER
        or record["project"] != SGE_PROJECT
        or record["jobname"] != ledger["job_name"]
        or record["taskid"] != "undefined"
        or record["granted_pe"] != SGE_PARALLEL_ENVIRONMENT
        or record["failed"] != "0"
        or record["exit_status"] != "0"
    ):
        raise BootstrapError(
            "SGE accounting does not identify the exact successful scalar job"
        )
    if record["slots"] != str(SGE_SLOTS):
        raise BootstrapError("SGE accounting slot count differs from submission")
    qsub_time = _qacct_time(record["qsub_time"], "qsub_time")
    start_time = _qacct_time(record["start_time"], "start_time")
    end_time = _qacct_time(record["end_time"], "end_time")
    if not qsub_time <= start_time <= end_time:
        raise BootstrapError("qacct scheduler timestamps are not ordered")
    scheduler_context: dict[str, Any] = {}
    if "cwd" in record:
        if record["cwd"] != ledger["submit_cwd"]:
            raise BootstrapError("qacct cwd differs from the sealed submission")
        scheduler_context["cwd"] = record["cwd"]
    if "scriptfile" in record:
        if record["scriptfile"] != ledger["scriptfile"]:
            raise BootstrapError("qacct scriptfile differs from the sealed submission")
        scheduler_context["scriptfile"] = record["scriptfile"]
    command_fields = [
        field for field in ("submit_cmd_line", "submit_cmdline") if field in record
    ]
    if len(command_fields) > 1:
        raise BootstrapError("qacct exposes duplicate submit command fields")
    if command_fields:
        field = command_fields[0]
        try:
            command = shlex.split(record[field])
        except ValueError as error:
            raise BootstrapError("qacct submit command line is malformed") from error
        if command != ledger["argv"]:
            raise BootstrapError(
                "qacct submit command differs from the sealed submission"
            )
        scheduler_context[field] = record[field]
    try:
        wallclock = float(record["ru_wallclock"])
        cpu = float(record["cpu"])
    except ValueError as error:
        raise BootstrapError("SGE accounting resources are malformed") from error
    if (
        wallclock < 0.0
        or cpu < 0.0
        or re.fullmatch(
            r"[0-9]+(?:\.[0-9]+)?(?:[KMGTPE](?:B)?|B)?",
            record["maxvmem"],
        ) is None
    ):
        raise BootstrapError("SGE accounting resources are malformed")
    return {
        "job_id": expected_job,
        "owner": SGE_OWNER,
        "project": SGE_PROJECT,
        "job_role": SGE_JOB_ROLE,
        "job_name": ledger["job_name"],
        "task_id": record["taskid"],
        "qsub_time": record["qsub_time"],
        "start_time": record["start_time"],
        "end_time": record["end_time"],
        "granted_pe": SGE_PARALLEL_ENVIRONMENT,
        "qname": record["qname"],
        "hostname": record["hostname"],
        "slots": SGE_SLOTS,
        "failed": 0,
        "exit_status": 0,
        "ru_wallclock": wallclock,
        "cpu": cpu,
        "maxvmem": record["maxvmem"],
        "scheduler_context": scheduler_context,
    }


def close(args: argparse.Namespace) -> None:
    root = _run_root(args.run_root)
    preparation = verify_input(root)
    verify_deployment(root)
    submission = verify_submission(root)
    job = verify_job(root)
    if job["job_id"] != submission["job_id"]:
        raise BootstrapError("job receipt differs from the submitted SGE job")
    if job["job_name"] != submission["ledger"]["job_name"]:
        raise BootstrapError("job receipt differs from the submitted SGE job name")
    accounting_path = root / "receipts/qacct.txt"
    accounting = parse_qacct(accounting_path, submission, root)
    source = preparation["source"]
    artifact_path = _verify_file_record(root, job["artifact"], "Linux artifact")
    capture_path = _verify_file_record(root, job["capture"], "Linux capture")
    attestation_path = _verify_file_record(
        root, job["native_capture"]["attestation"], "Linux native attestation"
    )
    platform_record_path = root / "output/platform-record.json"
    platform_receipt_path = root / "receipts/platform-load.json"
    platform_record = platform_receipts.create(
        platform="linux-x86_64",
        artifact=artifact_path,
        capture_path=capture_path,
        receipt=platform_receipt_path,
        record=platform_record_path,
        source_repo=args.candidate_repo,
        source_commit=source["commit"],
        source_archive=root / "input/source.tar",
        controller_repo=args.harness_repo,
        controller_commit=preparation["harness_source"]["commit"],
        capture_program=(
            args.harness_repo
            / "tests/stata/installation/feppml_platform_load_capture.do"
        ),
        native_attestation=attestation_path,
    )
    platform_payload = platform_receipts.verify(
        platform_receipt_path, platform_record_path
    )
    if (
        platform_record["source_commit"] != source["commit"]
        or platform_record["source_tree"] != source["tree"]
        or platform_record["embedded_git_commit"] != source["embedded_git_commit"]
        or platform_payload["source_archive_sha256"] != source["archive_sha256"]
        or platform_payload["source_archive_size_bytes"]
        != source["archive_size_bytes"]
        or platform_payload["build_date_utc"] != source["build_date_utc"]
        or platform_payload["controller_commit"]
        != preparation["harness_source"]["commit"]
        or platform_payload["controller_tree"]
        != preparation["harness_source"]["tree"]
        or platform_payload["artifact_sha256"] != job["artifact"]["sha256"]
        or platform_payload["artifact_size_bytes"]
        != job["artifact"]["size_bytes"]
        or platform_payload["capture_output_sha256"] != job["capture"]["sha256"]
        or platform_payload["capture_program_sha256"]
        != job["capture_program_sha256"]
        or platform_payload["platform_provenance"]
        != _native_provenance_identity(job["native_capture"])
    ):
        raise BootstrapError("sealed platform receipt differs from the closed job")
    payload = {
        "schema_version": 1,
        "kind": "feppml_linux_platform_bootstrap_close",
        "run_id": root.name,
        "source": dict(source),
        "harness_source": dict(preparation["harness_source"]),
        "build_date_utc": source["build_date_utc"],
        "job_id": submission["job_id"],
        "qacct": accounting,
        "qacct_sha256": digest_file(accounting_path),
        "submission_ledger": dict(submission["ledger"]),
        "build_log": dict(job["build_log"]),
        "native_capture": dict(job["native_capture"]),
        "platform_record": _file_record(platform_record_path, root),
        "platform_receipt": _file_record(platform_receipt_path, root),
        "platform_receipt_payload_sha256": platform_record["receipt"][
            "payload_sha256"
        ],
        "licensed_stata_load": True,
        "mock_only": False,
        "scientific_claim": False,
        "complete_sentinel": "FEPPML_LINUX_PLATFORM_BOOTSTRAP_COMPLETE",
    }
    close_path = root / "receipts/linux-platform-close.json"
    write_sealed(close_path, payload)
    _write(
        root / "RUN.SUCCESS",
        f"FEPPML_LINUX_PLATFORM_BOOTSTRAP_COMPLETE|{digest_file(close_path)}\n".encode("ascii"),
    )


def _closed(value: Mapping[str, Any], fields: set[str], where: str) -> None:
    if set(value) != fields:
        raise BootstrapError(f"{where} fields are not closed")


def _verify_repo_identity(
    repo: Path, identity: Mapping[str, Any], where: str
) -> None:
    resolved = repo.resolve()
    commit = str(identity["commit"])
    if (
        _git(resolved, "rev-parse", f"{commit}^{{commit}}").decode().strip()
        != commit
        or _git(resolved, "rev-parse", f"{commit}^{{tree}}").decode().strip()
        != identity["tree"]
        or (
            "build_date_utc" in identity
            and _commit_build_date_utc(resolved, commit)
            != identity["build_date_utc"]
        )
    ):
        raise BootstrapError(f"{where} repository identity differs from the closed run")


def _verify_bundled_git_identity(
    *,
    archive: Path,
    bundle: Path,
    identity: Mapping[str, Any],
    where: str,
) -> None:
    with tempfile.TemporaryDirectory(prefix="feppml-closed-git-") as temporary:
        repo = Path(temporary) / "repo"
        _run(["git", "clone", "-q", "--no-checkout", str(bundle), str(repo)])
        commit = str(identity["commit"])
        observed_commit = _git(repo, "rev-parse", f"{commit}^{{commit}}").decode(
            "ascii"
        ).strip()
        observed_tree = _git(repo, "rev-parse", f"{commit}^{{tree}}").decode(
            "ascii"
        ).strip()
        canonical_archive = _git(repo, "archive", "--format=tar", commit)
    if (
        observed_commit != commit
        or observed_tree != identity["tree"]
        or canonical_archive != archive.read_bytes()
    ):
        raise BootstrapError(
            f"{where} bundle/archive does not reconstruct its exact Git identity"
        )


def verify_closed_run(
    run_root: Path,
    *,
    candidate_repo: Path | None = None,
    harness_repo: Path | None = None,
    expected_source_commit: str | None = None,
    expected_harness_commit: str | None = None,
) -> Mapping[str, Any]:
    """Verify the complete Linux bootstrap and return normalized identities.

    This is the sole read-only schema authority consumed by F0 and Phase 12.
    Callers must not reimplement preparation/job/close JSON validation.
    """

    root = _run_root(run_root)
    preparation = verify_input(root)
    _closed(
        preparation,
        {
            "schema_version", "kind", "run_id", "platform", "source",
            "harness_source", "source_files", "harness", "toolchain", "files",
            "complete_sentinel",
        },
        "Linux preparation",
    )
    source = _read_record(preparation["source"], "Linux target source")
    _closed(
        source,
        {
            "commit", "tree", "archive_sha256", "archive_size_bytes",
            "bundle_sha256", "bundle_size_bytes", "embedded_git_commit",
            "build_date_utc",
        },
        "Linux target source",
    )
    harness = _read_record(preparation["harness_source"], "Linux controller source")
    _closed(
        harness,
        {
            "commit", "tree", "archive_sha256", "archive_size_bytes",
            "bundle_sha256", "bundle_size_bytes",
        },
        "Linux controller source",
    )
    source_files = _read_record(
        preparation["source_files"], "Linux target source files"
    )
    harness_files = _read_record(
        preparation["harness"], "Linux controller files"
    )
    prepared_files = _read_record(
        preparation["files"], "Linux prepared file inventory"
    )
    if (
        set(source_files) != set(SOURCE_IDENTITY_PATHS)
        or set(harness_files) != set(HARNESS_PATHS)
        or set(prepared_files)
        != {
            "input/source.tar", "input/source.bundle", "input/controller.tar",
            "input/controller.bundle",
            *(f"input/harness/{relative}" for relative in HARNESS_PATHS),
        }
        or preparation["toolchain"]
        != {
            "rustc": EXPECTED_RUSTC,
            "cargo": EXPECTED_CARGO,
            "rustup_toolchain": "1.85.1-x86_64-unknown-linux-gnu",
            "stata_module": "stata-mp/19",
            "python_module": "python3/3.12.4",
        }
    ):
        raise BootstrapError("Linux prepared inventory/toolchain is not closed")
    for relative in SOURCE_IDENTITY_PATHS:
        record = _read_record(source_files[relative], f"target source {relative}")
        if set(record) != {"sha256", "size_bytes"}:
            raise BootstrapError("Linux target source record is not closed")
        try:
            with tarfile.open(root / "input/source.tar", "r:") as archive:
                member = archive.getmember(relative)
                opened = archive.extractfile(member)
                content = None if opened is None else opened.read()
        except (KeyError, tarfile.TarError, OSError) as error:
            raise BootstrapError(
                f"Linux target archive omits {relative}"
            ) from error
        if (
            not member.isfile()
            or content is None
            or len(content) != record["size_bytes"]
            or digest_bytes(content) != record["sha256"]
        ):
            raise BootstrapError(f"Linux target source changed: {relative}")
    for relative in HARNESS_PATHS:
        record = _read_record(harness_files[relative], f"controller {relative}")
        prepared = _read_record(
            prepared_files[f"input/harness/{relative}"],
            f"prepared controller {relative}",
        )
        if (
            set(record) != {"sha256", "size_bytes"}
            or prepared != record
        ):
            raise BootstrapError(f"Linux controller record changed: {relative}")
        try:
            with tarfile.open(root / "input/controller.tar", "r:") as archive:
                member = archive.getmember(relative)
                opened = archive.extractfile(member)
                content = None if opened is None else opened.read()
        except (KeyError, tarfile.TarError, OSError) as error:
            raise BootstrapError(
                f"Linux controller archive omits {relative}"
            ) from error
        if (
            not member.isfile()
            or content is None
            or len(content) != record["size_bytes"]
            or digest_bytes(content) != record["sha256"]
        ):
            raise BootstrapError(f"Linux controller source changed: {relative}")
    if (
        COMMIT.fullmatch(str(source["commit"])) is None
        or COMMIT.fullmatch(str(source["tree"])) is None
        or source["embedded_git_commit"] != str(source["commit"])[:12]
        or re.fullmatch(
            r"[0-9]{4}-[0-9]{2}-[0-9]{2}", str(source["build_date_utc"])
        )
        is None
        or COMMIT.fullmatch(str(harness["commit"])) is None
        or COMMIT.fullmatch(str(harness["tree"])) is None
    ):
        raise BootstrapError("Linux source/controller identity is malformed")
    for relative, digest_key, size_key in (
        ("input/source.tar", "archive_sha256", "archive_size_bytes"),
        ("input/source.bundle", "bundle_sha256", "bundle_size_bytes"),
        ("input/controller.tar", "archive_sha256", "archive_size_bytes"),
        ("input/controller.bundle", "bundle_sha256", "bundle_size_bytes"),
    ):
        identity = source if relative.startswith("input/source.") else harness
        path = root / relative
        if (
            not path.is_file()
            or path.is_symlink()
            or digest_file(path) != identity[digest_key]
            or path.stat().st_size != identity[size_key]
        ):
            raise BootstrapError(f"closed Linux source input changed: {relative}")
    _verify_bundled_git_identity(
        archive=root / "input/source.tar",
        bundle=root / "input/source.bundle",
        identity=source,
        where="Linux target",
    )
    _verify_bundled_git_identity(
        archive=root / "input/controller.tar",
        bundle=root / "input/controller.bundle",
        identity=harness,
        where="Linux controller",
    )

    deployment = verify_deployment(root)
    _closed(
        deployment,
        {
            "schema_version", "kind", "run_id", "source_commit",
            "preparation_sha256", "complete_sentinel",
        },
        "Linux deployment",
    )
    submission = verify_submission(root)
    _closed(
        submission,
        {
            "schema_version", "kind", "run_id", "source_commit",
            "harness_commit", "job_id", "slots", "h_rt", "mem_per_core",
            "ledger", "complete_sentinel",
        },
        "Linux submission",
    )
    job = verify_job(root)
    _closed(
        job,
        {
            "schema_version", "kind", "run_id", "job_id", "job_name",
            "platform", "source", "harness_source", "rustc", "cargo",
            "build_date_utc", "artifact", "build_log", "capture", "native_capture",
            "capture_program_sha256", "stata_version", "stata_flavor",
            "licensed_stata_load", "mock_only", "scientific_claim",
            "build_binary_audit_actual_loader", "complete_sentinel",
        },
        "Linux platform job",
    )
    if job["source"] != source or job["harness_source"] != harness:
        raise BootstrapError("Linux job source/controller differs from preparation")
    accounting_path = root / "receipts/qacct.txt"
    accounting = parse_qacct(accounting_path, submission, root)
    if (
        job["job_id"] != submission["job_id"]
        or job["job_name"] != submission["ledger"]["job_name"]
    ):
        raise BootstrapError("Linux job identity differs from submission")

    close_path = root / "receipts/linux-platform-close.json"
    close_payload = read_sealed(
        close_path, "feppml_linux_platform_bootstrap_close"
    )
    _closed(
        close_payload,
        {
            "schema_version", "kind", "run_id", "source", "harness_source",
            "build_date_utc", "job_id", "qacct", "qacct_sha256", "submission_ledger",
            "build_log", "native_capture", "platform_record",
            "platform_receipt", "platform_receipt_payload_sha256",
            "licensed_stata_load", "mock_only", "scientific_claim",
            "complete_sentinel",
        },
        "Linux platform close",
    )
    if (
        close_payload["schema_version"] != 1
        or close_payload["run_id"] != root.name
        or close_payload["source"] != source
        or close_payload["harness_source"] != harness
        or close_payload["build_date_utc"] != source["build_date_utc"]
        or close_payload["job_id"] != submission["job_id"]
        or close_payload["qacct"] != accounting
        or close_payload["qacct_sha256"] != digest_file(accounting_path)
        or close_payload["submission_ledger"] != submission["ledger"]
        or close_payload["build_log"] != job["build_log"]
        or close_payload["native_capture"] != job["native_capture"]
        or close_payload["licensed_stata_load"] is not True
        or close_payload["mock_only"] is not False
        or close_payload["scientific_claim"] is not False
        or close_payload["complete_sentinel"]
        != "FEPPML_LINUX_PLATFORM_BOOTSTRAP_COMPLETE"
    ):
        raise BootstrapError("Linux platform close differs from exact job/accounting")
    platform_record_path = _verify_file_record(
        root, close_payload["platform_record"], "Linux close platform record"
    )
    platform_receipt_path = _verify_file_record(
        root, close_payload["platform_receipt"], "Linux close platform receipt"
    )
    platform_payload = platform_receipts.verify_portable(
        receipt=platform_receipt_path,
        record=platform_record_path,
        artifact=_verify_file_record(
            root, job["artifact"], "portable Linux artifact"
        ),
        capture_path=_verify_file_record(
            root, job["capture"], "portable Linux capture"
        ),
        source_archive=root / "input/source.tar",
        capture_program=(
            root
            / "input/harness/tests/stata/installation/feppml_platform_load_capture.do"
        ),
        native_attestation=_verify_file_record(
            root,
            job["native_capture"]["attestation"],
            "portable Linux native attestation",
        ),
        native_producer=_verify_file_record(
            root,
            job["native_capture"]["producer"],
            "portable Linux native producer",
        ),
    )
    expected_native_provenance = _native_provenance_identity(
        job["native_capture"]
    )
    expected_profile = (
        "candidate"
        if source["commit"] == harness["commit"]
        and source["tree"] == harness["tree"]
        else "f0-baseline"
    )
    if (
        platform_payload["platform"] != "linux-x86_64"
        or platform_payload.get("qualification_profile") != expected_profile
        or platform_payload["source_commit"] != source["commit"]
        or platform_payload["source_tree"] != source["tree"]
        or platform_payload["source_archive_sha256"] != source["archive_sha256"]
        or platform_payload["source_archive_size_bytes"] != source["archive_size_bytes"]
        or platform_payload.get("build_date_utc") != source["build_date_utc"]
        or platform_payload["controller_commit"] != harness["commit"]
        or platform_payload["controller_tree"] != harness["tree"]
        or platform_payload["artifact_sha256"] != job["artifact"]["sha256"]
        or platform_payload["artifact_size_bytes"] != job["artifact"]["size_bytes"]
        or platform_payload["capture_output_sha256"] != job["capture"]["sha256"]
        or platform_payload["capture_program_sha256"]
        != job["capture_program_sha256"]
        or platform_payload.get("platform_provenance")
        != expected_native_provenance
        or close_payload["platform_receipt_payload_sha256"]
        != _read(platform_receipt_path, "Linux platform receipt")["seal"][
            "payload_sha256"
        ]
    ):
        raise BootstrapError("Linux shared platform receipt differs from closed job")
    success = root / "RUN.SUCCESS"
    if (
        not success.is_file()
        or success.is_symlink()
        or success.read_text(encoding="ascii")
        != f"FEPPML_LINUX_PLATFORM_BOOTSTRAP_COMPLETE|{digest_file(close_path)}\n"
    ):
        raise BootstrapError("Linux platform RUN.SUCCESS is absent or stale")
    if candidate_repo is not None:
        _verify_repo_identity(candidate_repo, source, "candidate")
    if harness_repo is not None:
        _verify_repo_identity(harness_repo, harness, "controller")
    if expected_source_commit is not None and source["commit"] != expected_source_commit:
        raise BootstrapError("Linux target source differs from the allowlisted commit")
    if (
        expected_harness_commit is not None
        and harness["commit"] != expected_harness_commit
    ):
        raise BootstrapError("Linux controller differs from the allowlisted commit")
    return {
        "record_type": "feppml-linux-platform-closed-run-v1",
        "qualification_profile": expected_profile,
        "run_id": root.name,
        "source": dict(source),
        "build_date_utc": source["build_date_utc"],
        "harness_source": dict(harness),
        "job_id": submission["job_id"],
        "qacct": accounting,
        "qacct_sha256": close_payload["qacct_sha256"],
        "artifact": dict(job["artifact"]),
        "build_log": dict(job["build_log"]),
        "capture": dict(job["capture"]),
        "native_capture": dict(job["native_capture"]),
        "submission_ledger": dict(submission["ledger"]),
        "capture_program_sha256": job["capture_program_sha256"],
        "platform_record": dict(close_payload["platform_record"]),
        "platform_receipt": dict(close_payload["platform_receipt"]),
        "platform_receipt_payload_sha256": close_payload[
            "platform_receipt_payload_sha256"
        ],
        "build_binary_audit_actual_loader": "success",
        "licensed_stata_load": True,
        "mock_only": False,
        "scientific_claim": False,
        "close_sha256": digest_file(close_path),
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare")
    prepare_parser.add_argument("--run-root", type=Path, required=True)
    prepare_parser.add_argument("--candidate-repo", type=Path, required=True)
    prepare_parser.add_argument("--candidate-commit", required=True)
    prepare_parser.add_argument("--harness-repo", type=Path, required=True)
    prepare_parser.add_argument("--harness-commit", required=True)
    for name in ("verify-input", "verify-deployment", "verify-job"):
        item = commands.add_parser(name)
        item.add_argument("--run-root", type=Path, required=True)
    deployment = commands.add_parser("record-deployment")
    deployment.add_argument("--run-root", type=Path, required=True)
    deployment.add_argument("--preparation-sha256", required=True)
    submission = commands.add_parser("record-submission")
    submission.add_argument("--run-root", type=Path, required=True)
    submission.add_argument("--job-id", required=True)
    print_submission = commands.add_parser("print-submission-command")
    print_submission.add_argument("--run-root", type=Path, required=True)
    verify_submission_parser = commands.add_parser("verify-submission")
    verify_submission_parser.add_argument("--run-root", type=Path, required=True)
    run_parser = commands.add_parser("run")
    run_parser.add_argument("--run-root", type=Path, required=True)
    close_parser = commands.add_parser("close")
    close_parser.add_argument("--run-root", type=Path, required=True)
    close_parser.add_argument("--candidate-repo", type=Path, required=True)
    close_parser.add_argument("--harness-repo", type=Path, required=True)
    closed_parser = commands.add_parser("verify-closed-run")
    closed_parser.add_argument("--run-root", type=Path, required=True)
    closed_parser.add_argument("--candidate-repo", type=Path)
    closed_parser.add_argument("--harness-repo", type=Path)
    closed_parser.add_argument("--expected-source-commit")
    closed_parser.add_argument("--expected-harness-commit")
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        if args.command == "print-submission-command":
            root = _run_root(args.run_root)
            verify_deployment(root)
            print(shlex.join(_submission_ledger(root)["argv"]))
            return 0
        if args.command == "prepare":
            prepare(args)
        elif args.command == "verify-input":
            verify_input(args.run_root)
        elif args.command == "record-deployment":
            record_deployment(args)
        elif args.command == "verify-deployment":
            verify_deployment(args.run_root)
        elif args.command == "record-submission":
            record_submission(args)
        elif args.command == "verify-submission":
            verify_submission(args.run_root)
        elif args.command == "run":
            run_job(args.run_root)
        elif args.command == "verify-job":
            verify_job(args.run_root)
        elif args.command == "close":
            close(args)
        else:
            verify_closed_run(
                args.run_root,
                candidate_repo=args.candidate_repo,
                harness_repo=args.harness_repo,
                expected_source_commit=args.expected_source_commit,
                expected_harness_commit=args.expected_harness_commit,
            )
    except (
        BootstrapError,
        platform_receipts.PlatformReceiptError,
        OSError,
        tarfile.TarError,
    ) as error:
        raise SystemExit(f"feppml Linux platform bootstrap failed closed: {error}") from error
    print(f"FEPPML_LINUX_PLATFORM_{args.command.upper().replace('-', '_')}_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
