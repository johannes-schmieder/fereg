#!/usr/bin/env python3
"""Stage the deterministic, audited fereg 0.5.0 beta distribution."""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import stat
import subprocess
import tempfile
import zipfile
from pathlib import Path
from typing import Optional

VERSION = "0.5.0"
PACKAGE_NAME = f"fereg-{VERSION}"
FIXED_TIME = (2026, 8, 24, 0, 0, 0)
FIXED_UTC = "2026-08-24T00:00:00Z"
EXPECTED_PLUGINS = (
    "fereg_linux.plugin",
    "fereg_windows.plugin",
    "fereg_macos.plugin",
)
PLATFORM_PLUGINS = {
    "linux": "fereg_linux.plugin",
    "windows": "fereg_windows.plugin",
    "macos": "fereg_macos.plugin",
}
PRODUCTION_STATA_FILES = (
    "fereg.ado",
    "fereg_run.ado",
    "_fereg_estimate_public.ado",
    "_fereg_ols.ado",
    "_fereg_estimate_ols.ado",
    "_fereg_load.ado",
    "_fereg_memory_baseline.ado",
    "_fereg_memory_validate.ado",
    "feppml.ado",
    "_fereg_ppml.ado",
    "_fereg_estimate_ppml.ado",
    "_fereg_ppml_result_mata.ado",
    "_fereg_ppml_result_mata.json",
    "_fereg_ppml_validation_error.mo",
    "_fereg_ppml_validate_result.mo",
    "_fereg_ppml_post_scalars.mo",
    "feppml_p.ado",
    "fereg_parse.ado",
    "fereg_p.ado",
    "fereg_estat.ado",
    "fereg.sthlp",
    "feppml.sthlp",
    "fereg_parse.sthlp",
    "fereg_postestimation.sthlp",
    "fereg.pkg",
    "stata.toc",
)
ROOT_FILES = (
    "README.md",
    "STATUS.md",
    "CHANGELOG.md",
    "LICENSE",
    "THIRD_PARTY_NOTICES.md",
    "references.lock.toml",
)
SUPPORT_CAVEATS = (
    "Licensed Windows Stata qualification is deferred; Windows x64 is "
    "qualified by hosted MSVC build, binary audit, and mock SPI only.",
    "Native Intel-host macOS Stata qualification is deferred; the x86_64 "
    "slice is exercised under Rosetta on Apple Silicon.",
)


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            value.update(block)
    return value.hexdigest()


def git_output(source: Path, *arguments: str) -> str:
    completed = subprocess.run(
        ["git", *arguments],
        cwd=source,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def source_identity(source: Path) -> tuple[str, bool]:
    sha = git_output(source, "rev-parse", "HEAD")
    dirty = bool(
        git_output(source, "status", "--porcelain=v1", "--untracked-files=no")
    )
    return sha, dirty


def require_external_output(source: Path, output: Path) -> None:
    source = source.resolve()
    output = output.resolve()
    if output == source or source in output.parents:
        raise RuntimeError("release artifacts must be staged outside the source tree")


def expected_digest(checksum_path: Path) -> str:
    fields = checksum_path.read_text(encoding="ascii").strip().split()
    if not fields or len(fields[0]) != 64:
        raise RuntimeError(f"invalid checksum file: {checksum_path}")
    try:
        int(fields[0], 16)
    except ValueError as error:
        raise RuntimeError(f"invalid checksum digest: {checksum_path}") from error
    return fields[0].lower()


def verify_plugins(
    directory: Path, names: tuple[str, ...] = EXPECTED_PLUGINS
) -> list[tuple[Path, Path]]:
    verified: list[tuple[Path, Path]] = []
    for name in names:
        plugin = directory / name
        checksum = directory / f"{name}.sha256"
        if not plugin.is_file() or plugin.stat().st_size == 0:
            raise RuntimeError(f"missing or empty native artifact: {plugin}")
        if not checksum.is_file():
            raise RuntimeError(f"missing native checksum: {checksum}")
        expected = expected_digest(checksum)
        actual = digest(plugin)
        if actual != expected:
            raise RuntimeError(
                f"native checksum mismatch for {name}: expected {expected}, got {actual}"
            )
        verified.append((plugin, checksum))
    return verified


def validate_dependency_audit(
    source: Path, audit_path: Path, source_sha: str
) -> dict[str, object]:
    if not audit_path.is_file():
        raise RuntimeError(f"missing dependency audit receipt: {audit_path}")
    try:
        audit = json.loads(audit_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise RuntimeError(f"invalid dependency audit receipt: {audit_path}") from error
    required = {
        "schema",
        "status",
        "source_sha",
        "cargo_lock_sha256",
        "advisory_count",
        "toolchain",
    }
    if not isinstance(audit, dict) or set(audit) != required:
        raise RuntimeError("dependency audit receipt has the wrong schema")
    if audit["schema"] != 1 or audit["status"] != "pass":
        raise RuntimeError("dependency audit did not pass")
    if audit["source_sha"] != source_sha:
        raise RuntimeError("dependency audit is not bound to the source SHA")
    if audit["cargo_lock_sha256"] != digest(source / "Cargo.lock"):
        raise RuntimeError("dependency audit is not bound to Cargo.lock")
    if audit["advisory_count"] != 0 or audit["toolchain"] != "1.85.1":
        raise RuntimeError("dependency audit has advisories or the wrong toolchain")
    return audit


def cargo_packages(source: Path) -> list[dict[str, object]]:
    packages: list[dict[str, object]] = []
    current: Optional[dict[str, object]] = None
    for raw_line in (source / "Cargo.lock").read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if line == "[[package]]":
            if current is not None:
                packages.append(current)
            current = {}
            continue
        if current is None or "=" not in line:
            continue
        key, raw_value = (field.strip() for field in line.split("=", 1))
        if key not in {"name", "version", "source", "checksum"}:
            continue
        if len(raw_value) < 2 or raw_value[0] != '"' or raw_value[-1] != '"':
            raise RuntimeError(f"unsupported Cargo.lock value for {key}")
        current[key] = raw_value[1:-1]
    if current is not None:
        packages.append(current)
    if not packages or any(
        "name" not in package or "version" not in package for package in packages
    ):
        raise RuntimeError("Cargo.lock does not contain a valid package list")
    return packages


def write_sbom(source: Path, output: Path, source_sha: str) -> None:
    packages: list[dict[str, object]] = [
        {
            "SPDXID": "SPDXRef-Package-fereg",
            "name": "fereg",
            "versionInfo": VERSION,
            "downloadLocation": "NOASSERTION",
            "filesAnalyzed": False,
            "licenseConcluded": "MIT",
            "licenseDeclared": "MIT",
            "supplier": "Organization: fereg contributors",
            "checksums": [
                {"algorithm": "SHA1", "checksumValue": source_sha}
            ],
        }
    ]
    relationships: list[dict[str, str]] = []
    for index, package in enumerate(cargo_packages(source), start=1):
        name = str(package["name"])
        version = str(package["version"])
        spdx_id = f"SPDXRef-Cargo-{index}"
        entry: dict[str, object] = {
            "SPDXID": spdx_id,
            "name": name,
            "versionInfo": version,
            "downloadLocation": str(package.get("source", "NOASSERTION")),
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": "NOASSERTION",
        }
        checksum = package.get("checksum")
        if isinstance(checksum, str):
            entry["checksums"] = [
                {"algorithm": "SHA256", "checksumValue": checksum}
            ]
        packages.append(entry)
        if name not in {"fereg-core", "fereg-plugin", "fereg-testkit"}:
            relationships.append(
                {
                    "spdxElementId": "SPDXRef-Package-fereg",
                    "relationshipType": "DEPENDS_ON",
                    "relatedSpdxElement": spdx_id,
                }
            )
    document = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"{PACKAGE_NAME}-sbom",
        "documentNamespace": (
            f"https://github.com/johannes-schmieder/fereg/spdx/{VERSION}/{source_sha}"
        ),
        "creationInfo": {
            "created": FIXED_UTC,
            "creators": ["Tool: fereg stage_release.py"],
        },
        "documentDescribes": ["SPDXRef-Package-fereg"],
        "packages": packages,
        "relationships": relationships,
    }
    output.write_text(
        json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def validate_ppml_mata_objects(stata: Path) -> None:
    receipt = json.loads((stata / "_fereg_ppml_result_mata.json").read_text())
    expected = {name for name in PRODUCTION_STATA_FILES if name.endswith(".mo")}
    if (receipt.get("schema") != "fereg-ppml-mata-v1"
            or receipt.get("compiler_release") != "18"
            or set(receipt.get("objects", {})) != expected
            or receipt.get("source_sha256") != digest(stata / "_fereg_ppml_result_mata.ado")
            or any(receipt["objects"][name] != digest(stata / name) for name in expected)):
        raise RuntimeError("PPML Mata objects are stale or lack a Stata 18 source binding")


def copy_release_files(
    source: Path,
    staging: Path,
    plugins: list[tuple[Path, Path]],
) -> None:
    validate_ppml_mata_objects(source / "stata")
    for name in PRODUCTION_STATA_FILES:
        path = source / "stata" / name
        if not path.is_file():
            raise RuntimeError(f"missing production Stata file: {path}")
        shutil.copy2(path, staging / name)
    for plugin, checksum in plugins:
        shutil.copy2(plugin, staging / plugin.name)
        shutil.copy2(checksum, staging / checksum.name)
    for name in ROOT_FILES:
        path = source / name
        if not path.is_file():
            raise RuntimeError(f"missing release file: {path}")
        shutil.copy2(path, staging / name)


def file_records(staging: Path, excluded: set[str]) -> list[dict[str, object]]:
    records = []
    for path in sorted(staging.iterdir(), key=lambda item: item.name):
        if not path.is_file() or path.name in excluded:
            continue
        records.append(
            {"path": path.name, "sha256": digest(path), "bytes": path.stat().st_size}
        )
    return records


def write_manifest(
    staging: Path, source_sha: str, source_dirty: bool,
    *, review_platforms: Optional[tuple[str, ...]] = None,
) -> None:
    plugin_names = selected_plugins(review_platforms)
    records = file_records(
        staging, {"MANIFEST.sha256", "RELEASE_MANIFEST.json"}
    )
    (staging / "MANIFEST.sha256").write_text(
        "".join(f"{row['sha256']}  {row['path']}\n" for row in records),
        encoding="utf-8",
    )
    metadata = {
        "schema": 2,
        "package": PACKAGE_NAME,
        "version": VERSION,
        "phase": "beta",
        "created_utc": FIXED_UTC,
        "source_sha": source_sha,
        "source_dirty": source_dirty,
        "action_protocol": "1.6",
        "memory_result_schema": 2,
        "ols_result_wire": "1.5",
        "ppml_request_wire": "1.6",
        "ppml_result_wire": "1.3",
        "model_wire": "2.1",
        "plugins": [
            {
                "path": plugin,
                "sha256": expected_digest(staging / f"{plugin}.sha256"),
            }
            for plugin in plugin_names
        ],
        "support_caveats": list(SUPPORT_CAVEATS),
        "files": records,
    }
    if review_platforms is not None:
        metadata.update(
            schema=3,
            package=f"{PACKAGE_NAME}-review",
            phase="beta-review",
            review_only=True,
            publication_eligible=False,
            included_platforms=[p for p in PLATFORM_PLUGINS if p in review_platforms],
            missing_platforms=[p for p in PLATFORM_PLUGINS if p not in review_platforms],
            support_caveats=[
                "Review-only package; not a qualified release or publication candidate.",
                "Only the explicitly included platform binaries are present.",
                "Licensed Windows and native Intel-host Mac qualification remain deferred.",
            ],
        )
    (staging / "RELEASE_MANIFEST.json").write_text(
        json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def selected_plugins(review_platforms: Optional[tuple[str, ...]]) -> tuple[str, ...]:
    if review_platforms is None:
        return EXPECTED_PLUGINS
    if (not review_platforms or len(set(review_platforms)) != len(review_platforms)
            or not set(review_platforms) <= PLATFORM_PLUGINS.keys()):
        raise RuntimeError("review platforms must be a nonempty unique supported selection")
    return tuple(name for platform, name in PLATFORM_PLUGINS.items()
                 if platform in review_platforms)


def write_zip(staging: Path, output: Path, package_name: str = PACKAGE_NAME) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(
        output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
    ) as archive:
        for path in sorted(staging.rglob("*")):
            if not path.is_file():
                continue
            relative = path.relative_to(staging)
            info = zipfile.ZipInfo(f"{package_name}/{relative.as_posix()}", FIXED_TIME)
            permissions = 0o755 if path.stat().st_mode & stat.S_IXUSR else 0o644
            info.external_attr = (permissions & 0xFFFF) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, path.read_bytes())


def create(
    source: Path,
    plugin_directory: Path,
    audit_path: Path,
    output: Path,
    *,
    allow_dirty: bool = False,
    review_platforms: Optional[tuple[str, ...]] = None,
) -> None:
    require_external_output(source, output)
    source_sha, source_dirty = source_identity(source)
    if source_dirty and not allow_dirty:
        raise RuntimeError("tracked source is dirty; commit before staging a release")
    plugins = verify_plugins(plugin_directory, selected_plugins(review_platforms))
    validate_dependency_audit(source, audit_path, source_sha)
    with tempfile.TemporaryDirectory(prefix="fereg-release-") as temporary:
        package_name = PACKAGE_NAME if review_platforms is None else f"{PACKAGE_NAME}-review"
        staging = Path(temporary) / package_name
        staging.mkdir()
        copy_release_files(source, staging, plugins)
        if review_platforms is not None:
            omitted = set(EXPECTED_PLUGINS) - set(selected_plugins(review_platforms))
            package = staging / "fereg.pkg"
            lines = package.read_text(encoding="utf-8").splitlines(keepends=True)
            package.write_text("".join(line for line in lines
                                      if line.strip() not in {f"f {name}" for name in omitted}),
                               encoding="utf-8")
        shutil.copy2(audit_path, staging / "DEPENDENCY_AUDIT.json")
        write_sbom(source, staging / "SBOM.spdx.json", source_sha)
        write_manifest(staging, source_sha, source_dirty, review_platforms=review_platforms)
        write_zip(staging, output, package_name)
    print(f"created {output}")
    print(f"sha256 {digest(output)}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=Path.cwd())
    parser.add_argument("--plugins", type=Path, required=True)
    parser.add_argument("--dependency-audit", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--review-platforms", nargs="+", choices=tuple(PLATFORM_PLUGINS),
                        help="create a visibly review-only subset; never a release package")
    args = parser.parse_args()
    create(
        args.source.resolve(),
        args.plugins.resolve(),
        args.dependency_audit.resolve(),
        args.output.resolve(),
        review_platforms=None if args.review_platforms is None else tuple(args.review_platforms),
    )


if __name__ == "__main__":
    main()
