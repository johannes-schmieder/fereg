#!/usr/bin/env python3
"""Resolve an offline, lock-pinned Stata reference root without enumeration.

Only a finite allowlist of conventional locations is tested. A candidate is
accepted solely when the existing reference preflight certifies all three clean
non-symlink direct-child checkouts, exact commits, trees, and required file
hashes. The selected absolute path is written privately and is never printed.
"""
from __future__ import annotations

import argparse
import os
import stat
import sys
from pathlib import Path

from ols_reference import load_reference_lock, preflight_reference_root

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "references.lock.toml"

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
    def __init__(self, kind: str) -> None:
        super().__init__(kind)
        self.kind = kind


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
            raise ResolverError("explicit_reference_root_invalid")
        return ([Path(value).expanduser()], True)

    home = Path.home()
    candidates = [home / suffix for suffix in CANDIDATE_SUFFIXES]
    candidates.extend(SHARED_CANDIDATES)
    return (_deduplicate(candidates), False)


def resolve_reference_root() -> tuple[Path, int, int]:
    candidates, explicit = candidate_roots()
    lock = load_reference_lock(LOCK)
    valid: list[tuple[int, Path]] = []
    for index, candidate in enumerate(candidates, start=1):
        try:
            if not candidate.is_absolute() or not candidate.is_dir() or candidate.is_symlink():
                continue
            preflight_reference_root(candidate, lock, ROOT)
        except Exception:
            # Candidate details and preflight errors may contain private paths.
            continue
        valid.append((index, candidate))

    if len(valid) == 1:
        index, selected = valid[0]
        return selected, index, len(candidates)
    if explicit:
        raise ResolverError("explicit_reference_root_failed_preflight")
    if not valid:
        raise ResolverError("offline_reference_root_unresolved")
    raise ResolverError("offline_reference_root_ambiguous")


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
    args = parser.parse_args()
    try:
        selected, index, checked = resolve_reference_root()
        write_private_path(args.output, selected)
    except ResolverError as exc:
        print(f"REFERENCE_ROOT_RESOLVER status=failure kind={exc.kind}")
        return 1
    print(
        "REFERENCE_ROOT_RESOLVER "
        f"status=success candidate_index={index} candidates_checked={checked}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
