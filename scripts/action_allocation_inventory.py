#!/usr/bin/env python3
"""Closed structural inventory of both native actions; not a byte-coverage proof.

The OLS/F0 route guard remains independent. This companion also watches PPML,
separation and session bodies that the OLS-only guard deliberately excludes.
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
DEFAULT_INVENTORY = ROOT / "benchmarks/manifests/action_allocation_inventory_v19.json"
SOURCE_GLOBS = ("crates/fereg-core/src/**/*.rs", "crates/fereg-plugin/src/**/*.rs")


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
        "schema_version": 1,
        "coverage": "structural_only_not_capacity_or_reachability",
        "source_globs": list(SOURCE_GLOBS),
        "dependency_lock_sha256": hashlib.sha256((root / "Cargo.lock").read_bytes()).hexdigest(),
        "source_files": records,
    }


def audit(root: Path, inventory: Path) -> list[str]:
    expected = json.loads(inventory.read_text(encoding="utf-8"))
    observed = build_inventory(root)
    if set(expected) != set(observed):
        return ["action inventory fields changed"]
    errors = [f"action inventory policy changed: {key}" for key in observed
              if key != "source_files" and expected[key] != observed[key]]
    before = {r["path"]: r for r in expected["source_files"]}
    after = {r["path"]: r for r in observed["source_files"]}
    if len(before) != len(expected["source_files"]):
        errors.append("duplicate action inventory source")
    if before.keys() != after.keys():
        errors.append("action allocation source set changed")
    errors.extend(f"action allocation source changed: {path}" for path in sorted(before.keys() & after.keys())
                  if before[path] != after[path])
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
