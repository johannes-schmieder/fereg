#!/usr/bin/env python3
"""Materialize the exact lock-pinned Stata reference root from Git bundles.

All three public reference repositories are carried as digest-bound,
full-history Git bundles in this private repository. Capture is offline-only:
this resolver never searches the machine, never fetches, and never prints a
selected path. It clones each bundle into runner-temporary storage, restores
the public origin, and applies the unchanged clean-checkout and locked-file
preflight.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
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
    ResolverError,
    _locked_capture_entries,
    _prepare_materialization_parent,
    write_private_path,
)

COMPONENTS = ("reghdfe", "ftools", "require")
BUNDLE_ROOT = ROOT / "ci/reference-bundles/stata-require"
MATERIALIZED_PREFIX = "stfixest-bundled-reference-"
HEX_COMMIT = re.compile(r"^[0-9a-f]{40}$")
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")


class BundledResolverError(RuntimeError):
    def __init__(self, kind: str, *, component: str = "none") -> None:
        super().__init__(kind)
        self.kind = kind
        self.component = component if component in {*COMPONENTS, "none"} else "invalid"


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


def _run(arguments: Sequence[str], *, cwd: Path | None = None, component: str) -> str:
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
        raise BundledResolverError("local_git_command_failed", component=component)
    return completed.stdout.strip()


def _run_git(checkout: Path, arguments: Sequence[str], *, component: str) -> str:
    return _run(
        ["git", "-C", os.fspath(checkout), *arguments],
        component=component,
    )


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _required_file_hashes(
    entry: Mapping[str, Any],
    *,
    component: str,
) -> dict[str, str]:
    raw_files = entry.get("required_capture_files")
    raw_hashes = entry.get("required_capture_sha256")
    if (
        not isinstance(raw_files, list)
        or not raw_files
        or not all(isinstance(value, str) and value for value in raw_files)
        or len(set(raw_files)) != len(raw_files)
        or not isinstance(raw_hashes, Mapping)
        or set(raw_hashes) != set(raw_files)
    ):
        raise BundledResolverError("bundle_lock_files_invalid", component=component)
    output: dict[str, str] = {}
    for path in raw_files:
        digest = raw_hashes.get(path)
        if not isinstance(digest, str) or not HEX_SHA256.fullmatch(digest):
            raise BundledResolverError("bundle_lock_files_invalid", component=component)
        output[path] = digest
    return output


def _bundle_inventory(
    component: str,
    entry: Mapping[str, Any],
) -> tuple[Path, Path]:
    commit = str(entry["commit"])
    directory = BUNDLE_ROOT / component / commit
    manifest = directory / "manifest.json"
    bundle = directory / f"{component}-{commit}.bundle"
    for path in (BUNDLE_ROOT, directory, manifest, bundle):
        if path.is_symlink():
            raise BundledResolverError("bundle_inventory_symlink", component=component)
    if not directory.is_dir() or not manifest.is_file() or not bundle.is_file():
        raise BundledResolverError("bundle_inventory_absent", component=component)
    return manifest, bundle


def _validate_bundle_manifest(
    component: str,
    entry: Mapping[str, Any],
    manifest_path: Path,
    bundle_path: Path,
) -> str:
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise BundledResolverError("bundle_manifest_invalid", component=component) from exc
    expected = {
        "schema": 2,
        "status": "success",
        "component": component,
        "source_repository": entry["repository"],
        "source_commit": entry["commit"],
        "checkout_dir": entry["checkout_dir"],
        "required_files": _required_file_hashes(entry, component=component),
        "bundle_file": bundle_path.name,
        "provisioning_repository": "johannes-schmieder/StFixest",
        "provisioning_network_policy": "network-allowed-public-source-only",
        "capture_network_policy": "offline-only",
    }
    for key, value in expected.items():
        if manifest.get(key) != value:
            raise BundledResolverError(
                "bundle_manifest_identity_mismatch",
                component=component,
            )
    if manifest.get("bundle_sha256") != _sha256(bundle_path):
        raise BundledResolverError("bundle_digest_mismatch", component=component)
    source_sha = manifest.get("provisioning_source_sha")
    if not isinstance(source_sha, str) or not HEX_COMMIT.fullmatch(source_sha):
        raise BundledResolverError(
            "bundle_provisioning_identity_invalid",
            component=component,
        )
    if not isinstance(manifest.get("workflow_run_id"), int) or not isinstance(
        manifest.get("workflow_run_attempt"), int
    ):
        raise BundledResolverError(
            "bundle_provisioning_identity_invalid",
            component=component,
        )
    head_ref = manifest.get("bundle_head_ref")
    if head_ref != f"refs/heads/stfixest-pin-{component}":
        raise BundledResolverError("bundle_head_identity_mismatch", component=component)
    return head_ref


def _validate_bundle_heads(
    component: str,
    entry: Mapping[str, Any],
    bundle: Path,
    head_ref: str,
) -> None:
    heads = _run(
        ["git", "bundle", "list-heads", os.fspath(bundle)],
        cwd=ROOT,
        component=component,
    ).splitlines()
    if len(heads) != 1:
        raise BundledResolverError("bundle_heads_mismatch", component=component)
    pieces = heads[0].split()
    if len(pieces) != 2 or pieces[0] != entry["commit"] or pieces[1] != head_ref:
        raise BundledResolverError("bundle_heads_mismatch", component=component)


def _finish_checkout(
    destination: Path,
    entry: Mapping[str, Any],
    *,
    component: str,
) -> None:
    commit = str(entry["commit"])
    observed = _run_git(
        destination,
        ["rev-parse", "--verify", f"{commit}^{{commit}}"],
        component=component,
    )
    if observed != commit:
        raise BundledResolverError("materialized_commit_mismatch", component=component)
    _run_git(
        destination,
        ["checkout", "--detach", "--quiet", commit],
        component=component,
    )
    _run_git(
        destination,
        ["remote", "set-url", "origin", str(entry["repository"])],
        component=component,
    )
    if _run_git(
        destination,
        ["status", "--porcelain=v1", "--untracked-files=all"],
        component=component,
    ):
        raise BundledResolverError("materialized_checkout_dirty", component=component)


def _clone_bundle(
    component: str,
    bundle: Path,
    destination: Path,
    entry: Mapping[str, Any],
) -> None:
    _run(
        ["git", "clone", "--no-checkout", "--quiet", os.fspath(bundle), os.fspath(destination)],
        component=component,
    )
    _finish_checkout(destination, entry, component=component)
    _run_git(destination, ["fsck", "--full", "--strict"], component=component)


def materialize_bundled_reference_root(parent: Path) -> tuple[Path, int]:
    try:
        materialize_parent = _prepare_materialization_parent(parent)
        lock = load_reference_lock(LOCK)
        entries = _locked_capture_entries(lock)
    except ResolverError as exc:
        raise BundledResolverError(exc.kind) from exc

    inventory: dict[str, tuple[Path, Mapping[str, Any]]] = {}
    for component in COMPONENTS:
        entry = entries[component]
        if not isinstance(entry.get("commit"), str) or not HEX_COMMIT.fullmatch(
            str(entry["commit"])
        ):
            raise BundledResolverError("reference_commit_invalid", component=component)
        manifest, bundle = _bundle_inventory(component, entry)
        head_ref = _validate_bundle_manifest(component, entry, manifest, bundle)
        _validate_bundle_heads(component, entry, bundle, head_ref)
        inventory[component] = (bundle, entry)

    target = Path(tempfile.mkdtemp(prefix=MATERIALIZED_PREFIX, dir=materialize_parent))
    target.chmod(stat.S_IRWXU)
    try:
        for component in COMPONENTS:
            bundle, entry = inventory[component]
            _clone_bundle(component, bundle, target / component, entry)
        preflight_reference_root(target, lock, ROOT)
        return target, len(inventory)
    except Exception:
        shutil.rmtree(target, ignore_errors=True)
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--materialize-parent", type=Path, required=True)
    args = parser.parse_args()
    try:
        selected, present = materialize_bundled_reference_root(args.materialize_parent)
        write_private_path(args.output, selected)
    except BundledResolverError as exc:
        print(
            "BUNDLED_REFERENCE_ROOT "
            f"status=failure kind={exc.kind} component={exc.component} bundles_present=0"
        )
        return 1
    except Exception:
        print(
            "BUNDLED_REFERENCE_ROOT status=failure "
            "kind=preflight_or_materialization_failed component=none bundles_present=0"
        )
        return 1
    print(
        "BUNDLED_REFERENCE_ROOT "
        f"status=success components={','.join(COMPONENTS)} bundles_present={present}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
