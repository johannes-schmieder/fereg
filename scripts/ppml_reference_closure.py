#!/usr/bin/env python3
"""Build and verify the exact offline Stata comparator closure for feppml.

The command never searches or changes PLUS/PERSONAL.  ``assemble`` accepts the
existing three-component OLS reference root plus the digest-bound ppmlhdfe
source archive, stages only each locked ``src`` subtree outside this repository,
and emits a deterministic runtime tar and sealed receipt.  ``verify`` and
``materialize`` are network-free and are suitable for the SCC deployment edge.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Mapping, Sequence

from ols_reference import (
    PreflightError,
    ReferenceIdentity,
    load_reference_lock,
    preflight_reference_root,
)


ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "references.lock.toml"
COMPONENTS = ("ppmlhdfe", "reghdfe", "ftools", "require")
OLS_COMPONENTS = ("reghdfe", "ftools", "require")
RECEIPT_NAME = "feppml-comparator-closure-v1.json"
BUNDLE_NAME = "feppml-comparator-runtime-v1.tar"
RECEIPT_TYPE = "fereg-feppml-comparator-closure-v1"
BUNDLE_FORMAT = "ustar-normalized-runtime-v1"
HEX_SHA1 = re.compile(r"^[0-9a-f]{40}$")
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
SAFE_COMPONENT = re.compile(r"^[a-z][a-z0-9_]{1,31}$")
MAX_ARCHIVE_MEMBERS = 20_000
MAX_ARCHIVE_FILE_BYTES = 512 * 1024 * 1024
MAX_ARCHIVE_TOTAL_BYTES = 2 * 1024 * 1024 * 1024


class ClosureError(RuntimeError):
    """A lock, source, bundle, receipt, or destination failed closed."""

    def __init__(self, kind: str, *, component: str = "none") -> None:
        super().__init__(kind)
        self.kind = kind
        self.component = component if component in {*COMPONENTS, "none"} else "none"


@dataclass(frozen=True)
class SourceArchive:
    sha256: str
    size_bytes: int


def canonical_json_bytes(value: Any) -> bytes:
    try:
        encoded = json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
    except (TypeError, ValueError) as error:
        raise ClosureError("receipt_not_canonical") from error
    return (encoded + "\n").encode("ascii")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1 << 20), b""):
                digest.update(block)
    except OSError as error:
        raise ClosureError("file_unreadable") from error
    return digest.hexdigest()


def _entry_by_name(lock: Mapping[str, Any], name: str) -> Mapping[str, Any]:
    raw = lock.get("reference")
    if not isinstance(raw, list):
        raise ClosureError("reference_lock_inventory_invalid", component=name)
    matches = [item for item in raw if isinstance(item, Mapping) and item.get("name") == name]
    if len(matches) != 1:
        raise ClosureError("reference_lock_inventory_invalid", component=name)
    return matches[0]


def _safe_relative(raw: Any, *, component: str) -> str:
    if not isinstance(raw, str) or not raw or "\\" in raw:
        raise ClosureError("reference_lock_path_invalid", component=component)
    path = PurePosixPath(raw)
    if path.is_absolute() or any(piece in {"", ".", ".."} for piece in path.parts):
        raise ClosureError("reference_lock_path_invalid", component=component)
    return path.as_posix()


def locked_entries(lock: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    """Return the closed four-component lock inventory after strict validation."""
    entries: dict[str, Mapping[str, Any]] = {}
    for name in COMPONENTS:
        entry = _entry_by_name(lock, name)
        commit = entry.get("commit")
        tree = entry.get("git_tree")
        checkout = entry.get("checkout_dir")
        ado_subdir = entry.get("capture_ado_subdir")
        files = entry.get("required_capture_files")
        hashes = entry.get("required_capture_sha256")
        archive_format = entry.get("source_archive_format")
        archive_scope = entry.get("source_archive_scope")
        archive_size = entry.get("source_archive_size_bytes")
        archive_sha256 = entry.get("source_archive_sha256")
        if (
            entry.get("pin_status") != "resolved"
            or not isinstance(commit, str)
            or HEX_SHA1.fullmatch(commit) is None
            or not isinstance(tree, str)
            or HEX_SHA1.fullmatch(tree) is None
            or checkout != name
            or SAFE_COMPONENT.fullmatch(str(checkout)) is None
            or archive_scope != ado_subdir
            or isinstance(archive_size, bool)
            or not isinstance(archive_size, int)
            or archive_size <= 0
            or not isinstance(archive_sha256, str)
            or HEX_SHA256.fullmatch(archive_sha256) is None
            or (
                name != "ppmlhdfe"
                and archive_format != "git-archive-tar"
            )
        ):
            raise ClosureError("reference_lock_identity_invalid", component=name)
        safe_ado = _safe_relative(ado_subdir, component=name)
        if not isinstance(files, list) or not files or len(set(files)) != len(files):
            raise ClosureError("reference_lock_files_invalid", component=name)
        if not isinstance(hashes, Mapping) or set(hashes) != set(files):
            raise ClosureError("reference_lock_files_invalid", component=name)
        for raw in files:
            safe = _safe_relative(raw, component=name)
            if safe != raw or not (safe == safe_ado or safe.startswith(safe_ado + "/")):
                raise ClosureError("reference_lock_files_invalid", component=name)
            digest = hashes.get(raw)
            if not isinstance(digest, str) or HEX_SHA256.fullmatch(digest) is None:
                raise ClosureError("reference_lock_files_invalid", component=name)
        entries[name] = entry

    ppml = entries["ppmlhdfe"]
    expected_root = f"ppmlhdfe-{ppml['commit']}"
    if (
        ppml.get("source_archive_format") != "tar.gz"
        or ppml.get("source_archive_root") != expected_root
        or not isinstance(ppml.get("source_archive_url"), str)
        or not str(ppml["source_archive_url"]).endswith("/" + str(ppml["commit"]))
        or not isinstance(ppml.get("source_archive_size_bytes"), int)
        or isinstance(ppml.get("source_archive_size_bytes"), bool)
        or int(ppml["source_archive_size_bytes"]) <= 0
        or not isinstance(ppml.get("source_archive_sha256"), str)
        or HEX_SHA256.fullmatch(str(ppml["source_archive_sha256"])) is None
    ):
        raise ClosureError("ppmlhdfe_archive_lock_invalid", component="ppmlhdfe")
    return entries


def _archive_member_path(raw: str) -> PurePosixPath:
    if not raw or "\\" in raw or "\x00" in raw:
        raise ClosureError("archive_path_unsafe")
    path = PurePosixPath(raw.rstrip("/"))
    if path.is_absolute() or not path.parts or any(piece in {"", ".", ".."} for piece in path.parts):
        raise ClosureError("archive_path_unsafe")
    return path


def _extract_archive_bytes(
    data: bytes,
    destination: Path,
    *,
    source_root: str | None = None,
    subtree: str | None = None,
    allowed_roots: set[str] | None = None,
    allowed_files: set[str] | None = None,
) -> None:
    """Extract regular files/directories only, with traversal and collision checks."""
    try:
        archive = tarfile.open(fileobj=io.BytesIO(data), mode="r:*")
    except tarfile.TarError as error:
        raise ClosureError("archive_invalid") from error
    selected: list[tuple[tarfile.TarInfo, PurePosixPath]] = []
    observed: set[str] = set()
    observed_folded: set[str] = set()
    total = 0
    with archive:
        members = archive.getmembers()
        if not members or len(members) > MAX_ARCHIVE_MEMBERS:
            raise ClosureError("archive_inventory_invalid")
        for member in members:
            path = _archive_member_path(member.name)
            if not (member.isdir() or member.isfile()):
                raise ClosureError("archive_entry_unsafe")
            if member.size < 0 or member.size > MAX_ARCHIVE_FILE_BYTES:
                raise ClosureError("archive_entry_too_large")
            total += member.size
            if total > MAX_ARCHIVE_TOTAL_BYTES:
                raise ClosureError("archive_too_large")
            relative = path
            if source_root is not None:
                if path.parts[0] != source_root:
                    raise ClosureError("archive_root_mismatch")
                if len(path.parts) == 1:
                    continue
                relative = PurePosixPath(*path.parts[1:])
            if subtree is not None:
                subtree_path = PurePosixPath(subtree)
                if relative != subtree_path and subtree_path not in relative.parents:
                    continue
            if allowed_files is not None:
                key = relative.as_posix()
                if member.isfile() and key not in allowed_files:
                    continue
                if member.isdir() and not any(
                    allowed == key or allowed.startswith(key + "/")
                    for allowed in allowed_files
                ):
                    continue
            if allowed_roots is not None and relative.parts[0] not in allowed_roots:
                raise ClosureError("archive_component_unexpected")
            key = relative.as_posix()
            folded = key.casefold()
            if key in observed or folded in observed_folded:
                raise ClosureError("archive_path_collision")
            observed.add(key)
            observed_folded.add(folded)
            selected.append((member, relative))
        if not selected:
            raise ClosureError("archive_subtree_missing")
        destination.mkdir(parents=True, exist_ok=False)
        for member, relative in selected:
            target = destination.joinpath(*relative.parts)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
                target.chmod(stat.S_IRWXU)
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            source = archive.extractfile(member)
            if source is None:
                raise ClosureError("archive_file_unreadable")
            try:
                with target.open("xb") as output:
                    shutil.copyfileobj(source, output, length=1 << 20)
            except OSError as error:
                raise ClosureError("archive_extract_failed") from error
            target.chmod(stat.S_IRUSR | stat.S_IWUSR)


def _verify_required_files(component_root: Path, entry: Mapping[str, Any], name: str) -> None:
    for relative in entry["required_capture_files"]:
        path = component_root / relative
        if path.is_symlink() or not path.is_file():
            raise ClosureError("required_file_missing", component=name)
        if sha256_file(path) != entry["required_capture_sha256"][relative]:
            raise ClosureError("required_file_digest_mismatch", component=name)
    ado = component_root / entry["capture_ado_subdir"]
    if ado.is_symlink() or not ado.is_dir():
        raise ClosureError("ado_directory_missing", component=name)


def stage_ppmlhdfe_archive(
    archive_path: Path,
    entry: Mapping[str, Any],
    destination: Path,
) -> SourceArchive:
    if archive_path.is_symlink() or not archive_path.is_file():
        raise ClosureError("ppmlhdfe_archive_missing", component="ppmlhdfe")
    size = archive_path.stat().st_size
    digest = sha256_file(archive_path)
    if size != entry["source_archive_size_bytes"] or digest != entry["source_archive_sha256"]:
        raise ClosureError("ppmlhdfe_archive_digest_mismatch", component="ppmlhdfe")
    try:
        data = archive_path.read_bytes()
    except OSError as error:
        raise ClosureError("ppmlhdfe_archive_unreadable", component="ppmlhdfe") from error
    try:
        _extract_archive_bytes(
            data,
            destination,
            source_root=str(entry["source_archive_root"]),
            subtree=str(entry["capture_ado_subdir"]),
            allowed_files=set(entry["required_capture_files"]),
        )
        _verify_required_files(destination, entry, "ppmlhdfe")
    except ClosureError as error:
        if error.component == "none":
            error.component = "ppmlhdfe"
        raise
    return SourceArchive(digest, size)


def _git_archive(identity: ReferenceIdentity, entry: Mapping[str, Any]) -> bytes:
    environment = os.environ.copy()
    environment.update(
        {
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_TERMINAL_PROMPT": "0",
        }
    )
    completed = subprocess.run(
        [
            "git", "-C", os.fspath(identity.checkout), "archive", "--format=tar",
            identity.commit, "--", str(entry["capture_ado_subdir"]),
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment,
    )
    if completed.returncode or not completed.stdout:
        raise ClosureError("git_archive_failed", component=identity.name)
    return completed.stdout


def _stage_git_reference(
    identity: ReferenceIdentity,
    entry: Mapping[str, Any],
    destination: Path,
) -> SourceArchive:
    if identity.tree != entry["git_tree"]:
        raise ClosureError("git_tree_mismatch", component=identity.name)
    data = _git_archive(identity, entry)
    if (
        sha256_bytes(data) != entry["source_archive_sha256"]
        or len(data) != entry["source_archive_size_bytes"]
    ):
        raise ClosureError("git_archive_digest_mismatch", component=identity.name)
    try:
        _extract_archive_bytes(
            data,
            destination,
            allowed_files=set(entry["required_capture_files"]),
        )
        _verify_required_files(destination, entry, identity.name)
    except ClosureError as error:
        if error.component == "none":
            error.component = identity.name
        raise
    return SourceArchive(sha256_bytes(data), len(data))


def _runtime_paths(root: Path) -> list[Path]:
    paths: list[Path] = []
    for path in root.rglob("*"):
        if path.is_symlink() or not (path.is_dir() or path.is_file()):
            raise ClosureError("runtime_entry_unsafe")
        paths.append(path)
    return sorted(paths, key=lambda value: value.relative_to(root).as_posix())


def write_deterministic_bundle(runtime_root: Path, bundle_path: Path) -> SourceArchive:
    if bundle_path.exists() or bundle_path.is_symlink():
        raise ClosureError("bundle_destination_exists")
    with tarfile.open(bundle_path, mode="x:", format=tarfile.USTAR_FORMAT) as archive:
        for path in _runtime_paths(runtime_root):
            relative = path.relative_to(runtime_root).as_posix()
            info = tarfile.TarInfo(relative + ("/" if path.is_dir() else ""))
            info.uid = 0
            info.gid = 0
            info.uname = ""
            info.gname = ""
            info.mtime = 0
            if path.is_dir():
                info.type = tarfile.DIRTYPE
                info.mode = 0o755
                archive.addfile(info)
            else:
                info.type = tarfile.REGTYPE
                info.mode = 0o644
                info.size = path.stat().st_size
                with path.open("rb") as source:
                    archive.addfile(info, source)
    bundle_path.chmod(stat.S_IRUSR)
    return SourceArchive(sha256_file(bundle_path), bundle_path.stat().st_size)


def _receipt_document(
    *,
    lock_sha256: str,
    entries: Mapping[str, Mapping[str, Any]],
    sources: Mapping[str, SourceArchive],
    bundle: SourceArchive,
) -> dict[str, Any]:
    comparator = {
        "bundle_sha256": bundle.sha256,
        "bundle_size_bytes": bundle.size_bytes,
        "references": {
            name: {
                "commit": entries[name]["commit"],
                "tree": entries[name]["git_tree"],
                "archive_sha256": sources[name].sha256,
                "archive_size_bytes": sources[name].size_bytes,
            }
            for name in COMPONENTS
        },
    }
    payload = {
        "bundle_format": BUNDLE_FORMAT,
        "capture_network_policy": "offline-only",
        "references_lock_sha256": lock_sha256,
        "comparator": comparator,
        "runtime": {
            name: {
                "ado_subdir": f"{name}/{entries[name]['capture_ado_subdir']}",
                "required_file_sha256": dict(entries[name]["required_capture_sha256"]),
            }
            for name in COMPONENTS
        },
    }
    unsigned = {
        "schema_version": 1,
        "receipt_type": RECEIPT_TYPE,
        "payload": payload,
    }
    return {**unsigned, "seal_sha256": sha256_bytes(canonical_json_bytes(unsigned))}


def _write_receipt(path: Path, document: Mapping[str, Any]) -> None:
    try:
        with path.open("xb") as output:
            output.write(canonical_json_bytes(document))
        path.chmod(stat.S_IRUSR)
    except OSError as error:
        raise ClosureError("receipt_write_failed") from error


def _expect_exact_keys(value: Mapping[str, Any], expected: set[str], kind: str) -> None:
    if set(value) != expected:
        raise ClosureError(kind)


def verify_receipt_document(
    document: Mapping[str, Any],
    bundle_path: Path,
    lock_sha256: str,
    entries: Mapping[str, Mapping[str, Any]],
) -> Mapping[str, Any]:
    _expect_exact_keys(document, {"schema_version", "receipt_type", "payload", "seal_sha256"}, "receipt_fields_invalid")
    if document["schema_version"] != 1 or document["receipt_type"] != RECEIPT_TYPE:
        raise ClosureError("receipt_identity_invalid")
    seal = document["seal_sha256"]
    if not isinstance(seal, str) or HEX_SHA256.fullmatch(seal) is None:
        raise ClosureError("receipt_seal_invalid")
    unsigned = {key: document[key] for key in ("schema_version", "receipt_type", "payload")}
    if sha256_bytes(canonical_json_bytes(unsigned)) != seal:
        raise ClosureError("receipt_seal_mismatch")
    payload = document["payload"]
    if not isinstance(payload, Mapping):
        raise ClosureError("receipt_payload_invalid")
    _expect_exact_keys(
        payload,
        {"bundle_format", "capture_network_policy", "references_lock_sha256", "comparator", "runtime"},
        "receipt_payload_fields_invalid",
    )
    if (
        payload["bundle_format"] != BUNDLE_FORMAT
        or payload["capture_network_policy"] != "offline-only"
        or payload["references_lock_sha256"] != lock_sha256
    ):
        raise ClosureError("receipt_policy_or_lock_mismatch")
    comparator = payload["comparator"]
    if not isinstance(comparator, Mapping):
        raise ClosureError("comparator_invalid")
    _expect_exact_keys(comparator, {"bundle_sha256", "bundle_size_bytes", "references"}, "comparator_fields_invalid")
    if bundle_path.is_symlink() or not bundle_path.is_file():
        raise ClosureError("bundle_missing")
    if (
        comparator["bundle_sha256"] != sha256_file(bundle_path)
        or comparator["bundle_size_bytes"] != bundle_path.stat().st_size
    ):
        raise ClosureError("bundle_identity_mismatch")
    references = comparator["references"]
    runtime = payload["runtime"]
    if not isinstance(references, Mapping) or set(references) != set(COMPONENTS):
        raise ClosureError("comparator_references_invalid")
    if not isinstance(runtime, Mapping) or set(runtime) != set(COMPONENTS):
        raise ClosureError("runtime_inventory_invalid")
    for name in COMPONENTS:
        record = references[name]
        if not isinstance(record, Mapping):
            raise ClosureError("comparator_reference_invalid", component=name)
        _expect_exact_keys(record, {"commit", "tree", "archive_sha256", "archive_size_bytes"}, "comparator_reference_fields_invalid")
        if record["commit"] != entries[name]["commit"] or record["tree"] != entries[name]["git_tree"]:
            raise ClosureError("comparator_reference_identity_mismatch", component=name)
        if not isinstance(record["archive_sha256"], str) or HEX_SHA256.fullmatch(record["archive_sha256"]) is None:
            raise ClosureError("comparator_archive_identity_invalid", component=name)
        if isinstance(record["archive_size_bytes"], bool) or not isinstance(record["archive_size_bytes"], int) or record["archive_size_bytes"] <= 0:
            raise ClosureError("comparator_archive_identity_invalid", component=name)
        expected_runtime = {
            "ado_subdir": f"{name}/{entries[name]['capture_ado_subdir']}",
            "required_file_sha256": dict(entries[name]["required_capture_sha256"]),
        }
        if runtime[name] != expected_runtime:
            raise ClosureError("runtime_inventory_mismatch", component=name)
        if (
            record["archive_sha256"] != entries[name]["source_archive_sha256"]
            or record["archive_size_bytes"] != entries[name]["source_archive_size_bytes"]
        ):
            raise ClosureError("reference_archive_identity_mismatch", component=name)
    return comparator


def _load_receipt(path: Path) -> Mapping[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise ClosureError("receipt_missing")
    try:
        value = json.loads(path.read_text(encoding="ascii"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ClosureError("receipt_invalid_json") from error
    if not isinstance(value, Mapping):
        raise ClosureError("receipt_invalid_json")
    return value


def verify_runtime_root(
    runtime_root: Path,
    entries: Mapping[str, Mapping[str, Any]],
) -> None:
    if runtime_root.is_symlink() or not runtime_root.is_dir():
        raise ClosureError("runtime_root_missing")
    try:
        names = {path.name for path in runtime_root.iterdir()}
    except OSError as error:
        raise ClosureError("runtime_root_unreadable") from error
    if names != set(COMPONENTS):
        raise ClosureError("runtime_component_inventory_invalid")
    _runtime_paths(runtime_root)
    for name in COMPONENTS:
        component = runtime_root / name
        expected_files = set(entries[name]["required_capture_files"])
        observed_files = {
            path.relative_to(component).as_posix()
            for path in component.rglob("*")
            if path.is_file()
        }
        expected_directories = {
            parent.as_posix()
            for relative in expected_files
            for parent in PurePosixPath(relative).parents
            if parent != PurePosixPath(".")
        }
        observed_directories = {
            path.relative_to(component).as_posix()
            for path in component.rglob("*")
            if path.is_dir()
        }
        if (
            observed_files != expected_files
            or observed_directories != expected_directories
        ):
            raise ClosureError("runtime_file_inventory_invalid", component=name)
        _verify_required_files(runtime_root / name, entries[name], name)


def _outside_repository(path: Path) -> Path:
    if not path.is_absolute():
        raise ClosureError("output_root_not_absolute")
    parent = path.parent
    if parent.is_symlink() or not parent.is_dir():
        raise ClosureError("output_parent_invalid")
    resolved_parent = parent.resolve()
    resolved = resolved_parent / path.name
    repository = ROOT.resolve()
    if resolved == repository or repository in resolved.parents or resolved in repository.parents:
        raise ClosureError("output_root_inside_repository")
    if path.exists() or path.is_symlink():
        raise ClosureError("output_root_exists")
    return resolved


def assemble(
    *,
    ols_reference_root: Path,
    ppmlhdfe_archive: Path,
    output_root: Path,
    lock_path: Path = LOCK,
) -> tuple[Path, Path, Mapping[str, Any]]:
    output = _outside_repository(output_root)
    lock = load_reference_lock(lock_path)
    entries = locked_entries(lock)
    try:
        identities = preflight_reference_root(ols_reference_root, lock, ROOT)
    except PreflightError as error:
        raise ClosureError("ols_reference_preflight_failed") from error
    if set(identities) != set(OLS_COMPONENTS):
        raise ClosureError("ols_reference_inventory_invalid")

    temporary = Path(tempfile.mkdtemp(prefix=".fereg-feppml-closure-", dir=output.parent))
    temporary.chmod(stat.S_IRWXU)
    moved = False
    try:
        runtime = temporary / "runtime"
        runtime.mkdir(mode=stat.S_IRWXU)
        sources: dict[str, SourceArchive] = {}
        sources["ppmlhdfe"] = stage_ppmlhdfe_archive(
            ppmlhdfe_archive,
            entries["ppmlhdfe"],
            runtime / "ppmlhdfe",
        )
        for name in OLS_COMPONENTS:
            sources[name] = _stage_git_reference(
                identities[name], entries[name], runtime / name
            )
        verify_runtime_root(runtime, entries)
        bundle_path = temporary / BUNDLE_NAME
        bundle = write_deterministic_bundle(runtime, bundle_path)
        document = _receipt_document(
            lock_sha256=sha256_file(lock_path),
            entries=entries,
            sources=sources,
            bundle=bundle,
        )
        receipt_path = temporary / RECEIPT_NAME
        _write_receipt(receipt_path, document)
        verify_receipt_document(document, bundle_path, sha256_file(lock_path), entries)
        temporary.rename(output)
        moved = True
        return output / BUNDLE_NAME, output / RECEIPT_NAME, document["payload"]["comparator"]
    finally:
        if not moved:
            shutil.rmtree(temporary, ignore_errors=True)


def verify(
    *,
    receipt_path: Path,
    bundle_path: Path,
    runtime_root: Path | None = None,
    lock_path: Path = LOCK,
) -> Mapping[str, Any]:
    lock = load_reference_lock(lock_path)
    entries = locked_entries(lock)
    document = _load_receipt(receipt_path)
    comparator = verify_receipt_document(
        document, bundle_path, sha256_file(lock_path), entries
    )
    if runtime_root is not None:
        verify_runtime_root(runtime_root, entries)
    return comparator


def materialize(
    *,
    receipt_path: Path,
    bundle_path: Path,
    output_root: Path,
    lock_path: Path = LOCK,
) -> Path:
    output = _outside_repository(output_root)
    comparator = verify(
        receipt_path=receipt_path,
        bundle_path=bundle_path,
        lock_path=lock_path,
    )
    del comparator
    lock = load_reference_lock(lock_path)
    entries = locked_entries(lock)
    temporary = Path(tempfile.mkdtemp(prefix=".fereg-feppml-runtime-", dir=output.parent))
    temporary.chmod(stat.S_IRWXU)
    moved = False
    try:
        try:
            data = bundle_path.read_bytes()
        except OSError as error:
            raise ClosureError("bundle_unreadable") from error
        _extract_archive_bytes(data, temporary / "runtime", allowed_roots=set(COMPONENTS))
        verify_runtime_root(temporary / "runtime", entries)
        temporary.rename(output)
        moved = True
        return output / "runtime"
    finally:
        if not moved:
            shutil.rmtree(temporary, ignore_errors=True)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", type=Path, default=LOCK)
    subparsers = parser.add_subparsers(dest="action", required=True)

    assemble_parser = subparsers.add_parser("assemble")
    assemble_parser.add_argument("--ols-reference-root", type=Path, required=True)
    assemble_parser.add_argument("--ppmlhdfe-archive", type=Path, required=True)
    assemble_parser.add_argument("--output-root", type=Path, required=True)

    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("--receipt", type=Path, required=True)
    verify_parser.add_argument("--bundle", type=Path, required=True)
    verify_parser.add_argument("--runtime-root", type=Path)

    materialize_parser = subparsers.add_parser("materialize")
    materialize_parser.add_argument("--receipt", type=Path, required=True)
    materialize_parser.add_argument("--bundle", type=Path, required=True)
    materialize_parser.add_argument("--output-root", type=Path, required=True)

    args = parser.parse_args(argv)
    try:
        if args.action == "assemble":
            bundle, _receipt, comparator = assemble(
                ols_reference_root=args.ols_reference_root,
                ppmlhdfe_archive=args.ppmlhdfe_archive,
                output_root=args.output_root,
                lock_path=args.lock,
            )
            print(
                "FEPPML_REFERENCE_CLOSURE status=success action=assemble "
                f"components={len(COMPONENTS)} bundle_sha256={comparator['bundle_sha256']} "
                f"bundle_size_bytes={bundle.stat().st_size}"
            )
        elif args.action == "verify":
            comparator = verify(
                receipt_path=args.receipt,
                bundle_path=args.bundle,
                runtime_root=args.runtime_root,
                lock_path=args.lock,
            )
            print(
                "FEPPML_REFERENCE_CLOSURE status=success action=verify "
                f"components={len(COMPONENTS)} bundle_sha256={comparator['bundle_sha256']}"
            )
        else:
            materialize(
                receipt_path=args.receipt,
                bundle_path=args.bundle,
                output_root=args.output_root,
                lock_path=args.lock,
            )
            print(
                "FEPPML_REFERENCE_CLOSURE status=success action=materialize "
                f"components={len(COMPONENTS)}"
            )
    except ClosureError as error:
        print(
            "FEPPML_REFERENCE_CLOSURE status=failure "
            f"action={args.action} kind={error.kind} component={error.component}"
        )
        return 1
    except Exception:
        print(
            "FEPPML_REFERENCE_CLOSURE status=failure "
            f"action={args.action} kind=unexpected_failure component=none"
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
