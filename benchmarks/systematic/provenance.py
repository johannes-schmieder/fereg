"""Freeze and verify the actual measurement and estimator runtime bytes."""
import argparse
import hashlib
import json
from pathlib import Path

from common import HERE, read_json, sha256, write_json

MEASUREMENT_FILES = ("manifest.json", "references.json", "common.py", "generate.py",
                     "runner.py", "oracle.py", "stata_worker.do", "r_worker.R",
                     "python_worker.py", "job.sh", "provenance.py")


def fingerprint(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def freeze(root):
    target = root / "env/frozen.json"
    if target.exists():
        raise ValueError("runtime already frozen; preserve the existing evidence")
    candidate = read_json(root / "input/candidate.json")
    manifest = read_json(HERE / "manifest.json")
    if candidate["commit"] != manifest["candidate_commit"] or sha256(root / "input/candidate.tar.gz") != candidate["sha256"]:
        raise ValueError("candidate archive identity differs")
    paths = [root / "candidate/dist/fereg_linux.plugin", root / "env/python-freeze.txt",
             root / "env/R-packages.csv", root / "input/candidate.tar.gz"]
    paths += sorted((root / "references").glob("*"))
    paths += sorted((root / "candidate/stata").glob("*.ado"))
    # Bind comparator implementations and compiled extensions, in addition to
    # the package-version receipts and exact source archives.
    for directory in [root / "env/R/fixest", *root.glob("env/python/lib/python*/site-packages/pyfixest")]:
        paths += [p for p in directory.rglob("*") if p.is_file() and "__pycache__" not in p.parts]
    value = {"candidate": candidate, "controller": {f: sha256(HERE / f) for f in MEASUREMENT_FILES},
             "runtime": {str(p.relative_to(root)): sha256(p) for p in paths if p.is_file()}}
    value["fingerprint"] = fingerprint(value)
    write_json(target, value)
    return value


def verify(root):
    value = read_json(root / "env/frozen.json")
    if fingerprint({k: v for k, v in value.items() if k != "fingerprint"}) != value["fingerprint"]:
        raise ValueError("frozen inventory is internally inconsistent")
    for name, expected in value["controller"].items():
        if sha256(HERE / name) != expected:
            raise ValueError(f"measurement controller changed: {name}")
    for name, expected in value["runtime"].items():
        if sha256(root / name) != expected:
            raise ValueError(f"estimator runtime changed: {name}")
    return value["fingerprint"]


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("freeze", "verify"))
    parser.add_argument("run_root", type=Path)
    args = parser.parse_args()
    result = freeze(args.run_root)["fingerprint"] if args.mode == "freeze" else verify(args.run_root)
    print(result)
