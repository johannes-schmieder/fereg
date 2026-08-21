#!/usr/bin/env python3
"""Run licensed Stata do-files when a local executable is supplied."""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import tempfile
from pathlib import Path
from typing import Optional

SENTINEL = re.compile(r"^END_FIXTURE\|(?P<name>[^|]+)\|rc=(?P<rc>-?[0-9]+)\s*$", re.MULTILINE)


def validate_fixture_log(log_text: str, expected_name: str) -> Optional[str]:
    """Return a failure reason unless exactly one successful fixture sentinel exists."""
    matches = list(SENTINEL.finditer(log_text))
    if not matches:
        return "missing END_FIXTURE sentinel"
    if len(matches) != 1:
        return f"expected one END_FIXTURE sentinel, found {len(matches)}"
    name = matches[0].group("name")
    if name != expected_name:
        return f"sentinel names {name!r}, expected {expected_name!r}"
    fixture_rc = int(matches[0].group("rc"))
    if fixture_rc != 0:
        return f"fixture sentinel reported rc={fixture_rc}"
    return None


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args()
    if not args.stata:
        raise SystemExit("set STATA_BIN or pass --stata")
    root = Path(__file__).resolve().parents[1]
    paths = args.paths or sorted((root / "tests/stata").rglob("*.do"))
    failures: list[tuple[Path, str]] = []
    for path in paths:
        print(f"==> {path}")
        with tempfile.TemporaryDirectory(prefix="stfixest-stata-") as temporary:
            run_dir = Path(temporary)
            wrapper = run_dir / f"{path.stem}.do"
            fixture = path.resolve().as_posix().replace('"', '""')
            stata_ado = (root / "stata").as_posix().replace('"', '""')
            dist_ado = (root / "dist").as_posix().replace('"', '""')
            wrapper.write_text(
                "version 18.0\n"
                f"adopath ++ \"{stata_ado}\"\n"
                f"adopath ++ \"{dist_ado}\"\n"
                f"do \"{fixture}\"\n",
                encoding="utf-8",
            )
            completed = subprocess.run(
                [args.stata, "-b", "do", str(wrapper)], cwd=run_dir, check=False
            )
            if completed.returncode:
                failures.append((path, f"Stata process returned {completed.returncode}"))
            log_path = run_dir / f"{path.stem}.log"
            if not log_path.exists():
                failures.append((path, "Stata did not produce the expected batch log"))
                continue
            log_text = log_path.read_text(encoding="utf-8", errors="replace")
            sentinel_failure = validate_fixture_log(log_text, path.stem)
            if sentinel_failure:
                failures.append((path, sentinel_failure))
    if failures:
        rendered = "; ".join(f"{path}: {reason}" for path, reason in failures)
        raise SystemExit("Stata tests failed: " + rendered)


if __name__ == "__main__":
    main()
