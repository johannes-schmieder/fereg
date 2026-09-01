#!/usr/bin/env python3
"""Produce and verify the closed Phase-12 cross-platform native evidence.

The licensed-load receipt is intentionally not native-build evidence.  This
module joins three platform-specific authorities instead:

* a concrete macOS execution of the committed ``build_macos.sh`` (whose
  fail-closed body performs the binary audit and actual ABI loader test), plus
  the independently verified licensed-Stata platform record;
* the Linux SCC closed-run verifier, which traverses build, audit, actual
  loader, licensed load, scheduler identity, qacct, and its terminal marker;
* the Windows platform record, whose typed controller provenance traverses the
  accepted controller PASS, source-side manifest, build/audit/loader job, and
  licensed load.

All path-bearing companions remain mandatory.  A seal supplies byte integrity
and cross-binding; it is not an authentication or signing mechanism.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from typing import Any, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = 1
RECEIPT_TYPE = "feppml-native-platform-qualification-v1"
MACOS_BUILD_TYPE = "feppml-native-macos-build-v1"
CANONICALIZATION = "json-sort-keys-compact-ascii-newline-v1"
COMPLETE_SENTINEL = "FEPPML_NATIVE_PLATFORM_QUALIFICATION_COMPLETE"
MACOS_SENTINEL = "FEPPML_NATIVE_MACOS_BUILD_COMPLETE"
SHA1 = re.compile(r"^[0-9a-f]{40}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
PLATFORMS = ("linux-x86_64", "macos-universal", "windows-x86_64")


class NativeQualificationError(ValueError):
    """Native evidence is absent, mixed, mutable, or not authoritative."""


def canonical_json_bytes(value: Any) -> bytes:
    try:
        encoded = json.dumps(
            value, sort_keys=True, separators=(",", ":"), ensure_ascii=True,
            allow_nan=False,
        )
    except (TypeError, ValueError) as error:
        raise NativeQualificationError(f"value is not canonical JSON: {error}") from error
    return (encoded + "\n").encode("ascii")


def digest_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def digest_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1 << 20), b""):
                digest.update(block)
    except OSError as error:
        raise NativeQualificationError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def seal_payload(payload: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "payload": dict(payload),
        "seal": {
            "algorithm": "sha256",
            "canonicalization": CANONICALIZATION,
            "payload_sha256": digest_bytes(canonical_json_bytes(payload)),
        },
    }


def _write_immutable(path: Path, value: Mapping[str, Any]) -> None:
    data = canonical_json_bytes(value)
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise NativeQualificationError(f"refusing to overwrite immutable output: {path}") from error
    with os.fdopen(descriptor, "wb") as target:
        target.write(data)
        target.flush()
        os.fsync(target.fileno())


def _mapping(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise NativeQualificationError(f"{where} must be an object")
    return value


def _closed(value: Mapping[str, Any], fields: set[str], where: str) -> None:
    if set(value) != fields:
        raise NativeQualificationError(
            f"{where} fields are not closed; missing={sorted(fields-set(value))}, "
            f"extra={sorted(set(value)-fields)}"
        )


def _git(root: Path, *args: str) -> bytes:
    try:
        return subprocess.check_output(
            ["git", "-C", str(root), *args], stderr=subprocess.STDOUT
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise NativeQualificationError(f"git source verification failed: {error}") from error


def _source(root: Path, revision: str) -> dict[str, str]:
    commit = _git(root, "rev-parse", f"{revision}^{{commit}}").decode("ascii").strip()
    tree = _git(root, "rev-parse", f"{commit}^{{tree}}").decode("ascii").strip()
    if SHA1.fullmatch(commit) is None or SHA1.fullmatch(tree) is None:
        raise NativeQualificationError("source commit/tree is malformed")
    return {"commit": commit, "tree": tree}


def _commit_build_date(root: Path, commit: str) -> str:
    raw = _git(root, "show", "-s", "--format=%cI", commit).decode("ascii").strip()
    try:
        timestamp = datetime.fromisoformat(raw.replace("Z", "+00:00"))
    except ValueError as error:
        raise NativeQualificationError("target commit timestamp is not ISO-8601") from error
    if timestamp.tzinfo is None:
        raise NativeQualificationError("target commit timestamp has no timezone")
    return timestamp.astimezone(timezone.utc).strftime("%Y-%m-%d")


def _require_exact_checkout(root: Path, source: Mapping[str, str]) -> None:
    if _source(root, "HEAD") != dict(source):
        raise NativeQualificationError("native producer checkout differs from exact source")
    status = _git(root, "status", "--porcelain=v1", "--untracked-files=all")
    if status:
        raise NativeQualificationError("native producer requires a completely clean checkout")


def _file_record(path: Path, *, payload_sha256: str | None = None) -> dict[str, Any]:
    raw = Path(path)
    resolved = raw.resolve()
    if raw.is_symlink() or resolved.is_symlink() or not resolved.is_file():
        raise NativeQualificationError(f"evidence is not a regular non-symlink file: {raw}")
    result: dict[str, Any] = {
        "path": str(resolved),
        "sha256": digest_file(resolved),
        "size_bytes": resolved.stat().st_size,
    }
    if payload_sha256 is not None:
        result["payload_sha256"] = payload_sha256
    return result


def _validate_file_record(value: Any, where: str, *, sealed: bool = False) -> Path:
    record = _mapping(value, where)
    fields = {"path", "sha256", "size_bytes"} | ({"payload_sha256"} if sealed else set())
    _closed(record, fields, where)
    path = Path(str(record["path"])).resolve()
    if (
        path.is_symlink() or not path.is_file()
        or SHA256.fullmatch(str(record["sha256"])) is None
        or isinstance(record["size_bytes"], bool)
        or not isinstance(record["size_bytes"], int)
        or record["size_bytes"] < 0
        or path.stat().st_size != record["size_bytes"]
        or digest_file(path) != record["sha256"]
    ):
        raise NativeQualificationError(f"{where} bytes changed")
    if sealed and SHA256.fullmatch(str(record["payload_sha256"])) is None:
        raise NativeQualificationError(f"{where} payload digest is malformed")
    return path


def _read_sealed(path: Path, expected_type: str) -> tuple[Mapping[str, Any], Mapping[str, Any]]:
    try:
        document = _mapping(json.loads(path.read_text(encoding="ascii")), str(path))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise NativeQualificationError(f"cannot read sealed evidence {path}: {error}") from error
    _closed(document, {"payload", "seal"}, str(path))
    payload = _mapping(document["payload"], f"{path} payload")
    if document["seal"] != seal_payload(payload)["seal"]:
        raise NativeQualificationError(f"sealed evidence digest mismatch: {path}")
    observed = payload.get("receipt_type", payload.get("kind"))
    if observed != expected_type:
        raise NativeQualificationError(f"sealed evidence type mismatch: {path}")
    return document, payload


def _artifact(value: Any, platform: str, source: Mapping[str, str]) -> dict[str, Any]:
    record = _mapping(value, f"{platform} artifact")
    _closed(record, {"platform", "sha256", "size_bytes", "embedded_git_commit"}, f"{platform} artifact")
    if (
        record["platform"] != platform
        or SHA256.fullmatch(str(record["sha256"])) is None
        or isinstance(record["size_bytes"], bool)
        or not isinstance(record["size_bytes"], int)
        or record["size_bytes"] < 1
        or record["embedded_git_commit"] != source["commit"][:12]
    ):
        raise NativeQualificationError(f"{platform} artifact identity is invalid")
    return dict(record)


def run_macos_build(
    *, source_root: Path, source_sha: str, output_dir: Path, receipt: Path,
    timeout_seconds: int,
) -> Mapping[str, Any]:
    """Execute the exact committed macOS build/audit/ABI producer."""
    if timeout_seconds < 1:
        raise NativeQualificationError("native build timeout must be positive")
    root = source_root.resolve()
    source = _source(root, source_sha)
    _require_exact_checkout(root, source)
    build = (root / "scripts/build_macos.sh").resolve()
    if build.read_bytes() != _git(root, "show", f"{source['commit']}:scripts/build_macos.sh"):
        raise NativeQualificationError("macOS build producer differs from committed source")
    out = output_dir.resolve()
    out.mkdir(parents=True, exist_ok=True)
    stdout = out / "build_macos.stdout.log"
    stderr = out / "build_macos.stderr.log"
    if any(path.exists() for path in (stdout, stderr, receipt)):
        raise NativeQualificationError("native build outputs already exist")
    environment = os.environ.copy()
    environment["FEREG_GIT_COMMIT"] = source["commit"][:12]
    build_date = _commit_build_date(root, source["commit"])
    environment["FEREG_BUILD_DATE"] = build_date
    try:
        with stdout.open("xb") as stdout_handle, stderr.open("xb") as stderr_handle:
            completed = subprocess.run(
                [str(build)], cwd=root, env=environment, stdin=subprocess.DEVNULL,
                stdout=stdout_handle, stderr=stderr_handle, check=False,
                timeout=timeout_seconds,
            )
    except subprocess.TimeoutExpired as error:
        raise NativeQualificationError("macOS native build timed out") from error
    plugin = (root / "dist/fereg_macos.plugin").resolve()
    checksum = (root / "dist/fereg_macos.plugin.sha256").resolve()
    if completed.returncode != 0 or not plugin.is_file() or not checksum.is_file():
        raise NativeQualificationError("committed macOS build/audit/ABI producer failed")
    digest = digest_file(plugin)
    parts = checksum.read_text(encoding="ascii").strip().split()
    if (
        not parts or parts[0] != digest
        or source["commit"][:12].encode("ascii") not in plugin.read_bytes()
        or build_date.encode("ascii") not in plugin.read_bytes()
    ):
        raise NativeQualificationError("macOS build output is not checksum/source embedded")
    expected_line = f"Built universal {plugin}\n".encode("utf-8")
    if stdout.read_bytes().count(expected_line) != 1:
        raise NativeQualificationError("macOS build did not emit its exact terminal sentinel once")
    audit_program = (root / "scripts/audit_binary.py").resolve()
    loader_program = (root / "tests/mock_spi/run_actual_unix_plugin_test.sh").resolve()
    for relative, program in (
        ("scripts/audit_binary.py", audit_program),
        ("tests/mock_spi/run_actual_unix_plugin_test.sh", loader_program),
    ):
        if program.read_bytes() != _git(root, "show", f"{source['commit']}:{relative}"):
            raise NativeQualificationError(f"native verifier differs from committed source: {relative}")
    step_specs = {
        "audit": ([str(Path(sys.executable).resolve()), str(audit_program), str(plugin)], audit_program),
        "actual_loader": ([str(loader_program), str(plugin)], loader_program),
    }
    verification_commands: dict[str, Any] = {}
    for name, (argv, program) in step_specs.items():
        step_stdout = out / f"{name}.stdout.log"
        step_stderr = out / f"{name}.stderr.log"
        with step_stdout.open("xb") as stdout_handle, step_stderr.open("xb") as stderr_handle:
            step = subprocess.run(
                argv, cwd=root, stdin=subprocess.DEVNULL, stdout=stdout_handle,
                stderr=stderr_handle, check=False, timeout=timeout_seconds,
            )
        if step.returncode != 0:
            raise NativeQualificationError(f"independent macOS {name} verification failed")
        verification_commands[name] = {
            "argv": argv,
            "argv_sha256": digest_bytes(canonical_json_bytes(argv)),
            "program_sha256": digest_file(program),
            "exit_code": 0,
            "stdout": _file_record(step_stdout),
            "stderr": _file_record(step_stderr),
        }
    try:
        audit_report = json.loads((out / "audit.stdout.log").read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise NativeQualificationError("macOS audit did not retain a JSON report") from error
    if (
        audit_report.get("audit_passed") is not True
        or audit_report.get("format") != "macho"
        or set(audit_report.get("architectures", [])) != {"x86_64", "arm64"}
        or audit_report.get("unresolved_symbols") != []
        or (out / "actual_loader.stdout.log").read_text(encoding="utf-8").splitlines()
        != ["actual-plugin ABI/self-test harness: PASS"]
    ):
        raise NativeQualificationError("macOS audit/actual-loader outputs are not authoritative PASSes")
    payload = {
        "schema_version": 1,
        "kind": MACOS_BUILD_TYPE,
        "source": source,
        "producer": {
            "path": "scripts/build_macos.sh",
            "sha256": digest_file(build),
            "argv": [str(build)],
            "environment": {
                "FEREG_BUILD_DATE": build_date,
                "FEREG_GIT_COMMIT": source["commit"][:12],
            },
        },
        "build_date": build_date,
        "artifact": {
            "platform": "macos-universal", "sha256": digest,
            "size_bytes": plugin.stat().st_size,
            "embedded_git_commit": source["commit"][:12],
        },
        "files": {
            "artifact": _file_record(plugin),
            "checksum": _file_record(checksum),
            "stdout": _file_record(stdout),
            "stderr": _file_record(stderr),
        },
        "verification_commands": verification_commands,
        "exit_code": 0,
        "build_binary_audit_actual_loader": "success",
        "complete_sentinel": MACOS_SENTINEL,
    }
    _write_immutable(receipt.resolve(), seal_payload(payload))
    return payload


def verify_macos_build(path: Path, source_root: Path) -> Mapping[str, Any]:
    document, payload = _read_sealed(path, MACOS_BUILD_TYPE)
    _closed(
        payload,
        {
            "schema_version", "kind", "source", "producer", "artifact", "files",
            "verification_commands", "build_date",
            "exit_code", "build_binary_audit_actual_loader", "complete_sentinel",
        },
        "macOS native build payload",
    )
    source = _mapping(payload["source"], "macOS build source")
    _closed(source, {"commit", "tree"}, "macOS build source")
    if source != _source(source_root.resolve(), str(source["commit"])):
        raise NativeQualificationError("macOS build source differs from repository")
    producer = _mapping(payload["producer"], "macOS build producer")
    _closed(
        producer, {"path", "sha256", "argv", "environment"},
        "macOS build producer",
    )
    script = source_root.resolve() / "scripts/build_macos.sh"
    committed = _git(source_root.resolve(), "show", f"{source['commit']}:scripts/build_macos.sh")
    if (
        producer != {
            "path": "scripts/build_macos.sh", "sha256": digest_bytes(committed),
            "argv": [str(script.resolve())],
            "environment": {
                "FEREG_BUILD_DATE": _commit_build_date(
                    source_root.resolve(), str(source["commit"])
                ),
                "FEREG_GIT_COMMIT": str(source["commit"])[:12],
            },
        }
        or script.read_bytes() != committed
    ):
        raise NativeQualificationError("macOS build did not name the exact committed producer")
    files = _mapping(payload["files"], "macOS build files")
    _closed(files, {"artifact", "checksum", "stdout", "stderr"}, "macOS build files")
    paths = {name: _validate_file_record(record, f"macOS build {name}") for name, record in files.items()}
    artifact = _artifact(payload["artifact"], "macos-universal", source)
    commands = _mapping(payload["verification_commands"], "macOS verification commands")
    _closed(commands, {"audit", "actual_loader"}, "macOS verification commands")
    audit_program = source_root.resolve() / "scripts/audit_binary.py"
    loader_program = source_root.resolve() / "tests/mock_spi/run_actual_unix_plugin_test.sh"
    expected_steps = {
        "audit": ([str(Path(sys.executable).resolve()), str(audit_program), str(paths["artifact"])], audit_program),
        "actual_loader": ([str(loader_program), str(paths["artifact"])], loader_program),
    }
    step_paths: dict[str, dict[str, Path]] = {}
    for name, (argv, program) in expected_steps.items():
        record = _mapping(commands[name], f"macOS {name} command")
        _closed(
            record,
            {"argv", "argv_sha256", "program_sha256", "exit_code", "stdout", "stderr"},
            f"macOS {name} command",
        )
        relative = program.relative_to(source_root.resolve()).as_posix()
        committed_program = _git(
            source_root.resolve(), "show", f"{source['commit']}:{relative}"
        )
        if (
            record["argv"] != argv
            or record["argv_sha256"] != digest_bytes(canonical_json_bytes(argv))
            or record["program_sha256"] != digest_bytes(committed_program)
            or program.read_bytes() != committed_program
            or record["exit_code"] != 0
        ):
            raise NativeQualificationError(f"macOS {name} command is not exact/committed")
        step_paths[name] = {
            stream: _validate_file_record(record[stream], f"macOS {name} {stream}")
            for stream in ("stdout", "stderr")
        }
    try:
        audit_report = json.loads(step_paths["audit"]["stdout"].read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise NativeQualificationError("retained macOS audit report is not JSON") from error
    retained_loader_lines = step_paths["actual_loader"]["stdout"].read_text(
        encoding="utf-8"
    ).splitlines()
    if (
        paths["artifact"].name != "fereg_macos.plugin"
        or digest_file(paths["artifact"]) != artifact["sha256"]
        or paths["artifact"].stat().st_size != artifact["size_bytes"]
        or source["commit"][:12].encode("ascii") not in paths["artifact"].read_bytes()
        or payload["build_date"] != _commit_build_date(
            source_root.resolve(), str(source["commit"])
        )
        or str(payload["build_date"]).encode("ascii") not in paths["artifact"].read_bytes()
        or paths["checksum"].read_text(encoding="ascii").strip().split()[0] != artifact["sha256"]
        or paths["stdout"].read_bytes().count(
            f"Built universal {paths['artifact']}\n".encode("utf-8")
        ) != 1
        or audit_report.get("audit_passed") is not True
        or audit_report.get("format") != "macho"
        or set(audit_report.get("architectures", [])) != {"x86_64", "arm64"}
        or audit_report.get("unresolved_symbols") != []
        or retained_loader_lines != ["actual-plugin ABI/self-test harness: PASS"]
        or payload["schema_version"] != 1
        or payload["exit_code"] != 0
        or payload["build_binary_audit_actual_loader"] != "success"
        or payload["complete_sentinel"] != MACOS_SENTINEL
    ):
        raise NativeQualificationError("macOS build record differs from exact outputs")
    if platform.system() != "Darwin":
        raise NativeQualificationError(
            "macOS native evidence must be reverified on a target-native Darwin host"
        )
    rerun_audit = subprocess.run(
        expected_steps["audit"][0], cwd=source_root.resolve(),
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        check=False, timeout=300,
    )
    try:
        rerun_report = json.loads(rerun_audit.stdout.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise NativeQualificationError("fresh macOS binary audit did not return JSON") from error
    if (
        rerun_audit.returncode != 0 or rerun_report.get("audit_passed") is not True
        or rerun_report.get("format") != "macho"
        or set(rerun_report.get("architectures", [])) != {"x86_64", "arm64"}
    ):
        raise NativeQualificationError("fresh macOS binary audit failed")
    cc = shutil.which("cc")
    if cc is None:
        raise NativeQualificationError("target-native C compiler is unavailable for ABI recheck")
    loader_source = source_root.resolve() / "tests/mock_spi/actual_plugin_loader_test.c"
    committed_loader_source = _git(
        source_root.resolve(), "show",
        f"{source['commit']}:tests/mock_spi/actual_plugin_loader_test.c",
    )
    if loader_source.read_bytes() != committed_loader_source:
        raise NativeQualificationError("actual-loader C source differs from committed source")
    with tempfile.TemporaryDirectory(prefix="feppml-native-abi-verify-") as temporary:
        harness = Path(temporary) / "actual_plugin_loader_test"
        compile_result = subprocess.run(
            [
                cc, "-std=c11", "-O2", "-Wall", "-Wextra", "-Wpedantic",
                "-Wconversion", "-Wshadow", "-Wstrict-prototypes",
                "-Wmissing-prototypes", "-Werror", "-DSYSTEM=APPLEMAC",
                f"-I{source_root.resolve() / 'plugin'}", str(loader_source), "-lc",
                "-o", str(harness),
            ],
            cwd=source_root.resolve(), stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            raise NativeQualificationError("fresh target-native ABI harness did not compile")
        loader_result = subprocess.run(
            [str(harness), str(paths["artifact"])], stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, timeout=300,
        )
        if (
            loader_result.returncode != 0
            or loader_result.stdout.decode("utf-8").splitlines()
            != ["actual-plugin ABI/self-test harness: PASS"]
        ):
            raise NativeQualificationError("fresh target-native actual ABI loader failed")
    return payload


def _platform_modules(source_root: Path):
    import importlib.util

    def load(relative: str, name: str):
        path = source_root / relative
        spec = importlib.util.spec_from_file_location(name, path)
        if spec is None or spec.loader is None:
            raise NativeQualificationError(f"cannot import authoritative module: {relative}")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    return (
        load("scripts/feppml_platform_load_receipt.py", "_native_platform_receipt"),
        load("benchmarks/scc/feppml_linux_platform_scc.py", "_native_linux_scc"),
    )


def _platform_identity(
    payload: Mapping[str, Any], source: Mapping[str, str], source_root: Path
) -> dict[str, Any]:
    platform = str(payload.get("platform"))
    if platform not in PLATFORMS:
        raise NativeQualificationError("native platform payload has an unsupported platform")
    if (
        payload.get("qualification_profile") != "candidate"
        or payload.get("source_commit") != source["commit"]
        or payload.get("source_tree") != source["tree"]
        or payload.get("embedded_git_commit") != source["commit"][:12]
        or payload.get("build_date_utc") != _commit_build_date(
            source_root, source["commit"]
        )
        or (
            platform != "windows-x86_64"
            and (
                payload.get("controller_commit") != source["commit"]
                or payload.get("controller_tree") != source["tree"]
            )
        )
        or payload.get("licensed_stata_load") is not True
        or payload.get("mock_only") is not False
    ):
        raise NativeQualificationError(f"{platform} platform payload is source/load mixed")
    return _artifact(
        {
            "platform": platform,
            "sha256": payload.get("artifact_sha256"),
            "size_bytes": payload.get("artifact_size_bytes"),
            "embedded_git_commit": payload.get("embedded_git_commit"),
        },
        platform,
        source,
    )


def _verify_inputs(
    *, source_root: Path, source: Mapping[str, str], macos_build: Path,
    macos_record: Path, macos_receipt: Path, linux_close: Path,
    windows_record: Path, windows_receipt: Path,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    platform_receipts, linux_scc = _platform_modules(source_root)
    mac_build = verify_macos_build(macos_build, source_root)
    try:
        mac = platform_receipts.verify(macos_receipt, macos_record)
        windows = platform_receipts.verify(windows_receipt, windows_record)
        linux_root = linux_close.resolve().parent.parent
        linux = linux_scc.verify_closed_run(
            linux_root,
            candidate_repo=source_root,
            harness_repo=source_root,
            expected_source_commit=source["commit"],
            expected_harness_commit=source["commit"],
        )
    except Exception as error:
        raise NativeQualificationError(f"authoritative native platform verification failed: {error}") from error
    mac_artifact = _platform_identity(mac, source, source_root)
    win_artifact = _platform_identity(windows, source, source_root)
    linux_source = _mapping(linux.get("source"), "Linux closed-run source")
    linux_artifact_raw = _mapping(linux.get("artifact"), "Linux closed-run artifact")
    linux_artifact = _artifact(
        {
            "platform": "linux-x86_64",
            "sha256": linux_artifact_raw.get("sha256"),
            "size_bytes": linux_artifact_raw.get("size_bytes"),
            "embedded_git_commit": source["commit"][:12],
        },
        "linux-x86_64",
        source,
    )
    if (
        linux_source.get("commit") != source["commit"]
        or linux_source.get("tree") != source["tree"]
        or linux.get("build_date_utc") != _commit_build_date(
            source_root, source["commit"]
        )
        or linux.get("build_binary_audit_actual_loader") != "success"
        or mac_artifact != dict(mac_build["artifact"])
        or mac.get("platform_provenance", {}).get("record_type")
        != "feppml-native-capture-controller-v1"
        or windows.get("platform_provenance", {}).get("record_type")
        != "fereg-windows-controller-qualification-v1"
        or windows.get("platform_provenance", {}).get("target_source_commit")
        != source["commit"]
        or windows.get("platform_provenance", {}).get("target_source_tree")
        != source["tree"]
        or windows.get("platform_provenance", {}).get("harness_commit")
        != source["commit"]
        or windows.get("platform_provenance", {}).get("harness_tree")
        != source["tree"]
    ):
        raise NativeQualificationError("native platform authorities do not cross-bind")
    artifacts = sorted(
        [linux_artifact, mac_artifact, win_artifact], key=lambda item: item["platform"]
    )
    verified = {
        "build_date": _commit_build_date(source_root, source["commit"]),
        "macos_build_sha256": digest_file(macos_build),
        "linux_close_sha256": digest_file(linux_close),
        "linux_job_id": linux.get("job_id"),
        "windows_controller_receipt_sha256": windows["platform_provenance"].get(
            "controller_receipt_sha256"
        ),
    }
    return artifacts, verified


INPUT_FIELDS = {
    "macos_build", "macos_record", "macos_receipt", "linux_close",
    "windows_record", "windows_receipt",
}


def create(
    *, source_root: Path, source_sha: str, macos_build: Path,
    macos_record: Path, macos_receipt: Path, linux_close: Path,
    windows_record: Path, windows_receipt: Path, receipt: Path,
) -> Mapping[str, Any]:
    root = source_root.resolve()
    source = _source(root, source_sha)
    artifacts, verified = _verify_inputs(
        source_root=root, source=source, macos_build=macos_build,
        macos_record=macos_record, macos_receipt=macos_receipt,
        linux_close=linux_close, windows_record=windows_record,
        windows_receipt=windows_receipt,
    )
    inputs = {
        name: _file_record(path)
        for name, path in {
            "macos_build": macos_build, "macos_record": macos_record,
            "macos_receipt": macos_receipt, "linux_close": linux_close,
            "windows_record": windows_record, "windows_receipt": windows_receipt,
        }.items()
    }
    payload = {
        "schema_version": 1,
        "receipt_type": RECEIPT_TYPE,
        "source": source,
        "artifacts": artifacts,
        "inputs": inputs,
        "verified": verified,
        "claims": ["build", "binary_audit", "actual_abi_loader"],
        "status": "pass",
        "complete_sentinel": COMPLETE_SENTINEL,
    }
    _write_immutable(receipt.resolve(), seal_payload(payload))
    return payload


def verify(
    path: Path,
    *,
    source_root: Path = ROOT,
    expected_source: Mapping[str, str] | None = None,
    expected_artifacts: Sequence[Mapping[str, Any]] | None = None,
) -> Mapping[str, Any]:
    _document, payload = _read_sealed(path, RECEIPT_TYPE)
    _closed(
        payload,
        {
            "schema_version", "receipt_type", "source", "artifacts", "inputs",
            "verified", "claims", "status", "complete_sentinel",
        },
        "native qualification payload",
    )
    source = _mapping(payload["source"], "native qualification source")
    _closed(source, {"commit", "tree"}, "native qualification source")
    if source != _source(source_root.resolve(), str(source["commit"])):
        raise NativeQualificationError("native qualification source differs from repository")
    inputs = _mapping(payload["inputs"], "native qualification inputs")
    _closed(inputs, INPUT_FIELDS, "native qualification inputs")
    paths = {
        name: _validate_file_record(inputs[name], f"native qualification {name}")
        for name in sorted(INPUT_FIELDS)
    }
    artifacts, verified = _verify_inputs(
        source_root=source_root.resolve(), source=source,
        macos_build=paths["macos_build"], macos_record=paths["macos_record"],
        macos_receipt=paths["macos_receipt"], linux_close=paths["linux_close"],
        windows_record=paths["windows_record"],
        windows_receipt=paths["windows_receipt"],
    )
    if (
        payload["schema_version"] != 1
        or payload["artifacts"] != artifacts
        or payload["verified"] != verified
        or payload["claims"] != ["build", "binary_audit", "actual_abi_loader"]
        or payload["status"] != "pass"
        or payload["complete_sentinel"] != COMPLETE_SENTINEL
        or (expected_source is not None and dict(source) != dict(expected_source))
    ):
        raise NativeQualificationError("native qualification receipt is stale or incomplete")
    if expected_artifacts is not None:
        expected = sorted(
            [
                {
                    "platform": item["platform"], "sha256": item["sha256"],
                    "size_bytes": item["size_bytes"],
                    "embedded_git_commit": item["embedded_git_commit"],
                }
                for item in expected_artifacts
                if item.get("role") == "plugin"
            ],
            key=lambda item: item["platform"],
        )
        if artifacts != expected:
            raise NativeQualificationError("native artifacts differ from aggregate plugins")
    return payload


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    mac = commands.add_parser("run-macos")
    mac.add_argument("--source-root", type=Path, required=True)
    mac.add_argument("--source-sha", required=True)
    mac.add_argument("--output-dir", type=Path, required=True)
    mac.add_argument("--timeout-seconds", type=int, default=3600)
    mac.add_argument("--receipt", type=Path, required=True)
    seal = commands.add_parser("seal")
    seal.add_argument("--source-root", type=Path, required=True)
    seal.add_argument("--source-sha", required=True)
    for name in INPUT_FIELDS:
        seal.add_argument(f"--{name.replace('_', '-')}", type=Path, required=True)
    seal.add_argument("--receipt", type=Path, required=True)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--source-root", type=Path, default=ROOT)
    verify_parser.add_argument("--receipt", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    if args.command == "run-macos":
        run_macos_build(
            source_root=args.source_root, source_sha=args.source_sha,
            output_dir=args.output_dir, receipt=args.receipt,
            timeout_seconds=args.timeout_seconds,
        )
        print(MACOS_SENTINEL)
        return 0
    if args.command == "seal":
        create(
            source_root=args.source_root, source_sha=args.source_sha,
            macos_build=args.macos_build, macos_record=args.macos_record,
            macos_receipt=args.macos_receipt, linux_close=args.linux_close,
            windows_record=args.windows_record,
            windows_receipt=args.windows_receipt, receipt=args.receipt,
        )
        print(COMPLETE_SENTINEL)
        return 0
    verify(args.receipt, source_root=args.source_root)
    print(COMPLETE_SENTINEL)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
