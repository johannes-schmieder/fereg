#!/usr/bin/env python3
"""Closed structural inventory of both native actions; not a byte-coverage proof.

The OLS/F0 route guard remains independent. This companion also watches PPML,
separation, retained sessions and locally vendored runtime source. Build wiring
is bound separately: Cargo.lock alone does not identify local path patches.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys

if __package__:
    from . import fereg_allocation_source_contract as source
else:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import fereg_allocation_source_contract as source

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_INVENTORY = ROOT / "benchmarks/manifests/action_allocation_inventory_v24.json"
SOURCE_GLOBS = (
    "legacy/fereg-core/src/**/*.rs",
    "crates/fereg-plugin/src/**/*.rs",
    "vendor/*/src/**/*.rs",
)
BUILD_INPUT_GLOBS = (
    "Cargo.toml",
    ".cargo/config",
    ".cargo/config.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    "crates/*/Cargo.toml",
    "crates/*/build.rs",
    "legacy/*/Cargo.toml",
    "legacy/*/build.rs",
    "vendor/*/Cargo.toml",
    "vendor/*/Cargo.lock",
    "vendor/*/build.rs",
)


def build_inventory(root: Path) -> dict:
    paths = sorted({p.relative_to(root).as_posix() for glob in SOURCE_GLOBS for p in root.glob(glob)})
    records = []
    for relative in paths:
        text = (root / relative).read_text(encoding="utf-8")
        tokens = source._production_tokens(text)
        sites = source._allocation_sites(tokens)
        records.append({
            "path": relative,
            "production_tokens_sha256": source._sha256_text(source._canonical_tokens(tokens)),
            "production_literals_sha256": source._sha256_text(source._canonical_json(source._production_literal_payloads(text, tokens))),
            "module_bindings_sha256": source._sha256_text(source._canonical_json(source._module_bindings(text))),
            "allocation_site_count": len(sites),
            "allocation_sites_sha256": source._sha256_text(source._canonical_json(sites)),
        })
    return {
        "schema_version": 2,
        "coverage": "structural_only_not_capacity_or_reachability",
        "source_globs": list(SOURCE_GLOBS),
        "build_input_globs": list(BUILD_INPUT_GLOBS),
        "dependency_lock_sha256": hashlib.sha256((root / "Cargo.lock").read_bytes()).hexdigest(),
        "build_inputs": [
            {"path": path.relative_to(root).as_posix(),
             "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
            for path in sorted({p for glob in BUILD_INPUT_GLOBS for p in root.glob(glob)
                                if p.is_file()})
        ],
        "source_files": records,
    }


def audit(root: Path, inventory: Path) -> list[str]:
    expected = json.loads(inventory.read_text(encoding="utf-8"))
    observed = build_inventory(root)
    # A new schema must fail an old receipt without hiding its existing source
    # drift. No manifest refresh or dependency-coverage claim is implicit here.
    errors = [] if set(expected) == set(observed) else ["action inventory fields changed"]
    errors.extend(f"action inventory policy changed: {key}" for key in observed
                  if key not in ("source_files", "build_inputs") and key in expected
                  and expected[key] != observed[key])
    before = {r["path"]: r for r in expected["source_files"]}
    after = {r["path"]: r for r in observed["source_files"]}
    if len(before) != len(expected["source_files"]):
        errors.append("duplicate action inventory source")
    if before.keys() != after.keys():
        errors.append("action allocation source set changed")
    errors.extend(f"action allocation source changed: {path}" for path in sorted(before.keys() & after.keys())
                  if before[path] != after[path])
    build_before = {r["path"]: r for r in expected.get("build_inputs", [])}
    build_after = {r["path"]: r for r in observed["build_inputs"]}
    if len(build_before) != len(expected.get("build_inputs", [])):
        errors.append("duplicate action build input")
    if build_before.keys() != build_after.keys():
        errors.append("action build input set changed")
    errors.extend(f"action build input changed: {path}" for path in sorted(build_before.keys() & build_after.keys())
                  if build_before[path] != build_after[path])
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("render", "audit"))
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--inventory", type=Path, default=DEFAULT_INVENTORY)
    args = parser.parse_args()
    if args.command == "render":
        print(json.dumps(build_inventory(args.root), indent=2))
        return 0
    errors = audit(args.root, args.inventory)
    for error in errors:
        print(error, file=sys.stderr)
    print("action allocation inventory: " + ("FAIL" if errors else "PASS"))
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
