"""Offline identity verification for pinned scientific reference packages."""
from __future__ import annotations
import hashlib
import re
import subprocess
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence

ROOT = Path(__file__).resolve().parents[1]

DEFAULT_LOCK = ROOT / "references.lock.toml"

HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")

HEX_COMMIT = re.compile(r"^[0-9a-f]{40}$")

SAFE_ID = re.compile(r"^[a-z][a-z0-9_]{1,63}$")

REFERENCE_NAMES = ("reghdfe", "ftools", "require")

class ContractError(ValueError):
    """The frozen manifest, raw capture, or receipt violates its schema."""

class PreflightError(RuntimeError):
    """A local reference or input cannot be proved immutable and current."""

@dataclass(frozen=True)
class ReferenceIdentity:
    name: str
    commit: str
    tree: str
    checkout: Path
    ado_dir: Path

    def receipt_value(self) -> dict[str, str]:
        return {"commit": self.commit, "git_tree": self.tree}

def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()

def load_reference_lock(path: Path = DEFAULT_LOCK) -> dict[str, Any]:
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise ContractError(f"cannot read reference lock {path}: {exc}") from exc
    if value.get("schema_version") not in {1, 2}:
        raise ContractError("unsupported references.lock.toml schema")
    return value

def _reference_by_name(lock: Mapping[str, Any], name: str) -> Mapping[str, Any]:
    matches = [entry for entry in lock.get("reference", []) if entry.get("name") == name]
    if len(matches) != 1:
        raise PreflightError(f"reference lock must contain exactly one {name!r} entry")
    return matches[0]

def _git(checkout: Path, arguments: Sequence[str], runner: Callable[..., subprocess.CompletedProcess[str]]) -> str:
    completed = runner(
        ["git", "-C", str(checkout), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.returncode:
        raise PreflightError(f"git {' '.join(arguments)} failed for isolated {checkout.name} checkout")
    return completed.stdout.strip()

def preflight_reference_root(
    reference_root: Path,
    lock: Mapping[str, Any],
    repository_root: Path = ROOT,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> dict[str, ReferenceIdentity]:
    """Require the full exact, clean, hash-bound reference dependency closure."""
    if not reference_root.is_absolute():
        raise PreflightError("--reference-root must be an absolute path")
    if reference_root.is_symlink() or not reference_root.is_dir():
        raise PreflightError("--reference-root must be a real directory, not a symlink")
    resolved_root = reference_root.resolve()
    resolved_repository = repository_root.resolve()
    if resolved_root == resolved_repository or resolved_repository in resolved_root.parents or resolved_root in resolved_repository.parents:
        raise PreflightError("reference checkouts must be outside the fereg repository")

    entries: dict[str, Mapping[str, Any]] = {}
    for name in REFERENCE_NAMES:
        entry = _reference_by_name(lock, name)
        commit = entry.get("commit")
        if entry.get("pin_status") != "resolved" or not isinstance(commit, str) or not HEX_COMMIT.fullmatch(commit):
            raise PreflightError(f"{name} exact pin is unresolved; update references.lock.toml from adjudicated provenance before capture")
        entries[name] = entry

    identities: dict[str, ReferenceIdentity] = {}
    for name in REFERENCE_NAMES:
        entry = entries[name]
        commit = entry["commit"]
        checkout_name = entry.get("checkout_dir")
        ado_subdir = entry.get("capture_ado_subdir")
        required_files = entry.get("required_capture_files")
        required_hashes = entry.get("required_capture_sha256")
        if not isinstance(checkout_name, str) or not SAFE_ID.fullmatch(checkout_name):
            raise PreflightError(f"{name} checkout_dir is not locked")
        if not isinstance(ado_subdir, str) or Path(ado_subdir).is_absolute() or ".." in Path(ado_subdir).parts:
            raise PreflightError(f"{name} capture_ado_subdir is not safe")
        if not isinstance(required_files, list) or not required_files:
            raise PreflightError(f"{name} required_capture_files are not locked")
        if not isinstance(required_hashes, Mapping) or set(required_hashes) != set(required_files) or not all(
            isinstance(value, str) and HEX_SHA256.fullmatch(value) for value in required_hashes.values()
        ):
            raise PreflightError(f"{name} required_capture_sha256 is not a closed exact file inventory")
        checkout = resolved_root / checkout_name
        if checkout.is_symlink() or not checkout.is_dir() or checkout.resolve() != checkout:
            raise PreflightError(f"{name} must be a non-symlink direct child of --reference-root")
        top = Path(_git(checkout, ["rev-parse", "--show-toplevel"], runner)).resolve()
        if top != checkout:
            raise PreflightError(f"{name} path is not the root of its isolated git checkout")
        observed_commit = _git(checkout, ["rev-parse", "HEAD"], runner)
        if observed_commit != commit:
            raise PreflightError(f"{name} checkout is stale: expected {commit}, observed {observed_commit}")
        if _git(checkout, ["status", "--porcelain=v1", "--untracked-files=all"], runner):
            raise PreflightError(f"{name} checkout is mutable or dirty")
        tree = _git(checkout, ["rev-parse", "HEAD^{tree}"], runner)
        if not HEX_COMMIT.fullmatch(tree):
            raise PreflightError(f"{name} git tree identity is malformed")
        for raw in required_files:
            if not isinstance(raw, str) or Path(raw).is_absolute() or ".." in Path(raw).parts:
                raise PreflightError(f"{name} has an unsafe required file")
            required = checkout / raw
            if required.is_symlink() or not required.is_file():
                raise PreflightError(f"{name} required tracked file is missing: {raw}")
            _git(checkout, ["ls-files", "--error-unmatch", "--", raw], runner)
            observed_sha = sha256_file(required)
            if observed_sha != required_hashes[raw]:
                raise PreflightError(
                    f"{name} required tracked file hash mismatch: {raw}; expected {required_hashes[raw]}, observed {observed_sha}"
                )
        ado_dir = checkout / ado_subdir
        if not ado_dir.is_dir():
            raise PreflightError(f"{name} ado directory is missing: {ado_subdir}")
        identities[name] = ReferenceIdentity(name, commit, tree, checkout, ado_dir)
    return identities
