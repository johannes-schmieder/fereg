#!/usr/bin/env python3
"""Prepare a clean Windows controller commit around an exact target archive.

The target source and trusted harness are intentionally independent.  This is
what permits a pre-harness F0 commit to be built by the current reviewed
Windows controller without rewriting either identity.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
from typing import Any, Mapping, Sequence

try:
    from scripts import feppml_qualification as qualification
    from scripts.prepare_windows_feppml_target import CONTRACT_PATHS
except ImportError:
    import feppml_qualification as qualification  # type: ignore[no-redef]
    from prepare_windows_feppml_target import CONTRACT_PATHS  # type: ignore[no-redef]


COMMIT = re.compile(r"^[0-9a-f]{40}$")
MODES = ("candidate", "f0-baseline")


class PreparationError(RuntimeError):
    """A target, harness, archive, or clean controller failed closed."""


def _run(command: Sequence[str], *, cwd: Path | None = None) -> bytes:
    completed = subprocess.run(
        list(command), cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode:
        raise PreparationError(
            completed.stderr.decode("utf-8", errors="replace")[:500]
        )
    return completed.stdout


def _git(repo: Path, *arguments: str) -> bytes:
    return _run(["git", "-C", str(repo), *arguments])


def _identity(repo: Path, commit: str) -> tuple[str, str]:
    resolved = repo.resolve()
    if COMMIT.fullmatch(commit) is None:
        raise PreparationError("source commit must be a full lowercase SHA")
    observed = _git(resolved, "rev-parse", f"{commit}^{{commit}}").decode().strip()
    tree = _git(resolved, "rev-parse", f"{commit}^{{tree}}").decode().strip()
    if observed != commit or COMMIT.fullmatch(tree) is None:
        raise PreparationError("source commit/tree did not resolve exactly")
    return commit, tree


def _extract(archive: Path, destination: Path) -> None:
    destination.mkdir(parents=True)
    with tarfile.open(archive, "r:") as source:
        for member in source.getmembers():
            relative = Path(member.name)
            if (
                relative.is_absolute()
                or ".." in relative.parts
                or not (member.isfile() or member.isdir())
            ):
                raise PreparationError(f"unsafe harness archive member: {member.name}")
        source.extractall(destination)


def _write_new(path: Path, value: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    except FileExistsError as error:
        raise PreparationError(f"refusing to overwrite {path}") from error
    with os.fdopen(descriptor, "wb") as target:
        target.write(value)


def prepare(
    *,
    output_repo: Path,
    target_repo: Path,
    target_commit: str,
    harness_repo: Path,
    harness_commit: str,
    mode: str,
) -> Mapping[str, Any]:
    output = output_repo.resolve()
    if output.exists():
        raise PreparationError("controller repository already exists")
    if mode not in MODES:
        raise PreparationError("unsupported Windows controller mode")
    target = target_repo.resolve()
    harness = harness_repo.resolve()
    target_commit, target_tree = _identity(target, target_commit)
    harness_commit, harness_tree = _identity(harness, harness_commit)
    if mode == "f0-baseline" and target_commit == harness_commit:
        raise PreparationError("F0 target must be distinct from the trusted harness")
    if mode == "candidate" and (
        target_commit != harness_commit or target_tree != harness_tree
    ):
        raise PreparationError(
            "candidate target must equal the trusted harness commit and tree"
        )
    target_archive = _git(target, "archive", "--format=tar", target_commit)
    try:
        target_timestamp = int(
            _git(target, "show", "-s", "--format=%ct", target_commit)
            .decode()
            .strip()
        )
    except ValueError as error:
        raise PreparationError("target commit timestamp is malformed") from error
    target_build_date = datetime.fromtimestamp(
        target_timestamp, timezone.utc
    ).date().isoformat()
    harness_archive = _git(harness, "archive", "--format=tar", harness_commit)
    with tempfile.TemporaryDirectory(prefix="feppml-windows-harness-") as temporary:
        archive_path = Path(temporary) / "harness.tar"
        archive_path.write_bytes(harness_archive)
        _extract(archive_path, output)
    for relative in CONTRACT_PATHS:
        path = output / relative
        if not path.is_file() or path.is_symlink():
            raise PreparationError(f"trusted harness omits {relative}")
    target_path = output / "input/feppml-windows-target-source.tar"
    _write_new(target_path, target_archive)
    contract = {
        relative: {
            "sha256": qualification.sha256_file(output / relative),
            "size_bytes": (output / relative).stat().st_size,
        }
        for relative in CONTRACT_PATHS
    }
    payload = {
        "schema_version": 1,
        "kind": "feppml_windows_controller_preparation",
        "mode": mode,
        "target": {
            "commit": target_commit,
            "tree": target_tree,
            "archive_sha256": qualification.sha256_file(target_path),
            "archive_size_bytes": target_path.stat().st_size,
            "embedded_git_commit": target_commit[:12],
            "build_date_utc": target_build_date,
        },
        "harness": {"commit": harness_commit, "tree": harness_tree},
        "contract_files": contract,
        "complete_sentinel": "FEPPML_WINDOWS_CONTROLLER_PREPARED",
    }
    preparation = output / "input/feppml-windows-preparation.json"
    _write_new(
        preparation,
        json.dumps(
            qualification.seal_payload(payload), sort_keys=True, indent=2,
            ensure_ascii=True,
        ).encode("ascii") + b"\n",
    )
    _run(["git", "init", "-q"], cwd=output)
    _run(["git", "config", "user.name", "feppml Windows controller"], cwd=output)
    _run(
        ["git", "config", "user.email", "feppml-controller@example.invalid"],
        cwd=output,
    )
    _run(["git", "add", "-A"], cwd=output)
    environment = dict(os.environ)
    environment.update(
        {
            "GIT_AUTHOR_DATE": "2000-01-01T00:00:00Z",
            "GIT_COMMITTER_DATE": "2000-01-01T00:00:00Z",
        }
    )
    completed = subprocess.run(
        [
            "git", "commit", "-qm",
            f"feppml Windows controller {target_commit} via {harness_commit}",
        ],
        cwd=output,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode:
        raise PreparationError(completed.stderr.decode(errors="replace")[:500])
    controller_commit = _git(output, "rev-parse", "HEAD").decode().strip()
    controller_tree = _git(output, "rev-parse", "HEAD^{tree}").decode().strip()
    if _git(output, "status", "--porcelain").strip():
        raise PreparationError("prepared Windows controller is not clean")
    return {
        "record_type": "feppml-windows-controller-prepared-v1",
        "controller_commit": controller_commit,
        "controller_tree": controller_tree,
        "target": payload["target"],
        "harness": payload["harness"],
        "preparation_sha256": qualification.sha256_file(preparation),
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--output-repo", type=Path, required=True)
    result.add_argument("--target-repo", type=Path, required=True)
    result.add_argument("--target-commit", required=True)
    result.add_argument("--harness-repo", type=Path, required=True)
    result.add_argument("--harness-commit", required=True)
    result.add_argument("--mode", choices=MODES, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        result = prepare(
            output_repo=args.output_repo,
            target_repo=args.target_repo,
            target_commit=args.target_commit,
            harness_repo=args.harness_repo,
            harness_commit=args.harness_commit,
            mode=args.mode,
        )
    except (PreparationError, OSError, tarfile.TarError) as error:
        raise SystemExit(f"Windows controller preparation failed closed: {error}") from error
    print(json.dumps(result, sort_keys=True))
    print("FEPPML_WINDOWS_CONTROLLER_PREPARE_COMPLETE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
