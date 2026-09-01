#!/usr/bin/env python3
"""Create the deterministic, closed Windows result bundle for the controller."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
from typing import Any, Mapping, Sequence
import zipfile

try:
    from scripts import feppml_platform_load_receipt as platform
    from scripts import feppml_qualification as qualification
    from scripts import seal_windows_feppml_platform_manifest as source_manifest
except ImportError:
    import feppml_platform_load_receipt as platform  # type: ignore[no-redef]
    import feppml_qualification as qualification  # type: ignore[no-redef]
    import seal_windows_feppml_platform_manifest as source_manifest  # type: ignore[no-redef]


class BundleError(RuntimeError):
    pass


INPUT_KEYS = {
    "artifact", "checksum", "capture", "target_receipt", "source_manifest",
    "built_verification", "staged_verification", "capture_inputs",
    "capture_verification",
}
RESULT_MEMBERS = {
    "artifact/fereg_windows.plugin": "artifact",
    "evidence/capture.json": "capture",
    "evidence/source-manifest.json": "source_manifest",
}
SOURCE_MANIFEST_FIELDS = {
    "schema_version", "kind", "platform", "mode", "run_id", "source_commit",
    "source_tree", "source_archive_sha256", "source_archive_size_bytes",
    "embedded_git_commit", "build_date_utc", "controller_commit",
    "harness_commit", "harness_tree", "artifact_sha256",
    "artifact_size_bytes", "capture_sha256", "capture_program_sha256",
    "plugin_load_rc", "licensed_stata_load", "mock_only",
    "scientific_claim", "complete_sentinel",
}


def _regular(path: Path, where: str) -> Path:
    raw = Path(path)
    resolved = raw.resolve()
    if raw.is_symlink() or resolved.is_symlink() or not resolved.is_file():
        raise BundleError(f"{where} is not a regular non-symlink file")
    if resolved.stat().st_size < 1:
        raise BundleError(f"{where} is empty")
    return resolved


def _json(path: Path, where: str) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="ascii"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise BundleError(f"{where} is not ASCII JSON") from error
    if not isinstance(value, Mapping):
        raise BundleError(f"{where} must be an object")
    return value


def _normalized_text(path: Path) -> str:
    return path.read_text(encoding="ascii").replace("\r\n", "\n")


def bundle(*, output: Path, **inputs: Path) -> Mapping[str, Any]:
    if set(inputs) != INPUT_KEYS:
        raise BundleError("Windows result-bundle inputs are not closed")
    paths = {key: _regular(path, key) for key, path in inputs.items()}
    document = _json(paths["source_manifest"], "source manifest")
    if set(document) != {"payload", "seal"} or not isinstance(
        document.get("payload"), Mapping
    ):
        raise BundleError("source manifest envelope is not closed")
    payload = document["payload"]
    if document["seal"] != qualification.seal_payload(payload)["seal"]:
        raise BundleError("source manifest seal changed")
    run_id = payload.get("run_id")
    if (
        set(payload) != SOURCE_MANIFEST_FIELDS
        or payload.get("schema_version") != 1
        or payload.get("kind") != platform.WINDOWS_MANIFEST_KIND
        or payload.get("platform") != "windows-x86_64"
        or payload.get("mode") not in {"candidate", "f0-baseline"}
        or re.fullmatch(r"win-[0-9]{8}T[0-9]{6}Z-[0-9a-f]{8}", str(run_id)) is None
        or payload.get("plugin_load_rc") != 0
        or payload.get("licensed_stata_load") is not True
        or payload.get("mock_only") is not False
        or payload.get("scientific_claim") is not False
        or payload.get("complete_sentinel")
        != "FEPPML_WINDOWS_PLATFORM_SOURCE_MANIFEST_COMPLETE"
    ):
        raise BundleError("source manifest has no authoritative controller run ID")

    artifact = paths["artifact"]
    capture_path = paths["capture"]
    capture = platform.validate_capture(
        _json(capture_path, "licensed-load capture"), artifact, "windows-x86_64"
    )
    target_match = source_manifest.TARGET_PATTERN.fullmatch(
        _normalized_text(paths["target_receipt"])
    )
    if target_match is None:
        raise BundleError("target receipt is malformed")
    if (
        (
            target_match.group(1) == "candidate"
            and (
                target_match.group(2) != target_match.group(8)
                or target_match.group(3) != target_match.group(9)
            )
        )
        or (
            target_match.group(1) == "f0-baseline"
            and target_match.group(2) == target_match.group(8)
        )
    ):
        raise BundleError("target receipt mode does not match target/harness identities")
    expected = {
        "mode": target_match.group(1),
        "source_commit": target_match.group(2),
        "source_tree": target_match.group(3),
        "source_archive_sha256": target_match.group(4),
        "source_archive_size_bytes": int(target_match.group(5)),
        "embedded_git_commit": target_match.group(6),
        "build_date_utc": target_match.group(7),
        "harness_commit": target_match.group(8),
        "harness_tree": target_match.group(9),
        "artifact_sha256": qualification.sha256_file(artifact),
        "artifact_size_bytes": artifact.stat().st_size,
        "capture_sha256": qualification.sha256_file(capture_path),
        "capture_program_sha256": capture["capture_program_sha256"],
    }
    if any(payload.get(key) != value for key, value in expected.items()):
        raise BundleError("source manifest differs from target/artifact/capture bytes")
    artifact_sha = expected["artifact_sha256"]
    checksum = _normalized_text(paths["checksum"])
    if re.fullmatch(rf"{artifact_sha}  fereg_windows\.plugin\n?", checksum) is None:
        raise BundleError("artifact checksum record changed")
    verification = f"WINDOWS_PLUGIN_SHA256={artifact_sha}"
    for name in ("built_verification", "staged_verification"):
        if _normalized_text(paths[name]) != verification:
            raise BundleError(f"{name} changed")
    capture_verification = (
        "FEPPML_PLATFORM_CAPTURE_SHA256=" + expected["capture_sha256"]
    )
    if _normalized_text(paths["capture_verification"]) != capture_verification:
        raise BundleError("capture verification record changed")
    capture_inputs = "\n".join(
        (
            f"controller_commit={payload['controller_commit']}",
            f"mode={expected['mode']}",
            f"source_commit={expected['source_commit']}",
            f"source_tree={expected['source_tree']}",
            f"source_archive_sha256={expected['source_archive_sha256']}",
            f"source_archive_size_bytes={expected['source_archive_size_bytes']}",
            f"embedded_git_commit={expected['embedded_git_commit']}",
            f"build_date_utc={expected['build_date_utc']}",
            f"harness_commit={expected['harness_commit']}",
            f"harness_tree={expected['harness_tree']}",
            f"artifact_sha256={artifact_sha}",
            f"artifact_size_bytes={expected['artifact_size_bytes']}",
            f"capture_program_sha256={expected['capture_program_sha256']}",
            "FEPPML_PLATFORM_CAPTURE_INPUTS_COMPLETE",
        )
    )
    if _normalized_text(paths["capture_inputs"]) != capture_inputs:
        raise BundleError("capture-input receipt changed")
    destination = Path(output)
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        handle = destination.open("xb")
    except FileExistsError as error:
        raise BundleError("refusing to overwrite Windows result bundle") from error
    try:
        with handle:
            with zipfile.ZipFile(handle, "w", compression=zipfile.ZIP_STORED) as archive:
                contents = {
                    name: paths[key].read_bytes()
                    for name, key in RESULT_MEMBERS.items()
                }
                for name, data in sorted(contents.items()):
                    info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                    info.compress_type = zipfile.ZIP_STORED
                    info.create_system = 3
                    info.external_attr = 0o100444 << 16
                    archive.writestr(info, data)
            handle.flush()
            os.fsync(handle.fileno())
    except Exception:
        # An incomplete create-only file is intentionally left as failure evidence.
        raise
    return {
        "record_type": "feppml-windows-results-bundle-v1",
        "run_id": run_id,
        "sha256": qualification.sha256_file(destination),
        "size_bytes": destination.stat().st_size,
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--artifact", type=Path, required=True)
    result.add_argument("--checksum", type=Path, required=True)
    result.add_argument("--capture", type=Path, required=True)
    result.add_argument("--target-receipt", type=Path, required=True)
    result.add_argument("--source-manifest", type=Path, required=True)
    result.add_argument("--built-verification", type=Path, required=True)
    result.add_argument("--staged-verification", type=Path, required=True)
    result.add_argument("--capture-inputs", type=Path, required=True)
    result.add_argument("--capture-verification", type=Path, required=True)
    result.add_argument("--output", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        result = bundle(
            artifact=args.artifact,
            checksum=args.checksum,
            capture=args.capture,
            target_receipt=args.target_receipt,
            source_manifest=args.source_manifest,
            built_verification=args.built_verification,
            staged_verification=args.staged_verification,
            capture_inputs=args.capture_inputs,
            capture_verification=args.capture_verification,
            output=args.output,
        )
    except (BundleError, platform.PlatformReceiptError, OSError, UnicodeError) as error:
        raise SystemExit(f"Windows result bundle failed closed: {error}") from error
    print(json.dumps(result, sort_keys=True))
    print("FEPPML_WINDOWS_RESULTS_BUNDLE_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
