#!/usr/bin/env python3
"""Build an isolated fereg candidate and run selected licensed fixtures."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def write_json_atomic(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    temporary.replace(path)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def rust_environment(tested_sha: str) -> dict[str, str]:
    environment = os.environ.copy()
    rustup = environment.get("RUSTUP_BIN", "/opt/homebrew/bin/rustup")
    toolchain = environment.get("RUST_TOOLCHAIN", "1.85.1")
    cargo = subprocess.check_output(
        [rustup, "which", "--toolchain", toolchain, "cargo"], text=True
    ).strip()
    environment["PATH"] = f"{Path(cargo).parent}:/opt/homebrew/bin:{environment.get('PATH', '')}"
    environment["CARGO_TERM_COLOR"] = "always"
    environment["RUST_BACKTRACE"] = "1"
    environment["FEREG_GIT_COMMIT"] = tested_sha[:12]
    return environment


def initial_summary(profile: str) -> dict[str, object]:
    return {
        "schema_version": 1,
        "profile": profile,
        "completed": True,
        "status": "failure",
        "failure_kind": None,
        "failure_detail": None,
        "stata_rc": None,
        "stata_version": None,
        "stata_edition": None,
        "stata_os": None,
        "stata_machine_type": None,
        "stata_processors": None,
        "tests_passed": 0,
        "tests_failed": 1,
        "plugin_sha256": None,
        "build_rc": None,
        "results": [],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile-config", required=True, type=Path)
    parser.add_argument("--profile", required=True)
    parser.add_argument("--repo-root", required=True, type=Path)
    parser.add_argument("--run-root", required=True, type=Path)
    parser.add_argument("--summary-json", required=True, type=Path)
    parser.add_argument("--stata", required=True)
    parser.add_argument("--tested-sha", required=True)
    args = parser.parse_args()

    config = json.loads(args.profile_config.read_text(encoding="utf-8"))
    profile = config["profiles"][args.profile]
    summary = initial_summary(args.profile)
    environment = os.environ.copy()
    plugin: Path | None = None
    plugin_sha256: str | None = None

    if profile.get("build_plugin") is True:
        try:
            environment = rust_environment(args.tested_sha)
        except (OSError, subprocess.CalledProcessError) as exc:
            summary["failure_kind"] = "rust_toolchain_error"
            summary["failure_detail"] = str(exc)
            write_json_atomic(args.summary_json, summary)
            return 1
        print("==> building universal macOS plugin")
        build = subprocess.run(
            [str(args.repo_root / "scripts" / "build_macos.sh")],
            cwd=args.repo_root,
            env=environment,
            check=False,
        )
        summary["build_rc"] = build.returncode
        if build.returncode != 0:
            summary["failure_kind"] = "plugin_build_error"
            summary["failure_detail"] = (
                f"scripts/build_macos.sh returned {build.returncode}"
            )
            write_json_atomic(args.summary_json, summary)
            return 1
        plugin = args.repo_root / "dist" / "fereg_macos.plugin"
        if not plugin.is_file():
            summary["failure_kind"] = "missing_result"
            summary["failure_detail"] = "build produced no dist/fereg_macos.plugin"
            write_json_atomic(args.summary_json, summary)
            return 1
        plugin_sha256 = sha256_file(plugin)
        summary["plugin_sha256"] = plugin_sha256
        evidence = args.run_root / "plugin-evidence"
        evidence.mkdir(parents=True, exist_ok=True)
        shutil.copy2(plugin, evidence / plugin.name)
        (evidence / "fereg_macos.plugin.sha256").write_text(
            f"{plugin_sha256}  fereg_macos.plugin\n", encoding="utf-8"
        )

    fixture_summary = args.run_root / "fixture-summary.json"
    fixture_logs = args.run_root / "fixture-logs"
    command = [
        sys.executable,
        str(args.repo_root / "scripts" / "run_stata_tests.py"),
        "--stata",
        args.stata,
        "--log-dir",
        str(fixture_logs),
        "--summary-json",
        str(fixture_summary),
    ]
    if plugin is not None and plugin_sha256 is not None:
        command.extend(
            ["--plugin", str(plugin), "--plugin-sha256", plugin_sha256]
        )
    command.extend(str(args.repo_root / path) for path in profile["fixtures"])
    fixture_process = subprocess.run(
        command, cwd=args.repo_root, env=environment, check=False
    )
    if not fixture_summary.is_file():
        summary["failure_kind"] = "missing_status"
        summary["failure_detail"] = "fixture driver wrote no summary"
        write_json_atomic(args.summary_json, summary)
        return 1

    fixture_payload = json.loads(fixture_summary.read_text(encoding="utf-8"))
    for key in (
        "status",
        "failure_kind",
        "failure_detail",
        "stata_rc",
        "stata_version",
        "stata_edition",
        "stata_os",
        "stata_machine_type",
        "stata_processors",
        "tests_passed",
        "tests_failed",
        "results",
    ):
        summary[key] = fixture_payload.get(key)

    if fixture_process.returncode != 0 or summary["status"] != "success":
        write_json_atomic(args.summary_json, summary)
        return 1

    if profile.get("run_break_fixture") is True:
        break_log = args.run_root / "fixture-logs" / "estimate_ols_eclass_break.log"
        print("==> tests/licensed_stata/estimate_ols_eclass_break.do")
        with break_log.open("w", encoding="utf-8") as output:
            break_process = subprocess.run(
                [
                    sys.executable,
                    str(args.repo_root / "scripts" / "run_stata_break_test.py"),
                    "--stata",
                    args.stata,
                    "--plugin",
                    str(plugin),
                    "--plugin-sha256",
                    str(plugin_sha256),
                ],
                cwd=args.repo_root,
                env=environment,
                stdout=output,
                stderr=subprocess.STDOUT,
                check=False,
            )
        if break_process.returncode != 0:
            summary["status"] = "failure"
            summary["failure_kind"] = "stata_break_error"
            summary["failure_detail"] = (
                f"interactive Break fixture returned {break_process.returncode}"
            )
            summary["tests_failed"] = int(summary["tests_failed"] or 0) + 1
            write_json_atomic(args.summary_json, summary)
            return 1
        summary["tests_passed"] = int(summary["tests_passed"] or 0) + 1
        results = list(summary["results"] or [])
        results.append(
            {
                "fixture": "tests/licensed_stata/estimate_ols_eclass_break.do",
                "status": "success",
                "failure_kind": None,
                "failure_detail": None,
                "process_rc": 0,
                "stata_rc": 0,
            }
        )
        summary["results"] = results

    summary["status"] = "success"
    summary["failure_kind"] = None
    summary["failure_detail"] = None
    write_json_atomic(args.summary_json, summary)
    return 0


if __name__ == "__main__":
    sys.exit(main())
