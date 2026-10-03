"""Check local package/build inputs before reusing a validated Linux plugin.

This is a development source inventory, not a compiler/environment closure.
The plugin hash and successful build/validation receipt remain mandatory.
"""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath


PACKAGE_DIRECTORIES = ("crates", "legacy", "vendor", "plugin", "stata", ".cargo")
ROOT_INPUTS = ("Cargo.toml", "Cargo.lock", "references.lock.toml",
               "rust-toolchain", "rust-toolchain.toml", "scripts/build_linux.sh")
REQUIRED_INPUTS = ("Cargo.toml", "Cargo.lock", "references.lock.toml",
                   "rust-toolchain.toml", ".cargo/config.toml", "scripts/build_linux.sh",
                   "crates/fereg-core/Cargo.toml", "crates/fereg-core/build.rs",
                   "crates/fereg-core/src/lib.rs", "crates/fereg-plugin/Cargo.toml",
                   "crates/fereg-plugin/src/lib.rs", "plugin/stata_bridge.c",
                   "plugin/stata_bridge.h", "plugin/stplugin.c", "plugin/stplugin.h",
                   "plugin/exports_linux.map", "stata/fereg.ado", "stata/feppml.ado")


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def snapshot(files):
    return hashlib.sha256(json.dumps(files, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def package_files(root):
    """Discover additions as well as changes; omit only generated build products."""
    def unreadable(error):
        raise error

    paths = {root / name for name in ROOT_INPUTS if (root / name).is_file()}
    for directory in PACKAGE_DIRECTORIES:
        base = root / directory
        if base.is_symlink():
            raise ValueError(f"symlink in package input tree: {directory}")
        if not base.exists():
            continue
        for parent, directories, files in os.walk(base, onerror=unreadable):
            directories[:] = [name for name in directories if name not in ("target", "__pycache__", ".git")]
            files = [name for name in files if name != ".DS_Store"]
            for name in (*directories, *files):
                path = Path(parent) / name
                if path.is_symlink():
                    raise ValueError(f"symlink in package input tree: {path.relative_to(root)}")
            paths.update(Path(parent) / name for name in files)
    missing = sorted(name for name in REQUIRED_INPUTS if root / name not in paths)
    if missing:
        raise ValueError("missing required package inputs: " + ", ".join(missing))
    return {path.relative_to(root).as_posix(): sha256(path) for path in sorted(paths)}


def verify_source(root, source):
    files = source["files"]
    if not isinstance(files, dict) or source["snapshot"] != snapshot(files):
        raise ValueError("source snapshot does not bind its file inventory")
    observed = package_files(root)
    omitted = sorted(observed.keys() - files.keys())
    if omitted:
        raise ValueError("source inventory omits package/build inputs: " + ", ".join(omitted))
    # Additional explicitly registered inputs (for example validation fixtures)
    # are also binding, even when outside the package/build discovery roots.
    for name, expected in files.items():
        relative = PurePosixPath(name)
        if relative.is_absolute() or ".." in relative.parts or relative.as_posix() != name:
            raise ValueError(f"invalid source input path: {name}")
        path = root / name
        if not path.resolve().is_relative_to(root.resolve()) or not path.is_file():
            raise ValueError(f"missing or external source input: {name}")
        if sha256(path) != expected:
            raise ValueError(f"source input changed: {name}")
    return source["snapshot"]
