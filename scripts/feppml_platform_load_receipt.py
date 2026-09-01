#!/usr/bin/env python3
"""Seal a licensed-Stata load receipt for one exact feppml plugin artifact.

The input capture is produced by a platform-native licensed Stata smoke driver.
This tool does not run or infer scientific parity.  It only proves that the
named artifact bytes loaded successfully in licensed Stata.  Mock SPI, build,
binary-audit, and unit-test records are explicitly ineligible.
"""
from __future__ import annotations

import argparse
import copy
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tarfile
from typing import Any, Mapping, Sequence
import zipfile


ROOT = Path(__file__).resolve().parents[1]

# Formal callers use ``python -I -B -S`` from arbitrary working directories.
# Make only the reviewed repository roots importable; ambient cwd and site
# packages remain excluded by the interpreter flags.
for import_root in (ROOT / "scripts", ROOT):
    import_root_text = str(import_root)
    if import_root_text not in sys.path:
        sys.path.insert(0, import_root_text)

try:
    from scripts import feppml_qualification as qualification
    from scripts.prepare_windows_feppml_target import (
        CONTRACT_PATHS as WINDOWS_CONTROLLER_CONTRACT_PATHS,
    )
except ImportError:
    import feppml_qualification as qualification  # type: ignore[no-redef]
    from prepare_windows_feppml_target import (  # type: ignore[no-redef]
        CONTRACT_PATHS as WINDOWS_CONTROLLER_CONTRACT_PATHS,
    )


PLATFORMS = ("macos-universal", "linux-x86_64", "windows-x86_64")
CAPTURE_TYPE = "feppml-licensed-stata-load-v1"
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
HEX_SHA1 = re.compile(r"^[0-9a-f]{40}$")
FILE_FIELDS = {"path", "sha256", "size_bytes"}
RECEIPT_FILE_FIELDS = FILE_FIELDS | {"payload_sha256"}
RECORD_TYPE = "feppml-platform-evidence-record-v1"
RECORD_FIELDS = {
    "record_type", "qualification_profile", "platform", "artifact", "capture", "source_archive",
    "capture_program", "receipt", "load_status",
    "licensed_stata_load", "mock_only", "scientific_claim", "receipt_scope",
    "source_commit", "source_tree", "source_archive_sha256",
    "source_archive_size_bytes", "embedded_git_commit", "build_date_utc",
    "controller_commit", "controller_tree", "platform_provenance",
}
QUALIFICATION_PROFILES = {"candidate", "f0-baseline"}

WINDOWS_CONTROLLER_RECEIPT_FIELDS = {
    "schema_version", "run_id", "project", "profile", "source_revision",
    "source_sha256", "windows_target", "execution_identity", "status",
    "error_code", "checks", "completed_at_utc", "results_sha256",
    "results_size_bytes",
}
WINDOWS_CONTROLLER_CHECKS = {
    "archive_paths", "execution_identity", "project_test", "shutdown_backstop",
    "source_hash", "source_manifest", "stata", "results_hash",
}
WINDOWS_MANIFEST_KIND = "feppml_windows_platform_source_manifest"
NATIVE_ATTESTATION_KIND = "feppml_native_capture_execution"
NATIVE_PRODUCER_RELATIVE = "scripts/capture_feppml_platform_load.py"
WINDOWS_OUTPUT_FIELDS = {
    "RUN_ID", "PROJECT", "PROFILE", "SOURCE_REVISION", "SOURCE_SHA256",
    "RECEIPT_SHA256", "WINDOWS_STATUS", "INSTANCE_STOPPED",
    "TRANSIENT_OBJECTS_DELETED", "LOCK_RELEASED", "RECEIPT_PATH",
    "RESULTS_SHA256", "RESULTS_SIZE_BYTES", "RESULTS_PATH", "WINDOWS_CI_RUN",
}


class PlatformReceiptError(ValueError):
    """A platform load capture or immutable receipt failed closed."""


def _stata_quoted(path: Path) -> str:
    text = str(path)
    if '"' in text:
        raise PlatformReceiptError("native Stata path contains an unsupported quote")
    return f'"{text}"'


def native_wrapper_text(
    *,
    capture_program: Path,
    artifact: Path,
    platform: str,
    artifact_sha256: str,
    artifact_size_bytes: int,
    capture_program_sha256: str,
    source_commit: str,
    embedded_git_commit: str,
    raw_capture: Path,
    status: Path,
    isolated_root: Path,
) -> str:
    """Return the one canonical native Stata wrapper accepted by the verifier."""

    lines = ["version 18.0", "clear all", "set more off"]
    for name in ("PERSONAL", "PLUS", "OLDPLACE", "SITE"):
        lines.append(
            f"sysdir set {name} {_stata_quoted(isolated_root / name.lower())}"
        )
    lines.extend(
        [
            "capture noisily do "
            f"{_stata_quoted(capture_program)} {_stata_quoted(artifact)} "
            f'"{platform}" "{artifact_sha256}" "{artifact_size_bytes}" '
            f'"{capture_program_sha256}" "{source_commit}" '
            f'"{embedded_git_commit}" {_stata_quoted(raw_capture)}',
            "local capture_rc = _rc",
            "local capture_stata_version = c(stata_version)",
            'local capture_stata_flavor `"`c(flavor)\'"\'',
            'local capture_stata_os `"`c(os)\'"\'',
            "tempname capture_status_handle",
            f"capture file open `capture_status_handle' using {_stata_quoted(status)}, write text",
            "local capture_status_open_rc = _rc",
            "if `capture_status_open_rc' == 0 {",
            '    file write `capture_status_handle\' "schema_version=1" _n',
            '    file write `capture_status_handle\' "completed=1" _n',
            '    file write `capture_status_handle\' "stata_rc=`capture_rc\'" _n',
            '    file write `capture_status_handle\' "stata_version=`capture_stata_version\'" _n',
            '    file write `capture_status_handle\' "stata_flavor=`capture_stata_flavor\'" _n',
            '    file write `capture_status_handle\' "stata_os=`capture_stata_os\'" _n',
            "    file close `capture_status_handle'",
            "}",
            "exit `capture_rc'",
            "",
        ]
    )
    return "\n".join(lines)


def _read(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise PlatformReceiptError(f"cannot read {where}: {error}") from error
    if not isinstance(value, Mapping):
        raise PlatformReceiptError(f"{where} must be an object")
    return value


def _write(path: Path, value: Any) -> None:
    data = json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True).encode("ascii") + b"\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise PlatformReceiptError(f"refusing to overwrite immutable output: {path}") from error
    with os.fdopen(descriptor, "wb") as target:
        target.write(data)
        target.flush()
        os.fsync(target.fileno())


def _file_record(path: Path) -> dict[str, Any]:
    source = Path(path)
    if source.is_symlink():
        raise PlatformReceiptError(
            f"platform prerequisite is not a regular non-symlink file: {source}"
        )
    resolved = source.resolve()
    if not resolved.is_file():
        raise PlatformReceiptError(
            f"platform prerequisite is not a regular non-symlink file: {resolved}"
        )
    return {
        "path": str(resolved),
        "sha256": qualification.sha256_file(resolved),
        "size_bytes": resolved.stat().st_size,
    }


def _regular_file(path: Path, where: str) -> Path:
    source = Path(path)
    if source.is_symlink():
        raise PlatformReceiptError(f"{where} is not a regular non-symlink file")
    resolved = source.resolve()
    if not resolved.is_file() or resolved.is_symlink():
        raise PlatformReceiptError(f"{where} is not a regular non-symlink file")
    return resolved


def _git(repo: Path, *arguments: str) -> bytes:
    try:
        return subprocess.check_output(
            ["git", "-C", str(repo), *arguments], stderr=subprocess.PIPE
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise PlatformReceiptError(
            f"git identity command failed: {' '.join(arguments)}"
        ) from error


def _validate_file_record(
    value: Any, where: str, *, receipt: bool = False
) -> tuple[Path, Mapping[str, Any]]:
    record = _read_record(value, where)
    expected = RECEIPT_FILE_FIELDS if receipt else FILE_FIELDS
    if set(record) != expected:
        raise PlatformReceiptError(f"{where} file record is not closed")
    source = Path(record["path"])
    path = source.resolve()
    if (
        source.is_symlink()
        or not path.is_file()
        or path.is_symlink()
        or path.stat().st_size != record["size_bytes"]
        or qualification.sha256_file(path) != record["sha256"]
    ):
        raise PlatformReceiptError(f"{where} bytes are absent or changed")
    if receipt and HEX_SHA256.fullmatch(str(record["payload_sha256"])) is None:
        raise PlatformReceiptError("platform receipt payload SHA-256 is malformed")
    return path, record


def _read_record(value: Any, where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise PlatformReceiptError(f"{where} must be an object")
    return value


CAPTURE_FIELDS = {
    "schema_version", "evidence_type", "platform", "artifact_sha256",
    "artifact_size_bytes", "stata_version", "stata_flavor", "license_status",
    "plugin_load_status", "plugin_load_rc", "loaded_plugin_sha256",
    "capture_program_sha256", "mock_only", "scientific_claim",
    "source_commit", "embedded_git_commit", "complete_sentinel",
}


def validate_capture(
    capture: Mapping[str, Any], artifact: Path, platform: str
) -> dict[str, Any]:
    if set(capture) != CAPTURE_FIELDS:
        raise PlatformReceiptError("licensed-load capture fields are not closed")
    if platform not in PLATFORMS or capture["platform"] != platform:
        raise PlatformReceiptError("platform identity is unsupported or mismatched")
    artifact_sha = qualification.sha256_file(artifact)
    artifact_size = artifact.stat().st_size
    if (
        capture["schema_version"] != 1
        or capture["evidence_type"] != CAPTURE_TYPE
        or capture["artifact_sha256"] != artifact_sha
        or capture["loaded_plugin_sha256"] != artifact_sha
        or capture["artifact_size_bytes"] != artifact_size
        or capture["complete_sentinel"] != "FEPPML_LICENSED_STATA_LOAD_COMPLETE"
    ):
        raise PlatformReceiptError("licensed-load capture is not bound to the exact artifact")
    if (
        capture["license_status"] != "licensed"
        or capture["plugin_load_status"] != "success"
        or capture["plugin_load_rc"] != 0
        or capture["mock_only"] is not False
        or capture["scientific_claim"] is not False
    ):
        raise PlatformReceiptError("mock, failed, unlicensed, or scientific-claim capture is ineligible")
    for field in ("stata_version", "stata_flavor"):
        if not isinstance(capture[field], str) or not capture[field]:
            raise PlatformReceiptError(f"capture.{field} must be nonempty")
    if not isinstance(capture["capture_program_sha256"], str) or HEX_SHA256.fullmatch(
        capture["capture_program_sha256"]
    ) is None:
        raise PlatformReceiptError("capture program SHA-256 is malformed")
    if (
        not isinstance(capture["source_commit"], str)
        or HEX_SHA1.fullmatch(capture["source_commit"]) is None
        or capture["embedded_git_commit"] != capture["source_commit"][:12]
        or capture["embedded_git_commit"].encode("ascii") not in artifact.read_bytes()
    ):
        raise PlatformReceiptError(
            "licensed-load capture is not bound to the artifact's embedded source"
        )
    return dict(capture)


def _source_identity(
    source_repo: Path,
    source_commit: str,
    source_archive: Path,
) -> tuple[dict[str, Any], Path]:
    repo = source_repo.resolve()
    if not (repo / ".git").exists() or HEX_SHA1.fullmatch(source_commit) is None:
        raise PlatformReceiptError("candidate source repository or full commit is invalid")
    resolved_commit = _git(repo, "rev-parse", f"{source_commit}^{{commit}}").decode(
        "ascii"
    ).strip()
    source_tree = _git(repo, "rev-parse", f"{source_commit}^{{tree}}").decode(
        "ascii"
    ).strip()
    if resolved_commit != source_commit or HEX_SHA1.fullmatch(source_tree) is None:
        raise PlatformReceiptError("candidate source commit/tree identity is malformed")
    try:
        commit_epoch = int(
            _git(repo, "show", "-s", "--format=%ct", source_commit).decode("ascii").strip()
        )
    except ValueError as error:
        raise PlatformReceiptError("candidate commit timestamp is malformed") from error
    build_date = datetime.fromtimestamp(commit_epoch, timezone.utc).date().isoformat()
    archive = _regular_file(source_archive, "candidate source archive")
    expected_archive = _git(repo, "archive", "--format=tar", source_commit)
    observed_archive = archive.read_bytes()
    if observed_archive != expected_archive:
        raise PlatformReceiptError(
            "candidate source archive bytes differ from the exact git archive"
        )
    source = {
        "source_commit": source_commit,
        "source_tree": source_tree,
        "source_archive_sha256": qualification.sha256_file(archive),
        "source_archive_size_bytes": archive.stat().st_size,
        "embedded_git_commit": source_commit[:12],
        "build_date_utc": build_date,
    }
    return source, archive


def _controller_identity(
    controller_repo: Path,
    controller_commit: str,
    capture_program: Path,
    expected_program_sha256: str,
) -> tuple[dict[str, str], Path]:
    repo = controller_repo.resolve()
    if not (repo / ".git").exists() or HEX_SHA1.fullmatch(controller_commit) is None:
        raise PlatformReceiptError("capture controller repository or commit is invalid")
    resolved_commit = _git(
        repo, "rev-parse", f"{controller_commit}^{{commit}}"
    ).decode("ascii").strip()
    tree = _git(repo, "rev-parse", f"{controller_commit}^{{tree}}").decode(
        "ascii"
    ).strip()
    program = _regular_file(capture_program, "capture controller program")
    try:
        relative = program.relative_to(repo).as_posix()
    except ValueError as error:
        raise PlatformReceiptError(
            "capture controller program is outside its exact repository"
        ) from error
    committed = _git(repo, "show", f"{controller_commit}:{relative}")
    if (
        resolved_commit != controller_commit
        or HEX_SHA1.fullmatch(tree) is None
        or committed != program.read_bytes()
        or qualification.sha256_file(program) != expected_program_sha256
    ):
        raise PlatformReceiptError(
            "capture program bytes differ from the exact controller commit"
        )
    return {"controller_commit": controller_commit, "controller_tree": tree}, program


def _native_producer(controller_repo: Path, controller_commit: str) -> Path:
    producer = _regular_file(
        controller_repo / NATIVE_PRODUCER_RELATIVE,
        "native capture controller producer",
    )
    if _git(
        controller_repo.resolve(), "show", f"{controller_commit}:{NATIVE_PRODUCER_RELATIVE}"
    ) != producer.read_bytes():
        raise PlatformReceiptError(
            "native capture producer differs from the exact controller commit"
        )
    return producer


def _read_windows_output(path: Path) -> Mapping[str, str]:
    output = _regular_file(path, "sanitized Windows controller output")
    values: dict[str, str] = {}
    for raw in output.read_text(encoding="ascii").splitlines():
        key, separator, value = raw.partition("=")
        if not separator or key in values:
            raise PlatformReceiptError("sanitized Windows controller output is malformed")
        values[key] = value
    if set(values) != WINDOWS_OUTPUT_FIELDS:
        raise PlatformReceiptError("sanitized Windows controller output fields are not closed")
    return values


def _windows_controller_archive(
    archive: Path, controller_commit: str, controller_tree: str
) -> tuple[Path, Mapping[str, Any], Mapping[str, Any]]:
    source = _regular_file(archive, "retained Windows controller archive")
    with tempfile.TemporaryDirectory(prefix="feppml-windows-controller-") as temporary:
        extracted = Path(temporary) / "source"
        extracted.mkdir()
        manifest: Mapping[str, Any] | None = None
        preparation: Mapping[str, Any] | None = None
        try:
            with zipfile.ZipFile(source) as zipped:
                seen: set[str] = set()
                for info in zipped.infolist():
                    name = info.filename
                    relative = Path(name)
                    if (
                        name in seen
                        or relative.is_absolute()
                        or ".." in relative.parts
                        or "\\" in name
                    ):
                        raise PlatformReceiptError(
                            "Windows controller archive inventory is unsafe"
                        )
                    seen.add(name)
                    if name == "windows-ci-source.json":
                        value = json.loads(zipped.read(info).decode("utf-8"))
                        if not isinstance(value, Mapping):
                            raise PlatformReceiptError(
                                "Windows controller source manifest is invalid"
                            )
                        manifest = value
                        continue
                    if name == "input/feppml-windows-preparation.json":
                        value = json.loads(zipped.read(info).decode("ascii"))
                        if (
                            not isinstance(value, Mapping)
                            or set(value) != {"payload", "seal"}
                            or not isinstance(value.get("payload"), Mapping)
                            or value["seal"]
                            != qualification.seal_payload(value["payload"])["seal"]
                        ):
                            raise PlatformReceiptError(
                                "Windows controller preparation seal is invalid"
                            )
                        preparation = value["payload"]
                    if name.endswith("/"):
                        (extracted / relative).mkdir(parents=True, exist_ok=True)
                        continue
                    mode = (info.external_attr >> 16) & 0o177777
                    if mode & 0o170000 == 0o120000:
                        raise PlatformReceiptError(
                            "Windows controller archive contains a symlink"
                        )
                    target = extracted / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(zipped.read(info))
                    target.chmod(0o755 if mode & 0o111 else 0o644)
        except (OSError, UnicodeError, zipfile.BadZipFile, json.JSONDecodeError) as error:
            raise PlatformReceiptError("Windows controller archive is invalid") from error
        if manifest != {
            "schema_version": 1,
            "project": "fereg",
            "source_revision": f"commit-{controller_commit}",
            "dirty": False,
        }:
            raise PlatformReceiptError(
                "Windows controller archive has no exact clean source manifest"
            )
        if (
            preparation is None
            or preparation.get("kind")
            != "feppml_windows_controller_preparation"
            or preparation.get("complete_sentinel")
            != "FEPPML_WINDOWS_CONTROLLER_PREPARED"
        ):
            raise PlatformReceiptError(
                "Windows controller archive omits its sealed two-source preparation"
            )
        target_identity = _read_record(
            preparation.get("target"), "Windows prepared target identity"
        )
        harness_identity = _read_record(
            preparation.get("harness"), "Windows prepared harness identity"
        )
        contracts = _read_record(
            preparation.get("contract_files"), "Windows controller contract files"
        )
        if (
            set(harness_identity) != {"commit", "tree"}
            or HEX_SHA1.fullmatch(str(harness_identity.get("commit"))) is None
            or HEX_SHA1.fullmatch(str(harness_identity.get("tree"))) is None
            or set(contracts) != set(WINDOWS_CONTROLLER_CONTRACT_PATHS)
        ):
            raise PlatformReceiptError(
                "Windows prepared harness/contract identity is not closed"
            )
        embedded_target = _regular_file(
            extracted / "input/feppml-windows-target-source.tar",
            "Windows embedded target archive",
        )
        if (
            qualification.sha256_file(embedded_target)
            != target_identity.get("archive_sha256")
            or embedded_target.stat().st_size
            != target_identity.get("archive_size_bytes")
        ):
            raise PlatformReceiptError(
                "Windows embedded target archive differs from preparation"
            )
        for relative in WINDOWS_CONTROLLER_CONTRACT_PATHS:
            record = _read_record(
                contracts[relative], f"Windows controller contract {relative}"
            )
            contract_path = _regular_file(
                extracted / relative, f"Windows controller contract {relative}"
            )
            if (
                set(record) != {"sha256", "size_bytes"}
                or qualification.sha256_file(contract_path) != record.get("sha256")
                or contract_path.stat().st_size != record.get("size_bytes")
            ):
                raise PlatformReceiptError(
                    f"Windows controller contract bytes changed: {relative}"
                )
        _git(extracted, "init", "-q")
        _git(extracted, "config", "core.autocrlf", "false")
        _git(extracted, "add", "-A")
        observed_tree = _git(extracted, "write-tree").decode("ascii").strip()
        if observed_tree != controller_tree:
            raise PlatformReceiptError(
                "Windows controller archive inventory differs from its exact tree"
            )
        _git(
            extracted,
            "update-index",
            "--force-remove",
            "--",
            "input/feppml-windows-target-source.tar",
            "input/feppml-windows-preparation.json",
        )
        observed_harness_tree = _git(extracted, "write-tree").decode("ascii").strip()
        if observed_harness_tree != harness_identity["tree"]:
            raise PlatformReceiptError(
                "Windows controller base bytes differ from the reviewed harness tree"
            )
    return source, manifest, preparation


def _windows_results_archive(
    archive: Path,
    *,
    artifact: Path,
    capture: Path,
    source_manifest: Path,
) -> Path:
    source = _regular_file(archive, "retained Windows controller results archive")
    expected = {
        "artifact/fereg_windows.plugin": artifact.read_bytes(),
        "evidence/capture.json": capture.read_bytes(),
        "evidence/source-manifest.json": source_manifest.read_bytes(),
    }
    try:
        with zipfile.ZipFile(source) as zipped:
            infos = zipped.infolist()
            names = [info.filename for info in infos]
            if len(names) != len(set(names)) or set(names) != set(expected):
                raise PlatformReceiptError(
                    "Windows results archive inventory is not closed"
                )
            for info in infos:
                relative = Path(info.filename)
                mode = (info.external_attr >> 16) & 0o177777
                if (
                    relative.is_absolute()
                    or ".." in relative.parts
                    or "\\" in info.filename
                    or info.is_dir()
                    or mode & 0o170000 == 0o120000
                    or zipped.read(info) != expected[info.filename]
                ):
                    raise PlatformReceiptError(
                        "Windows results archive bytes are unsafe or mismatched"
                    )
    except (OSError, zipfile.BadZipFile) as error:
        raise PlatformReceiptError("Windows results archive is invalid") from error
    return source


def _windows_provenance(
    *,
    controller_receipt: Path,
    controller_output: Path,
    controller_archive: Path,
    results_archive: Path,
    source_manifest: Path,
    source: Mapping[str, Any],
    controller: Mapping[str, str],
    artifact: Path,
    capture_path: Path,
    capture: Mapping[str, Any],
    capture_program: Path,
) -> dict[str, Any]:
    receipt_path = _regular_file(
        controller_receipt, "sanitized Windows controller receipt"
    )
    receipt = _read(receipt_path, "sanitized Windows controller receipt")
    archive_path, _controller_manifest, preparation = _windows_controller_archive(
        controller_archive,
        controller["controller_commit"],
        controller["controller_tree"],
    )
    if set(receipt) != WINDOWS_CONTROLLER_RECEIPT_FIELDS:
        raise PlatformReceiptError("Windows controller receipt fields are not closed")
    checks = _read_record(receipt.get("checks"), "Windows controller checks")
    if set(checks) != WINDOWS_CONTROLLER_CHECKS or any(
        checks[name] is not True for name in WINDOWS_CONTROLLER_CHECKS
    ):
        raise PlatformReceiptError("Windows controller checks are incomplete")
    expected_revision = f"commit-{controller['controller_commit']}"
    if (
        receipt.get("schema_version") != 1
        or receipt.get("project") != "fereg"
        or receipt.get("profile") != "stata-do"
        or receipt.get("source_revision") != expected_revision
        or receipt.get("source_sha256")
        != qualification.sha256_file(archive_path)
        or receipt.get("windows_target") != "x86_64"
        or receipt.get("execution_identity") != "SYSTEM"
        or receipt.get("status") != "PASS"
        or receipt.get("error_code") is not None
        or HEX_SHA256.fullmatch(str(receipt.get("results_sha256"))) is None
        or isinstance(receipt.get("results_size_bytes"), bool)
        or not isinstance(receipt.get("results_size_bytes"), int)
        or receipt["results_size_bytes"] < 1
        or re.fullmatch(
            r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z",
            str(receipt.get("completed_at_utc")),
        ) is None
        or not isinstance(receipt.get("run_id"), str)
        or re.fullmatch(r"win-[0-9]{8}T[0-9]{6}Z-[0-9a-f]{8}", receipt["run_id"])
        is None
    ):
        raise PlatformReceiptError("Windows controller receipt is not an exact clean PASS")
    output_path = _regular_file(
        controller_output, "sanitized Windows controller output"
    )
    output = _read_windows_output(output_path)
    receipt_sha = qualification.sha256_file(receipt_path)
    expected_output = {
        "RUN_ID": receipt["run_id"],
        "PROJECT": "fereg",
        "PROFILE": "stata-do",
        "SOURCE_REVISION": expected_revision,
        "SOURCE_SHA256": receipt["source_sha256"],
        "RECEIPT_SHA256": receipt_sha,
        "WINDOWS_STATUS": "PASS",
        "INSTANCE_STOPPED": "true",
        "TRANSIENT_OBJECTS_DELETED": "true",
        "LOCK_RELEASED": "true",
        "RECEIPT_PATH": output["RECEIPT_PATH"],
        "RESULTS_SHA256": receipt["results_sha256"],
        "RESULTS_SIZE_BYTES": str(receipt["results_size_bytes"]),
        "RESULTS_PATH": output["RESULTS_PATH"],
        "WINDOWS_CI_RUN": "PASS",
    }
    if output != expected_output:
        raise PlatformReceiptError(
            "sanitized Windows controller output differs from its PASS receipt"
        )
    if re.fullmatch(
        rf"/private/tmp/windows-ci-results/{re.escape(str(receipt['run_id']))}/receipt\.json",
        output["RECEIPT_PATH"],
    ) is None:
        raise PlatformReceiptError("Windows controller receipt path is not sanitized")
    if re.fullmatch(
        rf"/private/tmp/windows-ci-results/{re.escape(str(receipt['run_id']))}/results\.zip",
        output["RESULTS_PATH"],
    ) is None:
        raise PlatformReceiptError("Windows controller results path is not sanitized")
    manifest_path = _regular_file(
        source_manifest, "Windows source-side artifact/capture manifest"
    )
    manifest = _read(manifest_path, "Windows source-side artifact/capture manifest")
    if set(manifest) != {"payload", "seal"}:
        raise PlatformReceiptError("Windows source-side manifest envelope is not closed")
    payload = _read_record(manifest["payload"], "Windows source-side manifest payload")
    if manifest["seal"] != qualification.seal_payload(payload)["seal"]:
        raise PlatformReceiptError("Windows source-side manifest seal mismatch")
    target = preparation.get("target")
    harness = preparation.get("harness")
    if (
        set(preparation) != {
            "schema_version", "kind", "mode", "target", "harness",
            "contract_files", "complete_sentinel",
        }
        or preparation.get("schema_version") != 1
        or preparation.get("mode") not in {"candidate", "f0-baseline"}
        or not isinstance(target, Mapping)
        or not isinstance(harness, Mapping)
        or set(target) != {
            "commit", "tree", "archive_sha256", "archive_size_bytes",
            "embedded_git_commit", "build_date_utc",
        }
        or set(harness) != {"commit", "tree"}
        or not isinstance(preparation.get("contract_files"), Mapping)
    ):
        raise PlatformReceiptError("Windows two-source preparation is not closed")
    results_path = _windows_results_archive(
        results_archive,
        artifact=artifact,
        capture=capture_path,
        source_manifest=manifest_path,
    )
    if (
        qualification.sha256_file(results_path) != receipt["results_sha256"]
        or results_path.stat().st_size != receipt["results_size_bytes"]
    ):
        raise PlatformReceiptError(
            "Windows results archive differs from the controller receipt"
        )
    expected_payload = {
        "schema_version": 1,
        "kind": WINDOWS_MANIFEST_KIND,
        "mode": preparation["mode"],
        "platform": "windows-x86_64",
        "source_commit": source["source_commit"],
        "source_tree": source["source_tree"],
        "source_archive_sha256": source["source_archive_sha256"],
        "source_archive_size_bytes": source["source_archive_size_bytes"],
        "embedded_git_commit": source["embedded_git_commit"],
        "build_date_utc": source["build_date_utc"],
        "controller_commit": controller["controller_commit"],
        "run_id": receipt["run_id"],
        "harness_commit": harness["commit"],
        "harness_tree": harness["tree"],
        "artifact_sha256": qualification.sha256_file(artifact),
        "artifact_size_bytes": artifact.stat().st_size,
        "capture_sha256": qualification.sha256_file(capture_path),
        "capture_program_sha256": qualification.sha256_file(capture_program),
        "plugin_load_rc": 0,
        "licensed_stata_load": True,
        "mock_only": False,
        "scientific_claim": False,
        "complete_sentinel": "FEPPML_WINDOWS_PLATFORM_SOURCE_MANIFEST_COMPLETE",
    }
    if payload != expected_payload or capture["source_commit"] != source["source_commit"]:
        raise PlatformReceiptError(
            "Windows source-side manifest differs from artifact/capture bytes"
        )
    if (
        target
        != {
            "commit": source["source_commit"],
            "tree": source["source_tree"],
            "archive_sha256": source["source_archive_sha256"],
            "archive_size_bytes": source["source_archive_size_bytes"],
            "embedded_git_commit": source["embedded_git_commit"],
            "build_date_utc": source["build_date_utc"],
        }
        or set(harness) != {"commit", "tree"}
        or HEX_SHA1.fullmatch(str(harness["commit"])) is None
        or HEX_SHA1.fullmatch(str(harness["tree"])) is None
        or (
            preparation["mode"] == "f0-baseline"
            and target["commit"] == harness["commit"]
        )
        or (
            preparation["mode"] == "candidate"
            and (
                target["commit"] != harness["commit"]
                or target["tree"] != harness["tree"]
            )
        )
    ):
        raise PlatformReceiptError(
            "Windows controller preparation differs from target/harness identities"
        )
    return {
        "record_type": "fereg-windows-controller-qualification-v1",
        "qualification_profile": preparation["mode"],
        "controller_receipt": _file_record(receipt_path),
        "controller_output": _file_record(output_path),
        "controller_archive": _file_record(archive_path),
        "results_archive": _file_record(results_path),
        "source_manifest": _file_record(manifest_path),
        "controller_source_revision": expected_revision,
        "controller_source_archive_sha256": receipt["source_sha256"],
        "target_source_commit": source["source_commit"],
        "target_source_tree": source["source_tree"],
        "build_date_utc": source["build_date_utc"],
        "harness_commit": harness["commit"],
        "harness_tree": harness["tree"],
        "run_id": receipt["run_id"],
        "status": "PASS",
        "instance_stopped": True,
        "transient_objects_deleted": True,
        "lock_released": True,
    }


def _archive_identity(path: Path) -> tuple[str, str, str]:
    archive = _regular_file(path, "candidate source archive")
    try:
        with archive.open("rb") as source:
            completed = subprocess.run(
                ["git", "get-tar-commit-id"],
                stdin=source,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
                text=True,
            )
    except OSError as error:
        raise PlatformReceiptError("candidate source archive could not be inspected") from error
    commit = completed.stdout.strip()
    if completed.returncode or HEX_SHA1.fullmatch(commit) is None:
        raise PlatformReceiptError("candidate source archive has no exact git commit identity")
    try:
        with tarfile.open(archive, "r:") as source:
            members = source.getmembers()
    except (OSError, tarfile.TarError) as error:
        raise PlatformReceiptError("candidate source archive is not a valid tar") from error
    mtimes = {member.mtime for member in members}
    if not members or len(mtimes) != 1:
        raise PlatformReceiptError("candidate source archive timestamps are not canonical")
    build_date = datetime.fromtimestamp(mtimes.pop(), timezone.utc).date().isoformat()
    files: dict[tuple[str, ...], tuple[str, bytes]] = {}
    directories: set[tuple[str, ...]] = {()}
    for member in members:
        name = member.name.rstrip("/") if member.isdir() else member.name
        parts = tuple(part for part in name.split("/") if part)
        if (
            not parts
            or name.startswith("/")
            or any(part in {".", ".."} for part in parts)
            or parts in files
        ):
            raise PlatformReceiptError("candidate source archive member inventory is unsafe")
        for index in range(len(parts)):
            directories.add(parts[:index])
        if member.isdir():
            directories.add(parts)
            continue
        if member.isfile():
            try:
                with tarfile.open(archive, "r:") as reopened:
                    opened = reopened.extractfile(member)
                    content = None if opened is None else opened.read()
            except (OSError, tarfile.TarError) as error:
                raise PlatformReceiptError(
                    "candidate source archive file is unreadable"
                ) from error
            if opened is None:
                raise PlatformReceiptError("candidate source archive file is unreadable")
            assert content is not None
            mode = "100755" if member.mode & 0o111 else "100644"
        elif member.issym():
            content = member.linkname.encode("utf-8")
            mode = "120000"
        else:
            raise PlatformReceiptError("candidate source archive member type is unsupported")
        if parts in directories:
            raise PlatformReceiptError("candidate source archive has a path collision")
        files[parts] = (mode, content)
    with tempfile.TemporaryDirectory(prefix="feppml-archive-tree-") as temporary:
        git_dir = Path(temporary) / "objects.git"
        try:
            subprocess.run(
                ["git", "init", "--bare", "-q", str(git_dir)],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            blobs: dict[tuple[str, ...], tuple[str, str]] = {}
            for parts, (mode, content) in files.items():
                digest = subprocess.check_output(
                    ["git", "--git-dir", str(git_dir), "hash-object", "-w", "--stdin"],
                    input=content,
                    stderr=subprocess.PIPE,
                ).decode("ascii").strip()
                blobs[parts] = (mode, digest)
            trees: dict[tuple[str, ...], str] = {}
            all_directories = directories | {
                parts[:index] for parts in files for index in range(len(parts))
            }
            for directory in sorted(all_directories, key=len, reverse=True):
                entries: list[tuple[str, str, str, str]] = []
                for parts, (mode, digest) in blobs.items():
                    if parts[:-1] == directory:
                        entries.append((parts[-1], mode, "blob", digest))
                for child, digest in trees.items():
                    if child[:-1] == directory:
                        entries.append((child[-1], "040000", "tree", digest))
                data = b"".join(
                    f"{mode} {kind} {digest}\t{name}".encode("utf-8") + b"\0"
                    for name, mode, kind, digest in sorted(entries)
                )
                trees[directory] = subprocess.check_output(
                    ["git", "--git-dir", str(git_dir), "mktree", "-z"],
                    input=data,
                    stderr=subprocess.PIPE,
                ).decode("ascii").strip()
        except (OSError, subprocess.CalledProcessError, UnicodeError) as error:
            raise PlatformReceiptError(
                "candidate source archive tree could not be reconstructed"
            ) from error
    return commit, trees[()], build_date


def native_sidecar_paths(attestation: Path) -> dict[str, Path]:
    base = Path(str(attestation.resolve()))
    return {
        "status": Path(f"{base}.status"),
        "stata_log": Path(f"{base}.stata.log"),
        "wrapper": Path(f"{base}.wrapper.do"),
        "invocation": Path(f"{base}.invocation.json"),
    }


def _native_provenance(
    *,
    attestation: Path,
    stata_executable: Path | None,
    status_file: Path,
    stata_log: Path,
    wrapper: Path,
    invocation: Path,
    platform: str,
    source: Mapping[str, Any],
    controller: Mapping[str, str],
    artifact: Path,
    capture_path: Path,
    capture: Mapping[str, Any],
    capture_program: Path,
    producer: Path,
) -> dict[str, Any]:
    attestation_path = _regular_file(attestation, "native capture attestation")
    sidecars = {
        "status": _regular_file(status_file, "native Stata status"),
        "stata_log": _regular_file(
            stata_log, "native retained Stata log"
        ),
        "wrapper": _regular_file(wrapper, "native Stata wrapper"),
        "invocation": _regular_file(invocation, "native capture invocation"),
    }
    stata_path = (
        None
        if stata_executable is None
        else _regular_file(stata_executable, "licensed Stata executable")
    )
    document = _read(attestation_path, "native capture attestation")
    if set(document) != {"payload", "seal"}:
        raise PlatformReceiptError("native capture attestation envelope is not closed")
    payload = _read_record(document["payload"], "native capture attestation payload")
    if document["seal"] != qualification.seal_payload(payload)["seal"]:
        raise PlatformReceiptError("native capture attestation seal mismatch")
    expected_fields = {
        "schema_version", "kind", "platform", "source_commit", "source_tree",
        "source_archive_sha256", "source_archive_size_bytes",
        "embedded_git_commit", "build_date_utc", "controller_commit",
        "controller_tree",
        "artifact_sha256", "artifact_size_bytes", "capture_sha256",
        "capture_program_sha256", "stata_executable_sha256",
        "producer_sha256", "producer_size_bytes",
        "stata_executable_size_bytes", "stata_version", "stata_flavor",
        "process_returncode", "stata_rc", "status_sha256", "status_size_bytes",
        "raw_log_sha256", "raw_log_size_bytes",
        "wrapper_sha256", "wrapper_size_bytes",
        "invocation_sha256", "invocation_size_bytes",
        "terminal_sentinel_count", "embedded_commit_line_count",
        "licensed_stata_load", "mock_only", "scientific_claim",
        "complete_sentinel",
    }
    if set(payload) != expected_fields:
        raise PlatformReceiptError("native capture attestation fields are not closed")
    expected = {
        "schema_version": 1,
        "kind": NATIVE_ATTESTATION_KIND,
        "platform": platform,
        **source,
        **controller,
        "artifact_sha256": qualification.sha256_file(artifact),
        "artifact_size_bytes": artifact.stat().st_size,
        "capture_sha256": qualification.sha256_file(capture_path),
        "capture_program_sha256": qualification.sha256_file(capture_program),
        "producer_sha256": qualification.sha256_file(producer),
        "producer_size_bytes": producer.stat().st_size,
        "stata_executable_sha256": (
            payload.get("stata_executable_sha256")
            if stata_path is None
            else qualification.sha256_file(stata_path)
        ),
        "stata_executable_size_bytes": (
            payload.get("stata_executable_size_bytes")
            if stata_path is None
            else stata_path.stat().st_size
        ),
        "stata_version": capture["stata_version"],
        "stata_flavor": capture["stata_flavor"],
        "process_returncode": 0,
        "stata_rc": 0,
        "status_sha256": qualification.sha256_file(sidecars["status"]),
        "status_size_bytes": sidecars["status"].stat().st_size,
        "raw_log_sha256": qualification.sha256_file(sidecars["stata_log"]),
        "raw_log_size_bytes": sidecars["stata_log"].stat().st_size,
        "wrapper_sha256": qualification.sha256_file(sidecars["wrapper"]),
        "wrapper_size_bytes": sidecars["wrapper"].stat().st_size,
        "invocation_sha256": qualification.sha256_file(sidecars["invocation"]),
        "invocation_size_bytes": sidecars["invocation"].stat().st_size,
        "terminal_sentinel_count": 1,
        "embedded_commit_line_count": 1,
        "licensed_stata_load": True,
        "mock_only": False,
        "scientific_claim": False,
        "complete_sentinel": "FEPPML_NATIVE_CAPTURE_EXECUTION_COMPLETE",
    }
    for field in (
        "status_sha256", "raw_log_sha256",
        "wrapper_sha256", "invocation_sha256", "stata_executable_sha256",
        "producer_sha256",
    ):
        if HEX_SHA256.fullmatch(str(payload.get(field))) is None:
            raise PlatformReceiptError(f"native capture {field} is malformed")
    if (
        isinstance(payload.get("stata_executable_size_bytes"), bool)
        or not isinstance(payload.get("stata_executable_size_bytes"), int)
        or payload["stata_executable_size_bytes"] < 1
    ):
        raise PlatformReceiptError("native capture Stata executable size is malformed")
    if payload != expected:
        raise PlatformReceiptError("native capture attestation differs from exact inputs")
    status_values: dict[str, str] = {}
    for line in sidecars["status"].read_text(encoding="ascii").splitlines():
        key, separator, value = line.partition("=")
        if not separator or key in status_values:
            raise PlatformReceiptError("native Stata status is malformed")
        status_values[key] = value
    if (
        status_values
        != {
            "schema_version": "1",
            "completed": "1",
            "stata_rc": "0",
            "stata_version": capture["stata_version"],
            "stata_flavor": capture["stata_flavor"],
            "stata_os": status_values.get("stata_os", ""),
        }
        or not status_values["stata_os"]
        or qualification.sha256_file(sidecars["status"])
        != payload["status_sha256"]
    ):
        raise PlatformReceiptError("native Stata status differs from the attestation")
    log_lines = [line.strip() for line in sidecars["stata_log"].read_bytes().splitlines()]
    if (
        log_lines.count(
            f"fereg.git_commit={source['embedded_git_commit']}".encode("ascii")
        ) != 1
        or log_lines.count(b"FEPPML_LICENSED_STATA_LOAD_COMPLETE") != 1
    ):
        raise PlatformReceiptError("native Stata log is incomplete or duplicated")
    invocation_value = _read(sidecars["invocation"], "native capture invocation")
    invocation_fields = {
        "schema_version", "kind", "platform", "source_commit",
        "controller_commit", "artifact_sha256", "capture_program_sha256",
        "producer_sha256", "stata_executable_sha256",
        "stata_executable_size_bytes", "process_returncode", "wrapper_sha256",
        "cwd", "stata_executable", "capture_program", "artifact",
        "raw_capture", "status", "isolated_root", "wrapper", "argv",
        "complete_sentinel",
    }
    runtime_paths: dict[str, Path] = {}
    for field in (
        "cwd", "stata_executable", "capture_program", "artifact",
        "raw_capture", "status", "isolated_root", "wrapper",
    ):
        value = invocation_value.get(field)
        if not isinstance(value, str):
            raise PlatformReceiptError("native capture invocation path is malformed")
        runtime_paths[field] = Path(value)
        if not runtime_paths[field].is_absolute():
            raise PlatformReceiptError("native capture invocation path is not absolute")
    expected_argv = [
        str(runtime_paths["stata_executable"]), "-q", "-b", "do",
        str(runtime_paths["wrapper"]),
    ]
    expected_wrapper = native_wrapper_text(
        capture_program=runtime_paths["capture_program"],
        artifact=runtime_paths["artifact"],
        platform=platform,
        artifact_sha256=qualification.sha256_file(artifact),
        artifact_size_bytes=artifact.stat().st_size,
        capture_program_sha256=qualification.sha256_file(capture_program),
        source_commit=str(source["source_commit"]),
        embedded_git_commit=str(source["embedded_git_commit"]),
        raw_capture=runtime_paths["raw_capture"],
        status=runtime_paths["status"],
        isolated_root=runtime_paths["isolated_root"],
    ).encode("utf-8")
    if (
        set(invocation_value) != invocation_fields
        or invocation_value.get("schema_version") != 1
        or invocation_value.get("kind") != "feppml-native-capture-invocation-v1"
        or invocation_value.get("platform") != platform
        or invocation_value.get("source_commit") != source["source_commit"]
        or invocation_value.get("controller_commit") != controller["controller_commit"]
        or invocation_value.get("artifact_sha256")
        != qualification.sha256_file(artifact)
        or invocation_value.get("capture_program_sha256")
        != qualification.sha256_file(capture_program)
        or invocation_value.get("producer_sha256")
        != qualification.sha256_file(producer)
        or invocation_value.get("stata_executable_sha256")
        != payload["stata_executable_sha256"]
        or invocation_value.get("stata_executable_size_bytes")
        != payload["stata_executable_size_bytes"]
        or invocation_value.get("process_returncode") != 0
        or invocation_value.get("wrapper_sha256")
        != qualification.sha256_file(sidecars["wrapper"])
        or runtime_paths["capture_program"].name != capture_program.name
        or runtime_paths["artifact"].name != artifact.name
        or runtime_paths["raw_capture"].parent != runtime_paths["cwd"]
        or runtime_paths["status"].parent != runtime_paths["cwd"]
        or runtime_paths["isolated_root"] != runtime_paths["cwd"] / "sysdirs"
        or runtime_paths["wrapper"] != runtime_paths["cwd"] / "run.do"
        or invocation_value.get("argv") != expected_argv
        or sidecars["wrapper"].read_bytes() != expected_wrapper
        or invocation_value.get("complete_sentinel")
        != "FEPPML_NATIVE_CAPTURE_INVOCATION_COMPLETE"
    ):
        raise PlatformReceiptError("native capture invocation is not exact")
    return {
        "record_type": "feppml-native-capture-controller-v1",
        "producer": _file_record(producer),
        "attestation": _file_record(attestation_path),
        **{name: _file_record(path) for name, path in sidecars.items()},
    }


def _provenance_identity(
    provenance: Mapping[str, Any],
    *,
    platform: str,
    source: Mapping[str, Any],
    controller: Mapping[str, str],
    artifact: Path,
    capture_path: Path,
    capture: Mapping[str, Any],
    capture_program: Path,
) -> dict[str, Any]:
    record_type = provenance.get("record_type")
    if platform in {"macos-universal", "linux-x86_64"}:
        expected_fields = {
            "record_type", "producer", "attestation", "status", "stata_log",
            "wrapper", "invocation",
        }
        if platform == "linux-x86_64":
            expected_fields.add("outer_authority")
        if (
            set(provenance) != expected_fields
            or record_type != "feppml-native-capture-controller-v1"
            or (
                platform == "linux-x86_64"
                and provenance.get("outer_authority") != "scc-closed-run"
            )
        ):
            raise PlatformReceiptError(
                f"native {platform} capture provenance is not closed"
            )
        attestation_path, _attestation_record = _validate_file_record(
            provenance["attestation"], "native capture attestation"
        )
        producer, _producer_record = _validate_file_record(
            provenance["producer"], "native capture producer"
        )
        sidecars = {}
        for field in ("status", "stata_log", "wrapper", "invocation"):
            sidecars[field], _record = _validate_file_record(
                provenance[field], f"native capture {field}"
            )
        observed = _native_provenance(
            attestation=attestation_path,
            stata_executable=None,
            status_file=sidecars["status"],
            stata_log=sidecars["stata_log"],
            wrapper=sidecars["wrapper"],
            invocation=sidecars["invocation"],
            platform=platform,
            source=source,
            controller=controller,
            artifact=artifact,
            capture_path=capture_path,
            capture=capture,
            capture_program=capture_program,
            producer=producer,
        )
        if platform == "linux-x86_64":
            observed["outer_authority"] = "scc-closed-run"
        if provenance != observed:
            raise PlatformReceiptError("native capture provenance record changed")
        return {
            "record_type": observed["record_type"],
            "producer_sha256": observed["producer"]["sha256"],
            "producer_size_bytes": observed["producer"]["size_bytes"],
            "attestation_sha256": observed["attestation"]["sha256"],
            "attestation_size_bytes": observed["attestation"]["size_bytes"],
            "status_sha256": observed["status"]["sha256"],
            "status_size_bytes": observed["status"]["size_bytes"],
            "raw_log_sha256": observed["stata_log"]["sha256"],
            "raw_log_size_bytes": observed["stata_log"]["size_bytes"],
            "wrapper_sha256": observed["wrapper"]["sha256"],
            "wrapper_size_bytes": observed["wrapper"]["size_bytes"],
            "invocation_sha256": observed["invocation"]["sha256"],
            "invocation_size_bytes": observed["invocation"]["size_bytes"],
            **(
                {"outer_authority": "scc-closed-run"}
                if platform == "linux-x86_64"
                else {}
            ),
        }
    expected_fields = {
        "record_type", "qualification_profile", "controller_receipt", "controller_output",
        "controller_archive", "results_archive",
        "source_manifest", "controller_source_revision",
        "controller_source_archive_sha256", "run_id", "status",
        "target_source_commit", "target_source_tree", "build_date_utc",
        "harness_commit",
        "harness_tree",
        "instance_stopped", "transient_objects_deleted", "lock_released",
    }
    if set(provenance) != expected_fields or record_type != (
        "fereg-windows-controller-qualification-v1"
    ):
        raise PlatformReceiptError("Windows controller provenance is not closed or typed")
    paths: dict[str, Path] = {}
    for field in (
        "controller_receipt", "controller_output", "controller_archive",
        "results_archive",
        "source_manifest",
    ):
        paths[field], _record = _validate_file_record(
            provenance[field], f"Windows provenance {field}"
        )
    observed = _windows_provenance(
        controller_receipt=paths["controller_receipt"],
        controller_output=paths["controller_output"],
        controller_archive=paths["controller_archive"],
        results_archive=paths["results_archive"],
        source_manifest=paths["source_manifest"],
        source=source,
        controller=controller,
        artifact=artifact,
        capture_path=capture_path,
        capture=capture,
        capture_program=capture_program,
    )
    if provenance != observed:
        raise PlatformReceiptError("Windows controller provenance record changed")
    return {
        "record_type": observed["record_type"],
        "qualification_profile": observed["qualification_profile"],
        "controller_receipt_sha256": observed["controller_receipt"]["sha256"],
        "controller_receipt_size_bytes": observed["controller_receipt"]["size_bytes"],
        "controller_output_sha256": observed["controller_output"]["sha256"],
        "controller_output_size_bytes": observed["controller_output"]["size_bytes"],
        "controller_archive_sha256": observed["controller_archive"]["sha256"],
        "controller_archive_size_bytes": observed["controller_archive"]["size_bytes"],
        "results_archive_sha256": observed["results_archive"]["sha256"],
        "results_archive_size_bytes": observed["results_archive"]["size_bytes"],
        "source_manifest_sha256": observed["source_manifest"]["sha256"],
        "source_manifest_size_bytes": observed["source_manifest"]["size_bytes"],
        "controller_source_revision": observed["controller_source_revision"],
        "controller_source_archive_sha256": observed[
            "controller_source_archive_sha256"
        ],
        "target_source_commit": observed["target_source_commit"],
        "target_source_tree": observed["target_source_tree"],
        "build_date_utc": observed["build_date_utc"],
        "harness_commit": observed["harness_commit"],
        "harness_tree": observed["harness_tree"],
        "run_id": observed["run_id"],
        "status": "PASS",
        "instance_stopped": True,
        "transient_objects_deleted": True,
        "lock_released": True,
    }


def _qualification_profile(
    platform: str,
    source: Mapping[str, Any],
    controller: Mapping[str, str],
    provenance: Mapping[str, Any],
) -> str:
    if platform == "windows-x86_64":
        profile = provenance.get("qualification_profile")
        if profile not in QUALIFICATION_PROFILES:
            raise PlatformReceiptError(
                "Windows qualification profile is absent from controller provenance"
            )
        return str(profile)
    same_controller = (
        source["source_commit"] == controller["controller_commit"]
        and source["source_tree"] == controller["controller_tree"]
    )
    return "candidate" if same_controller else "f0-baseline"


def create(
    *,
    platform: str,
    artifact: Path,
    capture_path: Path,
    receipt: Path,
    record: Path,
    source_repo: Path,
    source_commit: str,
    source_archive: Path,
    controller_repo: Path,
    controller_commit: str,
    capture_program: Path,
    native_attestation: Path | None = None,
    native_stata_executable: Path | None = None,
    windows_controller_receipt: Path | None = None,
    windows_controller_output: Path | None = None,
    windows_controller_archive: Path | None = None,
    windows_results_archive: Path | None = None,
    windows_source_manifest: Path | None = None,
) -> dict[str, Any]:
    artifact_path = _regular_file(artifact, "platform artifact")
    capture_file = _regular_file(capture_path, "licensed-load capture")
    capture = validate_capture(
        _read(capture_file, "licensed-load capture"), artifact_path, platform
    )
    source, archive = _source_identity(source_repo, source_commit, source_archive)
    controller, program = _controller_identity(
        controller_repo,
        controller_commit,
        capture_program,
        str(capture["capture_program_sha256"]),
    )
    producer = (
        None
        if platform == "windows-x86_64"
        else _native_producer(controller_repo.resolve(), controller_commit)
    )
    if (
        capture["source_commit"] != source["source_commit"]
        or capture["embedded_git_commit"] != source["embedded_git_commit"]
    ):
        raise PlatformReceiptError("capture source differs from the exact build source")
    windows_inputs = (
        windows_controller_receipt,
        windows_controller_output,
        windows_controller_archive,
        windows_results_archive,
        windows_source_manifest,
    )
    if platform == "windows-x86_64":
        if native_attestation is not None or native_stata_executable is not None:
            raise PlatformReceiptError(
                "native macOS/Linux evidence cannot be attached to Windows"
            )
        if not all(value is not None for value in windows_inputs):
            raise PlatformReceiptError(
                "Windows evidence requires controller receipt/output and source manifest"
            )
        provenance = _windows_provenance(
            controller_receipt=windows_controller_receipt,
            controller_output=windows_controller_output,
            controller_archive=windows_controller_archive,
            results_archive=windows_results_archive,
            source_manifest=windows_source_manifest,
            source=source,
            controller=controller,
            artifact=artifact_path,
            capture_path=capture_file,
            capture=capture,
            capture_program=program,
        )
    else:
        if any(value is not None for value in windows_inputs):
            raise PlatformReceiptError(
                "Windows controller evidence cannot be attached to another platform"
            )
        if native_attestation is None or (
            platform == "macos-universal" and native_stata_executable is None
        ):
            raise PlatformReceiptError(
                f"{platform} evidence requires its native execution attestation"
            )
        sidecars = native_sidecar_paths(native_attestation)
        assert producer is not None
        provenance = _native_provenance(
            attestation=native_attestation,
            stata_executable=native_stata_executable,
            status_file=sidecars["status"],
            stata_log=sidecars["stata_log"],
            wrapper=sidecars["wrapper"],
            invocation=sidecars["invocation"],
            platform=platform,
            source=source,
            controller=controller,
            artifact=artifact_path,
            capture_path=capture_file,
            capture=capture,
            capture_program=program,
            producer=producer,
        )
        if platform == "linux-x86_64":
            provenance["outer_authority"] = "scc-closed-run"
    provenance_identity = _provenance_identity(
        provenance,
        platform=platform,
        source=source,
        controller=controller,
        artifact=artifact_path,
        capture_path=capture_file,
        capture=capture,
        capture_program=program,
    )
    qualification_profile = _qualification_profile(
        platform, source, controller, provenance
    )
    payload = {
        "schema_version": 1,
        "kind": "feppml_platform_licensed_stata_load",
        "qualification_profile": qualification_profile,
        "platform": platform,
        "artifact_sha256": capture["artifact_sha256"],
        "artifact_size_bytes": capture["artifact_size_bytes"],
        "stata_version": capture["stata_version"],
        "stata_flavor": capture["stata_flavor"],
        "capture_program_sha256": capture["capture_program_sha256"],
        "capture_output_sha256": qualification.sha256_file(capture_file),
        **source,
        **controller,
        "platform_provenance": provenance_identity,
        "licensed_stata_load": True,
        "mock_only": False,
        "scientific_claim": False,
        "receipt_scope": "artifact_and_licensed_stata_load_only",
        "complete_sentinel": "FEPPML_PLATFORM_LOAD_RECEIPT_COMPLETE",
    }
    sealed = qualification.seal_payload(payload)
    _write(receipt, sealed)
    receipt_file = _file_record(receipt)
    receipt_file["payload_sha256"] = sealed["seal"]["payload_sha256"]
    output = {
        "record_type": RECORD_TYPE,
        "qualification_profile": qualification_profile,
        "platform": platform,
        **source,
        **controller,
        "artifact": _file_record(artifact),
        "capture": _file_record(capture_path),
        "source_archive": _file_record(archive),
        "capture_program": _file_record(program),
        "receipt": receipt_file,
        "platform_provenance": provenance,
        "load_status": "success",
        "licensed_stata_load": True,
        "mock_only": False,
        "scientific_claim": False,
        "receipt_scope": "artifact_and_licensed_stata_load_only",
    }
    _verify_evidence(receipt, output)
    _write(record, output)
    return output


def _verify_evidence(
    receipt: Path, evidence: Mapping[str, Any]
) -> Mapping[str, Any]:
    if set(evidence) != RECORD_FIELDS:
        raise PlatformReceiptError("platform evidence record fields are not closed")
    if evidence.get("record_type") != RECORD_TYPE:
        raise PlatformReceiptError("platform evidence record type is invalid")
    if evidence.get("qualification_profile") not in QUALIFICATION_PROFILES:
        raise PlatformReceiptError("platform qualification profile is invalid")
    artifact_path, artifact_record = _validate_file_record(
        evidence["artifact"], "platform artifact"
    )
    capture_path, capture_record = _validate_file_record(
        evidence["capture"], "platform capture"
    )
    archive_path, archive_record = _validate_file_record(
        evidence["source_archive"], "candidate source archive"
    )
    program_path, program_record = _validate_file_record(
        evidence["capture_program"], "capture controller program"
    )
    receipt_path, receipt_record = _validate_file_record(
        evidence["receipt"], "platform receipt", receipt=True
    )
    if receipt_path != receipt.resolve():
        raise PlatformReceiptError("platform receipt argument differs from its file record")
    capture = validate_capture(
        _read(capture_path, "licensed-load capture"),
        artifact_path,
        str(evidence["platform"]),
    )
    archive_commit, archive_tree, archive_build_date = _archive_identity(archive_path)
    if (
        HEX_SHA1.fullmatch(str(evidence["source_commit"])) is None
        or HEX_SHA1.fullmatch(str(evidence["source_tree"])) is None
        or HEX_SHA256.fullmatch(str(evidence["source_archive_sha256"])) is None
        or isinstance(evidence["source_archive_size_bytes"], bool)
        or not isinstance(evidence["source_archive_size_bytes"], int)
        or evidence["source_archive_size_bytes"] < 1
        or evidence["embedded_git_commit"] != str(evidence["source_commit"])[:12]
        or re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", str(evidence["build_date_utc"])) is None
        or archive_record["sha256"] != evidence["source_archive_sha256"]
        or archive_record["size_bytes"] != evidence["source_archive_size_bytes"]
        or archive_commit != evidence["source_commit"]
        or archive_tree != evidence["source_tree"]
        or archive_build_date != evidence["build_date_utc"]
        or str(evidence["build_date_utc"]).encode("ascii") not in artifact_path.read_bytes()
        or HEX_SHA1.fullmatch(str(evidence["controller_commit"])) is None
        or HEX_SHA1.fullmatch(str(evidence["controller_tree"])) is None
        or program_record["sha256"] != capture["capture_program_sha256"]
        or capture["source_commit"] != evidence["source_commit"]
        or capture["embedded_git_commit"] != evidence["embedded_git_commit"]
    ):
        raise PlatformReceiptError("platform evidence source identity is invalid")
    source = {
        "source_commit": evidence["source_commit"],
        "source_tree": evidence["source_tree"],
        "source_archive_sha256": evidence["source_archive_sha256"],
        "source_archive_size_bytes": evidence["source_archive_size_bytes"],
        "embedded_git_commit": evidence["embedded_git_commit"],
        "build_date_utc": evidence["build_date_utc"],
    }
    controller = {
        "controller_commit": evidence["controller_commit"],
        "controller_tree": evidence["controller_tree"],
    }
    provenance = _read_record(evidence["platform_provenance"], "platform provenance")
    provenance_identity = _provenance_identity(
        provenance,
        platform=str(evidence["platform"]),
        source=source,
        controller=controller,
        artifact=artifact_path,
        capture_path=capture_path,
        capture=capture,
        capture_program=program_path,
    )
    if evidence["qualification_profile"] != _qualification_profile(
        str(evidence["platform"]), source, controller, provenance
    ):
        raise PlatformReceiptError(
            "platform qualification profile differs from target/controller provenance"
        )
    if evidence["qualification_profile"] == "candidate":
        try:
            with tarfile.open(archive_path, "r:") as archived_source:
                member = archived_source.getmember(
                    "tests/stata/installation/feppml_platform_load_capture.do"
                )
                opened = archived_source.extractfile(member)
                committed_program = None if opened is None else opened.read()
        except (KeyError, OSError, tarfile.TarError) as error:
            raise PlatformReceiptError(
                "candidate source archive omits the platform capture program"
            ) from error
        if not member.isfile() or committed_program != program_path.read_bytes():
            raise PlatformReceiptError(
                "candidate platform capture program differs from exact source bytes"
            )
    document = _read(receipt_path, "platform receipt")
    if set(document) != {"payload", "seal"}:
        raise PlatformReceiptError("platform receipt envelope is not closed")
    payload = document["payload"]
    if not isinstance(payload, Mapping) or document["seal"] != qualification.seal_payload(payload)["seal"]:
        raise PlatformReceiptError("platform receipt seal mismatch")
    if (
        payload.get("kind") != "feppml_platform_licensed_stata_load"
        or payload.get("licensed_stata_load") is not True
        or payload.get("mock_only") is not False
        or payload.get("scientific_claim") is not False
    ):
        raise PlatformReceiptError("platform receipt scope is invalid")
    expected_payload = {
        "schema_version": 1,
        "kind": "feppml_platform_licensed_stata_load",
        "qualification_profile": evidence["qualification_profile"],
        "platform": capture["platform"],
        "artifact_sha256": artifact_record["sha256"],
        "artifact_size_bytes": artifact_record["size_bytes"],
        "stata_version": capture["stata_version"],
        "stata_flavor": capture["stata_flavor"],
        "capture_program_sha256": capture["capture_program_sha256"],
        "capture_output_sha256": capture_record["sha256"],
        "source_commit": evidence["source_commit"],
        "source_tree": evidence["source_tree"],
        "source_archive_sha256": evidence["source_archive_sha256"],
        "source_archive_size_bytes": evidence["source_archive_size_bytes"],
        "embedded_git_commit": evidence["embedded_git_commit"],
        "build_date_utc": evidence["build_date_utc"],
        "controller_commit": evidence["controller_commit"],
        "controller_tree": evidence["controller_tree"],
        "platform_provenance": provenance_identity,
        "licensed_stata_load": True,
        "mock_only": False,
        "scientific_claim": False,
        "receipt_scope": "artifact_and_licensed_stata_load_only",
        "complete_sentinel": "FEPPML_PLATFORM_LOAD_RECEIPT_COMPLETE",
    }
    if payload != expected_payload:
        raise PlatformReceiptError(
            "platform receipt payload differs from exact artifact/capture bytes"
        )
    expected = {
        "record_type": RECORD_TYPE,
        "qualification_profile": payload["qualification_profile"],
        "platform": payload["platform"],
        "source_commit": payload["source_commit"],
        "source_tree": payload["source_tree"],
        "source_archive_sha256": payload["source_archive_sha256"],
        "source_archive_size_bytes": payload["source_archive_size_bytes"],
        "embedded_git_commit": payload["embedded_git_commit"],
        "build_date_utc": payload["build_date_utc"],
        "controller_commit": payload["controller_commit"],
        "controller_tree": payload["controller_tree"],
        "artifact": dict(artifact_record),
        "capture": dict(capture_record),
        "source_archive": dict(archive_record),
        "capture_program": dict(program_record),
        "receipt": dict(receipt_record),
        "platform_provenance": dict(provenance),
        "load_status": "success",
        "licensed_stata_load": True,
        "mock_only": False,
        "scientific_claim": False,
        "receipt_scope": "artifact_and_licensed_stata_load_only",
    }
    if receipt_record["payload_sha256"] != document["seal"]["payload_sha256"]:
        raise PlatformReceiptError("platform receipt payload digest is misbound")
    if evidence != expected:
        raise PlatformReceiptError("platform evidence record differs from sealed receipt")
    return payload


def verify(receipt: Path, record: Path) -> Mapping[str, Any]:
    return _verify_evidence(
        receipt, _read(record, "platform evidence record")
    )


def verify_evidence(evidence: Mapping[str, Any]) -> Mapping[str, Any]:
    """Verify an already-loaded closed record using its sealed receipt file."""

    receipt_record = _read_record(
        evidence.get("receipt"), "platform evidence receipt record"
    )
    receipt_path = receipt_record.get("path")
    if not isinstance(receipt_path, str) or not receipt_path:
        raise PlatformReceiptError("platform evidence receipt path is malformed")
    return _verify_evidence(Path(receipt_path), evidence)


def _portable_file_record(
    original: Mapping[str, Any], path: Path, *, receipt: bool = False
) -> dict[str, Any]:
    actual = _file_record(path)
    if (
        actual["sha256"] != original.get("sha256")
        or actual["size_bytes"] != original.get("size_bytes")
    ):
        raise PlatformReceiptError("portable evidence bytes differ from the sealed record")
    if receipt:
        actual["payload_sha256"] = original.get("payload_sha256")
    return actual


def verify_portable(
    *,
    receipt: Path,
    record: Path,
    artifact: Path,
    capture_path: Path,
    source_archive: Path,
    capture_program: Path,
    native_attestation: Path | None = None,
    native_producer: Path | None = None,
    windows_controller_receipt: Path | None = None,
    windows_controller_output: Path | None = None,
    windows_controller_archive: Path | None = None,
    windows_results_archive: Path | None = None,
    windows_source_manifest: Path | None = None,
) -> Mapping[str, Any]:
    """Verify copied evidence without trusting machine-local stored paths."""

    original = copy.deepcopy(_read(record, "platform evidence record"))
    if set(original) != RECORD_FIELDS:
        raise PlatformReceiptError("portable platform evidence record is not closed")
    for field, path in (
        ("artifact", artifact),
        ("capture", capture_path),
        ("source_archive", source_archive),
        ("capture_program", capture_program),
    ):
        original[field] = _portable_file_record(original[field], path)
    original["receipt"] = _portable_file_record(
        original["receipt"], receipt, receipt=True
    )
    provenance = original["platform_provenance"]
    platform = original["platform"]
    if platform in {"macos-universal", "linux-x86_64"}:
        if native_attestation is None or native_producer is None:
            raise PlatformReceiptError(
                f"portable {platform} evidence requires its attestation and producer"
            )
        provenance["producer"] = _portable_file_record(
            provenance["producer"], native_producer
        )
        provenance["attestation"] = _portable_file_record(
            provenance["attestation"], native_attestation
        )
        for field, path in native_sidecar_paths(native_attestation).items():
            provenance[field] = _portable_file_record(provenance[field], path)
    elif platform == "windows-x86_64":
        supplied = {
            "controller_receipt": windows_controller_receipt,
            "controller_output": windows_controller_output,
            "controller_archive": windows_controller_archive,
            "results_archive": windows_results_archive,
            "source_manifest": windows_source_manifest,
        }
        if not all(value is not None for value in supplied.values()):
            raise PlatformReceiptError("portable Windows evidence is incomplete")
        for field, path in supplied.items():
            provenance[field] = _portable_file_record(provenance[field], path)
    return _verify_evidence(receipt, original)


def rebind(
    *,
    platform: str,
    artifact: Path,
    capture_path: Path,
    source_archive: Path,
    capture_program: Path,
    receipt: Path,
    record: Path,
    windows_controller_receipt: Path | None = None,
    windows_controller_output: Path | None = None,
    windows_controller_archive: Path | None = None,
    windows_results_archive: Path | None = None,
    windows_source_manifest: Path | None = None,
    native_attestation: Path | None = None,
    native_producer: Path | None = None,
) -> dict[str, Any]:
    """Create a path-local record for copied but byte-identical evidence."""

    document = _read(receipt, "platform receipt")
    payload = _read_record(document.get("payload"), "platform receipt payload")
    seal = _read_record(document.get("seal"), "platform receipt seal")
    payload_sha = seal.get("payload_sha256")
    if HEX_SHA256.fullmatch(str(payload_sha)) is None:
        raise PlatformReceiptError("platform receipt payload SHA-256 is malformed")
    receipt_file = _file_record(receipt)
    receipt_file["payload_sha256"] = payload_sha
    source = {
        key: payload.get(key)
        for key in (
            "source_commit", "source_tree", "source_archive_sha256",
            "source_archive_size_bytes", "embedded_git_commit", "build_date_utc",
        )
    }
    controller = {
        "controller_commit": payload.get("controller_commit"),
        "controller_tree": payload.get("controller_tree"),
    }
    artifact_path = _regular_file(artifact, "platform artifact")
    capture_file = _regular_file(capture_path, "platform capture")
    program = _regular_file(capture_program, "capture controller program")
    capture = validate_capture(_read(capture_file, "platform capture"), artifact_path, platform)
    if platform == "windows-x86_64":
        if not all(
            value is not None
            for value in (
                windows_controller_receipt,
                windows_controller_output,
                windows_controller_archive,
                windows_results_archive,
                windows_source_manifest,
            )
        ):
            raise PlatformReceiptError("Windows rebind requires all controller evidence")
        provenance = _windows_provenance(
            controller_receipt=windows_controller_receipt,
            controller_output=windows_controller_output,
            controller_archive=windows_controller_archive,
            results_archive=windows_results_archive,
            source_manifest=windows_source_manifest,
            source=source,
            controller=controller,
            artifact=artifact_path,
            capture_path=capture_file,
            capture=capture,
            capture_program=program,
        )
    elif platform in {"macos-universal", "linux-x86_64"}:
        if native_attestation is None or native_producer is None:
            raise PlatformReceiptError(
                f"{platform} rebind requires its execution attestation and producer"
            )
        sidecars = native_sidecar_paths(native_attestation)
        provenance = _native_provenance(
            attestation=native_attestation,
            stata_executable=None,
            status_file=sidecars["status"],
            stata_log=sidecars["stata_log"],
            wrapper=sidecars["wrapper"],
            invocation=sidecars["invocation"],
            platform=platform,
            source=source,
            controller=controller,
            artifact=artifact_path,
            capture_path=capture_file,
            capture=capture,
            capture_program=program,
            producer=_regular_file(native_producer, "native capture producer"),
        )
        if platform == "linux-x86_64":
            provenance["outer_authority"] = "scc-closed-run"
    else:
        raise PlatformReceiptError(f"unsupported platform: {platform}")
    output = {
        "record_type": RECORD_TYPE,
        "qualification_profile": payload.get("qualification_profile"),
        "platform": platform,
        "source_commit": payload.get("source_commit"),
        "source_tree": payload.get("source_tree"),
        "source_archive_sha256": payload.get("source_archive_sha256"),
        "source_archive_size_bytes": payload.get("source_archive_size_bytes"),
        "embedded_git_commit": payload.get("embedded_git_commit"),
        "build_date_utc": payload.get("build_date_utc"),
        "controller_commit": payload.get("controller_commit"),
        "controller_tree": payload.get("controller_tree"),
        "artifact": _file_record(artifact),
        "capture": _file_record(capture_path),
        "source_archive": _file_record(source_archive),
        "capture_program": _file_record(capture_program),
        "receipt": receipt_file,
        "platform_provenance": provenance,
        "load_status": "success",
        "licensed_stata_load": True,
        "mock_only": False,
        "scientific_claim": False,
        "receipt_scope": "artifact_and_licensed_stata_load_only",
    }
    _verify_evidence(receipt, output)
    _write(record, output)
    return output


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    create_parser = commands.add_parser("create")
    create_parser.add_argument("--platform", choices=PLATFORMS, required=True)
    create_parser.add_argument("--artifact", type=Path, required=True)
    create_parser.add_argument("--capture", type=Path, required=True)
    create_parser.add_argument("--receipt", type=Path, required=True)
    create_parser.add_argument("--record", type=Path, required=True)
    create_parser.add_argument("--source-repo", type=Path, required=True)
    create_parser.add_argument("--source-commit", required=True)
    create_parser.add_argument("--source-archive", type=Path, required=True)
    create_parser.add_argument("--controller-repo", type=Path, required=True)
    create_parser.add_argument("--controller-commit", required=True)
    create_parser.add_argument("--capture-program", type=Path, required=True)
    create_parser.add_argument("--native-attestation", type=Path)
    create_parser.add_argument("--native-stata-executable", type=Path)
    create_parser.add_argument("--windows-controller-receipt", type=Path)
    create_parser.add_argument("--windows-controller-output", type=Path)
    create_parser.add_argument("--windows-controller-archive", type=Path)
    create_parser.add_argument("--windows-results-archive", type=Path)
    create_parser.add_argument("--windows-source-manifest", type=Path)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--receipt", type=Path, required=True)
    verify_parser.add_argument("--record", type=Path, required=True)
    portable_parser = commands.add_parser("verify-portable")
    portable_parser.add_argument("--receipt", type=Path, required=True)
    portable_parser.add_argument("--record", type=Path, required=True)
    portable_parser.add_argument("--artifact", type=Path, required=True)
    portable_parser.add_argument("--capture", type=Path, required=True)
    portable_parser.add_argument("--source-archive", type=Path, required=True)
    portable_parser.add_argument("--capture-program", type=Path, required=True)
    portable_parser.add_argument("--native-attestation", type=Path)
    portable_parser.add_argument("--native-producer", type=Path)
    portable_parser.add_argument("--windows-controller-receipt", type=Path)
    portable_parser.add_argument("--windows-controller-output", type=Path)
    portable_parser.add_argument("--windows-controller-archive", type=Path)
    portable_parser.add_argument("--windows-results-archive", type=Path)
    portable_parser.add_argument("--windows-source-manifest", type=Path)
    rebind_parser = commands.add_parser("rebind")
    rebind_parser.add_argument("--platform", choices=PLATFORMS, required=True)
    rebind_parser.add_argument("--artifact", type=Path, required=True)
    rebind_parser.add_argument("--capture", type=Path, required=True)
    rebind_parser.add_argument("--source-archive", type=Path, required=True)
    rebind_parser.add_argument("--capture-program", type=Path, required=True)
    rebind_parser.add_argument("--receipt", type=Path, required=True)
    rebind_parser.add_argument("--record", type=Path, required=True)
    rebind_parser.add_argument("--windows-controller-receipt", type=Path)
    rebind_parser.add_argument("--windows-controller-output", type=Path)
    rebind_parser.add_argument("--windows-controller-archive", type=Path)
    rebind_parser.add_argument("--windows-results-archive", type=Path)
    rebind_parser.add_argument("--windows-source-manifest", type=Path)
    rebind_parser.add_argument("--native-attestation", type=Path)
    rebind_parser.add_argument("--native-producer", type=Path)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        if args.command == "create":
            create(
                platform=args.platform,
                artifact=args.artifact,
                capture_path=args.capture,
                receipt=args.receipt,
                record=args.record,
                source_repo=args.source_repo,
                source_commit=args.source_commit,
                source_archive=args.source_archive,
                controller_repo=args.controller_repo,
                controller_commit=args.controller_commit,
                capture_program=args.capture_program,
                native_attestation=args.native_attestation,
                native_stata_executable=args.native_stata_executable,
                windows_controller_receipt=args.windows_controller_receipt,
                windows_controller_output=args.windows_controller_output,
                windows_controller_archive=args.windows_controller_archive,
                windows_results_archive=args.windows_results_archive,
                windows_source_manifest=args.windows_source_manifest,
            )
        elif args.command == "verify":
            verify(args.receipt, args.record)
        elif args.command == "verify-portable":
            verify_portable(
                receipt=args.receipt,
                record=args.record,
                artifact=args.artifact,
                capture_path=args.capture,
                source_archive=args.source_archive,
                capture_program=args.capture_program,
                native_attestation=args.native_attestation,
                native_producer=args.native_producer,
                windows_controller_receipt=args.windows_controller_receipt,
                windows_controller_output=args.windows_controller_output,
                windows_controller_archive=args.windows_controller_archive,
                windows_results_archive=args.windows_results_archive,
                windows_source_manifest=args.windows_source_manifest,
            )
        else:
            rebind(
                platform=args.platform,
                artifact=args.artifact,
                capture_path=args.capture,
                source_archive=args.source_archive,
                capture_program=args.capture_program,
                receipt=args.receipt,
                record=args.record,
                windows_controller_receipt=args.windows_controller_receipt,
                windows_controller_output=args.windows_controller_output,
                windows_controller_archive=args.windows_controller_archive,
                windows_results_archive=args.windows_results_archive,
                windows_source_manifest=args.windows_source_manifest,
                native_attestation=args.native_attestation,
                native_producer=args.native_producer,
            )
    except PlatformReceiptError as error:
        raise SystemExit(f"feppml platform receipt failed closed: {error}") from error
    print(f"FEPPML_PLATFORM_{args.command.upper()}_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
