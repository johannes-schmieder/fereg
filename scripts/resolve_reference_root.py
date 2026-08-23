#!/usr/bin/env python3
"""Resolve an offline, lock-pinned Stata reference root without enumeration.

Only a finite allowlist of conventional locations is tested. An existing root
is accepted solely when the reference preflight certifies all three clean,
non-symlink direct-child checkouts at the exact commits and file hashes. When
an allowlisted root contains canonical local Git object stores but the working
checkouts are stale or dirty, the resolver may create clean temporary local
clones under an explicitly supplied runner-temporary parent. It never fetches,
never scans the home directory, and never prints a selected absolute path.
"""
from __future__ import annotations

import argparse
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Mapping, Sequence

from ols_reference import load_reference_lock, preflight_reference_root

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "references.lock.toml"
REQUIRED_CHILDREN = ("reghdfe", "ftools", "require")
MATERIALIZED_PREFIX = "stfixest-reference-materialized-"

# Finite conventional roots only. Do not replace this with filesystem search.
CANDIDATE_SUFFIXES = (
    "Git",
    "Git/StFixest-references",
    "Git/stfixest-references",
    "Git/references",
    "Git/References",
    "Git/StFixest/references",
    "Git/StFixest/docs/references",
    "Git/stfixest/docs/references",
    "Git/Vault/references",
    "git",
    "git/StFixest-references",
    "git/stfixest-references",
    "git/references",
    "Projects/StFixest-references",
    "Projects/stfixest-references",
    "Projects/references",
    "src/StFixest-references",
    "src/stfixest-references",
    "src/references",
)
SHARED_CANDIDATES = (
    Path("/Users/Shared/StFixest-references"),
    Path("/Users/Shared/stfixest-references"),
    Path("/Users/Shared/references"),
)


class ResolverError(RuntimeError):
    def __init__(
        self,
        kind: str,
        *,
        checked: int = 0,
        existing: int = 0,
        layouts: int = 0,
        preflight: int = 0,
        materialization_attempts: int = 0,
    ) -> None:
        super().__init__(kind)
        self.kind = kind
        self.checked = checked
        self.existing = existing
        self.layouts = layouts
        self.preflight = preflight
        self.materialization_attempts = materialization_attempts


class CandidateMaterializationError(RuntimeError):
    """One allowlisted local object-store candidate cannot be materialized."""


def _deduplicate(candidates: list[Path]) -> list[Path]:
    output: list[Path] = []
    seen: set[str] = set()
    for candidate in candidates:
        key = os.path.abspath(os.fspath(candidate))
        if key in seen:
            continue
        seen.add(key)
        output.append(Path(key))
    return output


def candidate_roots() -> tuple[list[Path], bool]:
    if "STFIXEST_REFERENCE_ROOT" in os.environ:
        value = os.environ["STFIXEST_REFERENCE_ROOT"]
        if not value or not Path(value).expanduser().is_absolute():
            raise ResolverError("explicit_reference_root_invalid", checked=1)
        return ([Path(value).expanduser()], True)

    home = Path.home()
    candidates = [home / suffix for suffix in CANDIDATE_SUFFIXES]
    candidates.extend(SHARED_CANDIDATES)
    return (_deduplicate(candidates), False)


def _has_direct_child_layout(candidate: Path) -> bool:
    return all(
        (candidate / child).is_dir() and not (candidate / child).is_symlink()
        for child in REQUIRED_CHILDREN
    )


def _locked_capture_entries(lock: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    entries: dict[str, Mapping[str, Any]] = {}
    raw_entries = lock.get("reference", [])
    if not isinstance(raw_entries, list):
        raise ResolverError("reference_lock_inventory_invalid")
    for raw in raw_entries:
        if not isinstance(raw, Mapping):
            continue
        name = raw.get("name")
        if name in REQUIRED_CHILDREN:
            if name in entries:
                raise ResolverError("reference_lock_inventory_invalid")
            entries[name] = raw
    if set(entries) != set(REQUIRED_CHILDREN):
        raise ResolverError("reference_lock_inventory_invalid")
    for name, entry in entries.items():
        checkout_dir = entry.get("checkout_dir")
        repository = entry.get("repository")
        commit = entry.get("commit")
        if checkout_dir != name or not isinstance(repository, str) or not isinstance(commit, str):
            raise ResolverError("reference_lock_inventory_invalid")
    return entries


def _github_repository_identity(value: str) -> str | None:
    raw = value.strip()
    prefixes = (
        "https://github.com/",
        "http://github.com/",
        "ssh://git@github.com/",
        "git@github.com:",
    )
    path = None
    for prefix in prefixes:
        if raw.startswith(prefix):
            path = raw[len(prefix) :]
            break
    if path is None:
        return None
    path = path.rstrip("/")
    if path.endswith(".git"):
        path = path[:-4]
    pieces = path.split("/")
    if len(pieces) != 2 or not all(pieces):
        return None
    return "/".join(pieces).lower()


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


def _run_command(arguments: Sequence[str]) -> str:
    completed = subprocess.run(
        list(arguments),
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=_git_environment(),
    )
    if completed.returncode:
        raise CandidateMaterializationError("local_git_command_failed")
    return completed.stdout.strip()


def _run_git(checkout: Path, arguments: Sequence[str]) -> str:
    return _run_command(["git", "-C", os.fspath(checkout), *arguments])


def _verify_local_object_store(
    checkout: Path,
    entry: Mapping[str, Any],
) -> None:
    if checkout.is_symlink() or not checkout.is_dir():
        raise CandidateMaterializationError("local_object_store_absent")
    resolved = checkout.resolve()
    top = Path(_run_git(checkout, ["rev-parse", "--show-toplevel"])).resolve()
    if top != resolved:
        raise CandidateMaterializationError("local_object_store_not_checkout_root")
    observed_repository = _github_repository_identity(
        _run_git(checkout, ["remote", "get-url", "origin"])
    )
    expected_repository = _github_repository_identity(str(entry["repository"]))
    if observed_repository is None or observed_repository != expected_repository:
        raise CandidateMaterializationError("local_object_store_origin_mismatch")
    commit = str(entry["commit"])
    observed_commit = _run_git(
        checkout,
        ["rev-parse", "--verify", f"{commit}^{{commit}}"],
    )
    if observed_commit != commit:
        raise CandidateMaterializationError("local_object_store_missing_pin")


def _prepare_materialization_parent(parent: Path) -> Path:
    if not parent.is_absolute():
        raise ResolverError("materialization_parent_not_absolute")
    resolved = parent.resolve(strict=False)
    repository = ROOT.resolve()
    if (
        resolved == repository
        or repository in resolved.parents
        or resolved in repository.parents
    ):
        raise ResolverError("materialization_parent_overlaps_repository")
    parent.mkdir(mode=stat.S_IRWXU, parents=True, exist_ok=True)
    if parent.is_symlink() or not parent.is_dir():
        raise ResolverError("materialization_parent_invalid")
    parent.chmod(stat.S_IRWXU)
    return parent.resolve()


def _materialize_candidate(
    candidate: Path,
    entries: Mapping[str, Mapping[str, Any]],
    lock: Mapping[str, Any],
    parent: Path,
) -> Path:
    target = Path(tempfile.mkdtemp(prefix=MATERIALIZED_PREFIX, dir=parent))
    target.chmod(stat.S_IRWXU)
    try:
        for name in REQUIRED_CHILDREN:
            entry = entries[name]
            source = candidate / str(entry["checkout_dir"])
            _verify_local_object_store(source, entry)
            destination = target / str(entry["checkout_dir"])
            _run_command(
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
            commit = str(entry["commit"])
            _run_git(destination, ["checkout", "--detach", "--quiet", commit])
            _run_git(
                destination,
                ["remote", "set-url", "origin", str(entry["repository"])],
            )
        preflight_reference_root(target, lock, ROOT)
        return target
    except Exception as exc:
        shutil.rmtree(target, ignore_errors=True)
        if isinstance(exc, CandidateMaterializationError):
            raise
        raise CandidateMaterializationError("materialized_preflight_failed") from exc


def resolve_reference_root(
    materialize_parent: Path | None = None,
) -> tuple[Path, int, int, int, int, bool, int]:
    candidates, explicit = candidate_roots()
    lock = load_reference_lock(LOCK)
    entries = _locked_capture_entries(lock)
    valid: list[tuple[int, Path]] = []
    existing = 0
    layouts = 0
    preflight = 0
    layout_candidates: list[tuple[int, Path]] = []
    for index, candidate in enumerate(candidates, start=1):
        if not candidate.is_absolute() or not candidate.is_dir() or candidate.is_symlink():
            continue
        existing += 1
        if not _has_direct_child_layout(candidate):
            continue
        layouts += 1
        layout_candidates.append((index, candidate))
        try:
            preflight_reference_root(candidate, lock, ROOT)
        except Exception:
            # Candidate details and preflight errors may contain private paths.
            continue
        preflight += 1
        valid.append((index, candidate))

    checked = len(candidates)
    if len(valid) == 1:
        index, selected = valid[0]
        return selected, index, checked, existing, layouts, False, 0
    if len(valid) > 1:
        raise ResolverError(
            "offline_reference_root_ambiguous",
            checked=checked,
            existing=existing,
            layouts=layouts,
            preflight=preflight,
        )

    materialization_attempts = 0
    if materialize_parent is not None:
        parent = _prepare_materialization_parent(materialize_parent)
        for index, candidate in layout_candidates:
            materialization_attempts += 1
            try:
                selected = _materialize_candidate(candidate, entries, lock, parent)
            except CandidateMaterializationError:
                continue
            return (
                selected,
                index,
                checked,
                existing,
                layouts,
                True,
                materialization_attempts,
            )

    if explicit:
        kind = (
            "explicit_reference_root_materialization_failed"
            if materialization_attempts
            else "explicit_reference_root_failed_preflight"
        )
        raise ResolverError(
            kind,
            checked=checked,
            existing=existing,
            layouts=layouts,
            preflight=preflight,
            materialization_attempts=materialization_attempts,
        )
    if existing == 0:
        kind = "offline_reference_root_not_found"
    elif layouts == 0:
        kind = "offline_reference_layout_absent"
    elif materialization_attempts:
        kind = "offline_reference_materialization_failed"
    elif preflight == 0:
        kind = "offline_reference_preflight_failed"
    else:
        kind = "offline_reference_root_ambiguous"
    raise ResolverError(
        kind,
        checked=checked,
        existing=existing,
        layouts=layouts,
        preflight=preflight,
        materialization_attempts=materialization_attempts,
    )


def write_private_path(output: Path, selected: Path) -> None:
    if not output.is_absolute():
        raise ResolverError("output_path_not_absolute")
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(output.name + ".tmp")
    try:
        temporary.unlink(missing_ok=True)
        descriptor = os.open(
            temporary,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL,
            stat.S_IRUSR | stat.S_IWUSR,
        )
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(os.path.abspath(os.fspath(selected)) + "\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, output)
        output.chmod(stat.S_IRUSR | stat.S_IWUSR)
    finally:
        temporary.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--materialize-parent", type=Path)
    args = parser.parse_args()
    try:
        (
            selected,
            index,
            checked,
            existing,
            layouts,
            materialized,
            materialization_attempts,
        ) = resolve_reference_root(args.materialize_parent)
        write_private_path(args.output, selected)
    except ResolverError as exc:
        print(
            "REFERENCE_ROOT_RESOLVER "
            f"status=failure kind={exc.kind} candidates_checked={exc.checked} "
            f"existing_candidates={exc.existing} layout_candidates={exc.layouts} "
            f"preflight_candidates={exc.preflight} "
            f"materialization_attempts={exc.materialization_attempts}"
        )
        return 1
    print(
        "REFERENCE_ROOT_RESOLVER "
        f"status=success candidate_index={index} candidates_checked={checked} "
        f"existing_candidates={existing} layout_candidates={layouts} "
        f"preflight_candidates={int(not materialized)} "
        f"materialized={int(materialized)} "
        f"materialization_attempts={materialization_attempts}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
