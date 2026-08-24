#!/usr/bin/env python3
"""Create a deterministic source/handoff ZIP and cryptographic manifest."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import zipfile
from pathlib import Path

FIXED_TIME = (2026, 8, 20, 0, 0, 0)
EXCLUDED_DIRS = {
    ".git",
    ".pytest_cache",
    "__pycache__",
    "build",
    "native-artifacts",
    "release-artifacts",
    "target",
}
EXCLUDED_NAMES = {"MANIFEST.sha256", "SOURCE_MANIFEST.json"}


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def selected_files(source: Path, output: Path) -> list[Path]:
    files: list[Path] = []
    for path in source.rglob("*"):
        if not path.is_file():
            continue
        relative = path.relative_to(source)
        if any(part in EXCLUDED_DIRS for part in relative.parts):
            continue
        if relative.name in EXCLUDED_NAMES or path.resolve() == output.resolve():
            continue
        files.append(path)
    return sorted(files, key=lambda item: item.relative_to(source).as_posix())


def create(source: Path, output: Path) -> None:
    files = selected_files(source, output)
    records = [
        {
            "path": path.relative_to(source).as_posix(),
            "sha256": digest(path),
            "bytes": path.stat().st_size,
        }
        for path in files
    ]
    manifest_text = "".join(f"{record['sha256']}  {record['path']}\n" for record in records)
    (source / "MANIFEST.sha256").write_text(manifest_text, encoding="utf-8")
    metadata = {
        "schema": 1,
        "package": "fereg-0.1.0-phase01",
        "created_utc": "2026-08-20T00:00:00Z",
        "files": records,
    }
    (source / "SOURCE_MANIFEST.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    files = selected_files(source, output) + [source / "MANIFEST.sha256", source / "SOURCE_MANIFEST.json"]
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(files, key=lambda item: item.relative_to(source).as_posix()):
            relative = Path(source.name) / path.relative_to(source)
            info = zipfile.ZipInfo(relative.as_posix(), FIXED_TIME)
            mode = path.stat().st_mode
            permissions = 0o755 if mode & stat.S_IXUSR else 0o644
            info.external_attr = (permissions & 0xFFFF) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, path.read_bytes())
    print(f"created {output}")
    print(f"sha256 {digest(output)}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    create(args.source.resolve(), args.output.resolve())
