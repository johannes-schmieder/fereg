#!/usr/bin/env python3
"""Check the root Stata package and its native build's runtime identity."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

import check_complexity
import stage_release

ROOT = Path(__file__).resolve().parents[1]


def runtime_identity(root: Path) -> str:
    inputs = {
        "rust": check_complexity.production_streams(root),
        "artifacts": {
            name: hashlib.sha256(data).hexdigest()
            for name, data in check_complexity.artifact_bytes(root).items()
        },
    }
    return hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest()


def check(root: Path) -> None:
    metadata = json.loads((root / "dist/fereg-native.json").read_text())
    if metadata["runtime_sha256"] != runtime_identity(root):
        raise RuntimeError("main's runtime changed: rebuild and qualify the installed plugin")
    plugins = tuple(item["path"] for item in metadata["plugins"])
    stage_release.verify_plugins(root / "dist", plugins)
    for item in metadata["plugins"]:
        if stage_release.digest(root / "dist" / item["path"]) != item["sha256"]:
            raise RuntimeError("native build identity does not match the installed plugin")
    stage_release.validate_ppml_mata_objects(root / "stata")
    expected = {
        f"stata/{name}" for name in stage_release.PRODUCTION_STATA_FILES
        if name not in {"fereg.pkg", "stata.toc"}
    }
    expected.update(f"dist/{name}" for name in plugins)
    expected.update(f"dist/{name}.sha256" for name in plugins)
    expected.update(f"dist/{name}" for name in (
        "fereg-LICENSE.txt", "fereg-THIRD-PARTY-NOTICES.txt", "fereg-SOURCE.txt",
        "fereg-native.json",
    ))
    paths = []
    for line in (root / "fereg.pkg").read_text().splitlines():
        fields = line.split()
        if not fields or fields[0] not in {"f", "F"}:
            continue
        if len(fields) != 2:
            raise RuntimeError(f"invalid package file directive: {line}")
        path = fields[1]
        if path not in expected or not (root / path).is_file():
            raise RuntimeError(f"missing or unexpected installation file: {path}")
        if Path(path).suffix in {".txt", ".json", ".sha256"} and fields[0] != "F":
            raise RuntimeError(f"installation metadata needs uppercase F: {path}")
        paths.append(path)
    if len(paths) != len(expected) or set(paths) != expected:
        raise RuntimeError("root package inventory is incomplete or duplicated")
    if not any(line.startswith("p fereg ") for line in (root / "stata.toc").read_text().splitlines()):
        raise RuntimeError("root stata.toc does not advertise fereg")
    print(f"GitHub installation: PASS ({len(paths)} files, runtime bound to {metadata['source_sha'][:12]})")


if __name__ == "__main__":
    check(ROOT)
