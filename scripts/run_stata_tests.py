#!/usr/bin/env python3
"""Run licensed Stata fixtures and require their terminal success sentinels."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
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
    process_returncode: int | None
    stata_rc: int | None
    log_text: str
    error: str | None
    failure_kind: str | None
    metadata: dict[str, str]
    process_output: str


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
        return "fereg_macos.plugin"
    if system == "Linux":
        return "fereg_linux.plugin"
    if system == "Windows":
        return "fereg_windows.plugin"
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


def read_status(path: Path) -> tuple[dict[str, str], str | None]:
    if not path.is_file():
        return {}, "fixture status file was not written"
    values: dict[str, str] = {}
    try:
        for raw_line in path.read_text(encoding="utf-8").splitlines():
            if not raw_line.strip():
                continue
            key, separator, value = raw_line.partition("=")
            if not separator or not key:
                return {}, f"malformed fixture status line: {raw_line!r}"
            values[key] = value
    except OSError as exc:
        return {}, f"cannot read fixture status: {exc}"
    return values, None


def optional_int(value: str | None) -> int | None:
    try:
        return int(value) if value is not None else None
    except ValueError:
        return None


def write_json_atomic(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    temporary.replace(path)


def render_fixture_wrapper(
    *, path: Path, root: Path, run_dir: Path,
    bound_plugin: Path | None, bound_ado_root: Path | None,
) -> bytes:
    status_path = run_dir / "fixture.status"
    lines = ["version 15.0", "clear all", "set more off"]
    if bound_plugin is not None:
        isolated = run_dir / "sysdirs"
        for name in ("PERSONAL", "PLUS", "OLDPLACE", "SITE"):
            lines.append(f"sysdir set {name} {stata_string(isolated / name.lower())}")
        lines.extend(
            [
                f"adopath ++ {stata_string(bound_ado_root or root / 'stata')}",
                f"adopath ++ {stata_string(path.parent)}",
                *([] if bound_ado_root is not None else [f"adopath ++ {stata_string(bound_plugin.parent)}"]),
                "capture program __fereg_runner_provenance, plugin "
                f"using({stata_string(bound_plugin)})",
                "local provenance_load_rc = _rc",
                "if `provenance_load_rc' != 0 & `provenance_load_rc' != 110 exit `provenance_load_rc'",
                'plugin call __fereg_runner_provenance, selftest "ado=0.5.0" '
                '"protocol_major=1" "protocol_minor=6" '
                '"stata=`c(stata_version)\'" "threads=1" "verbose=1"',
            ]
        )
        if bound_ado_root is not None:
            lines.extend(
                [
                    "global FEREG_STAGED_PACKAGE_ROOT " f"{stata_string(bound_ado_root)}",
                    "global FEREG_STAGED_PLUGIN_NAME " f'"{bound_plugin.name}"',
                ]
            )
    else:
        lines.extend(
            [
                f"adopath ++ {stata_string(root / 'stata')}",
                f"adopath ++ {stata_string(path.parent)}",
                f"adopath ++ {stata_string(root / 'dist')}",
            ]
        )
    lines.extend(
        [
            f"capture noisily do {stata_string(path)}",
            "local fixture_rc = _rc",
            "local fixture_stata_version = c(stata_version)",
            "local fixture_stata_edition `\"`c(edition_real)'\"'",
            "local fixture_stata_os `\"`c(os)'\"'",
            "local fixture_stata_machine_type `\"`c(machine_type)'\"'",
            "local fixture_stata_processors = c(processors)",
            "tempname fixture_status_handle",
            f"capture file open `fixture_status_handle' using {stata_string(status_path)}, write text replace",
            "local fixture_status_open_rc = _rc",
            "if `fixture_status_open_rc' == 0 {",
            "    file write `fixture_status_handle' \"schema_version=1\" _n",
            "    file write `fixture_status_handle' \"completed=1\" _n",
            "    file write `fixture_status_handle' \"stata_rc=`fixture_rc'\" _n",
            "    file write `fixture_status_handle' \"stata_version=`fixture_stata_version'\" _n",
            "    file write `fixture_status_handle' \"stata_edition=`fixture_stata_edition'\" _n",
            "    file write `fixture_status_handle' \"stata_os=`fixture_stata_os'\" _n",
            "    file write `fixture_status_handle' \"stata_machine_type=`fixture_stata_machine_type'\" _n",
            "    file write `fixture_status_handle' \"stata_processors=`fixture_stata_processors'\" _n",
            "    file close `fixture_status_handle'",
            "}",
            "exit `fixture_rc'",
            "",
        ]
    )
    return "\n".join(lines).encode("utf-8")


def run_fixture(
    stata: str,
    path: Path,
    root: Path,
    runner=subprocess.run,
    *,
    plugin: Path | None = None,
    plugin_sha256: str | None = None,
    log_dir: Path | None = None,
    require_status: bool = False,
    ado_root: Path | None = None,
    environment: dict[str, str] | None = None,
) -> FixtureResult:
    path = path.resolve()
    bound_plugin = None
    if plugin is not None or plugin_sha256 is not None:
        if plugin is None or plugin_sha256 is None:
            raise ValueError("candidate plugin path and SHA-256 must be supplied together")
        bound_plugin = validate_candidate_plugin(plugin, plugin_sha256)
    bound_ado_root = None
    if ado_root is not None:
        bound_ado_root = ado_root.resolve()
        if not bound_ado_root.is_dir():
            raise ValueError("staged package ado root is not a directory")
        for required in ("fereg.pkg", "stata.toc"):
            if not (bound_ado_root / required).is_file():
                raise ValueError(f"staged package ado root omits {required}")
        if bound_plugin is None:
            raise ValueError("staged package execution requires an exact plugin binding")
        if bound_plugin.parent != bound_ado_root:
            raise ValueError("staged package plugin is outside the extracted ado root")
    with tempfile.TemporaryDirectory(prefix="fereg-stata-") as temp_name:
        # Freeze the lexical path used in wrapper bytes, argv, cwd, and retained
        # invocation.  On macOS /var is a symlink to /private/var; mixing the
        # unresolved cwd with Path.resolve() inside the renderer is not portable.
        run_dir = Path(temp_name).resolve()
        wrapper = run_dir / "run.do"
        status_path = run_dir / "fixture.status"
        if bound_plugin is not None:
            isolated = run_dir / "sysdirs"
            for name in ("PERSONAL", "PLUS", "OLDPLACE", "SITE"):
                target = isolated / name.lower()
                target.mkdir(parents=True)
        wrapper.write_bytes(render_fixture_wrapper(
            path=path, root=root, run_dir=run_dir,
            bound_plugin=bound_plugin, bound_ado_root=bound_ado_root,
        ))
        launch_error = None
        process_output = ""
        try:
            completed = runner(
                [stata, "-q", "-b", "do", str(wrapper)],
                cwd=run_dir,
                env=(dict(environment) if environment is not None else None),
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
            )
            process_returncode = completed.returncode
            process_output = completed.stdout
        except OSError as exc:
            launch_error = f"{type(exc).__name__}: {exc}"
            process_returncode = None
        log_path = run_dir / "run.log"
        log_text = (
            log_path.read_text(encoding="utf-8", errors="replace")
            if log_path.exists()
            else ""
        )
        metadata, status_error = read_status(status_path)
        stata_rc = optional_int(metadata.get("stata_rc"))
        error = None
        failure_kind = None
        if launch_error is not None:
            failure_kind = "launch_error"
            error = f"Stata failed to launch: {launch_error}"
        elif process_returncode:
            failure_kind = "crash"
            error = f"Stata process returned {process_returncode}"
        elif not log_path.exists():
            failure_kind = "missing_log"
            error = "Stata produced no run.log"
        elif status_error is None and metadata.get("completed") != "1":
            failure_kind = "missing_status"
            error = "fixture status did not record completion"
        elif status_error is None and stata_rc is None:
            failure_kind = "missing_status"
            error = "fixture status did not contain a numeric stata_rc"
        elif stata_rc not in (None, 0):
            failure_kind = "stata_error"
            error = f"Stata fixture returned r({stata_rc})"
        elif require_status and status_error is not None:
            failure_kind = "missing_status"
            error = status_error
        else:
            error = validate_fixture_log(path, log_text)
            if error is not None:
                failure_kind = "missing_success_sentinel"
        if bound_plugin is not None and sha256_file(bound_plugin) != plugin_sha256:
            failure_kind = "candidate_changed"
            error = "candidate plugin bytes changed during the Stata fixture"
        if log_dir is not None:
            log_dir.mkdir(parents=True, exist_ok=True)
            stable_name = path.relative_to(root).as_posix().replace("/", "__")
            (log_dir / f"{stable_name}.log").write_text(log_text, encoding="utf-8")
            shutil.copy2(wrapper, log_dir / f"{stable_name}.wrapper.do")
            (log_dir / f"{stable_name}.invocation.json").write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "argv": [stata, "-q", "-b", "do", str(wrapper)],
                        "cwd": str(run_dir),
                        "environment": (
                            dict(sorted(environment.items()))
                            if environment is not None else None
                        ),
                        "stdout_stderr": "merged",
                    },
                    sort_keys=True,
                    separators=(",", ":"),
                ) + "\n",
                encoding="ascii",
            )
            if status_path.is_file():
                shutil.copy2(status_path, log_dir / f"{stable_name}.status")
        return FixtureResult(
            path=path,
            process_returncode=process_returncode,
            stata_rc=stata_rc,
            log_text=log_text,
            error=error,
            failure_kind=failure_kind,
            metadata=metadata,
            process_output=process_output,
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stata", default=os.environ.get("STATA_BIN"))
    parser.add_argument("--plugin", type=Path)
    parser.add_argument("--plugin-sha256")
    parser.add_argument("--log-dir", type=Path)
    parser.add_argument("--summary-json", type=Path)
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args()
    if not args.stata:
        raise SystemExit("set STATA_BIN or pass --stata")
    root = Path(__file__).resolve().parents[1]
    paths = args.paths or sorted((root / "tests/stata").rglob("*.do"))
    failures: list[FixtureResult] = []
    results: list[FixtureResult] = []
    for path in paths:
        print(f"==> {path}")
        result = run_fixture(
            args.stata,
            path,
            root,
            plugin=args.plugin,
            plugin_sha256=args.plugin_sha256,
            log_dir=args.log_dir,
            require_status=True,
        )
        results.append(result)
        if result.error:
            failures.append(result)
            print(f"FAIL: {result.error}")
            if result.log_text:
                print("--- fixture log tail ---")
                print("\n".join(result.log_text.splitlines()[-60:]))
        else:
            print(f"PASS: {expected_sentinel(path)}")
    first_metadata = next((item.metadata for item in results if item.metadata), {})
    first_failure = failures[0] if failures else None
    summary_payload: dict[str, object] = {
        "schema_version": 1,
        "completed": True,
        "status": "failure" if failures else "success",
        "failure_kind": first_failure.failure_kind if first_failure else None,
        "failure_detail": first_failure.error if first_failure else None,
        "stata_rc": first_failure.stata_rc if first_failure else 0,
        "stata_version": first_metadata.get("stata_version"),
        "stata_edition": first_metadata.get("stata_edition"),
        "stata_os": first_metadata.get("stata_os"),
        "stata_machine_type": first_metadata.get("stata_machine_type"),
        "stata_processors": optional_int(first_metadata.get("stata_processors")),
        "tests_passed": len(results) - len(failures),
        "tests_failed": len(failures),
        "results": [
            {
                "fixture": item.path.relative_to(root).as_posix(),
                "status": "failure" if item.error else "success",
                "failure_kind": item.failure_kind,
                "failure_detail": item.error,
                "process_rc": item.process_returncode,
                "stata_rc": item.stata_rc,
            }
            for item in results
        ],
    }
    if args.summary_json is not None:
        write_json_atomic(args.summary_json, summary_payload)
    if failures:
        failure_summary = "; ".join(
            f"{item.path}: {item.error}" for item in failures
        )
        raise SystemExit("Stata tests failed: " + failure_summary)


if __name__ == "__main__":
    main()
