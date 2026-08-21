#!/usr/bin/env python3
"""Run licensed Stata fixtures and require their terminal success sentinels."""
from __future__ import annotations

import argparse
import os
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

SENTINEL_PREFIX = "END_FIXTURE|"


@dataclass(frozen=True)
class FixtureResult:
    path: Path
    process_returncode: int
    log_text: str
    error: str | None


def fixture_name(path: Path) -> str:
    """Return the stable sentinel name for a fixture path."""
    return path.stem


def expected_sentinel(path: Path) -> str:
    return f"{SENTINEL_PREFIX}{fixture_name(path)}|rc=0"


def validate_fixture_log(path: Path, log_text: str) -> str | None:
    """Require exactly one sentinel-prefixed line equal to the expected value."""
    expected = expected_sentinel(path)
    sentinels = [
        line.strip()
        for line in log_text.splitlines()
        if line.strip().startswith(SENTINEL_PREFIX)
    ]
    if not sentinels:
        return f"missing {expected!r}"
    if len(sentinels) != 1:
        return f"expected exactly one {expected!r}; observed {sentinels!r}"
    if sentinels[0] != expected:
        return f"expected {expected!r}; observed {sentinels[0]!r}"
    return None


def stata_string(value: Path) -> str:
    text = str(value.resolve())
    if '"' in text:
        raise ValueError(f"Stata path contains an unsupported quote: {text}")
    return f'"{text}"'


def run_fixture(stata: str, path: Path, root: Path, runner=subprocess.run) -> FixtureResult:
    path = path.resolve()
    with tempfile.TemporaryDirectory(prefix="stfixest-stata-") as temp_name:
        run_dir = Path(temp_name)
        wrapper = run_dir / "run.do"
        wrapper.write_text(
            "\n".join(
                [
                    "version 18.0",
                    "clear all",
                    "set more off",
                    f"adopath ++ {stata_string(root / 'stata')}",
                    f"adopath ++ {stata_string(root / 'dist')}",
                    f"do {stata_string(path)}",
                    "exit 0",
                    "",
                ]
            ),
            encoding="utf-8",
        )
        completed = runner(
            [stata, "-b", "do", str(wrapper)],
            cwd=run_dir,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        log_path = run_dir / "run.log"
        log_text = (
            log_path.read_text(encoding="utf-8", errors="replace")
            if log_path.exists()
            else ""
        )
        error = None
        if completed.returncode:
            error = f"Stata process returned {completed.returncode}"
        elif not log_path.exists():
            error = "Stata produced no run.log"
        else:
            error = validate_fixture_log(path, log_text)
        return FixtureResult(
            path=path,
            process_returncode=completed.returncode,
            log_text=log_text,
            error=error,
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args()
    if not args.stata:
        raise SystemExit("set STATA_BIN or pass --stata")
    root = Path(__file__).resolve().parents[1]
    paths = args.paths or sorted((root / "tests/stata").rglob("*.do"))
    failures: list[FixtureResult] = []
    for path in paths:
        print(f"==> {path}")
        result = run_fixture(args.stata, path, root)
        if result.error:
            failures.append(result)
            print(f"FAIL: {result.error}")
        else:
            print(f"PASS: {expected_sentinel(path)}")
    if failures:
        summary = "; ".join(f"{item.path}: {item.error}" for item in failures)
        raise SystemExit("Stata tests failed: " + summary)


if __name__ == "__main__":
    main()
