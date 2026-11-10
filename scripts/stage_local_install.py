#!/usr/bin/env python3
"""Prepare a current-platform package for a local Stata net install.

`--root-package` instead stages a candidate of the repository's root package
(fereg.pkg, stata.toc, stata/ and dist/ with fereg-native.json) for the named
platform from a clean source, outside the source tree, for
scripts/check_github_install.py --package-root and a local net install. It
never writes the repository's dist/ and is not a publication package.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import sys
import tempfile

try:
    from . import stage_release
except ImportError:
    import stage_release

ROOT_PACKAGE_CAVEATS = (
    "Staged candidate for local installed-package admission; not a published package.",
    "Only platforms named in included_platforms are supplied by this package.",
    "Its notices omit the vendored dependency license texts that a corresponding-source archive supplies.",
)


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


def stage_root(source: Path, plugins: Path, output: Path, platform: str) -> None:
    """Stage a one-platform candidate of the root package bound to a clean source."""
    # The runtime identity's helpers import one another as top-level scripts.
    scripts = str(Path(__file__).resolve().parent)
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    import check_github_install

    if output.exists():
        raise RuntimeError("output already exists; choose a new --output directory")
    stage_release.require_external_output(source, output)
    source_sha, dirty = stage_release.source_identity(source)
    if dirty:
        raise RuntimeError("tracked source is dirty; commit before staging a candidate")
    names = stage_release.selected_plugins((platform,))
    verified = stage_release.verify_plugins(plugins, names)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="fereg-root-", dir=output.parent) as temporary:
        flat = Path(temporary) / "flat"
        flat.mkdir()
        # The release file set (it validates the Mata objects), arranged as
        # the root package: frontend in stata/, native files in dist/.
        stage_release.copy_release_files(source, flat, verified)
        staging = Path(temporary) / "package"
        (staging / "stata").mkdir(parents=True)
        (staging / "dist").mkdir()
        for name in stage_release.PRODUCTION_STATA_FILES:
            if name not in {"fereg.pkg", "stata.toc"}:
                shutil.copy2(flat / name, staging / "stata" / name)
        for plugin, _ in verified:
            shutil.copy2(flat / plugin.name, staging / "dist" / plugin.name)
            shutil.copy2(flat / f"{plugin.name}.sha256", staging / "dist" / f"{plugin.name}.sha256")
        for name in ("fereg-LICENSE.txt", "fereg-THIRD-PARTY-NOTICES.txt", "fereg-SOURCE.txt"):
            shutil.copy2(flat / name, staging / "dist" / name)
        shutil.copy2(source / "stata.toc", staging / "stata.toc")
        omitted = {f"dist/{name}{suffix}" for name in stage_release.EXPECTED_PLUGINS
                   if name not in names for suffix in ("", ".sha256")}
        lines = (source / "fereg.pkg").read_text(encoding="utf-8").splitlines(keepends=True)
        (staging / "fereg.pkg").write_text(
            "".join(line for line in lines
                    if not (len(line.split()) == 2 and line.split()[0] in {"f", "F"}
                            and line.split()[1] in omitted)),
            encoding="utf-8",
        )
        metadata = {
            "schema": 1,
            "version": stage_release.VERSION,
            "source_sha": source_sha,
            "runtime_sha256": check_github_install.runtime_identity(source),
            "plugins": [
                {
                    "path": plugin.name,
                    "source_sha": source_sha,
                    "sha256": stage_release.digest(plugin),
                    "bytes": plugin.stat().st_size,
                }
                for plugin, _ in verified
            ],
            "included_platforms": [platform],
            "support_caveats": list(ROOT_PACKAGE_CAVEATS),
            "candidate": True,
        }
        (staging / "dist" / "fereg-native.json").write_text(
            json.dumps(metadata, indent=2) + "\n", encoding="utf-8"
        )
        shutil.move(str(staging), str(output))


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=root)
    parser.add_argument("--plugins", type=Path)
    parser.add_argument("--output", type=Path, default=root / "target" / "local-install")
    parser.add_argument("--platform", required=True, choices=tuple(stage_release.PLATFORM_PLUGINS))
    parser.add_argument("--root-package", action="store_true",
                        help="stage a candidate root package from a clean source")
    args = parser.parse_args()
    source = args.source.resolve()
    output = args.output.resolve()
    plugins = (args.plugins or source / "dist").resolve()
    if args.root_package:
        stage_root(source, plugins, output, args.platform)
        print(f"Staged candidate root package {output}; check it with:")
        print(f"python3 scripts/check_github_install.py --package-root {output}")
    else:
        stage(source, plugins, output, args.platform)
    print("In a fresh Stata session, run:")
    print(f'net install fereg, from("{output.as_posix()}") replace')
    print("fereg, selftest")


if __name__ == "__main__":
    main()
