#!/usr/bin/env python3
"""Run and publish a source-bound full Stata/Rust development baseline."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import advance_stfeols as advance

ROOT = advance.ROOT
FULL_RESULTS = ROOT / ".ci/stata/full-results"
EXPECTED_FIXTURES = (
    "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_parity.do",
    "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_keepsingletons_parity.do",
    "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_cascade_parity.do",
    "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_fweight_parity.do",
    "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_remaining_iid_parity.do",
    "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_cluster_parity.do",
)


def run(command: tuple[str, ...], env: dict[str, str], *, check: bool = True) -> int:
    print("+", " ".join(command), flush=True)
    result = subprocess.run(command, cwd=ROOT, env=env, check=False)
    if check and result.returncode != 0:
        raise SystemExit(f"command failed with rc={result.returncode}: {' '.join(command)}")
    return result.returncode


def full_environment(sha: str) -> dict[str, str]:
    env = os.environ.copy()
    env.update(
        {
            "GITHUB_SHA": sha,
            "GITHUB_REF": "refs/heads/main",
            "GITHUB_REF_NAME": "main",
            "STATA_PROFILE": "full",
            "STATA_BIN": advance.STATA_BIN,
            "CARGO_TERM_COLOR": "always",
            "RUST_BACKTRACE": "1",
        }
    )
    return env


def main() -> None:
    os.chdir(ROOT)
    advance.configure_git()
    advance.sync_main()
    source_sha, quick = advance.require_current_green()
    for relative in EXPECTED_FIXTURES:
        if not (ROOT / relative).is_file():
            raise SystemExit(f"interaction differential fixture is missing: {relative}")
    if int(quick.get("tests_failed", -1)) != 0:
        raise SystemExit("quick source receipt is not green")

    shutil.rmtree(ROOT / ".ci/stata/run", ignore_errors=True)
    env = full_environment(source_sha)
    stata_rc = run(("./ci/run_stata_ci.sh", "full"), env, check=False)
    rust_rc = run(("./ci/record_rust_quick.sh",), env, check=False)
    run(
        (
            "/usr/bin/python3",
            "ci/augment_ci_receipt.py",
            ".ci/stata/run/receipt.json",
            ".ci/stata/run/rust-quick.status",
        ),
        env,
    )
    run(
        (
            "/usr/bin/python3",
            "ci/check_stata_receipt.py",
            ".ci/stata/run/receipt.json",
            "--expect-tested-sha",
            source_sha,
            "--expect-profile",
            "full",
        ),
        env,
    )
    document = json.loads(
        (ROOT / ".ci/stata/run/receipt.json").read_text(encoding="utf-8")
    )
    if document.get("tested_sha") != source_sha:
        raise SystemExit("full receipt tested a different source")
    FULL_RESULTS.mkdir(parents=True, exist_ok=True)
    destination = FULL_RESULTS / f"{source_sha}.json"
    shutil.copy2(ROOT / ".ci/stata/run/receipt.json", destination)
    receipt_commit = advance.commit_source(
        f"ci(stata-full): record {source_sha[:12]} [skip ci]", (destination,)
    )
    print(
        "FULL_BASELINE",
        f"source={source_sha}",
        f"receipt_commit={receipt_commit}",
        f"status={document.get('status')}",
        f"stata_rc={stata_rc}",
        f"rust_rc={rust_rc}",
        f"tests_passed={document.get('tests_passed')}",
        f"tests_failed={document.get('tests_failed')}",
        flush=True,
    )
    if not advance.receipt_is_green(document):
        raise SystemExit("full source-bound baseline is red")


if __name__ == "__main__":
    main()
