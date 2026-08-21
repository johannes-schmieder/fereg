#!/usr/bin/env python3
"""Manifest-driven benchmark launcher; licensed Stata is required to capture."""
from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path

try:
    from scripts.ols_reference import DEFAULT_MANIFEST, capture_reference, verify_receipt
except ModuleNotFoundError:  # Direct ``python scripts/run_benchmarks.py`` execution.
    from ols_reference import DEFAULT_MANIFEST, capture_reference, verify_receipt


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    parser.add_argument("--suite", choices=["ols", "ppml", "all"], default="all")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--reference-root", type=Path)
    parser.add_argument("--receipt", type=Path)
    parser.add_argument("--verify-receipt", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    if args.verify_receipt:
        if args.suite != "ols" or args.receipt is None:
            raise SystemExit("--verify-receipt requires --suite ols and --receipt")
        verify_receipt(args.receipt, args.manifest)
        print(f"OLS receipt PASS: {args.receipt}")
        return
    if not args.stata:
        raise SystemExit("set STATA_BIN or pass --stata")
    if args.suite in {"ols", "all"}:
        if args.reference_root is None or args.receipt is None:
            raise SystemExit("OLS capture requires --reference-root and --receipt")
        digest = capture_reference(
            args.stata,
            args.reference_root,
            args.receipt,
            manifest_path=args.manifest,
        )
        print(f"sealed OLS reference receipt: {args.receipt} sha256={digest}")
    if args.suite in {"ppml", "all"}:
        # PPML remains a Phase 6+ baseline and is intentionally not accepted as
        # OLS-v1 evidence by the sealed receipt verifier.
        subprocess.run(
            [args.stata, "-b", "do", str(root / "benchmarks/stata/baseline_ppmlhdfe.do")],
            cwd=root,
            check=True,
        )


if __name__ == "__main__":
    main()
