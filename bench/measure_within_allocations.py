#!/usr/bin/env python3
"""Run the independent heap-payload/science witness against an exact within checkout.

Not a timing harness or RSS bound. The process allocator includes Rayon bookkeeping.
Use a separate output directory for each source revision under target/tmp/<task>/.
Example (with pinned Rust binaries on PATH):
  python3 bench/measure_within_allocations.py --within ../within \\
    --output target/tmp/migration/allocations --workers 1 4 --reproducible
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib


def run(args: list[str], **kwargs: object) -> str:
    return subprocess.check_output(args, text=True, **kwargs).strip()


def registry_versions(path: Path) -> set[tuple[str, str, str]]:
    packages = tomllib.loads(path.read_text())["package"]
    return {(p["name"], p["version"], p["source"]) for p in packages if "source" in p}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--within", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo-target", type=Path, help="optional existing isolated Cargo target")
    parser.add_argument("--workers", type=int, nargs="+", default=[1, 4])
    parser.add_argument("--reproducible", action="store_true", help="within #478 API and exact repeats")
    args = parser.parse_args()
    if any(n < 1 for n in args.workers):
        parser.error("workers must be positive")
    source = args.within.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    witness = Path(__file__).with_name("within_allocation.rs").resolve(strict=True)
    manifest = output / "Cargo.toml"
    manifest.write_text("\n".join([
        '[package]', 'name = "within-allocation-witness"', 'version = "0.0.0"',
        'edition = "2021"', '[workspace]', '[features]', 'repro-stack = []',
        '[dependencies]',
        'within = { path = ' + json.dumps(str(source / 'crates/within')) + ' }',
        'schwarz-precond = { path = ' + json.dumps(str(source / 'crates/schwarz-precond')) + ' }',
        'rayon = "1.11"', '[[bin]]', 'name = "within-allocation-witness"',
        'path = ' + json.dumps(str(witness)), '',
    ]))
    shutil.copyfile(source / "Cargo.lock", output / "Cargo.lock")
    cargo_target = args.cargo_target.resolve() if args.cargo_target else output / "cargo"
    env = dict(os.environ, CARGO_TARGET_DIR=str(cargo_target), CARGO_INCREMENTAL="0",
               CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_DEV_OPT_LEVEL="1")
    build = ["cargo", "build", "--offline", "--manifest-path", str(manifest)]
    if args.reproducible:
        build += ["--features", "repro-stack"]
    with (output / "build.log").open("w") as log:
        subprocess.run(build, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=300)
    extra = registry_versions(output / "Cargo.lock") - registry_versions(source / "Cargo.lock")
    if extra:
        raise RuntimeError(f"witness selected unpinned dependency versions: {sorted(extra)}")
    binary = cargo_target / "debug/within-allocation-witness"
    identity = {
        "source_commit": run(["git", "-C", str(source), "rev-parse", "HEAD"]),
        "source_diff_sha256": hashlib.sha256(run(["git", "-C", str(source), "diff", "HEAD"]).encode()).hexdigest(),
        "witness_sha256": hashlib.sha256(witness.read_bytes()).hexdigest(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "rustc": run(["rustc", "--version"]), "cargo": run(["cargo", "--version"]),
        "workers": args.workers, "reproducible": args.reproducible,
        "scope": "process heap payload and independent numerical witnesses; no timing or RSS",
    }
    (output / "identity.json").write_text(json.dumps(identity, indent=2) + "\n")
    for workers in args.workers:
        with (output / f"workers-{workers}.jsonl").open("w") as log:
            subprocess.run([str(binary), str(workers)], stdout=log, stderr=subprocess.STDOUT,
                           check=True, timeout=240)
    print(json.dumps(identity, indent=2))


if __name__ == "__main__":
    main()
