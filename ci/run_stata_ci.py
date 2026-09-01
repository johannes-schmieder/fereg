#!/usr/bin/env python3
"""Stage fereg, run one licensed-Stata profile, and emit a receipt."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import hashlib
from datetime import datetime, timezone


DEFAULT_STATA = Path("/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp")
EVIDENCE_NAMES = (
    "process.json",
    "process.stdout.log",
    "process.stderr.log",
    "stata.status",
    "stata.log",
    "profile-summary.json",
    "fixture-summary.json",
    "fixture-logs",
    "plugin-evidence",
    "source.json",
    "source.tar",
)


def command_output(command: list[str], cwd: Path) -> str:
    return subprocess.check_output(command, cwd=cwd, text=True).strip()


def repo_root() -> Path:
    declared_root = os.environ.get("FEREG_REPO_ROOT")
    if declared_root:
        root = Path(declared_root).expanduser().resolve()
        if not (root / "ci" / "stata_profiles.json").is_file():
            raise RuntimeError(
                "FEREG_REPO_ROOT does not contain ci/stata_profiles.json"
            )
        return root
    return Path(
        command_output(["git", "rev-parse", "--show-toplevel"], Path.cwd())
    ).resolve()


def declared_files(root: Path, manifest: Path) -> list[Path]:
    root = root.resolve()
    if not manifest.is_file():
        raise RuntimeError(f"tracked-file manifest does not exist: {manifest}")
    files: list[Path] = []
    seen: set[Path] = set()
    for line_number, name in enumerate(
        manifest.read_text(encoding="utf-8").splitlines(), start=1
    ):
        if not name:
            continue
        relative = Path(name)
        source = (root / relative).resolve()
        if relative.is_absolute() or not source.is_relative_to(root):
            raise RuntimeError(
                f"unsafe path in tracked-file manifest at line {line_number}: "
                f"{name!r}"
            )
        if relative in seen:
            raise RuntimeError(
                f"duplicate path in tracked-file manifest at line "
                f"{line_number}: {name!r}"
            )
        if not source.is_file():
            raise RuntimeError(
                f"missing declared source file at line {line_number}: {name!r}"
            )
        seen.add(relative)
        files.append(relative)
    if not files:
        raise RuntimeError("tracked-file manifest is empty")
    return files


def tracked_files(root: Path, manifest: Path | None = None) -> list[Path]:
    if manifest is not None:
        return declared_files(root, manifest)
    raw = subprocess.check_output(["git", "ls-files", "--cached", "-z"], cwd=root)
    files: list[Path] = []
    for name in raw.split(b"\0"):
        if not name:
            continue
        relative = Path(os.fsdecode(name))
        source = (root / relative).resolve()
        if source.is_file() and source.is_relative_to(root):
            files.append(relative)
    return files


def stage_repository(
    root: Path, destination: Path, manifest: Path | None = None
) -> None:
    for relative in tracked_files(root, manifest):
        source = root / relative
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def materialize_exact_commit(
    root: Path, commit: str, destination: Path, archive_path: Path
) -> dict[str, object]:
    """Stage only bytes from one resolved commit and retain the exact archive."""

    if os.environ.get("FEREG_TRACKED_FILES_MANIFEST"):
        raise RuntimeError(
            "FEREG_TRACKED_FILES_MANIFEST is not allowed for formal Stata evidence; "
            "the exact tested commit is always materialized"
        )
    resolved = command_output(["git", "rev-parse", f"{commit}^{{commit}}"], root)
    tree = command_output(["git", "rev-parse", f"{resolved}^{{tree}}"], root)
    timestamp_raw = command_output(
        ["git", "show", "-s", "--format=%cI", resolved], root
    )
    try:
        timestamp = datetime.fromisoformat(timestamp_raw.replace("Z", "+00:00"))
    except ValueError as error:
        raise RuntimeError("tested commit timestamp is not exact ISO-8601") from error
    if timestamp.tzinfo is None:
        raise RuntimeError("tested commit timestamp has no timezone")
    build_date = timestamp.astimezone(timezone.utc).strftime("%Y-%m-%d")
    with archive_path.open("xb") as archive:
        completed = subprocess.run(
            ["git", "archive", "--format=tar", resolved], cwd=root,
            stdout=archive, stderr=subprocess.PIPE, check=False,
        )
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"cannot materialize exact tested commit: {detail}")
    try:
        with tarfile.open(archive_path, mode="r:") as archive:
            seen: set[str] = set()
            for member in archive.getmembers():
                relative = Path(member.name)
                target = (destination / relative).resolve()
                if (
                    member.name in seen or not member.name
                    or relative.is_absolute() or ".." in relative.parts
                    or destination.resolve() not in target.parents
                    or not (member.isdir() or member.isfile())
                ):
                    raise RuntimeError(
                        f"exact source archive contains an unsafe member: {member.name!r}"
                    )
                seen.add(member.name)
                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                    target.chmod(member.mode & 0o777)
                    continue
                target.parent.mkdir(parents=True, exist_ok=True)
                source = archive.extractfile(member)
                if source is None:
                    raise RuntimeError(
                        f"cannot read exact source archive member: {member.name}"
                    )
                with source, target.open("xb") as output:
                    shutil.copyfileobj(source, output, length=1 << 20)
                target.chmod(member.mode & 0o777)
    except (OSError, tarfile.TarError) as error:
        raise RuntimeError(f"cannot extract exact tested source archive: {error}") from error
    return {
        "schema_version": 1,
        "record_type": "fereg-exact-source-archive-v1",
        "source": {"commit": resolved, "tree": tree},
        "build_date_utc": build_date,
        "archive": {
            "filename": archive_path.name,
            "sha256": sha256_file(archive_path),
            "size_bytes": archive_path.stat().st_size,
        },
    }


def clear_directory(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    for child in path.iterdir():
        if child.is_dir() and not child.is_symlink():
            shutil.rmtree(child)
        else:
            child.unlink()


def remove_tree_with_retries(
    path: Path, *, attempts: int = 5, initial_delay_seconds: float = 0.05
) -> None:
    """Remove an APFS tree despite a bounded transient ENOTEMPTY race."""
    for attempt in range(attempts):
        try:
            shutil.rmtree(path)
            return
        except FileNotFoundError:
            return
        except OSError:
            if attempt + 1 == attempts:
                raise
            time.sleep(initial_delay_seconds * (2**attempt))


def copy_evidence(run_root: Path, artifact_dir: Path) -> None:
    for name in EVIDENCE_NAMES:
        source = run_root / name
        destination = artifact_dir / name
        if not source.exists():
            continue
        if destination.exists():
            if destination.is_dir() and not destination.is_symlink():
                shutil.rmtree(destination)
            else:
                destination.unlink()
        if source.is_dir():
            shutil.copytree(source, destination)
        else:
            shutil.copy2(source, destination)


def bundle_version(stata_executable: Path) -> str:
    info_plist = stata_executable.parents[1] / "Info.plist"
    try:
        with info_plist.open("rb") as handle:
            value = plistlib.load(handle).get("CFBundleShortVersionString")
    except (OSError, plistlib.InvalidFileException):
        return "unknown"
    return str(value) if value is not None else "unknown"


def tail(path: Path, lines: int = 100) -> str:
    try:
        values = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return ""
    return "\n".join(values[-lines:])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("profile", nargs="?", default="smoke")
    args = parser.parse_args()

    root = repo_root()
    artifact_dir = root / ".ci" / "stata" / "run"
    clear_directory(artifact_dir)
    parent = Path(os.environ.get("RUNNER_TEMP", tempfile.gettempdir())).resolve()
    parent.mkdir(parents=True, exist_ok=True)
    run_root = Path(tempfile.mkdtemp(prefix="fereg-stata-ci-", dir=parent))
    staged_root = run_root / "repo"
    staged_root.mkdir()
    requested_sha = os.environ.get("GITHUB_SHA") or command_output(
        ["git", "rev-parse", "HEAD^{commit}"], root
    )
    source_record = materialize_exact_commit(
        root, requested_sha, staged_root, run_root / "source.tar"
    )
    tested_sha = str(source_record["source"]["commit"])
    (run_root / "source.json").write_text(
        json.dumps(source_record, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="ascii",
    )
    config_path = staged_root / "ci" / "stata_profiles.json"
    config = json.loads(config_path.read_text(encoding="utf-8"))
    profiles = config.get("profiles", {})
    if args.profile not in profiles:
        choices = ", ".join(sorted(profiles))
        parser.error(f"unknown profile {args.profile!r}; choose one of: {choices}")
    profile = profiles[args.profile]
    for directory in (
        "stata-plus",
        "stata-personal",
        "stata-oldplace",
        "stata-site",
    ):
        (run_root / directory).mkdir()

    stata_executable = Path(
        os.environ.get("STATA_BIN", str(DEFAULT_STATA))
    ).expanduser().resolve()
    status_file = run_root / "stata.status"
    stata_log = run_root / "stata.log"
    profile_summary = run_root / "profile-summary.json"
    process_json = run_root / "process.json"
    process_log = run_root / "process.stdout.log"
    process_error = run_root / "process.stderr.log"
    timeout_seconds = int(profile["timeout_seconds"])
    driver = profile.get("driver", "stata-harness")
    python_executable = str(Path(sys.executable).resolve())

    if driver == "stata-harness":
        profile_command = [
            str(stata_executable),
            "-q",
            "-b",
            "do",
            str(staged_root / "ci" / "stata_ci.do"),
            str(staged_root / str(profile["suite"])),
            str(status_file),
            str(stata_log),
            args.profile,
            str(staged_root),
            str(run_root),
        ]
    elif driver == "fereg-fixtures":
        profile_command = [
            python_executable,
            str(staged_root / "ci" / "run_fixture_profile.py"),
            "--profile-config",
            str(staged_root / "ci" / "stata_profiles.json"),
            "--profile",
            args.profile,
            "--repo-root",
            str(staged_root),
            "--run-root",
            str(run_root),
            "--summary-json",
            str(profile_summary),
            "--stata",
            str(stata_executable),
            "--tested-sha",
            tested_sha,
        ]
    else:
        raise SystemExit(f"unsupported profile driver: {driver}")

    supervisor = [
        python_executable,
        str(staged_root / "ci" / "supervise_stata.py"),
        "--timeout-seconds",
        str(timeout_seconds),
        "--process-json",
        str(process_json),
        "--stdout-log",
        str(process_log),
        "--stderr-log",
        str(process_error),
        "--cwd",
        str(run_root),
        "--",
        *profile_command,
    ]
    formal_environment = os.environ.copy()
    formal_environment["FEREG_GIT_COMMIT"] = tested_sha[:12]
    formal_environment["FEREG_BUILD_DATE"] = str(source_record["build_date_utc"])
    supervisor_rc = subprocess.run(
        supervisor, check=False, env=formal_environment
    ).returncode
    if supervisor_rc != 0 and not process_json.exists():
        process_json.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "started_at": None,
                    "completed_at": None,
                    "duration_seconds": None,
                    "process_rc": supervisor_rc,
                    "timed_out": False,
                    "launch_error": (
                        f"Stata supervisor exited with code {supervisor_rc}"
                    ),
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )

    receipt_path = artifact_dir / "receipt.json"
    make_receipt = [
        python_executable,
        str(staged_root / "ci" / "make_stata_receipt.py"),
        "--profile-config",
        str(staged_root / "ci" / "stata_profiles.json"),
        "--profile",
        args.profile,
        "--process-json",
        str(process_json),
        "--status-file",
        str(status_file),
        "--profile-summary",
        str(profile_summary),
        "--run-dir",
        str(run_root),
        "--receipt",
        str(receipt_path),
        "--tested-sha",
        tested_sha,
        "--repository",
        os.environ.get("GITHUB_REPOSITORY", "local/fereg"),
        "--ref",
        os.environ.get("GITHUB_REF", "local"),
        "--run-id",
        os.environ.get("GITHUB_RUN_ID", "local"),
        "--run-attempt",
        os.environ.get("GITHUB_RUN_ATTEMPT", "1"),
        "--runner-name",
        os.environ.get("RUNNER_NAME", "local-mac"),
        "--stata-executable",
        str(stata_executable),
        "--stata-bundle-version",
        bundle_version(stata_executable),
    ]
    receipt_rc = subprocess.run(make_receipt, check=False).returncode
    copy_evidence(run_root, artifact_dir)

    if os.environ.get("STATA_CI_KEEP_TEMP") == "1":
        print(f"STATA_CI_TEMP={run_root}")
    else:
        remove_tree_with_retries(run_root)

    if receipt_rc != 0:
        return receipt_rc
    checker = [
        sys.executable,
        str(root / "ci" / "check_stata_receipt.py"),
        str(receipt_path),
        "--expect-tested-sha",
        tested_sha,
        "--expect-profile",
        args.profile,
        "--require-success",
    ]
    checker_result = subprocess.run(checker, check=False)
    if checker_result.returncode != 0:
        log_tail = tail(artifact_dir / "process.stdout.log") or tail(
            artifact_dir / "stata.log"
        )
        if log_tail:
            print("--- CI log tail ---", file=sys.stderr)
            print(log_tail, file=sys.stderr)
    return checker_result.returncode


if __name__ == "__main__":
    sys.exit(main())
