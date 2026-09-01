#!/usr/bin/env python3
"""Capture one exact plugin load in platform-native licensed Stata.

The capture is deliberately scope-limited: Stata loads the supplied artifact
by exact path and executes only the plugin ``version`` action.  Artifact and
capture-program bytes are hashed before the process, checked again after it,
and then bound to the closed JSON consumed by
``feppml_platform_load_receipt.py``.  The exact Stata status, batch log,
wrapper, and invocation are retained as immutable evidence sidecars so the
offline verifier can recompute the execution claims.  Licensed raw logs are
run evidence and must not be committed to the source repository.

The command-line runner is for macOS and Linux.  Windows uses the same Stata
capture program from ``windows-ci.do`` because the accepted private Windows
controller owns the licensed process invocation.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform as host_platform
import re
import shutil
import subprocess
import tempfile
from typing import Any, Callable, Mapping, Sequence

try:
    from scripts import feppml_platform_load_receipt as platform_receipt
except ImportError:
    import feppml_platform_load_receipt as platform_receipt  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[1]
CAPTURE_PROGRAM = (
    ROOT / "tests/stata/installation/feppml_platform_load_capture.do"
)
EXPECTED_HOST = {
    "macos-universal": "Darwin",
    "linux-x86_64": "Linux",
}
EXPECTED_ARTIFACT = {
    "macos-universal": "fereg_macos.plugin",
    "linux-x86_64": "fereg_linux.plugin",
}
STATUS_FIELDS = {
    "schema_version",
    "completed",
    "stata_rc",
    "stata_version",
    "stata_flavor",
    "stata_os",
}
STATA_BASENAME = re.compile(r"^stata(?:-mp|-se)?$", re.IGNORECASE)
TERMINAL_SENTINEL = "FEPPML_LICENSED_STATA_LOAD_COMPLETE"


class CaptureError(RuntimeError):
    """The licensed-load process or exact-byte binding failed closed."""


def _regular_file(path: Path, where: str) -> Path:
    source = Path(path)
    if source.is_symlink():
        raise CaptureError(f"{where} must be a regular non-symlink file")
    resolved = source.resolve()
    if not resolved.is_file() or resolved.is_symlink():
        raise CaptureError(f"{where} must be a regular non-symlink file")
    return resolved


def _stata_string(path: Path) -> str:
    text = str(path.resolve())
    if '"' in text:
        raise CaptureError("Stata path contains an unsupported quote")
    return f'"{text}"'


def _resolve_stata(value: str) -> Path:
    selected = Path(value)
    if selected.parent == Path("."):
        discovered = shutil.which(value)
        if discovered is None:
            raise CaptureError("licensed Stata executable was not found")
        selected = Path(discovered)
    executable = _regular_file(selected, "licensed Stata executable")
    if STATA_BASENAME.fullmatch(executable.name) is None:
        raise CaptureError("licensed Stata executable has an unsupported basename")
    if not os.access(executable, os.X_OK):
        raise CaptureError("licensed Stata executable is not executable")
    return executable


def _resolve_source(
    source_repo: Path,
    source_commit: str,
    expected_tree: str | None = None,
) -> tuple[Path, str, str]:
    repo = source_repo.resolve()
    if not (repo / ".git").exists() or platform_receipt.HEX_SHA1.fullmatch(
        source_commit
    ) is None:
        raise CaptureError("candidate source repository or full commit is invalid")
    if expected_tree is not None and platform_receipt.HEX_SHA1.fullmatch(
        expected_tree
    ) is None:
        raise CaptureError("expected candidate source tree is malformed")
    try:
        commit = subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", f"{source_commit}^{{commit}}"],
            text=True,
            stderr=subprocess.PIPE,
        ).strip()
        tree = subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", f"{source_commit}^{{tree}}"],
            text=True,
            stderr=subprocess.PIPE,
        ).strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise CaptureError("candidate source commit did not resolve exactly") from error
    if commit != source_commit or platform_receipt.HEX_SHA1.fullmatch(tree) is None:
        raise CaptureError("candidate source commit/tree identity is malformed")
    if expected_tree is not None and tree != expected_tree:
        raise CaptureError("reconstructed source tree differs from the exact candidate")
    return repo, commit, tree


def _resolve_controller(
    controller_repo: Path,
    controller_commit: str,
    program_path: Path,
    expected_tree: str | None = None,
) -> tuple[Path, str, str]:
    if expected_tree is not None and platform_receipt.HEX_SHA1.fullmatch(
        expected_tree
    ) is None:
        raise CaptureError("expected capture controller tree is malformed")
    try:
        controller, program = platform_receipt._controller_identity(
            controller_repo,
            controller_commit,
            program_path,
            platform_receipt.qualification.sha256_file(program_path),
        )
    except platform_receipt.PlatformReceiptError as error:
        raise CaptureError(str(error)) from error
    if expected_tree is not None and controller["controller_tree"] != expected_tree:
        raise CaptureError("capture controller tree differs from the exact harness")
    return (
        controller_repo.resolve(),
        controller["controller_commit"],
        controller["controller_tree"],
    )


def _read_status(path: Path) -> Mapping[str, str]:
    if not path.is_file() or path.is_symlink():
        raise CaptureError("licensed Stata did not write a capture status")
    values: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        key, separator, value = line.partition("=")
        if not separator or not key or key in values:
            raise CaptureError("licensed Stata capture status is malformed")
        values[key] = value
    if set(values) != STATUS_FIELDS:
        raise CaptureError("licensed Stata capture status fields are not closed")
    if values["schema_version"] != "1" or values["completed"] != "1":
        raise CaptureError("licensed Stata capture status is incomplete")
    try:
        stata_rc = int(values["stata_rc"])
    except ValueError as error:
        raise CaptureError("licensed Stata capture return code is malformed") from error
    if stata_rc != 0:
        raise CaptureError(f"licensed Stata capture returned r({stata_rc})")
    for field in ("stata_version", "stata_flavor", "stata_os"):
        if not values[field]:
            raise CaptureError(f"licensed Stata capture omitted {field}")
    return values


def _read_capture(path: Path) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="ascii"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CaptureError(f"licensed Stata capture JSON is invalid: {error}") from error
    if not isinstance(value, Mapping):
        raise CaptureError("licensed Stata capture JSON must be an object")
    return value


def _publish_new(source: Path, destination: Path) -> None:
    data = source.read_bytes()
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise CaptureError(f"refusing to overwrite immutable capture: {destination}") from error
    try:
        with os.fdopen(descriptor, "wb") as target:
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
        destination.chmod(0o444)
    except BaseException:
        try:
            destination.unlink()
        except OSError:
            pass
        raise


def capture(
    *,
    platform: str,
    artifact: Path,
    stata: str,
    output: Path,
    source_repo: Path,
    source_commit: str,
    source_tree: str | None = None,
    controller_repo: Path | None = None,
    controller_commit: str | None = None,
    controller_tree: str | None = None,
    source_archive: Path | None = None,
    attestation: Path | None = None,
    runner: Callable[..., subprocess.CompletedProcess[Any]] = subprocess.run,
    system: str | None = None,
) -> Mapping[str, Any]:
    """Run the bounded native Stata load and publish an immutable capture."""

    expected_system = EXPECTED_HOST.get(platform)
    if expected_system is None:
        raise CaptureError(
            "the command-line capture supports macOS/Linux; use windows-ci.do on Windows"
        )
    observed_system = system or host_platform.system()
    if observed_system != expected_system:
        raise CaptureError(
            f"platform {platform} requires a native {expected_system} host"
        )
    artifact_path = _regular_file(artifact, "plugin artifact")
    if artifact_path.name != EXPECTED_ARTIFACT[platform]:
        raise CaptureError(
            f"platform {platform} requires {EXPECTED_ARTIFACT[platform]}"
        )
    program_path = _regular_file(CAPTURE_PROGRAM, "capture program")
    _repo, exact_commit, _source_tree = _resolve_source(
        source_repo, source_commit, source_tree
    )
    exact_controller_repo = controller_repo or source_repo
    exact_controller_commit = controller_commit or source_commit
    _controller_repo, exact_controller_commit, exact_controller_tree = _resolve_controller(
        exact_controller_repo,
        exact_controller_commit,
        program_path,
        controller_tree,
    )
    if (source_archive is None) != (attestation is None):
        raise CaptureError(
            "source archive and native execution attestation must be supplied together"
        )
    producer_path: Path | None = None
    if attestation is not None:
        try:
            producer_path = platform_receipt._native_producer(
                exact_controller_repo, exact_controller_commit
            )
        except platform_receipt.PlatformReceiptError as error:
            raise CaptureError(str(error)) from error
    source_identity: Mapping[str, Any] | None = None
    if source_archive is not None:
        try:
            source_identity, _archive = platform_receipt._source_identity(
                source_repo, exact_commit, source_archive
            )
        except platform_receipt.PlatformReceiptError as error:
            raise CaptureError(str(error)) from error
    embedded_commit = exact_commit[:12]
    if embedded_commit.encode("ascii") not in artifact_path.read_bytes():
        raise CaptureError("plugin artifact omits the exact embedded source commit")
    stata_path = _resolve_stata(stata)
    output_path = output.resolve()
    if output.is_symlink() or output_path.exists():
        raise CaptureError(f"refusing to overwrite immutable capture: {output_path}")

    artifact_sha256 = platform_receipt.qualification.sha256_file(artifact_path)
    artifact_size_bytes = artifact_path.stat().st_size
    program_sha256 = platform_receipt.qualification.sha256_file(program_path)
    if artifact_size_bytes <= 0:
        raise CaptureError("plugin artifact is empty")

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="feppml-platform-load-", dir=output_path.parent
    ) as temporary:
        run_root = Path(temporary)
        raw_capture = run_root / "capture.json"
        status_path = run_root / "capture.status"
        wrapper_path = run_root / "run.do"
        isolated = run_root / "sysdirs"
        for name in ("PERSONAL", "PLUS", "OLDPLACE", "SITE"):
            target = isolated / name.lower()
            target.mkdir(parents=True)
        wrapper_path.write_text(
            platform_receipt.native_wrapper_text(
                capture_program=program_path,
                artifact=artifact_path,
                platform=platform,
                artifact_sha256=artifact_sha256,
                artifact_size_bytes=artifact_size_bytes,
                capture_program_sha256=program_sha256,
                source_commit=exact_commit,
                embedded_git_commit=embedded_commit,
                raw_capture=raw_capture,
                status=status_path,
                isolated_root=isolated,
            ),
            encoding="utf-8",
        )
        command = [str(stata_path), "-q", "-b", "do", str(wrapper_path)]
        try:
            completed = runner(
                command,
                cwd=run_root,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                check=False,
            )
        except OSError as error:
            raise CaptureError(f"licensed Stata failed to launch: {error}") from error
        if completed.returncode != 0:
            raise CaptureError(
                f"licensed Stata process returned {completed.returncode}"
            )
        status = _read_status(status_path)
        log_path = run_root / "run.log"
        if not log_path.is_file():
            raise CaptureError("licensed Stata did not write its batch log")
        sentinels = [
            line.strip()
            for line in log_path.read_text(
                encoding="utf-8", errors="replace"
            ).splitlines()
            if line.strip() == TERMINAL_SENTINEL
        ]
        if len(sentinels) != 1:
            raise CaptureError("licensed Stata terminal sentinel is absent or duplicated")
        embedded_lines = [
            line.strip()
            for line in log_path.read_text(
                encoding="utf-8", errors="replace"
            ).splitlines()
            if line.strip() == f"fereg.git_commit={embedded_commit}"
        ]
        if len(embedded_lines) != 1:
            raise CaptureError("plugin version output omits the exact embedded source commit")
        capture_value = _read_capture(raw_capture)
        try:
            validated = platform_receipt.validate_capture(
                capture_value, artifact_path, platform
            )
        except platform_receipt.PlatformReceiptError as error:
            raise CaptureError(str(error)) from error
        if (
            validated["capture_program_sha256"] != program_sha256
            or validated["source_commit"] != exact_commit
            or validated["embedded_git_commit"] != embedded_commit
            or platform_receipt.qualification.sha256_file(program_path)
            != program_sha256
            or platform_receipt.qualification.sha256_file(artifact_path)
            != artifact_sha256
            or artifact_path.stat().st_size != artifact_size_bytes
        ):
            raise CaptureError("artifact or capture-program bytes changed during Stata load")
        if (
            validated["stata_version"] != status["stata_version"]
            or validated["stata_flavor"] != status["stata_flavor"]
        ):
            raise CaptureError("capture JSON disagrees with Stata-owned process metadata")
        _publish_new(raw_capture, output_path)
        if attestation is not None:
            assert producer_path is not None
            attestation_path = attestation.resolve()
            if attestation.is_symlink() or attestation_path.exists():
                raise CaptureError(
                    f"refusing to overwrite immutable attestation: {attestation_path}"
                )
            controller_identity = {
                "controller_commit": exact_controller_commit,
                "controller_tree": exact_controller_tree,
            }
            sidecars = platform_receipt.native_sidecar_paths(attestation_path)
            invocation = run_root / "invocation.json"
            invocation.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "kind": "feppml-native-capture-invocation-v1",
                        "platform": platform,
                        "source_commit": exact_commit,
                        "controller_commit": exact_controller_commit,
                        "artifact_sha256": artifact_sha256,
                        "capture_program_sha256": program_sha256,
                        "producer_sha256": (
                            platform_receipt.qualification.sha256_file(producer_path)
                        ),
                        "stata_executable_sha256": (
                            platform_receipt.qualification.sha256_file(stata_path)
                        ),
                        "stata_executable_size_bytes": stata_path.stat().st_size,
                        "process_returncode": completed.returncode,
                        "wrapper_sha256": (
                            platform_receipt.qualification.sha256_file(wrapper_path)
                        ),
                        "cwd": str(run_root),
                        "stata_executable": str(stata_path),
                        "capture_program": str(program_path),
                        "artifact": str(artifact_path),
                        "raw_capture": str(raw_capture),
                        "status": str(status_path),
                        "isolated_root": str(isolated),
                        "wrapper": str(wrapper_path),
                        "argv": command,
                        "complete_sentinel": (
                            "FEPPML_NATIVE_CAPTURE_INVOCATION_COMPLETE"
                        ),
                    },
                    sort_keys=True,
                    indent=2,
                )
                + "\n",
                encoding="ascii",
            )
            for source_path, destination in (
                (status_path, sidecars["status"]),
                (log_path, sidecars["stata_log"]),
                (wrapper_path, sidecars["wrapper"]),
                (invocation, sidecars["invocation"]),
            ):
                _publish_new(source_path, destination)
            payload = {
                "schema_version": 1,
                "kind": platform_receipt.NATIVE_ATTESTATION_KIND,
                "platform": platform,
                **dict(source_identity),
                **controller_identity,
                "artifact_sha256": artifact_sha256,
                "artifact_size_bytes": artifact_size_bytes,
                "capture_sha256": platform_receipt.qualification.sha256_file(
                    output_path
                ),
                "capture_program_sha256": program_sha256,
                "producer_sha256": platform_receipt.qualification.sha256_file(
                    producer_path
                ),
                "producer_size_bytes": producer_path.stat().st_size,
                "stata_executable_sha256": (
                    platform_receipt.qualification.sha256_file(stata_path)
                ),
                "stata_executable_size_bytes": stata_path.stat().st_size,
                "stata_version": status["stata_version"],
                "stata_flavor": status["stata_flavor"],
                "process_returncode": completed.returncode,
                "stata_rc": int(status["stata_rc"]),
                "status_sha256": platform_receipt.qualification.sha256_file(
                    sidecars["status"]
                ),
                "status_size_bytes": sidecars["status"].stat().st_size,
                "raw_log_sha256": platform_receipt.qualification.sha256_file(
                    sidecars["stata_log"]
                ),
                "raw_log_size_bytes": sidecars["stata_log"].stat().st_size,
                "wrapper_sha256": platform_receipt.qualification.sha256_file(
                    sidecars["wrapper"]
                ),
                "wrapper_size_bytes": sidecars["wrapper"].stat().st_size,
                "invocation_sha256": platform_receipt.qualification.sha256_file(
                    sidecars["invocation"]
                ),
                "invocation_size_bytes": sidecars["invocation"].stat().st_size,
                "terminal_sentinel_count": len(sentinels),
                "embedded_commit_line_count": len(embedded_lines),
                "licensed_stata_load": True,
                "mock_only": False,
                "scientific_claim": False,
                "complete_sentinel": "FEPPML_NATIVE_CAPTURE_EXECUTION_COMPLETE",
            }
            platform_receipt._write(
                attestation_path, platform_receipt.qualification.seal_payload(payload)
            )
    return validated


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument(
        "--platform", choices=platform_receipt.PLATFORMS, required=True
    )
    result.add_argument("--artifact", type=Path, required=True)
    result.add_argument("--stata", required=True)
    result.add_argument("--output", type=Path, required=True)
    result.add_argument("--source-repo", type=Path, default=ROOT)
    result.add_argument("--source-commit", required=True)
    result.add_argument("--source-tree")
    result.add_argument("--controller-repo", type=Path)
    result.add_argument("--controller-commit")
    result.add_argument("--controller-tree")
    result.add_argument("--source-archive", type=Path)
    result.add_argument("--attestation", type=Path)
    result.add_argument("--receipt", type=Path)
    result.add_argument("--record", type=Path)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        capture(
            platform=args.platform,
            artifact=args.artifact,
            stata=args.stata,
            output=args.output,
            source_repo=args.source_repo,
            source_commit=args.source_commit,
            source_tree=args.source_tree,
            controller_repo=args.controller_repo,
            controller_commit=args.controller_commit,
            controller_tree=args.controller_tree,
            source_archive=args.source_archive,
            attestation=args.attestation,
        )
        seal_arguments = (
            args.source_archive,
            args.attestation,
            args.receipt,
            args.record,
        )
        if any(value is not None for value in seal_arguments):
            if not all(value is not None for value in seal_arguments):
                raise CaptureError(
                    "--source-archive, --receipt, and --record must be supplied together"
                )
            archive = _regular_file(args.source_archive, "candidate source archive")
            platform_receipt.create(
                platform=args.platform,
                artifact=args.artifact,
                capture_path=args.output,
                receipt=args.receipt,
                record=args.record,
                source_repo=args.source_repo,
                source_commit=args.source_commit,
                source_archive=archive,
                controller_repo=args.controller_repo or args.source_repo,
                controller_commit=args.controller_commit or args.source_commit,
                capture_program=CAPTURE_PROGRAM,
                native_attestation=args.attestation,
                native_stata_executable=_resolve_stata(args.stata),
            )
    except (CaptureError, platform_receipt.PlatformReceiptError) as error:
        raise SystemExit(f"feppml licensed-load capture failed closed: {error}") from error
    print("FEPPML_LICENSED_STATA_LOAD_CAPTURE_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
