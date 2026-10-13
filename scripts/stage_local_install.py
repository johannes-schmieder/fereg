#!/usr/bin/env python3
"""Prepare a current-platform package for a local Stata net install."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import tempfile

try:
    from . import stage_release
except ImportError:
    import stage_release


def stage(source: Path, plugins: Path, output: Path, platform: str) -> None:
    """Validate inputs before creating a new, self-contained installation directory."""
    if output.exists():
        raise RuntimeError("output already exists; choose a new --output directory")
    names = stage_release.selected_plugins((platform,))
    verified = stage_release.verify_plugins(plugins, names)
    stage_release.validate_ppml_mata_objects(source / "stata")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="fereg-install-", dir=output.parent) as temporary:
        staging = Path(temporary) / "package"
        staging.mkdir()
        stage_release.copy_release_files(source, staging, verified)
        stage_release.filter_package(staging, names)
        records = stage_release.file_records(staging, set())
        (staging / "LOCAL_INSTALL.json").write_text(json.dumps({
            "schema": "fereg-local-install-v1", "platform": platform,
            "files": records,
        }, indent=2) + "\n", encoding="utf-8")
        shutil.move(str(staging), str(output))


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=root)
    parser.add_argument("--plugins", type=Path)
    parser.add_argument("--output", type=Path, default=root / "target" / "local-install")
    parser.add_argument("--platform", required=True, choices=tuple(stage_release.PLATFORM_PLUGINS))
    args = parser.parse_args()
    source = args.source.resolve()
    output = args.output.resolve()
    plugins = (args.plugins or source / "dist").resolve()
    stage(source, plugins, output, args.platform)
    print("In a fresh Stata session, run:")
    print(f'net install fereg, from("{output.as_posix()}") replace')
    print("fereg, selftest")


if __name__ == "__main__":
    main()
