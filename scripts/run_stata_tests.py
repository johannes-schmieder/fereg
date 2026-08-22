#!/usr/bin/env python3
"""Run licensed Stata fixtures and require their terminal success sentinels."""
from __future__ import annotations

import argparse
import hashlib
import os
import platform
import re
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

SENTINEL_PREFIX = "END_FIXTURE|"
FORBIDDEN_SUCCESS_DIAGNOSTICS = (
    "warning: variance matrix is nonsymmetric or highly singular.",
)
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")


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
    lowered = log_text.lower()
    for diagnostic in FORBIDDEN_SUCCESS_DIAGNOSTICS:
        if diagnostic in lowered:
            return f"forbidden Stata success diagnostic {diagnostic!r}"
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


def expected_plugin_filename() -> str:
    system = platform.system()
    if system == "Darwin":
        return "stfixest_macos.plugin"
    if system == "Linux":
        return "stfixest_linux.plugin"
    if system == "Windows":
        return "stfixest_windows.plugin"
    raise ValueError(f"unsupported Stata plugin platform {system!r}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def validate_candidate_plugin(plugin: Path, expected_sha256: str) -> Path:
    candidate = plugin.resolve()
    if not HEX_SHA256.fullmatch(expected_sha256):
        raise ValueError("candidate plugin SHA-256 is malformed")
    if not candidate.is_file() or candidate.name != expected_plugin_filename():
        raise ValueError(
            f"candidate plugin must be an existing {expected_plugin_filename()} file"
        )
    if sha256_file(candidate) != expected_sha256:
        raise ValueError("candidate plugin SHA-256 does not match the supplied binding")
    return candidate


def run_fixture(
    stata: str,
    path: Path,
    root: Path,
    runner=subprocess.run,
    *,
    plugin: Path | None = None,
    plugin_sha256: str | None = None,
) -> FixtureResult:
    path = path.resolve()
    bound_plugin = None
    if plugin is not None or plugin_sha256 is not None:
        if plugin is None or plugin_sha256 is None:
            raise ValueError("candidate plugin path and SHA-256 must be supplied together")
        bound_plugin = validate_candidate_plugin(plugin, plugin_sha256)
    with tempfile.TemporaryDirectory(prefix="stfixest-stata-") as temp_name:
        run_dir = Path(temp_name)
        wrapper = run_dir / "run.do"
        lines = ["version 18.0", "clear all", "set more off"]
        if bound_plugin is not None:
            isolated = run_dir / "sysdirs"
            for name in ("PERSONAL", "PLUS", "OLDPLACE", "SITE"):
                target = isolated / name.lower()
                target.mkdir(parents=True)
                lines.append(f"sysdir set {name} {stata_string(target)}")
            lines.extend(
                [
                    f"adopath ++ {stata_string(root / 'stata')}",
                    f"adopath ++ {stata_string(bound_plugin.parent)}",
                    "capture program __stfixest_runner_provenance, plugin "
                    f"using({stata_string(bound_plugin)})",
                    "local provenance_load_rc = _rc",
                    "if `provenance_load_rc' != 0 & `provenance_load_rc' != 110 "
                    "exit `provenance_load_rc'",
                    "plugin call __stfixest_runner_provenance, version",
                ]
            )
        else:
            lines.extend(
                [
                    f"adopath ++ {stata_string(root / 'stata')}",
                    f"adopath ++ {stata_string(root / 'dist')}",
                ]
            )
        lines.extend([f"do {stata_string(path)}", "exit 0", ""])
        wrapper.write_text(
            "\n".join(lines),
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
        if bound_plugin is not None and sha256_file(bound_plugin) != plugin_sha256:
            error = "candidate plugin bytes changed during the Stata fixture"
        return FixtureResult(
            path=path,
            process_returncode=completed.returncode,
            log_text=log_text,
            error=error,
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    parser.add_argument("--plugin", type=Path)
    parser.add_argument("--plugin-sha256")
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args()
    if not args.stata:
        raise SystemExit("set STATA_BIN or pass --stata")
    root = Path(__file__).resolve().parents[1]
    paths = args.paths or sorted((root / "tests/stata").rglob("*.do"))
    failures: list[FixtureResult] = []
    for path in paths:
        print(f"==> {path}")
        result = run_fixture(
            args.stata,
            path,
            root,
            plugin=args.plugin,
            plugin_sha256=args.plugin_sha256,
        )
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
