#!/usr/bin/env python3
"""Seal the sanitized source-side Windows artifact/load manifest."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
from typing import Any, Mapping, Sequence

try:
    from scripts import feppml_platform_load_receipt as platform
except ImportError:
    import feppml_platform_load_receipt as platform  # type: ignore[no-redef]


class ManifestError(RuntimeError):
    pass


TARGET_PATTERN = re.compile(
    r"\Amode=(candidate|f0-baseline)\r?\n"
    r"target_commit=([0-9a-f]{40})\r?\n"
    r"target_tree=([0-9a-f]{40})\r?\n"
    r"target_archive_sha256=([0-9a-f]{64})\r?\n"
    r"target_archive_size_bytes=([1-9][0-9]*)\r?\n"
    r"embedded_git_commit=([0-9a-f]{12})\r?\n"
    r"build_date_utc=([0-9]{4}-[0-9]{2}-[0-9]{2})\r?\n"
    r"harness_commit=([0-9a-f]{40})\r?\n"
    r"harness_tree=([0-9a-f]{40})\r?\n"
    r"FEPPML_WINDOWS_TARGET_MATERIALIZED\r?\n\Z"
)


def _read(path: Path) -> Mapping[str, Any]:
    value = json.loads(path.read_text(encoding="ascii"))
    if not isinstance(value, Mapping):
        raise ManifestError("JSON input must be an object")
    return value


def seal(
    *,
    artifact: Path,
    capture: Path,
    capture_program: Path,
    controller_source_manifest: Path,
    target_receipt: Path,
    run_id: str,
    output: Path,
) -> Mapping[str, Any]:
    controller = _read(controller_source_manifest)
    match = TARGET_PATTERN.fullmatch(target_receipt.read_text(encoding="ascii"))
    revision = controller.get("source_revision")
    if (
        set(controller) != {"schema_version", "project", "source_revision", "dirty"}
        or controller.get("schema_version") != 1
        or controller.get("project") != "fereg"
        or controller.get("dirty") is not False
        or not isinstance(revision, str)
        or re.fullmatch(r"commit-([0-9a-f]{40})", revision) is None
        or match is None
        or re.fullmatch(r"win-[0-9]{8}T[0-9]{6}Z-[0-9a-f]{8}", run_id)
        is None
    ):
        raise ManifestError("controller or target source identity is invalid")
    source_commit = match.group(2)
    embedded = match.group(6)
    if (
        embedded != source_commit[:12]
        or (
            match.group(1) == "candidate"
            and (match.group(2) != match.group(8) or match.group(3) != match.group(9))
        )
        or (match.group(1) == "f0-baseline" and match.group(2) == match.group(8))
        or embedded.encode("ascii") not in artifact.read_bytes()
        or match.group(7).encode("ascii") not in artifact.read_bytes()
    ):
        raise ManifestError("target mode, embedded commit, or build date is invalid")
    capture_value = platform.validate_capture(
        _read(capture), artifact, "windows-x86_64"
    )
    if (
        capture_value["source_commit"] != source_commit
        or capture_value["embedded_git_commit"] != embedded
        or capture_value["capture_program_sha256"]
        != platform.qualification.sha256_file(capture_program)
    ):
        raise ManifestError("capture differs from target/controller inputs")
    payload = {
        "schema_version": 1,
        "kind": platform.WINDOWS_MANIFEST_KIND,
        "platform": "windows-x86_64",
        "mode": match.group(1),
        "run_id": run_id,
        "source_commit": source_commit,
        "source_tree": match.group(3),
        "source_archive_sha256": match.group(4),
        "source_archive_size_bytes": int(match.group(5)),
        "embedded_git_commit": embedded,
        "build_date_utc": match.group(7),
        "controller_commit": revision[7:],
        "harness_commit": match.group(8),
        "harness_tree": match.group(9),
        "artifact_sha256": platform.qualification.sha256_file(artifact),
        "artifact_size_bytes": artifact.stat().st_size,
        "capture_sha256": platform.qualification.sha256_file(capture),
        "capture_program_sha256": platform.qualification.sha256_file(
            capture_program
        ),
        "plugin_load_rc": 0,
        "licensed_stata_load": True,
        "mock_only": False,
        "scientific_claim": False,
        "complete_sentinel": "FEPPML_WINDOWS_PLATFORM_SOURCE_MANIFEST_COMPLETE",
    }
    platform._write(output, platform.qualification.seal_payload(payload))
    return payload


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--artifact", type=Path, required=True)
    result.add_argument("--capture", type=Path, required=True)
    result.add_argument("--capture-program", type=Path, required=True)
    result.add_argument("--controller-source-manifest", type=Path, required=True)
    result.add_argument("--target-receipt", type=Path, required=True)
    result.add_argument("--run-id", required=True)
    result.add_argument("--output", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        seal(
            artifact=args.artifact,
            capture=args.capture,
            capture_program=args.capture_program,
            controller_source_manifest=args.controller_source_manifest,
            target_receipt=args.target_receipt,
            run_id=args.run_id,
            output=args.output,
        )
    except (ManifestError, platform.PlatformReceiptError, OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"Windows platform manifest failed closed: {error}") from error
    print("FEPPML_WINDOWS_PLATFORM_MANIFEST_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
