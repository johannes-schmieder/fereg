#!/usr/bin/env python3
"""Assemble verified platform plugins and Stata files into a release ZIP."""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import stat
import tempfile
import zipfile
from pathlib import Path

VERSION = "0.1.0-phase01"
PACKAGE_NAME = f"stfixest-{VERSION}"
FIXED_TIME = (2026, 8, 20, 0, 0, 0)
EXPECTED_PLUGINS = (
    "stfixest_linux.plugin",
    "stfixest_windows.plugin",
    "stfixest_macos.plugin",
)


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            value.update(block)
    return value.hexdigest()


def expected_digest(checksum_path: Path) -> str:
    fields = checksum_path.read_text(encoding="ascii").strip().split()
    if not fields or len(fields[0]) != 64:
        raise RuntimeError(f"invalid checksum file: {checksum_path}")
    try:
        int(fields[0], 16)
    except ValueError as error:
        raise RuntimeError(f"invalid checksum digest: {checksum_path}") from error
    return fields[0].lower()


def verify_plugins(directory: Path) -> list[Path]:
    verified: list[Path] = []
    for name in EXPECTED_PLUGINS:
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
        verified.append(plugin)
    return verified


def copy_release_files(source: Path, staging: Path, plugins: list[Path]) -> None:
    for path in sorted((source / "stata").iterdir()):
        if path.is_file():
            shutil.copy2(path, staging / path.name)
    for plugin in plugins:
        shutil.copy2(plugin, staging / plugin.name)
    for name in (
        "README.md",
        "CHANGELOG.md",
        "LICENSE",
        "THIRD_PARTY_NOTICES.md",
        "references.lock.toml",
    ):
        shutil.copy2(source / name, staging / name)


def write_manifest(staging: Path) -> None:
    records = []
    for path in sorted(staging.iterdir(), key=lambda item: item.name):
        if not path.is_file() or path.name in {"MANIFEST.sha256", "RELEASE_MANIFEST.json"}:
            continue
        records.append(
            {"path": path.name, "sha256": digest(path), "bytes": path.stat().st_size}
        )
    (staging / "MANIFEST.sha256").write_text(
        "".join(f"{row['sha256']}  {row['path']}\n" for row in records),
        encoding="utf-8",
    )
    metadata = {
        "schema": 1,
        "package": PACKAGE_NAME,
        "phase": "0-1",
        "created_utc": "2026-08-20T00:00:00Z",
        "plugins": list(EXPECTED_PLUGINS),
        "files": records,
    }
    (staging / "RELEASE_MANIFEST.json").write_text(
        json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def write_zip(staging: Path, output: Path) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(
        output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
    ) as archive:
        for path in sorted(staging.iterdir(), key=lambda item: item.name):
            if not path.is_file():
                continue
            info = zipfile.ZipInfo(f"{PACKAGE_NAME}/{path.name}", FIXED_TIME)
            permissions = 0o755 if path.stat().st_mode & stat.S_IXUSR else 0o644
            info.external_attr = (permissions & 0xFFFF) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, path.read_bytes())


def create(source: Path, plugin_directory: Path, output: Path) -> None:
    plugins = verify_plugins(plugin_directory)
    with tempfile.TemporaryDirectory(prefix="stfixest-release-") as temporary:
        staging = Path(temporary) / PACKAGE_NAME
        staging.mkdir()
        copy_release_files(source, staging, plugins)
        write_manifest(staging)
        write_zip(staging, output)
    print(f"created {output}")
    print(f"sha256 {digest(output)}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=Path.cwd())
    parser.add_argument("--plugins", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    create(args.source.resolve(), args.plugins.resolve(), args.output.resolve())


if __name__ == "__main__":
    main()
