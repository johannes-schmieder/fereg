#!/usr/bin/env python3
"""Run one exact local Phase-12 prerequisite command set without a shell."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
CHECKS = {"repository", "python", "rust", "fmt", "clippy", "c_mock", "ffi"}


def _tool(name: str) -> str:
    value = shutil.which(name)
    if value is None:
        raise RuntimeError(f"required Phase-12 tool is absent: {name}")
    return str(Path(value).resolve())


def commands(check_id: str) -> list[list[str]]:
    python = str(Path(sys.executable).resolve())
    cargo = _tool("cargo") if check_id in {"rust", "fmt", "clippy", "ffi"} else ""
    bash = _tool("bash") if check_id == "c_mock" else ""
    return {
        "repository": [[python, str(ROOT / "scripts/check_repository.py")]],
        "python": [
            [python, "-m", "unittest", "discover", "-s", "tests/python", "-v"]
        ],
        "rust": [
            [cargo, "test", "--workspace", "--all-targets"],
            [cargo, "test", "--workspace", "--all-targets", "--no-default-features"],
            [cargo, "test", "--workspace", "--all-targets", "--all-features"],
        ],
        "fmt": [
            [cargo, "fmt", "--all", "--", "--check"],
            [cargo, "fmt", "--manifest-path", "fuzz/Cargo.toml", "--all", "--", "--check"],
        ],
        "clippy": [
            [
                cargo, "clippy", "--workspace", "--all-targets",
                "--", "-D", "warnings",
            ],
            [
                cargo, "clippy", "--workspace", "--all-targets",
                "--no-default-features", "--", "-D", "warnings",
            ],
            [
                cargo, "clippy", "--workspace", "--all-targets", "--all-features",
                "--", "-D", "warnings",
            ]
        ],
        "c_mock": [
            [bash, str(ROOT / "scripts/run_portable_static_checks.sh")],
            [bash, str(ROOT / "tests/mock_spi/run_mock_bridge_test.sh")],
        ],
        "ffi": [[
            cargo, "test", "-p", "fereg-plugin", "--all-targets", "--features", "ffi",
        ]],
    }[check_id]


def canonical_json_bytes(value: object) -> bytes:
    return (
        json.dumps(
            value, sort_keys=True, separators=(",", ":"), ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("ascii")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def file_record(path: Path) -> dict[str, object]:
    return {
        "filename": path.name,
        "sha256": sha256_file(path),
        "size_bytes": path.stat().st_size,
    }


def toolchain(check_id: str) -> str:
    python = f"Python {platform.python_version()} ({Path(sys.executable).resolve()})"
    if check_id in {"rust", "fmt", "clippy", "ffi"}:
        cargo = subprocess.check_output(
            [_tool("cargo"), "--version"], text=True
        ).strip()
        rustc = subprocess.check_output(
            [_tool("rustc"), "--version"], text=True
        ).strip()
        return f"{cargo}; {rustc}"
    if check_id == "c_mock":
        bash = subprocess.check_output(
            [_tool("bash"), "--version"], text=True
        ).splitlines()[0].strip()
        return f"{bash}; {python}"
    return python


def git(*args: str) -> str:
    return subprocess.check_output(
        ["git", "-C", str(ROOT), *args], stderr=subprocess.STDOUT, text=True
    ).strip()


def write_exclusive(path: Path, value: object) -> None:
    data = canonical_json_bytes(value)
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    with os.fdopen(descriptor, "wb") as target:
        target.write(data)
        target.flush()
        os.fsync(target.fileno())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("check_id", choices=sorted(CHECKS))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--evidence-dir", type=Path, required=True)
    args = parser.parse_args()
    command_set = commands(args.check_id)
    source = {"commit": git("rev-parse", "HEAD^{commit}"), "tree": git("rev-parse", "HEAD^{tree}")}
    sentinel = f"FEPPML_PHASE12_{args.check_id.upper()}_PASS"
    evidence = args.evidence_dir.resolve()
    try:
        evidence.mkdir(parents=True, exist_ok=False)
    except FileExistsError as error:
        raise RuntimeError(f"refusing preexisting raw evidence directory: {evidence}") from error
    process_records = []
    overall_rc = 0
    for index, argv in enumerate(command_set):
        stdout = evidence / f"command-{index:03d}.stdout.log"
        stderr = evidence / f"command-{index:03d}.stderr.log"
        with stdout.open("xb") as stdout_handle, stderr.open("xb") as stderr_handle:
            completed = subprocess.run(
                argv, cwd=ROOT, stdin=subprocess.DEVNULL, stdout=stdout_handle,
                stderr=stderr_handle, check=False, shell=False,
            )
        process_records.append(
            {
                "index": index,
                "argv": argv,
                "argv_sha256": hashlib.sha256(canonical_json_bytes(argv)).hexdigest(),
                "exit_code": completed.returncode,
                "stdout": file_record(stdout),
                "stderr": file_record(stderr),
            }
        )
        if completed.returncode != 0:
            overall_rc = completed.returncode or 1
            break
    write_exclusive(
        evidence / "processes.json",
        {
            "schema_version": 1,
            "record_type": "feppml-phase12-local-processes-v1",
            "check_id": args.check_id,
            "source": source,
            "commands": process_records,
            "toolchain": toolchain(args.check_id),
            "status": "pass" if overall_rc == 0 else "failure",
            "complete_sentinel": sentinel if overall_rc == 0 else None,
        },
    )
    if overall_rc != 0:
        return overall_rc
    write_exclusive(
        args.output.resolve(),
        {
            "schema_version": 1,
            "result_type": "feppml-phase12-local-command-summary-v1",
            "check_id": args.check_id,
            "source": source,
            "commands": command_set,
            "status": "pass",
            "complete_sentinel": sentinel,
        },
    )
    print(sentinel, flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
