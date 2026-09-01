#!/usr/bin/env python3
"""Verify and materialize the exact target inside a Windows controller run."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
from typing import Any, Mapping, Sequence

try:
    from scripts import feppml_qualification as qualification
except ImportError:
    import feppml_qualification as qualification  # type: ignore[no-redef]


class TargetError(RuntimeError):
    pass


HEX_SHA1 = re.compile(r"^[0-9a-f]{40}$")
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")

# These are the controller-owned programs/data that may influence a Windows
# result.  The target archive is deliberately absent: it has its own exact
# Git-archive identity in the sealed preparation.
CONTRACT_PATHS = (
    "windows-ci.do",
    "scripts/bundle_windows_feppml_results.py",
    "scripts/build_windows.ps1",
    "scripts/verify_windows_plugin.ps1",
    "scripts/audit_binary.py",
    "scripts/feppml_qualification.py",
    "scripts/feppml_platform_load_receipt.py",
    "scripts/prepare_windows_feppml_target.py",
    "scripts/prepare_windows_feppml_platform_capture.ps1",
    "scripts/seal_windows_feppml_platform_manifest.py",
    "tests/mock_spi/run_actual_windows_plugin_test.ps1",
    "tests/mock_spi/actual_plugin_loader_test_windows.c",
    "plugin/stplugin.h",
    "stata/_fereg_ppml.ado",
    "stata/_fereg_estimate_ppml.ado",
    "stata/feppml_p.ado",
    "tests/stata/installation/feppml_platform_load_capture.do",
    "tests/stata/installation/windows_ci_smoke.do",
    "tests/stata/parity_ppmlhdfe/estimate_ppml_hidden_transaction.do",
    "tests/stata/parity_ppmlhdfe/feppml_fail_closed.do",
)


def _read(path: Path) -> Mapping[str, Any]:
    value = json.loads(path.read_text(encoding="ascii"))
    if not isinstance(value, Mapping):
        raise TargetError("preparation must be an object")
    return value


def _write_new(path: Path, text: str) -> None:
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise TargetError("target receipt already exists") from error
    with os.fdopen(descriptor, "w", encoding="ascii", newline="\n") as output:
        output.write(text)


def materialize(
    preparation: Path, archive: Path, output: Path, receipt: Path
) -> Mapping[str, Any]:
    document = _read(preparation)
    if set(document) != {"payload", "seal"}:
        raise TargetError("preparation envelope is not closed")
    payload = document["payload"]
    if not isinstance(payload, Mapping) or set(payload) != {
        "schema_version", "kind", "mode", "target", "harness",
        "contract_files", "complete_sentinel",
    }:
        raise TargetError("preparation payload fields are not closed")
    if (
        document["seal"] != qualification.seal_payload(payload)["seal"]
        or payload.get("schema_version") != 1
        or payload.get("kind") != "feppml_windows_controller_preparation"
        or payload.get("mode") not in {"candidate", "f0-baseline"}
        or payload.get("complete_sentinel") != "FEPPML_WINDOWS_CONTROLLER_PREPARED"
    ):
        raise TargetError("preparation seal or identity is invalid")
    target = payload.get("target")
    if not isinstance(target, Mapping) or set(target) != {
        "commit", "tree", "archive_sha256", "archive_size_bytes",
        "embedded_git_commit", "build_date_utc",
    }:
        raise TargetError("target identity is not closed")
    harness = payload.get("harness")
    if (
        HEX_SHA1.fullmatch(str(target["commit"])) is None
        or HEX_SHA1.fullmatch(str(target["tree"])) is None
        or HEX_SHA256.fullmatch(str(target["archive_sha256"])) is None
        or isinstance(target["archive_size_bytes"], bool)
        or not isinstance(target["archive_size_bytes"], int)
        or target["archive_size_bytes"] < 1
        or target["embedded_git_commit"] != str(target["commit"])[:12]
        or re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", str(target["build_date_utc"]))
        is None
        or not isinstance(harness, Mapping)
        or set(harness) != {"commit", "tree"}
        or HEX_SHA1.fullmatch(str(harness["commit"])) is None
        or HEX_SHA1.fullmatch(str(harness["tree"])) is None
    ):
        raise TargetError("target or harness identity is invalid")
    if payload["mode"] == "f0-baseline" and target["commit"] == harness["commit"]:
        raise TargetError("F0 target must be distinct from the trusted harness")
    if payload["mode"] == "candidate" and (
        target["commit"] != harness["commit"]
        or target["tree"] != harness["tree"]
    ):
        raise TargetError(
            "candidate target must equal the trusted harness commit and tree"
        )

    contract_files = payload.get("contract_files")
    if not isinstance(contract_files, Mapping) or set(contract_files) != set(
        CONTRACT_PATHS
    ):
        raise TargetError("controller contract-file inventory is not closed")
    controller_root = preparation.resolve().parent.parent
    for relative in CONTRACT_PATHS:
        record = contract_files[relative]
        path = controller_root / relative
        if (
            not isinstance(record, Mapping)
            or set(record) != {"sha256", "size_bytes"}
            or HEX_SHA256.fullmatch(str(record.get("sha256"))) is None
            or isinstance(record.get("size_bytes"), bool)
            or not isinstance(record.get("size_bytes"), int)
            or record["size_bytes"] < 1
            or path.is_symlink()
            or not path.is_file()
            or path.stat().st_size != record["size_bytes"]
            or qualification.sha256_file(path) != record["sha256"]
        ):
            raise TargetError(f"controller contract file changed: {relative}")

    if (
        archive.is_symlink()
        or not archive.is_file()
        or qualification.sha256_file(archive) != target["archive_sha256"]
        or archive.stat().st_size != target["archive_size_bytes"]
    ):
        raise TargetError("target archive differs from preparation")
    if output.exists():
        raise TargetError("target output directory already exists")
    with tarfile.open(archive, "r:") as source:
        members = source.getmembers()
        mtimes = {member.mtime for member in members}
        if (
            not members
            or len(mtimes) != 1
            or datetime.fromtimestamp(
                mtimes.pop(), timezone.utc
            ).date().isoformat() != target["build_date_utc"]
        ):
            raise TargetError("target archive build date differs from preparation")
        for member in members:
            relative = Path(member.name)
            if (
                relative.is_absolute()
                or not relative.parts
                or ".." in relative.parts
                or not (member.isfile() or member.isdir())
            ):
                raise TargetError("target archive contains an unsafe member")
        output.mkdir(parents=True)
        source.extractall(output, members=members)
    with archive.open("rb") as source:
        completed = subprocess.run(
            ["git", "get-tar-commit-id"], stdin=source, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, check=False, text=True,
        )
    if completed.returncode or completed.stdout.strip() != target["commit"]:
        raise TargetError("target archive has no exact prepared commit identity")
    lines = (
        f"mode={payload['mode']}\n"
        f"target_commit={target['commit']}\n"
        f"target_tree={target['tree']}\n"
        f"target_archive_sha256={target['archive_sha256']}\n"
        f"target_archive_size_bytes={target['archive_size_bytes']}\n"
        f"embedded_git_commit={target['embedded_git_commit']}\n"
        f"build_date_utc={target['build_date_utc']}\n"
        f"harness_commit={payload['harness']['commit']}\n"
        f"harness_tree={payload['harness']['tree']}\n"
        "FEPPML_WINDOWS_TARGET_MATERIALIZED\n"
    )
    _write_new(receipt, lines)
    return payload


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--preparation", type=Path, required=True)
    result.add_argument("--archive", type=Path, required=True)
    result.add_argument("--output", type=Path, required=True)
    result.add_argument("--receipt", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        materialize(args.preparation, args.archive, args.output, args.receipt)
    except (TargetError, OSError, json.JSONDecodeError, tarfile.TarError) as error:
        raise SystemExit(f"Windows target materialization failed closed: {error}") from error
    print("FEPPML_WINDOWS_TARGET_MATERIALIZE_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
