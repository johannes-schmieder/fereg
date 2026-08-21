#!/usr/bin/env python3
"""Run licensed Stata do-files when a local executable is supplied."""
from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args()
    if not args.stata:
        raise SystemExit("set STATA_BIN or pass --stata")
    root = Path(__file__).resolve().parents[1]
    paths = args.paths or sorted((root / "tests/stata").rglob("*.do"))
    failures: list[Path] = []
    for path in paths:
        print(f"==> {path}")
        completed = subprocess.run([args.stata, "-b", "do", str(path.resolve())], cwd=root, check=False)
        if completed.returncode:
            failures.append(path)
    if failures:
        raise SystemExit("Stata tests failed: " + ", ".join(map(str, failures)))


if __name__ == "__main__":
    main()
