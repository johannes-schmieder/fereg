#!/usr/bin/env python3
"""Run cargo-audit and write the exact-SHA receipt required by release staging."""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

try:
    from . import stage_release
except ImportError:  # Direct CLI execution from scripts/.
    import stage_release


def advisory_count(payload: dict[str, object]) -> int:
    vulnerabilities = payload.get("vulnerabilities")
    if not isinstance(vulnerabilities, dict):
        raise RuntimeError("cargo-audit JSON has no vulnerabilities object")
    count = vulnerabilities.get("count")
    if not isinstance(count, int) or count < 0:
        raise RuntimeError("cargo-audit JSON has an invalid vulnerability count")
    return count


def rust_toolchain(rustc: Path) -> str:
    completed = subprocess.run(
        [str(rustc), "--version"],
        check=True,
        capture_output=True,
        text=True,
    )
    fields = completed.stdout.split()
    if len(fields) < 2:
        raise RuntimeError("unable to parse rustc version")
    return fields[1]


def run(source: Path, cargo: Path, rustc: Path, output: Path) -> None:
    stage_release.require_external_output(source, output)
    source_sha, source_dirty = stage_release.source_identity(source)
    if source_dirty:
        raise RuntimeError("tracked source is dirty; dependency audit needs an exact SHA")
    toolchain = rust_toolchain(rustc)
    if toolchain != "1.85.1":
        raise RuntimeError(f"expected Rust 1.85.1, got {toolchain}")
    completed = subprocess.run(
        [str(cargo), "audit", "--json", "--file", str(source / "Cargo.lock")],
        cwd=source,
        capture_output=True,
        text=True,
    )
    try:
        raw = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RuntimeError(f"cargo audit did not return JSON: {detail}") from error
    count = advisory_count(raw)
    if completed.returncode != 0 or count != 0:
        raise RuntimeError(
            f"cargo audit failed with rc={completed.returncode}, advisories={count}"
        )
    receipt = {
        "schema": 1,
        "status": "pass",
        "source_sha": source_sha,
        "cargo_lock_sha256": stage_release.digest(source / "Cargo.lock"),
        "advisory_count": count,
        "toolchain": toolchain,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=Path.cwd())
    parser.add_argument("--cargo", type=Path, required=True)
    parser.add_argument("--rustc", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    run(
        args.source.resolve(),
        args.cargo.resolve(),
        args.rustc.resolve(),
        args.output.resolve(),
    )


if __name__ == "__main__":
    main()
