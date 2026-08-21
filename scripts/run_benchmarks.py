#!/usr/bin/env python3
"""Manifest-driven benchmark launcher; requires licensed Stata."""
from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    parser.add_argument("--suite", choices=["ols", "ppml", "all"], default="all")
    args = parser.parse_args()
    if not args.stata:
        raise SystemExit("set STATA_BIN or pass --stata")
    root = Path(__file__).resolve().parents[1]
    names = ["baseline_reghdfe.do", "baseline_ppmlhdfe.do"] if args.suite == "all" else [f"baseline_{'reghdfe' if args.suite == 'ols' else 'ppmlhdfe'}.do"]
    for name in names:
        subprocess.run([args.stata, "-b", "do", str(root / "benchmarks/stata" / name)], cwd=root, check=True)


if __name__ == "__main__":
    main()
