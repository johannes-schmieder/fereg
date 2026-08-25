#!/usr/bin/env python3
"""Create a deterministic, exact-SHA fereg 0.9.0 source archive."""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

try:
    from . import stage_release
except ImportError:  # Direct CLI execution from scripts/.
    import stage_release

PACKAGE_NAME = f"{stage_release.PACKAGE_NAME}-source"
EXCLUDED_PREFIXES = (
    ".ci/",
    "build_evidence/",
    "benchmarks/reports/captured/",
    "docs/references/",
    "dist/",
    "native-artifacts/",
    "release-artifacts/",
    "target/",
)
EXCLUDED_FILES = {
    "stata/_fereg_estimate_ols_test.ado",
    "stata/_fereg_ols_hidden_numeric_v1.ado",
    "stata/_fereg_prepare_ols_test.ado",
}
EXCLUDED_SUFFIXES = (".log", ".pdf", ".mlib", ".pyc")


def tracked_paths(source: Path) -> list[Path]:
    completed = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=source,
        check=True,
        capture_output=True,
    )
    paths: list[Path] = []
    for encoded in completed.stdout.split(b"\0"):
        if not encoded:
            continue
        relative_text = encoded.decode("utf-8")
        if relative_text in EXCLUDED_FILES:
            continue
        if relative_text.startswith(EXCLUDED_PREFIXES):
            continue
        if relative_text.endswith(EXCLUDED_SUFFIXES):
            continue
        path = source / relative_text
        if path.is_file():
            paths.append(path)
    return sorted(paths, key=lambda item: item.relative_to(source).as_posix())


def copy_source_files(source: Path, staging: Path) -> list[Path]:
    copied: list[Path] = []
    for path in tracked_paths(source):
        relative = path.relative_to(source)
        target = staging / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        copied.append(target)
    return copied


def write_source_manifest(
    staging: Path, source_sha: str, source_dirty: bool
) -> None:
    excluded = {"MANIFEST.sha256", "SOURCE_MANIFEST.json"}
    records = []
    for path in sorted(staging.rglob("*")):
        if not path.is_file() or path.name in excluded:
            continue
        records.append(
            {
                "path": path.relative_to(staging).as_posix(),
                "sha256": stage_release.digest(path),
                "bytes": path.stat().st_size,
            }
        )
    (staging / "MANIFEST.sha256").write_text(
        "".join(f"{row['sha256']}  {row['path']}\n" for row in records),
        encoding="utf-8",
    )
    metadata = {
        "schema": 2,
        "package": PACKAGE_NAME,
        "version": stage_release.VERSION,
        "phase": "release-candidate-source",
        "created_utc": stage_release.FIXED_UTC,
        "source_sha": source_sha,
        "source_dirty": source_dirty,
        "support_caveats": list(stage_release.SUPPORT_CAVEATS),
        "excluded_developer_adapters": sorted(EXCLUDED_FILES),
        "excluded_prefixes": list(EXCLUDED_PREFIXES),
        "files": records,
    }
    (staging / "SOURCE_MANIFEST.json").write_text(
        json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def create(
    source: Path,
    audit_path: Path,
    output: Path,
    *,
    allow_dirty: bool = False,
) -> None:
    stage_release.require_external_output(source, output)
    source_sha, source_dirty = stage_release.source_identity(source)
    if source_dirty and not allow_dirty:
        raise RuntimeError(
            "tracked source is dirty; commit before staging a source archive"
        )
    stage_release.validate_dependency_audit(source, audit_path, source_sha)
    with tempfile.TemporaryDirectory(prefix="fereg-source-") as temporary:
        staging = Path(temporary) / PACKAGE_NAME
        staging.mkdir()
        copy_source_files(source, staging)
        shutil.copy2(audit_path, staging / "DEPENDENCY_AUDIT.json")
        stage_release.write_sbom(
            source, staging / "SBOM.spdx.json", source_sha
        )
        write_source_manifest(staging, source_sha, source_dirty)
        stage_release.write_zip(staging, output, package_name=PACKAGE_NAME)
    print(f"created {output}")
    print(f"sha256 {stage_release.digest(output)}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=Path.cwd())
    parser.add_argument("--dependency-audit", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    create(
        args.source.resolve(),
        args.dependency_audit.resolve(),
        args.output.resolve(),
    )


if __name__ == "__main__":
    main()
