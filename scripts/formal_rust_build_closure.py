#!/usr/bin/env python3
"""Create and replay a closed, offline Rust build supply-chain receipt.

This module is deliberately independent of the SCC controllers.  A caller
supplies the *input* Cargo cache locations, exact compiler/binutil paths, and a
run root.  The retained result is a private ``CARGO_HOME`` containing only an
exact config plus a versioned vendor tree.  Registry packages are reconstructed
from ``.crate`` archives whose SHA-256 is pinned by ``Cargo.lock``.  Git
packages are checked against the locked commit, Git objects, a clean worktree,
and the bytes emitted into the vendor tree.

The public verifier has two modes.  Local replay rechecks external source
caches, compilers, helpers, and rustlib.  Portable replay needs only the
collected run tree and verifies every retained byte plus the closed receipt
relationships; it never silently weakens a local replay into a portable one.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shlex
import shutil
import stat
import subprocess
import tarfile
import tempfile
import tomllib
from typing import Any, Mapping, Sequence
from urllib.parse import parse_qs, urlsplit, urlunsplit


SCHEMA_VERSION = 1
RECEIPT_TYPE = "formal-rust-build-closure-v1"
DEPENDENCY_RECEIPT_TYPE = "formal-rust-dependency-closure-v1"
TOOLCHAIN_RECEIPT_TYPE = "formal-rust-toolchain-closure-v1"
LINUX_RUNTIME_RECEIPT_TYPE = "formal-linux-runtime-closure-v1"
COMPLETE_SENTINEL = "FORMAL_RUST_BUILD_CLOSURE_COMPLETE"
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
HEX_GIT_COMMIT = re.compile(r"^[0-9a-f]{40}$")
REQUIRED_HELPER_ROLES = (
    "cc",
    "linker",
    "ar",
    "ranlib",
    "nm",
    "strip",
    "objcopy",
)
REQUIRED_BUILD_TOOL_ROLES = (
    "bash",
    "python3",
    "dirname",
    "basename",
    "mkdir",
    "rm",
    "sha256sum",
    "readelf",
    "nm",
    "ldd",
    "uname",
)
CC_DISCOVERED_ROLES = ("cc1", "collect2", "assembler", "cc_ld")
C_LINK_INPUT_NAMES = (
    "crt1.o",
    "Scrt1.o",
    "crti.o",
    "crtbegin.o",
    "crtbeginS.o",
    "crtbeginT.o",
    "crtend.o",
    "crtendS.o",
    "crtn.o",
    "libgcc.a",
    "libgcc_s.so",
    "libgcc_s.so.1",
    "libc.so",
    "libc.so.6",
    "libdl.so",
    "libdl.so.2",
    "libpthread.so",
    "libpthread.so.0",
    "libm.so",
    "libm.so.6",
    "librt.so",
    "librt.so.1",
    "libutil.so",
    "libutil.so.1",
    "ld-linux-x86-64.so.2",
)
RESERVED_BUILD_ENVIRONMENT = frozenset(
    {
        "HOME",
        "TMPDIR",
        "PATH",
        "LANG",
        "LC_ALL",
        "TZ",
        "CARGO_HOME",
        "CARGO_NET_OFFLINE",
        "CARGO_INCREMENTAL",
        "CARGO_BUILD_JOBS",
        "CARGO_TARGET_DIR",
        "RUSTC",
        "CC",
        "AR",
        "RANLIB",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER",
        "PYTHONNOUSERSITE",
        "PYTHONDONTWRITEBYTECODE",
        "PYTHONHASHSEED",
    }
)


def compiler_base_image_trust_boundary() -> dict[str, Any]:
    """Return the exact boundary between pre-build and post-build closure."""

    return {
        "schema_version": 1,
        "classification": "self-contained-linux-compiler-input-closure",
        "covered_by_this_receipt": True,
        "pre_build_inputs": [
            "gcc_system_include_directories_and_headers",
            "gcc_specs_search_directories_and_internal_data_files",
            "crt_startfiles_and_endfiles",
            "libgcc_and_libgcc_s",
            "linker_scripts_and_dynamic_loader",
            "system_libc_libdl_libpthread_libm_librt_libutil",
        ],
        "required_post_build_receipt": LINUX_RUNTIME_RECEIPT_TYPE,
        "post_build_inputs": [
            "elf_interpreter_and_needed_entries",
            "transitive_ldd_runtime_shared_library_closure",
            "runtime_symbol_resolution_ldd_r",
        ],
        "qualification_rule": (
            "A build is closed only when this toolchain receipt and a verified "
            "formal-linux-runtime-closure-v1 receipt for the final artifact are both present."
        ),
    }


class ClosureError(ValueError):
    """A build closure is incomplete, mutable, or not exactly replayable."""


@dataclass(frozen=True)
class SourceCacheInputs:
    """Read-only input locations used to produce the run-local vendor tree.

    ``git_repositories`` is keyed by the Git source URL without its ``#commit``
    suffix, exactly as it occurs in Cargo's generated source-replacement key.
    For a formal materialization each repository must be the unique checkout at
    the locked revision below ``cargo_source_home/git/checkouts``.
    """

    cargo_source_home: Path
    registry_archive_roots: tuple[Path, ...]
    git_repositories: Mapping[str, Path]
    git_executable: Path = Path("/usr/bin/git")


def canonical_json_bytes(value: Any) -> bytes:
    try:
        result = json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
    except (TypeError, ValueError) as error:
        raise ClosureError(f"value is not canonical finite JSON: {error}") from error
    return (result + "\n").encode("ascii")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1 << 20), b""):
                digest.update(block)
    except OSError as error:
        raise ClosureError(f"unable to hash {path}: {error}") from error
    return digest.hexdigest()


def _expect_keys(value: Any, keys: set[str], where: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ClosureError(f"{where} must be an object")
    if set(value) != keys:
        raise ClosureError(
            f"{where} fields are not closed; "
            f"missing={sorted(keys - set(value))}, extra={sorted(set(value) - keys)}"
        )
    return value


def _sha256(value: Any, where: str) -> str:
    if not isinstance(value, str) or HEX_SHA256.fullmatch(value) is None:
        raise ClosureError(f"{where} must be a lowercase SHA-256")
    return value


def _integer(value: Any, where: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        raise ClosureError(f"{where} must be an integer >= {minimum}")
    return value


def _safe_relative(value: Any, where: str) -> str:
    if not isinstance(value, str) or not value:
        raise ClosureError(f"{where} must be a nonempty relative path")
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or "." in path.parts:
        raise ClosureError(f"{where} is not a safe relative path")
    return value


def _retained_run_root(run_root: Path, where: str) -> Path:
    """Return a lexical absolute run root that is a real directory, not a link."""

    root = Path(os.path.abspath(run_root))
    try:
        details = os.lstat(root)
    except OSError as error:
        raise ClosureError(f"{where} is absent or unreadable: {error}") from error
    if stat.S_ISLNK(details.st_mode) or not stat.S_ISDIR(details.st_mode):
        raise ClosureError(f"{where} is not a real directory")
    return root


def _retained_path(
    run_root: Path,
    relative_value: Any,
    where: str,
    *,
    leaf_kind: str,
) -> Path:
    """Resolve a retained path without permitting a symlink in any component."""

    root = _retained_run_root(run_root, "retained run root")
    relative = _safe_relative(relative_value, where)
    parts = PurePosixPath(relative).parts
    current = root
    for index, part in enumerate(parts):
        current = current / part
        try:
            details = os.lstat(current)
        except OSError as error:
            raise ClosureError(f"{where} is absent or unreadable: {error}") from error
        if stat.S_ISLNK(details.st_mode):
            raise ClosureError(f"{where} contains a symlinked path component: {current}")
        if index < len(parts) - 1 and not stat.S_ISDIR(details.st_mode):
            raise ClosureError(f"{where} contains a non-directory ancestor: {current}")
    leaf_mode = os.lstat(current).st_mode
    if leaf_kind == "file":
        valid_leaf = stat.S_ISREG(leaf_mode)
    elif leaf_kind == "directory":
        valid_leaf = stat.S_ISDIR(leaf_mode)
    else:
        raise ClosureError(f"internal retained-path leaf kind is invalid: {leaf_kind}")
    if not valid_leaf:
        raise ClosureError(f"{where} is not a retained {leaf_kind}")
    try:
        current.resolve(strict=True).relative_to(root.resolve(strict=True))
    except (OSError, ValueError) as error:
        raise ClosureError(f"{where} resolves outside the retained run root") from error
    return current


def _descendant(root: Path, path: Path, where: str) -> Path:
    root_resolved = root.resolve()
    path_resolved = path.resolve()
    try:
        path_resolved.relative_to(root_resolved)
    except ValueError as error:
        raise ClosureError(f"{where} is outside the run root") from error
    return path_resolved


def _relative_file_record(run_root: Path, path: Path) -> dict[str, Any]:
    root = _retained_run_root(run_root, "retained run root")
    candidate = Path(os.path.abspath(path))
    try:
        relative = candidate.relative_to(root).as_posix()
    except ValueError as error:
        raise ClosureError(f"retained file is outside the run root: {path}") from error
    resolved = _retained_path(root, relative, "retained file", leaf_kind="file")
    return {
        "path": relative,
        "sha256": sha256_file(resolved),
        "size_bytes": resolved.stat().st_size,
    }


def _verify_relative_file(run_root: Path, value: Any, where: str) -> Path:
    record = _expect_keys(value, {"path", "sha256", "size_bytes"}, where)
    relative = _safe_relative(record["path"], f"{where}.path")
    _sha256(record["sha256"], f"{where}.sha256")
    _integer(record["size_bytes"], f"{where}.size_bytes")
    path = _retained_path(run_root, relative, where, leaf_kind="file")
    if (
        path.stat().st_size != record["size_bytes"]
        or sha256_file(path) != record["sha256"]
    ):
        raise ClosureError(f"{where} bytes changed")
    return path


def tree_inventory(
    root: Path, *, allow_symlinks: bool = False, allow_empty: bool = False
) -> dict[str, Any]:
    """Return a full, canonical file/symlink inventory for ``root``."""

    if root.is_symlink() or not root.is_dir():
        raise ClosureError(f"tree is absent or unsafe: {root}")
    entries: list[dict[str, Any]] = []
    total = 0
    for path in sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            if not allow_symlinks:
                raise ClosureError(f"tree contains a symlink: {relative}")
            entries.append(
                {
                    "path": relative,
                    "kind": "symlink",
                    "target": os.readlink(path),
                }
            )
            continue
        if path.is_dir():
            continue
        if not path.is_file():
            raise ClosureError(f"tree contains a non-regular entry: {relative}")
        details = path.stat()
        total += details.st_size
        entries.append(
            {
                "path": relative,
                "kind": "file",
                "sha256": sha256_file(path),
                "size_bytes": details.st_size,
                "mode": stat.S_IMODE(details.st_mode),
            }
        )
    if not entries and not allow_empty:
        raise ClosureError(f"tree inventory is empty: {root}")
    return {
        "sha256": sha256_bytes(canonical_json_bytes(entries)),
        "entries": entries,
        "entry_count": len(entries),
        "size_bytes": total,
    }


def _verify_tree_record(
    root: Path,
    value: Any,
    where: str,
    *,
    allow_symlinks: bool,
    allow_empty: bool = False,
    containment_root: Path | None = None,
) -> None:
    record = _expect_keys(
        value, {"sha256", "entries", "entry_count", "size_bytes"}, where
    )
    _sha256(record["sha256"], f"{where}.sha256")
    if not isinstance(record["entries"], list):
        raise ClosureError(f"{where}.entries must be an array")
    _integer(
        record["entry_count"],
        f"{where}.entry_count",
        minimum=0 if allow_empty else 1,
    )
    _integer(record["size_bytes"], f"{where}.size_bytes")
    if containment_root is not None:
        lexical_root = Path(os.path.abspath(containment_root))
        lexical_tree = Path(os.path.abspath(root))
        try:
            relative = lexical_tree.relative_to(lexical_root).as_posix()
        except ValueError as error:
            raise ClosureError(f"{where} is outside the retained run root") from error
        root = _retained_path(
            lexical_root, relative, where, leaf_kind="directory"
        )
    if record != tree_inventory(
        root, allow_symlinks=allow_symlinks, allow_empty=allow_empty
    ):
        raise ClosureError(f"{where} changed")


def _git_source_parts(source: str) -> tuple[str, str, str]:
    if not source.startswith("git+") or "#" not in source:
        raise ClosureError(f"unsupported Git Cargo source: {source}")
    without_fragment, revision = source.rsplit("#", 1)
    if HEX_GIT_COMMIT.fullmatch(revision) is None:
        raise ClosureError(f"Git Cargo source is not pinned to a full commit: {source}")
    parsed = urlsplit(without_fragment[4:])
    query = parse_qs(parsed.query, strict_parsing=True)
    if set(query) != {"rev"} or query["rev"] != [revision]:
        raise ClosureError(f"Git Cargo source lacks an exact matching rev pin: {source}")
    repository_url = urlunsplit((parsed.scheme, parsed.netloc, parsed.path, "", ""))
    return without_fragment, repository_url, revision


def load_cargo_lock(path: Path) -> dict[str, Any]:
    """Parse and close the committed Cargo.lock dependency identities."""

    if path.is_symlink() or not path.is_file():
        raise ClosureError(f"Cargo.lock is absent or unsafe: {path}")
    raw = path.read_bytes()
    try:
        document = tomllib.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise ClosureError(f"Cargo.lock is not valid UTF-8 TOML: {error}") from error
    if set(document) != {"version", "package"} or document.get("version") != 4:
        raise ClosureError("Cargo.lock must be an exact version-4 package document")
    packages = document.get("package")
    if not isinstance(packages, list) or not packages:
        raise ClosureError("Cargo.lock package list is empty")
    registry: list[dict[str, str]] = []
    git: list[dict[str, str]] = []
    workspace: list[dict[str, str]] = []
    identities: set[tuple[str, str, str]] = set()
    vendor_names: set[str] = set()
    for index, raw_package in enumerate(packages):
        where = f"Cargo.lock.package[{index}]"
        if not isinstance(raw_package, Mapping):
            raise ClosureError(f"{where} must be a table")
        allowed = {"name", "version", "source", "checksum", "dependencies"}
        if not set(raw_package).issubset(allowed) or not {"name", "version"}.issubset(raw_package):
            raise ClosureError(f"{where} has unsupported or missing fields")
        name = raw_package["name"]
        version = raw_package["version"]
        if not isinstance(name, str) or not name or not isinstance(version, str) or not version:
            raise ClosureError(f"{where} name/version is invalid")
        source = raw_package.get("source")
        if source is None:
            if "checksum" in raw_package:
                raise ClosureError(f"{where} workspace package unexpectedly has a checksum")
            workspace.append({"name": name, "version": version})
            continue
        if not isinstance(source, str):
            raise ClosureError(f"{where}.source must be text")
        identity = (name, version, source)
        if identity in identities:
            raise ClosureError(f"duplicate Cargo.lock package identity: {identity}")
        identities.add(identity)
        vendor_name = f"{name}-{version}"
        if vendor_name in vendor_names:
            raise ClosureError(f"Cargo.lock has a colliding versioned vendor name: {vendor_name}")
        vendor_names.add(vendor_name)
        if source.startswith("registry+"):
            checksum = raw_package.get("checksum")
            _sha256(checksum, f"{where}.checksum")
            registry.append(
                {
                    "name": name,
                    "version": version,
                    "source": source,
                    "checksum": checksum,
                    "vendor_name": vendor_name,
                }
            )
        elif source.startswith("git+"):
            if "checksum" in raw_package:
                raise ClosureError(f"{where} Git package unexpectedly has a checksum")
            source_key, repository_url, revision = _git_source_parts(source)
            git.append(
                {
                    "name": name,
                    "version": version,
                    "source": source,
                    "source_key": source_key,
                    "repository_url": repository_url,
                    "revision": revision,
                    "vendor_name": vendor_name,
                }
            )
        else:
            raise ClosureError(f"unsupported Cargo.lock source: {source}")
    return {
        "sha256": sha256_bytes(raw),
        "size_bytes": len(raw),
        "lock_version": 4,
        "registry": sorted(registry, key=lambda item: item["vendor_name"]),
        "git": sorted(git, key=lambda item: item["vendor_name"]),
        "workspace": sorted(workspace, key=lambda item: (item["name"], item["version"])),
    }


def _find_registry_archive(package: Mapping[str, str], roots: Sequence[Path]) -> Path:
    name = f"{package['vendor_name']}.crate"
    candidates: set[Path] = set()
    for root in roots:
        if root.is_symlink():
            raise ClosureError(f"registry archive input root is a symlink: {root}")
        if root.is_file() and root.name == name:
            candidates.add(root.resolve())
        elif root.is_dir():
            direct = root / name
            if direct.is_file():
                if direct.is_symlink():
                    raise ClosureError(f"registry archive is a symlink: {direct}")
                candidates.add(direct.resolve())
            for child in root.iterdir():
                nested = child / name
                if child.is_dir() and nested.is_file():
                    if child.is_symlink() or nested.is_symlink():
                        raise ClosureError(f"registry archive path is a symlink: {nested}")
                    candidates.add(nested.resolve())
    if len(candidates) != 1:
        raise ClosureError(
            f"registry archive resolution for {name} is not unique: "
            f"{sorted(str(path) for path in candidates)}"
        )
    archive = next(iter(candidates))
    if archive.is_symlink() or sha256_file(archive) != package["checksum"]:
        raise ClosureError(f"registry archive does not match Cargo.lock: {archive}")
    return archive


def _crate_archive_files(archive: Path, vendor_name: str) -> dict[str, tuple[bytes, int]]:
    files: dict[str, tuple[bytes, int]] = {}
    try:
        source = tarfile.open(archive, mode="r:*")
    except (OSError, tarfile.TarError) as error:
        raise ClosureError(f"unable to read crate archive {archive}: {error}") from error
    with source:
        for member in source.getmembers():
            path = PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or not path.parts:
                raise ClosureError(f"crate archive contains an unsafe path: {member.name}")
            if path.parts[0] != vendor_name:
                raise ClosureError(f"crate archive top directory changed: {member.name}")
            if member.isdir():
                continue
            if not member.isfile() or len(path.parts) == 1:
                raise ClosureError(f"crate archive contains a non-regular entry: {member.name}")
            relative = PurePosixPath(*path.parts[1:]).as_posix()
            if relative in files:
                raise ClosureError(f"crate archive repeats a file: {relative}")
            extracted = source.extractfile(member)
            if extracted is None:
                raise ClosureError(f"unable to extract crate member: {member.name}")
            payload = extracted.read()
            if len(payload) != member.size:
                raise ClosureError(f"crate member size changed: {member.name}")
            mode = 0o755 if member.mode & 0o111 else 0o644
            files[relative] = (payload, mode)
    if "Cargo.toml" not in files or not any(path.startswith("src/") for path in files):
        raise ClosureError(f"crate archive lacks its package contract: {archive}")
    return files


def _write_new(path: Path, payload: bytes, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open("xb") as destination:
            destination.write(payload)
    except OSError as error:
        raise ClosureError(f"unable to create retained closure file {path}: {error}") from error
    path.chmod(mode)


def _make_tree_read_only(root: Path) -> None:
    if root.is_symlink() or not root.is_dir():
        raise ClosureError(f"cannot make an unsafe tree read-only: {root}")
    for path in sorted(root.rglob("*"), key=lambda item: len(item.parts), reverse=True):
        if path.is_symlink():
            raise ClosureError(f"read-only closure contains a symlink: {path}")
        if path.is_dir():
            path.chmod(0o555)
        elif path.is_file():
            path.chmod(stat.S_IMODE(path.stat().st_mode) & ~0o222)
        else:
            raise ClosureError(f"read-only closure contains an unsafe entry: {path}")
    root.chmod(0o555)


def materialize_registry_package(
    package: Mapping[str, str], archive: Path, destination: Path
) -> dict[str, Any]:
    """Reconstruct one registry vendor directory from its pinned archive."""

    if destination.exists() or destination.is_symlink():
        raise ClosureError(f"registry vendor destination already exists: {destination}")
    files = _crate_archive_files(archive, package["vendor_name"])
    destination.mkdir(parents=True)
    checksums: dict[str, str] = {}
    for relative, (payload, mode) in sorted(files.items()):
        _write_new(destination / relative, payload, mode)
        checksums[relative] = sha256_bytes(payload)
    checksum_document = {"files": checksums, "package": package["checksum"]}
    _write_new(destination / ".cargo-checksum.json", canonical_json_bytes(checksum_document))
    return {
        "name": package["name"],
        "version": package["version"],
        "source": package["source"],
        "checksum": package["checksum"],
        "archive_path": str(archive),
        "archive_sha256": sha256_file(archive),
        "archive_size_bytes": archive.stat().st_size,
        "vendor_name": package["vendor_name"],
        "vendor": tree_inventory(destination),
    }


def _git_environment(git: Path) -> dict[str, str]:
    return {
        "HOME": "/dev/null",
        "XDG_CONFIG_HOME": "/dev/null",
        "PATH": str(git.parent),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_ATTR_NOSYSTEM": "1",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
    }


def _git_run(git: Path, repository: Path, arguments: Sequence[str], where: str) -> bytes:
    command = [str(git), "--no-replace-objects", "-C", str(repository), *arguments]
    completed = subprocess.run(
        command,
        env=_git_environment(git),
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise ClosureError(f"{where} failed: {detail}")
    return completed.stdout


def _reject_git_rewrite_mechanisms(repository: Path, git: Path) -> None:
    storage_roots: list[Path] = []
    for argument, where in (
        ("--git-dir", "Git directory"),
        ("--git-common-dir", "Git common directory"),
    ):
        raw = _git_run(git, repository, ["rev-parse", argument], where).decode(
            "utf-8"
        ).strip()
        root = Path(raw)
        if not root.is_absolute():
            root = (repository / root).resolve()
        else:
            root = root.resolve()
        if root not in storage_roots:
            storage_roots.append(root)
    for storage in storage_roots:
        replace_root = storage / "refs/replace"
        if replace_root.exists() or replace_root.is_symlink():
            raise ClosureError(f"Git source has replacement refs: {replace_root}")
        packed_refs = storage / "packed-refs"
        if packed_refs.is_symlink():
            raise ClosureError(f"Git packed refs is a symlink: {packed_refs}")
        if packed_refs.is_file():
            try:
                packed_text = packed_refs.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError) as error:
                raise ClosureError(f"unable to inspect Git packed refs: {error}") from error
            if any(
                line
                and not line.startswith(("#", "^"))
                and line.split()[-1].startswith("refs/replace/")
                for line in packed_text.splitlines()
            ):
                raise ClosureError("Git source has packed replacement refs")
        grafts = storage / "info/grafts"
        if grafts.exists() or grafts.is_symlink():
            raise ClosureError(f"Git source has a graft file: {grafts}")


def _git_repository_record(
    repository: Path, revision: str, git_executable: Path
) -> dict[str, Any]:
    if repository.is_symlink() or not repository.is_dir():
        raise ClosureError(f"Git source repository is absent or unsafe: {repository}")
    git = git_executable.resolve()
    if git.is_symlink() or not git.is_file() or not os.access(git, os.X_OK):
        raise ClosureError(f"Git executable is absent or unsafe: {git_executable}")
    _reject_git_rewrite_mechanisms(repository, git)
    head = _git_run(git, repository, ["rev-parse", "HEAD"], "Git HEAD").decode().strip()
    commit = _git_run(
        git, repository, ["rev-parse", f"{revision}^{{commit}}"], "locked Git commit"
    ).decode().strip()
    if head != revision or commit != revision:
        raise ClosureError(f"Git source checkout is not at locked revision {revision}")
    tree = _git_run(
        git, repository, ["rev-parse", f"{revision}^{{tree}}"], "locked Git tree"
    ).decode().strip()
    _git_run(git, repository, ["fsck", "--strict", "--no-dangling"], "Git object audit")
    status_output = _git_run(
        git,
        repository,
        ["status", "--porcelain=v1", "--untracked-files=all"],
        "Git worktree status",
    ).decode("utf-8")
    status_lines = [line for line in status_output.splitlines() if line]
    cargo_marker = repository / ".cargo-ok"
    allowed_marker = status_lines == ["?? .cargo-ok"] and cargo_marker.is_file() and cargo_marker.stat().st_size == 0
    if status_lines and not allowed_marker:
        raise ClosureError(f"Git source worktree is not clean: {status_lines}")
    entries: list[dict[str, Any]] = []
    total = 0
    raw_paths = _git_run(
        git, repository, ["ls-tree", "-r", "-z", "--name-only", revision], "Git tree paths"
    )
    for raw_relative in raw_paths.split(b"\0"):
        if not raw_relative:
            continue
        try:
            relative = raw_relative.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ClosureError("Git source contains a non-UTF-8 path") from error
        _safe_relative(relative, "Git tree path")
        path = repository / relative
        if path.is_symlink() or not path.is_file():
            raise ClosureError(f"Git source contains an unsupported tracked entry: {relative}")
        payload = _git_run(
            git, repository, ["show", f"{revision}:{relative}"], f"Git blob {relative}"
        )
        if path.read_bytes() != payload:
            raise ClosureError(f"Git worktree bytes differ from locked object: {relative}")
        size = len(payload)
        total += size
        entries.append(
            {
                "path": relative,
                "sha256": sha256_bytes(payload),
                "size_bytes": size,
            }
        )
    return {
        "path": str(repository.resolve()),
        "revision": revision,
        "tree": tree,
        "git": {
            "path": str(git_executable),
            "resolved_path": str(git),
            "sha256": sha256_file(git),
            "size_bytes": git.stat().st_size,
        },
        "git_environment": _git_environment(git),
        "tracked_tree": {
            "sha256": sha256_bytes(canonical_json_bytes(entries)),
            "entries": entries,
            "entry_count": len(entries),
            "size_bytes": total,
        },
        "cargo_cache_marker": allowed_marker,
    }


def _require_unique_cargo_checkout(
    cargo_home: Path,
    repository: Path,
    revision: str,
    git_executable: Path,
) -> None:
    """Prove Cargo has exactly one checkout it can select for ``revision``."""

    checkout_root = cargo_home / "git/checkouts"
    if checkout_root.is_symlink() or not checkout_root.is_dir():
        raise ClosureError(f"Cargo Git checkout cache is absent or unsafe: {checkout_root}")
    candidates: set[Path] = set()
    for parent in checkout_root.iterdir():
        if parent.is_symlink() or not parent.is_dir():
            continue
        for checkout in parent.iterdir():
            if checkout.is_symlink() or not checkout.is_dir():
                continue
            try:
                head = _git_run(
                    git_executable.resolve(), checkout, ["rev-parse", "HEAD"], "cached Git HEAD"
                ).decode().strip()
            except ClosureError:
                continue
            if head == revision:
                candidates.add(checkout.resolve())
    if candidates != {repository.resolve()}:
        raise ClosureError(
            f"Cargo checkout for locked revision {revision} is not unique and exact: "
            f"{sorted(str(path) for path in candidates)}"
        )


def _package_manifest_roots(repository: Path) -> dict[tuple[str, str], Path]:
    roots: dict[tuple[str, str], Path] = {}
    for manifest in sorted(repository.rglob("Cargo.toml")):
        relative_parts = manifest.relative_to(repository).parts
        if any(part in {".git", "target", "vendor"} for part in relative_parts):
            continue
        try:
            document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
            raise ClosureError(f"unable to parse Git package manifest {manifest}: {error}") from error
        package = document.get("package")
        if not isinstance(package, Mapping) or not isinstance(package.get("name"), str):
            continue
        version = package.get("version")
        if not isinstance(version, str):
            workspace = document.get("workspace")
            # A member using ``version.workspace = true`` inherits from the
            # closest ancestor workspace.package table.
            cursor = manifest.parent
            inherited: str | None = None
            while cursor == repository or repository in cursor.parents:
                candidate = cursor / "Cargo.toml"
                if candidate != manifest and candidate.is_file():
                    parent_doc = tomllib.loads(candidate.read_text(encoding="utf-8"))
                    parent_workspace = parent_doc.get("workspace")
                    if isinstance(parent_workspace, Mapping):
                        workspace_package = parent_workspace.get("package")
                        if isinstance(workspace_package, Mapping) and isinstance(
                            workspace_package.get("version"), str
                        ):
                            inherited = workspace_package["version"]
                            break
                if cursor == repository:
                    break
                cursor = cursor.parent
            version = inherited
        if not isinstance(version, str):
            raise ClosureError(f"Git package version cannot be resolved: {manifest}")
        key = (package["name"], version)
        if key in roots:
            raise ClosureError(f"Git repository repeats package {key}")
        roots[key] = manifest.parent
    return roots


def _checksum_document(directory: Path, *, expected_package: str | None) -> dict[str, str]:
    checksum_path = directory / ".cargo-checksum.json"
    if checksum_path.is_symlink() or not checksum_path.is_file():
        raise ClosureError(f"vendored package lacks .cargo-checksum.json: {directory}")
    try:
        document = json.loads(checksum_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ClosureError(f"invalid vendor checksum document: {checksum_path}") from error
    if not isinstance(document, Mapping) or set(document) != {"files", "package"}:
        raise ClosureError(f"vendor checksum fields are not closed: {checksum_path}")
    if document["package"] != expected_package or not isinstance(document["files"], Mapping):
        raise ClosureError(f"vendor package checksum changed: {checksum_path}")
    actual_files = {
        path.relative_to(directory).as_posix()
        for path in directory.rglob("*")
        if path.is_file() and not path.is_symlink() and path != checksum_path
    }
    if set(document["files"]) != actual_files:
        raise ClosureError(f"vendor checksum file set changed: {directory}")
    result: dict[str, str] = {}
    for relative, digest in document["files"].items():
        _safe_relative(relative, "vendor checksum path")
        _sha256(digest, f"vendor checksum {relative}")
        if sha256_file(directory / relative) != digest:
            raise ClosureError(f"vendored file differs from checksum: {directory / relative}")
        result[relative] = digest
    return result


def _validate_registry_vendor(
    package: Mapping[str, str], archive: Path, vendor_directory: Path
) -> dict[str, Any]:
    archive_files = _crate_archive_files(archive, package["vendor_name"])
    checksums = _checksum_document(vendor_directory, expected_package=package["checksum"])
    if set(checksums) != set(archive_files):
        raise ClosureError(f"registry vendor file set differs from pinned archive: {package['vendor_name']}")
    for relative, (payload, _mode) in archive_files.items():
        if (vendor_directory / relative).read_bytes() != payload:
            raise ClosureError(f"registry vendor bytes differ from pinned archive: {relative}")
    return {
        "name": package["name"],
        "version": package["version"],
        "source": package["source"],
        "checksum": package["checksum"],
        "archive_path": str(archive),
        "archive_sha256": sha256_file(archive),
        "archive_size_bytes": archive.stat().st_size,
        "vendor_name": package["vendor_name"],
        "vendor": tree_inventory(vendor_directory),
    }


def _validate_git_vendor(
    package: Mapping[str, str],
    repository: Path,
    repository_record: Mapping[str, Any],
    package_root: Path,
    vendor_directory: Path,
) -> dict[str, Any]:
    checksums = _checksum_document(vendor_directory, expected_package=None)
    manifest = vendor_directory / "Cargo.toml"
    if not manifest.is_file() or manifest.is_symlink():
        raise ClosureError(f"Git vendor package lacks Cargo.toml: {vendor_directory}")
    try:
        normalized = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise ClosureError(f"invalid normalized Git Cargo.toml: {manifest}") from error
    normalized_package = normalized.get("package")
    if (
        not isinstance(normalized_package, Mapping)
        or normalized_package.get("name") != package["name"]
        or normalized_package.get("version") != package["version"]
    ):
        raise ClosureError(f"normalized Git package identity changed: {vendor_directory}")
    if not any(relative.startswith("src/") for relative in checksums):
        raise ClosureError(f"Git vendor package has no source files: {vendor_directory}")
    for relative in checksums:
        if relative == "Cargo.toml":
            continue
        source = package_root / relative
        if source.is_symlink() or not source.is_file():
            raise ClosureError(f"Git vendor introduced a non-source file: {relative}")
        if source.read_bytes() != (vendor_directory / relative).read_bytes():
            raise ClosureError(f"Git vendor bytes differ from locked worktree: {relative}")
    source_manifest = package_root / "Cargo.toml"
    source_manifest_relative = source_manifest.relative_to(repository).as_posix()
    source_entry = next(
        (
            item
            for item in repository_record["tracked_tree"]["entries"]
            if item["path"] == source_manifest_relative
        ),
        None,
    )
    if source_entry is None:
        raise ClosureError(f"Git package manifest is not in the locked tree: {source_manifest}")
    return {
        "name": package["name"],
        "version": package["version"],
        "source": package["source"],
        "source_key": package["source_key"],
        "repository_url": package["repository_url"],
        "revision": package["revision"],
        "vendor_name": package["vendor_name"],
        "repository": dict(repository_record),
        "package_root": package_root.relative_to(repository).as_posix() or ".",
        "source_manifest_sha256": source_entry["sha256"],
        "normalized_manifest_sha256": sha256_file(manifest),
        "vendor": tree_inventory(vendor_directory),
    }


def _cargo_config_bytes(lock: Mapping[str, Any]) -> bytes:
    lines = [
        "[net]",
        "offline = true",
        "",
        "[source.crates-io]",
        'replace-with = "formal-vendor"',
        "",
    ]
    sources: dict[str, tuple[str, str]] = {}
    for package in lock["git"]:
        sources[package["source_key"]] = (
            package["repository_url"],
            package["revision"],
        )
    for source_key in sorted(sources):
        repository_url, revision = sources[source_key]
        lines.extend(
            [
                f'[source."{source_key}"]',
                f'git = "{repository_url}"',
                f'rev = "{revision}"',
                'replace-with = "formal-vendor"',
                "",
            ]
        )
    lines.extend(
        [
            "[source.formal-vendor]",
            # Cargo resolves a CARGO_HOME config path relative to the directory
            # containing that home (the same rule used for ``.cargo/config``).
            # ``vendor`` therefore names the sibling retained vendor tree.
            'directory = "vendor"',
            "",
        ]
    )
    return "\n".join(lines).encode("utf-8")


def validate_dependency_vendor(
    lock_path: Path,
    vendor_root: Path,
    cache_inputs: SourceCacheInputs,
    *,
    enforce_cargo_cache_checkout: bool = True,
) -> dict[str, Any]:
    """Validate all vendor bytes against Cargo.lock and source-cache inputs."""

    lock = load_cargo_lock(lock_path)
    if vendor_root.is_symlink() or not vendor_root.is_dir():
        raise ClosureError(f"vendor root is absent or unsafe: {vendor_root}")
    expected_names = {
        package["vendor_name"] for package in [*lock["registry"], *lock["git"]]
    }
    actual_names = {
        path.name
        for path in vendor_root.iterdir()
        if path.is_dir() and not path.is_symlink()
    }
    if expected_names != actual_names or any(
        not path.is_dir() or path.is_symlink() for path in vendor_root.iterdir()
    ):
        raise ClosureError(
            "versioned vendor package directories are not closed; "
            f"missing={sorted(expected_names - actual_names)}, "
            f"extra={sorted(actual_names - expected_names)}"
        )
    registry_records: list[dict[str, Any]] = []
    for package in lock["registry"]:
        archive = _find_registry_archive(package, cache_inputs.registry_archive_roots)
        registry_records.append(
            _validate_registry_vendor(package, archive, vendor_root / package["vendor_name"])
        )
    required_source_keys = {package["source_key"] for package in lock["git"]}
    if set(cache_inputs.git_repositories) != required_source_keys:
        raise ClosureError(
            "Git source input mapping is not closed; "
            f"missing={sorted(required_source_keys - set(cache_inputs.git_repositories))}, "
            f"extra={sorted(set(cache_inputs.git_repositories) - required_source_keys)}"
        )
    repositories: dict[str, tuple[Path, dict[str, Any], dict[tuple[str, str], Path]]] = {}
    cargo_checkout_root = (cache_inputs.cargo_source_home / "git/checkouts").resolve()
    for source_key in sorted(required_source_keys):
        packages = [package for package in lock["git"] if package["source_key"] == source_key]
        revisions = {package["revision"] for package in packages}
        if len(revisions) != 1:
            raise ClosureError(f"Git source key resolves to multiple revisions: {source_key}")
        repository = Path(cache_inputs.git_repositories[source_key]).resolve()
        if enforce_cargo_cache_checkout:
            try:
                repository.relative_to(cargo_checkout_root)
            except ValueError as error:
                raise ClosureError(
                    f"Git repository is not the Cargo checkout used by vendoring: {repository}"
                ) from error
            _require_unique_cargo_checkout(
                cache_inputs.cargo_source_home,
                repository,
                next(iter(revisions)),
                cache_inputs.git_executable,
            )
        record = _git_repository_record(
            repository, next(iter(revisions)), cache_inputs.git_executable
        )
        repositories[source_key] = (repository, record, _package_manifest_roots(repository))
    git_records: list[dict[str, Any]] = []
    for package in lock["git"]:
        repository, repository_record, manifests = repositories[package["source_key"]]
        package_root = manifests.get((package["name"], package["version"]))
        if package_root is None:
            raise ClosureError(
                f"locked Git package is absent at its revision: {package['vendor_name']}"
            )
        git_records.append(
            _validate_git_vendor(
                package,
                repository,
                repository_record,
                package_root,
                vendor_root / package["vendor_name"],
            )
        )
    return {
        "lock": lock,
        "registry_packages": registry_records,
        "git_packages": git_records,
        "vendor": tree_inventory(vendor_root),
    }


def _tool_output(path: Path, arguments: Sequence[str], environment: Mapping[str, str], where: str) -> str:
    completed = subprocess.run(
        [str(path), *arguments],
        env=dict(environment),
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    output = completed.stdout.strip()
    if completed.returncode != 0 or not output:
        raise ClosureError(f"{where} did not return an identity")
    return output


def _tool_record(path: Path, version_arguments: Sequence[str], environment: Mapping[str, str]) -> dict[str, Any]:
    if path.is_symlink():
        # User-facing tool paths may be symlinks, but both the path and resolved
        # executable are bound.  Rustup proxies are rejected separately.
        pass
    resolved = path.resolve()
    if not path.exists() or not resolved.is_file() or not os.access(resolved, os.X_OK):
        raise ClosureError(f"formal build tool is absent or non-executable: {path}")
    return {
        "path": str(path),
        "resolved_path": str(resolved),
        "sha256": sha256_file(resolved),
        "size_bytes": resolved.stat().st_size,
        "version": _tool_output(path, version_arguments, environment, f"tool {path}"),
    }


def _command_bytes(
    command: Sequence[str],
    environment: Mapping[str, str],
    where: str,
    *,
    input_bytes: bytes | None = None,
) -> tuple[bytes, bytes]:
    completed = subprocess.run(
        list(command),
        env=dict(environment),
        input=input_bytes,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        raise ClosureError(f"{where} failed: {detail}")
    return completed.stdout, completed.stderr


def _text_output_record(command: Sequence[str], stdout: bytes, stderr: bytes) -> dict[str, Any]:
    try:
        stdout_text = stdout.decode("utf-8")
        stderr_text = stderr.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ClosureError(f"compiler identity output is not UTF-8: {error}") from error
    return {
        "command": list(command),
        "stdout": stdout_text,
        "stdout_sha256": sha256_bytes(stdout),
        "stderr": stderr_text,
        "stderr_sha256": sha256_bytes(stderr),
    }


def _external_file_identity(path: Path, where: str) -> dict[str, Any]:
    if not path.is_absolute() or (not path.exists() and not path.is_symlink()):
        raise ClosureError(f"{where} is absent or not absolute: {path}")
    resolved = path.resolve()
    if not resolved.is_file():
        raise ClosureError(f"{where} does not resolve to a regular file: {path}")
    return {
        "path": str(path),
        "resolved_path": str(resolved),
        "symlink_target": os.readlink(path) if path.is_symlink() else None,
        "sha256": sha256_file(resolved),
        "size_bytes": resolved.stat().st_size,
        "mode": stat.S_IMODE(resolved.stat().st_mode),
    }


def _verify_external_file_identity(value: Any, where: str, *, portable: bool) -> None:
    record = _expect_keys(
        value,
        {"path", "resolved_path", "symlink_target", "sha256", "size_bytes", "mode"},
        where,
    )
    if (
        not isinstance(record["path"], str)
        or not Path(record["path"]).is_absolute()
        or not isinstance(record["resolved_path"], str)
        or not Path(record["resolved_path"]).is_absolute()
        or record["symlink_target"] is not None
        and not isinstance(record["symlink_target"], str)
    ):
        raise ClosureError(f"{where} path identity is invalid")
    _sha256(record["sha256"], f"{where}.sha256")
    _integer(record["size_bytes"], f"{where}.size_bytes")
    _integer(record["mode"], f"{where}.mode")
    if portable:
        return
    if _external_file_identity(Path(record["path"]), where) != record:
        raise ClosureError(f"{where} changed")


def _gcc_search_directory_records(
    raw: str, sysroot: str
) -> dict[str, list[dict[str, Any]]]:
    parsed: dict[str, list[dict[str, Any]]] = {
        "install": [],
        "programs": [],
        "libraries": [],
    }
    seen: set[tuple[str, str]] = set()
    for line in raw.splitlines():
        if ":" not in line:
            continue
        label, value = line.split(":", 1)
        label = label.strip()
        if label not in parsed:
            continue
        for raw_path in value.strip().split(os.pathsep):
            if not raw_path:
                continue
            reported = raw_path
            if raw_path.startswith("="):
                base = Path(sysroot or "/")
                path = base / raw_path[1:].lstrip("/")
            else:
                path = Path(raw_path)
            if not path.is_absolute():
                raise ClosureError(f"GCC search directory is not absolute: {reported}")
            resolved = path.resolve()
            key = (label, str(resolved))
            if key in seen:
                continue
            seen.add(key)
            if not resolved.exists():
                parsed[label].append(
                    {
                        "reported_path": reported,
                        "resolved_path": str(resolved),
                        "kind": "absent",
                        "tree": None,
                    }
                )
            elif resolved.is_dir() and not resolved.is_symlink():
                parsed[label].append(
                    {
                        "reported_path": reported,
                        "resolved_path": str(resolved),
                        "kind": "directory",
                        "tree": tree_inventory(
                            resolved, allow_symlinks=True, allow_empty=True
                        ),
                    }
                )
            else:
                raise ClosureError(f"GCC search root is not a safe directory: {reported}")
    if not parsed["programs"] or not parsed["libraries"]:
        raise ClosureError("GCC did not report closed program/library search directories")
    return parsed


def _gcc_include_roots(stderr_text: str) -> list[str]:
    marker = "#include <...> search starts here:"
    end = "End of search list."
    if marker not in stderr_text or end not in stderr_text:
        raise ClosureError("GCC verbose preprocessor output omits its include search list")
    body = stderr_text.split(marker, 1)[1].split(end, 1)[0]
    roots: list[str] = []
    for line in body.splitlines():
        value = line.strip()
        if not value or value.startswith("ignoring "):
            continue
        value = value.removesuffix(" (framework directory)")
        path = Path(value)
        if not path.is_absolute() or not path.resolve().is_dir():
            raise ClosureError(f"GCC include search root is absent or unsafe: {value}")
        resolved = str(path.resolve())
        if resolved not in roots:
            roots.append(resolved)
    if not roots:
        raise ClosureError("GCC include search list is empty")
    return roots


def capture_c_system_closure(
    cc: Path, environment: Mapping[str, str]
) -> dict[str, Any]:
    """Capture effective GCC specs, searched trees, and exact link inputs."""

    commands = {
        "target": [str(cc), "-dumpmachine"],
        "version": [str(cc), "-dumpfullversion", "-dumpversion"],
        "sysroot": [str(cc), "-print-sysroot"],
        "specs": [str(cc), "-dumpspecs"],
        "search_dirs": [str(cc), "-print-search-dirs"],
        "includes": [str(cc), "-E", "-x", "c", "-", "-v"],
    }
    outputs: dict[str, dict[str, Any]] = {}
    for role, command in commands.items():
        stdout, stderr = _command_bytes(
            command,
            environment,
            f"GCC {role}",
            input_bytes=b"" if role == "includes" else None,
        )
        outputs[role] = _text_output_record(command, stdout, stderr)
    target = outputs["target"]["stdout"].strip()
    version = outputs["version"]["stdout"].strip()
    sysroot = outputs["sysroot"]["stdout"].strip()
    if not target or not version or (sysroot and not Path(sysroot).is_absolute()):
        raise ClosureError("GCC target/version/sysroot identity is invalid")
    search_directories = _gcc_search_directory_records(
        outputs["search_dirs"]["stdout"], sysroot
    )
    include_roots = _gcc_include_roots(outputs["includes"]["stderr"])
    includes = [
        {
            "path": root,
            "tree": tree_inventory(
                Path(root), allow_symlinks=True, allow_empty=True
            ),
        }
        for root in include_roots
    ]
    link_inputs: list[dict[str, Any]] = []
    for name in C_LINK_INPUT_NAMES:
        command = [str(cc), f"-print-file-name={name}"]
        stdout, stderr = _command_bytes(command, environment, f"GCC link input {name}")
        output = stdout.decode("utf-8").strip()
        query = _text_output_record(command, stdout, stderr)
        if not output or output == name:
            link_inputs.append({"name": name, "status": "absent", "query": query, "file": None})
            continue
        path = Path(output)
        link_inputs.append(
            {
                "name": name,
                "status": "present",
                "query": query,
                "file": _external_file_identity(path, f"GCC link input {name}"),
            }
        )
    required_any = {
        "crti.o",
        "crtbeginS.o",
        "crtendS.o",
        "crtn.o",
        "libgcc.a",
        "libc.so",
    }
    present = {item["name"] for item in link_inputs if item["status"] == "present"}
    if not required_any.issubset(present):
        raise ClosureError(f"GCC link-input closure is incomplete: {sorted(required_any - present)}")
    return {
        "schema_version": 1,
        "receipt_type": "formal-c-system-closure-v1",
        "environment": dict(environment),
        "target": target,
        "version": version,
        "sysroot": sysroot,
        "outputs": outputs,
        "search_directories": search_directories,
        "include_roots": includes,
        "link_inputs": link_inputs,
    }


def _verify_text_output_record(value: Any, where: str) -> None:
    record = _expect_keys(
        value,
        {"command", "stdout", "stdout_sha256", "stderr", "stderr_sha256"},
        where,
    )
    if not isinstance(record["command"], list) or not all(
        isinstance(item, str) for item in record["command"]
    ):
        raise ClosureError(f"{where}.command is invalid")
    for stream in ("stdout", "stderr"):
        if not isinstance(record[stream], str) or sha256_bytes(
            record[stream].encode("utf-8")
        ) != _sha256(record[f"{stream}_sha256"], f"{where}.{stream}_sha256"):
            raise ClosureError(f"{where}.{stream} identity changed")


def verify_c_system_closure(
    value: Any,
    *,
    cc: Path,
    environment: Mapping[str, str],
    portable: bool,
) -> Mapping[str, Any]:
    record = _expect_keys(
        value,
        {
            "schema_version",
            "receipt_type",
            "environment",
            "target",
            "version",
            "sysroot",
            "outputs",
            "search_directories",
            "include_roots",
            "link_inputs",
        },
        "formal C system closure",
    )
    if (
        record["schema_version"] != 1
        or record["receipt_type"] != "formal-c-system-closure-v1"
        or record["environment"] != dict(environment)
    ):
        raise ClosureError("formal C system closure identity/environment changed")
    outputs = _expect_keys(
        record["outputs"],
        {"target", "version", "sysroot", "specs", "search_dirs", "includes"},
        "formal C command outputs",
    )
    for role, output in outputs.items():
        _verify_text_output_record(output, f"formal C {role} output")
    if (
        record["target"] != outputs["target"]["stdout"].strip()
        or record["version"] != outputs["version"]["stdout"].strip()
        or record["sysroot"] != outputs["sysroot"]["stdout"].strip()
    ):
        raise ClosureError("formal C target/version/sysroot binding changed")
    searches = _expect_keys(
        record["search_directories"], {"install", "programs", "libraries"}, "GCC search dirs"
    )
    for category, roots in searches.items():
        if not isinstance(roots, list):
            raise ClosureError(f"GCC {category} search roots must be an array")
        for index, root_value in enumerate(roots):
            root = _expect_keys(
                root_value,
                {"reported_path", "resolved_path", "kind", "tree"},
                f"GCC {category} root[{index}]",
            )
            if root["kind"] == "directory":
                if portable:
                    tree = _expect_keys(
                        root["tree"], {"sha256", "entries", "entry_count", "size_bytes"},
                        f"GCC {category} root tree[{index}]",
                    )
                    if sha256_bytes(canonical_json_bytes(tree["entries"])) != tree["sha256"]:
                        raise ClosureError(f"GCC {category} root inventory digest changed")
                    if (
                        not isinstance(tree["entries"], list)
                        or not isinstance(tree["entry_count"], int)
                        or isinstance(tree["entry_count"], bool)
                        or tree["entry_count"] != len(tree["entries"])
                        or not isinstance(tree["size_bytes"], int)
                        or isinstance(tree["size_bytes"], bool)
                        or tree["size_bytes"] < 0
                    ):
                        raise ClosureError(
                            f"GCC {category} root inventory structure changed"
                        )
                else:
                    _verify_tree_record(
                        Path(root["resolved_path"]), root["tree"],
                        f"GCC {category} root[{index}]", allow_symlinks=True,
                        allow_empty=True,
                    )
            elif root["kind"] != "absent" or root["tree"] is not None:
                raise ClosureError(f"GCC {category} root status is invalid")
    if not isinstance(record["include_roots"], list) or not record["include_roots"]:
        raise ClosureError("GCC include-root closure is empty")
    for index, include_value in enumerate(record["include_roots"]):
        include = _expect_keys(include_value, {"path", "tree"}, f"GCC include root[{index}]")
        if portable:
            tree = _expect_keys(
                include["tree"], {"sha256", "entries", "entry_count", "size_bytes"},
                f"GCC include tree[{index}]",
            )
            if sha256_bytes(canonical_json_bytes(tree["entries"])) != tree["sha256"]:
                raise ClosureError("GCC include-root inventory digest changed")
            if (
                not isinstance(tree["entries"], list)
                or not isinstance(tree["entry_count"], int)
                or isinstance(tree["entry_count"], bool)
                or tree["entry_count"] != len(tree["entries"])
                or not isinstance(tree["size_bytes"], int)
                or isinstance(tree["size_bytes"], bool)
                or tree["size_bytes"] < 0
            ):
                raise ClosureError("GCC include-root inventory structure changed")
        else:
            _verify_tree_record(
                Path(include["path"]), include["tree"],
                f"GCC include root[{index}]", allow_symlinks=True,
                allow_empty=True,
            )
    if not isinstance(record["link_inputs"], list) or [
        item.get("name") if isinstance(item, Mapping) else None for item in record["link_inputs"]
    ] != list(C_LINK_INPUT_NAMES):
        raise ClosureError("GCC link-input query set/order changed")
    for index, input_value in enumerate(record["link_inputs"]):
        item = _expect_keys(
            input_value, {"name", "status", "query", "file"}, f"GCC link input[{index}]"
        )
        _verify_text_output_record(item["query"], f"GCC link input query[{index}]")
        if item["status"] == "present":
            _verify_external_file_identity(
                item["file"], f"GCC link input {item['name']}", portable=portable
            )
        elif item["status"] != "absent" or item["file"] is not None:
            raise ClosureError(f"GCC link input status changed: {item['name']}")
    if not portable:
        current = capture_c_system_closure(cc, environment)
        if current != record:
            raise ClosureError("effective GCC/system input closure changed")
    return record


def default_build_tools(*, python3: Path | None = None) -> dict[str, Path]:
    """Resolve the complete Linux build-script tool surface once.

    Formal SCC callers should normally pass an explicit mapping.  This helper
    preserves API compatibility for callers that previously supplied only the
    compiler helpers: resolution happens before the closed environment is
    constructed, and every resolved executable is then hash-bound.
    """

    discovered_python = shutil.which("python3") if python3 is None else str(python3)
    if discovered_python is None:
        raise ClosureError("required Linux build tool is unavailable: python3")
    preferred = {
        "bash": Path("/bin/bash"),
        "python3": Path(discovered_python),
    }
    result: dict[str, Path] = {}
    for role in REQUIRED_BUILD_TOOL_ROLES:
        if role in preferred:
            candidate = preferred[role]
        else:
            resolved = shutil.which(role)
            if resolved is None:
                raise ClosureError(f"required Linux build tool is unavailable: {role}")
            candidate = Path(resolved)
        if not candidate.is_absolute():
            candidate = candidate.resolve()
        result[role] = candidate
    return result


def _closed_extra_tool_mapping(value: Mapping[str, Path] | None) -> dict[str, Path]:
    result = {} if value is None else dict(value)
    for role, path in result.items():
        if re.fullmatch(r"[a-z][a-z0-9_]*", role) is None:
            raise ClosureError(f"extra build tool role is invalid: {role!r}")
        if role in {
            "cargo",
            "rustc",
            *REQUIRED_HELPER_ROLES,
            *REQUIRED_BUILD_TOOL_ROLES,
            *CC_DISCOVERED_ROLES,
        }:
            raise ClosureError(f"extra build tool role collides with a closed role: {role}")
        if not isinstance(path, Path):
            raise ClosureError(f"extra build tool path must be a Path: {role}")
    return result


def _tool_wrapper_bytes(target: str) -> bytes:
    if not Path(target).is_absolute() or "\n" in target or "\r" in target:
        raise ClosureError(f"tool wrapper target is not a safe absolute path: {target!r}")
    return (
        "#!/bin/bash\n"
        "set -euo pipefail\n"
        f"exec -- {shlex.quote(target)} \"$@\"\n"
    ).encode("utf-8")


def capture_toolchain(
    probe_root: Path,
    *,
    cargo: Path,
    rustc: Path,
    helpers: Mapping[str, Path],
    build_tools: Mapping[str, Path] | None = None,
    extra_tools: Mapping[str, Path] | None = None,
) -> dict[str, Any]:
    """Capture actual compiler binaries, rustlib, C helpers, and binutils."""

    if set(helpers) != set(REQUIRED_HELPER_ROLES):
        raise ClosureError("formal helper roles are not closed")
    resolved_build_tools = (
        default_build_tools() if build_tools is None else dict(build_tools)
    )
    if set(resolved_build_tools) != set(REQUIRED_BUILD_TOOL_ROLES):
        raise ClosureError("formal build-script tool roles are not closed")
    for role, path in resolved_build_tools.items():
        if not isinstance(path, Path):
            raise ClosureError(f"build-script tool path must be a Path: {role}")
    if resolved_build_tools["bash"] != Path("/bin/bash"):
        raise ClosureError(
            "build-script bash must be /bin/bash because the retained loader uses that shebang"
        )
    resolved_extra_tools = _closed_extra_tool_mapping(extra_tools)
    all_input_paths = [
        cargo,
        rustc,
        *helpers.values(),
        *resolved_build_tools.values(),
        *resolved_extra_tools.values(),
    ]
    if any(not isinstance(path, Path) or not path.is_absolute() for path in all_input_paths):
        raise ClosureError("all formal compiler/build tool paths must be absolute")
    if probe_root.exists() or probe_root.is_symlink():
        raise ClosureError(f"toolchain probe root already exists: {probe_root}")
    (probe_root / "home").mkdir(parents=True)
    (probe_root / "tmp").mkdir()
    ordered_probe_paths = [
        cargo,
        rustc,
        *(helpers[role] for role in REQUIRED_HELPER_ROLES),
        *(resolved_build_tools[role] for role in REQUIRED_BUILD_TOOL_ROLES),
        *(resolved_extra_tools[role] for role in sorted(resolved_extra_tools)),
    ]
    directories: list[str] = []
    for path in ordered_probe_paths:
        parent = str(path.resolve().parent)
        if parent not in directories:
            directories.append(parent)
    environment = {
        "HOME": str(probe_root / "home"),
        "TMPDIR": str(probe_root / "tmp"),
        "PATH": ":".join(directories),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
    }
    sysroot = Path(_tool_output(rustc, ["--print", "sysroot"], environment, "rustc sysroot")).resolve()
    actual_rustc = sysroot / "bin/rustc"
    actual_cargo = sysroot / "bin/cargo"
    if rustc.resolve() != actual_rustc.resolve() or cargo.resolve() != actual_cargo.resolve():
        raise ClosureError("cargo/rustc must be actual sysroot binaries, not rustup proxies")
    rust_tools = {
        "cargo": _tool_record(cargo, ["--version", "--verbose"], environment),
        "rustc": _tool_record(rustc, ["-vV"], environment),
    }
    helper_records = {
        role: _tool_record(path, ["--version"], environment)
        for role, path in sorted(helpers.items())
    }
    build_tool_records = {
        role: _tool_record(path, ["--version"], environment)
        for role, path in sorted(resolved_build_tools.items())
    }
    if build_tool_records["nm"] != helper_records["nm"]:
        raise ClosureError("build-script nm differs from the compiler/binutils nm")
    extra_tool_records = {
        role: _tool_record(path, ["--version"], environment)
        for role, path in sorted(resolved_extra_tools.items())
    }
    cc = helpers["cc"]
    discovered_arguments = {
        "cc1": ["-print-prog-name=cc1"],
        "collect2": ["-print-prog-name=collect2"],
        "assembler": ["-print-prog-name=as"],
        "cc_ld": ["-print-prog-name=ld"],
    }
    discovered: dict[str, dict[str, Any]] = {}
    for role, arguments in discovered_arguments.items():
        output = _tool_output(cc, arguments, environment, f"cc helper {role}").splitlines()[-1]
        candidate = Path(output)
        if not candidate.is_absolute():
            matches = [Path(directory) / candidate for directory in directories]
            existing = [match for match in matches if match.exists()]
            if len(existing) != 1:
                raise ClosureError(f"cc helper {role} did not resolve uniquely: {output}")
            candidate = existing[0]
        discovered[role] = _tool_record(candidate, ["--version"], environment)
    c_system_closure = capture_c_system_closure(cc, environment)
    invocation_targets: dict[str, str] = {
        "cargo": rust_tools["cargo"]["resolved_path"],
        "rustc": rust_tools["rustc"]["resolved_path"],
        "cc": helper_records["cc"]["resolved_path"],
        "ar": helper_records["ar"]["resolved_path"],
        "ranlib": helper_records["ranlib"]["resolved_path"],
        "cc1": discovered["cc1"]["resolved_path"],
        "collect2": discovered["collect2"]["resolved_path"],
        "as": discovered["assembler"]["resolved_path"],
        "ld": discovered["cc_ld"]["resolved_path"],
    }
    for role, tool_record in [*build_tool_records.items(), *extra_tool_records.items()]:
        target = tool_record["resolved_path"]
        if role in invocation_targets and invocation_targets[role] != target:
            raise ClosureError(f"PATH tool role resolves to conflicting executables: {role}")
        invocation_targets[role] = target
    tool_bin = probe_root / "tool-bin"
    tool_bin.mkdir()
    for name, target in sorted(invocation_targets.items()):
        destination = tool_bin / name
        _write_new(destination, _tool_wrapper_bytes(target), mode=0o555)
    tool_bin_record = {
        "path": str(tool_bin),
        **tree_inventory(tool_bin, allow_symlinks=False),
    }
    tool_bin.chmod(0o555)
    rustlib = sysroot / "lib/rustlib"
    sysroot_record = {
        "path": str(sysroot),
        **tree_inventory(sysroot, allow_symlinks=True),
    }
    rustlib_record = {
        "path": str(rustlib),
        **tree_inventory(rustlib, allow_symlinks=True),
    }
    return {
        "schema_version": SCHEMA_VERSION,
        "receipt_type": TOOLCHAIN_RECEIPT_TYPE,
        "probe_environment": environment,
        "rust_tools": rust_tools,
        "helpers": helper_records,
        "build_tools": build_tool_records,
        "extra_tools": extra_tool_records,
        "cc_discovered_helpers": discovered,
        "c_system_closure": c_system_closure,
        "tool_bin": tool_bin_record,
        "sysroot": sysroot_record,
        "rustlib": rustlib_record,
        "base_image_trust_boundary": compiler_base_image_trust_boundary(),
    }


def _tool_path(record: Mapping[str, Any]) -> Path:
    return Path(str(record["resolved_path"]))


def closed_build_environment(
    run_root: Path,
    closure_root: Path,
    target_root: Path,
    toolchain: Mapping[str, Any],
    *,
    slots: int,
    build_variables: Mapping[str, str],
) -> dict[str, str]:
    """Return the complete, non-inherited environment for the formal build."""

    if isinstance(slots, bool) or not isinstance(slots, int) or slots <= 0:
        raise ClosureError("formal build slots must be a positive integer")
    if not all(isinstance(key, str) and isinstance(value, str) for key, value in build_variables.items()):
        raise ClosureError("formal build variables must be a string mapping")
    collision = RESERVED_BUILD_ENVIRONMENT & set(build_variables)
    if collision:
        raise ClosureError(f"formal build variables override reserved fields: {sorted(collision)}")
    closure = _descendant(run_root, closure_root, "closure root")
    target = _descendant(run_root, target_root, "target root")
    rust_tools = toolchain["rust_tools"]
    helpers = toolchain["helpers"]
    build_tools = toolchain["build_tools"]
    extra_tools = toolchain["extra_tools"]
    tool_bin = toolchain["tool_bin"]
    environment = {
        "HOME": str(closure / "home"),
        "TMPDIR": str(closure / "tmp"),
        "PATH": str(tool_bin["path"]),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "CARGO_HOME": str(closure / "cargo-home"),
        "CARGO_NET_OFFLINE": "true",
        "CARGO_INCREMENTAL": "0",
        "CARGO_BUILD_JOBS": str(slots),
        "CARGO_TARGET_DIR": str(target),
        "RUSTC": str(_tool_path(rust_tools["rustc"])),
        "CC": str(_tool_path(helpers["cc"])),
        "AR": str(_tool_path(helpers["ar"])),
        "RANLIB": str(_tool_path(helpers["ranlib"])),
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER": str(
            _tool_path(helpers["linker"])
        ),
        "PYTHONNOUSERSITE": "1",
        "PYTHONDONTWRITEBYTECODE": "1",
        "PYTHONHASHSEED": "0",
    }
    environment.update(dict(sorted(build_variables.items())))
    return environment


def materialize_dependency_closure(
    run_root: Path,
    source_root: Path,
    closure_root: Path,
    cache_inputs: SourceCacheInputs,
    *,
    cargo: Path,
) -> dict[str, Any]:
    """Run exact offline Cargo vendoring and retain a self-contained closure."""

    run = run_root.resolve()
    source = _descendant(run, source_root, "source root")
    closure = _descendant(run, closure_root.parent, "closure parent") / closure_root.name
    if closure.exists() or closure.is_symlink():
        raise ClosureError(f"dependency closure root already exists: {closure}")
    cargo_home = closure / "cargo-home"
    vendor = closure / "vendor"
    home = closure / "vendor-home"
    tmp = closure / "vendor-tmp"
    cargo_home.mkdir(parents=True)
    home.mkdir()
    tmp.mkdir()
    lock_path = source / "Cargo.lock"
    lock = load_cargo_lock(lock_path)
    source_keys = sorted({package["source_key"] for package in lock["git"]})
    if set(cache_inputs.git_repositories) != set(source_keys):
        raise ClosureError("Git source inputs do not match Cargo.lock")
    # Audit Git inputs before Cargo reads the source cache.
    for source_key in source_keys:
        revision = next(
            package["revision"] for package in lock["git"] if package["source_key"] == source_key
        )
        _git_repository_record(
            Path(cache_inputs.git_repositories[source_key]),
            revision,
            cache_inputs.git_executable,
        )
    cargo_resolved = cargo.resolve()
    if not cargo_resolved.is_file() or not os.access(cargo_resolved, os.X_OK):
        raise ClosureError(f"Cargo executable is absent: {cargo}")
    environment = {
        "HOME": str(home),
        "TMPDIR": str(tmp),
        "PATH": str(cargo_resolved.parent),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "CARGO_HOME": str(cache_inputs.cargo_source_home.resolve()),
        "CARGO_NET_OFFLINE": "true",
    }
    command = [
        str(cargo_resolved),
        "vendor",
        "--locked",
        "--offline",
        "--versioned-dirs",
        os.path.relpath(vendor, source),
    ]
    completed = subprocess.run(
        command,
        cwd=source,
        env=environment,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    log = closure / "cargo-vendor.log"
    _write_new(log, completed.stderr)
    stdout = closure / "cargo-vendor.stdout"
    _write_new(stdout, completed.stdout)
    if completed.returncode != 0 or not vendor.is_dir():
        raise ClosureError("offline cargo vendor failed; see retained vendor log")
    # Replace registry output with independent extraction from the exact
    # lock-checksummed archives.  Git output remains Cargo-normalized, but is
    # validated against the exact object-backed clean worktree below.
    for package in lock["registry"]:
        destination = vendor / package["vendor_name"]
        if destination.exists():
            shutil.rmtree(destination)
        archive = _find_registry_archive(package, cache_inputs.registry_archive_roots)
        materialize_registry_package(package, archive, destination)
    _make_tree_read_only(vendor)
    validated = validate_dependency_vendor(lock_path, vendor, cache_inputs)
    config = cargo_home / "config.toml"
    _write_new(config, _cargo_config_bytes(lock))
    config.chmod(0o444)
    # Recheck source inputs after Cargo has finished to close mutation races.
    validated_after = validate_dependency_vendor(lock_path, vendor, cache_inputs)
    if validated != validated_after:
        raise ClosureError("dependency source inputs changed during vendoring")
    return {
        "schema_version": SCHEMA_VERSION,
        "receipt_type": DEPENDENCY_RECEIPT_TYPE,
        "lock": {
            "path": lock_path.relative_to(run).as_posix(),
            "sha256": lock["sha256"],
            "size_bytes": lock["size_bytes"],
            "lock_version": lock["lock_version"],
            "registry_count": len(lock["registry"]),
            "git_count": len(lock["git"]),
            "workspace_count": len(lock["workspace"]),
        },
        "command": command,
        "cwd": source.relative_to(run).as_posix(),
        "source_environment": environment,
        "cargo": _tool_record(cargo_resolved, ["--version", "--verbose"], environment),
        "config": _relative_file_record(run, config),
        "vendor_log": _relative_file_record(run, log),
        "vendor_stdout": _relative_file_record(run, stdout),
        "registry_packages": validated["registry_packages"],
        "git_packages": validated["git_packages"],
        "vendor": {
            "path": vendor.relative_to(run).as_posix(),
            **validated["vendor"],
        },
    }


def prepare_formal_rust_build_closure(
    run_root: Path,
    source_root: Path,
    closure_root: Path,
    target_root: Path,
    cache_inputs: SourceCacheInputs,
    *,
    cargo: Path,
    rustc: Path,
    helpers: Mapping[str, Path],
    build_tools: Mapping[str, Path] | None = None,
    extra_tools: Mapping[str, Path] | None = None,
    slots: int,
    build_variables: Mapping[str, str],
) -> dict[str, Any]:
    """Materialize one complete dependency/toolchain/environment receipt."""

    dependency = materialize_dependency_closure(
        run_root, source_root, closure_root, cache_inputs, cargo=cargo
    )
    closure = closure_root.resolve()
    (closure / "home").mkdir()
    (closure / "tmp").mkdir()
    target_root.mkdir(parents=True, exist_ok=False)
    toolchain = capture_toolchain(
        closure / "toolchain-probe",
        cargo=cargo,
        rustc=rustc,
        helpers=helpers,
        build_tools=build_tools,
        extra_tools=extra_tools,
    )
    environment = closed_build_environment(
        run_root,
        closure,
        target_root,
        toolchain,
        slots=slots,
        build_variables=build_variables,
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "receipt_type": RECEIPT_TYPE,
        "run_root_name": run_root.resolve().name,
        "source_root": source_root.resolve().relative_to(run_root.resolve()).as_posix(),
        "closure_root": closure.relative_to(run_root.resolve()).as_posix(),
        "target_root": target_root.resolve().relative_to(run_root.resolve()).as_posix(),
        "slots": slots,
        "build_variables": dict(sorted(build_variables.items())),
        "dependencies": dependency,
        "toolchain": toolchain,
        "environment": environment,
        "complete_sentinel": COMPLETE_SENTINEL,
    }


def _verify_tool_record(value: Any, where: str, *, portable: bool) -> None:
    record = _expect_keys(
        value, {"path", "resolved_path", "sha256", "size_bytes", "version"}, where
    )
    if not all(isinstance(record[field], str) and record[field] for field in ("path", "resolved_path", "version")):
        raise ClosureError(f"{where} text identity is invalid")
    _sha256(record["sha256"], f"{where}.sha256")
    _integer(record["size_bytes"], f"{where}.size_bytes", minimum=1)
    if portable:
        return
    path = Path(record["path"])
    resolved = path.resolve()
    if (
        not resolved.is_file()
        or str(resolved) != record["resolved_path"]
        or resolved.stat().st_size != record["size_bytes"]
        or sha256_file(resolved) != record["sha256"]
    ):
        raise ClosureError(f"{where} executable changed")


def verify_toolchain_receipt(value: Any, *, portable: bool) -> Mapping[str, Any]:
    record = _expect_keys(
        value,
        {
            "schema_version",
            "receipt_type",
            "probe_environment",
            "rust_tools",
            "helpers",
            "build_tools",
            "extra_tools",
            "cc_discovered_helpers",
            "c_system_closure",
            "tool_bin",
            "sysroot",
            "rustlib",
            "base_image_trust_boundary",
        },
        "formal Rust toolchain receipt",
    )
    if record["schema_version"] != SCHEMA_VERSION or record["receipt_type"] != TOOLCHAIN_RECEIPT_TYPE:
        raise ClosureError("formal Rust toolchain receipt identity changed")
    if record["base_image_trust_boundary"] != compiler_base_image_trust_boundary():
        raise ClosureError("compiler base-image trust boundary was omitted or overstated")
    rust_tools = _expect_keys(record["rust_tools"], {"cargo", "rustc"}, "Rust tools")
    helpers = _expect_keys(record["helpers"], set(REQUIRED_HELPER_ROLES), "build helpers")
    build_tools = _expect_keys(
        record["build_tools"], set(REQUIRED_BUILD_TOOL_ROLES), "build-script tools"
    )
    if not isinstance(record["extra_tools"], Mapping):
        raise ClosureError("extra build tools must be an object")
    extra_tools = _expect_keys(
        record["extra_tools"], set(record["extra_tools"]), "extra build tools"
    )
    for role in extra_tools:
        if (
            re.fullmatch(r"[a-z][a-z0-9_]*", role) is None
            or role
            in {
                "cargo",
                "rustc",
                *REQUIRED_HELPER_ROLES,
                *REQUIRED_BUILD_TOOL_ROLES,
                *CC_DISCOVERED_ROLES,
            }
        ):
            raise ClosureError(f"extra build tool role is invalid or colliding: {role}")
    discovered = _expect_keys(
        record["cc_discovered_helpers"], set(CC_DISCOVERED_ROLES), "cc-discovered helpers"
    )
    for name, tool in [
        *rust_tools.items(),
        *helpers.items(),
        *build_tools.items(),
        *extra_tools.items(),
        *discovered.items(),
    ]:
        _verify_tool_record(tool, f"formal tool {name}", portable=portable)
    verify_c_system_closure(
        record["c_system_closure"],
        cc=Path(str(helpers["cc"]["path"])),
        environment=record["probe_environment"],
        portable=portable,
    )
    for role, tool in build_tools.items():
        if not Path(tool["path"]).is_absolute():
            raise ClosureError(f"formal build tool path is not absolute: {role}")
    if build_tools["bash"]["path"] != "/bin/bash":
        raise ClosureError("formal build/loader bash path changed")
    if build_tools["nm"] != helpers["nm"]:
        raise ClosureError("formal build nm differs from compiler/binutils nm")
    tool_bin = _expect_keys(
        record["tool_bin"],
        {"path", "sha256", "entries", "entry_count", "size_bytes"},
        "run-local tool bin",
    )
    if not isinstance(tool_bin["path"], str) or not Path(tool_bin["path"]).is_absolute():
        raise ClosureError("run-local tool bin path is invalid")
    invocation_targets = {
        "cargo": rust_tools["cargo"]["resolved_path"],
        "rustc": rust_tools["rustc"]["resolved_path"],
        "cc": helpers["cc"]["resolved_path"],
        "ar": helpers["ar"]["resolved_path"],
        "ranlib": helpers["ranlib"]["resolved_path"],
        "cc1": discovered["cc1"]["resolved_path"],
        "collect2": discovered["collect2"]["resolved_path"],
        "as": discovered["assembler"]["resolved_path"],
        "ld": discovered["cc_ld"]["resolved_path"],
    }
    for role, tool in [*build_tools.items(), *extra_tools.items()]:
        if role in invocation_targets and invocation_targets[role] != tool["resolved_path"]:
            raise ClosureError(f"run-local PATH role has conflicting tool records: {role}")
        invocation_targets[role] = tool["resolved_path"]
    expected_tool_bin_entries = []
    expected_tool_bin_size = 0
    for role, target in sorted(invocation_targets.items()):
        payload = _tool_wrapper_bytes(target)
        expected_tool_bin_size += len(payload)
        expected_tool_bin_entries.append(
            {
                "path": role,
                "kind": "file",
                "sha256": sha256_bytes(payload),
                "size_bytes": len(payload),
                "mode": 0o555,
            }
        )
    if (
        tool_bin["entries"] != expected_tool_bin_entries
        or tool_bin["entry_count"] != len(expected_tool_bin_entries)
        or tool_bin["size_bytes"] != expected_tool_bin_size
        or tool_bin["sha256"]
        != sha256_bytes(canonical_json_bytes(expected_tool_bin_entries))
    ):
        raise ClosureError("run-local tool bin does not resolve every bare tool exactly")
    if not portable:
        _verify_tree_record(
            Path(tool_bin["path"]),
            {key: tool_bin[key] for key in ("sha256", "entries", "entry_count", "size_bytes")},
            "run-local tool bin",
            allow_symlinks=False,
        )
    sysroot = _expect_keys(
        record["sysroot"],
        {"path", "sha256", "entries", "entry_count", "size_bytes"},
        "Rust sysroot inventory",
    )
    if not isinstance(sysroot["path"], str) or not Path(sysroot["path"]).is_absolute():
        raise ClosureError("Rust sysroot path is invalid")
    if not isinstance(sysroot["entries"], list) or not sysroot["entries"]:
        raise ClosureError("Rust sysroot full inventory is absent")
    if (
        sha256_bytes(canonical_json_bytes(sysroot["entries"])) != sysroot["sha256"]
        or sysroot["entry_count"] != len(sysroot["entries"])
    ):
        raise ClosureError("Rust sysroot inventory identity changed")
    rustlib = _expect_keys(
        record["rustlib"],
        {"path", "sha256", "entries", "entry_count", "size_bytes"},
        "Rust rustlib inventory",
    )
    if rustlib["path"] != str(Path(sysroot["path"]) / "lib/rustlib"):
        raise ClosureError("Rust rustlib path is not below the recorded sysroot")
    if not isinstance(rustlib["entries"], list) or not rustlib["entries"]:
        raise ClosureError("Rust rustlib full inventory is absent")
    if sha256_bytes(canonical_json_bytes(rustlib["entries"])) != rustlib["sha256"]:
        raise ClosureError("Rust rustlib inventory digest changed")
    if rustlib["entry_count"] != len(rustlib["entries"]):
        raise ClosureError("Rust rustlib inventory count changed")
    if not portable:
        _verify_tree_record(
            Path(sysroot["path"]),
            {key: sysroot[key] for key in ("sha256", "entries", "entry_count", "size_bytes")},
            "Rust sysroot",
            allow_symlinks=True,
        )
        _verify_tree_record(Path(rustlib["path"]), {key: rustlib[key] for key in ("sha256", "entries", "entry_count", "size_bytes")}, "Rust rustlib", allow_symlinks=True)
    return record


def _verify_package_vendor_record(
    vendor_root: Path, value: Any, where: str, *, git: bool
) -> Mapping[str, Any]:
    base_keys = {
        "name",
        "version",
        "source",
        "vendor_name",
        "vendor",
    }
    if git:
        keys = base_keys | {
            "source_key",
            "repository_url",
            "revision",
            "repository",
            "package_root",
            "source_manifest_sha256",
            "normalized_manifest_sha256",
        }
    else:
        keys = base_keys | {
            "checksum",
            "archive_path",
            "archive_sha256",
            "archive_size_bytes",
        }
    record = _expect_keys(value, keys, where)
    if not all(isinstance(record[key], str) and record[key] for key in ("name", "version", "source", "vendor_name")):
        raise ClosureError(f"{where} identity is invalid")
    directory = _retained_path(
        vendor_root,
        record["vendor_name"],
        f"{where} vendor path",
        leaf_kind="directory",
    )
    _verify_tree_record(
        directory,
        record["vendor"],
        f"{where} vendor",
        allow_symlinks=False,
        containment_root=vendor_root,
    )
    if git:
        if HEX_GIT_COMMIT.fullmatch(str(record["revision"])) is None:
            raise ClosureError(f"{where} revision is invalid")
        _sha256(record["source_manifest_sha256"], f"{where}.source_manifest_sha256")
        normalized_digest = _sha256(
            record["normalized_manifest_sha256"], f"{where}.normalized_manifest_sha256"
        )
        if sha256_file(directory / "Cargo.toml") != normalized_digest:
            raise ClosureError(f"{where} normalized manifest changed")
        repository = _expect_keys(
            record["repository"],
            {
                "path",
                "revision",
                "tree",
                "git",
                "git_environment",
                "tracked_tree",
                "cargo_cache_marker",
            },
            f"{where} repository",
        )
        if (
            not isinstance(repository["path"], str)
            or not Path(repository["path"]).is_absolute()
            or repository["revision"] != record["revision"]
            or not isinstance(repository["tree"], str)
            or re.fullmatch(r"[0-9a-f]{40,64}", repository["tree"]) is None
            or not isinstance(repository["cargo_cache_marker"], bool)
        ):
            raise ClosureError(f"{where} repository identity is invalid")
        git_tool = _expect_keys(
            repository["git"],
            {"path", "resolved_path", "sha256", "size_bytes"},
            f"{where} Git tool",
        )
        if not all(isinstance(git_tool[field], str) and git_tool[field] for field in ("path", "resolved_path")):
            raise ClosureError(f"{where} Git tool paths are invalid")
        _sha256(git_tool["sha256"], f"{where} Git tool SHA-256")
        _integer(git_tool["size_bytes"], f"{where} Git tool size", minimum=1)
        if repository["git_environment"] != _git_environment(
            Path(str(git_tool["resolved_path"]))
        ):
            raise ClosureError(f"{where} closed Git environment changed")
        tracked = _expect_keys(
            repository["tracked_tree"],
            {"sha256", "entries", "entry_count", "size_bytes"},
            f"{where} tracked Git tree",
        )
        if not isinstance(tracked["entries"], list) or not tracked["entries"]:
            raise ClosureError(f"{where} tracked Git inventory is empty")
        if tracked["entry_count"] != len(tracked["entries"]):
            raise ClosureError(f"{where} tracked Git inventory count changed")
        _integer(tracked["size_bytes"], f"{where} tracked Git size")
        if sha256_bytes(canonical_json_bytes(tracked["entries"])) != tracked["sha256"]:
            raise ClosureError(f"{where} tracked Git inventory digest changed")
        tracked_by_path: dict[str, Mapping[str, Any]] = {}
        for index, entry_value in enumerate(tracked["entries"]):
            entry = _expect_keys(
                entry_value,
                {"path", "sha256", "size_bytes"},
                f"{where} tracked Git entry[{index}]",
            )
            relative = _safe_relative(entry["path"], f"{where} tracked Git path")
            if relative in tracked_by_path:
                raise ClosureError(f"{where} repeats a tracked Git path")
            _sha256(entry["sha256"], f"{where} tracked Git entry SHA-256")
            _integer(entry["size_bytes"], f"{where} tracked Git entry size")
            tracked_by_path[relative] = entry
        package_root = record["package_root"]
        if package_root == ".":
            manifest_path = "Cargo.toml"
        else:
            _safe_relative(package_root, f"{where}.package_root")
            manifest_path = f"{package_root}/Cargo.toml"
        if (
            manifest_path not in tracked_by_path
            or tracked_by_path[manifest_path]["sha256"] != record["source_manifest_sha256"]
        ):
            raise ClosureError(f"{where} source manifest is not bound to its Git tree")
        _checksum_document(directory, expected_package=None)
    else:
        checksum = _sha256(record["checksum"], f"{where}.checksum")
        if _sha256(record["archive_sha256"], f"{where}.archive_sha256") != checksum:
            raise ClosureError(f"{where} archive differs from Cargo.lock checksum")
        _integer(record["archive_size_bytes"], f"{where}.archive_size_bytes", minimum=1)
        _checksum_document(directory, expected_package=checksum)
    return record


def verify_dependency_receipt(
    run_root: Path,
    value: Any,
    *,
    portable: bool,
    cache_inputs: SourceCacheInputs | None = None,
) -> Mapping[str, Any]:
    run_root = _retained_run_root(run_root, "dependency run root")
    record = _expect_keys(
        value,
        {
            "schema_version",
            "receipt_type",
            "lock",
            "command",
            "cwd",
            "source_environment",
            "cargo",
            "config",
            "vendor_log",
            "vendor_stdout",
            "registry_packages",
            "git_packages",
            "vendor",
        },
        "formal dependency receipt",
    )
    if record["schema_version"] != SCHEMA_VERSION or record["receipt_type"] != DEPENDENCY_RECEIPT_TYPE:
        raise ClosureError("formal dependency receipt identity changed")
    lock_record = _expect_keys(
        record["lock"],
        {
            "path",
            "sha256",
            "size_bytes",
            "lock_version",
            "registry_count",
            "git_count",
            "workspace_count",
        },
        "Cargo.lock receipt",
    )
    lock_path = _retained_path(
        run_root,
        lock_record["path"],
        "Cargo.lock receipt path",
        leaf_kind="file",
    )
    lock = load_cargo_lock(lock_path)
    expected_lock = {
        "path": lock_record["path"],
        "sha256": lock["sha256"],
        "size_bytes": lock["size_bytes"],
        "lock_version": lock["lock_version"],
        "registry_count": len(lock["registry"]),
        "git_count": len(lock["git"]),
        "workspace_count": len(lock["workspace"]),
    }
    if lock_record != expected_lock:
        raise ClosureError("Cargo.lock receipt changed")
    for field in ("config", "vendor_log", "vendor_stdout"):
        _verify_relative_file(run_root, record[field], f"dependency {field}")
    vendor_record = _expect_keys(
        record["vendor"],
        {"path", "sha256", "entries", "entry_count", "size_bytes"},
        "vendor tree receipt",
    )
    vendor_root = _retained_path(
        run_root, vendor_record["path"], "vendor path", leaf_kind="directory"
    )
    _verify_tree_record(
        vendor_root,
        {key: vendor_record[key] for key in ("sha256", "entries", "entry_count", "size_bytes")},
        "retained vendor tree",
        allow_symlinks=False,
        containment_root=run_root,
    )
    registry_values = record["registry_packages"]
    git_values = record["git_packages"]
    if not isinstance(registry_values, list) or not isinstance(git_values, list):
        raise ClosureError("dependency package receipts must be arrays")
    registry_records = [
        _verify_package_vendor_record(vendor_root, item, f"registry package[{index}]", git=False)
        for index, item in enumerate(registry_values)
    ]
    git_records = [
        _verify_package_vendor_record(vendor_root, item, f"Git package[{index}]", git=True)
        for index, item in enumerate(git_values)
    ]
    expected_registry = [package["vendor_name"] for package in lock["registry"]]
    expected_git = [package["vendor_name"] for package in lock["git"]]
    if [item["vendor_name"] for item in registry_records] != expected_registry:
        raise ClosureError("registry package receipt order/identity changed")
    if [item["vendor_name"] for item in git_records] != expected_git:
        raise ClosureError("Git package receipt order/identity changed")
    for package, package_record in zip(lock["registry"], registry_records, strict=True):
        for field in ("name", "version", "source", "checksum", "vendor_name"):
            if package_record[field] != package[field]:
                raise ClosureError(f"registry package lock identity changed: {package['vendor_name']}")
    for package, package_record in zip(lock["git"], git_records, strict=True):
        for field in (
            "name",
            "version",
            "source",
            "source_key",
            "repository_url",
            "revision",
            "vendor_name",
        ):
            if package_record[field] != package[field]:
                raise ClosureError(f"Git package lock identity changed: {package['vendor_name']}")
    config_path = _retained_path(
        run_root,
        record["config"]["path"],
        "dependency config path",
        leaf_kind="file",
    )
    if config_path.read_bytes() != _cargo_config_bytes(lock):
        raise ClosureError("run-local Cargo config is not the exact offline source replacement")
    _verify_tool_record(record["cargo"], "dependency Cargo executable", portable=portable)
    if not isinstance(record["command"], list) or record["command"] != [
        record["cargo"]["resolved_path"],
        "vendor",
        "--locked",
        "--offline",
        "--versioned-dirs",
        os.path.relpath(
            vendor_root,
            _retained_path(
                run_root, record["cwd"], "vendor cwd", leaf_kind="directory"
            ),
        ),
    ]:
        raise ClosureError("offline cargo vendor argv changed")
    source_environment = _expect_keys(
        record["source_environment"],
        {"HOME", "TMPDIR", "PATH", "LANG", "LC_ALL", "TZ", "CARGO_HOME", "CARGO_NET_OFFLINE"},
        "cargo vendor environment",
    )
    if (
        source_environment["LANG"] != "C"
        or source_environment["LC_ALL"] != "C"
        or source_environment["TZ"] != "UTC"
        or source_environment["CARGO_NET_OFFLINE"] != "true"
        or source_environment["PATH"] != str(Path(record["cargo"]["resolved_path"]).parent)
        or not all(
            isinstance(source_environment[field], str) and Path(source_environment[field]).is_absolute()
            for field in ("HOME", "TMPDIR", "CARGO_HOME")
        )
    ):
        raise ClosureError("cargo vendor environment is not closed")
    if not portable:
        if cache_inputs is None:
            raise ClosureError("local dependency replay requires explicit source-cache inputs")
        if source_environment["CARGO_HOME"] != str(cache_inputs.cargo_source_home.resolve()):
            raise ClosureError("cargo vendor source cache path changed")
        validated = validate_dependency_vendor(lock_path, vendor_root, cache_inputs)
        if validated["registry_packages"] != registry_values or validated["git_packages"] != git_values:
            raise ClosureError("external dependency sources differ from the retained receipt")
    return record


def verify_formal_rust_build_closure(
    run_root: Path,
    value: Any,
    *,
    portable: bool,
    cache_inputs: SourceCacheInputs | None = None,
) -> Mapping[str, Any]:
    """Strictly replay a complete formal Rust build closure receipt."""

    run_root = _retained_run_root(run_root, "formal build run root")
    record = _expect_keys(
        value,
        {
            "schema_version",
            "receipt_type",
            "run_root_name",
            "source_root",
            "closure_root",
            "target_root",
            "slots",
            "build_variables",
            "dependencies",
            "toolchain",
            "environment",
            "complete_sentinel",
        },
        "formal Rust build closure",
    )
    if (
        record["schema_version"] != SCHEMA_VERSION
        or record["receipt_type"] != RECEIPT_TYPE
        or record["run_root_name"] != run_root.resolve().name
        or record["complete_sentinel"] != COMPLETE_SENTINEL
    ):
        raise ClosureError("formal Rust build closure identity changed")
    source_root = _retained_path(
        run_root, record["source_root"], "source root", leaf_kind="directory"
    )
    closure_root = _retained_path(
        run_root, record["closure_root"], "closure root", leaf_kind="directory"
    )
    target_root = _retained_path(
        run_root, record["target_root"], "target root", leaf_kind="directory"
    )
    slots = _integer(record["slots"], "formal build slots", minimum=1)
    build_variables = record["build_variables"]
    if not isinstance(build_variables, Mapping) or not all(
        isinstance(key, str) and isinstance(item, str) for key, item in build_variables.items()
    ):
        raise ClosureError("formal build variables are invalid")
    dependencies = verify_dependency_receipt(
        run_root,
        record["dependencies"],
        portable=portable,
        cache_inputs=cache_inputs,
    )
    toolchain = verify_toolchain_receipt(record["toolchain"], portable=portable)
    recorded_run_root = run_root.resolve()
    if portable:
        recorded_home = Path(str(record["environment"].get("HOME", "")))
        closure_relative = PurePosixPath(record["closure_root"])
        expected_suffix = (*closure_relative.parts, "home")
        if (
            not recorded_home.is_absolute()
            or tuple(recorded_home.parts[-len(expected_suffix) :]) != expected_suffix
        ):
            raise ClosureError("portable formal build HOME does not bind the closure root")
        recorded_run_root = recorded_home
        for _part in expected_suffix:
            recorded_run_root = recorded_run_root.parent
        if recorded_run_root.name != record["run_root_name"]:
            raise ClosureError("portable formal build run-root identity changed")
    expected_environment = closed_build_environment(
        recorded_run_root,
        recorded_run_root / record["closure_root"],
        recorded_run_root / record["target_root"],
        toolchain,
        slots=slots,
        build_variables=build_variables,
    )
    if record["environment"] != expected_environment:
        raise ClosureError("formal build environment is not the complete closed environment")
    if record["environment"]["CARGO_HOME"] != str(
        recorded_run_root / record["closure_root"] / "cargo-home"
    ):
        raise ClosureError("formal build does not use its run-local Cargo home")
    if dependencies["config"]["path"] != (closure_root / "cargo-home/config.toml").relative_to(run_root).as_posix():
        raise ClosureError("formal build Cargo home is not bound to the retained config")
    probe_environment = toolchain["probe_environment"]
    expected_probe_root = recorded_run_root / record["closure_root"] / "toolchain-probe"
    expected_probe_path_dirs: list[str] = []
    for tool_record in [
        toolchain["rust_tools"]["cargo"],
        toolchain["rust_tools"]["rustc"],
        *(toolchain["helpers"][role] for role in REQUIRED_HELPER_ROLES),
        *(toolchain["build_tools"][role] for role in REQUIRED_BUILD_TOOL_ROLES),
        *(toolchain["extra_tools"][role] for role in sorted(toolchain["extra_tools"])),
    ]:
        parent = str(Path(tool_record["resolved_path"]).parent)
        if parent not in expected_probe_path_dirs:
            expected_probe_path_dirs.append(parent)
    if probe_environment != {
        "HOME": str(expected_probe_root / "home"),
        "TMPDIR": str(expected_probe_root / "tmp"),
        "PATH": ":".join(expected_probe_path_dirs),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
    }:
        raise ClosureError("formal toolchain probe environment changed")
    expected_recorded_tool_bin = expected_probe_root / "tool-bin"
    if toolchain["tool_bin"]["path"] != str(expected_recorded_tool_bin):
        raise ClosureError("run-local tool bin is not below the exact closure probe root")
    current_tool_bin = closure_root / "toolchain-probe/tool-bin"
    _verify_tree_record(
        current_tool_bin,
        {
            key: toolchain["tool_bin"][key]
            for key in ("sha256", "entries", "entry_count", "size_bytes")
        },
        "collected run-local tool bin",
        allow_symlinks=False,
        containment_root=run_root,
    )
    return record


def _linux_runtime_commands(
    artifact: Path, toolchain: Mapping[str, Any]
) -> tuple[dict[str, list[str]], dict[str, str]]:
    tool_bin = Path(str(toolchain["tool_bin"]["path"]))
    commands = {
        "program_headers": [
            str(tool_bin / "readelf"),
            "--program-headers",
            "--wide",
            str(artifact),
        ],
        "dynamic": [
            str(tool_bin / "readelf"),
            "--dynamic",
            "--wide",
            str(artifact),
        ],
        "ldd_r": [str(tool_bin / "ldd"), "-r", str(artifact)],
    }
    environment = {
        "PATH": str(tool_bin),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
    }
    return commands, environment


def _parse_elf_runtime(
    program_headers: str, dynamic: str, ldd_output: str
) -> tuple[str | None, list[str], list[tuple[str, Path]]]:
    interpreter_match = re.search(
        r"Requesting program interpreter:\s*([^\]]+)\]", program_headers
    )
    interpreter = interpreter_match.group(1).strip() if interpreter_match else None
    needed = re.findall(r"\(NEEDED\).*Shared library:\s*\[([^\]]+)\]", dynamic)
    if len(needed) != len(set(needed)):
        raise ClosureError("ELF dynamic table repeats a NEEDED entry")
    libraries: list[tuple[str, Path]] = []
    seen: set[tuple[str, str]] = set()
    for raw_line in ldd_output.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        lowered = line.lower()
        if "not found" in lowered or "undefined symbol" in lowered:
            raise ClosureError(f"ldd -r reports an unresolved runtime input: {line}")
        if lowered in {"statically linked", "not a dynamic executable"}:
            raise ClosureError(f"ldd -r did not return a dynamic closure: {line}")
        logical: str | None = None
        path_text: str | None = None
        if "=>" in line:
            left, right = line.split("=>", 1)
            logical = left.strip().split()[0]
            candidate = right.strip().split()[0]
            if candidate.startswith("/"):
                path_text = candidate
        else:
            candidate = line.split()[0]
            if candidate.startswith("/"):
                path_text = candidate
                logical = Path(candidate).name
        if logical is None or path_text is None:
            # Kernel-provided virtual DSOs are intentionally not file inputs.
            if "linux-vdso" in line or "linux-gate" in line:
                continue
            raise ClosureError(f"ldd -r output has an unclosed line: {line}")
        path = Path(path_text)
        key = (logical, str(path))
        if key not in seen:
            seen.add(key)
            libraries.append((logical, path))
    if not libraries:
        raise ClosureError("ldd -r runtime library closure is empty")
    logical_names = {logical for logical, _path in libraries}
    path_names = {path.name for _logical, path in libraries}
    missing = [name for name in needed if name not in logical_names and name not in path_names]
    if missing:
        raise ClosureError(f"ELF NEEDED entries are absent from ldd -r: {missing}")
    loader_paths = [
        path
        for _logical, path in libraries
        if path.name.startswith(("ld-linux", "ld-musl"))
    ]
    if interpreter is not None:
        interpreter_path = Path(interpreter)
        if not interpreter_path.is_absolute():
            raise ClosureError("ELF interpreter is not absolute")
        if interpreter_path not in [path for _logical, path in libraries]:
            libraries.append((interpreter_path.name, interpreter_path))
        loader_paths.append(interpreter_path)
    if not loader_paths:
        raise ClosureError("ELF runtime closure omits its dynamic loader")
    return interpreter, needed, libraries


def _capture_linux_runtime_outputs(
    artifact: Path, toolchain: Mapping[str, Any]
) -> dict[str, Any]:
    commands, environment = _linux_runtime_commands(artifact, toolchain)
    outputs: dict[str, dict[str, Any]] = {}
    for role, command in commands.items():
        stdout, stderr = _command_bytes(
            command, environment, f"Linux runtime {role}"
        )
        outputs[role] = _text_output_record(command, stdout, stderr)
    interpreter, needed, libraries = _parse_elf_runtime(
        outputs["program_headers"]["stdout"],
        outputs["dynamic"]["stdout"],
        outputs["ldd_r"]["stdout"] + outputs["ldd_r"]["stderr"],
    )
    library_records = [
        {
            "logical_name": logical,
            "file": _external_file_identity(path, f"runtime library {logical}"),
        }
        for logical, path in libraries
    ]
    return {
        "commands": commands,
        "environment": environment,
        "outputs": outputs,
        "interpreter": interpreter,
        "needed": needed,
        "runtime_libraries": library_records,
    }


def capture_linux_runtime_closure(
    run_root: Path,
    artifact: Path,
    toolchain: Mapping[str, Any],
) -> dict[str, Any]:
    """Capture the final plugin's interpreter/NEEDED/``ldd -r`` closure."""

    verify_toolchain_receipt(toolchain, portable=False)
    artifact_record = _relative_file_record(run_root, artifact)
    captured = _capture_linux_runtime_outputs(artifact.resolve(), toolchain)
    return {
        "schema_version": 1,
        "receipt_type": LINUX_RUNTIME_RECEIPT_TYPE,
        "artifact": artifact_record,
        "toolchain_sha256": sha256_bytes(canonical_json_bytes(toolchain)),
        **captured,
        "complete_sentinel": "FORMAL_LINUX_RUNTIME_CLOSURE_COMPLETE",
    }


def verify_linux_runtime_closure(
    run_root: Path,
    value: Any,
    toolchain: Mapping[str, Any],
    *,
    portable: bool,
) -> Mapping[str, Any]:
    """Replay a final Linux artifact runtime closure locally or portably."""

    record = _expect_keys(
        value,
        {
            "schema_version",
            "receipt_type",
            "artifact",
            "toolchain_sha256",
            "commands",
            "environment",
            "outputs",
            "interpreter",
            "needed",
            "runtime_libraries",
            "complete_sentinel",
        },
        "formal Linux runtime closure",
    )
    if (
        record["schema_version"] != 1
        or record["receipt_type"] != LINUX_RUNTIME_RECEIPT_TYPE
        or record["complete_sentinel"] != "FORMAL_LINUX_RUNTIME_CLOSURE_COMPLETE"
        or record["toolchain_sha256"]
        != sha256_bytes(canonical_json_bytes(toolchain))
    ):
        raise ClosureError("formal Linux runtime closure identity changed")
    verify_toolchain_receipt(toolchain, portable=portable)
    artifact = _verify_relative_file(run_root, record["artifact"], "runtime artifact")
    expected_commands, expected_environment = _linux_runtime_commands(
        artifact.resolve(), toolchain
    )
    # Portable relocation changes the artifact argument, while every tool path
    # remains the captured external identity.  Normalize only that final argv.
    if portable:
        recorded_artifact = Path(str(record["commands"]["dynamic"][-1]))
        expected_commands = {
            role: [*command[:-1], str(recorded_artifact)]
            for role, command in expected_commands.items()
        }
    if record["commands"] != expected_commands or record["environment"] != expected_environment:
        raise ClosureError("formal Linux runtime command/environment changed")
    outputs = _expect_keys(
        record["outputs"], {"program_headers", "dynamic", "ldd_r"},
        "formal Linux runtime outputs",
    )
    for role, output in outputs.items():
        _verify_text_output_record(output, f"Linux runtime {role} output")
    parsed_interpreter, parsed_needed, parsed_libraries = _parse_elf_runtime(
        outputs["program_headers"]["stdout"],
        outputs["dynamic"]["stdout"],
        outputs["ldd_r"]["stdout"] + outputs["ldd_r"]["stderr"],
    )
    if record["interpreter"] != parsed_interpreter or record["needed"] != parsed_needed:
        raise ClosureError("ELF interpreter/NEEDED receipt changed")
    library_values = record["runtime_libraries"]
    if not isinstance(library_values, list) or len(library_values) != len(parsed_libraries):
        raise ClosureError("runtime library closure count changed")
    recorded_pairs: list[tuple[str, str]] = []
    for index, library_value in enumerate(library_values):
        library = _expect_keys(
            library_value, {"logical_name", "file"}, f"runtime library[{index}]"
        )
        if not isinstance(library["logical_name"], str) or not library["logical_name"]:
            raise ClosureError(f"runtime library[{index}] logical name is invalid")
        _verify_external_file_identity(
            library["file"], f"runtime library[{index}]", portable=portable
        )
        recorded_pairs.append((library["logical_name"], library["file"]["path"]))
    if recorded_pairs != [(logical, str(path)) for logical, path in parsed_libraries]:
        raise ClosureError("runtime library logical/path closure changed")
    if not portable:
        current = _capture_linux_runtime_outputs(artifact.resolve(), toolchain)
        for field in ("commands", "environment", "interpreter", "needed"):
            if current[field] != record[field]:
                raise ClosureError(f"Linux runtime closure changed: {field}")
        current_pairs = [
            (item["logical_name"], item["file"]["path"])
            for item in current["runtime_libraries"]
        ]
        if current_pairs != recorded_pairs:
            raise ClosureError("Linux runtime resolved library closure changed")
        for role in ("program_headers", "dynamic"):
            if current["outputs"][role] != outputs[role]:
                raise ClosureError(f"Linux runtime {role} output changed")
    return record


def verify_qualified_formal_rust_build(
    run_root: Path,
    build_closure: Any,
    runtime_closure: Any,
    *,
    portable: bool,
    cache_inputs: SourceCacheInputs | None = None,
) -> dict[str, Any]:
    """Verify the mandatory pre-build *and* post-build Linux closures together."""

    build = verify_formal_rust_build_closure(
        run_root,
        build_closure,
        portable=portable,
        cache_inputs=cache_inputs,
    )
    runtime = verify_linux_runtime_closure(
        run_root,
        runtime_closure,
        build["toolchain"],
        portable=portable,
    )
    return {
        "schema_version": 1,
        "receipt_type": "qualified-formal-rust-linux-build-v1",
        "build_closure_sha256": sha256_bytes(canonical_json_bytes(build)),
        "runtime_closure_sha256": sha256_bytes(canonical_json_bytes(runtime)),
        "artifact": dict(runtime["artifact"]),
        "status": "complete",
        "complete_sentinel": "QUALIFIED_FORMAL_RUST_LINUX_BUILD_COMPLETE",
    }


__all__ = [
    "CC_DISCOVERED_ROLES",
    "COMPLETE_SENTINEL",
    "ClosureError",
    "DEPENDENCY_RECEIPT_TYPE",
    "LINUX_RUNTIME_RECEIPT_TYPE",
    "RECEIPT_TYPE",
    "REQUIRED_BUILD_TOOL_ROLES",
    "REQUIRED_HELPER_ROLES",
    "SCHEMA_VERSION",
    "SourceCacheInputs",
    "TOOLCHAIN_RECEIPT_TYPE",
    "canonical_json_bytes",
    "capture_toolchain",
    "capture_c_system_closure",
    "capture_linux_runtime_closure",
    "closed_build_environment",
    "compiler_base_image_trust_boundary",
    "default_build_tools",
    "load_cargo_lock",
    "materialize_dependency_closure",
    "materialize_registry_package",
    "prepare_formal_rust_build_closure",
    "sha256_bytes",
    "sha256_file",
    "tree_inventory",
    "validate_dependency_vendor",
    "verify_dependency_receipt",
    "verify_c_system_closure",
    "verify_formal_rust_build_closure",
    "verify_linux_runtime_closure",
    "verify_qualified_formal_rust_build",
    "verify_toolchain_receipt",
]
