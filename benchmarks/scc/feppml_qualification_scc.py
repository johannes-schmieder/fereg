#!/usr/bin/env python3
"""Immutable SCC lifecycle for the feppml qualification.

The lifecycle is intentionally split into prepare, setup, task, collect, and
close stages.  Preparation archives exact committed candidate and comparator
sources.  Setup completes and seals the all-cell scientific preflight before
the array is allowed to start.  Each array task captures one cell's fresh
same-node AB/BA pairs.  Collection rejects an incomplete shard inventory and
invokes the platform-independent evaluator.  Close binds SGE accounting and
publishes ``RUN.SUCCESS`` last.

This module never submits jobs and never treats mock SPI results as licensed
Stata load or scientific evidence.
"""
from __future__ import annotations

import argparse
import base64
import binascii
import copy
from datetime import datetime, timezone
import hashlib
import io
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import time
import tomllib
from typing import Any, Mapping, Sequence
from zoneinfo import ZoneInfo


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))
RUNNER_PATH = ROOT / "scripts/run_feppml_qualification.py"
CAPTURE_PROGRAM_PATH = ROOT / "scripts/feppml_qualification_capture.py"
RECIPE_COMPILER_PATH = ROOT / "scripts/generate_feppml_qualification_recipes.py"
RECIPE_CONTRACT_PATH = ROOT / "scripts/feppml_qualification_recipe_contract.py"
PLATFORM_RECEIPT_PATH = ROOT / "scripts/feppml_platform_load_receipt.py"
ORACLE_COMPILER_PATH = ROOT / "scripts/generate_feppml_oracle_fixtures.py"
CONSTRAINED_COMPILER_PATH = ROOT / "scripts/generate_feppml_constrained_recipes.py"
MATERIALIZER_PATH = ROOT / "scripts/materialize_feppml_capture_plan.py"
REFERENCE_CLOSURE_PATH = ROOT / "scripts/ppml_reference_closure.py"
SPEC_GENERATOR_PATH = ROOT / "scripts/generate_feppml_qualification_spec.py"
FORMAL_BUILD_PATH = ROOT / "scripts/formal_rust_build_closure.py"


def _module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    previous = sys.modules.get(name)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        if previous is None:
            sys.modules.pop(name, None)
        else:
            sys.modules[name] = previous
        raise
    return module


capture = _module("feppml_qualification_capture_scc", RUNNER_PATH)
capture_program = _module("feppml_qualification_capture_program_scc", CAPTURE_PROGRAM_PATH)
qualification = capture.qualification
development = capture.development
recipes = _module("feppml_qualification_recipes_scc", RECIPE_COMPILER_PATH)
recipe_contract = _module("feppml_qualification_recipe_contract_scc", RECIPE_CONTRACT_PATH)
platform_receipts = _module("feppml_platform_load_receipt_scc", PLATFORM_RECEIPT_PATH)
oracle_recipes = _module("feppml_oracle_fixtures_scc", ORACLE_COMPILER_PATH)
constrained_recipes = _module("feppml_constrained_recipes_scc", CONSTRAINED_COMPILER_PATH)
materializer = _module("feppml_capture_materializer_scc", MATERIALIZER_PATH)
reference_closure = _module("feppml_reference_closure_scc", REFERENCE_CLOSURE_PATH)
spec_generator = _module("feppml_qualification_spec_generator_scc", SPEC_GENERATOR_PATH)
formal_build = _module("formal_rust_build_closure_scc", FORMAL_BUILD_PATH)

PROJECT_ROOT = Path("/projectnb/welfgr/fereg")
RUN_ID = re.compile(r"^[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
JOB_ID = re.compile(r"^[1-9][0-9]*$")
REFERENCE_NAMES = ("ppmlhdfe", "reghdfe", "ftools", "require")
EXPECTED_RUSTC = "rustc 1.85.1 (4eb161250 2025-03-15)"
EXPECTED_CARGO = "cargo 1.85.1 (d73d2caf9 2024-12-31)"
EXPECTED_PYTHON = "Python 3.12.4"
QACCT_TIMEOUT_SECONDS = 30.0
EXPECTED_CONTROLLER_PYTHON = {
    "path": "/share/pkg.8/python3/3.12.4/install/bin/python3",
    "resolved_path": "/share/pkg.8/python3/3.12.4/install/bin/python3.12",
    "sha256": "f62d663ae97a6edcd3070f1437627f3cb34829a87826abd6969d1f795d44f0c4",
    "size_bytes": 20_944,
    "version": EXPECTED_PYTHON,
}
EXPECTED_CARGO_HOME = Path("/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo")
EXPECTED_RUSTUP_HOME = Path("/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup")
EXPECTED_RUSTUP_TOOLCHAIN = "1.85.1-x86_64-unknown-linux-gnu"
LOCAL_FORMAL_CARGO = Path(
    "/Users/johannes/.rustup/toolchains/1.85.1-aarch64-apple-darwin/bin/cargo"
)
LOCAL_CARGO_SOURCE_HOME = Path("/Users/johannes/.cargo")
LOCAL_REGISTRY_ARCHIVE_ROOT = Path(
    "/Users/johannes/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f"
)
LOCAL_GIT_DEPENDENCIES = {
    "git+https://github.com/johannes-schmieder/CMG?rev=ce126358a82cc1a461cfe7b9f9318b4c3c1c1316": Path(
        "/Users/johannes/.cargo/git/checkouts/cmg-2012f6025e587b49/ce12635"
    ),
    "git+https://github.com/py-econometrics/within?rev=b7779cbab7a3116be56aae4389fde1f6e6a99a9f": Path(
        "/Users/johannes/.cargo/git/checkouts/within-937ec8f21002c884/b7779cb"
    ),
}
EXPECTED_SOURCE_GIT = {
    "path": "/usr/bin/git",
    "resolved_path": "/usr/bin/git",
    "sha256": "b8763cf250e607a778bb4603cecb5b90338814d0a3dfcba0d57b1de242f610e9",
    "size_bytes": 118_640,
    "version": "git version 2.50.1 (Apple Git-155)",
}
BUILD_TOOL_ROLES = (
    "bash", "cargo", "rustc", "python3", "cc", "linker", "ar", "ranlib",
    "sha256sum",
    "readelf", "nm", "ldd", "uname", "dirname", "basename", "mkdir", "rm",
)
HARNESS_PATHS = (
    "benchmarks/scc/feppml_qualification_scc.py",
    "benchmarks/scc/feppml_qualification_deploy.sh",
    "benchmarks/scc/feppml_qualification_setup_job.sh",
    "benchmarks/scc/feppml_qualification_job.sh",
    "benchmarks/scc/feppml_qualification_collector_job.sh",
    "benchmarks/scc/feppml_qualification_submit.sh",
    "benchmarks/scc/feppml_qualification_close.sh",
    "scripts/feppml_qualification.py",
    "scripts/feppml_qualification_capture.py",
    "scripts/generate_feppml_qualification_spec.py",
    "scripts/generate_feppml_qualification_recipes.py",
    "scripts/feppml_qualification_recipe_contract.py",
    "scripts/feppml_separation_recipe_contract.py",
    "scripts/generate_feppml_oracle_fixtures.py",
    "scripts/generate_feppml_constrained_recipes.py",
    "scripts/materialize_feppml_capture_plan.py",
    "scripts/normalize_feppml_stata_raw.py",
    "scripts/parse_feppml_stata_protocol.py",
    "scripts/feppml_explicit_dummy_oracle.py",
    "scripts/feppml_platform_load_receipt.py",
    "scripts/prepare_windows_feppml_target.py",
    "scripts/run_feppml_qualification.py",
    "scripts/ppml_memory_performance.py",
    "scripts/ppml_legacy_boundary.py",
    "scripts/ppml_reference_closure.py",
    "scripts/formal_rust_build_closure.py",
    "scripts/ols_reference.py",
    "benchmarks/manifests/feppml_qualification_v1.json",
    "benchmarks/manifests/ppml_memory_performance_v1.json",
    "docs/adr/0016-ppml-scientific-contract.md",
    "docs/adr/0017-certified-ppml-separation-ledger.md",
    "docs/adr/0018-ppml-memory-routing-performance.md",
    "docs/adr/0019-additive-ppml-transaction-and-fereg-regression-boundary.md",
    "references.lock.toml",
    "tests/python/ppml_explicit_dummy_oracle.py",
    "tests/python/ppml_multiway_cluster_oracle.py",
    "tests/stata/installation/feppml_platform_load_capture.do",
)


class SccError(RuntimeError):
    """An SCC run input, shard, accounting record, or sentinel failed closed."""


CONTROLLER_ENVIRONMENT_KEYS = {
    "submission": {
        "HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ", "RUN_ROOT",
        "USER", "LOGNAME", "SGE_ROOT", "SGE_CELL",
        "FEPPML_QSUB_EXECUTABLE", "FEPPML_QSTAT_EXECUTABLE",
        "FEPPML_QACCT_EXECUTABLE", "FEPPML_QRLS_EXECUTABLE",
        "FEPPML_CONTROLLER_PYTHON",
    },
    "deployment": {
        "HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ", "RUN_ROOT",
        "FEPPML_CONTROLLER_PYTHON",
    },
    "setup": {
        "HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ", "RUN_ROOT",
        "JOB_ID", "NSLOTS", "FEPPML_STATA_EXECUTABLE",
        "FEPPML_CONTROLLER_PYTHON",
    },
    "task": {
        "HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ", "RUN_ROOT",
        "JOB_ID", "NSLOTS", "SGE_TASK_ID", "FEPPML_CONTROLLER_PYTHON",
    },
    "collector": {
        "HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ", "RUN_ROOT",
        "JOB_ID", "NSLOTS", "FEPPML_CONTROLLER_PYTHON",
    },
    "close": {
        "HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ", "RUN_ROOT",
        "FEPPML_CONTROLLER_PYTHON",
    },
}
SCHEDULER_CLIENT_ENV = {
    "qsub": "FEPPML_QSUB_EXECUTABLE",
    "qstat": "FEPPML_QSTAT_EXECUTABLE",
    "qacct": "FEPPML_QACCT_EXECUTABLE",
    "qrls": "FEPPML_QRLS_EXECUTABLE",
}
SCHEDULER_CLIENT_CONTRACT = {
    "qsub": {
        "path": "/usr/local/bin/qsub",
        "resolved_path": "/usr/local/ogs-ge2011.11.p1/sge_root/bin/linux-x64/qsub",
        "sha256": "fdd46c981d8f3107a44e6985ca9e689e4969b7c6281c8e56a78420c084fd7a6e",
        "size_bytes": 1_732_888,
        "version": "OGS/GE 2011.11p1",
    },
    "qstat": {
        "path": "/usr/local/bin/qstat",
        "resolved_path": "/usr/local/ogs-ge2011.11.p1/sge_root/bin/linux-x64/qstat",
        "sha256": "0fb1c3334e9885f7df3873986c71465ef8b5528674a938441dbaefb6214c9941",
        "size_bytes": 2_043_136,
        "version": "OGS/GE 2011.11p1",
    },
    "qacct": {
        "path": "/usr/local/bin/qacct",
        "resolved_path": "/usr/local/ogs-ge2011.11.p1/sge_root/bin/linux-x64/qacct",
        "sha256": "aa8575f51ad1f07673ef862d6dfbe06381ebc53bdb88bb3a0256573ededc37e0",
        "size_bytes": 1_753_600,
        "version": "OGS/GE 2011.11p1",
    },
    "qrls": {
        "path": "/usr/local/bin/qrls",
        "resolved_path": "/usr/local/ogs-ge2011.11.p1/sge_root/bin/linux-x64/qalter",
        "sha256": "1f2b544bcb31b82d88f453a47f9570ad34ac068871e3fc9164c1dfb8fe8faf12",
        "size_bytes": 1_624_792,
        "version": "OGS/GE 2011.11p1",
    },
}


def canonical(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False
    ).encode("ascii") + b"\n"


def digest_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def digest_file(path: Path) -> str:
    return qualification.sha256_file(path)


def _controller_identity(role: str, run_root: Path) -> dict[str, Any]:
    if role not in CONTROLLER_ENVIRONMENT_KEYS:
        raise SccError("unknown qualification controller role")
    environment = dict(os.environ)
    if set(environment) != CONTROLLER_ENVIRONMENT_KEYS[role]:
        raise SccError(
            f"{role} controller environment is not the exact closed inventory"
        )
    executable = Path(environment["FEPPML_CONTROLLER_PYTHON"])
    resolved = executable.resolve()
    flags = {
        "isolated": sys.flags.isolated,
        "ignore_environment": sys.flags.ignore_environment,
        "no_site": sys.flags.no_site,
        "dont_write_bytecode": sys.flags.dont_write_bytecode,
        "safe_path": sys.flags.safe_path,
    }
    if (
        environment["RUN_ROOT"] != str(run_root)
        or not executable.is_absolute()
        or str(executable) != EXPECTED_CONTROLLER_PYTHON["path"]
        or resolved != Path(sys.executable).resolve()
        or str(resolved) != EXPECTED_CONTROLLER_PYTHON["resolved_path"]
        or not resolved.is_file()
        or digest_file(resolved) != EXPECTED_CONTROLLER_PYTHON["sha256"]
        or resolved.stat().st_size != EXPECTED_CONTROLLER_PYTHON["size_bytes"]
        or flags
            != {
                "isolated": 1,
                "ignore_environment": 1,
                "no_site": 1,
                "dont_write_bytecode": 1,
                "safe_path": True,
            }
        or environment["LANG"] != "C"
        or environment["LC_ALL"] != "C"
        or environment["TZ"] != "UTC"
    ):
        raise SccError(f"{role} controller Python isolation is invalid")
    version = subprocess.run(
        [str(executable), "-I", "-B", "-S", "--version"],
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        check=False,
    )
    if version.returncode != 0 or version.stdout.strip() != EXPECTED_PYTHON:
        raise SccError("controller Python version differs from the formal runtime")
    record = {
        "schema_version": 1,
        "record_type": "feppml-isolated-controller-v1",
        "role": role,
        "executable": {
            "path": str(executable),
            "resolved_path": str(resolved),
            "sha256": digest_file(resolved),
            "size_bytes": resolved.stat().st_size,
            "version": version.stdout.strip(),
        },
        "flags": flags,
        "environment": environment,
        "environment_sha256": digest_bytes(canonical(environment)),
        "complete_sentinel": "FEPPML_ISOLATED_CONTROLLER_COMPLETE",
    }
    return dict(
        _verify_controller_identity(
            run_root, record, role, portable=False
        )
    )


def _verify_controller_identity(
    run_root: Path,
    record: Any,
    role: str,
    *,
    portable: bool,
) -> Mapping[str, Any]:
    value = record if isinstance(record, Mapping) else {}
    if set(value) != {
        "schema_version", "record_type", "role", "executable", "flags",
        "environment", "environment_sha256", "complete_sentinel",
    } or (
        value.get("schema_version") != 1
        or value.get("record_type") != "feppml-isolated-controller-v1"
        or value.get("role") != role
        or value.get("flags")
            != {
                "isolated": 1,
                "ignore_environment": 1,
                "no_site": 1,
                "dont_write_bytecode": 1,
                "safe_path": True,
            }
        or value.get("complete_sentinel")
            != "FEPPML_ISOLATED_CONTROLLER_COMPLETE"
    ):
        raise SccError(f"{role} controller receipt is invalid")
    environment = value.get("environment")
    executable = value.get("executable")
    python_directory = str(Path(EXPECTED_CONTROLLER_PYTHON["path"]).parent)
    if (
        not isinstance(environment, Mapping)
        or set(environment) != CONTROLLER_ENVIRONMENT_KEYS[role]
        or value.get("environment_sha256")
            != digest_bytes(canonical(environment))
        or not isinstance(executable, Mapping)
        or set(executable) != {
            "path", "resolved_path", "sha256", "size_bytes", "version"
        }
        or executable.get("version") != EXPECTED_PYTHON
        or dict(executable) != EXPECTED_CONTROLLER_PYTHON
    ):
        raise SccError(f"{role} controller closure is malformed")
    recorded_root = Path(str(environment["RUN_ROOT"]))
    expected_controller = (
        recorded_root / "work/submission-controller"
        if role == "submission"
        else recorded_root / "work/controller" / (
        "setup" if role == "setup" else (
            f"task-{environment.get('SGE_TASK_ID')}"
            if role == "task"
            else role
        ))
    )
    if (
        recorded_root.name != run_root.name
        or environment["HOME"] != str(expected_controller / "home")
        or environment["TMPDIR"] != str(expected_controller / "tmp")
        or environment["LANG"] != "C"
        or environment["LC_ALL"] != "C"
        or environment["TZ"] != "UTC"
        or environment["FEPPML_CONTROLLER_PYTHON"] != executable["path"]
        or (
            role == "submission"
            and (
                environment["PATH"]
                    != f"{python_directory}:/usr/local/bin:/usr/bin:/bin"
                or environment["USER"] != "johannes"
                or environment["LOGNAME"] != "johannes"
                or environment["SGE_ROOT"] != "/usr/local/sge/sge_root"
                or environment["SGE_CELL"] != "default"
                or any(
                    environment[SCHEDULER_CLIENT_ENV[name]]
                        != SCHEDULER_CLIENT_CONTRACT[name]["path"]
                    for name in SCHEDULER_CLIENT_ENV
                )
            )
        )
        or (
            role in {"deployment", "collector", "close"}
            and environment["PATH"]
                != f"{python_directory}:/usr/bin:/bin"
        )
        or (
            role in {"setup", "task"}
            and (
                len(environment["PATH"].split(":")) != 4
                or environment["PATH"].split(":")[0] != python_directory
                or not Path(environment["PATH"].split(":")[1]).is_absolute()
                or environment["PATH"].split(":")[2:] != ["/usr/bin", "/bin"]
            )
        )
        or (
            role in {"setup", "task", "collector"}
            and not str(environment["JOB_ID"]).isdigit()
        )
        or (
            role in {"setup", "task", "collector"}
            and environment["NSLOTS"]
                != ("1" if role == "collector" else "16")
        )
        or (
            role == "setup"
            and (
                not Path(environment["FEPPML_STATA_EXECUTABLE"]).is_absolute()
                or Path(environment["FEPPML_STATA_EXECUTABLE"]).name.lower()
                    != "stata-mp"
                or str(Path(environment["FEPPML_STATA_EXECUTABLE"]).parent)
                    != environment["PATH"].split(":")[1]
            )
        )
    ):
        raise SccError(f"{role} controller environment is misbound")
    if role == "task" and not str(environment["SGE_TASK_ID"]).isdigit():
        raise SccError("task controller has an invalid array task ID")
    if not portable:
        path = Path(str(executable["path"]))
        resolved = path.resolve()
        if (
            not path.is_file()
            or str(resolved) != executable["resolved_path"]
            or digest_file(resolved) != executable["sha256"]
            or resolved.stat().st_size != executable["size_bytes"]
        ):
            raise SccError(f"{role} controller executable bytes changed")
    return value


def _bound_file_inventory(run_root: Path, roots: Sequence[Path]) -> dict[str, dict[str, Any]]:
    files: set[Path] = set()
    for raw_root in roots:
        root = raw_root.resolve()
        if run_root.resolve() not in root.parents and root != run_root.resolve():
            raise SccError("raw evidence root escapes the immutable run")
        if root.is_symlink() or not root.exists():
            raise SccError("raw evidence root is absent or unsafe")
        if root.is_file():
            files.add(root)
            continue
        for path in root.rglob("*"):
            if path.is_symlink() or (not path.is_file() and not path.is_dir()):
                raise SccError("raw evidence contains an unsafe entry")
            if path.is_file():
                files.add(path)
    return {
        path.relative_to(run_root).as_posix(): {
            "sha256": digest_file(path),
            "size_bytes": path.stat().st_size,
        }
        for path in sorted(files, key=lambda value: value.relative_to(run_root).as_posix())
    }


def _verify_bound_file_inventory(
    run_root: Path,
    relative_roots: Sequence[Any],
    expected: Any,
) -> dict[str, dict[str, Any]]:
    if (
        not isinstance(relative_roots, list)
        or not relative_roots
        or not all(isinstance(value, str) and value for value in relative_roots)
        or not isinstance(expected, Mapping)
    ):
        raise SccError("raw evidence inventory schema is invalid")
    roots = []
    for value in relative_roots:
        relative = Path(value)
        if relative.is_absolute() or ".." in relative.parts:
            raise SccError("raw evidence root is unsafe")
        roots.append(run_root / relative)
    observed = _bound_file_inventory(run_root, roots)
    if observed != expected:
        raise SccError("raw evidence inventory contains missing, changed, or extra files")
    return observed


def _read(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SccError(f"cannot read {where}: {error}") from error
    if not isinstance(value, Mapping):
        raise SccError(f"{where} must be an object")
    return value


def _write(path: Path, data: bytes, mode: int = 0o444) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError as error:
        raise SccError(f"refusing to overwrite immutable output: {path}") from error
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


def write_json(path: Path, value: Any) -> None:
    _write(path, json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True).encode("ascii") + b"\n")


def write_sealed(path: Path, payload: Mapping[str, Any]) -> None:
    _write(path, json.dumps(qualification.seal_payload(payload), sort_keys=True, indent=2).encode("ascii") + b"\n")


def read_sealed(path: Path, kind: str) -> Mapping[str, Any]:
    document = _read(path, "sealed receipt")
    if set(document) != {"payload", "seal"}:
        raise SccError("sealed receipt envelope is not closed")
    payload = document["payload"]
    if not isinstance(payload, Mapping) or document["seal"] != qualification.seal_payload(payload)["seal"]:
        raise SccError("sealed receipt digest mismatch")
    if payload.get("kind") != kind:
        raise SccError(f"sealed receipt has wrong kind: {path.name}")
    return payload


def _run(command: Sequence[str], *, cwd: Path | None = None) -> bytes:
    completed = subprocess.run(
        list(command), cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False
    )
    if completed.returncode:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise SccError(f"command failed rc={completed.returncode}: {detail}")
    return completed.stdout


def _commit(repo: Path, commit: str) -> str:
    if COMMIT.fullmatch(commit) is None:
        raise SccError("source commit must be a full lowercase SHA")
    observed = _git(repo, "rev-parse", f"{commit}^{{commit}}")
    if observed.decode("ascii").strip() != commit:
        raise SccError("source commit did not resolve exactly")
    return commit


def _git(repo: Path, *arguments: str) -> bytes:
    git = Path(EXPECTED_SOURCE_GIT["path"])
    resolved = git.resolve()
    if (
        str(resolved) != EXPECTED_SOURCE_GIT["resolved_path"]
        or not resolved.is_file()
        or digest_file(resolved) != EXPECTED_SOURCE_GIT["sha256"]
        or resolved.stat().st_size != EXPECTED_SOURCE_GIT["size_bytes"]
    ):
        raise SccError("formal source Git executable changed")
    try:
        formal_build._reject_git_rewrite_mechanisms(repo, git)
        return formal_build._git_run(
            git, repo, list(arguments), "formal qualification source Git"
        )
    except formal_build.ClosureError as error:
        raise SccError(f"formal qualification source Git failed: {error}") from error


def _source_git_record(repo: Path) -> dict[str, Any]:
    git = Path(EXPECTED_SOURCE_GIT["path"])
    version = _git(repo, "--version").decode("utf-8", errors="strict").strip()
    if version != EXPECTED_SOURCE_GIT["version"]:
        raise SccError("formal source Git version changed")
    return {
        **EXPECTED_SOURCE_GIT,
        "environment": formal_build._git_environment(git.resolve()),
        "replace_objects_disabled": True,
        "rewrite_mechanisms_absent": True,
    }


def _archive(repo: Path, commit: str) -> bytes:
    return _git(repo, "archive", "--format=tar", commit)


def _local_dependency_cache_inputs() -> Any:
    return formal_build.SourceCacheInputs(
        cargo_source_home=LOCAL_CARGO_SOURCE_HOME,
        registry_archive_roots=(LOCAL_REGISTRY_ARCHIVE_ROOT,),
        git_repositories=LOCAL_GIT_DEPENDENCIES,
        git_executable=Path(EXPECTED_SOURCE_GIT["path"]),
    )


def _prepare_dependency_closure(
    run_root: Path, candidate_archive: bytes
) -> Mapping[str, Any]:
    source = run_root / "input/build-source"
    archive_path = run_root / "input/archives/candidate.tar"
    if archive_path.read_bytes() != candidate_archive:
        raise SccError("formal dependency source archive changed before extraction")
    _extract_archive(archive_path, source)
    try:
        _commit_id, _tree_id, archive_files = qualification._archive_tree_and_files(
            candidate_archive
        )
        qualification._verify_extracted_archive_inventory(source, archive_files)
    except qualification.QualificationError as error:
        raise SccError(
            f"formal dependency source extraction is not exact: {error}"
        ) from error
    try:
        receipt = formal_build.materialize_dependency_closure(
            run_root,
            source,
            run_root / "input/formal-build",
            _local_dependency_cache_inputs(),
            cargo=LOCAL_FORMAL_CARGO,
        )
        formal_build.verify_dependency_receipt(
            run_root,
            receipt,
            portable=False,
            cache_inputs=_local_dependency_cache_inputs(),
        )
    except formal_build.ClosureError as error:
        raise SccError(f"formal dependency closure failed: {error}") from error
    return receipt


def _source_record(repo: Path, commit: str, archive: bytes) -> dict[str, Any]:
    return {
        "commit": commit,
        "tree": _git(repo, "rev-parse", f"{commit}^{{tree}}").decode("ascii").strip(),
        "archive_sha256": digest_bytes(archive),
        "archive_size_bytes": len(archive),
    }


def _candidate_record(repo: Path, commit: str, archive: bytes) -> dict[str, Any]:
    record = _source_record(repo, commit, archive)
    commit_epoch_raw = _git(repo, "show", "-s", "--format=%ct", commit).decode("ascii").strip()
    if not commit_epoch_raw.isdigit():
        raise SccError("candidate commit epoch is not a deterministic build date")
    build_date = datetime.fromtimestamp(
        int(commit_epoch_raw), tz=timezone.utc
    ).strftime("%Y-%m-%d")
    record.update(
        {
            "cargo_lock_sha256": digest_bytes(_git(repo, "show", f"{commit}:Cargo.lock")),
            "references_lock_sha256": digest_bytes(
                _git(repo, "show", f"{commit}:references.lock.toml")
            ),
            "build_date_utc": build_date,
        }
    )
    return record


def _reference_entries(lock_bytes: bytes) -> Mapping[str, Mapping[str, Any]]:
    try:
        lock = tomllib.loads(lock_bytes.decode("utf-8"))
        return reference_closure.locked_entries(lock)
    except (UnicodeError, tomllib.TOMLDecodeError, reference_closure.ClosureError) as error:
        raise SccError(f"committed comparator lock is invalid: {error}") from error


def _reference_archive_inventory(
    archive: bytes, name: str, entry: Mapping[str, Any]
) -> dict[str, dict[str, Any]]:
    """Derive the exact runtime src inventory from one lock-bound archive."""

    expected_root = str(entry.get("source_archive_root", "")) if name == "ppmlhdfe" else ""
    scope = str(entry["source_archive_scope"])
    files: dict[str, dict[str, Any]] = {}
    try:
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:*") as source:
            members = source.getmembers()
            if not members:
                raise SccError(f"{name} reference archive is empty")
            for member in members:
                raw = Path(member.name.rstrip("/"))
                if (
                    raw.is_absolute() or ".." in raw.parts or member.issym()
                    or member.islnk() or member.isdev()
                ):
                    raise SccError(f"{name} reference archive contains an unsafe member")
                parts = raw.parts
                if expected_root:
                    if not parts or parts[0] != expected_root:
                        raise SccError(f"{name} reference archive root is not locked")
                    parts = parts[1:]
                if not parts or parts[0] != scope:
                    continue
                relative = Path(*parts).as_posix()
                if not member.isfile():
                    continue
                opened = source.extractfile(member)
                if opened is None:
                    raise SccError(f"{name} reference archive member is unreadable")
                data = opened.read()
                if relative in files:
                    raise SccError(f"{name} reference archive has duplicate runtime paths")
                files[relative] = {
                    "sha256": digest_bytes(data),
                    "size_bytes": len(data),
                    "executable": bool(member.mode & 0o111),
                }
    except tarfile.TarError as error:
        raise SccError(f"{name} reference archive is invalid: {error}") from error
    required = entry["required_capture_sha256"]
    if not files or any(
        relative not in files or files[relative]["sha256"] != expected
        for relative, expected in required.items()
    ):
        raise SccError(f"{name} reference archive omits or changes locked capture files")
    return {relative: files[relative] for relative in sorted(files)}


def _locked_reference_record(
    name: str,
    entry: Mapping[str, Any],
    archive: bytes,
) -> dict[str, Any]:
    if (
        digest_bytes(archive) != entry["source_archive_sha256"]
        or len(archive) != entry["source_archive_size_bytes"]
    ):
        raise SccError(f"{name} source archive differs from references.lock.toml")
    return {
        "commit": entry["commit"],
        "tree": entry["git_tree"],
        "archive_sha256": digest_bytes(archive),
        "archive_size_bytes": len(archive),
        "archive_format": entry["source_archive_format"],
        "archive_scope": entry["source_archive_scope"],
        "runtime_files": _reference_archive_inventory(archive, name, entry),
    }


def _validate_locked_reference_archives(
    run_root: Path, preparation: Mapping[str, Any]
) -> Mapping[str, Mapping[str, Any]]:
    lock_path = run_root / "input/harness/references.lock.toml"
    lock_bytes = lock_path.read_bytes()
    if digest_bytes(lock_bytes) != preparation["candidate"]["references_lock_sha256"]:
        raise SccError("prepared comparator lock differs from the candidate")
    entries = _reference_entries(lock_bytes)
    for name in REFERENCE_NAMES:
        archive_path = run_root / "input/archives" / f"{name}.tar"
        if name == "ppmlhdfe":
            archive_path = archive_path.with_suffix(".tar.gz")
        if archive_path.is_symlink() or not archive_path.is_file():
            raise SccError(f"prepared comparator archive is absent: {name}")
        observed = _locked_reference_record(name, entries[name], archive_path.read_bytes())
        if preparation["references"].get(name) != observed:
            raise SccError(f"prepared comparator record is stale or fabricated: {name}")
    return entries


def _run_root(path: Path) -> Path:
    path = path.resolve()
    if RUN_ID.fullmatch(path.name) is None:
        raise SccError("run root basename must be a sortable UTC run ID")
    return path


def _stage_platform_prerequisites(
    input_root: Path, records: Sequence[Mapping[str, Any]]
) -> dict[str, dict[str, str]]:
    """Copy exact cross-platform load evidence into the sealed run input."""

    staged: dict[str, dict[str, str]] = {}
    for record in records:
        platform = str(record["platform"])
        try:
            payload = platform_receipts.verify_evidence(record)
        except platform_receipts.PlatformReceiptError as error:
            raise SccError(
                f"platform prerequisite failed authoritative verification: {platform}: {error}"
            ) from error
        if payload.get("platform") != platform or payload.get(
            "qualification_profile"
        ) != "candidate":
            raise SccError(
                f"platform prerequisite is not candidate qualification evidence: {platform}"
            )
        root = input_root / "platforms" / platform
        source_artifact = Path(record["artifact"]["path"]).resolve()
        source_program = Path(record["capture_program"]["path"]).resolve()
        paths = {
            "artifact": root / source_artifact.name,
            "capture": root / "capture.json",
            "source_archive": root / "source-archive.tar",
            "capture_program": root / source_program.name,
            "receipt": root / "receipt.json",
        }
        for field, destination in paths.items():
            source = Path(record[field]["path"]).resolve()
            _write(destination, source.read_bytes())
        provenance = record.get("platform_provenance")
        if not isinstance(provenance, Mapping):
            raise SccError(f"platform provenance is absent: {platform}")
        if platform in {"macos-universal", "linux-x86_64"}:
            attestation = root / "native-attestation.json"
            native_paths = {
                "native_producer": root / "native-producer.py",
                "native_attestation": attestation,
                "native_status": Path(f"{attestation}.status"),
                "native_stata_log": Path(f"{attestation}.stata.log"),
                "native_wrapper": Path(f"{attestation}.wrapper.do"),
                "native_invocation": Path(f"{attestation}.invocation.json"),
            }
            provenance_fields = {
                "native_producer": "producer",
                "native_attestation": "attestation",
                "native_status": "status",
                "native_stata_log": "stata_log",
                "native_wrapper": "wrapper",
                "native_invocation": "invocation",
            }
            for destination_field, source_field in provenance_fields.items():
                source_record = provenance.get(source_field)
                if not isinstance(source_record, Mapping):
                    raise SccError(
                        f"native platform prerequisite omits {source_field}: {platform}"
                    )
                _write(
                    native_paths[destination_field],
                    Path(str(source_record["path"])).resolve().read_bytes(),
                )
            paths.update(native_paths)
        elif platform == "windows-x86_64":
            windows_paths = {
                "windows_controller_receipt": root / "controller-receipt.json",
                "windows_controller_output": root / "controller-output.json",
                "windows_controller_archive": root / "controller-archive.tar",
                "windows_results_archive": root / "results-archive.zip",
                "windows_source_manifest": root / "source-manifest.json",
            }
            provenance_fields = {
                "windows_controller_receipt": "controller_receipt",
                "windows_controller_output": "controller_output",
                "windows_controller_archive": "controller_archive",
                "windows_results_archive": "results_archive",
                "windows_source_manifest": "source_manifest",
            }
            for destination_field, source_field in provenance_fields.items():
                source_record = provenance.get(source_field)
                if not isinstance(source_record, Mapping):
                    raise SccError(
                        f"Windows platform prerequisite omits {source_field}"
                    )
                _write(
                    windows_paths[destination_field],
                    Path(str(source_record["path"])).resolve().read_bytes(),
                )
            paths.update(windows_paths)
        else:
            raise SccError(f"unsupported platform prerequisite: {platform}")
        staged[platform] = {
            field: path.relative_to(input_root.parent).as_posix()
            for field, path in paths.items()
        }
    return staged


def _late_platform_records(
    run_root: Path,
    preparation: Mapping[str, Any],
    manifest: Mapping[str, Any],
    candidate: Mapping[str, Any],
) -> list[dict[str, Any]]:
    """Rebind copied platform evidence to this run's absolute paths."""

    output = []
    record_root = run_root / "work/capture/platforms"
    for platform in manifest["platform_gates"]["required_artifacts"]:
        paths = preparation["platform_inputs"][platform]
        record_path = record_root / f"{platform}.json"
        arguments: dict[str, Any] = {
            "platform": platform,
            "artifact": run_root / paths["artifact"],
            "capture_path": run_root / paths["capture"],
            "source_archive": run_root / paths["source_archive"],
            "capture_program": run_root / paths["capture_program"],
            "receipt": run_root / paths["receipt"],
            "record": record_path,
        }
        if platform in {"macos-universal", "linux-x86_64"}:
            arguments.update(
                {
                    "native_attestation": run_root / paths["native_attestation"],
                    "native_producer": run_root / paths["native_producer"],
                }
            )
        else:
            for field in (
                "windows_controller_receipt",
                "windows_controller_output",
                "windows_controller_archive",
                "windows_results_archive",
                "windows_source_manifest",
            ):
                arguments[field] = run_root / paths[field]
        rebound = platform_receipts.rebind(**arguments)
        try:
            payload = platform_receipts.verify_evidence(rebound)
        except platform_receipts.PlatformReceiptError as error:
            raise SccError(
                f"rebound platform evidence failed authoritative verification: {platform}: {error}"
            ) from error
        if payload.get("platform") != platform:
            raise SccError("rebound platform verifier returned another platform")
        output.append(rebound)
    qualification._validate_platforms(
        manifest,
        output,
        candidate,
    )
    return output


def _portable_platform_verifier(
    run_root: Path, preparation: Mapping[str, Any]
):
    """Return a read-only verifier for relocated platform prerequisite bytes."""

    def verify(record: Mapping[str, Any]) -> Mapping[str, Any]:
        platform = record.get("platform")
        if not isinstance(platform, str) or platform not in preparation["platform_inputs"]:
            raise SccError("portable platform evidence names an unknown platform")
        record_path = run_root / "work/capture/platforms" / f"{platform}.json"
        if _read(record_path, f"portable {platform} record") != record:
            raise SccError(
                f"portable {platform} record differs from the spec-bound evidence"
            )
        relative = preparation["platform_inputs"][platform]
        arguments: dict[str, Any] = {
            "receipt": run_root / relative["receipt"],
            "record": record_path,
            "artifact": run_root / relative["artifact"],
            "capture_path": run_root / relative["capture"],
            "source_archive": run_root / relative["source_archive"],
            "capture_program": run_root / relative["capture_program"],
        }
        if platform in {"macos-universal", "linux-x86_64"}:
            arguments.update(
                {
                    "native_attestation": run_root / relative["native_attestation"],
                    "native_producer": run_root / relative["native_producer"],
                }
            )
        elif platform == "windows-x86_64":
            for field in (
                "windows_controller_receipt", "windows_controller_output",
                "windows_controller_archive", "windows_results_archive",
                "windows_source_manifest",
            ):
                arguments[field] = run_root / relative[field]
        else:
            raise SccError(f"portable platform is unsupported: {platform}")
        try:
            return platform_receipts.verify_portable(**arguments)
        except platform_receipts.PlatformReceiptError as error:
            raise SccError(
                f"portable platform evidence failed: {platform}: {error}"
            ) from error

    return verify


def _validate_capture_template(
    document: Mapping[str, Any], manifest: Mapping[str, Any], manifest_sha: str
) -> Mapping[str, Any]:
    if document.get("qualification_manifest") != {
        "id": manifest["manifest_id"], "sha256": manifest_sha
    }:
        raise SccError("capture spec is bound to a different qualification manifest")
    if (
        set(document)
        != {
            "schema_version", "spec_type", "qualification_manifest",
            "evidence_header", "environment", "cell_ids",
            "complete_sentinel",
        }
        or document.get("schema_version") != 1
        or document.get("spec_type") != spec_generator.TEMPLATE_TYPE
        or document.get("complete_sentinel") != spec_generator.TEMPLATE_SENTINEL
        or document.get("cell_ids")
            != manifest["matrix_contract"]["expected_cell_ids"]
    ):
        raise SccError("capture spec is not the closed non-executable SCC template")
    template_environment = document.get("environment")
    if (
        not isinstance(template_environment, Mapping)
        or set(template_environment) != {"stata_version", "rustc_version"}
        or any(
            not isinstance(template_environment[field], str)
            or not template_environment[field].strip()
            for field in ("stata_version", "rustc_version")
        )
    ):
        raise SccError("capture template environment is not closed")
    return document


def prepare(args: argparse.Namespace) -> None:
    run_root = _run_root(args.run_root)
    if run_root.exists():
        raise SccError("run root already exists; retries require a new run ID")
    candidate_repo = args.candidate_repo.resolve()
    source_git = _source_git_record(candidate_repo)
    candidate_commit = _commit(candidate_repo, args.candidate_commit)

    manifest_relative = args.manifest.resolve().relative_to(candidate_repo).as_posix()
    manifest_bytes = _git(candidate_repo, "show", f"{candidate_commit}:{manifest_relative}")
    if args.manifest.read_bytes() != manifest_bytes:
        raise SccError("qualification manifest differs from exact candidate source")
    manifest = qualification.load_manifest(args.manifest)
    source_audit = qualification.audit_source(
        candidate_repo,
        candidate_commit,
        manifest,
        git_reader=lambda root, arguments: _git(root, *arguments),
    )
    if not source_audit["forbidden_symbols_absent"]:
        raise SccError("candidate live PPML route still reaches a forbidden legacy fitter")

    candidate_archive = _archive(candidate_repo, candidate_commit)
    lock_bytes = _git(candidate_repo, "show", f"{candidate_commit}:references.lock.toml")
    reference_entries = _reference_entries(lock_bytes)
    references: dict[str, Any] = {}
    archives: dict[str, bytes] = {"candidate": candidate_archive}
    ppml_archive_path = args.ppmlhdfe_archive.resolve()
    if ppml_archive_path.is_symlink() or not ppml_archive_path.is_file():
        raise SccError("locked ppmlhdfe source archive is absent")
    ppml_archive = ppml_archive_path.read_bytes()
    references["ppmlhdfe"] = _locked_reference_record(
        "ppmlhdfe", reference_entries["ppmlhdfe"], ppml_archive
    )
    archives["ppmlhdfe"] = ppml_archive
    for name in ("reghdfe", "ftools", "require"):
        repo = getattr(args, f"{name}_repo").resolve()
        expected = reference_entries[name]["commit"]
        commit = _commit(repo, expected)
        observed_tree = _git(repo, "rev-parse", f"{commit}^{{tree}}").decode("ascii").strip()
        if observed_tree != reference_entries[name]["git_tree"]:
            raise SccError(f"{name} checkout tree differs from references.lock.toml")
        archive = _git(
            repo, "archive", "--format=tar", commit, "--",
            str(reference_entries[name]["source_archive_scope"]),
        )
        references[name] = _locked_reference_record(
            name, reference_entries[name], archive
        )
        archives[name] = archive

    spec_bytes = args.capture_spec.read_bytes()
    request_bytes = args.capture_plan.read_bytes()
    spec_sha = digest_bytes(spec_bytes)
    try:
        spec_document = json.loads(spec_bytes)
    except json.JSONDecodeError as error:
        raise SccError(f"capture spec is not JSON: {error}") from error
    if not isinstance(spec_document, Mapping):
        raise SccError("capture spec must be an object")
    try:
        request_document = json.loads(request_bytes)
    except json.JSONDecodeError as error:
        raise SccError(f"capture materialization request is not JSON: {error}") from error
    if not isinstance(request_document, Mapping):
        raise SccError("capture materialization request must be an object")
    manifest_sha = digest_bytes(manifest_bytes)
    _validate_capture_template(spec_document, manifest, manifest_sha)
    if (
        request_document.get("request_type") != "feppml-capture-materialization-request-v1"
        or request_document.get("qualification_manifest_sha256") != manifest_sha
        or request_document.get("comparator_processor_limit")
            != manifest["capture_contract"]["comparator_processor_limit"]
        or request_document.get("cell_ids")
        != manifest["matrix_contract"]["expected_cell_ids"]
    ):
        raise SccError("capture materialization request or closed cell inventory is invalid")
    header = spec_document.get("evidence_header")
    if not isinstance(header, Mapping):
        raise SccError("capture spec has no evidence header")
    candidate_identity = _candidate_record(candidate_repo, candidate_commit, candidate_archive)
    candidate_header = qualification._validate_candidate(header.get("candidate"))
    expected_candidate = {
        "source_commit": candidate_identity["commit"],
        "source_tree": candidate_identity["tree"],
        "cargo_lock_sha256": candidate_identity["cargo_lock_sha256"],
        "references_lock_sha256": candidate_identity["references_lock_sha256"],
        "source_archive_sha256": candidate_identity["archive_sha256"],
        "source_archive_size_bytes": candidate_identity["archive_size_bytes"],
    }
    if any(candidate_header[field] != expected for field, expected in expected_candidate.items()):
        raise SccError("candidate evidence header differs from exact committed archive")
    comparator_header = qualification._validate_comparator(manifest, header.get("comparator"))
    if any(
        comparator_header["references"][name]
        != {
            key: references[name][key]
            for key in ("commit", "tree", "archive_sha256", "archive_size_bytes")
        }
        for name in REFERENCE_NAMES
    ):
        raise SccError("comparator evidence header differs from exact reference archives")
    # Oracle evidence is derived from the completed benchmark bundles during
    # collection.  Preparation only validates that the template reserves the
    # field; there is no benchmark yet from which exact evidence can be derived.
    if not isinstance(header.get("oracles"), list):
        raise SccError("capture spec oracle evidence template must be a list")
    platform_templates = header.get("platform_artifacts")
    qualification._validate_platforms(manifest, platform_templates, candidate_header)
    if header.get("source_audit") != source_audit:
        raise SccError("source audit header differs from committed production audit")
    harness: dict[str, Any] = {}
    for relative in HARNESS_PATHS:
        content = _git(candidate_repo, "show", f"{candidate_commit}:{relative}")
        harness[relative] = {"sha256": digest_bytes(content), "size_bytes": len(content)}
    input_root = run_root / "input"
    for name, archive in archives.items():
        suffix = ".tar.gz" if name == "ppmlhdfe" else ".tar"
        _write(input_root / "archives" / f"{name}{suffix}", archive)
    build_dependencies = _prepare_dependency_closure(
        run_root, candidate_archive
    )
    platform_inputs = _stage_platform_prerequisites(input_root, platform_templates)
    _write(input_root / "manifests/qualification.json", manifest_bytes)
    development_bytes = _git(
        candidate_repo,
        "show",
        f"{candidate_commit}:benchmarks/manifests/ppml_memory_performance_v1.json",
    )
    if request_document.get("development_manifest_sha256") != digest_bytes(
        development_bytes
    ):
        raise SccError("capture request differs from the exact development manifest")
    expected_oracles = manifest["matrix_contract"]["oracle_required_cell_ids"]
    expected_constrained = manifest["memory_gates"]["required_fixtures"]
    expected_constrained = [
        value["id"] if isinstance(value, Mapping) else value
        for value in expected_constrained
    ]
    if (
        request_document.get("oracle_cell_ids") != expected_oracles
        or request_document.get("constrained_fixture_ids") != expected_constrained
    ):
        raise SccError("capture request oracle/constrained inventory is not frozen")
    _write(input_root / "manifests/development.json", development_bytes)
    _write(input_root / "capture/spec-template.json", spec_bytes)
    _write(input_root / "capture/materialization-request.json", request_bytes)
    for relative, identity in harness.items():
        content = _git(candidate_repo, "show", f"{candidate_commit}:{relative}")
        if digest_bytes(content) != identity["sha256"]:
            raise SccError("committed harness changed during preparation")
        _write(input_root / "harness" / relative, content, mode=0o555 if relative.endswith((".py", ".sh")) else 0o444)

    dev_manifest = development.load_manifest(input_root / "manifests/development.json")
    cells = development.expand_cells(dev_manifest)
    tasks = "task_id\tcell_id\n" + "".join(
        f"{index}\t{cell['cell_id']}\n" for index, cell in enumerate(cells, 1)
    )
    _write(input_root / "tasks.tsv", tasks.encode("ascii"))
    file_records = {}
    for path in sorted(input_root.rglob("*")):
        if path.is_file():
            relative = path.relative_to(run_root).as_posix()
            file_records[relative] = {"sha256": digest_file(path), "size_bytes": path.stat().st_size}
    preparation = {
        "schema_version": 1,
        "kind": "feppml_qualification_scc_preparation",
        "run_id": run_root.name,
        "qualification_manifest_sha256": manifest_sha,
        "development_manifest_sha256": digest_bytes(development_bytes),
        "capture_spec_sha256": spec_sha,
        "capture_materialization_request_sha256": digest_bytes(request_bytes),
        "candidate": candidate_identity,
        "references": references,
        "source_audit": source_audit,
        "source_git": source_git,
        "build_dependencies": build_dependencies,
        "platform_inputs": platform_inputs,
        "task_count": len(cells),
        "files": file_records,
        "complete_sentinel": "FEPPML_SCC_PREPARED",
    }
    write_sealed(input_root / "preparation.json", preparation)
    preparation_sha256 = digest_file(input_root / "preparation.json")
    _write(
        input_root / "PREPARED",
        f"FEPPML_SCC_PREPARED|{preparation_sha256}\n".encode("ascii"),
    )


def verify_input(run_root: Path) -> Mapping[str, Any]:
    run_root = _run_root(run_root)
    preparation_path = run_root / "input/preparation.json"
    preparation = read_sealed(
        preparation_path, "feppml_qualification_scc_preparation"
    )
    preparation_sha256 = digest_file(preparation_path)
    if (
        preparation.get("kind") != "feppml_qualification_scc_preparation"
        or preparation.get("run_id") != run_root.name
        or preparation.get("complete_sentinel") != "FEPPML_SCC_PREPARED"
        or (run_root / "input/PREPARED").read_text(encoding="ascii")
        != f"FEPPML_SCC_PREPARED|{preparation_sha256}\n"
    ):
        raise SccError("prepared input identity/sentinel is invalid")
    expected_source_git = {
        **EXPECTED_SOURCE_GIT,
        "environment": formal_build._git_environment(
            Path(EXPECTED_SOURCE_GIT["resolved_path"])
        ),
        "replace_objects_disabled": True,
        "rewrite_mechanisms_absent": True,
    }
    if preparation.get("source_git") != expected_source_git:
        raise SccError("prepared source Git authority differs from the frozen contract")
    try:
        formal_build.verify_dependency_receipt(
            run_root,
            preparation.get("build_dependencies"),
            portable=True,
        )
    except formal_build.ClosureError as error:
        raise SccError(f"prepared formal dependency closure failed: {error}") from error
    for relative, expected in preparation["files"].items():
        path = run_root / relative
        if (
            not path.is_file()
            or path.stat().st_size != expected["size_bytes"]
            or digest_file(path) != expected["sha256"]
        ):
            raise SccError(f"prepared input changed: {relative}")
    observed_files = {
        path.relative_to(run_root).as_posix()
        for path in (run_root / "input").rglob("*")
        if path.is_file()
        and path not in {preparation_path, run_root / "input/PREPARED"}
    }
    if observed_files != set(preparation["files"]):
        raise SccError("prepared input contains missing or unsealed extra files")
    _validate_locked_reference_archives(run_root, preparation)
    return preparation


def _preparation_sha256(run_root: Path) -> str:
    verify_input(run_root)
    return digest_file(run_root / "input/preparation.json")


def _paths(run_root: Path) -> tuple[Path, Path, Path]:
    return (
        run_root / "input/manifests/qualification.json",
        run_root / "input/manifests/development.json",
        run_root / "work/capture/spec.json",
    )


def _extract_archive(archive: Path, destination: Path) -> None:
    if destination.exists():
        raise SccError(f"refusing to reuse extracted source: {destination}")
    destination.mkdir(parents=True)
    with tarfile.open(archive, mode="r:") as source:
        for member in source.getmembers():
            relative = Path(member.name)
            if (
                relative.is_absolute()
                or ".." in relative.parts
                or member.issym()
                or member.islnk()
                or member.isdev()
            ):
                raise SccError(f"unsafe source archive member: {member.name}")
        source.extractall(destination)


def _runtime_file_inventory(root: Path) -> dict[str, dict[str, Any]]:
    if root.is_symlink() or not root.is_dir():
        raise SccError("comparator runtime root is absent or unsafe")
    result: dict[str, dict[str, Any]] = {}
    for path in sorted(root.rglob("*"), key=lambda value: value.relative_to(root).as_posix()):
        if path.is_symlink() or (not path.is_file() and not path.is_dir()):
            raise SccError("comparator runtime contains an unsafe entry")
        if path.is_file():
            relative = path.relative_to(root).as_posix()
            result[relative] = {
                "sha256": digest_file(path),
                "size_bytes": path.stat().st_size,
                "executable": bool(path.stat().st_mode & 0o111),
            }
    return result


def _extract_locked_reference(
    archive: Path,
    destination: Path,
    name: str,
    record: Mapping[str, Any],
) -> None:
    if destination.exists():
        raise SccError(f"refusing to reuse extracted reference: {name}")
    expected = record["runtime_files"]
    destination.mkdir(parents=True)
    root_prefix = f"ppmlhdfe-{record['commit']}/" if name == "ppmlhdfe" else ""
    observed: set[str] = set()
    try:
        with tarfile.open(archive, mode="r:*") as source:
            for member in source.getmembers():
                if not member.isfile():
                    continue
                member_name = member.name
                if root_prefix:
                    if not member_name.startswith(root_prefix):
                        continue
                    member_name = member_name[len(root_prefix):]
                relative = Path(member_name).as_posix()
                if relative not in expected:
                    continue
                if relative in observed:
                    raise SccError(f"{name} reference archive has duplicate files")
                opened = source.extractfile(member)
                if opened is None:
                    raise SccError(f"{name} reference archive file is unreadable")
                target = destination / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                _write(target, opened.read(), mode=0o555 if expected[relative]["executable"] else 0o444)
                observed.add(relative)
    except tarfile.TarError as error:
        raise SccError(f"{name} reference archive extraction failed: {error}") from error
    if observed != set(expected) or _runtime_file_inventory(destination) != expected:
        raise SccError(f"{name} extracted runtime inventory differs from its locked archive")


def _rebound_dependency_receipt(
    run_root: Path, source_receipt: Mapping[str, Any]
) -> Mapping[str, Any]:
    """Purely derive the one permitted run-local vendor receipt relocation."""

    record = copy.deepcopy(source_receipt)
    old_prefix = Path("input/formal-build")
    new_prefix = Path("work/formal-build")
    for field in ("config", "vendor_log", "vendor_stdout"):
        relative = Path(str(record[field]["path"]))
        try:
            suffix = relative.relative_to(old_prefix)
        except ValueError as error:
            raise SccError("formal dependency file is outside its preparation root") from error
        record[field]["path"] = (new_prefix / suffix).as_posix()
    vendor = Path(str(record["vendor"]["path"]))
    try:
        vendor_suffix = vendor.relative_to(old_prefix)
    except ValueError as error:
        raise SccError("formal vendor tree is outside its preparation root") from error
    record["vendor"]["path"] = (new_prefix / vendor_suffix).as_posix()
    record["command"][-1] = os.path.relpath(
        run_root / record["vendor"]["path"],
        run_root / record["cwd"],
    )
    return record


def _relocated_dependency_receipt(
    run_root: Path, source_receipt: Mapping[str, Any]
) -> Mapping[str, Any]:
    """Copy and verify the preparation-time vendor closure at its build path."""

    source_root = run_root / "input/formal-build"
    destination = run_root / "work/formal-build"
    if destination.exists():
        raise SccError("formal run-local build closure already exists")
    shutil.copytree(source_root, destination, symlinks=False)
    record = _rebound_dependency_receipt(run_root, source_receipt)
    try:
        formal_build.verify_dependency_receipt(
            run_root, record, portable=True
        )
    except formal_build.ClosureError as error:
        raise SccError(f"relocated formal dependency closure failed: {error}") from error
    return record


def _formal_helper_paths() -> dict[str, Path]:
    return {
        "cc": Path("/usr/bin/cc"),
        "linker": Path("/usr/bin/cc"),
        "ar": Path("/usr/bin/ar"),
        "ranlib": Path("/usr/bin/ranlib"),
        "nm": Path("/usr/bin/nm"),
        "strip": Path("/usr/bin/strip"),
        "objcopy": Path("/usr/bin/objcopy"),
    }


def _formal_build_tool_paths() -> dict[str, Path]:
    return {
        "bash": Path("/bin/bash"),
        "python3": Path(EXPECTED_CONTROLLER_PYTHON["path"]),
        "dirname": Path("/usr/bin/dirname"),
        "basename": Path("/usr/bin/basename"),
        "mkdir": Path("/usr/bin/mkdir"),
        "rm": Path("/usr/bin/rm"),
        "sha256sum": Path("/usr/bin/sha256sum"),
        "readelf": Path("/usr/bin/readelf"),
        "nm": Path("/usr/bin/nm"),
        "ldd": Path("/usr/bin/ldd"),
        "uname": Path("/usr/bin/uname"),
    }


def _build_tool_paths() -> dict[str, Path]:
    """Resolve the fixed SCC build executables without consulting ambient PATH."""

    cargo_bin = EXPECTED_CARGO_HOME / "bin"
    return {
        "bash": Path("/bin/bash"),
        "cargo": cargo_bin / "cargo",
        "rustc": cargo_bin / "rustc",
        "python3": Path(sys.executable).resolve(),
        "cc": Path("/usr/bin/cc"),
        "linker": Path("/usr/bin/cc"),
        "ar": Path("/usr/bin/ar"),
        "ranlib": Path("/usr/bin/ranlib"),
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
    for role in ("cargo", "python3", "cc", "bash", *BUILD_TOOL_ROLES):
        parent = str(paths[role].parent)
        if parent not in directories:
            directories.append(parent)
    return ":".join(directories)


def _build_version(
    role: str, path: Path, environment: Mapping[str, str]
) -> str:
    completed = subprocess.run(
        [str(path), "--version"],
        env=dict(environment),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        check=False,
    )
    lines = completed.stdout.splitlines()
    if completed.returncode != 0 or not lines:
        raise SccError(f"formal build tool did not report a version: {role}")
    return lines[0].strip()


def _build_tool_records(
    paths: Mapping[str, Path], environment: Mapping[str, str]
) -> dict[str, dict[str, Any]]:
    if set(paths) != set(BUILD_TOOL_ROLES):
        raise SccError("formal build tool role inventory is not closed")
    records: dict[str, dict[str, Any]] = {}
    for role in BUILD_TOOL_ROLES:
        path = paths[role]
        resolved = path.resolve()
        if not path.is_file() or not os.access(path, os.X_OK) or not resolved.is_file():
            raise SccError(f"formal build tool is absent or not executable: {role}")
        records[role] = {
            "path": str(path),
            "resolved_path": str(resolved),
            "sha256": digest_file(resolved),
            "size_bytes": resolved.stat().st_size,
            "version": _build_version(role, path, environment),
        }
    if (
        records["cargo"]["version"] != EXPECTED_CARGO
        or records["rustc"]["version"] != EXPECTED_RUSTC
        or records["python3"]["version"] != EXPECTED_PYTHON
    ):
        raise SccError("formal build cargo/rustc/Python versions differ from SCC contract")
    return records


def _formal_build_environment(
    run_root: Path,
    candidate: Mapping[str, Any],
    tools: Mapping[str, Mapping[str, Any]],
    *,
    slots: int,
) -> dict[str, str]:
    """Return the complete build environment; no caller environment is inherited."""

    if slots != 16 or set(tools) != set(BUILD_TOOL_ROLES):
        raise SccError("formal build requires the exact 16-slot tool closure")
    path_value = _build_path(
        {role: Path(str(tools[role]["path"])) for role in BUILD_TOOL_ROLES}
    )
    return {
        "HOME": str(run_root / "work/build-home"),
        "TMPDIR": str(run_root / "work/build-tmp"),
        "PATH": path_value,
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "CARGO_HOME": str(EXPECTED_CARGO_HOME),
        "RUSTUP_HOME": str(EXPECTED_RUSTUP_HOME),
        "RUSTUP_TOOLCHAIN": EXPECTED_RUSTUP_TOOLCHAIN,
        "CARGO_NET_OFFLINE": "true",
        "CARGO_INCREMENTAL": "0",
        "CARGO_BUILD_JOBS": "16",
        "CC": str(tools["cc"]["path"]),
        "AR": str(tools["ar"]["path"]),
        "RANLIB": str(tools["ranlib"]["path"]),
        "RUSTC": str(tools["rustc"]["path"]),
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER": str(
            tools["linker"]["path"]
        ),
        "FEREG_GIT_COMMIT": str(candidate["commit"][:12]),
        "FEREG_BUILD_DATE": str(candidate["build_date_utc"]),
    }


def _relative_file_record(run_root: Path, path: Path) -> dict[str, Any]:
    relative = path.resolve().relative_to(run_root.resolve()).as_posix()
    return {
        "path": relative,
        "sha256": digest_file(path),
        "size_bytes": path.stat().st_size,
    }


def _verify_build_receipt(
    run_root: Path,
    preparation: Mapping[str, Any],
    receipt: Any,
    *,
    portable: bool,
) -> None:
    record = receipt if isinstance(receipt, Mapping) else {}
    expected_fields = {
        "schema_version", "receipt_type", "source_commit", "source_tree",
        "build_date_utc", "command", "cwd", "environment", "formal_closure",
        "dependency_origin_sha256", "runtime_closure", "log", "artifact",
        "complete_sentinel",
    }
    if (
        set(record) != expected_fields
        or record.get("schema_version") != 1
        or record.get("receipt_type") != "feppml-formal-linux-build-v1"
        or record.get("source_commit") != preparation["candidate"]["commit"]
        or record.get("source_tree") != preparation["candidate"]["tree"]
        or record.get("build_date_utc")
            != preparation["candidate"]["build_date_utc"]
        or record.get("complete_sentinel") != "FEPPML_FORMAL_LINUX_BUILD_COMPLETE"
    ):
        raise SccError("formal candidate build receipt identity is invalid")
    formal_receipt = record.get("formal_closure")
    runtime_receipt = record.get("runtime_closure")
    try:
        qualified = formal_build.verify_qualified_formal_rust_build(
            run_root,
            formal_receipt,
            runtime_receipt,
            portable=True,
        )
        if not portable:
            formal_build.verify_toolchain_receipt(
                formal_receipt["toolchain"], portable=False
            )
            formal_build.verify_linux_runtime_closure(
                run_root,
                runtime_receipt,
                formal_receipt["toolchain"],
                portable=False,
            )
    except formal_build.ClosureError as error:
        raise SccError(f"formal build supply-chain receipt failed: {error}") from error
    formal_verified = formal_receipt
    environment = record.get("environment")
    if environment != formal_verified["environment"]:
        raise SccError("formal build environment differs from the supply-chain receipt")
    dependency_origin_sha256 = digest_bytes(
        canonical(preparation["build_dependencies"])
    )
    expected_relocation = _rebound_dependency_receipt(
        run_root, preparation["build_dependencies"]
    )
    if (
        record.get("dependency_origin_sha256") != dependency_origin_sha256
        or formal_verified["dependencies"] != expected_relocation
    ):
        raise SccError(
            "formal build dependency relocation is not derived from the sealed preparation"
        )
    recorded_root = Path(record["cwd"]).parents[1]
    if (
        recorded_root.name != run_root.name
        or record["cwd"] != str(recorded_root / "work/candidate")
        or record["command"]
            != ["/bin/bash", "scripts/build_linux.sh"]
        or formal_verified["source_root"] != "work/candidate"
        or formal_verified["slots"] != 16
        or formal_verified["build_variables"]
            != {
                "FEREG_GIT_COMMIT": preparation["candidate"]["commit"][:12],
                "FEREG_BUILD_DATE": preparation["candidate"]["build_date_utc"],
            }
    ):
        raise SccError("formal build command/cwd is not the exact SCC invocation")
    for field in ("log", "artifact"):
        file_record = record[field]
        if not isinstance(file_record, Mapping) or set(file_record) != {
            "path", "sha256", "size_bytes"
        }:
            raise SccError(f"formal build {field} record is not closed")
        local = run_root / str(file_record["path"])
        if (
            Path(str(file_record["path"])).is_absolute()
            or ".." in Path(str(file_record["path"])).parts
            or not local.is_file()
            or local.is_symlink()
            or digest_file(local) != file_record["sha256"]
            or local.stat().st_size != file_record["size_bytes"]
        ):
            raise SccError(f"formal build {field} bytes changed")
    if qualified.get("artifact") != record["artifact"]:
        raise SccError("formal runtime closure names another candidate artifact")


def _stage_exact_bundles(run_root: Path, preparation: Mapping[str, Any]) -> Mapping[str, Any]:
    """Build the exact candidate and stage the frozen comparator closure."""
    source = run_root / "work/candidate"
    qualification_manifest = qualification.load_manifest(
        run_root / "input/manifests/qualification.json"
    )
    capture_template = _read(
        run_root / "input/capture/spec-template.json", "capture spec template"
    )
    qualification._verify_candidate_archive_tree(
        source,
        run_root / "input/archives/candidate.tar",
        qualification_manifest,
        {"candidate": capture_template["evidence_header"]["candidate"]},
        manifest_path=(
            source / "benchmarks/manifests/feppml_qualification_v1.json"
        ),
    )
    logs = run_root / "logs/build"
    logs.mkdir(parents=True, exist_ok=False)
    try:
        slots = int(os.environ.get("NSLOTS", ""))
    except ValueError as error:
        raise SccError("formal candidate build has no exact NSLOTS value") from error
    if slots != 16:
        raise SccError("formal candidate build requires exactly 16 SCC slots")
    dependency = _relocated_dependency_receipt(
        run_root, preparation["build_dependencies"]
    )
    closure_root = run_root / "work/formal-build"
    (closure_root / "home").mkdir()
    (closure_root / "tmp").mkdir()
    target_root = run_root / "work/formal-target"
    target_root.mkdir()
    rust_toolchain = (
        EXPECTED_RUSTUP_HOME / "toolchains" / EXPECTED_RUSTUP_TOOLCHAIN / "bin"
    )
    try:
        toolchain = formal_build.capture_toolchain(
            closure_root / "toolchain-probe",
            cargo=rust_toolchain / "cargo",
            rustc=rust_toolchain / "rustc",
            helpers=_formal_helper_paths(),
            build_tools=_formal_build_tool_paths(),
        )
        build_variables = {
            "FEREG_GIT_COMMIT": str(preparation["candidate"]["commit"][:12]),
            "FEREG_BUILD_DATE": str(preparation["candidate"]["build_date_utc"]),
        }
        environment = formal_build.closed_build_environment(
            run_root,
            closure_root,
            target_root,
            toolchain,
            slots=slots,
            build_variables=build_variables,
        )
        formal_receipt = {
            "schema_version": formal_build.SCHEMA_VERSION,
            "receipt_type": formal_build.RECEIPT_TYPE,
            "run_root_name": run_root.name,
            "source_root": source.relative_to(run_root).as_posix(),
            "closure_root": closure_root.relative_to(run_root).as_posix(),
            "target_root": target_root.relative_to(run_root).as_posix(),
            "slots": slots,
            "build_variables": build_variables,
            "dependencies": dependency,
            "toolchain": toolchain,
            "environment": environment,
            "complete_sentinel": formal_build.COMPLETE_SENTINEL,
        }
        formal_build.verify_toolchain_receipt(toolchain, portable=False)
        formal_build.verify_formal_rust_build_closure(
            run_root, formal_receipt, portable=True
        )
    except formal_build.ClosureError as error:
        raise SccError(f"formal Rust build closure failed: {error}") from error
    command = ["/bin/bash", "scripts/build_linux.sh"]
    completed = subprocess.run(
        command, cwd=source, env=environment,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=False,
    )
    build_log = logs / "candidate.log"
    _write(build_log, completed.stdout.encode("utf-8"))
    plugin = source / "dist/fereg_linux.plugin"
    if completed.returncode != 0 or not plugin.is_file():
        raise SccError("exact candidate Linux build/audit failed")
    if preparation["candidate"]["commit"][:12].encode("ascii") not in plugin.read_bytes():
        raise SccError("candidate plugin omits exact source commit")
    try:
        runtime_closure = formal_build.capture_linux_runtime_closure(
            run_root, plugin, toolchain
        )
    except formal_build.ClosureError as error:
        raise SccError(f"formal Linux runtime closure failed: {error}") from error
    candidate = run_root / "work/bundles/candidate/stata"
    shutil.copytree(source / "stata", candidate)
    shutil.copy2(plugin, candidate / "fereg_linux.plugin")
    comparator = run_root / "work/bundles/comparator"
    comparator.mkdir(parents=True, exist_ok=False)
    reference_inventory = {}
    for name in REFERENCE_NAMES:
        reference = run_root / "work/references" / name
        destination = comparator / name
        shutil.copytree(reference, destination)
        observed = _runtime_file_inventory(destination)
        if observed != preparation["references"][name]["runtime_files"]:
            raise SccError(f"{name} runtime comparator inventory changed during staging")
        reference_inventory[name] = {
            "commit": preparation["references"][name]["commit"],
            "tree": preparation["references"][name]["tree"],
            "archive_sha256": preparation["references"][name]["archive_sha256"],
            "files": len(observed),
            "bytes": sum(value["size_bytes"] for value in observed.values()),
            "inventory_sha256": digest_bytes(canonical(observed)),
        }
    build_receipt = {
        "schema_version": 1,
        "receipt_type": "feppml-formal-linux-build-v1",
        "source_commit": preparation["candidate"]["commit"],
        "source_tree": preparation["candidate"]["tree"],
        "build_date_utc": preparation["candidate"]["build_date_utc"],
        "command": command,
        "cwd": str(source.resolve()),
        "environment": environment,
        "formal_closure": formal_receipt,
        "dependency_origin_sha256": digest_bytes(
            canonical(preparation["build_dependencies"])
        ),
        "runtime_closure": runtime_closure,
        "log": _relative_file_record(run_root, build_log),
        "artifact": _relative_file_record(run_root, plugin),
        "complete_sentinel": "FEPPML_FORMAL_LINUX_BUILD_COMPLETE",
    }
    _verify_build_receipt(
        run_root, preparation, build_receipt, portable=False
    )
    return {
        "candidate_plugin": {
            "path": str(candidate / "fereg_linux.plugin"),
            "sha256": digest_file(candidate / "fereg_linux.plugin"),
            "size_bytes": (candidate / "fereg_linux.plugin").stat().st_size,
        },
        "candidate_stata": str(candidate),
        "comparator_root": str(comparator),
        "references": reference_inventory,
        "build": build_receipt,
    }


def _setup_stata_identity() -> dict[str, Any]:
    raw = os.environ.get("FEPPML_STATA_EXECUTABLE", "")
    if not raw or not Path(raw).is_absolute():
        raise SccError("setup has no explicitly bound licensed Stata executable")
    path = Path(raw)
    resolved = path.resolve()
    discovered = shutil.which("stata-mp")
    if (
        path.name.lower() != "stata-mp"
        or not path.is_file()
        or not os.access(path, os.X_OK)
        or not resolved.is_file()
        or discovered is None
        or Path(discovered).resolve() != resolved
    ):
        raise SccError("setup Stata identity differs from the loaded MP module")
    return {
        "path": str(path),
        "resolved_path": str(resolved),
        "sha256": digest_file(resolved),
        "size_bytes": resolved.stat().st_size,
    }


def _setup_stata_environment(
    recorded_root: Path, action_id: str, stata: Mapping[str, Any]
) -> dict[str, str]:
    action_root = recorded_root / "work/setup-stata" / action_id
    return {
        "HOME": str(action_root / "home"),
        "TMPDIR": str(action_root / "tmp"),
        "STATATMP": str(action_root / "tmp"),
        "PATH": f"{Path(str(stata['path'])).parent}:/usr/bin:/bin",
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
    }


def _setup_stata_wrapper(action_id: str, body: Sequence[str]) -> bytes:
    return (
        "\n".join(
            [
                "version 19.0",
                "clear all",
                "set more off",
                "set linesize 255",
                *body,
                (
                    'display as result "FEPPML_SETUP_ACTION_COMPLETE|'
                    f'{action_id}|" c(stata_version) "|" c(flavor)'
                ),
                "exit 0",
                "",
            ]
        )
    ).encode("ascii")


def _run_setup_stata_action(
    run_root: Path,
    action_id: str,
    stata: Mapping[str, Any],
    body: Sequence[str],
    outputs: Mapping[str, Path],
) -> dict[str, Any]:
    if re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,95}", action_id) is None:
        raise SccError("setup Stata action ID is unsafe")
    action_root = run_root / "work/setup-stata" / action_id
    action_root.mkdir(parents=True, exist_ok=False)
    (action_root / "home").mkdir()
    (action_root / "tmp").mkdir()
    wrapper = action_root / "run.do"
    _write(wrapper, _setup_stata_wrapper(action_id, body), mode=0o444)
    command = [
        str(stata["path"]), "-q", "-b", "do", str(wrapper.resolve())
    ]
    environment = _setup_stata_environment(run_root, action_id, stata)
    stdout_path = action_root / "process.stdout"
    stderr_path = action_root / "process.stderr"
    with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr:
        completed = subprocess.run(
            command,
            cwd=action_root,
            env=environment,
            stdout=stdout,
            stderr=stderr,
            check=False,
        )
    log_path = action_root / "run.log"
    log_text = (
        log_path.read_text(encoding="utf-8", errors="strict")
        if log_path.is_file()
        else ""
    )
    prefix = f"FEPPML_SETUP_ACTION_COMPLETE|{action_id}|"
    sentinels = [
        line.strip() for line in log_text.splitlines()
        if line.strip().startswith(prefix)
    ]
    if completed.returncode != 0 or len(sentinels) != 1:
        raise SccError(f"licensed Stata setup action failed: {action_id}")
    match = re.fullmatch(
        re.escape(prefix) + r"([^|\s]+)\|([^|\s]+)", sentinels[0]
    )
    if match is None or match.group(2) != "MP":
        raise SccError("setup action did not prove exact licensed Stata MP flavor")
    output_records = {
        name: _relative_file_record(run_root, path)
        for name, path in sorted(outputs.items())
        if path.is_file() and not path.is_symlink()
    }
    if set(output_records) != set(outputs):
        raise SccError(f"setup Stata action omitted an exact output: {action_id}")
    return {
        "schema_version": 1,
        "receipt_type": "feppml-setup-stata-action-v1",
        "action_id": action_id,
        "stata": dict(stata),
        "stata_version": match.group(1),
        "stata_flavor": match.group(2),
        "command": command,
        "cwd": str(action_root.resolve()),
        "environment": environment,
        "return_code": completed.returncode,
        "raw_root": action_root.relative_to(run_root).as_posix(),
        "raw_files": _bound_file_inventory(run_root, [action_root]),
        "outputs": output_records,
        "terminal_sentinel": sentinels[0],
        "complete_sentinel": "FEPPML_SETUP_STATA_ACTION_COMPLETE",
    }


def _verify_setup_stata_action(
    run_root: Path,
    action_id: str,
    receipt: Any,
    expected_body: Sequence[str],
    expected_outputs: Mapping[str, Path],
    *,
    portable: bool,
) -> None:
    record = receipt if isinstance(receipt, Mapping) else {}
    expected_fields = {
        "schema_version", "receipt_type", "action_id", "stata",
        "stata_version", "stata_flavor", "command", "cwd", "environment",
        "return_code", "raw_root", "raw_files", "outputs",
        "terminal_sentinel", "complete_sentinel",
    }
    if (
        set(record) != expected_fields
        or record.get("schema_version") != 1
        or record.get("receipt_type") != "feppml-setup-stata-action-v1"
        or record.get("action_id") != action_id
        or record.get("stata_flavor") != "MP"
        or record.get("return_code") != 0
        or record.get("complete_sentinel")
            != "FEPPML_SETUP_STATA_ACTION_COMPLETE"
    ):
        raise SccError(f"setup Stata action receipt is invalid: {action_id}")
    stata = record.get("stata")
    if not isinstance(stata, Mapping) or set(stata) != {
        "path", "resolved_path", "sha256", "size_bytes"
    }:
        raise SccError("setup Stata executable record is not closed")
    recorded_root = Path(str(record["cwd"])).parents[2]
    expected_cwd = recorded_root / "work/setup-stata" / action_id
    local_wrapper = run_root / "work/setup-stata" / action_id / "run.do"
    local_action_root = local_wrapper.parent
    expected_raw_paths = {
        f"work/setup-stata/{action_id}/{name}"
        for name in ("run.do", "run.log", "process.stdout", "process.stderr")
    }
    raw_files = record.get("raw_files")
    log_path = local_action_root / "run.log"
    stdout_path = local_action_root / "process.stdout"
    stderr_path = local_action_root / "process.stderr"
    if (
        not isinstance(raw_files, Mapping)
        or set(raw_files) != expected_raw_paths
        or not log_path.is_file()
        or not stdout_path.is_file()
        or not stderr_path.is_file()
        or stdout_path.read_bytes() != b""
        or stderr_path.read_bytes() != b""
    ):
        raise SccError(f"setup Stata raw file inventory is not exact: {action_id}")
    prefix = f"FEPPML_SETUP_ACTION_COMPLETE|{action_id}|"
    sentinels = [
        line.strip()
        for line in log_path.read_text(
            encoding="utf-8", errors="strict"
        ).splitlines()
        if line.strip().startswith(prefix)
    ]
    if (
        recorded_root.name != run_root.name
        or record["cwd"] != str(expected_cwd)
        or record["raw_root"] != f"work/setup-stata/{action_id}"
        or record["command"]
            != [
                str(stata["path"]), "-q", "-b", "do",
                str(expected_cwd / "run.do"),
            ]
        or record["environment"]
            != _setup_stata_environment(recorded_root, action_id, stata)
        or record["terminal_sentinel"]
            != (
                f"FEPPML_SETUP_ACTION_COMPLETE|{action_id}|"
                f"{record['stata_version']}|MP"
            )
        or not local_wrapper.is_file()
        or local_wrapper.read_bytes()
            != _setup_stata_wrapper(action_id, expected_body)
        or sentinels != [record["terminal_sentinel"]]
    ):
        raise SccError(f"setup Stata action invocation is misbound: {action_id}")
    _verify_bound_file_inventory(
        run_root, [record["raw_root"]], record["raw_files"]
    )
    outputs = record.get("outputs")
    if not isinstance(outputs, Mapping) or set(outputs) != set(expected_outputs):
        raise SccError(f"setup Stata action output inventory differs: {action_id}")
    for name, local in expected_outputs.items():
        expected = outputs[name]
        if (
            not isinstance(expected, Mapping)
            or expected.get("path") != local.relative_to(run_root).as_posix()
            or not local.is_file()
            or local.is_symlink()
            or digest_file(local) != expected.get("sha256")
            or local.stat().st_size != expected.get("size_bytes")
        ):
            raise SccError(f"setup Stata action output bytes changed: {action_id}")
    if not portable:
        path = Path(str(stata["path"]))
        resolved = path.resolve()
        if (
            not path.is_file()
            or str(resolved) != stata["resolved_path"]
            or digest_file(resolved) != stata["sha256"]
            or resolved.stat().st_size != stata["size_bytes"]
        ):
            raise SccError("licensed Stata executable changed after setup")


def _dataset_action_body(
    generator: Path, destination: Path, observations: int
) -> list[str]:
    return [
        f'capture noisily do "{generator}" "{destination}"',
        "if _rc exit _rc",
        f'confirm file "{destination}"',
        f'use "{destination}", clear',
        f"assert _N == {observations}",
        "isid obs_id",
        "confirm numeric variable y",
    ]


def _oracle_action_body(
    generator: Path, data: Path, export: Path, observations: int
) -> list[str]:
    return [
        f'capture noisily do "{generator}" "{data}" "{export}"',
        "if _rc exit _rc",
        f'confirm file "{data}"',
        f'confirm file "{export}"',
        f'use "{data}", clear',
        f"assert _N == {observations}",
        "isid obs_id",
        "confirm numeric variable y",
    ]


def _materialize_datasets(
    run_root: Path,
    recipe_document: Mapping[str, Any],
    stata: Mapping[str, Any],
) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    dataset_root = run_root / "work/datasets"
    dataset_root.mkdir(parents=True, exist_ok=False)
    inventory: dict[str, dict[str, Any]] = {}
    actions: dict[str, dict[str, Any]] = {}
    for cell in recipe_document["cells"]:
        dataset_id = cell["dataset_id"]
        if dataset_id in inventory:
            continue
        destination = dataset_root / f"{dataset_id}.dta"
        generator = run_root / "work/recipes" / cell["generator"]
        action_id = f"dataset-{dataset_id}"
        actions[dataset_id] = _run_setup_stata_action(
            run_root,
            action_id,
            stata,
            _dataset_action_body(generator, destination, int(cell["n"])),
            {"dataset": destination},
        )
        inventory[dataset_id] = {
            "path": str(destination), "sha256": digest_file(destination),
            "size_bytes": destination.stat().st_size,
            "generator_sha256": digest_file(generator), "seed": cell["seed"],
            "observations": cell["n"],
        }
        destination.chmod(0o444)
    return inventory, actions


def _materialize_oracle_fixtures(
    run_root: Path,
    development_manifest: Path,
    stata: Mapping[str, Any],
) -> tuple[Mapping[str, Any], list[dict[str, Any]], Mapping[str, Any]]:
    """Compile and materialize every bounded oracle prerequisite once."""

    source_root = run_root / "work/oracle-recipes"
    source_root.mkdir(parents=True, exist_ok=False)
    document = oracle_recipes.compile_fixtures(development_manifest, source_root)
    output_root = run_root / "work/oracle-fixtures"
    output_root.mkdir(parents=True, exist_ok=False)
    inventory: list[dict[str, Any]] = []
    actions: dict[str, dict[str, Any]] = {}
    for record in document["cells"]:
        cell_id = record["cell_id"]
        generator = source_root / record["generator"]
        model = source_root / record["model"]
        fixture_root = output_root / cell_id
        fixture_root.mkdir(parents=True, exist_ok=False)
        data = fixture_root / "fixture.dta"
        export = fixture_root / "fixture.csv"
        action_id = f"oracle-{cell_id}"
        actions[cell_id] = _run_setup_stata_action(
            run_root,
            action_id,
            stata,
            _oracle_action_body(
                generator, data, export, int(record["row_count"])
            ),
            {"data": data, "canonical_export": export},
        )
        for path in (data, export):
            path.chmod(0o444)
        inventory.append(
            {
                **dict(record),
                "generator": materializer.file_record(generator),
                "model": materializer.file_record(model),
                "data": materializer.file_record(data),
                "canonical_export": materializer.file_record(export),
            }
        )
    return document, inventory, actions


def _ftools_action_body(
    source_root: Path, library: Path, *, compile_library: bool
) -> list[str]:
    expected_ado = source_root / "ftools.ado"
    lines = [
        f'adopath ++ "{source_root}"',
        "capture findfile ftools.ado",
        "if _rc exit _rc",
        "local found = r(fn)",
        f'''if "`found'" != "{expected_ado}" exit 603''',
    ]
    if compile_library:
        lines.extend(
            [
                "capture noisily ftools, compile",
                "if _rc exit _rc",
            ]
        )
    lines.extend(
        [
            "capture findfile lftools.mlib",
            "if _rc exit _rc",
            "local found = r(fn)",
            f'''if "`found'" != "{library}" exit 603''',
            "capture mata: mata mlib index",
            "if _rc exit _rc",
            "capture mata: mata describe using lftools",
            "if _rc exit _rc",
        ]
    )
    return lines


def _compile_private_ftools(
    run_root: Path, stata: Mapping[str, Any]
) -> dict[str, Any]:
    source_root = run_root / "work/bundles/comparator/ftools/src"
    library = source_root / "l/lftools.mlib"
    if library.exists():
        raise SccError("private ftools library exists before controlled compilation")
    compile_body = _ftools_action_body(
        source_root, library, compile_library=True
    )
    compiled = _run_setup_stata_action(
        run_root,
        "ftools-compile",
        stata,
        compile_body,
        {"lftools_mlib": library},
    )
    matches = sorted(
        (run_root / "work/bundles/comparator").rglob("lftools.mlib")
    )
    if matches != [library] or library.is_symlink() or not library.is_file():
        raise SccError("controlled ftools compilation did not create one exact mlib")
    library.chmod(0o444)
    verify_body = _ftools_action_body(
        source_root, library, compile_library=False
    )
    verified = _run_setup_stata_action(
        run_root,
        "ftools-verify",
        stata,
        verify_body,
        {"lftools_mlib": library},
    )
    if compiled["outputs"] != verified["outputs"]:
        raise SccError("private ftools library changed in a fresh licensed process")
    return {
        "library": _relative_file_record(run_root, library),
        "compile": compiled,
        "verify": verified,
    }


def _verify_setup_stata_closure(
    run_root: Path,
    setup_receipt: Mapping[str, Any],
    *,
    portable: bool,
) -> None:
    closure = setup_receipt.get("setup_stata")
    if not isinstance(closure, Mapping) or set(closure) != {
        "identity", "datasets", "oracles", "ftools"
    }:
        raise SccError("setup receipt omits the closed licensed-Stata actions")
    identity = closure.get("identity")
    if not isinstance(identity, Mapping) or set(identity) != {
        "path", "resolved_path", "sha256", "size_bytes"
    }:
        raise SccError("setup receipt has an invalid licensed-Stata identity")

    dev_path = run_root / "input/manifests/development.json"
    recipe_root = run_root / "work/recipes"
    recipe_document = _read(recipe_root / "recipes.json", "recipe manifest")
    recipes.validate_compiled_recipes(recipe_document, dev_path, recipe_root)
    expected_datasets: dict[str, Mapping[str, Any]] = {}
    for cell in recipe_document["cells"]:
        expected_datasets.setdefault(cell["dataset_id"], cell)
    dataset_actions = closure.get("datasets")
    datasets = setup_receipt.get("datasets")
    if (
        not isinstance(dataset_actions, Mapping)
        or not isinstance(datasets, Mapping)
        or set(dataset_actions) != set(expected_datasets)
        or set(datasets) != set(expected_datasets)
    ):
        raise SccError("setup dataset action inventory is not exact")
    for dataset_id, cell in expected_datasets.items():
        action = dataset_actions[dataset_id]
        if not isinstance(action, Mapping):
            raise SccError("setup dataset action is malformed")
        if action.get("stata") != identity:
            raise SccError("setup dataset used different licensed-Stata bytes")
        recorded_root = Path(str(action.get("cwd", ""))).parents[2]
        generator = recorded_root / "work/recipes" / cell["generator"]
        destination = recorded_root / "work/datasets" / f"{dataset_id}.dta"
        _verify_setup_stata_action(
            run_root,
            f"dataset-{dataset_id}",
            action,
            _dataset_action_body(generator, destination, int(cell["n"])),
            {"dataset": run_root / "work/datasets" / f"{dataset_id}.dta"},
            portable=portable,
        )

    oracle_root = run_root / "work/oracle-recipes"
    oracle_document = _read(oracle_root / "fixtures.json", "oracle fixtures")
    oracle_recipes.validate_compiled_fixtures(
        oracle_document, dev_path, oracle_root
    )
    constrained_root = run_root / "work/constrained-recipes"
    constrained_document = _read(
        constrained_root / "constrained.json", "constrained recipes"
    )
    constrained_recipes.validate_compiled_recipes(
        constrained_document, dev_path, constrained_root
    )
    canonical_profile = recipe_contract.canonical_profile(dev_path)
    frozen_manifest = qualification.load_manifest(
        run_root / "input/manifests/qualification.json"
    )
    if (
        recipe_contract.profile_digest(canonical_profile)
        != frozen_manifest["recipe_contract"]["profile_sha256"]
    ):
        raise SccError("independent qualification recipe profile changed")
    recipe_contract.validate_documents(
        canonical_profile,
        recipe_document,
        oracle_document,
        constrained_document,
    )
    recipe_contract.validate_generator_sources(
        canonical_profile,
        recipe_document,
        recipe_root,
        oracle_document,
        oracle_root,
    )
    recipe_contract.validate_executable_sources(
        canonical_profile, recipe_document, recipe_root
    )
    recipe_contract.validate_oracle_models(
        canonical_profile, oracle_document, oracle_root
    )
    recipe_contract.validate_constrained_sources(
        canonical_profile, constrained_document, constrained_root
    )
    oracle_actions = closure.get("oracles")
    oracle_inventory = setup_receipt.get("oracle_fixtures")
    if (
        not isinstance(oracle_actions, Mapping)
        or not isinstance(oracle_inventory, list)
        or set(oracle_actions)
            != {record["cell_id"] for record in oracle_document["cells"]}
        or [record.get("cell_id") for record in oracle_inventory]
            != [record["cell_id"] for record in oracle_document["cells"]]
    ):
        raise SccError("setup oracle action inventory is not exact")
    for record in oracle_document["cells"]:
        cell_id = record["cell_id"]
        action = oracle_actions[cell_id]
        if not isinstance(action, Mapping):
            raise SccError("setup oracle action is malformed")
        if action.get("stata") != identity:
            raise SccError("setup oracle used different licensed-Stata bytes")
        recorded_root = Path(str(action.get("cwd", ""))).parents[2]
        fixture_root = recorded_root / "work/oracle-fixtures" / cell_id
        _verify_setup_stata_action(
            run_root,
            f"oracle-{cell_id}",
            action,
            _oracle_action_body(
                recorded_root / "work/oracle-recipes" / record["generator"],
                fixture_root / "fixture.dta",
                fixture_root / "fixture.csv",
                int(record["row_count"]),
            ),
            {
                "data": run_root / "work/oracle-fixtures" / cell_id / "fixture.dta",
                "canonical_export": run_root
                / "work/oracle-fixtures"
                / cell_id
                / "fixture.csv",
            },
            portable=portable,
        )

    ftools = closure.get("ftools")
    if not isinstance(ftools, Mapping) or set(ftools) != {
        "library", "compile", "verify"
    }:
        raise SccError("setup ftools compilation receipt is not closed")
    library = run_root / "work/bundles/comparator/ftools/src/l/lftools.mlib"
    for action_id, compile_library in (
        ("ftools-compile", True), ("ftools-verify", False)
    ):
        action = ftools[action_id.split("-", 1)[1]]
        if not isinstance(action, Mapping):
            raise SccError("setup ftools action is malformed")
        recorded_root = Path(str(action.get("cwd", ""))).parents[2]
        recorded_source = recorded_root / "work/bundles/comparator/ftools/src"
        recorded_library = recorded_source / "l/lftools.mlib"
        _verify_setup_stata_action(
            run_root,
            action_id,
            action,
            _ftools_action_body(
                recorded_source,
                recorded_library,
                compile_library=compile_library,
            ),
            {"lftools_mlib": library},
            portable=portable,
        )
        if action.get("stata") != identity:
            raise SccError("setup actions used different licensed-Stata bytes")
    if (
        not isinstance(ftools["library"], Mapping)
        or ftools["library"].get("path")
            != library.relative_to(run_root).as_posix()
        or ftools["compile"].get("outputs")
            != ftools["verify"].get("outputs")
    ):
        raise SccError("setup ftools output binding is invalid")


def _write_comparator_artifact(comparator_root: Path, artifact: Path) -> Path:
    """Create a deterministic USTAR identity file inside the live adopath root."""

    if artifact.exists():
        raise SccError("comparator artifact already exists")
    _write(artifact, _comparator_artifact_bytes(comparator_root, artifact))
    return artifact


def _comparator_artifact_bytes(comparator_root: Path, artifact: Path) -> bytes:
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w", format=tarfile.USTAR_FORMAT) as target:
        for path in sorted(comparator_root.rglob("*")):
            if not path.is_file() or path == artifact:
                continue
            relative = path.relative_to(comparator_root).as_posix()
            info = target.gettarinfo(str(path), arcname=relative)
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            info.mtime = 0
            with path.open("rb") as source:
                target.addfile(info, source)
    return output.getvalue()


def _verify_runtime_bundles(
    run_root: Path, preparation: Mapping[str, Any]
) -> None:
    """Bind the live adopath trees to committed candidate/reference bytes."""

    candidate_root = run_root / "work/bundles/candidate/stata"
    expected_candidate = _runtime_file_inventory(run_root / "work/candidate/stata")
    plugin = run_root / "work/candidate/dist/fereg_linux.plugin"
    expected_candidate["fereg_linux.plugin"] = {
        "sha256": digest_file(plugin),
        "size_bytes": plugin.stat().st_size,
        "executable": bool(plugin.stat().st_mode & 0o111),
    }
    if _runtime_file_inventory(candidate_root) != expected_candidate:
        raise SccError(
            "candidate runtime bundle differs from committed stata/ plus exact plugin"
        )

    comparator_root = run_root / "work/bundles/comparator"
    artifact = comparator_root / "feppml-comparator-bundle.tar"
    expected_comparator: dict[str, dict[str, Any]] = {}
    for name in REFERENCE_NAMES:
        for relative, identity in preparation["references"][name][
            "runtime_files"
        ].items():
            expected_comparator[f"{name}/{relative}"] = dict(identity)
    generated_library = comparator_root / "ftools/src/l/lftools.mlib"
    if generated_library.is_symlink() or not generated_library.is_file():
        raise SccError("comparator runtime omits the controlled ftools library")
    expected_comparator[
        generated_library.relative_to(comparator_root).as_posix()
    ] = {
        "sha256": digest_file(generated_library),
        "size_bytes": generated_library.stat().st_size,
        "executable": False,
    }
    if not artifact.is_file() or artifact.read_bytes() != _comparator_artifact_bytes(
        comparator_root, artifact
    ):
        raise SccError("comparator identity artifact differs from its locked source tree")
    expected_comparator[artifact.relative_to(comparator_root).as_posix()] = {
        "sha256": digest_file(artifact),
        "size_bytes": artifact.stat().st_size,
        "executable": False,
    }
    if _runtime_file_inventory(comparator_root) != expected_comparator:
        raise SccError("comparator runtime contains missing, changed, or shadow files")


def _runtime_identity(
    preparation: Mapping[str, Any],
    development_sha256: str,
    recipe_manifest: Path,
) -> dict[str, Any]:
    return {
        "source_commit": preparation["candidate"]["commit"],
        "source_tree": preparation["candidate"]["tree"],
        "cargo_lock_sha256": preparation["candidate"]["cargo_lock_sha256"],
        "references_lock_sha256": preparation["candidate"]["references_lock_sha256"],
        "manifest_sha256": development_sha256,
        "generator_sha256": digest_file(recipe_manifest),
        "reference_commits": {
            name: preparation["references"][name]["commit"] for name in REFERENCE_NAMES
        },
    }


def _runtime_environment(
    template: Mapping[str, Any],
    comparator_limit: int,
    *,
    stata_flavor: str | None = None,
) -> dict[str, Any]:
    cpu = "unknown"
    cpuinfo = Path("/proc/cpuinfo")
    if cpuinfo.is_file():
        for line in cpuinfo.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.lower().startswith("model name") and ":" in line:
                cpu = line.split(":", 1)[1].strip()
                break
    if cpu == "unknown":
        cpu = os.uname().machine
    try:
        rustc = _run(["rustc", "--version"]).decode("ascii").strip()
    except SccError:
        rustc = str(template.get("rustc_version", "unavailable"))
    return {
        "node_id": socket.gethostname(),
        "stata_version": str(template["stata_version"]),
        "stata_flavor": (
            str(template["stata_flavor"])
            if stata_flavor is None
            else stata_flavor
        ),
        "scheduler_job_id": os.environ.get("JOB_ID", f"manual-{os.getpid()}"),
        "comparator_processor_limit": comparator_limit,
        "os": f"{os.uname().sysname} {os.uname().release}",
        "cpu": cpu,
        "rustc_version": rustc,
    }


def _materialize_capture(
    run_root: Path,
    preparation: Mapping[str, Any],
    bundles: Mapping[str, Any],
    recipe_document: Mapping[str, Any],
    datasets: Mapping[str, Any],
    oracle_inventory: Sequence[Mapping[str, Any]],
    constrained_document: Mapping[str, Any],
    stata: Mapping[str, Any],
) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    """Seal the concrete plan only after all runtime prerequisites exist."""

    work_capture = run_root / "work/capture"
    work_capture.mkdir(parents=True, exist_ok=False)
    request_path = run_root / "input/capture/materialization-request.json"
    dev_path = run_root / "input/manifests/development.json"
    recipes_by_id = {record["cell_id"]: record for record in recipe_document["cells"]}
    oracle_by_id = {record["cell_id"]: record for record in oracle_inventory}
    plugin_path = Path(bundles["candidate_plugin"]["path"])
    comparator_root = Path(bundles["comparator_root"])
    comparator_artifact = _write_comparator_artifact(
        comparator_root, comparator_root / "feppml-comparator-bundle.tar"
    )
    _verify_runtime_bundles(run_root, preparation)
    oracle_program = run_root / "input/harness/scripts/feppml_explicit_dummy_oracle.py"
    oracle_dependencies = [
        run_root / "input/harness/tests/python/ppml_explicit_dummy_oracle.py",
        run_root / "input/harness/tests/python/ppml_multiway_cluster_oracle.py",
    ]
    cells: list[dict[str, Any]] = []
    for plan_cell in development.expand_cells(development.load_manifest(dev_path)):
        cell_id = plan_cell["cell_id"]
        recipe = recipes_by_id[cell_id]
        data = Path(datasets[recipe["dataset_id"]]["path"])
        common = {
            "expected_status_map": recipe["expected_status_map"],
            "expected_output": recipe["expected_output"],
            "probe_rows": recipe["probe_rows"],
            "expected_cluster_dimensions": recipe["expected_cluster_dimensions"],
            "expected_saved_channels": recipe["expected_saved_channels"],
            "science_output": "science.raw",
        }
        candidate = {
            **common,
            "expected_source_provenance": recipe["candidate_source_provenance"],
            "science_do_file": materializer.immutable(
                run_root / "work/recipes" / recipe["candidate_science_do"]
            ),
            "timing_do_file": materializer.immutable(
                run_root / "work/recipes" / recipe["candidate_timing_do"]
            ),
            "artifact": materializer.immutable(plugin_path),
        }
        comparator = {
            **common,
            "expected_source_provenance": recipe["comparator_source_provenance"],
            "science_do_file": materializer.immutable(
                run_root / "work/recipes" / recipe["comparator_science_do"]
            ),
            "timing_do_file": materializer.immutable(
                run_root / "work/recipes" / recipe["comparator_timing_do"]
            ),
            "artifact": materializer.immutable(comparator_artifact),
        }
        oracle_fixture = None
        if cell_id in oracle_by_id:
            fixture = oracle_by_id[cell_id]
            oracle_fixture = {
                "fixture_id": fixture["fixture_id"],
                "data": materializer.immutable(Path(fixture["data"]["path"])),
                "canonical_export": materializer.immutable(
                    Path(fixture["canonical_export"]["path"])
                ),
                "model": materializer.immutable(Path(fixture["model"]["path"])),
                "generator": materializer.immutable(Path(fixture["generator"]["path"])),
                "row_count": fixture["row_count"],
                "independent": {
                    "command": [
                        str(Path(sys.executable).resolve()),
                        "-I",
                        "-B",
                        "-S",
                        str(oracle_program),
                    ],
                    "program": materializer.immutable(oracle_program),
                    "dependencies": [materializer.immutable(path) for path in oracle_dependencies],
                    "output": "independent.json",
                },
            }
        cells.append(
            {
                "cell_id": cell_id,
                "requested_threads": plan_cell["requested_threads"],
                "data": materializer.immutable(data),
                "candidate": candidate,
                "comparator": comparator,
                "oracle_fixture": oracle_fixture,
            }
        )
    constrained = []
    for fixture in constrained_document["fixtures"]:
        source_cell = recipes_by_id[fixture["dataset_cell_id"]]
        data = Path(datasets[source_cell["dataset_id"]]["path"])
        constrained.append(
            {
                "id": fixture["id"],
                "data": materializer.immutable(data),
                "attempts": [
                    {
                        "id": attempt["id"],
                        "do_file": materializer.immutable(
                            run_root / "work/constrained-recipes" / attempt["do_file"]
                        ),
                        "artifact": materializer.immutable(plugin_path),
                        "science_output": attempt["science_output"],
                        "memory_limit_bytes": attempt["memory_limit_bytes"],
                    }
                    for attempt in fixture["attempts"]
                ],
            }
        )
    tools = [run_root / "input/harness" / relative for relative in HARNESS_PATHS]
    plan, payload = materializer.materialize(
        request_path=request_path,
        development_manifest_path=dev_path,
        stata_executable=Path(str(stata["path"])),
        identity=_runtime_identity(
            preparation, preparation["development_manifest_sha256"],
            run_root / "work/recipes/recipes.json",
        ),
        cells=cells,
        constrained_memory=constrained,
        recipe_manifest_path=run_root / "work/recipes/recipes.json",
        recipe_root=run_root / "work/recipes",
        datasets=list(datasets.values()),
        oracle_fixtures=list(oracle_inventory),
        candidate_source=preparation["candidate"],
        candidate_plugin_path=plugin_path,
        candidate_bundle_root=Path(bundles["candidate_stata"]),
        comparator_sources=preparation["references"],
        comparator_bundle_root=comparator_root,
        tools=tools,
    )
    plan_path = work_capture / "plan.json"
    receipt_path = run_root / "receipts/capture-plan.json"
    materializer.write_new(plan_path, plan)
    materializer.write_new(receipt_path, materializer.seal_payload(payload))
    verified = materializer.verify_receipt(plan_path, receipt_path)

    template = dict(_read(run_root / "input/capture/spec-template.json", "capture spec template"))
    candidate_header = dict(template["evidence_header"]["candidate"])
    candidate_header.update(
        {
            "package_sha256": plan["closure"]["candidate_bundle_sha256"],
            "performance_plugin_sha256": plan["closure"]["candidate_plugin_sha256"],
            "performance_plugin_size_bytes": plugin_path.stat().st_size,
            "embedded_git_commit": preparation["candidate"]["commit"][:12],
            "build_date_utc": preparation["candidate"]["build_date_utc"],
        }
    )
    comparator_header = dict(template["evidence_header"]["comparator"])
    comparator_header.update(
        {
            "bundle_sha256": digest_file(comparator_artifact),
            "bundle_size_bytes": comparator_artifact.stat().st_size,
        }
    )
    qualification_manifest = qualification.load_manifest(
        run_root / "input/manifests/qualification.json"
    )
    platform_records = _late_platform_records(
        run_root, preparation, qualification_manifest, candidate_header
    )
    template["evidence_header"] = {
        **dict(template["evidence_header"]),
        "candidate": candidate_header,
        "comparator": comparator_header,
        "platform_artifacts": platform_records,
    }
    program = run_root / "input/harness/scripts/feppml_qualification_capture.py"
    interpreter = Path(sys.executable).resolve()
    completed = subprocess.run(
        [str(interpreter), "--version"], stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, text=True, check=False,
    )
    if completed.returncode != 0:
        raise SccError("capture Python interpreter version is unavailable")
    program_record = {
        "interpreter_path": str(interpreter),
        "interpreter_sha256": digest_file(interpreter),
        "interpreter_version": completed.stdout.strip(),
        "program_path": str(program),
        "program_sha256": digest_file(program),
        "plan_path": str(plan_path),
        "plan_sha256": digest_file(plan_path),
    }
    template["preflight_program"] = program_record
    template["arm_program"] = dict(program_record)
    template["environment"] = _runtime_environment(
        dict(template["environment"]),
        plan["stata"]["comparator_processor_limit"],
        stata_flavor=capture_program.stata_flavor(plan),
    )
    template["spec_type"] = "feppml-qualification-capture-v1"
    template["complete_sentinel"] = "FEPPML_CAPTURE_SPEC_COMPLETE"
    spec_path = work_capture / "spec.json"
    write_json(spec_path, template)
    capture.load_spec(
        spec_path,
        qualification_manifest,
        preparation["qualification_manifest_sha256"],
    )
    return verified, template


def setup(run_root: Path, timeout_seconds: float) -> None:
    controller = _controller_identity("setup", run_root)
    verify_deployment(run_root)
    preparation = verify_input(run_root)
    release_path = run_root / "receipts/submission-release.json"
    release_deadline = time.monotonic() + 30.0
    while not release_path.is_file() and time.monotonic() < release_deadline:
        time.sleep(0.25)
    if not release_path.is_file():
        raise SccError("timed out waiting for sealed scheduler release receipt")
    submission = _verify_submission(run_root, preparation)
    preparation_sha256 = _preparation_sha256(run_root)
    _extract_archive(run_root / "input/archives/candidate.tar", run_root / "work/candidate")
    for name in REFERENCE_NAMES:
        suffix = ".tar.gz" if name == "ppmlhdfe" else ".tar"
        _extract_locked_reference(
            run_root / "input/archives" / f"{name}{suffix}",
            run_root / "work/references" / name,
            name,
            preparation["references"][name],
        )
    bundles = _stage_exact_bundles(run_root, preparation)
    stata = _setup_stata_identity()
    ftools = _compile_private_ftools(run_root, stata)
    manifest_path, dev_path, spec_path = _paths(run_root)
    recipe_root = run_root / "work/recipes"
    recipe_root.mkdir(parents=True, exist_ok=False)
    recipe_document = recipes.compile_recipes(dev_path, recipe_root)
    recipes.validate_compiled_recipes(recipe_document, dev_path, recipe_root)
    datasets, dataset_actions = _materialize_datasets(
        run_root, recipe_document, stata
    )
    oracle_document, oracle_inventory, oracle_actions = _materialize_oracle_fixtures(
        run_root, dev_path, stata
    )
    oracle_recipes.validate_compiled_fixtures(
        oracle_document, dev_path, run_root / "work/oracle-recipes"
    )
    constrained_root = run_root / "work/constrained-recipes"
    constrained_root.mkdir(parents=True, exist_ok=False)
    constrained_document = constrained_recipes.compile_recipes(
        dev_path, constrained_root
    )
    constrained_recipes.validate_compiled_recipes(
        constrained_document, dev_path, constrained_root
    )
    qualification_manifest = qualification.load_manifest(manifest_path)
    canonical_profile = recipe_contract.canonical_profile(dev_path)
    if (
        recipe_contract.profile_digest(canonical_profile)
        != qualification_manifest["recipe_contract"]["profile_sha256"]
    ):
        raise SccError("independent qualification recipe profile changed")
    recipe_contract.validate_documents(
        canonical_profile,
        recipe_document,
        oracle_document,
        constrained_document,
    )
    recipe_contract.validate_generator_sources(
        canonical_profile,
        recipe_document,
        recipe_root,
        oracle_document,
        run_root / "work/oracle-recipes",
    )
    recipe_contract.validate_executable_sources(
        canonical_profile, recipe_document, recipe_root
    )
    recipe_contract.validate_oracle_models(
        canonical_profile,
        oracle_document,
        run_root / "work/oracle-recipes",
    )
    recipe_contract.validate_constrained_sources(
        canonical_profile, constrained_document, constrained_root
    )
    materialization, _late_spec = _materialize_capture(
        run_root,
        preparation,
        bundles,
        recipe_document,
        datasets,
        oracle_inventory,
        constrained_document,
        stata,
    )
    manifest = qualification.load_manifest(manifest_path)
    spec = capture.load_spec(spec_path, manifest, preparation["qualification_manifest_sha256"])
    loaded_plan = capture_program.load_plan(run_root / "work/capture/plan.json")
    sealed_stata_flavor = capture_program.stata_flavor(loaded_plan)
    if spec["environment"]["stata_flavor"] != sealed_stata_flavor:
        raise SccError("capture environment flavor differs from sealed Stata executable")
    output = run_root / "work/preflight/science.json"
    capture._run_program(
        spec["preflight_program"],
        [
            "--qualification-preflight", "--qualification-manifest", str(manifest_path),
            "--development-manifest", str(dev_path),
        ],
        output=output,
        log_prefix=run_root / "logs/preflight",
        timeout_seconds=timeout_seconds,
    )
    benchmark = capture._load_scientific_preflight(
        output,
        manifest,
        preparation["qualification_manifest_sha256"],
        development.load_manifest(dev_path),
        preparation["development_manifest_sha256"],
    )
    loaded_plan = capture_program.load_plan(run_root / "work/capture/plan.json")
    replayed_benchmark = capture_program.replay_preflight(loaded_plan, output)
    if replayed_benchmark != benchmark:
        raise SccError("science preflight differs from retained raw licensed children")
    capture.qualification._validate_benchmark_identity(
        spec["evidence_header"]["candidate"],
        spec["evidence_header"]["comparator"],
        benchmark,
        preparation["development_manifest_sha256"],
    )
    preflight_roots = [
        "work/preflight",
        "logs/preflight.stdout",
        "logs/preflight.stderr",
        "logs/preflight.supervisor.json",
    ]
    preflight_raw_files = _bound_file_inventory(
        run_root, [run_root / value for value in preflight_roots]
    )
    payload = {
        "schema_version": 1,
        "kind": "feppml_qualification_scc_setup",
        "run_id": run_root.name,
        "candidate_source_commit": preparation["candidate"]["commit"],
        "preparation_sha256": preparation_sha256,
        "submission_receipt_sha256": digest_file(
            run_root / "receipts/submission.json"
        ),
        "submission_release_receipt_sha256": digest_file(
            run_root / "receipts/submission-release.json"
        ),
        "capture_plan_sha256": materialization["plan_file_sha256"],
        "capture_spec_sha256": digest_file(spec_path),
        "capture_materialization_receipt_sha256": digest_file(
            run_root / "receipts/capture-plan.json"
        ),
        "preflight_sha256": digest_file(output),
        "preflight_raw_roots": preflight_roots,
        "preflight_raw_files": preflight_raw_files,
        "science_before_timing": True,
        "timed_arm_processes_started": 0,
        "job_id": os.environ.get("JOB_ID", f"manual-{os.getpid()}"),
        "node_id": socket.gethostname(),
        "controller": controller,
        "bundles": bundles,
        "datasets": datasets,
        "oracle_fixtures": oracle_inventory,
        "setup_stata": {
            "identity": stata,
            "datasets": dataset_actions,
            "oracles": oracle_actions,
            "ftools": ftools,
        },
        "complete_sentinel": "FEPPML_SCC_SETUP_COMPLETE",
    }
    write_sealed(run_root / "receipts/setup.json", payload)
    _write(
        run_root / "receipts/SETUP.SUCCESS",
        (
            "FEPPML_SCC_SETUP_COMPLETE|"
            f"{preparation_sha256}|{materialization['plan_file_sha256']}|"
            f"{digest_file(spec_path)}\n"
        ).encode("ascii"),
    )


def _portable_plan_resolver(run_root: Path):
    document = _read(run_root / "work/capture/plan.json", "capture plan")
    development_record = document.get("development_manifest")
    if not isinstance(development_record, Mapping):
        raise SccError("capture plan omits its recorded SCC run root")
    recorded = Path(str(development_record.get("path", "")))
    suffix = Path("input/manifests/development.json")
    if (
        not recorded.is_absolute()
        or len(recorded.parts) <= len(suffix.parts)
        or recorded.parts[-len(suffix.parts):] != suffix.parts
    ):
        raise SccError("capture plan development path is not run-scoped")
    recorded_root = recorded.parents[2]
    if recorded_root.name != run_root.name:
        raise SccError("capture plan recorded run ID differs from collected run")
    local_root = run_root.resolve()

    def relocate(path: Path) -> Path:
        try:
            relative = Path(path).relative_to(recorded_root)
        except ValueError as error:
            raise capture_program.CaptureFailure(
                f"capture plan path is outside the sealed SCC run: {path}"
            ) from error
        local = (local_root / relative).resolve()
        try:
            local.relative_to(local_root)
        except ValueError as error:
            raise capture_program.CaptureFailure(
                "relocated capture plan path escapes collected run"
            ) from error
        return local

    def logical(path: Path) -> Path:
        try:
            relative = Path(path).resolve().relative_to(local_root)
        except ValueError as error:
            raise capture_program.CaptureFailure(
                f"portable raw path is outside the collected run: {path}"
            ) from error
        return recorded_root / relative

    return relocate, logical


def _load_capture_plan(
    run_root: Path, *, portable: bool
) -> Mapping[str, Any]:
    plan_path = run_root / "work/capture/plan.json"
    if not portable:
        return capture_program.load_plan(plan_path)
    spec = _read(run_root / "work/capture/spec.json", "capture spec")
    program = spec.get("preflight_program")
    if not isinstance(program, Mapping):
        raise SccError("portable capture spec omits sealed interpreter provenance")
    interpreter = program.get("interpreter_path")
    if not isinstance(interpreter, str) or not interpreter:
        raise SccError("portable capture interpreter provenance is invalid")
    try:
        path_resolver, logical_path_resolver = _portable_plan_resolver(run_root)
        return capture_program.load_plan(
            plan_path,
            path_resolver=path_resolver,
            verify_stata_executable=False,
            oracle_interpreter=interpreter,
            logical_path_resolver=logical_path_resolver,
        )
    except capture_program.CaptureFailure as error:
        raise SccError(f"portable capture plan failed: {error}") from error


def _load_capture_spec(
    run_root: Path,
    manifest: Mapping[str, Any],
    preparation: Mapping[str, Any],
    *,
    portable: bool,
) -> Mapping[str, Any]:
    try:
        return capture.load_spec(
            run_root / "work/capture/spec.json",
            manifest,
            preparation["qualification_manifest_sha256"],
            portable_run_root=run_root if portable else None,
        )
    except capture.CaptureError as error:
        raise SccError(f"capture spec failed: {error}") from error


def _verify_materialized_capture(
    run_root: Path,
    preparation: Mapping[str, Any],
    *,
    portable: bool = False,
) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    preparation_sha256 = digest_file(run_root / "input/preparation.json")
    plan_path = run_root / "work/capture/plan.json"
    receipt_path = run_root / "receipts/capture-plan.json"
    spec_path = run_root / "work/capture/spec.json"
    try:
        materialization = (
            materializer.verify_receipt_portable(
                plan_path, receipt_path, run_root
            )
            if portable
            else materializer.verify_receipt(plan_path, receipt_path)
        )
    except materializer.MaterializationError as error:
        raise SccError(f"capture materialization receipt failed: {error}") from error
    _verify_runtime_bundles(run_root, preparation)
    setup_receipt = read_sealed(
        run_root / "receipts/setup.json", "feppml_qualification_scc_setup"
    )
    setup_controller = _verify_controller_identity(
        run_root,
        setup_receipt.get("controller"),
        "setup",
        portable=portable,
    )
    submission = _verify_submission(
        run_root, preparation, portable=portable
    )
    bundles = setup_receipt.get("bundles")
    if not isinstance(bundles, Mapping):
        raise SccError("setup receipt omits the formal runtime bundle closure")
    _verify_build_receipt(
        run_root, preparation, bundles.get("build"), portable=portable
    )
    _verify_setup_stata_closure(
        run_root, setup_receipt, portable=portable
    )
    setup_stata = setup_receipt.get("setup_stata", {}).get("identity", {})
    setup_environment = setup_controller["environment"]
    expected_setup_path = (
        f"{Path(setup_controller['executable']['path']).parent}:"
        f"{Path(str(setup_stata.get('path', ''))).parent}:/usr/bin:/bin"
    )
    if (
        setup_environment["JOB_ID"] != submission["setup_job"]
        or setup_environment["FEPPML_STATA_EXECUTABLE"]
            != setup_stata.get("path")
        or setup_environment["PATH"] != expected_setup_path
    ):
        raise SccError(
            "setup controller environment differs from submission/Stata authority"
        )
    expected_marker = (
        "FEPPML_SCC_SETUP_COMPLETE|"
        f"{preparation_sha256}|{materialization['plan_file_sha256']}|"
        f"{digest_file(spec_path)}\n"
    )
    preflight_path = run_root / "work/preflight/science.json"
    if (
        setup_receipt.get("preparation_sha256") != preparation_sha256
        or setup_receipt.get("submission_receipt_sha256")
        != digest_file(run_root / "receipts/submission.json")
        or setup_receipt.get("submission_release_receipt_sha256")
        != digest_file(run_root / "receipts/submission-release.json")
        or setup_receipt.get("capture_plan_sha256")
        != materialization["plan_file_sha256"]
        or setup_receipt.get("capture_spec_sha256") != digest_file(spec_path)
        or setup_receipt.get("capture_materialization_receipt_sha256")
        != digest_file(receipt_path)
        or (run_root / "receipts/SETUP.SUCCESS").read_text(encoding="ascii")
        != expected_marker
        or setup_receipt.get("candidate_source_commit")
        != preparation["candidate"]["commit"]
        or setup_receipt.get("preflight_sha256") != digest_file(preflight_path)
    ):
        raise SccError("setup/materialization receipt binding is invalid")
    _verify_bound_file_inventory(
        run_root,
        setup_receipt.get("preflight_raw_roots"),
        setup_receipt.get("preflight_raw_files"),
    )
    loaded_plan = _load_capture_plan(run_root, portable=portable)
    manifest = qualification.load_manifest(
        run_root / "input/manifests/qualification.json"
    )
    spec = _load_capture_spec(
        run_root, manifest, preparation, portable=portable
    )
    controller_executable = setup_controller["executable"]
    if (
        spec["preflight_program"]["interpreter_path"]
            != controller_executable["path"]
        or spec["preflight_program"]["interpreter_sha256"]
            != controller_executable["sha256"]
        or spec["preflight_program"]["interpreter_version"]
            != controller_executable["version"]
    ):
        raise SccError("capture program used a different controller Python")
    capture.verify_program_supervisor(
        spec["preflight_program"],
        [
            "--qualification-preflight",
            "--qualification-manifest",
            str(run_root / "input/manifests/qualification.json"),
            "--development-manifest",
            str(run_root / "input/manifests/development.json"),
        ],
        output=preflight_path,
        log_prefix=run_root / "logs/preflight",
        portable_run_root=run_root if portable else None,
    )
    if spec["environment"]["stata_flavor"] != capture_program.stata_flavor(
        loaded_plan
    ):
        raise SccError("capture environment flavor differs from sealed Stata executable")
    replayed = capture_program.replay_preflight(loaded_plan, preflight_path)
    verified = capture._load_scientific_preflight(
        preflight_path,
        manifest,
        preparation["qualification_manifest_sha256"],
        development.load_manifest(run_root / "input/manifests/development.json"),
        preparation["development_manifest_sha256"],
    )
    if replayed != verified:
        raise SccError("setup preflight cannot be reproduced from raw children")
    return materialization, setup_receipt


def _task_cell(run_root: Path, task_id: int) -> str:
    rows = (run_root / "input/tasks.tsv").read_text(encoding="ascii").splitlines()[1:]
    mapping = {int(row.split("\t")[0]): row.split("\t")[1] for row in rows}
    if task_id not in mapping:
        raise SccError("SGE task ID is outside the frozen inventory")
    return mapping[task_id]


def task(run_root: Path, task_id: int, timeout_seconds: float) -> None:
    controller = _controller_identity("task", run_root)
    verify_deployment(run_root)
    preparation = verify_input(run_root)
    materialization, _setup = _verify_materialized_capture(run_root, preparation)
    preparation_sha256 = digest_file(run_root / "input/preparation.json")
    cell_id = _task_cell(run_root, task_id)
    manifest_path, dev_path, spec_path = _paths(run_root)
    manifest = qualification.load_manifest(manifest_path)
    dev_manifest = development.load_manifest(dev_path)
    spec = capture.load_spec(spec_path, manifest, preparation["qualification_manifest_sha256"])
    loaded_plan = capture_program.load_plan(run_root / "work/capture/plan.json")
    sealed_stata_flavor = capture_program.stata_flavor(loaded_plan)
    if spec["environment"]["stata_flavor"] != sealed_stata_flavor:
        raise SccError("capture environment flavor differs from sealed Stata executable")
    preflight = _read(run_root / "work/preflight/science.json", "science preflight")
    benchmark = copy.deepcopy(preflight["benchmark_evidence"])
    plans = {item["cell_id"]: item for item in development.expand_cells(dev_manifest)}
    cells = {item["cell_id"]: item for item in benchmark["cells"]}
    plan = plans[cell_id]
    cell = cells[cell_id]
    identity = benchmark["identity"]
    environment = _runtime_environment(
        spec["environment"],
        spec["environment"]["comparator_processor_limit"],
        stata_flavor=sealed_stata_flavor,
    )
    orders = ["AB", *manifest["capture_contract"]["warm_pair_order"]]
    pairs = []
    arm_orders = []
    candidate_effective = []
    comparator_processors = []
    for index, order in enumerate(orders):
        pair, arm_order, effective, processors = capture._capture_pair(
            run_root=run_root / "work/tasks" / f"task-{task_id:03d}",
            program=spec["arm_program"], cell_id=cell_id,
            requested_threads=plan["requested_threads"], index=index, order=order,
            candidate_artifact=spec["evidence_header"]["candidate"]["performance_plugin_sha256"],
            comparator_artifact=spec["evidence_header"]["comparator"]["bundle_sha256"],
            environment=environment, timeout_seconds=timeout_seconds,
        )
        pairs.append(pair)
        arm_orders.append(arm_order)
        candidate_effective.append(effective)
        comparator_processors.append(processors)
    if len(set(candidate_effective)) != 1 or len(set(comparator_processors)) != 1:
        raise SccError("thread controls changed across one task's pairs")
    cold, warm = pairs[0], pairs[1:]
    cell["timing"] = {
        arm: {
            "cold_seconds": cold[arm]["seconds"],
            "warm_seconds": [pair[arm]["seconds"] for pair in warm],
        }
        for arm in ("candidate", "comparator")
    }
    cell["execution"] = {
        "scheduler_job_id": environment["scheduler_job_id"],
        "node_id": environment["node_id"],
        "plugin_sha256": spec["evidence_header"]["candidate"]["performance_plugin_sha256"],
        "comparator_bundle_sha256": spec["evidence_header"]["comparator"]["bundle_sha256"],
        "os": environment["os"], "cpu": environment["cpu"],
        "stata_version": environment["stata_version"],
        "stata_flavor": environment["stata_flavor"],
        "rustc_version": environment["rustc_version"],
        "candidate_process_ids": [pair["candidate"]["process_id"] for pair in pairs],
        "comparator_process_ids": [pair["comparator"]["process_id"] for pair in pairs],
        "arm_order": arm_orders, "requested_threads": plan["requested_threads"],
        "candidate_effective_threads": candidate_effective[0],
        "comparator_stata_processors": comparator_processors[0],
        "comparator_processor_limit": environment["comparator_processor_limit"],
    }
    cell["memory"]["process_peak_rss_bytes"] = max(
        pair["candidate"]["peak_rss_bytes"] for pair in pairs
    )
    paired = {"cell_id": cell_id, "cold_pair": cold, "warm_pairs": warm}
    task_root = run_root / "work/tasks" / f"task-{task_id:03d}"
    raw_roots = [task_root.relative_to(run_root).as_posix()]
    raw_files = _bound_file_inventory(run_root, [task_root])
    payload = {
        "schema_version": 1, "kind": "feppml_qualification_cell_shard",
        "run_id": run_root.name,
        "qualification_manifest_sha256": preparation["qualification_manifest_sha256"],
        "candidate_source_commit": preparation["candidate"]["commit"],
        "preparation_sha256": preparation_sha256,
        "capture_plan_sha256": materialization["plan_file_sha256"],
        "capture_spec_sha256": digest_file(spec_path),
        "capture_materialization_receipt_sha256": digest_file(
            run_root / "receipts/capture-plan.json"
        ),
        "task_id": task_id, "cell_id": cell_id,
        "benchmark_identity": identity, "benchmark_cell": cell, "paired_cell": paired,
        "raw_roots": raw_roots, "raw_files": raw_files,
        "task_environment": environment,
        "job_id": environment["scheduler_job_id"], "node_id": environment["node_id"],
        "licensed_stata_version": environment["stata_version"],
        "controller": controller,
        "complete_sentinel": "FEPPML_SCC_CELL_COMPLETE",
    }
    receipt = run_root / "receipts/tasks" / f"task-{task_id:03d}.json"
    write_sealed(receipt, payload)
    _write(
        receipt.with_suffix(".SUCCESS"),
        f"FEPPML_SCC_CELL_COMPLETE|{digest_file(receipt)}\n".encode("ascii"),
    )


SHARD_FIELDS = {
    "schema_version", "kind", "run_id", "qualification_manifest_sha256",
    "candidate_source_commit", "preparation_sha256", "capture_plan_sha256",
    "capture_spec_sha256", "capture_materialization_receipt_sha256", "task_id",
    "cell_id", "benchmark_identity", "benchmark_cell", "paired_cell", "raw_roots",
    "raw_files", "task_environment", "job_id", "node_id", "licensed_stata_version",
    "controller",
    "complete_sentinel",
}


def _replay_task_shard(
    run_root: Path,
    *,
    task_id: int,
    cell_id: str,
    preparation: Mapping[str, Any],
    materialization: Mapping[str, Any],
    manifest: Mapping[str, Any],
    spec: Mapping[str, Any],
    benchmark_identity: Mapping[str, Any],
    preflight_cell: Mapping[str, Any],
    loaded_plan: Mapping[str, Any],
    portable: bool,
) -> tuple[dict[str, Any], dict[str, Any], Mapping[str, Any]]:
    receipt = run_root / "receipts/tasks" / f"task-{task_id:03d}.json"
    payload = read_sealed(receipt, "feppml_qualification_cell_shard")
    preparation_sha256 = digest_file(run_root / "input/preparation.json")
    spec_sha256 = digest_file(run_root / "work/capture/spec.json")
    materialization_receipt_sha256 = digest_file(run_root / "receipts/capture-plan.json")
    if (
        set(payload) != SHARD_FIELDS
        or payload.get("schema_version") != 1
        or payload.get("run_id") != run_root.name
        or payload.get("task_id") != task_id
        or payload.get("cell_id") != cell_id
        or payload.get("qualification_manifest_sha256")
            != preparation["qualification_manifest_sha256"]
        or payload.get("candidate_source_commit") != preparation["candidate"]["commit"]
        or payload.get("preparation_sha256") != preparation_sha256
        or payload.get("capture_plan_sha256") != materialization["plan_file_sha256"]
        or payload.get("capture_spec_sha256") != spec_sha256
        or payload.get("capture_materialization_receipt_sha256")
            != materialization_receipt_sha256
        or payload.get("benchmark_identity") != benchmark_identity
        or payload.get("complete_sentinel") != "FEPPML_SCC_CELL_COMPLETE"
        or (receipt.with_suffix(".SUCCESS")).read_text(encoding="ascii")
            != f"FEPPML_SCC_CELL_COMPLETE|{digest_file(receipt)}\n"
    ):
        raise SccError(f"stale or misbound task shard: {task_id}")
    _verify_bound_file_inventory(
        run_root, payload["raw_roots"], payload["raw_files"]
    )
    task_controller = _verify_controller_identity(
        run_root, payload.get("controller"), "task", portable=portable
    )
    environment = payload["task_environment"]
    if not isinstance(environment, Mapping) or set(environment) != {
        "node_id", "stata_version", "stata_flavor", "scheduler_job_id",
        "comparator_processor_limit", "os", "cpu", "rustc_version",
    }:
        raise SccError(f"task environment is not closed: {task_id}")
    if (
        environment["scheduler_job_id"] != payload["job_id"]
        or environment["node_id"] != payload["node_id"]
        or environment["stata_version"] != payload["licensed_stata_version"]
        or environment["comparator_processor_limit"]
            != spec["environment"]["comparator_processor_limit"]
        or environment["stata_flavor"] != capture_program.stata_flavor(loaded_plan)
        or task_controller["environment"]["JOB_ID"] != payload["job_id"]
        or int(task_controller["environment"]["SGE_TASK_ID"]) != task_id
        or task_controller["executable"]["path"]
            != spec["arm_program"]["interpreter_path"]
        or task_controller["executable"]["sha256"]
            != spec["arm_program"]["interpreter_sha256"]
        or task_controller["environment"]["PATH"]
            != (
                f"{Path(task_controller['executable']['path']).parent}:"
                f"{Path(str(loaded_plan['stata']['executable'])).parent}:"
                "/usr/bin:/bin"
            )
    ):
        raise SccError(f"task environment differs from scheduler/capture: {task_id}")
    cell_plans = {cell["cell_id"]: cell for cell in loaded_plan["cells"]}
    if cell_id not in cell_plans:
        raise SccError(f"capture plan omits task cell: {cell_id}")
    cell_plan = cell_plans[cell_id]
    requested_threads = cell_plan["requested_threads"]
    orders = ["AB", *manifest["capture_contract"]["warm_pair_order"]]
    if len(orders) != 10:
        raise SccError("qualification replay requires exactly ten pairs per cell")
    pairs: list[dict[str, Any]] = []
    arm_orders: list[list[str]] = []
    candidate_effective: list[int] = []
    comparator_processors: list[int] = []
    task_root = run_root / "work/tasks" / f"task-{task_id:03d}"
    for index, order in enumerate(orders):
        temperature = "cold" if index == 0 else "warm"
        sequence = ["candidate", "comparator"] if order == "AB" else ["comparator", "candidate"]
        measurements: dict[str, Any] = {}
        for arm in sequence:
            arm_root = task_root / "pairs" / cell_id / f"{index:02d}-{order}" / arm
            capture.verify_program_supervisor(
                spec["arm_program"],
                [
                    "--qualification-arm", "--cell-id", cell_id,
                    "--arm", arm, "--pair-index", str(index),
                    "--temperature", temperature,
                    "--requested-threads", str(requested_threads),
                ],
                output=arm_root / "measurement.json",
                log_prefix=arm_root / "capture",
                portable_run_root=run_root if portable else None,
            )
            for log_name in ("capture.stdout", "capture.stderr"):
                if (arm_root / log_name).read_bytes() != b"":
                    raise SccError(f"timed capture emitted unexpected outer output: {task_id}")
            artifact = (
                spec["evidence_header"]["candidate"]["performance_plugin_sha256"]
                if arm == "candidate"
                else spec["evidence_header"]["comparator"]["bundle_sha256"]
            )
            measurement, effective, processors = capture._arm_measurement(
                arm_root / "measurement.json",
                cell_id=cell_id, arm=arm, pair_index=index,
                temperature=temperature, requested_threads=requested_threads,
                artifact_sha256=artifact, environment=environment,
            )
            child, child_pid, seconds, peak = capture_program.replay_stata_child(
                loaded_plan, cell_plan, arm, arm_root / "licensed-child",
                requested_threads=requested_threads, pair_index=index,
                temperature=temperature,
            )
            if not isinstance(child, Mapping) or set(child) != {
                "control", "stata", "complete_sentinel"
            } or child["complete_sentinel"] != "FEPPML_STATA_ARM_COMPLETE":
                raise SccError(f"timed raw child result is invalid: task {task_id} {arm}")
            control = child["control"]
            stata = child["stata"]
            if (
                not isinstance(control, Mapping)
                or set(control) != {"effective_threads", "stata_processors"}
                or not isinstance(stata, Mapping)
                or set(stata) != {"licensed", "version"}
                or stata["licensed"] is not True
            ):
                raise SccError(f"timed raw child control/Stata identity is invalid: {task_id}")
            expected_measurement = {
                "seconds": seconds,
                "peak_rss_bytes": peak,
                "process_id": child_pid,
                "node_id": environment["node_id"],
                "stata_version": stata["version"],
                "artifact_sha256": artifact,
                "fresh_process": True,
            }
            if measurement != expected_measurement:
                raise SccError(f"arm measurement differs from raw supervisor: {task_id}")
            if effective != control["effective_threads"] or processors != control["stata_processors"]:
                raise SccError(f"arm thread control differs from raw child: {task_id}")
            measurements[arm] = measurement
            if arm == "candidate":
                candidate_effective.append(effective)
            else:
                comparator_processors.append(processors)
        pairs.append(
            {
                "index": index,
                "order": order,
                "candidate": measurements["candidate"],
                "comparator": measurements["comparator"],
            }
        )
        arm_orders.append(sequence)
    if (
        len(set(candidate_effective)) != 1
        or len(set(comparator_processors)) != 1
        or len({pair[arm]["process_id"] for pair in pairs for arm in ("candidate", "comparator")})
            != 20
    ):
        raise SccError(f"task fresh-process/thread inventory is invalid: {task_id}")
    cold, warm = pairs[0], pairs[1:]
    cell = copy.deepcopy(dict(preflight_cell))
    cell["timing"] = {
        arm: {
            "cold_seconds": cold[arm]["seconds"],
            "warm_seconds": [pair[arm]["seconds"] for pair in warm],
        }
        for arm in ("candidate", "comparator")
    }
    cell["execution"] = {
        "scheduler_job_id": environment["scheduler_job_id"],
        "node_id": environment["node_id"],
        "plugin_sha256": spec["evidence_header"]["candidate"]["performance_plugin_sha256"],
        "comparator_bundle_sha256": spec["evidence_header"]["comparator"]["bundle_sha256"],
        "os": environment["os"], "cpu": environment["cpu"],
        "stata_version": environment["stata_version"],
        "stata_flavor": environment["stata_flavor"],
        "rustc_version": environment["rustc_version"],
        "candidate_process_ids": [pair["candidate"]["process_id"] for pair in pairs],
        "comparator_process_ids": [pair["comparator"]["process_id"] for pair in pairs],
        "arm_order": arm_orders, "requested_threads": requested_threads,
        "candidate_effective_threads": candidate_effective[0],
        "comparator_stata_processors": comparator_processors[0],
        "comparator_processor_limit": environment["comparator_processor_limit"],
    }
    cell["memory"]["process_peak_rss_bytes"] = max(
        pair["candidate"]["peak_rss_bytes"] for pair in pairs
    )
    paired = {"cell_id": cell_id, "cold_pair": cold, "warm_pairs": warm}
    if payload["benchmark_cell"] != cell or payload["paired_cell"] != paired:
        raise SccError(f"task derived values differ from retained raw files: {task_id}")
    return cell, paired, payload


def assemble_collection(
    run_root: Path,
    preparation: Mapping[str, Any] | None = None,
    materialization: Mapping[str, Any] | None = None,
    *,
    portable: bool = False,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Purely replay preflight and all 33 raw shards into final evidence."""

    run_root = _run_root(run_root)
    preparation = preparation or verify_input(run_root)
    if materialization is None:
        materialization, _setup = _verify_materialized_capture(
            run_root, preparation, portable=portable
        )
    manifest_path, _dev_path, spec_path = _paths(run_root)
    manifest = qualification.load_manifest(manifest_path)
    spec = _load_capture_spec(
        run_root, manifest, preparation, portable=portable
    )
    loaded_plan = _load_capture_plan(run_root, portable=portable)
    preflight_path = run_root / "work/preflight/science.json"
    replayed_preflight = capture_program.replay_preflight(loaded_plan, preflight_path)
    preflight = _read(preflight_path, "science preflight")
    if preflight.get("benchmark_evidence") != replayed_preflight:
        raise SccError("preflight evidence is not a raw replay")
    benchmark = copy.deepcopy(replayed_preflight)
    expected_ids = manifest["matrix_contract"]["expected_cell_ids"]
    if len(expected_ids) != 33 or int(preparation["task_count"]) != 33:
        raise SccError("qualification replay does not have exactly 33 cells")
    task_rows = [
        line.split("\t", 1)[1]
        for line in (run_root / "input/tasks.tsv").read_text(encoding="ascii").splitlines()[1:]
    ]
    if task_rows != expected_ids:
        raise SccError("tasks.tsv differs from the exact qualification cell order")
    receipt_root = run_root / "receipts/tasks"
    expected_receipts = {
        f"task-{task_id:03d}{suffix}"
        for task_id in range(1, 34)
        for suffix in (".json", ".SUCCESS")
    }
    observed_receipts = {
        path.name for path in receipt_root.iterdir() if path.is_file() and not path.is_symlink()
    } if receipt_root.is_dir() else set()
    if observed_receipts != expected_receipts:
        raise SccError("task receipt inventory is not exactly 33 sealed shards/markers")
    preflight_cells = {cell["cell_id"]: cell for cell in benchmark["cells"]}
    cells: dict[str, Any] = {}
    pairs: dict[str, Any] = {}
    shard_files: dict[str, Any] = {}
    raw_files: dict[str, Any] = {}
    for task_id, cell_id in enumerate(expected_ids, 1):
        cell, paired, shard = _replay_task_shard(
            run_root, task_id=task_id, cell_id=cell_id,
            preparation=preparation, materialization=materialization,
            manifest=manifest, spec=spec, benchmark_identity=benchmark["identity"],
            preflight_cell=preflight_cells[cell_id], loaded_plan=loaded_plan,
            portable=portable,
        )
        cells[cell_id] = cell
        pairs[cell_id] = paired
        for suffix in (".json", ".SUCCESS"):
            path = receipt_root / f"task-{task_id:03d}{suffix}"
            shard_files[path.relative_to(run_root).as_posix()] = {
                "sha256": digest_file(path), "size_bytes": path.stat().st_size,
            }
        for relative, identity in shard["raw_files"].items():
            if relative in raw_files:
                raise SccError("raw task file is claimed by multiple shards")
            raw_files[relative] = identity
    benchmark["cells"] = [cells[cell_id] for cell_id in expected_ids]
    evidence_header = copy.deepcopy(dict(spec["evidence_header"]))
    evidence_header["oracles"] = qualification.oracle_evidence_records(manifest, benchmark)
    evidence = {
        "schema_version": 1,
        "evidence_type": "feppml-qualification-v1-evidence",
        "qualification_manifest": {
            "id": manifest["manifest_id"],
            "sha256": preparation["qualification_manifest_sha256"],
        },
        **evidence_header,
        "paired_cells": [pairs[cell_id] for cell_id in expected_ids],
        "benchmark_evidence": benchmark,
        "complete_sentinel": "FEPPML_QUALIFICATION_COMPLETE",
    }
    replay = {
        "preflight_sha256": digest_file(preflight_path),
        "shard_inventory_sha256": digest_bytes(canonical(shard_files)),
        "raw_inventory_sha256": digest_bytes(canonical(raw_files)),
        "task_count": 33,
        "pair_count": 330,
        "arm_count": 660,
    }
    return evidence, replay


def collect(run_root: Path) -> Mapping[str, Any]:
    controller = _controller_identity("collector", run_root)
    verify_deployment(run_root)
    preparation = verify_input(run_root)
    materialization, _setup = _verify_materialized_capture(run_root, preparation)
    preparation_sha256 = digest_file(run_root / "input/preparation.json")
    materialization_receipt_sha256 = digest_file(run_root / "receipts/capture-plan.json")
    manifest_path, _dev_path, spec_path = _paths(run_root)
    manifest = qualification.load_manifest(manifest_path)
    evidence, replay = assemble_collection(run_root, preparation, materialization)
    evidence_path = run_root / "output/evidence.json"
    capture._write_new(evidence_path, evidence)
    result = qualification.evaluate_evidence(
        manifest, evidence, manifest_sha256=preparation["qualification_manifest_sha256"]
    )
    qualification.write_receipt(run_root / "receipts/qualification.json", result)
    payload = {
        "schema_version": 1, "kind": "feppml_qualification_scc_collection",
        "run_id": run_root.name, "task_count": replay["task_count"],
        "preparation_sha256": preparation_sha256,
        "capture_plan_sha256": materialization["plan_file_sha256"],
        "capture_spec_sha256": digest_file(spec_path),
        "capture_materialization_receipt_sha256": materialization_receipt_sha256,
        "evidence_sha256": digest_file(evidence_path),
        "qualification_receipt_sha256": digest_file(run_root / "receipts/qualification.json"),
        **replay,
        "status": result["status"], "stage": result["stage"],
        "job_id": os.environ.get("JOB_ID", f"manual-{os.getpid()}"),
        "node_id": socket.gethostname(),
        "controller": controller,
        "complete_sentinel": "FEPPML_SCC_COLLECTION_COMPLETE",
    }
    write_sealed(run_root / "receipts/collector.json", payload)
    _write(run_root / "receipts/COLLECTOR.COMPLETE", b"FEPPML_SCC_COLLECTION_COMPLETE\n")
    return result


def record_deployment(args: argparse.Namespace) -> None:
    """Preserve the local preparation digest independently after transfer."""

    controller = _controller_identity("deployment", args.run_root)
    verify_input(args.run_root)
    if re.fullmatch(r"[0-9a-f]{64}", args.preparation_sha256) is None:
        raise SccError("deployment preparation SHA-256 is malformed")
    observed = digest_file(args.run_root / "input/preparation.json")
    if args.preparation_sha256 != observed:
        raise SccError("deployed preparation differs from the local transfer digest")
    write_sealed(
        args.run_root / "receipts/deployment.json",
        {
            "schema_version": 1,
            "kind": "feppml_qualification_scc_deployment",
            "run_id": args.run_root.name,
            "preparation_sha256": observed,
            "controller": controller,
            "complete_sentinel": "FEPPML_SCC_DEPLOYMENT_COMPLETE",
        },
    )


def verify_deployment(run_root: Path) -> Mapping[str, Any]:
    verify_input(run_root)
    preparation_sha256 = digest_file(run_root / "input/preparation.json")
    deployment = read_sealed(
        run_root / "receipts/deployment.json",
        "feppml_qualification_scc_deployment",
    )
    controller = deployment.get("controller")
    environment = (
        controller.get("environment")
        if isinstance(controller, Mapping)
        else None
    )
    recorded_root = (
        Path(str(environment.get("RUN_ROOT", "")))
        if isinstance(environment, Mapping)
        else Path("")
    )
    verified_controller = _verify_controller_identity(
        run_root,
        controller,
        "deployment",
        portable=recorded_root != run_root,
    )
    if (
        deployment.get("run_id") != run_root.name
        or deployment.get("preparation_sha256") != preparation_sha256
        or verified_controller.get("role") != "deployment"
        or deployment.get("complete_sentinel")
            != "FEPPML_SCC_DEPLOYMENT_COMPLETE"
    ):
        raise SccError("deployment receipt is stale or bound to another preparation")
    return deployment


def _scheduler_environment(
    run_root: Path, clients: Mapping[str, Mapping[str, Any]]
) -> dict[str, str]:
    directories: list[str] = []
    for role in ("qsub", "qstat", "qacct", "qrls"):
        parent = str(Path(str(clients[role]["path"])).parent)
        if parent not in directories:
            directories.append(parent)
    return {
        "HOME": str(run_root / "work/submission/home"),
        "TMPDIR": str(run_root / "work/submission/tmp"),
        "PATH": ":".join([*directories, "/usr/bin", "/bin"]),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "USER": "johannes",
        "LOGNAME": "johannes",
        "SGE_ROOT": "/usr/local/sge/sge_root",
        "SGE_CELL": "default",
    }


def _scheduler_clients(run_root: Path) -> dict[str, dict[str, Any]]:
    records: dict[str, dict[str, Any]] = {}
    for role, variable in SCHEDULER_CLIENT_ENV.items():
        raw = os.environ.get(variable, "")
        path = Path(raw)
        resolved = path.resolve()
        expected = SCHEDULER_CLIENT_CONTRACT[role]
        if (
            not raw
            or not path.is_absolute()
            or str(path) != expected["path"]
            or not path.is_file()
            or not os.access(path, os.X_OK)
            or not resolved.is_file()
            or str(resolved) != expected["resolved_path"]
            or digest_file(resolved) != expected["sha256"]
            or resolved.stat().st_size != expected["size_bytes"]
        ):
            raise SccError(f"scheduler client identity is invalid: {role}")
        records[role] = {
            "path": str(path),
            "resolved_path": str(resolved),
            "sha256": digest_file(resolved),
            "size_bytes": resolved.stat().st_size,
            "version": expected["version"],
        }
    environment = _scheduler_environment(run_root, records)
    for role, record in records.items():
        completed = subprocess.run(
            [record["path"], "-help"],
            cwd=run_root,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            check=False,
        )
        first_line = completed.stdout.splitlines()[0] if completed.stdout else ""
        if completed.returncode != 0 or first_line != record["version"]:
            raise SccError(f"scheduler client version differs: {role}")
    return records


def _verify_scheduler_clients(
    clients: Any, *, portable: bool
) -> Mapping[str, Mapping[str, Any]]:
    if not isinstance(clients, Mapping) or set(clients) != set(
        SCHEDULER_CLIENT_ENV
    ):
        raise SccError("scheduler client inventory is not closed")
    for role, raw in clients.items():
        record = raw if isinstance(raw, Mapping) else {}
        expected = SCHEDULER_CLIENT_CONTRACT[role]
        if set(record) != {
            "path", "resolved_path", "sha256", "size_bytes", "version"
        } or (
            record != expected
            or re.fullmatch(r"[0-9a-f]{64}", str(record.get("sha256", "")))
                is None
            or not isinstance(record.get("size_bytes"), int)
            or record.get("size_bytes", 0) <= 0
        ):
            raise SccError(f"scheduler client record is invalid: {role}")
        if not portable:
            path = Path(str(expected["path"]))
            resolved = path.resolve()
            if (
                not path.is_file()
                or str(resolved) != expected["resolved_path"]
                or digest_file(resolved) != expected["sha256"]
                or resolved.stat().st_size != expected["size_bytes"]
            ):
                raise SccError(f"scheduler client bytes changed: {role}")
    return clients


def _qsub_argv(
    run_root: Path,
    qsub: str,
    *,
    role: str,
    slots: int,
    tasks: int,
    script: Path,
    hold: str | None,
) -> list[str]:
    wall = "00:30:00" if role == "collector" else "11:45:00"
    argv = [
        qsub,
        "-clear",
        "-terse",
        "-w",
        "e",
        "-P",
        "welfgr",
        "-N",
        _job_name(run_root, role),
        "-wd",
        str(run_root),
        "-o",
        str(run_root / "logs"),
        "-j",
        "y",
        "-m",
        "a",
        "-S",
        "/bin/bash",
        "-l",
        f"h_rt={wall}",
        "-l",
        "mem_per_core=4G",
    ]
    if role != "collector":
        insertion = argv.index("-l")
        argv[insertion:insertion] = ["-pe", "omp", str(slots)]
    if role == "setup":
        argv.append("-h")
    if hold is not None:
        argv.extend(["-hold_jid", hold])
    if role == "array":
        argv.extend(["-t", f"1-{tasks}:1"])
    argv.extend(["-v", f"RUN_ROOT={run_root}", str(script)])
    return argv


def _qstat_fields(text: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in text.splitlines():
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        normalized = re.sub(r"[^a-z0-9]+", "_", key.strip().lower()).strip("_")
        if normalized:
            fields[normalized] = value.strip()
    return fields


def _verify_qstat_attestation(
    text: str,
    *,
    job_id: str,
    role: str,
    job_name: str,
    run_root: Path,
    script: Path,
    slots: int,
    tasks: int,
    hold: str | None,
) -> dict[str, str]:
    fields = _qstat_fields(text)
    required = {
        "job_number", "owner", "job_name", "cwd", "script_file",
        "hard_resource_list", "env_list", "shell_list", "stdout_path_list",
        "project", "merge",
    }
    if not required <= set(fields):
        raise SccError(f"qstat omits mandatory submitted-command fields: {role}")
    parallel = fields.get("parallel_environment", "")
    resources = fields["hard_resource_list"]
    if (
        fields["job_number"] != job_id
        or fields["owner"] != "johannes"
        or fields["job_name"] != job_name
        or fields["project"] != "welfgr"
        or fields["merge"] != "y"
        or fields["cwd"] != str(run_root)
        or Path(fields["script_file"]) != script
        or (
            role != "collector"
            and re.search(rf"\bomp\b.*\b{slots}\b", parallel) is None
        )
        or (role == "collector" and parallel not in ("", "NONE"))
        or set(resources.split(","))
            != {
                "mem_per_core=4G",
                "h_rt=1800" if role == "collector" else "h_rt=42300",
            }
        or fields["env_list"] != f"RUN_ROOT={run_root}"
        or fields["shell_list"] not in {"/bin/bash", "NONE:/bin/bash"}
        or fields["stdout_path_list"]
            != f"NONE:NONE:{run_root / 'logs'}"
    ):
        raise SccError(f"qstat differs from the exact submitted contract: {role}")
    array = fields.get("job_array_tasks")
    predecessors = {
        value for key, value in fields.items()
        if key in {"jid_predecessor_list", "jid_predecessor_list_req"}
        and value != ""
    }
    if (
        (role == "array" and array != f"1-{tasks}:1")
        or (role != "array" and array not in (None, ""))
        or (hold is None and predecessors)
        or (hold is not None and predecessors != {hold})
    ):
        raise SccError(f"qstat array/dependency contract differs: {role}")
    return fields


def record_submission(args: argparse.Namespace) -> None:
    controller = _controller_identity("submission", args.run_root)
    deployment = verify_deployment(args.run_root)
    run_root = args.run_root
    preparation_sha256 = digest_file(args.run_root / "input/preparation.json")
    if deployment.get("preparation_sha256") != preparation_sha256:
        raise SccError("submission is not bound to the independently recorded deployment")
    if os.environ.get("USER") != "johannes":
        raise SccError("qualification submission must be recorded by owner johannes")
    logs = run_root / "logs"
    if logs.exists():
        raise SccError("submission log directory already exists")
    logs.mkdir()
    submission_root = run_root / "work/submission"
    (submission_root / "home").mkdir(parents=True)
    (submission_root / "tmp").mkdir()
    clients = _scheduler_clients(run_root)
    scheduler_environment = _scheduler_environment(run_root, clients)
    raw_root = run_root / "receipts/submission-records"
    raw_root.mkdir(parents=True, exist_ok=False)
    task_count = int(verify_input(run_root)["task_count"])
    jobs: dict[str, str] = {}
    job_records: dict[str, Any] = {}
    role_specs = (
        (
            "setup_job", "setup", 16, 1,
            "benchmarks/scc/feppml_qualification_setup_job.sh",
            lambda: None,
        ),
        (
            "array_job", "array", 16, task_count,
            "benchmarks/scc/feppml_qualification_job.sh",
            lambda: jobs["setup_job"],
        ),
        (
            "collector_job", "collector", 1, 1,
            "benchmarks/scc/feppml_qualification_collector_job.sh",
            lambda: jobs["array_job"],
        ),
    )
    for field, role, slots, tasks, relative_script, hold_value in role_specs:
        hold = hold_value()
        script = run_root / "input/harness" / relative_script
        qsub_argv = _qsub_argv(
            run_root,
            str(clients["qsub"]["path"]),
            role=role,
            slots=slots,
            tasks=tasks,
            script=script,
            hold=hold,
        )
        completed = subprocess.run(
            qsub_argv,
            cwd=run_root,
            env=scheduler_environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        stdout_path = raw_root / f"{role}.qsub.stdout"
        stderr_path = raw_root / f"{role}.qsub.stderr"
        _write(stdout_path, completed.stdout.encode("utf-8"))
        _write(stderr_path, completed.stderr.encode("utf-8"))
        raw_output = completed.stdout.strip()
        if completed.returncode != 0 or completed.stderr != "":
            raise SccError(f"exact {role} qsub command failed")
        if role == "array":
            match = re.fullmatch(r"([1-9][0-9]*)\.1-([1-9][0-9]*):1", raw_output)
            if match is None or int(match.group(2)) != tasks:
                raise SccError("array qsub output is invalid")
            job_id = match.group(1)
        else:
            if JOB_ID.fullmatch(raw_output) is None:
                raise SccError(f"{role} qsub output is invalid")
            job_id = raw_output
        jobs[field] = job_id
        qstat_command = [str(clients["qstat"]["path"]), "-j", job_id]
        qstat_completed: subprocess.CompletedProcess[str] | None = None
        for _attempt in range(10):
            qstat_completed = subprocess.run(
                qstat_command,
                cwd=run_root,
                env=scheduler_environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                check=False,
            )
            if qstat_completed.returncode == 0:
                break
            time.sleep(0.5)
        if qstat_completed is None or qstat_completed.returncode != 0:
            raise SccError(f"qstat did not attest submitted {role} job")
        qstat_path = raw_root / f"{role}.qstat.stdout"
        qstat_stderr = raw_root / f"{role}.qstat.stderr"
        _write(qstat_path, qstat_completed.stdout.encode("utf-8"))
        _write(qstat_stderr, qstat_completed.stderr.encode("utf-8"))
        if qstat_completed.stderr != "":
            raise SccError(f"qstat emitted unexpected stderr for {role}")
        job_name = _job_name(run_root, role)
        qstat_fields = _verify_qstat_attestation(
            qstat_completed.stdout,
            job_id=job_id,
            role=role,
            job_name=job_name,
            run_root=run_root,
            script=script,
            slots=slots,
            tasks=tasks,
            hold=hold,
        )
        job_records[field] = {
            "role": role,
            "job_id": job_id,
            "raw_qsub_output": raw_output,
            "job_name": job_name,
            "owner": "johannes",
            "project": "welfgr",
            "cwd": str(args.run_root),
            "slots": slots,
            "task_count": tasks,
            "hold_job_id": hold,
            "script": {
                "path": script.relative_to(args.run_root).as_posix(),
                "sha256": digest_file(script),
                "size_bytes": script.stat().st_size,
            },
            "qsub_argv": qsub_argv,
            "qsub_stdout": _relative_file_record(run_root, stdout_path),
            "qsub_stderr": _relative_file_record(run_root, stderr_path),
            "qstat_argv": qstat_command,
            "qstat_stdout": _relative_file_record(run_root, qstat_path),
            "qstat_stderr": _relative_file_record(run_root, qstat_stderr),
            "qstat_fields": qstat_fields,
        }
    release_argv = [str(clients["qrls"]["path"]), jobs["setup_job"]]
    submission_path = run_root / "receipts/submission.json"
    write_sealed(
        submission_path,
        {
            "schema_version": 1,
            "kind": "feppml_qualification_scc_submission",
            "run_id": run_root.name,
            "preparation_sha256": preparation_sha256,
            **jobs,
            "recorded_at_utc": datetime.now(timezone.utc).strftime(
                "%Y-%m-%dT%H:%M:%SZ"
            ),
            "scheduler_clients": clients,
            "scheduler_environment": scheduler_environment,
            "controller": controller,
            "jobs": job_records,
            "release_argv": release_argv,
            "complete_sentinel": "FEPPML_SCC_SUBMISSION_COMPLETE",
        },
    )
    released = subprocess.run(
        release_argv,
        cwd=run_root,
        env=scheduler_environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    release_stdout = raw_root / "setup.qrls.stdout"
    release_stderr = raw_root / "setup.qrls.stderr"
    _write(release_stdout, released.stdout.encode("utf-8"))
    _write(release_stderr, released.stderr.encode("utf-8"))
    if released.returncode != 0 or released.stderr != "":
        raise SccError("held setup job could not be released after sealing")
    write_sealed(
        run_root / "receipts/submission-release.json",
        {
            "schema_version": 1,
            "kind": "feppml_qualification_scc_submission_release",
            "run_id": run_root.name,
            "submission_sha256": digest_file(submission_path),
            "command": release_argv,
            "stdout": _relative_file_record(run_root, release_stdout),
            "stderr": _relative_file_record(run_root, release_stderr),
            "return_code": released.returncode,
            "complete_sentinel": "FEPPML_SCC_SUBMISSION_RELEASE_COMPLETE",
        },
    )


def _job_name(run_root: Path, role: str) -> str:
    suffix = {"setup": "s", "array": "a", "collector": "c"}.get(role)
    if suffix is None:
        raise SccError("unknown qualification scheduler role")
    timestamp = run_root.name.split("-", 1)[0]
    if re.fullmatch(r"[0-9]{8}T[0-9]{6}Z", timestamp) is None:
        raise SccError("run ID has no scheduler-safe UTC timestamp")
    run_digest = digest_bytes(run_root.name.encode("ascii"))[:8]
    return f"fq_{timestamp}_{run_digest}_{suffix}"


def _expected_qsub_output(job_id: str, role: str, tasks: int) -> str:
    if role == "array":
        return f"{job_id}.1-{tasks}:1"
    return job_id


def _verify_submission_release(
    run_root: Path, submission: Mapping[str, Any]
) -> Mapping[str, Any]:
    release = read_sealed(
        run_root / "receipts/submission-release.json",
        "feppml_qualification_scc_submission_release",
    )
    stdout = run_root / "receipts/submission-records/setup.qrls.stdout"
    stderr = run_root / "receipts/submission-records/setup.qrls.stderr"
    if (
        set(release) != {
            "schema_version", "kind", "run_id", "submission_sha256",
            "command", "stdout", "stderr", "return_code",
            "complete_sentinel",
        }
        or release.get("schema_version") != 1
        or release.get("run_id") != run_root.name
        or release.get("submission_sha256")
            != digest_file(run_root / "receipts/submission.json")
        or release.get("command") != submission.get("release_argv")
        or release.get("stdout") != _relative_file_record(run_root, stdout)
        or release.get("stderr") != _relative_file_record(run_root, stderr)
        or stderr.read_bytes() != b""
        or release.get("return_code") != 0
        or release.get("complete_sentinel")
            != "FEPPML_SCC_SUBMISSION_RELEASE_COMPLETE"
    ):
        raise SccError("setup scheduler release receipt is invalid")
    return release


def _verify_submission(
    run_root: Path,
    preparation: Mapping[str, Any],
    *,
    portable: bool = False,
) -> Mapping[str, Any]:
    submission = read_sealed(
        run_root / "receipts/submission.json",
        "feppml_qualification_scc_submission",
    )
    expected_fields = {
        "schema_version", "kind", "run_id", "preparation_sha256", "setup_job",
        "array_job", "collector_job", "recorded_at_utc", "scheduler_clients",
        "scheduler_environment", "controller", "jobs", "release_argv",
        "complete_sentinel",
    }
    if (
        set(submission) != expected_fields
        or submission["schema_version"] != 1
        or submission["run_id"] != run_root.name
        or submission["preparation_sha256"]
            != digest_file(run_root / "input/preparation.json")
        or submission["complete_sentinel"] != "FEPPML_SCC_SUBMISSION_COMPLETE"
    ):
        raise SccError("closed submission receipt fields/identity are invalid")
    try:
        datetime.strptime(submission["recorded_at_utc"], "%Y-%m-%dT%H:%M:%SZ")
    except (TypeError, ValueError) as error:
        raise SccError("submission ledger timestamp is invalid") from error
    jobs = submission["jobs"]
    if not isinstance(jobs, Mapping) or set(jobs) != {
        "setup_job", "array_job", "collector_job"
    }:
        raise SccError("submission ledger job inventory is invalid")
    clients = _verify_scheduler_clients(
        submission["scheduler_clients"], portable=portable
    )
    submission_controller = _verify_controller_identity(
        run_root, submission.get("controller"), "submission", portable=portable
    )
    submitted_root = run_root
    if portable:
        recorded_roots = {
            str(record.get("cwd"))
            for record in jobs.values()
            if isinstance(record, Mapping)
        }
        if len(recorded_roots) != 1:
            raise SccError("portable submission ledger has inconsistent SCC roots")
        submitted_root = Path(recorded_roots.pop())
        if (
            not submitted_root.is_absolute()
            or submitted_root
                != PROJECT_ROOT / "runs" / run_root.name
        ):
            raise SccError("portable submission ledger is outside the exact SCC run root")
    if submission["scheduler_environment"] != _scheduler_environment(
        submitted_root, clients
    ) or submission_controller["environment"]["RUN_ROOT"] != str(submitted_root):
        raise SccError("scheduler submission environment is not exact")
    roles = {
        "setup_job": ("setup", 16, 1, "benchmarks/scc/feppml_qualification_setup_job.sh", None),
        "array_job": (
            "array", 16, int(preparation["task_count"]),
            "benchmarks/scc/feppml_qualification_job.sh", submission["setup_job"],
        ),
        "collector_job": (
            "collector", 1, 1,
            "benchmarks/scc/feppml_qualification_collector_job.sh", submission["array_job"],
        ),
    }
    for field, (role, slots, tasks, relative_script, hold) in roles.items():
        job_id = submission[field]
        record = jobs[field]
        script = run_root / "input/harness" / relative_script
        if JOB_ID.fullmatch(str(job_id)) is None or not isinstance(record, Mapping):
            raise SccError("submission ledger job identity is invalid")
        submitted_script = submitted_root / "input/harness" / relative_script
        expected_argv = _qsub_argv(
            submitted_root,
            str(clients["qsub"]["path"]),
            role=role,
            slots=slots,
            tasks=tasks,
            script=submitted_script,
            hold=hold,
        )
        qsub_stdout = run_root / f"receipts/submission-records/{role}.qsub.stdout"
        qsub_stderr = run_root / f"receipts/submission-records/{role}.qsub.stderr"
        qstat_stdout = run_root / f"receipts/submission-records/{role}.qstat.stdout"
        qstat_stderr = run_root / f"receipts/submission-records/{role}.qstat.stderr"
        qstat_argv = [str(clients["qstat"]["path"]), "-j", str(job_id)]
        for field_name, local in (
            ("qsub_stdout", qsub_stdout),
            ("qsub_stderr", qsub_stderr),
            ("qstat_stdout", qstat_stdout),
            ("qstat_stderr", qstat_stderr),
        ):
            if record.get(field_name) != _relative_file_record(run_root, local):
                raise SccError(f"sealed {role} {field_name} bytes changed")
        if qsub_stderr.read_bytes() != b"" or qstat_stderr.read_bytes() != b"":
            raise SccError(f"sealed {role} scheduler stderr is nonempty")
        raw_output = qsub_stdout.read_text(encoding="utf-8", errors="strict").strip()
        qstat_text = qstat_stdout.read_text(encoding="utf-8", errors="strict")
        qstat_fields = _verify_qstat_attestation(
            qstat_text,
            job_id=str(job_id),
            role=role,
            job_name=_job_name(run_root, role),
            run_root=submitted_root,
            script=submitted_script,
            slots=slots,
            tasks=tasks,
            hold=hold,
        )
        if (
            set(record) != {
                "role", "job_id", "raw_qsub_output", "job_name", "owner",
                "project", "cwd", "slots", "task_count", "hold_job_id",
                "script", "qsub_argv", "qsub_stdout", "qsub_stderr",
                "qstat_argv", "qstat_stdout", "qstat_stderr", "qstat_fields",
            }
            or record["role"] != role
            or record["job_id"] != job_id
            or record["raw_qsub_output"] != raw_output
            or raw_output
                != _expected_qsub_output(str(job_id), role, tasks)
            or record["job_name"] != _job_name(run_root, role)
            or record["owner"] != "johannes"
            or record["project"] != "welfgr"
            or record["cwd"] != str(submitted_root)
            or record["slots"] != slots
            or record["task_count"] != tasks
            or record["hold_job_id"] != hold
            or record["script"]
                != {
                    "path": script.relative_to(run_root).as_posix(),
                    "sha256": digest_file(script),
                    "size_bytes": script.stat().st_size,
                }
            or record["qsub_argv"] != expected_argv
            or record["qstat_argv"] != qstat_argv
            or record["qstat_fields"] != qstat_fields
        ):
            raise SccError(f"sealed {role} submission ledger is invalid")
    expected_release = [
        str(clients["qrls"]["path"]), str(submission["setup_job"])
    ]
    if submission["release_argv"] != expected_release:
        raise SccError("setup release command is not exact")
    _verify_submission_release(run_root, submission)
    return submission


def _qacct(
    job_id: str,
    *,
    run_root: Path,
    clients: Mapping[str, Mapping[str, Any]],
    environment: Mapping[str, str],
) -> str:
    attempt = _qacct_attempt(
        job_id,
        run_root=run_root,
        clients=clients,
        environment=environment,
    )
    if (
        attempt["timed_out"] is True
        or attempt["return_code"] != 0
        or base64.b64decode(attempt["stderr_base64"], validate=True) != b""
    ):
        raise SccError(f"exact qacct command failed for job {job_id}")
    return base64.b64decode(
        attempt["stdout_base64"], validate=True
    ).decode("utf-8", errors="strict")


def _qacct_attempt(
    job_id: str,
    *,
    run_root: Path,
    clients: Mapping[str, Mapping[str, Any]],
    environment: Mapping[str, str],
) -> Mapping[str, Any]:
    """Execute one exact qacct query and retain its complete supervisor state."""

    argv = [str(clients["qacct"]["path"]), "-j", job_id]
    try:
        completed = subprocess.run(
            argv,
            cwd=run_root,
            env=dict(environment),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=QACCT_TIMEOUT_SECONDS,
        )
        return_code: int | None = completed.returncode
        stdout = completed.stdout
        stderr = completed.stderr
        timed_out = False
    except subprocess.TimeoutExpired as error:
        return_code = None
        stdout = error.stdout or b""
        stderr = error.stderr or b""
        timed_out = True
    return {
        "schema_version": 1,
        "record_type": "feppml-sge-qacct-attempt-v1",
        "job_id": job_id,
        "argv": argv,
        "cwd": str(run_root),
        "environment": dict(environment),
        "client": dict(clients["qacct"]),
        "timeout_seconds": QACCT_TIMEOUT_SECONDS,
        "timed_out": timed_out,
        "return_code": return_code,
        "stdout_base64": base64.b64encode(stdout).decode("ascii"),
        "stdout_sha256": digest_bytes(stdout),
        "stderr_base64": base64.b64encode(stderr).decode("ascii"),
        "stderr_sha256": digest_bytes(stderr),
        "complete_sentinel": "FEPPML_SGE_QACCT_ATTEMPT_COMPLETE",
    }


def _verify_qacct_attempt(
    run_root: Path,
    value: Any,
    job_id: str,
    clients: Mapping[str, Mapping[str, Any]],
    environment: Mapping[str, str],
) -> Mapping[str, Any]:
    record = value if isinstance(value, Mapping) else {}
    fields = {
        "schema_version", "record_type", "job_id", "argv", "cwd",
        "environment", "client", "timeout_seconds", "timed_out", "return_code",
        "stdout_base64", "stdout_sha256", "stderr_base64", "stderr_sha256",
        "complete_sentinel",
    }
    recorded_root = Path(str(record.get("cwd", "")))
    try:
        stdout = base64.b64decode(str(record.get("stdout_base64", "")), validate=True)
        stderr = base64.b64decode(str(record.get("stderr_base64", "")), validate=True)
    except (binascii.Error, ValueError, TypeError) as error:
        raise SccError("retained qacct stream encoding is invalid") from error
    if (
        set(record) != fields
        or record.get("schema_version") != 1
        or record.get("record_type") != "feppml-sge-qacct-attempt-v1"
        or record.get("job_id") != job_id
        or record.get("argv")
            != [str(clients["qacct"]["path"]), "-j", job_id]
        or not recorded_root.is_absolute()
        or recorded_root.name != run_root.name
        or record.get("environment") != environment
        or environment.get("HOME")
            != str(recorded_root / "work/submission/home")
        or environment.get("TMPDIR")
            != str(recorded_root / "work/submission/tmp")
        or record.get("client") != clients["qacct"]
        or record.get("timeout_seconds") != QACCT_TIMEOUT_SECONDS
        or not isinstance(record.get("timed_out"), bool)
        or (
            record.get("timed_out") is False
            and (
                isinstance(record.get("return_code"), bool)
                or not isinstance(record.get("return_code"), int)
            )
        )
        or (record.get("timed_out") is True and record.get("return_code") is not None)
        or record.get("stdout_sha256")
            != digest_bytes(stdout)
        or record.get("stderr_sha256")
            != digest_bytes(stderr)
        or record.get("complete_sentinel")
            != "FEPPML_SGE_QACCT_ATTEMPT_COMPLETE"
    ):
        raise SccError("retained qacct query attempt is not exact")
    return record


def _qacct_bounded(
    job_id: str,
    expected_tasks: int,
    *,
    run_root: Path,
    clients: Mapping[str, Mapping[str, Any]],
    environment: Mapping[str, str],
    attempts: int = 12,
    delay_seconds: float = 5.0,
    job_contract: Mapping[str, Any] | None = None,
) -> tuple[str, list[dict[str, str]], list[Mapping[str, Any]]]:
    """Wait boundedly for complete accounting rather than queue disappearance."""

    if attempts < 1 or delay_seconds < 0:
        raise SccError("qacct retry policy is invalid")
    last_error: Exception | None = None
    retained_attempts: list[Mapping[str, Any]] = []
    for attempt in range(attempts):
        try:
            query = _qacct_attempt(
                job_id,
                run_root=run_root,
                clients=clients,
                environment=environment,
            )
            retained_attempts.append(query)
            _verify_qacct_attempt(
                run_root, query, job_id, clients, environment
            )
            stdout = base64.b64decode(query["stdout_base64"], validate=True)
            stderr = base64.b64decode(query["stderr_base64"], validate=True)
            if query["timed_out"] is True or query["return_code"] != 0 or stderr != b"":
                raise SccError(f"exact qacct command failed for job {job_id}")
            try:
                text = stdout.decode("utf-8", errors="strict")
            except UnicodeDecodeError as error:
                raise SccError("exact qacct stdout is not strict UTF-8") from error
            return text, _validated_accounting(
                text, expected_tasks, job_id, job_contract=job_contract
            ), retained_attempts
        except SccError as error:
            last_error = error
            if attempt + 1 < attempts:
                time.sleep(delay_seconds)
    raise SccError(
        f"qacct remained incomplete after {attempts} attempts for job {job_id}: {last_error}"
    )


def _verify_qacct_queries(
    run_root: Path,
    submission: Mapping[str, Any],
    clients: Mapping[str, Mapping[str, Any]],
    environment: Mapping[str, str],
    accounting_text: Mapping[str, str],
) -> Mapping[str, Any]:
    payload = read_sealed(
        run_root / "receipts/qacct/queries.json",
        "feppml_qualification_scc_qacct_queries",
    )
    if (
        set(payload)
        != {
            "schema_version", "kind", "run_id", "scheduler_clients",
            "scheduler_environment", "queries", "complete_sentinel",
        }
        or payload.get("schema_version") != 1
        or payload.get("run_id") != run_root.name
        or payload.get("scheduler_clients") != clients
        or payload.get("scheduler_environment") != environment
        or payload.get("complete_sentinel")
            != "FEPPML_SGE_QACCT_QUERIES_COMPLETE"
    ):
        raise SccError("qacct query supervisor receipt is invalid")
    queries = payload.get("queries")
    names = {"setup_job", "array_job", "collector_job"}
    if not isinstance(queries, Mapping) or set(queries) != names:
        raise SccError("qacct query supervisor inventory is incomplete")
    for name in sorted(names):
        query = queries[name]
        if not isinstance(query, Mapping) or set(query) != {
            "job_id", "attempts", "accepted_attempt"
        }:
            raise SccError("qacct job query receipt is not closed")
        job_id = str(submission[name])
        attempts = query.get("attempts")
        if (
            query.get("job_id") != job_id
            or not isinstance(attempts, list)
            or not 1 <= len(attempts) <= 12
            or query.get("accepted_attempt") != len(attempts)
        ):
            raise SccError("qacct job query retry inventory is invalid")
        verified = [
            _verify_qacct_attempt(
                run_root, attempt, job_id, clients, environment
            )
            for attempt in attempts
        ]
        final = verified[-1]
        try:
            final_stdout = base64.b64decode(
                final["stdout_base64"], validate=True
            ).decode("utf-8", errors="strict")
            final_stderr = base64.b64decode(
                final["stderr_base64"], validate=True
            )
        except (binascii.Error, ValueError, UnicodeDecodeError) as error:
            raise SccError("qacct final retained streams are invalid") from error
        if (
            final["timed_out"] is True
            or
            final["return_code"] != 0
            or final_stderr != b""
            or final_stdout != accounting_text[name]
        ):
            raise SccError("qacct final successful query differs from saved accounting")
        expected_count = int(submission["jobs"][name]["task_count"])
        _validated_accounting(
            final_stdout, expected_count, job_id,
            job_contract=submission["jobs"][name],
        )
        for earlier in verified[:-1]:
            earlier_stderr = base64.b64decode(
                earlier["stderr_base64"], validate=True
            )
            if (
                earlier["timed_out"] is False
                and earlier["return_code"] == 0
                and earlier_stderr == b""
            ):
                try:
                    earlier_stdout = base64.b64decode(
                        earlier["stdout_base64"], validate=True
                    ).decode("utf-8", errors="strict")
                    _validated_accounting(
                        earlier_stdout, expected_count, job_id,
                        job_contract=submission["jobs"][name],
                    )
                except (SccError, UnicodeDecodeError):
                    pass
                else:
                    raise SccError("qacct retried after a complete valid accounting query")
    return payload


def _qacct_records(text: str) -> list[dict[str, str]]:
    """Parse closed SGE accounting records without interpreting free text."""
    records = []
    for block in re.split(r"(?m)^={10,}\s*$", text):
        if not re.search(r"(?m)^jobnumber\s+", block):
            continue
        record: dict[str, str] = {}
        for line in block.splitlines():
            fields = line.strip().split(None, 1)
            if len(fields) != 2:
                continue
            key, value = fields
            if key in record:
                raise SccError(f"duplicate qacct field: {key}")
            record[key] = value.strip()
        records.append(record)
    return records


def _resource_value(value: str) -> bool:
    return re.fullmatch(r"[0-9]+(?:\.[0-9]+)?(?:[KMGTPE](?:B)?|B)?", value) is not None


def _validated_accounting(
    text: str,
    expected_tasks: int | None = None,
    expected_job_id: str | None = None,
    *,
    job_contract: Mapping[str, Any] | None = None,
) -> list[dict[str, str]]:
    records = _qacct_records(text)
    if not records or (expected_tasks is not None and len(records) != expected_tasks):
        raise SccError("qacct record inventory is incomplete")
    required = {
        "jobnumber", "project", "failed", "exit_status", "hostname", "qname",
        "slots", "ru_wallclock", "cpu", "maxvmem", "owner", "jobname",
        "qsub_time", "start_time", "end_time", "granted_pe",
    }
    for record in records:
        if not required <= set(record):
            raise SccError("qacct record omits scheduler/resource fields")
        if (
            (expected_job_id is not None and record["jobnumber"] != expected_job_id)
            or record["project"] != "welfgr"
            or record["owner"] != "johannes"
            or record["failed"] != "0"
            or record["exit_status"] != "0"
            or not record["hostname"]
            or not record["qname"]
            or not record["slots"].isdigit()
            or int(record["slots"]) <= 0
            or record["granted_pe"] not in {"omp", "NONE"}
        ):
            raise SccError("qacct scheduler identity or exit state is invalid")
        try:
            wallclock = float(record["ru_wallclock"])
            cpu = float(record["cpu"])
        except ValueError as error:
            raise SccError("qacct resource accounting is not numeric") from error
        if wallclock < 0.0 or cpu < 0.0 or not _resource_value(record["maxvmem"]):
            raise SccError("qacct resource accounting is invalid")
        qsub_time = _qacct_time(record["qsub_time"])
        start_time = _qacct_time(record["start_time"])
        end_time = _qacct_time(record["end_time"])
        if not qsub_time <= start_time <= end_time:
            raise SccError("qacct timestamps violate submit/start/end ordering")
        if job_contract is not None:
            role = job_contract.get("role")
            if role not in {"setup", "array", "collector"}:
                raise SccError("sealed scheduler job contract has an invalid role")
            expected_pe = (
                {"NONE"}
                if role == "collector"
                else {"omp"}
            )
            if (
                record["jobname"] != job_contract["job_name"]
                or record["owner"] != job_contract["owner"]
                or record["project"] != job_contract["project"]
                or int(record["slots"]) != job_contract["slots"]
                or record["granted_pe"] not in expected_pe
            ):
                raise SccError("qacct record differs from the sealed submission ledger")
            if "cwd" in record and Path(record["cwd"]).resolve() != Path(job_contract["cwd"]).resolve():
                raise SccError("qacct cwd differs from the sealed submission ledger")
            if "scriptfile" in record:
                expected_script = run_root_script = Path(job_contract["cwd"]) / Path(
                    job_contract["script"]["path"]
                )
                observed_script = Path(record["scriptfile"])
                if (
                    observed_script.is_absolute()
                    and observed_script.resolve() != run_root_script.resolve()
                ) or (
                    not observed_script.is_absolute()
                    and observed_script.name != expected_script.name
                ):
                    raise SccError("qacct scriptfile differs from the submitted script")
            for field in (
                "submit_cmdline", "submit_cmd_line", "submission_command"
            ):
                if field in record:
                    try:
                        observed_argv = shlex.split(record[field])
                    except ValueError as error:
                        raise SccError("qacct submit command is malformed") from error
                    if observed_argv != job_contract["qsub_argv"]:
                        raise SccError("qacct submit command differs from the ledger")
    if expected_tasks is not None and expected_tasks > 1:
        task_ids = []
        for record in records:
            task = record.get("taskid", "")
            if not task.isdigit():
                raise SccError("array qacct record omits its numeric task ID")
            task_ids.append(int(task))
        if sorted(task_ids) != list(range(1, expected_tasks + 1)):
            raise SccError("array qacct task inventory is not exact")
    elif expected_tasks == 1:
        task = records[0].get("taskid")
        if task not in {None, "", "undefined", "NONE", "none"}:
            raise SccError("scalar qacct record unexpectedly has an array task ID")
    return records


def _qacct_time(value: str) -> datetime:
    try:
        naive = datetime.strptime(value, "%a %b %d %H:%M:%S %Y")
    except ValueError as error:
        raise SccError("qacct timestamp is malformed") from error
    zone = ZoneInfo("America/New_York")
    first = naive.replace(tzinfo=zone, fold=0)
    second = naive.replace(tzinfo=zone, fold=1)
    if first.utcoffset() != second.utcoffset():
        raise SccError("qacct timestamp falls in the ambiguous DST rollback hour")
    return first.astimezone(timezone.utc)


def _accounting_ok(
    text: str,
    expected_tasks: int | None = None,
    expected_job_id: str | None = None,
) -> bool:
    try:
        _validated_accounting(text, expected_tasks, expected_job_id)
    except SccError:
        return False
    return True


def _node(value: Any) -> str:
    return str(value).strip().lower().split(".", 1)[0]


def _accounting_summary(record: Mapping[str, str]) -> dict[str, Any]:
    return {
        "job_id": record["jobnumber"],
        "task_id": int(record["taskid"]) if record.get("taskid", "").isdigit() else None,
        "node_id": record["hostname"],
        "qname": record["qname"],
        "slots": int(record["slots"]),
        "ru_wallclock": float(record["ru_wallclock"]),
        "cpu": float(record["cpu"]),
        "maxvmem": record["maxvmem"],
        "owner": record["owner"],
        "job_name": record["jobname"],
        "qsub_time": record["qsub_time"],
        "start_time": record["start_time"],
        "end_time": record["end_time"],
    }


def _validate_accounting_lifecycle(
    records: Mapping[str, Sequence[Mapping[str, str]]],
    submission: Mapping[str, Any],
) -> None:
    setup = records["setup_job"][0]
    array = records["array_job"]
    collector = records["collector_job"][0]
    if (
        _qacct_time(setup["qsub_time"])
            > min(_qacct_time(record["qsub_time"]) for record in array)
        or max(_qacct_time(record["qsub_time"]) for record in array)
            > _qacct_time(collector["qsub_time"])
        or _qacct_time(setup["end_time"])
            > min(_qacct_time(record["start_time"]) for record in array)
        or max(_qacct_time(record["end_time"]) for record in array)
            > _qacct_time(collector["start_time"])
    ):
        raise SccError("qacct lifecycle violates held setup/array/collector ordering")
    run_time = datetime.strptime(
        submission["run_id"].split("-", 1)[0], "%Y%m%dT%H%M%SZ"
    ).replace(tzinfo=timezone.utc)
    recorded = datetime.strptime(
        submission["recorded_at_utc"], "%Y-%m-%dT%H:%M:%SZ"
    ).replace(tzinfo=timezone.utc)
    qsub_times = [
        _qacct_time(record["qsub_time"])
        for role_records in records.values()
        for record in role_records
    ]
    if min(qsub_times) < run_time or max(qsub_times) > recorded:
        raise SccError("qacct submission times fall outside the sealed run ledger")


def close(run_root: Path) -> None:
    controller = _controller_identity("close", run_root)
    preparation = verify_input(run_root)
    materialization, _setup_materialization = _verify_materialized_capture(
        run_root, preparation
    )
    preparation_sha256 = digest_file(run_root / "input/preparation.json")
    materialization_receipt_sha256 = digest_file(run_root / "receipts/capture-plan.json")
    capture_spec_sha256 = digest_file(run_root / "work/capture/spec.json")
    deployment = read_sealed(
        run_root / "receipts/deployment.json",
        "feppml_qualification_scc_deployment",
    )
    submission = _verify_submission(run_root, preparation)
    scheduler_clients = _verify_scheduler_clients(
        submission["scheduler_clients"], portable=False
    )
    scheduler_environment = _scheduler_environment(
        run_root, scheduler_clients
    )
    collection = read_sealed(run_root / "receipts/collector.json", "feppml_qualification_scc_collection")
    replayed_evidence, replay = assemble_collection(
        run_root, preparation, materialization
    )
    evidence_path = run_root / "output/evidence.json"
    if _read(evidence_path, "qualification evidence") != replayed_evidence:
        raise SccError("collector evidence differs from independent raw replay")
    for field, expected_value in replay.items():
        if collection.get(field) != expected_value:
            raise SccError(f"collector does not bind raw replay field: {field}")
    qualification_result = qualification.verify_qualification_receipt(
        run_root / "input/manifests/qualification.json",
        evidence_path,
        run_root / "receipts/qualification.json",
        source_root=run_root / "work/candidate",
        source_archive_path=run_root / "input/archives/candidate.tar",
    )
    accounting_text: dict[str, str] = {}
    accounting_records: dict[str, list[dict[str, str]]] = {}
    accounting_attempts: dict[str, list[Mapping[str, Any]]] = {}
    if (
        deployment.get("preparation_sha256") != preparation_sha256
        or submission.get("preparation_sha256") != preparation_sha256
    ):
        raise SccError("submission receipt is bound to another preparation")
    for name in ("setup_job", "array_job", "collector_job"):
        expected = preparation["task_count"] if name == "array_job" else 1
        text, records, attempts = _qacct_bounded(
            submission[name],
            expected,
            run_root=run_root,
            clients=scheduler_clients,
            environment=scheduler_environment,
            job_contract=submission["jobs"][name],
        )
        required_slots = 1 if name == "collector_job" else 16
        if any(int(record["slots"]) != required_slots for record in records):
            raise SccError(f"{name} qacct slots differ from the submitted resource contract")
        accounting_records[name] = records
        accounting_text[name] = text
        accounting_attempts[name] = attempts
    _validate_accounting_lifecycle(accounting_records, submission)

    setup = read_sealed(run_root / "receipts/setup.json", "feppml_qualification_scc_setup")
    setup_controller = _verify_controller_identity(
        run_root, setup.get("controller"), "setup", portable=False
    )
    collector_controller = _verify_controller_identity(
        run_root, collection.get("controller"), "collector", portable=False
    )
    setup_record = accounting_records["setup_job"][0]
    if (
        setup.get("job_id") != submission["setup_job"]
        or setup.get("preparation_sha256") != preparation_sha256
        or setup.get("capture_plan_sha256") != materialization["plan_file_sha256"]
        or setup.get("capture_spec_sha256") != capture_spec_sha256
        or setup.get("capture_materialization_receipt_sha256")
        != materialization_receipt_sha256
        or _node(setup.get("node_id")) != _node(setup_record["hostname"])
    ):
        raise SccError("setup receipt is not bound to its scheduler job and node")

    collector_record = accounting_records["collector_job"][0]
    if (
        collection.get("job_id") != submission["collector_job"]
        or collection.get("preparation_sha256") != preparation_sha256
        or collection.get("capture_plan_sha256") != materialization["plan_file_sha256"]
        or collection.get("capture_spec_sha256") != capture_spec_sha256
        or collection.get("capture_materialization_receipt_sha256")
        != materialization_receipt_sha256
        or _node(collection.get("node_id")) != _node(collector_record["hostname"])
        or collector_controller["environment"]["JOB_ID"]
            != submission["collector_job"]
        or collector_controller["executable"] != setup_controller["executable"]
        or collection.get("evidence_sha256")
        != digest_file(run_root / "output/evidence.json")
        or collection.get("qualification_receipt_sha256")
        != digest_file(run_root / "receipts/qualification.json")
    ):
        raise SccError("collector receipt is not bound to its scheduler job and node")

    array_by_task = {
        int(record["taskid"]): record for record in accounting_records["array_job"]
    }
    for task_id in range(1, preparation["task_count"] + 1):
        shard = read_sealed(
            run_root / "receipts/tasks" / f"task-{task_id:03d}.json",
            "feppml_qualification_cell_shard",
        )
        record = array_by_task[task_id]
        if (
            shard.get("job_id") != submission["array_job"]
            or shard.get("task_id") != task_id
            or shard.get("preparation_sha256") != preparation_sha256
            or shard.get("capture_plan_sha256") != materialization["plan_file_sha256"]
            or shard.get("capture_spec_sha256") != capture_spec_sha256
            or shard.get("capture_materialization_receipt_sha256")
            != materialization_receipt_sha256
            or _node(shard.get("node_id")) != _node(record["hostname"])
            or shard.get("benchmark_cell", {}).get("execution", {}).get("scheduler_job_id")
                != submission["array_job"]
            or _node(
                shard.get("benchmark_cell", {}).get("execution", {}).get("node_id")
            )
                != _node(record["hostname"])
        ):
            raise SccError(f"task receipt is not bound to qacct: {task_id}")

    accounting_sha256 = {}
    for name, text in accounting_text.items():
        path = run_root / "receipts/qacct" / f"{name}.txt"
        _write(path, text.encode("utf-8"))
        accounting_sha256[name] = digest_file(path)
    query_payload = {
        "schema_version": 1,
        "kind": "feppml_qualification_scc_qacct_queries",
        "run_id": run_root.name,
        "scheduler_clients": dict(scheduler_clients),
        "scheduler_environment": scheduler_environment,
        "queries": {
            name: {
                "job_id": str(submission[name]),
                "attempts": accounting_attempts[name],
                "accepted_attempt": len(accounting_attempts[name]),
            }
            for name in ("setup_job", "array_job", "collector_job")
        },
        "complete_sentinel": "FEPPML_SGE_QACCT_QUERIES_COMPLETE",
    }
    query_path = run_root / "receipts/qacct/queries.json"
    write_sealed(query_path, query_payload)
    _verify_qacct_queries(
        run_root,
        submission,
        scheduler_clients,
        scheduler_environment,
        accounting_text,
    )
    if collection["status"] != "pass" or qualification_result["status"] != "pass":
        raise SccError("qualification did not pass; RUN.SUCCESS is forbidden")
    payload = {
        "schema_version": 1, "kind": "feppml_qualification_scc_close",
        "run_id": run_root.name,
        "candidate": dict(preparation["candidate"]),
        "candidate_source_commit": preparation["candidate"]["commit"],
        "platform_inputs": dict(preparation["platform_inputs"]),
        "task_count": preparation["task_count"],
        "preparation_sha256": preparation_sha256,
        "deployment_receipt_sha256": digest_file(
            run_root / "receipts/deployment.json"
        ),
        "submission_receipt_sha256": digest_file(
            run_root / "receipts/submission.json"
        ),
        "capture_plan_sha256": materialization["plan_file_sha256"],
        "capture_spec_sha256": capture_spec_sha256,
        "capture_materialization_receipt_sha256": materialization_receipt_sha256,
        "qualification_receipt_sha256": digest_file(run_root / "receipts/qualification.json"),
        "evidence_sha256": digest_file(evidence_path),
        "setup_receipt_sha256": digest_file(run_root / "receipts/setup.json"),
        "collector_receipt_sha256": digest_file(run_root / "receipts/collector.json"),
        "accounting_sha256": accounting_sha256,
        "qacct_query_receipt_sha256": digest_file(query_path),
        "accounting": {
            "setup": _accounting_summary(setup_record),
            "array": [
                _accounting_summary(array_by_task[task_id])
                for task_id in range(1, preparation["task_count"] + 1)
            ],
            "collector": _accounting_summary(collector_record),
        },
        "replay": {
            **replay,
            "evidence_sha256": digest_file(evidence_path),
            "qualification_receipt_sha256": digest_file(
                run_root / "receipts/qualification.json"
            ),
        },
        "controller": controller,
        "status": "pass", "complete_sentinel": "FEPPML_SCC_RUN_COMPLETE",
    }
    final_path = run_root / "receipts/final.json"
    write_sealed(final_path, payload)
    _write(
        run_root / "receipts/RUN.SUCCESS",
        f"FEPPML_SCC_RUN_COMPLETE|{digest_file(final_path)}\n".encode("ascii"),
    )


def verify_closed_run(run_root: Path) -> Mapping[str, Any]:
    """Revalidate a run offline from retained bytes; never query or write qacct.

    Saved qacct text is integrity-bound to this closure, but SGE does not sign
    it.  Authenticity against a hostile author still requires a trusted SCC
    query or external attestation at collection time.
    """

    run_root = _run_root(run_root)
    preparation = verify_input(run_root)
    deployment = verify_deployment(run_root)
    materialization, setup = _verify_materialized_capture(
        run_root, preparation, portable=True
    )
    preparation_sha256 = digest_file(run_root / "input/preparation.json")
    capture_spec_sha256 = digest_file(run_root / "work/capture/spec.json")
    materialization_receipt_sha256 = digest_file(run_root / "receipts/capture-plan.json")
    submission = _verify_submission(run_root, preparation, portable=True)
    if deployment.get("preparation_sha256") != preparation_sha256:
        raise SccError("closed-run deployment is bound to another preparation")

    qacct_root = run_root / "receipts/qacct"
    expected_qacct = {
        *(f"{name}.txt" for name in ("setup_job", "array_job", "collector_job")),
        "queries.json",
    }
    observed_qacct = {
        path.name for path in qacct_root.iterdir() if path.is_file() and not path.is_symlink()
    } if qacct_root.is_dir() else set()
    if observed_qacct != expected_qacct:
        raise SccError("saved qacct/query inventory is not exact")
    accounting_records: dict[str, list[dict[str, str]]] = {}
    accounting_sha256: dict[str, str] = {}
    accounting_text: dict[str, str] = {}
    for name, count in (
        ("setup_job", 1),
        ("array_job", int(preparation["task_count"])),
        ("collector_job", 1),
    ):
        path = qacct_root / f"{name}.txt"
        text = path.read_text(encoding="utf-8", errors="strict")
        records = _validated_accounting(
            text,
            count, str(submission[name]), job_contract=submission["jobs"][name],
        )
        accounting_records[name] = records
        accounting_text[name] = text
        accounting_sha256[name] = digest_file(path)
    _validate_accounting_lifecycle(accounting_records, submission)
    scheduler_clients = _verify_scheduler_clients(
        submission["scheduler_clients"], portable=True
    )
    _verify_qacct_queries(
        run_root,
        submission,
        scheduler_clients,
        submission["scheduler_environment"],
        accounting_text,
    )

    setup_record = accounting_records["setup_job"][0]
    if (
        setup.get("job_id") != submission["setup_job"]
        or setup.get("preparation_sha256") != preparation_sha256
        or setup.get("capture_plan_sha256") != materialization["plan_file_sha256"]
        or setup.get("capture_spec_sha256") != capture_spec_sha256
        or setup.get("capture_materialization_receipt_sha256")
            != materialization_receipt_sha256
        or setup.get("preflight_sha256")
            != digest_file(run_root / "work/preflight/science.json")
        or _node(setup.get("node_id")) != _node(setup_record["hostname"])
        or setup.get("science_before_timing") is not True
        or setup.get("timed_arm_processes_started") != 0
    ):
        raise SccError("closed-run setup is not bound to qacct/raw preflight")

    replayed_evidence, replay = assemble_collection(
        run_root, preparation, materialization, portable=True
    )
    evidence_path = run_root / "output/evidence.json"
    qualification_path = run_root / "receipts/qualification.json"
    if _read(evidence_path, "qualification evidence") != replayed_evidence:
        raise SccError("stored evidence differs from the 33-shard raw replay")
    collection = read_sealed(
        run_root / "receipts/collector.json",
        "feppml_qualification_scc_collection",
    )
    setup_controller = _verify_controller_identity(
        run_root, setup.get("controller"), "setup", portable=True
    )
    collector_controller = _verify_controller_identity(
        run_root, collection.get("controller"), "collector", portable=True
    )
    collector_record = accounting_records["collector_job"][0]
    if (
        collection.get("job_id") != submission["collector_job"]
        or collection.get("preparation_sha256") != preparation_sha256
        or collection.get("capture_plan_sha256") != materialization["plan_file_sha256"]
        or collection.get("capture_spec_sha256") != capture_spec_sha256
        or collection.get("capture_materialization_receipt_sha256")
            != materialization_receipt_sha256
        or _node(collection.get("node_id")) != _node(collector_record["hostname"])
        or collector_controller["environment"]["JOB_ID"]
            != submission["collector_job"]
        or collector_controller["executable"] != setup_controller["executable"]
        or collection.get("evidence_sha256") != digest_file(evidence_path)
        or collection.get("qualification_receipt_sha256") != digest_file(qualification_path)
        or collection.get("status") != "pass"
        or collection.get("stage") != "complete"
        or collection.get("complete_sentinel") != "FEPPML_SCC_COLLECTION_COMPLETE"
        or any(collection.get(field) != expected for field, expected in replay.items())
        or (run_root / "receipts/COLLECTOR.COMPLETE").read_text(encoding="ascii")
            != "FEPPML_SCC_COLLECTION_COMPLETE\n"
    ):
        raise SccError("closed-run collector differs from qacct/raw replay")
    verified_qualification = qualification.verify_qualification_receipt(
        run_root / "input/manifests/qualification.json",
        evidence_path,
        qualification_path,
        source_root=run_root / "work/candidate",
        source_archive_path=run_root / "input/archives/candidate.tar",
        platform_verifier=_portable_platform_verifier(run_root, preparation),
    )
    if verified_qualification.get("status") != "pass":
        raise SccError("closed-run scientific qualification did not pass")

    array_by_task = {
        int(record["taskid"]): record for record in accounting_records["array_job"]
    }
    for task_id in range(1, int(preparation["task_count"]) + 1):
        shard = read_sealed(
            run_root / "receipts/tasks" / f"task-{task_id:03d}.json",
            "feppml_qualification_cell_shard",
        )
        record = array_by_task[task_id]
        if (
            shard.get("job_id") != submission["array_job"]
            or shard.get("task_id") != task_id
            or _node(shard.get("node_id")) != _node(record["hostname"])
            or shard.get("task_environment", {}).get("scheduler_job_id")
                != submission["array_job"]
            or _node(shard.get("task_environment", {}).get("node_id"))
                != _node(record["hostname"])
        ):
            raise SccError(f"closed-run raw shard is not bound to qacct: {task_id}")

    setup_record = accounting_records["setup_job"][0]
    final_path = run_root / "receipts/final.json"
    final = read_sealed(final_path, "feppml_qualification_scc_close")
    close_controller = _verify_controller_identity(
        run_root, final.get("controller"), "close", portable=True
    )
    if close_controller["executable"] != setup_controller["executable"]:
        raise SccError("close controller differs from the qualified SCC Python")
    expected_final = {
        "schema_version": 1,
        "kind": "feppml_qualification_scc_close",
        "run_id": run_root.name,
        "candidate": dict(preparation["candidate"]),
        "candidate_source_commit": preparation["candidate"]["commit"],
        "platform_inputs": dict(preparation["platform_inputs"]),
        "task_count": preparation["task_count"],
        "preparation_sha256": preparation_sha256,
        "deployment_receipt_sha256": digest_file(run_root / "receipts/deployment.json"),
        "submission_receipt_sha256": digest_file(run_root / "receipts/submission.json"),
        "capture_plan_sha256": materialization["plan_file_sha256"],
        "capture_spec_sha256": capture_spec_sha256,
        "capture_materialization_receipt_sha256": materialization_receipt_sha256,
        "qualification_receipt_sha256": digest_file(qualification_path),
        "evidence_sha256": digest_file(evidence_path),
        "setup_receipt_sha256": digest_file(run_root / "receipts/setup.json"),
        "collector_receipt_sha256": digest_file(run_root / "receipts/collector.json"),
        "accounting_sha256": accounting_sha256,
        "qacct_query_receipt_sha256": digest_file(
            run_root / "receipts/qacct/queries.json"
        ),
        "accounting": {
            "setup": _accounting_summary(setup_record),
            "array": [
                _accounting_summary(array_by_task[task_id])
                for task_id in range(1, int(preparation["task_count"]) + 1)
            ],
            "collector": _accounting_summary(collector_record),
        },
        "replay": {
            **replay,
            "evidence_sha256": digest_file(evidence_path),
            "qualification_receipt_sha256": digest_file(qualification_path),
        },
        "controller": close_controller,
        "status": "pass",
        "complete_sentinel": "FEPPML_SCC_RUN_COMPLETE",
    }
    if final != expected_final:
        raise SccError("final closed-run receipt differs from authoritative replay")
    if (run_root / "receipts/RUN.SUCCESS").read_text(encoding="ascii") != (
        f"FEPPML_SCC_RUN_COMPLETE|{digest_file(final_path)}\n"
    ):
        raise SccError("RUN.SUCCESS is absent or not hash-bound to final receipt")
    return final


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare")
    prepare_parser.add_argument("--run-root", type=Path, required=True)
    prepare_parser.add_argument("--candidate-repo", type=Path, required=True)
    prepare_parser.add_argument("--candidate-commit", required=True)
    prepare_parser.add_argument("--manifest", type=Path, required=True)
    prepare_parser.add_argument("--capture-spec", type=Path, required=True)
    prepare_parser.add_argument("--capture-plan", type=Path, required=True)
    prepare_parser.add_argument("--ppmlhdfe-archive", type=Path, required=True)
    for name in ("reghdfe", "ftools", "require"):
        prepare_parser.add_argument(f"--{name}-repo", type=Path, required=True)
    verify = commands.add_parser("verify-input")
    verify.add_argument("--run-root", type=Path, required=True)
    verify_deploy = commands.add_parser("verify-deployment")
    verify_deploy.add_argument("--run-root", type=Path, required=True)
    setup_parser = commands.add_parser("setup")
    setup_parser.add_argument("--run-root", type=Path, required=True)
    setup_parser.add_argument("--timeout-seconds", type=float, default=14400.0)
    task_parser = commands.add_parser("task")
    task_parser.add_argument("--run-root", type=Path, required=True)
    task_parser.add_argument("--task-id", type=int, required=True)
    task_parser.add_argument("--timeout-seconds", type=float, default=14400.0)
    collect_parser = commands.add_parser("collect")
    collect_parser.add_argument("--run-root", type=Path, required=True)
    deployment = commands.add_parser("record-deployment")
    deployment.add_argument("--run-root", type=Path, required=True)
    deployment.add_argument("--preparation-sha256", required=True)
    submission = commands.add_parser("record-submission")
    submission.add_argument("--run-root", type=Path, required=True)
    close_parser = commands.add_parser("close")
    close_parser.add_argument("--run-root", type=Path, required=True)
    verify_closed = commands.add_parser("verify-closed-run")
    verify_closed.add_argument("--run-root", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        if args.command == "prepare":
            prepare(args)
        elif args.command == "verify-input":
            verify_input(args.run_root)
        elif args.command == "verify-deployment":
            verify_deployment(args.run_root)
        elif args.command == "setup":
            setup(args.run_root, args.timeout_seconds)
        elif args.command == "task":
            task(args.run_root, args.task_id, args.timeout_seconds)
        elif args.command == "collect":
            result = collect(args.run_root)
            if result["status"] != "pass":
                return 1
        elif args.command == "record-deployment":
            record_deployment(args)
        elif args.command == "record-submission":
            record_submission(args)
        elif args.command == "close":
            close(args.run_root)
        else:
            verify_closed_run(args.run_root)
    except (
        SccError,
        capture.CaptureError,
        qualification.QualificationError,
        development.ContractError,
        materializer.MaterializationError,
        capture_program.CaptureFailure,
        oracle_recipes.FixtureCompileError,
        constrained_recipes.ConstrainedRecipeError,
        reference_closure.ClosureError,
    ) as error:
        raise SystemExit(f"feppml SCC qualification failed closed: {error}") from error
    print(f"FEPPML_SCC_{args.command.upper().replace('-', '_')}_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
