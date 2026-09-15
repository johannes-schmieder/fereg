#!/usr/bin/env python3
"""Run licensed Stata fixtures only against an extracted release archive."""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import tempfile
import zipfile

try:
    from .run_stata_tests import (
        expected_plugin_filename,
        expected_sentinel,
        optional_int,
        run_fixture,
        sha256_file,
        write_json_atomic,
    )
except ImportError:
    from run_stata_tests import (  # type: ignore[no-redef]
        expected_plugin_filename,
        expected_sentinel,
        optional_int,
        run_fixture,
        sha256_file,
        write_json_atomic,
    )


HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
HEX_GIT_SHA = re.compile(r"^[0-9a-f]{40}$")
MAX_MEMBER_BYTES = 512 * 1024 * 1024
MAX_ARCHIVE_BYTES = 2 * 1024 * 1024 * 1024


class StagedReleaseError(RuntimeError):
    """The release archive is unsafe, incomplete, or internally inconsistent."""


@dataclass(frozen=True)
class StagedRelease:
    package_root: Path
    archive_sha256: str
    archive_bytes: int
    source_sha: str
    manifest_sha256: str
    plugin: Path
    plugin_sha256: str
    plugin_bytes: int
    review_only: bool = False


def _safe_members(archive: zipfile.ZipFile) -> tuple[str, list[zipfile.ZipInfo]]:
    members: list[zipfile.ZipInfo] = []
    seen: set[str] = set()
    seen_casefolded: set[str] = set()
    package_name: str | None = None
    total_bytes = 0
    for info in archive.infolist():
        if info.is_dir():
            raise StagedReleaseError("release archive contains an unexpected directory entry")
        if info.flag_bits & 0x1:
            raise StagedReleaseError("release archive contains an encrypted member")
        name = info.filename
        if "\\" in name or "\0" in name or "\n" in name or "\r" in name:
            raise StagedReleaseError("release archive contains an unsafe member name")
        relative = PurePosixPath(name)
        if relative.is_absolute() or len(relative.parts) != 2 or any(
            part in {"", ".", ".."} for part in relative.parts
        ):
            raise StagedReleaseError(f"release archive member is outside one flat package: {name}")
        top, basename = relative.parts
        if package_name is None:
            package_name = top
        elif top != package_name:
            raise StagedReleaseError("release archive contains multiple package roots")
        if name in seen or name.casefold() in seen_casefolded:
            raise StagedReleaseError(f"release archive contains a duplicate member: {name}")
        mode = info.external_attr >> 16
        if stat.S_ISLNK(mode):
            raise StagedReleaseError(f"release archive contains a symbolic link: {name}")
        if info.file_size < 0 or info.file_size > MAX_MEMBER_BYTES:
            raise StagedReleaseError(f"release archive member is unreasonably large: {name}")
        total_bytes += info.file_size
        if total_bytes > MAX_ARCHIVE_BYTES:
            raise StagedReleaseError("release archive expands beyond the bounded package size")
        if not basename:
            raise StagedReleaseError("release archive contains an empty filename")
        seen.add(name)
        seen_casefolded.add(name.casefold())
        members.append(info)
    if package_name is None or not members:
        raise StagedReleaseError("release archive is empty")
    return package_name, members


def _records_by_path(manifest: object) -> dict[str, tuple[str, int]]:
    if not isinstance(manifest, dict) or manifest.get("schema") not in (2, 3):
        raise StagedReleaseError("release manifest has the wrong schema")
    records = manifest.get("files")
    if not isinstance(records, list):
        raise StagedReleaseError("release manifest has no file inventory")
    result: dict[str, tuple[str, int]] = {}
    for record in records:
        if not isinstance(record, dict) or set(record) != {"path", "sha256", "bytes"}:
            raise StagedReleaseError("release manifest file record has the wrong schema")
        name = record["path"]
        digest = record["sha256"]
        size = record["bytes"]
        if (
            not isinstance(name, str)
            or not name
            or PurePosixPath(name).name != name
            or name in result
            or not isinstance(digest, str)
            or HEX_SHA256.fullmatch(digest) is None
            or not isinstance(size, int)
            or isinstance(size, bool)
            or size < 0
        ):
            raise StagedReleaseError("release manifest contains an invalid file record")
        result[name] = (digest, size)
    if not result:
        raise StagedReleaseError("release manifest file inventory is empty")
    return result


def _manifest_checksums(path: Path) -> dict[str, str]:
    checksums: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        digest, separator, name = line.partition("  ")
        if (
            not separator
            or HEX_SHA256.fullmatch(digest) is None
            or not name
            or PurePosixPath(name).name != name
            or name in checksums
        ):
            raise StagedReleaseError("MANIFEST.sha256 contains an invalid record")
        checksums[name] = digest
    if not checksums:
        raise StagedReleaseError("MANIFEST.sha256 is empty")
    return checksums


def extract_staged_release(
    archive_path: Path,
    expected_sha256: str,
    destination: Path,
    *, allow_review: bool = False,
) -> StagedRelease:
    """Safely extract and byte-validate one deterministic binary release."""
    archive_path = archive_path.resolve()
    destination = destination.resolve()
    if HEX_SHA256.fullmatch(expected_sha256) is None:
        raise StagedReleaseError("release archive SHA-256 is malformed")
    if not archive_path.is_file() or sha256_file(archive_path) != expected_sha256:
        raise StagedReleaseError("release archive does not match its SHA-256 binding")
    destination.mkdir(parents=True, exist_ok=True)
    if any(destination.iterdir()):
        raise StagedReleaseError("release extraction destination is not empty")

    try:
        with zipfile.ZipFile(archive_path) as archive:
            package_name, members = _safe_members(archive)
            package_root = destination / package_name
            package_root.mkdir()
            for info in members:
                basename = PurePosixPath(info.filename).name
                target = package_root / basename
                with archive.open(info) as source, target.open("xb") as output:
                    shutil.copyfileobj(source, output, length=1 << 20)
                if target.stat().st_size != info.file_size:
                    raise StagedReleaseError(f"release member size changed while extracting: {basename}")
    except (OSError, zipfile.BadZipFile, RuntimeError) as error:
        if isinstance(error, StagedReleaseError):
            raise
        raise StagedReleaseError(f"release archive extraction failed: {error}") from error

    release_manifest = package_root / "RELEASE_MANIFEST.json"
    checksum_manifest = package_root / "MANIFEST.sha256"
    for required in ("fereg.pkg", "stata.toc", release_manifest.name, checksum_manifest.name):
        if not (package_root / required).is_file():
            raise StagedReleaseError(f"extracted release omits {required}")
    try:
        manifest = json.loads(release_manifest.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise StagedReleaseError("release manifest is not valid JSON") from error
    records = _records_by_path(manifest)
    review_only = manifest.get("schema") == 3
    if review_only:
        if not allow_review:
            raise StagedReleaseError("review-only archive requires explicit review mode")
        if (manifest.get("review_only") is not True
                or manifest.get("publication_eligible") is not False
                or manifest.get("phase") != "beta-review"
                or package_name != "fereg-0.5.0-review"):
            raise StagedReleaseError("invalid review-only publication boundary")
        included = manifest.get("included_platforms")
        missing = manifest.get("missing_platforms")
        platforms = {"linux", "macos", "windows"}
        if (not isinstance(included, list) or not included
                or not isinstance(missing, list)
                or not all(isinstance(value, str) for value in included+missing)
                or len(set(included+missing)) != len(included+missing)
                or set(included+missing) != platforms):
            raise StagedReleaseError("invalid review platform coverage")
        expected_plugins = {f"fereg_{platform}.plugin" for platform in included}
        actual_plugins = {name for name in records if name.endswith(".plugin")}
        declared = manifest.get("plugins")
        if (actual_plugins != expected_plugins or not isinstance(declared, list)
                or len(declared) != len(expected_plugins)
                or any(not isinstance(item, dict)
                       or not isinstance(item.get("path"), str) for item in declared)
                or {item.get("path") for item in declared} != expected_plugins):
            raise StagedReleaseError("review platform inventory is inconsistent")
    elif (manifest.get("review_only") or manifest.get("phase") == "beta-review"
          or package_name.endswith("-review")):
        raise StagedReleaseError("review-only package must use the review schema")
    if manifest.get("package") != package_name or manifest.get("version") != "0.5.0":
        raise StagedReleaseError("release manifest package identity is inconsistent")
    source_sha = manifest.get("source_sha")
    if not isinstance(source_sha, str) or HEX_GIT_SHA.fullmatch(source_sha) is None:
        raise StagedReleaseError("release manifest source SHA is malformed")
    if manifest.get("source_dirty") is not False:
        raise StagedReleaseError("licensed staged-install evidence requires a clean source manifest")

    actual_names = {path.name for path in package_root.iterdir() if path.is_file()}
    expected_names = set(records) | {release_manifest.name, checksum_manifest.name}
    if actual_names != expected_names:
        raise StagedReleaseError("extracted release files disagree with the sealed inventory")
    for name, (expected_digest, expected_bytes) in records.items():
        path = package_root / name
        if path.stat().st_size != expected_bytes or sha256_file(path) != expected_digest:
            raise StagedReleaseError(f"extracted release file fails its record: {name}")
    manifest_checksums = _manifest_checksums(checksum_manifest)
    if manifest_checksums != {name: digest for name, (digest, _) in records.items()}:
        raise StagedReleaseError("MANIFEST.sha256 disagrees with RELEASE_MANIFEST.json")

    plugin_name = expected_plugin_filename()
    plugin = package_root / plugin_name
    checksum_path = package_root / f"{plugin_name}.sha256"
    if not plugin.is_file() or not checksum_path.is_file():
        raise StagedReleaseError(f"extracted release omits {plugin_name} or its checksum")
    checksum_fields = checksum_path.read_text(encoding="ascii").strip().split()
    if not checksum_fields or HEX_SHA256.fullmatch(checksum_fields[0]) is None:
        raise StagedReleaseError("platform plugin checksum file is malformed")
    plugin_sha256 = checksum_fields[0]
    if sha256_file(plugin) != plugin_sha256:
        raise StagedReleaseError("platform plugin does not match its checksum file")
    plugins = manifest.get("plugins")
    if not isinstance(plugins, list) or not any(
        isinstance(record, dict)
        and record.get("path") == plugin_name
        and record.get("sha256") == plugin_sha256
        for record in plugins
    ):
        raise StagedReleaseError("release manifest does not bind the platform plugin")
    return StagedRelease(
        package_root=package_root,
        archive_sha256=expected_sha256,
        archive_bytes=archive_path.stat().st_size,
        source_sha=source_sha,
        manifest_sha256=sha256_file(release_manifest),
        plugin=plugin,
        plugin_sha256=plugin_sha256,
        plugin_bytes=plugin.stat().st_size,
        review_only=review_only,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--archive-sha256", required=True)
    parser.add_argument("--log-dir", type=Path)
    parser.add_argument("--summary-json", type=Path)
    parser.add_argument("--review-only", action="store_true",
                        help="validate a review subset without producing release evidence")
    parser.add_argument("fixtures", nargs="+", type=Path)
    args = parser.parse_args()
    if not args.stata:
        raise SystemExit("set STATA_BIN or pass --stata")
    stata_executable = Path(args.stata).expanduser().resolve()
    if not stata_executable.is_file() or stata_executable.is_symlink():
        raise SystemExit("staged installation requires an exact Stata executable")
    root = Path(__file__).resolve().parents[1]
    fixtures: list[Path] = []
    for fixture in args.fixtures:
        candidate = fixture.resolve()
        if not candidate.is_file() or not candidate.is_relative_to(root / "tests"):
            raise SystemExit(f"fixture must be a repository test file: {fixture}")
        fixtures.append(candidate)

    with tempfile.TemporaryDirectory(prefix="fereg-staged-install-") as temporary:
        try:
            staged = extract_staged_release(
                args.archive,
                args.archive_sha256,
                Path(temporary) / "extracted",
                allow_review=args.review_only,
            )
        except StagedReleaseError as error:
            raise SystemExit(str(error)) from error
        results = []
        failures = []
        for fixture in fixtures:
            print(f"==> {fixture}")
            result = run_fixture(
                str(stata_executable),
                fixture,
                root,
                plugin=staged.plugin,
                plugin_sha256=staged.plugin_sha256,
                log_dir=args.log_dir,
                require_status=True,
                ado_root=staged.package_root,
            )
            results.append(result)
            if result.error is None:
                print(f"PASS: {expected_sentinel(fixture)}")
            else:
                failures.append(result)
                print(f"FAIL: {result.error}")
                if result.log_text:
                    print("--- fixture log tail ---")
                    print("\n".join(result.log_text.splitlines()[-60:]))
        if sha256_file(args.archive.resolve()) != staged.archive_sha256:
            raise SystemExit("release archive bytes changed during staged installation tests")
        first_metadata = next((item.metadata for item in results if item.metadata), {})
        summary = {
            "schema_version": 1,
            "completed": True,
            "status": "failure" if failures else "success",
            "archive_sha256": staged.archive_sha256,
            "archive_bytes": staged.archive_bytes,
            "source_sha": staged.source_sha,
            "manifest_sha256": staged.manifest_sha256,
            "plugin": staged.plugin.name,
            "plugin_sha256": staged.plugin_sha256,
            "plugin_bytes": staged.plugin_bytes,
            "stata_executable": str(stata_executable),
            "stata_executable_sha256": sha256_file(stata_executable),
            "stata_executable_size_bytes": stata_executable.stat().st_size,
            "stata_version": first_metadata.get("stata_version"),
            "stata_edition": first_metadata.get("stata_edition"),
            "stata_os": first_metadata.get("stata_os"),
            "stata_machine_type": first_metadata.get("stata_machine_type"),
            "stata_processors": optional_int(first_metadata.get("stata_processors")),
            "tests_passed": len(results) - len(failures),
            "tests_failed": len(failures),
            "results": [
                {
                    "fixture": result.path.relative_to(root).as_posix(),
                    "status": "failure" if result.error else "success",
                    "failure_kind": result.failure_kind,
                    "failure_detail": result.error,
                    "process_rc": result.process_returncode,
                    "stata_rc": result.stata_rc,
                }
                for result in results
            ],
        }
        if staged.review_only:
            summary.update(review_only=True, publication_eligible=False,
                           qualification_eligible=False)
        if args.summary_json is not None:
            write_json_atomic(args.summary_json, summary)
        if failures:
            raise SystemExit("staged Stata installation tests failed")


if __name__ == "__main__":
    main()
