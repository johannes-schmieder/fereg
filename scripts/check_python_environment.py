#!/usr/bin/env python3
"""Require the interpreter that scripts/run_tests.sh runs every Python check with.

The scientific Python modules (independent oracles, accuracy and bias audits)
need NumPy, SciPy and pandas. Without them some modules would fail to import
and others would skip, so the suite would not establish their checks. This
preflight stops the entry point first and names the interpreter instead.
"""
from __future__ import annotations

import importlib
import platform
import sys

MINIMUM = (3, 11)
REQUIRED = ("numpy", "scipy", "pandas")


def main() -> int:
    problems = []
    if sys.version_info[:2] < MINIMUM:
        problems.append(
            f"Python {MINIMUM[0]}.{MINIMUM[1]} or newer is required; "
            f"this is Python {platform.python_version()}"
        )
    versions = []
    for name in REQUIRED:
        try:
            module = importlib.import_module(name)
        except ImportError as error:
            problems.append(f"{name} is not importable ({error})")
        else:
            versions.append(f"{name} {getattr(module, '__version__', 'unknown')}")
    if problems:
        print(f"PYTHON ENVIRONMENT: FAIL for {sys.executable}", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        print(
            "  No check ran: the scientific Python modules would not run, and they are "
            "never counted as passed. Set FEREG_PYTHON_BIN to an interpreter with "
            "Python 3.11+, NumPy, SciPy and pandas.",
            file=sys.stderr,
        )
        return 3
    print(
        f"Python environment: PASS ({sys.executable}, Python "
        f"{platform.python_version()}, {', '.join(versions)})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
