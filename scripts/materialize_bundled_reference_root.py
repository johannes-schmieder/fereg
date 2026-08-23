#!/usr/bin/env python3
"""Materialize the lock-pinned reference root from local stores and a Git bundle.

The repository-contained `stata-require` bundle is public-source provisioning,
not capture-time network access. This resolver never fetches or enumerates the
filesystem. It combines exactly one allowlisted local object store for each
large dependency with one digest-bound Git bundle, then applies the unchanged
clean-checkout and locked-file preflight before writing a private path file.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Mapping, Sequence

from ols_reference import load_reference_lock, preflight_reference_root
from resolve_reference_root import (
    LOCK,
    ROOT,
    CandidateMaterializationError,
    ResolverError,
    _locked_capture_entries,
    _prepare_materialization_parent,
    _verify_local_object_store,
    repository_candidates,
    write_private_path,
)

LOCAL_COMPONENTS = ("reghdfe", "ftools")
BUNDLED_COMPONENT = "require"
BUNDLE_ROOT = ROOT / "ci/reference-bundles/stata-require"
MATERIALIZED_PREFIX = "stfixest-bundled-reference-"


class BundledResolverError(RuntimeError):
    def __init__(
        self,
        kind: str,
        *,
        reghdfe_matches: int = 0,
        ftools_matches: int = 0,
        bundle_present: bool = False,
    ) -> None:
        super().__init__(kind)
        self.kind = kind
        self.reghdfe_matches = reghdfe_matches
        self.ftools_matches = ftools_matches
        self.bundle_present = bundle_present


def _git_environment() -> dict[str, str]:
    environment = os.environ.copy()
    environment.update(
        {
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_TERMINAL_PROMPT": "0",
        }
    )
    return environment


def _run(arguments: Sequence[str], *, cwd: Path | None = None) -> str:
    completed = subprocess.run(
        list(arguments),
        cwd=cwd,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=_git_environment(),
    )
    if completed.returncode:
        raise BundledResolverError("local_git_command_failed")
    return completed.stdout.strip()


def _run_git(checkout: Path, arguments: Sequence[str]) -> str:
    return _run(["git", "-C", os.fspath(checkout), *arguments])


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _locked_file_hashes(entry: Mapping[str, Any]) -> dict[str, str]:
    raw_files = entry.get("file")
    if not isinstance(raw_files, list) or not raw_files:
        raise BundledResolverError("bundle_lock_files_invalid")
    output: dict[str, str] = {}
    for raw in raw_files:
        if not isinstance(raw, Mapping):
            raise BundledResolverError("bundle_lock_files_invalid")
        path = raw.get("path")
        sha256 = raw.get("sha256")
        if not isinstance(path, str) or not isinstance(sha256, str):
            raise BundledResolverError("bundle_lock_files_invalid")
        if path in output or len(sha256) != 64:
            raise BundledResolverError("bundle_lock_files_invalid")
        output[path] = sha256
    return output


def _bundle_inventory(entry: Mapping[str, Any]) -> tuple[Path, Path]:
    commit = str(entry["commit"])
    directory = BUNDLE_ROOT / commit
    manifest = directory / "manifest.json"
    bundle = directory / f"stata-require-{commit}.bundle"
    for path in (directory, manifest, bundle):
        if path.is_symlink():
            raise BundledResolverError("bundle_inventory_symlink")
    if not directory.is_dir() or not manifest.is_file() or not bundle.is_file():
        raise BundledResolverError("bundle_inventory_absent")
    return manifest, bundle


def _validate_bundle_manifest(
    entry: Mapping[str, Any],
    manifest_path: Path,
    bundle_path: Path,
) -> None:
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise BundledResolverError("bundle_manifest_invalid") from exc
    expected = {
        "schema": 1,
        "status": "success",
        "source_repository": entry["repository"],
        "source_commit": entry["commit"],
        "required_files": _locked_file_hashes(entry),
        "bundle_file": bundle_path.name,
        "capture_network_policy": "offline-only",
    }
    for key, value in expected.items():
        if manifest.get(key) != value:
            raise BundledResolverError("bundle_manifest_identity_mismatch")
    observed = _sha256(bundle_path)
    if manifest.get("bundle_sha256") != observed:
        raise BundledResolverError("bundle_digest_mismatch")
    if not isinstance(manifest.get("provisioning_sha"), str) or len(manifest["provisioning_sha"]) != 40:
        raise BundledResolverError("bundle_provisioning_identity_invalid")
    if not isinstance(manifest.get("workflow_run_id"), int):
        raise BundledResolverError("bundle_provisioning_identity_invalid")


def _validate_bundle_heads(entry: Mapping[str, Any], bundle: Path) -> None:
    _run(["git", "bundle", "verify", os.fspath(bundle)], cwd=ROOT)
    heads = _run(["git", "bundle", "list-heads", os.fspath(bundle)], cwd=ROOT).splitlines()
    if len(heads) != 1 or heads[0].split(maxsplit=1)[0] != entry["commit"]:
        raise BundledResolverError("bundle_heads_mismatch")


def _find_unique_local_store(
    name: str,
    entry: Mapping[str, Any],
) -> tuple[Path | None, int]:
    matches: list[Path] = []
    for candidate in repository_candidates(name):
        try:
            _verify_local_object_store(candidate, entry)
        except (CandidateMaterializationError, ResolverError):
            continue
        matches.append(candidate)
    if len(matches) > 1:
        raise BundledResolverError(f"{name}_object_store_ambiguous", **{f"{name}_matches": len(matches)})
    return (matches[0] if matches else None, len(matches))


def _clone_local_store(source: Path, destination: Path, entry: Mapping[str, Any]) -> None:
    _run(
        [
            "git",
            "-c",
            "protocol.file.allow=always",
            "clone",
            "--local",
            "--no-hardlinks",
            "--no-checkout",
            "--quiet",
            os.fspath(source),
            os.fspath(destination),
        ]
    )
    _finish_checkout(destination, entry)


def _clone_bundle(bundle: Path, destination: Path, entry: Mapping[str, Any]) -> None:
    _run(["git", "clone", "--no-checkout", "--quiet", os.fspath(bundle), os.fspath(destination)])
    _finish_checkout(destination, entry)
    _run_git(destination, ["fsck", "--full", "--strict"])


def _finish_checkout(destination: Path, entry: Mapping[str, Any]) -> None:
    commit = str(entry["commit"])
    observed = _run_git(destination, ["rev-parse", "--verify", f"{commit}^{{commit}}"])
    if observed != commit:
        raise BundledResolverError("materialized_commit_mismatch")
    _run_git(destination, ["checkout", "--detach", "--quiet", commit])
    _run_git(destination, ["remote", "set-url", "origin", str(entry["repository"])])
    if _run_git(destination, ["status", "--porcelain=v1", "--untracked-files=all"]):
        raise BundledResolverError("materialized_checkout_dirty")


def materialize_bundled_reference_root(parent: Path) -> tuple[Path, int, int]:
    try:
        materialize_parent = _prepare_materialization_parent(parent)
        lock = load_reference_lock(LOCK)
        entries = _locked_capture_entries(lock)
    except ResolverError as exc:
        raise BundledResolverError(exc.kind) from exc

    reghdfe, reghdfe_count = _find_unique_local_store("reghdfe", entries["reghdfe"])
    ftools, ftools_count = _find_unique_local_store("ftools", entries["ftools"])
    if reghdfe is None or ftools is None:
        raise BundledResolverError(
            "bundled_reference_local_store_incomplete",
            reghdfe_matches=reghdfe_count,
            ftools_matches=ftools_count,
        )

    manifest, bundle = _bundle_inventory(entries[BUNDLED_COMPONENT])
    _validate_bundle_manifest(entries[BUNDLED_COMPONENT], manifest, bundle)
    _validate_bundle_heads(entries[BUNDLED_COMPONENT], bundle)

    target = Path(tempfile.mkdtemp(prefix=MATERIALIZED_PREFIX, dir=materialize_parent))
    target.chmod(stat.S_IRWXU)
    try:
        _clone_local_store(reghdfe, target / "reghdfe", entries["reghdfe"])
        _clone_local_store(ftools, target / "ftools", entries["ftools"])
        _clone_bundle(bundle, target / "require", entries["require"])
        preflight_reference_root(target, lock, ROOT)
        return target, reghdfe_count, ftools_count
    except Exception:
        shutil.rmtree(target, ignore_errors=True)
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--materialize-parent", type=Path, required=True)
    args = parser.parse_args()
    try:
        selected, reghdfe_count, ftools_count = materialize_bundled_reference_root(
            args.materialize_parent
        )
        write_private_path(args.output, selected)
    except BundledResolverError as exc:
        print(
            "BUNDLED_REFERENCE_ROOT "
            f"status=failure kind={exc.kind} "
            f"reghdfe_object_stores={exc.reghdfe_matches} "
            f"ftools_object_stores={exc.ftools_matches} "
            f"bundle_present={int(exc.bundle_present)}"
        )
        return 1
    except Exception:
        print("BUNDLED_REFERENCE_ROOT status=failure kind=preflight_or_materialization_failed")
        return 1
    print(
        "BUNDLED_REFERENCE_ROOT "
        f"status=success reghdfe_object_stores={reghdfe_count} "
        f"ftools_object_stores={ftools_count} bundle_present=1"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
